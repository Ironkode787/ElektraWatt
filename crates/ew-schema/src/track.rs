// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The strict reader underneath every record type: the duplicate-rejecting JSON parser, the path-tracking
//! deserialiser over a parsed [`Value`], the tag-first reader of open enums and the structured errors.
//!
//! # How a record is read
//!
//! 1. [`parse_value`] turns the text into a [`Value`] and rejects a repeated member name at any depth
//!    (doc 05 §3.6), with the path of the object.
//! 2. `Record::from_value` (or `Record::load`) runs typed deserialisation over that value through
//!    [`ValueDe`], which records the JSON Pointer of the member it is in. An error raised anywhere below,
//!    by serde, by a foundation type or by this crate, carries that path.
//!
//! The path lives in a thread-local context that only the entry points activate; deserialising an inner
//! type directly with `serde_json` reports errors without a path.
//!
//! # Open enums
//!
//! Every persisted enum is read tag first ([`de_open`]): a string or the key of a one-member object names
//! the variant. A tag that is not a known variant name is data of a newer format and is kept verbatim as
//! `Unknown` (doc 05 §10.3 rule 3). A known tag whose payload does not parse is not newer data, because a
//! known variant's payload never changes within a major: the strict readers reject it as
//! [`SchemaError::MalformedVariant`]; the loader's entry point keeps it verbatim as `Unknown` and returns it
//! as a finding, which `ew-format` reports as EW-CORE-017 (ADR-0011 amendment 2). A value that is neither a
//! string nor a one-member object is a schema violation.

use std::cell::RefCell;
use std::fmt;

use serde::de::value::BorrowedStrDeserializer;
use serde::de::{
    self, DeserializeOwned, DeserializeSeed, Deserializer, EnumAccess, MapAccess, SeqAccess, Unexpected,
    VariantAccess, Visitor,
};
use serde::{Deserialize, forward_to_deserialize_any};
use serde_json::{Map, Number, Value};

use crate::json::check_value;
use crate::{MalformedVariant, SchemaError};

// ---------------------------------------------------------------------------------------------------------
// Context

/// One step of a JSON Pointer.
enum Seg {
    Key(String),
    Index(usize),
}

#[derive(Default)]
struct Context {
    /// Whether an entry point is running; outside one, paths are not tracked.
    active: bool,
    /// Keep malformed known variants as `Unknown` and collect them instead of failing.
    lenient: bool,
    path: Vec<Seg>,
    findings: Vec<MalformedVariant>,
    /// The structured error of the [`raise`] in progress, taken by [`TrackError`]'s `custom`.
    pending: Option<SchemaError>,
    /// The last structured error raised, for entry points whose deserialiser has a foreign error type.
    raised: Option<SchemaError>,
}

thread_local! {
    static CTX: RefCell<Context> = RefCell::new(Context::default());
}

/// Restores the enclosing context when an entry point returns or unwinds.
struct Restore(Option<Context>);

impl Drop for Restore {
    fn drop(&mut self) {
        if let Some(outer) = self.0.take() {
            CTX.with(|c| *c.borrow_mut() = outer);
        }
    }
}

/// Runs `f` in a fresh active context and returns its result with the malformed variants it kept.
pub(crate) fn with_context<R>(lenient: bool, f: impl FnOnce() -> R) -> (R, Vec<MalformedVariant>) {
    let fresh = Context {
        active: true,
        lenient,
        ..Context::default()
    };
    let outer = CTX.with(|c| std::mem::replace(&mut *c.borrow_mut(), fresh));
    let restore = Restore(Some(outer));
    let result = f();
    let findings = CTX.with(|c| std::mem::take(&mut c.borrow_mut().findings));
    drop(restore);
    (result, findings)
}

/// Takes the last structured error raised in the current context.
pub(crate) fn take_raised() -> Option<SchemaError> {
    CTX.with(|c| c.borrow_mut().raised.take())
}

fn push(seg: impl FnOnce() -> Seg) {
    CTX.with(|c| {
        let mut c = c.borrow_mut();
        if c.active {
            let seg = seg();
            c.path.push(seg);
        }
    });
}

fn pop() {
    CTX.with(|c| {
        let mut c = c.borrow_mut();
        if c.active {
            c.path.pop();
        }
    });
}

/// Appends one JSON Pointer reference token (RFC 6901: `~` is `~0`, `/` is `~1`).
pub(crate) fn push_token(out: &mut String, token: &str) {
    out.push('/');
    for ch in token.chars() {
        match ch {
            '~' => out.push_str("~0"),
            '/' => out.push_str("~1"),
            c => out.push(c),
        }
    }
}

/// The JSON Pointer of the value being read; empty at the root and outside an entry point.
pub(crate) fn current_path() -> String {
    CTX.with(|c| {
        let c = c.borrow();
        let mut out = String::new();
        for seg in &c.path {
            match seg {
                Seg::Key(k) => push_token(&mut out, k),
                Seg::Index(i) => push_token(&mut out, &i.to_string()),
            }
        }
        out
    })
}

/// The JSON Pointer of member `key` of the value being read.
pub(crate) fn path_of(key: &str) -> String {
    let mut path = current_path();
    push_token(&mut path, key);
    path
}

fn is_lenient() -> bool {
    CTX.with(|c| {
        let c = c.borrow();
        c.active && c.lenient
    })
}

/// Turns a structured error into the error type of the deserialiser at hand. Under an entry point the
/// structure survives (the deserialiser's error is [`TrackError`]); elsewhere it becomes a message.
pub(crate) fn raise<E: de::Error>(err: SchemaError) -> E {
    CTX.with(|c| {
        let mut c = c.borrow_mut();
        if c.active {
            c.raised = Some(err.clone());
        }
        c.pending = Some(err.clone());
    });
    let e = E::custom(&err);
    CTX.with(|c| c.borrow_mut().pending = None);
    e
}

/// A [`SchemaError::Invalid`] at the current path.
pub(crate) fn invalid(message: impl fmt::Display) -> SchemaError {
    SchemaError::Invalid {
        path: current_path(),
        message: message.to_string(),
    }
}

/// The error type of [`ValueDe`]: a [`SchemaError`] with the path where it arose.
#[derive(Debug)]
pub(crate) struct TrackError(pub(crate) SchemaError);

impl fmt::Display for TrackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for TrackError {}

impl de::Error for TrackError {
    fn custom<T: fmt::Display>(msg: T) -> TrackError {
        let pending = CTX.with(|c| c.borrow_mut().pending.take());
        TrackError(pending.unwrap_or_else(|| invalid(msg)))
    }
}

// ---------------------------------------------------------------------------------------------------------
// Number rule with a path

/// [`check_value`] with the JSON Pointer of the offending number.
pub(crate) fn check_value_at(value: &Value, path: &mut String) -> Result<(), SchemaError> {
    match value {
        Value::Number(_) => check_value(value).map_err(|e| SchemaError::Invalid {
            path: path.clone(),
            message: e.to_string(),
        }),
        Value::Array(items) => items.iter().enumerate().try_for_each(|(i, v)| {
            let len = path.len();
            push_token(path, &i.to_string());
            let r = check_value_at(v, path);
            path.truncate(len);
            r
        }),
        Value::Object(map) => map.iter().try_for_each(|(k, v)| {
            let len = path.len();
            push_token(path, k);
            let r = check_value_at(v, path);
            path.truncate(len);
            r
        }),
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(()),
    }
}

// ---------------------------------------------------------------------------------------------------------
// Strict parser

/// Parses one JSON text into a [`Value`], rejecting a member name repeated within one object at any depth
/// (doc 05 §3.6) as [`SchemaError::DuplicateMember`]; any other syntax error is [`SchemaError::Json`].
///
/// This is the first step of the format loader: `ew-format` relies on it, because a `Value` built by
/// `serde_json` itself keeps the last of two equal members silently.
///
/// ```
/// use ew_schema::{SchemaError, parse_value};
///
/// assert!(parse_value(r#"{"a":{"b":1}}"#).is_ok());
/// let err = parse_value(r#"{"a":{"b":1,"b":2}}"#).unwrap_err();
/// assert_eq!(err, SchemaError::DuplicateMember { path: "/a".into(), key: "b".into() });
/// ```
pub fn parse_value(text: &str) -> Result<Value, SchemaError> {
    with_context(false, || {
        let mut de = serde_json::Deserializer::from_str(text);
        StrictValue
            .deserialize(&mut de)
            .and_then(|value| de.end().map(|()| value))
            .map_err(|e| {
                take_raised().unwrap_or(SchemaError::Json {
                    message: e.to_string(),
                })
            })
    })
    .0
}

/// Reads any deserialiser into a [`Value`] under an active context, rejecting repeated member names.
pub(crate) fn buffer_strict<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Value, D::Error> {
    with_context(false, || StrictValue.deserialize(deserializer)).0
}

/// A [`Value`] seed that rejects repeated member names.
struct StrictValue;

impl<'de> DeserializeSeed<'de> for StrictValue {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(StrictVisitor)
    }
}

struct StrictVisitor;

impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }

    fn visit_f64<E>(self, v: f64) -> Result<Value, E> {
        Ok(Number::from_f64(v).map_or(Value::Null, Value::Number))
    }

    fn visit_str<E>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.to_owned()))
    }

    fn visit_string<E>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }

    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        StrictValue.deserialize(deserializer)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        loop {
            push(|| Seg::Index(items.len()));
            let next = seq.next_element_seed(StrictValue);
            pop();
            match next? {
                Some(v) => items.push(v),
                None => return Ok(Value::Array(items)),
            }
        }
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let mut map = Map::new();
        while let Some(key) = access.next_key::<String>()? {
            if map.contains_key(&key) {
                return Err(raise(SchemaError::DuplicateMember {
                    path: current_path(),
                    key,
                }));
            }
            push(|| Seg::Key(key.clone()));
            let value = access.next_value_seed(StrictValue);
            pop();
            map.insert(key, value?);
        }
        Ok(Value::Object(map))
    }
}

// ---------------------------------------------------------------------------------------------------------
// Path-tracking deserialiser

/// A deserialiser over a borrowed [`Value`] that tracks the JSON Pointer of every member and element it
/// enters. It behaves like `serde_json`'s `&Value` deserialiser, except that a unit variant must be a bare
/// string (`{"dot": null}` is not accepted).
pub(crate) struct ValueDe<'a>(pub(crate) &'a Value);

/// Deserialises `value` at the current path.
pub(crate) fn de_here<T: DeserializeOwned>(value: &Value) -> Result<T, SchemaError> {
    T::deserialize(ValueDe(value)).map_err(|e| e.0)
}

/// Deserialises `value`, the member `key` of the value being read.
pub(crate) fn de_member<T: DeserializeOwned>(key: &str, value: &Value) -> Result<T, SchemaError> {
    push(|| Seg::Key(key.to_owned()));
    let r = de_here(value);
    pop();
    r
}

impl<'de> Deserializer<'de> for ValueDe<'de> {
    type Error = TrackError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, TrackError> {
        match self.0 {
            Value::Null => visitor.visit_unit(),
            Value::Bool(b) => visitor.visit_bool(*b),
            Value::Number(n) => {
                if let Some(u) = n.as_u64() {
                    visitor.visit_u64(u)
                } else if let Some(i) = n.as_i64() {
                    visitor.visit_i64(i)
                } else {
                    visitor.visit_f64(n.as_f64().unwrap_or(f64::NAN))
                }
            }
            Value::String(s) => visitor.visit_borrowed_str(s),
            Value::Array(items) => {
                let mut seq = SeqDe {
                    iter: items.iter(),
                    index: 0,
                };
                let out = visitor.visit_seq(&mut seq)?;
                if seq.iter.len() == 0 {
                    Ok(out)
                } else {
                    Err(de::Error::invalid_length(items.len(), &"fewer elements in array"))
                }
            }
            Value::Object(map) => {
                let mut access = MapDe {
                    iter: map.iter(),
                    value: None,
                };
                let out = visitor.visit_map(&mut access)?;
                if access.iter.len() == 0 {
                    Ok(out)
                } else {
                    Err(de::Error::invalid_length(map.len(), &"fewer elements in map"))
                }
            }
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, TrackError> {
        match self.0 {
            Value::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, TrackError> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, TrackError> {
        match self.0 {
            Value::String(variant) => visitor.visit_enum(EnumDe { variant, value: None }),
            Value::Object(map) if map.len() == 1 => {
                let (variant, value) = map.iter().next().expect("one member");
                visitor.visit_enum(EnumDe {
                    variant,
                    value: Some(value),
                })
            }
            Value::Object(_) => Err(de::Error::invalid_value(
                Unexpected::Map,
                &"a map with a single key",
            )),
            other => Err(de::Error::invalid_type(unexpected(other), &"string or map")),
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf unit
        unit_struct seq tuple tuple_struct map struct identifier ignored_any
    }
}

fn unexpected(value: &Value) -> Unexpected<'_> {
    match value {
        Value::Null => Unexpected::Unit,
        Value::Bool(b) => Unexpected::Bool(*b),
        Value::Number(n) => match (n.as_u64(), n.as_i64()) {
            (Some(u), _) => Unexpected::Unsigned(u),
            (None, Some(i)) => Unexpected::Signed(i),
            _ => Unexpected::Float(n.as_f64().unwrap_or(f64::NAN)),
        },
        Value::String(s) => Unexpected::Str(s),
        Value::Array(_) => Unexpected::Seq,
        Value::Object(_) => Unexpected::Map,
    }
}

struct SeqDe<'a> {
    iter: std::slice::Iter<'a, Value>,
    index: usize,
}

impl<'de> SeqAccess<'de> for SeqDe<'de> {
    type Error = TrackError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, TrackError> {
        let Some(value) = self.iter.next() else {
            return Ok(None);
        };
        let index = self.index;
        self.index += 1;
        push(|| Seg::Index(index));
        let r = seed.deserialize(ValueDe(value));
        pop();
        r.map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

struct MapDe<'a> {
    iter: serde_json::map::Iter<'a>,
    value: Option<(&'a str, &'a Value)>,
}

impl<'de> MapAccess<'de> for MapDe<'de> {
    type Error = TrackError;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, TrackError> {
        let Some((key, value)) = self.iter.next() else {
            return Ok(None);
        };
        self.value = Some((key, value));
        seed.deserialize(BorrowedStrDeserializer::new(key)).map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, TrackError> {
        let Some((key, value)) = self.value.take() else {
            return Err(de::Error::custom("value requested before its key"));
        };
        push(|| Seg::Key(key.to_owned()));
        let r = seed.deserialize(ValueDe(value));
        pop();
        r
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

struct EnumDe<'a> {
    variant: &'a str,
    value: Option<&'a Value>,
}

impl<'de> EnumAccess<'de> for EnumDe<'de> {
    type Error = TrackError;
    type Variant = VariantDe<'de>;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, VariantDe<'de>), TrackError> {
        let variant = seed.deserialize(BorrowedStrDeserializer::new(self.variant))?;
        Ok((
            variant,
            VariantDe {
                variant: self.variant,
                value: self.value,
            },
        ))
    }
}

struct VariantDe<'a> {
    variant: &'a str,
    value: Option<&'a Value>,
}

impl<'de> VariantDe<'de> {
    fn payload<T>(self, f: impl FnOnce(ValueDe<'de>) -> Result<T, TrackError>) -> Result<T, TrackError> {
        let Some(value) = self.value else {
            return Err(de::Error::invalid_type(
                Unexpected::UnitVariant,
                &"a variant with a value",
            ));
        };
        push(|| Seg::Key(self.variant.to_owned()));
        let r = f(ValueDe(value));
        pop();
        r
    }
}

impl<'de> VariantAccess<'de> for VariantDe<'de> {
    type Error = TrackError;

    fn unit_variant(self) -> Result<(), TrackError> {
        match self.value {
            None => Ok(()),
            Some(value) => Err(de::Error::invalid_type(
                unexpected(value),
                &"a unit variant (a bare string)",
            )),
        }
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, TrackError> {
        self.payload(|d| seed.deserialize(d))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, TrackError> {
        self.payload(|d| d.deserialize_seq(visitor))
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, TrackError> {
        self.payload(|d| d.deserialize_map(visitor))
    }
}

// ---------------------------------------------------------------------------------------------------------
// Open enums

/// The error of [`Capture`]: the variant names, or anything else.
#[derive(Debug)]
pub(crate) enum CaptureError {
    Names(&'static [&'static str]),
    Other,
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("variant-name capture")
    }
}

impl std::error::Error for CaptureError {}

impl de::Error for CaptureError {
    fn custom<T: fmt::Display>(_: T) -> CaptureError {
        CaptureError::Other
    }
}

/// A deserialiser that only answers `deserialize_enum`, to learn the variant names of a derived enum.
pub(crate) struct Capture;

impl<'de> Deserializer<'de> for Capture {
    type Error = CaptureError;

    fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, CaptureError> {
        Err(CaptureError::Other)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        variants: &'static [&'static str],
        _: V,
    ) -> Result<V::Value, CaptureError> {
        Err(CaptureError::Names(variants))
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf option unit
        unit_struct newtype_struct seq tuple tuple_struct map struct identifier ignored_any
    }
}

/// The variant names a derived (closed) enum deserialiser accepts.
pub(crate) fn variant_names<T>(
    closed: impl FnOnce(Capture) -> Result<T, CaptureError>,
) -> &'static [&'static str] {
    match closed(Capture) {
        Err(CaptureError::Names(names)) => names,
        _ => &[],
    }
}

/// The tag of an enum value: the string, or the key of a one-member object.
pub(crate) fn tag_of(value: &Value) -> Option<&str> {
    match value {
        Value::String(s) => Some(s),
        Value::Object(map) if map.len() == 1 => map.keys().next().map(String::as_str),
        _ => None,
    }
}

/// Reads any value at the current path, applying the number rule.
pub(crate) fn buffer<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Value, D::Error> {
    let value = Value::deserialize(deserializer)?;
    check_value_at(&value, &mut current_path()).map_err(raise)?;
    Ok(value)
}

/// What to do with a known tag whose payload failed to parse with `inner`: a strict reader fails with
/// [`SchemaError::MalformedVariant`]; the loader keeps the value verbatim as `unknown(value)` and records the
/// finding. A malformed variant found deeper inside is passed on unchanged.
pub(crate) fn malformed<T, E: de::Error>(
    variant: &str,
    inner: SchemaError,
    value: Value,
    unknown: impl FnOnce(Value) -> T,
) -> Result<T, E> {
    if matches!(inner, SchemaError::MalformedVariant(_)) {
        return Err(raise(inner));
    }
    let finding = MalformedVariant {
        path: current_path(),
        variant: variant.to_owned(),
        message: inner.to_string(),
    };
    if is_lenient() {
        CTX.with(|c| {
            let findings = &mut c.borrow_mut().findings;
            // Findings inside this value were recorded by a failed attempt; the whole value is kept now.
            let inner = format!("{}/", finding.path);
            findings.retain(|f| f.path != finding.path && !f.path.starts_with(&inner));
            findings.push(finding);
        });
        Ok(unknown(value))
    } else {
        Err(raise(SchemaError::MalformedVariant(finding)))
    }
}

/// The error for a value that is neither a string nor a one-member object where an enum is expected.
pub(crate) fn not_a_variant<E: de::Error>() -> E {
    raise(invalid("expected a variant name or a one-member object"))
}

/// The tag-first reader of an open enum (doc 05 §10.3 rule 3). `names` are the known variant names,
/// `closed` the derived deserialiser without the catch-all, `unknown` the catch-all.
pub(crate) fn de_open<'de, D, T>(
    deserializer: D,
    names: &'static [&'static str],
    closed: impl FnOnce(ValueDe<'_>) -> Result<T, TrackError>,
    unknown: impl FnOnce(Value) -> T,
) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
{
    let value = buffer(deserializer)?;
    let Some(tag) = tag_of(&value) else {
        return Err(not_a_variant());
    };
    if names.is_empty() {
        // A hand-written foreign enum (`Open<ew_rds::Aspect>`) does not reveal its names: a value it
        // accepts is known, anything else tag-shaped is kept as unknown.
        return Ok(closed(ValueDe(&value)).unwrap_or_else(|_| unknown(value)));
    }
    if !names.contains(&tag) {
        return Ok(unknown(value));
    }
    let tag = tag.to_owned();
    match closed(ValueDe(&value)) {
        Ok(t) => Ok(t),
        Err(TrackError(inner)) => malformed(&tag, inner, value, unknown),
    }
}

/// Implements `Serialize` and the tag-first `Deserialize` of open enums whose derives carry
/// `#[serde(remote = "Self")]` (so the derived code becomes inherent functions) and whose last variant is
/// `#[serde(skip)] Unknown(Value)`.
macro_rules! open_enum {
    ($($ty:ident),* $(,)?) => {$(
        impl ::serde::Serialize for $ty {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                match self {
                    $ty::Unknown(value) => ::serde::Serialize::serialize(value, serializer),
                    known => $ty::serialize(known, serializer),
                }
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $ty {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                $crate::track::de_open(
                    deserializer,
                    $crate::track::variant_names(|d| $ty::deserialize(d)),
                    |d| $ty::deserialize(d),
                    $ty::Unknown,
                )
            }
        }
    )*};
}

pub(crate) use open_enum;
