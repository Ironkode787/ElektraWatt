// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared serde helpers: the `extra` map, the number rule for unknown content, default predicates and the
//! [`Open`] wrapper that gives a foreign enum its `Unknown` catch-all.

use std::borrow::Cow;
use std::collections::BTreeMap;

use ew_mltext::to_nfc;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::SchemaError;
use crate::track::{check_value_at, de_open, path_of, raise, variant_names};

/// The `extra` map of a record or value object: unknown members of a newer format minor and namespaced
/// `plugin.<id>.*` keys (FR-EXT-021). It is flattened last and written in byte-wise key order (doc 05 §3.1).
///
/// A writer never puts a known member name of the same object into it: the member would be written twice.
/// [`Record::validate`](crate::Record::validate) detects it.
pub type Extra = BTreeMap<String, Value>;

/// The largest integer magnitude a persisted integer may have (doc 05 §3.2): 2⁵³ − 1.
pub use ew_units::MAX_SAFE_INTEGER;

/// Checks the number rule of doc 05 §3.2 on unknown content: no floats anywhere and every integer within
/// ±(2⁵³ − 1). Typed members enforce the rule through their own types; this function covers `extra`
/// values and the `Unknown` variants of persisted enums.
///
/// ```
/// use serde_json::json;
///
/// assert!(ew_schema::check_value(&json!({"a": [1, "x", {"b": -2}]})).is_ok());
/// assert!(ew_schema::check_value(&json!({"a": 1.5})).is_err());
/// assert!(ew_schema::check_value(&json!(9007199254740992_u64)).is_err());
/// ```
pub fn check_value(value: &Value) -> Result<(), SchemaError> {
    match value {
        Value::Number(n) => {
            let ok = match (n.as_i64(), n.as_u64()) {
                (Some(i), _) => (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&i),
                (None, Some(u)) => u <= MAX_SAFE_INTEGER as u64,
                (None, None) => false,
            };
            if ok {
                Ok(())
            } else if n.is_f64() {
                Err(SchemaError::Float)
            } else {
                Err(SchemaError::IntegerOutOfRange)
            }
        }
        Value::Array(items) => items.iter().try_for_each(check_value),
        Value::Object(map) => map.values().try_for_each(check_value),
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(()),
    }
}

/// `deserialize_with` for flattened `extra` maps: reads the remaining members and applies
/// [`check_value`].
pub(crate) fn de_extra<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Extra, D::Error> {
    let map = Extra::deserialize(deserializer)?;
    for (key, value) in &map {
        check_value_at(value, &mut path_of(key)).map_err(raise)?;
    }
    Ok(map)
}

/// Normalises every string of `value` to Unicode Normalization Form C, member names included, at every
/// depth, and returns whether anything changed (doc 05 §3.1: "a non-NFC string read from disk is
/// normalised in memory, and its file counts as non-canonical").
///
/// `ew-schema`'s typed readers stay strict ([`MlText`](ew_mltext::MlText) and [`Ident`](ew_mltext::Ident)
/// reject non-NFC text). The format loader (`ew-format`) calls this function on the parsed value of every
/// line **before** typed deserialisation and marks the file for rewriting when it returns `true`, so plain
/// `String` members are covered by the same pass. If normalising a member name would make it equal to
/// another name of the same object, that name is left as it is (the object keeps both members) and the
/// call still returns `true`, because a non-NFC name remains in the value.
///
/// Normalisation can leave a set unsorted or duplicated (`["Lu\u{0308}fter", "Lz"]` becomes
/// `["Lüfter", "Lz"]`, and `"Lüfter"` sorts after `"Lz"` in byte order). The strict reader then rejects the
/// value as [`SchemaError::UnsortedList`](crate::SchemaError::UnsortedList) or `DuplicateKey`. The file is
/// non-canonical (this function returned `true`); `ew-format` re-serialises it sorted and flags it.
///
/// ```
/// use serde_json::json;
///
/// let mut v = json!({"title": {"de-DE": "Lu\u{0308}fter"}, "Lu\u{0308}fter": ["Lu\u{0308}fter"]});
/// assert!(ew_schema::normalize_nfc(&mut v));
/// assert_eq!(v, json!({"title": {"de-DE": "Lüfter"}, "Lüfter": ["Lüfter"]}));
/// assert!(!ew_schema::normalize_nfc(&mut v));
/// ```
pub fn normalize_nfc(value: &mut Value) -> bool {
    match value {
        Value::String(s) => match to_nfc(s) {
            Cow::Borrowed(_) => false,
            Cow::Owned(n) => {
                *s = n;
                true
            }
        },
        Value::Array(items) => items
            .iter_mut()
            .fold(false, |changed, v| normalize_nfc(v) | changed),
        Value::Object(map) => {
            let mut changed = false;
            for v in map.values_mut() {
                changed |= normalize_nfc(v);
            }
            let renames: Vec<(String, String)> = map
                .keys()
                .filter_map(|k| match to_nfc(k) {
                    Cow::Owned(n) => Some((k.clone(), n)),
                    Cow::Borrowed(_) => None,
                })
                .collect();
            for (old, new) in renames {
                if map.contains_key(&new) {
                    // A collision leaves a non-NFC key in place, so the value is still non-canonical.
                    changed = true;
                } else if let Some(v) = map.remove(&old) {
                    map.insert(new, v);
                    changed = true;
                }
            }
            changed
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

/// `skip_serializing_if` predicate: the value equals its type's default.
pub(crate) fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// `skip_serializing_if` predicate for `bool` members whose default is `false`.
pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}

/// `skip_serializing_if` predicate for `bool` members whose default is `true`.
pub(crate) fn is_true(value: &bool) -> bool {
    *value
}

/// `default` for `bool` members whose default is `true`.
pub(crate) fn r#true() -> bool {
    true
}

/// `default` of the per-kind schema version `$v`.
pub(crate) fn v1() -> u16 {
    1
}

/// `skip_serializing_if` predicate of `$v`: omitted when 1 (doc 05 §3.1).
pub(crate) fn is_v1(v: &u16) -> bool {
    *v == 1
}

/// A persisted value of a foreign enum (one defined in a foundation crate without a catch-all), extended
/// with the `Unknown` variant every persisted enum needs (ADR-0011 amendment 1): a value written by a newer
/// minor stays readable and is written back unchanged.
///
/// It is read tag first like every persisted enum when `T` has a derived enum deserialiser: a tag `T` does
/// not name is kept as `Unknown`, a tag it names with a malformed payload is a malformed variant. For a
/// hand-written `T` such as `ew_rds::Aspect`, whose names cannot be learnt, a value `T` accepts is known
/// and any other string or one-member object is kept as `Unknown`.
///
/// ```
/// use ew_rds::Aspect;
/// use ew_schema::Open;
///
/// let known: Open<Aspect> = serde_json::from_str("\"location\"").unwrap();
/// assert_eq!(known, Open::Known(Aspect::Location));
/// let unknown: Open<Aspect> = serde_json::from_str("\"zone\"").unwrap();
/// assert_eq!(serde_json::to_string(&unknown).unwrap(), "\"zone\"");
/// assert!(serde_json::from_str::<Open<Aspect>>("7").is_err());
/// ```
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
#[serde(untagged)]
pub enum Open<T> {
    /// A value this reader understands.
    Known(T),
    /// A value of a newer format, kept verbatim.
    Unknown(Value),
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for Open<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        de_open(
            deserializer,
            variant_names(|d| T::deserialize(d)),
            |d| T::deserialize(d).map(Open::Known),
            Open::Unknown,
        )
    }
}

impl<T> Open<T> {
    /// The known value, if any.
    pub fn known(&self) -> Option<&T> {
        match self {
            Open::Known(t) => Some(t),
            Open::Unknown(_) => None,
        }
    }
}

impl<T> From<T> for Open<T> {
    fn from(value: T) -> Open<T> {
        Open::Known(value)
    }
}
