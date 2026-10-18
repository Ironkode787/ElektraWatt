// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A JSON Schema 2020-12 validator for the subset of keywords that schemars 1.x emits for command
//! parameters, so that `AnyCommand` parameters are rejected with the JSON Pointer of the offending value
//! (doc 17 §2.1) before they are deserialised.
//!
//! Supported: `$ref` (to `#/...` pointers, in practice `#/$defs/<Name>`), `type`, `const`, `enum`,
//! `properties`, `required`, `additionalProperties`, `minProperties`, `maxProperties`, `items`,
//! `prefixItems`, `minItems`, `maxItems`, `uniqueItems`, `minimum`, `maximum`, `exclusiveMinimum`, `exclusiveMaximum`
//! (integer bounds), `minLength`, `maxLength`, `anyOf`, `oneOf`, `allOf`, `not` and the boolean schemas.
//! Annotations (`title`, `description`, `default`, `examples`, `format`, `$schema`, `$id`) and `pattern`
//! are ignored: there is no regex engine, and serde enforces the formats when the parameters are read
//! into the command struct.
//!
//! When no branch of an `anyOf`/`oneOf` matches, the error of the branch the value names is reported: the
//! only branch whose `required` members are all present (an externally tagged variant) or, failing that,
//! the only branch whose `type` accepts the value (`Option<Struct>`). Otherwise the error is "no variant
//! matches" at the value.
//!
//! The validator itself is limit-free; [`check_limits`] bounds the size of an untrusted parameter document
//! (bytes, nesting, array length, string length) and runs before validation. Messages quote at most
//! [`QUOTE_CHARS`] characters of an instance, and are only built for the violation that is returned.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::io;

use serde_json::{Map, Value};

/// The most bytes a parameter document may have, measured as compact JSON.
pub const MAX_PARAM_BYTES: usize = 4 * 1024 * 1024;
/// The deepest nesting of arrays and objects a parameter document may have: a scalar root is depth 0,
/// `[]` is depth 1.
pub const MAX_PARAM_DEPTH: usize = 64;
/// The most items any one array of a parameter document may have.
pub const MAX_PARAM_ITEMS: usize = 1_048_576;
/// The most UTF-8 bytes any one string (object keys included) of a parameter document may have.
pub const MAX_PARAM_STRING: usize = 1024 * 1024;

/// The most characters of an instance rendering that an error message quotes.
const QUOTE_CHARS: usize = 256;

/// The first violation found: where and what.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SchemaViolation {
    /// The JSON Pointer of the offending value in the instance (`""` for the root, `/route/1/0`).
    pub path: String,
    /// What is wrong.
    pub message: String,
}

/// The nesting limit of schema evaluation (a guard against `$ref` cycles).
const MAX_DEPTH: usize = 128;

/// Validates `instance` against `schema` (a root schema with `$defs`). It does not apply the size limits of
/// [`check_limits`].
pub fn validate(schema: &Value, instance: &Value) -> Result<(), SchemaViolation> {
    Validator {
        root: schema,
        quiet: Cell::new(0),
    }
    .check(schema, instance, &Path::Root, 0)
}

/// Checks the size limits of a parameter document ([`MAX_PARAM_DEPTH`], [`MAX_PARAM_ITEMS`],
/// [`MAX_PARAM_STRING`], then [`MAX_PARAM_BYTES`]), reporting the first violation with the JSON Pointer of
/// the offending value (`""` for the byte length of the whole document). Exactly at a bound is accepted.
///
/// One walk bounded to depth `MAX_PARAM_DEPTH + 1` (it cannot overflow the stack), then a serialisation
/// that stops as soon as the compact length exceeds the bound, so a huge document is never fully rendered.
pub fn check_limits(instance: &Value) -> Result<(), SchemaViolation> {
    walk_limits(instance, &Path::Root, 0)?;
    let mut counter = Counting::default();
    if serde_json::to_writer(&mut counter, instance).is_err() || counter.bytes > MAX_PARAM_BYTES {
        return Err(violation(
            &Path::Root,
            format!("the document is longer than the limit of {MAX_PARAM_BYTES} bytes"),
        ));
    }
    Ok(())
}

/// `level` is the number of arrays and objects around `v`, not counting `v`.
fn walk_limits(v: &Value, path: &Path<'_>, level: usize) -> Result<(), SchemaViolation> {
    let too_long = |what: &str, at: &Path<'_>, len: usize| {
        violation(
            at,
            format!("the {what} is {len} bytes long; the limit is {MAX_PARAM_STRING} bytes"),
        )
    };
    match v {
        Value::String(s) if s.len() > MAX_PARAM_STRING => Err(too_long("string", path, s.len())),
        Value::Array(items) => {
            let level = level + 1;
            if level > MAX_PARAM_DEPTH {
                return Err(too_deep(path));
            }
            if items.len() > MAX_PARAM_ITEMS {
                return Err(violation(
                    path,
                    format!(
                        "the array has {} items; the limit is {MAX_PARAM_ITEMS} items",
                        items.len()
                    ),
                ));
            }
            for (i, item) in items.iter().enumerate() {
                walk_limits(item, &Path::Idx(path, i), level)?;
            }
            Ok(())
        }
        Value::Object(members) => {
            let level = level + 1;
            if level > MAX_PARAM_DEPTH {
                return Err(too_deep(path));
            }
            for (key, value) in members {
                let at = Path::Key(path, key);
                if key.len() > MAX_PARAM_STRING {
                    return Err(too_long("object key", &at, key.len()));
                }
                walk_limits(value, &at, level)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn too_deep(path: &Path<'_>) -> SchemaViolation {
    violation(
        path,
        format!("the nesting is deeper than the limit of {MAX_PARAM_DEPTH} levels"),
    )
}

/// An `io::Write` that counts bytes and fails once the count exceeds [`MAX_PARAM_BYTES`].
#[derive(Default)]
struct Counting {
    bytes: usize,
}

impl io::Write for Counting {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes = self.bytes.saturating_add(buf.len());
        if self.bytes > MAX_PARAM_BYTES {
            return Err(io::Error::other("document too long"));
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// An `io::Write` that keeps at most `cap` bytes and fails beyond them.
struct Bounded {
    buf: Vec<u8>,
    cap: usize,
}

impl io::Write for Bounded {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let room = self.cap - self.buf.len();
        if data.len() > room {
            self.buf.extend_from_slice(&data[..room]);
            return Err(io::Error::other("bound reached"));
        }
        self.buf.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Compact JSON of a value for an error message: at most [`QUOTE_CHARS`] characters, then `…`. The value
/// is never fully serialised.
fn render(v: &Value) -> String {
    let mut out = Bounded {
        buf: Vec::new(),
        cap: QUOTE_CHARS * 4,
    };
    let cut = serde_json::to_writer(&mut out, v).is_err();
    let text = String::from_utf8_lossy(&out.buf);
    let mut chars = text.chars();
    let mut s: String = chars.by_ref().take(QUOTE_CHARS).collect();
    if cut || chars.next().is_some() {
        s.push('…');
    }
    s
}

/// A string for an error message, clipped to [`QUOTE_CHARS`] characters.
fn clip(s: &str) -> String {
    let mut chars = s.chars();
    let mut out: String = chars.by_ref().take(QUOTE_CHARS).collect();
    if chars.next().is_some() {
        out.push('…');
    }
    out
}

/// The options of an `enum`, rendered and comma-separated, in total at most about [`QUOTE_CHARS`] characters.
fn render_options(options: &[Value]) -> String {
    let mut out = String::new();
    for (i, option) in options.iter().enumerate() {
        if out.chars().count() >= QUOTE_CHARS {
            out.push_str(", …");
            break;
        }
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&render(option));
    }
    out
}

/// Where a value sits in the instance, as a chain of borrowed steps: building a child costs nothing, the
/// JSON Pointer is only written for the violation that is returned.
#[derive(Clone, Copy)]
enum Path<'a> {
    Root,
    Key(&'a Path<'a>, &'a str),
    Idx(&'a Path<'a>, usize),
}

impl Path<'_> {
    /// The JSON Pointer (`~` is `~0`, `/` is `~1`).
    fn pointer(&self) -> String {
        let mut steps = Vec::new();
        let mut at = self;
        loop {
            match at {
                Path::Root => break,
                Path::Key(parent, key) => {
                    steps.push(Err(*key));
                    at = parent;
                }
                Path::Idx(parent, i) => {
                    steps.push(Ok(*i));
                    at = parent;
                }
            }
        }
        let mut out = String::new();
        for step in steps.into_iter().rev() {
            out.push('/');
            match step {
                Ok(i) => out.push_str(&i.to_string()),
                Err(key) => {
                    for c in key.chars() {
                        match c {
                            '~' => out.push_str("~0"),
                            '/' => out.push_str("~1"),
                            c => out.push(c),
                        }
                    }
                }
            }
        }
        out
    }
}

fn violation(path: &Path<'_>, message: impl Into<String>) -> SchemaViolation {
    SchemaViolation {
        path: path.pointer(),
        message: message.into(),
    }
}

/// The JSON type name of an instance (`integer` for an integral number).
fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn type_matches(expected: &str, v: &Value) -> bool {
    match expected {
        "integer" => matches!(v, Value::Number(n) if n.is_i64() || n.is_u64()),
        "number" => v.is_number(),
        other => type_name(v) == other,
    }
}

/// The names of a `type` keyword (a string or an array of strings).
fn type_names(t: &Value) -> Vec<&str> {
    match t {
        Value::String(s) => vec![s.as_str()],
        Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

/// An integer instance as `i128` (covers `i64` and `u64`).
fn int(v: &Value) -> Option<i128> {
    let n = v.as_number()?;
    n.as_i64().map(i128::from).or_else(|| n.as_u64().map(i128::from))
}

struct Validator<'a> {
    root: &'a Value,
    /// Non-zero while only the verdict matters (the probing of `anyOf`/`oneOf`/`not` branches): violations
    /// are returned empty, without building paths or messages.
    quiet: Cell<u32>,
}

impl Validator<'_> {
    /// A violation whose path and message are built only when someone will read them.
    fn fail(&self, path: &Path<'_>, message: impl FnOnce() -> String) -> SchemaViolation {
        if self.quiet.get() > 0 {
            SchemaViolation {
                path: String::new(),
                message: String::new(),
            }
        } else {
            violation(path, message())
        }
    }

    /// Runs `f` with violations suppressed.
    fn quietly<T>(&self, f: impl FnOnce() -> T) -> T {
        self.quiet.set(self.quiet.get() + 1);
        let out = f();
        self.quiet.set(self.quiet.get() - 1);
        out
    }

    fn resolve(&self, reference: &str) -> Option<&Value> {
        let pointer = reference.strip_prefix('#')?;
        self.root.pointer(pointer)
    }

    /// Follows `$ref`s of a schema to the schema that carries the keywords.
    fn deref<'s>(&'s self, mut schema: &'s Value) -> &'s Value {
        for _ in 0..MAX_DEPTH {
            match schema
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|r| self.resolve(r))
            {
                Some(next) => schema = next,
                None => break,
            }
        }
        schema
    }

    fn check(
        &self,
        schema: &Value,
        inst: &Value,
        path: &Path<'_>,
        depth: usize,
    ) -> Result<(), SchemaViolation> {
        if depth > MAX_DEPTH {
            return Err(self.fail(path, || "schema nesting too deep".into()));
        }
        let map = match schema {
            Value::Bool(true) => return Ok(()),
            Value::Bool(false) => return Err(self.fail(path, || "no value is allowed here".into())),
            Value::Object(map) => map,
            _ => return Err(self.fail(path, || "malformed schema".into())),
        };
        if let Some(r) = map.get("$ref") {
            let target = r
                .as_str()
                .and_then(|r| self.resolve(r))
                .ok_or_else(|| self.fail(path, || format!("unresolved $ref {}", render(r))))?;
            self.check(target, inst, path, depth + 1)?;
        }
        if let Some(t) = map.get("type") {
            self.check_type(t, inst, path)?;
        }
        if let Some(c) = map.get("const")
            && c != inst
        {
            return Err(self.fail(path, || {
                format!("expected the constant {}, found {}", render(c), render(inst))
            }));
        }
        if let Some(Value::Array(options)) = map.get("enum")
            && !options.contains(inst)
        {
            return Err(self.fail(path, || {
                format!("{} is not one of {}", render(inst), render_options(options))
            }));
        }
        if let Some(Value::Array(all)) = map.get("allOf") {
            for s in all {
                self.check(s, inst, path, depth + 1)?;
            }
        }
        if let Some(Value::Array(any)) = map.get("anyOf") {
            self.check_any(any, inst, path, depth, false)?;
        }
        if let Some(Value::Array(one)) = map.get("oneOf") {
            self.check_any(one, inst, path, depth, true)?;
        }
        if let Some(not) = map.get("not")
            && self.quietly(|| self.check(not, inst, path, depth + 1)).is_ok()
        {
            return Err(self.fail(path, || "the value matches a schema it must not match".into()));
        }
        match inst {
            Value::Object(members) => self.check_object(map, members, path, depth)?,
            Value::Array(items) => self.check_array(map, items, path, depth)?,
            Value::String(s) => self.check_string(map, s, path)?,
            Value::Number(_) => self.check_number(map, inst, path)?,
            _ => {}
        }
        Ok(())
    }

    /// `anyOf` (`exactly_one` false) and `oneOf`. The branches are probed quietly: `anyOf` stops at the
    /// first match, `oneOf` at the second. Only when none matches is the one branch the value names
    /// evaluated again, this time to report its violation.
    fn check_any(
        &self,
        branches: &[Value],
        inst: &Value,
        path: &Path<'_>,
        depth: usize,
        exactly_one: bool,
    ) -> Result<(), SchemaViolation> {
        let mut matching = 0usize;
        self.quietly(|| {
            for branch in branches {
                if self.check(branch, inst, path, depth + 1).is_ok() {
                    matching += 1;
                    if !exactly_one || matching > 1 {
                        break;
                    }
                }
            }
        });
        if matching == 1 || (matching > 0 && !exactly_one) {
            return Ok(());
        }
        if matching > 1 {
            return Err(self.fail(path, || "the value matches more than one variant".into()));
        }
        // No branch matches: report the branch the instance names, if exactly one.
        let named: Vec<usize> = (0..branches.len())
            .filter(|i| self.selects(&branches[*i], inst))
            .collect();
        let by_type: Vec<usize> = (0..branches.len())
            .filter(|i| self.accepts_type(&branches[*i], inst))
            .collect();
        if let ([i], _) | ([], [i]) = (named.as_slice(), by_type.as_slice())
            && let Err(e) = self.check(&branches[*i], inst, path, depth + 1)
        {
            return Err(e);
        }
        Err(self.fail(path, || {
            format!("no variant of the {} matches the value", type_name(inst))
        }))
    }

    /// Whether a branch is the one the instance names: an object with all `required` members of the
    /// branch (an externally tagged variant), or a value equal to the branch's `const`/`enum`.
    fn selects(&self, branch: &Value, inst: &Value) -> bool {
        let b = self.deref(branch);
        if let Some(c) = b.get("const") {
            return c == inst;
        }
        if let Some(Value::Array(options)) = b.get("enum") {
            return options.contains(inst);
        }
        match (inst, b.get("required")) {
            (Value::Object(members), Some(Value::Array(req))) if !req.is_empty() => req
                .iter()
                .all(|r| r.as_str().is_some_and(|r| members.contains_key(r))),
            _ => false,
        }
    }

    /// Whether a branch declares a `type` that the instance has.
    fn accepts_type(&self, branch: &Value, inst: &Value) -> bool {
        self.deref(branch)
            .get("type")
            .is_some_and(|t| type_names(t).iter().any(|n| type_matches(n, inst)))
    }

    fn check_object(
        &self,
        schema: &Map<String, Value>,
        members: &Map<String, Value>,
        path: &Path<'_>,
        depth: usize,
    ) -> Result<(), SchemaViolation> {
        if let Some(Value::Array(required)) = schema.get("required") {
            for r in required.iter().filter_map(Value::as_str) {
                if !members.contains_key(r) {
                    return Err(self.fail(path, || format!("missing required member `{}`", clip(r))));
                }
            }
        }
        if let Some(min) = schema.get("minProperties").and_then(Value::as_u64)
            && (members.len() as u64) < min
        {
            return Err(self.fail(path, || {
                format!("expected at least {min} members, found {}", members.len())
            }));
        }
        if let Some(max) = schema.get("maxProperties").and_then(Value::as_u64)
            && (members.len() as u64) > max
        {
            return Err(self.fail(path, || {
                format!("expected at most {max} members, found {}", members.len())
            }));
        }
        let properties = schema.get("properties").and_then(Value::as_object);
        let additional = schema.get("additionalProperties");
        for (name, value) in members {
            let at = Path::Key(path, name);
            if let Some(sub) = properties.and_then(|p| p.get(name)) {
                self.check(sub, value, &at, depth + 1)?;
            } else {
                match additional {
                    Some(Value::Bool(false)) => {
                        return Err(self.fail(&at, || format!("unknown member `{}`", clip(name))));
                    }
                    Some(sub @ Value::Object(_)) => self.check(sub, value, &at, depth + 1)?,
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn check_array(
        &self,
        schema: &Map<String, Value>,
        items: &[Value],
        path: &Path<'_>,
        depth: usize,
    ) -> Result<(), SchemaViolation> {
        if let Some(min) = schema.get("minItems").and_then(Value::as_u64)
            && (items.len() as u64) < min
        {
            return Err(self.fail(path, || {
                format!("expected at least {min} items, found {}", items.len())
            }));
        }
        if let Some(max) = schema.get("maxItems").and_then(Value::as_u64)
            && (items.len() as u64) > max
        {
            return Err(self.fail(path, || {
                format!("expected at most {max} items, found {}", items.len())
            }));
        }
        if schema.get("uniqueItems") == Some(&Value::Bool(true)) {
            let mut seen = BTreeSet::new();
            for (i, item) in items.iter().enumerate() {
                if !seen.insert(item.to_string()) {
                    return Err(self.fail(&Path::Idx(path, i), || {
                        format!("item {i} repeats an earlier item")
                    }));
                }
            }
        }
        let prefix = schema.get("prefixItems").and_then(Value::as_array);
        let prefix_len = prefix.map_or(0, Vec::len);
        for (i, item) in items.iter().enumerate() {
            let at = Path::Idx(path, i);
            if let Some(sub) = prefix.and_then(|p| p.get(i)) {
                self.check(sub, item, &at, depth + 1)?;
            } else if i >= prefix_len {
                match schema.get("items") {
                    Some(Value::Bool(false)) => {
                        return Err(self.fail(&at, || format!("unexpected item at index {i}")));
                    }
                    Some(sub) => self.check(sub, item, &at, depth + 1)?,
                    None => {}
                }
            }
        }
        Ok(())
    }

    fn check_type(&self, t: &Value, inst: &Value, path: &Path<'_>) -> Result<(), SchemaViolation> {
        let names = type_names(t);
        if names.is_empty() || names.iter().any(|n| type_matches(n, inst)) {
            return Ok(());
        }
        Err(self.fail(path, || {
            format!("expected {}, found {}", names.join(" or "), type_name(inst))
        }))
    }

    fn check_string(
        &self,
        schema: &Map<String, Value>,
        s: &str,
        path: &Path<'_>,
    ) -> Result<(), SchemaViolation> {
        let min = schema.get("minLength").and_then(Value::as_u64);
        let max = schema.get("maxLength").and_then(Value::as_u64);
        if min.is_none() && max.is_none() {
            return Ok(());
        }
        let len = s.chars().count() as u64;
        if let Some(min) = min
            && len < min
        {
            return Err(self.fail(path, || {
                format!("expected at least {min} characters, found {len}")
            }));
        }
        if let Some(max) = max
            && len > max
        {
            return Err(self.fail(path, || format!("expected at most {max} characters, found {len}")));
        }
        Ok(())
    }

    fn check_number(
        &self,
        schema: &Map<String, Value>,
        inst: &Value,
        path: &Path<'_>,
    ) -> Result<(), SchemaViolation> {
        let Some(v) = int(inst) else {
            return Ok(());
        };
        let bound = |key: &str| schema.get(key).and_then(int);
        if let Some(b) = bound("minimum")
            && v < b
        {
            return Err(self.fail(path, || format!("{v} is below the minimum {b}")));
        }
        if let Some(b) = bound("maximum")
            && v > b
        {
            return Err(self.fail(path, || format!("{v} is above the maximum {b}")));
        }
        if let Some(b) = bound("exclusiveMinimum")
            && v <= b
        {
            return Err(self.fail(path, || format!("{v} is not above {b}")));
        }
        if let Some(b) = bound("exclusiveMaximum")
            && v >= b
        {
            return Err(self.fail(path, || format!("{v} is not below {b}")));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn err(schema: Value, inst: Value) -> (String, String) {
        let e = validate(&schema, &inst).expect_err("must be rejected");
        (e.path, e.message)
    }

    fn ok(schema: Value, inst: Value) {
        assert_eq!(validate(&schema, &inst), Ok(()));
    }

    #[test]
    fn boolean_schemas() {
        ok(json!(true), json!({"a": 1}));
        assert_eq!(err(json!(false), json!(1)).0, "");
    }

    #[test]
    fn type_string_and_array() {
        ok(json!({"type": "string"}), json!("x"));
        let (p, m) = err(json!({"type": "integer"}), json!("x"));
        assert_eq!((p.as_str(), m.as_str()), ("", "expected integer, found string"));
        ok(json!({"type": ["string", "null"]}), json!(null));
        assert!(
            err(json!({"type": ["string", "null"]}), json!(1))
                .1
                .contains("string or null")
        );
    }

    #[test]
    fn integer_versus_number() {
        ok(json!({"type": "number"}), json!(3));
        ok(json!({"type": "integer"}), json!(-3));
        ok(json!({"type": "integer"}), json!(u64::MAX));
        assert!(
            err(json!({"type": "integer"}), json!(1.5))
                .1
                .contains("found number")
        );
        assert!(
            err(json!({"type": "boolean"}), json!(0))
                .1
                .contains("found integer")
        );
    }

    #[test]
    fn const_and_enum() {
        ok(json!({"const": "a"}), json!("a"));
        assert!(err(json!({"const": "a"}), json!("b")).1.contains("constant"));
        ok(json!({"enum": ["a", "b"]}), json!("b"));
        assert!(
            err(json!({"enum": ["a", "b"]}), json!("c"))
                .1
                .contains("not one of")
        );
    }

    #[test]
    fn required_and_unknown_members() {
        let s = json!({
            "type": "object",
            "properties": {"page": {"type": "string"}},
            "required": ["page"],
            "additionalProperties": false
        });
        ok(s.clone(), json!({"page": "p"}));
        let (p, m) = err(s.clone(), json!({}));
        assert_eq!((p.as_str(), m.as_str()), ("", "missing required member `page`"));
        let (p, m) = err(s, json!({"page": "p", "foo": 1}));
        assert_eq!((p.as_str(), m.as_str()), ("/foo", "unknown member `foo`"));
    }

    #[test]
    fn additional_properties_schema() {
        let s = json!({"type": "object", "additionalProperties": {"type": "integer"}});
        ok(s.clone(), json!({"a": 1, "b": 2}));
        assert_eq!(err(s, json!({"a": 1, "b": "x"})).0, "/b");
    }

    #[test]
    fn member_count_limits() {
        let s = json!({"type": "object", "minProperties": 1, "maxProperties": 2});
        assert!(err(s.clone(), json!({})).1.contains("at least 1"));
        assert!(
            err(s.clone(), json!({"a": 1, "b": 2, "c": 3}))
                .1
                .contains("at most 2")
        );
        ok(s, json!({"a": 1}));
    }

    #[test]
    fn nested_pointer_with_escapes() {
        let s = json!({
            "type": "object",
            "properties": {"a/b": {"type": "object", "properties": {"~c": {
                "type": "array", "items": {"type": "integer"}}}}}
        });
        ok(s.clone(), json!({"a/b": {"~c": [1, 2]}}));
        assert_eq!(err(s, json!({"a/b": {"~c": [1, "x"]}})).0, "/a~1b/~0c/1");
    }

    #[test]
    fn items_prefix_items_and_limits() {
        let tuple = json!({
            "type": "array",
            "prefixItems": [{"type": "integer"}, {"type": "string"}],
            "minItems": 2,
            "maxItems": 2,
            "items": false
        });
        ok(tuple.clone(), json!([1, "a"]));
        assert_eq!(err(tuple.clone(), json!([1, 2])).0, "/1");
        assert!(err(tuple.clone(), json!([1])).1.contains("at least 2"));
        assert!(err(tuple.clone(), json!([1, "a", 3])).1.contains("at most 2"));
        let no_max = json!({"type": "array", "prefixItems": [{"type": "integer"}], "items": false});
        assert_eq!(
            err(no_max, json!([1, 2])),
            ("/1".into(), "unexpected item at index 1".into())
        );
        let open =
            json!({"type": "array", "prefixItems": [{"type": "integer"}], "items": {"type": "string"}});
        ok(open.clone(), json!([1, "x", "y"]));
        assert_eq!(err(open, json!([1, "x", 2])).0, "/2");
    }

    #[test]
    fn integer_bounds() {
        let s = json!({"type": "integer", "minimum": 0, "maximum": 10});
        ok(s.clone(), json!(0));
        assert!(err(s.clone(), json!(-1)).1.contains("minimum"));
        assert!(err(s, json!(11)).1.contains("maximum"));
        let x = json!({"exclusiveMinimum": 0, "exclusiveMaximum": 5});
        assert!(err(x.clone(), json!(0)).1.contains("not above"));
        assert!(err(x.clone(), json!(5)).1.contains("not below"));
        ok(x, json!(3));
    }

    #[test]
    fn string_length_counts_chars() {
        let s = json!({"type": "string", "minLength": 2, "maxLength": 3});
        ok(s.clone(), json!("äö"));
        assert!(err(s.clone(), json!("a")).1.contains("at least 2"));
        assert!(err(s, json!("abcd")).1.contains("at most 3"));
    }

    #[test]
    fn annotations_and_pattern_are_ignored() {
        let s = json!({"type": "string", "title": "t", "description": "d", "format": "uuid",
            "pattern": "^x$", "default": "q", "examples": ["e"], "$schema": "s", "$id": "i"});
        ok(s, json!("anything"));
    }

    #[test]
    fn refs_resolve_against_the_root_defs() {
        let s = json!({
            "$defs": {"N": {"type": "integer"}},
            "type": "object",
            "properties": {"n": {"$ref": "#/$defs/N"}}
        });
        ok(s.clone(), json!({"n": 1}));
        assert_eq!(
            err(s, json!({"n": "x"})),
            ("/n".into(), "expected integer, found string".into())
        );
        let bad = json!({"$ref": "#/$defs/Missing"});
        assert!(err(bad, json!(1)).1.contains("unresolved"));
    }

    #[test]
    fn all_of_and_not() {
        let s = json!({"allOf": [{"type": "integer"}, {"minimum": 3}]});
        ok(s.clone(), json!(4));
        assert!(err(s, json!(2)).1.contains("minimum"));
        let n = json!({"not": {"type": "string"}});
        ok(n.clone(), json!(1));
        assert!(err(n, json!("x")).1.contains("must not match"));
    }

    #[test]
    fn any_of_and_one_of() {
        let any = json!({"anyOf": [{"type": "string"}, {"type": "null"}]});
        ok(any.clone(), json!(null));
        assert!(err(any, json!(1)).1.contains("no variant"));
        let one = json!({"oneOf": [{"type": "integer"}, {"minimum": 0}]});
        ok(one.clone(), json!(-1));
        assert!(err(one, json!(1)).1.contains("more than one"));
    }

    #[test]
    fn externally_tagged_variant_reports_its_own_error() {
        let s = json!({
            "$defs": {"Cp": {"type": "object", "properties": {"idx": {"type": "integer"}},
                "required": ["idx"], "additionalProperties": false}},
            "oneOf": [
                {"type": "object", "properties": {"cp": {"$ref": "#/$defs/Cp"}},
                    "required": ["cp"], "additionalProperties": false},
                {"type": "object", "properties": {"junction": {"type": "string"}},
                    "required": ["junction"], "additionalProperties": false}
            ]
        });
        ok(s.clone(), json!({"cp": {"idx": 1}}));
        assert_eq!(
            err(s.clone(), json!({"cp": {"idx": "x"}})),
            ("/cp/idx".into(), "expected integer, found string".into())
        );
        let (p, m) = err(s, json!({"other": 1}));
        assert_eq!(p, "");
        assert!(m.contains("no variant"), "{m}");
    }

    #[test]
    fn string_variants_without_a_match() {
        let s = json!({"oneOf": [{"type": "string", "const": "a"}, {"type": "string", "const": "b"}]});
        ok(s.clone(), json!("b"));
        let (p, m) = err(s, json!("c"));
        assert_eq!(p, "");
        assert!(m.contains("no variant"), "{m}");
    }

    #[test]
    fn single_applicable_branch_by_type_reports_its_error() {
        let s = json!({"anyOf": [
            {"type": "object", "properties": {"a": {"type": "integer"}}, "required": ["a"]},
            {"type": "null"}
        ]});
        ok(s.clone(), json!(null));
        assert_eq!(err(s, json!({"a": "x"})).0, "/a");
    }

    #[test]
    fn render_is_bounded() {
        assert_eq!(render(&json!({"a": [1, "x"]})), r#"{"a":[1,"x"]}"#);
        let long = render(&json!("é".repeat(10_000)));
        assert_eq!(long.chars().count(), QUOTE_CHARS + 1);
        assert!(long.ends_with('…'));
        let exact = render(&json!("x".repeat(QUOTE_CHARS - 2)));
        assert_eq!(exact.chars().count(), QUOTE_CHARS);
        assert!(!exact.ends_with('…'));
    }

    #[test]
    fn probed_branches_are_quiet_and_the_chosen_branch_reports() {
        let s = json!({"anyOf": [
            {"type": "object", "properties": {"a": {"type": "integer"}}, "required": ["a"]},
            {"type": "null"}
        ]});
        assert_eq!(
            err(s, json!({"a": "x"})),
            ("/a".into(), "expected integer, found string".into())
        );
    }
}
