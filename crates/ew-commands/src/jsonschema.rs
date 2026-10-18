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

use std::collections::BTreeSet;

use serde_json::{Map, Value};

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

/// Validates `instance` against `schema` (a root schema with `$defs`).
pub fn validate(schema: &Value, instance: &Value) -> Result<(), SchemaViolation> {
    Validator { root: schema }.check(schema, instance, "", 0)
}

fn violation(path: &str, message: impl Into<String>) -> SchemaViolation {
    SchemaViolation {
        path: path.to_string(),
        message: message.into(),
    }
}

/// The JSON Pointer of a member below `path` (`~` is `~0`, `/` is `~1`).
fn child(path: &str, token: &str) -> String {
    let mut out = String::with_capacity(path.len() + token.len() + 1);
    out.push_str(path);
    out.push('/');
    for c in token.chars() {
        match c {
            '~' => out.push_str("~0"),
            '/' => out.push_str("~1"),
            c => out.push(c),
        }
    }
    out
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
}

impl Validator<'_> {
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

    fn check(&self, schema: &Value, inst: &Value, path: &str, depth: usize) -> Result<(), SchemaViolation> {
        if depth > MAX_DEPTH {
            return Err(violation(path, "schema nesting too deep"));
        }
        let map = match schema {
            Value::Bool(true) => return Ok(()),
            Value::Bool(false) => return Err(violation(path, "no value is allowed here")),
            Value::Object(map) => map,
            _ => return Err(violation(path, "malformed schema")),
        };
        if let Some(r) = map.get("$ref") {
            let target = r
                .as_str()
                .and_then(|r| self.resolve(r))
                .ok_or_else(|| violation(path, format!("unresolved $ref {r}")))?;
            self.check(target, inst, path, depth + 1)?;
        }
        if let Some(t) = map.get("type") {
            check_type(t, inst, path)?;
        }
        if let Some(c) = map.get("const")
            && c != inst
        {
            return Err(violation(
                path,
                format!("expected the constant {c}, found {inst}"),
            ));
        }
        if let Some(Value::Array(options)) = map.get("enum")
            && !options.contains(inst)
        {
            return Err(violation(
                path,
                format!("{inst} is not one of {}", join_values(options)),
            ));
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
            && self.check(not, inst, path, depth + 1).is_ok()
        {
            return Err(violation(path, "the value matches a schema it must not match"));
        }
        match inst {
            Value::Object(members) => self.check_object(map, members, path, depth)?,
            Value::Array(items) => self.check_array(map, items, path, depth)?,
            Value::String(s) => check_string(map, s, path)?,
            Value::Number(_) => check_number(map, inst, path)?,
            _ => {}
        }
        Ok(())
    }

    fn check_any(
        &self,
        branches: &[Value],
        inst: &Value,
        path: &str,
        depth: usize,
        exactly_one: bool,
    ) -> Result<(), SchemaViolation> {
        let results: Vec<Result<(), SchemaViolation>> = branches
            .iter()
            .map(|b| self.check(b, inst, path, depth + 1))
            .collect();
        let matching = results.iter().filter(|r| r.is_ok()).count();
        if matching == 1 || (matching > 0 && !exactly_one) {
            return Ok(());
        }
        if matching > 1 {
            return Err(violation(path, "the value matches more than one variant"));
        }
        // No branch matches: report the branch the instance names, if exactly one.
        let named: Vec<usize> = (0..branches.len())
            .filter(|i| self.selects(&branches[*i], inst))
            .collect();
        let by_type: Vec<usize> = (0..branches.len())
            .filter(|i| self.accepts_type(&branches[*i], inst))
            .collect();
        let pick = match (named.as_slice(), by_type.as_slice()) {
            ([i], _) | ([], [i]) => Some(*i),
            _ => None,
        };
        if let Some(i) = pick
            && let Some(Err(e)) = results.into_iter().nth(i)
        {
            return Err(e);
        }
        Err(violation(
            path,
            format!("no variant of the {} matches the value", type_name(inst)),
        ))
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
        path: &str,
        depth: usize,
    ) -> Result<(), SchemaViolation> {
        if let Some(Value::Array(required)) = schema.get("required") {
            for r in required.iter().filter_map(Value::as_str) {
                if !members.contains_key(r) {
                    return Err(violation(path, format!("missing required member `{r}`")));
                }
            }
        }
        if let Some(min) = schema.get("minProperties").and_then(Value::as_u64)
            && (members.len() as u64) < min
        {
            return Err(violation(
                path,
                format!("expected at least {min} members, found {}", members.len()),
            ));
        }
        if let Some(max) = schema.get("maxProperties").and_then(Value::as_u64)
            && (members.len() as u64) > max
        {
            return Err(violation(
                path,
                format!("expected at most {max} members, found {}", members.len()),
            ));
        }
        let properties = schema.get("properties").and_then(Value::as_object);
        let additional = schema.get("additionalProperties");
        for (name, value) in members {
            let at = child(path, name);
            if let Some(sub) = properties.and_then(|p| p.get(name)) {
                self.check(sub, value, &at, depth + 1)?;
            } else {
                match additional {
                    Some(Value::Bool(false)) => {
                        return Err(violation(&at, format!("unknown member `{name}`")));
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
        path: &str,
        depth: usize,
    ) -> Result<(), SchemaViolation> {
        if let Some(min) = schema.get("minItems").and_then(Value::as_u64)
            && (items.len() as u64) < min
        {
            return Err(violation(
                path,
                format!("expected at least {min} items, found {}", items.len()),
            ));
        }
        if let Some(max) = schema.get("maxItems").and_then(Value::as_u64)
            && (items.len() as u64) > max
        {
            return Err(violation(
                path,
                format!("expected at most {max} items, found {}", items.len()),
            ));
        }
        if schema.get("uniqueItems") == Some(&Value::Bool(true)) {
            let mut seen = BTreeSet::new();
            for (i, item) in items.iter().enumerate() {
                if !seen.insert(item.to_string()) {
                    return Err(violation(
                        &child(path, &i.to_string()),
                        format!("item {i} repeats an earlier item"),
                    ));
                }
            }
        }
        let prefix = schema.get("prefixItems").and_then(Value::as_array);
        let prefix_len = prefix.map_or(0, Vec::len);
        for (i, item) in items.iter().enumerate() {
            let at = child(path, &i.to_string());
            if let Some(sub) = prefix.and_then(|p| p.get(i)) {
                self.check(sub, item, &at, depth + 1)?;
            } else if i >= prefix_len {
                match schema.get("items") {
                    Some(Value::Bool(false)) => {
                        return Err(violation(&at, format!("unexpected item at index {i}")));
                    }
                    Some(sub) => self.check(sub, item, &at, depth + 1)?,
                    None => {}
                }
            }
        }
        Ok(())
    }
}

fn join_values(values: &[Value]) -> String {
    let parts: Vec<String> = values.iter().map(Value::to_string).collect();
    parts.join(", ")
}

fn check_type(t: &Value, inst: &Value, path: &str) -> Result<(), SchemaViolation> {
    let names = type_names(t);
    if names.is_empty() || names.iter().any(|n| type_matches(n, inst)) {
        return Ok(());
    }
    Err(violation(
        path,
        format!("expected {}, found {}", names.join(" or "), type_name(inst)),
    ))
}

fn check_string(schema: &Map<String, Value>, s: &str, path: &str) -> Result<(), SchemaViolation> {
    let len = s.chars().count() as u64;
    if let Some(min) = schema.get("minLength").and_then(Value::as_u64)
        && len < min
    {
        return Err(violation(
            path,
            format!("expected at least {min} characters, found {len}"),
        ));
    }
    if let Some(max) = schema.get("maxLength").and_then(Value::as_u64)
        && len > max
    {
        return Err(violation(
            path,
            format!("expected at most {max} characters, found {len}"),
        ));
    }
    Ok(())
}

fn check_number(schema: &Map<String, Value>, inst: &Value, path: &str) -> Result<(), SchemaViolation> {
    let Some(v) = int(inst) else {
        return Ok(());
    };
    let bound = |key: &str| schema.get(key).and_then(int);
    if let Some(b) = bound("minimum")
        && v < b
    {
        return Err(violation(path, format!("{v} is below the minimum {b}")));
    }
    if let Some(b) = bound("maximum")
        && v > b
    {
        return Err(violation(path, format!("{v} is above the maximum {b}")));
    }
    if let Some(b) = bound("exclusiveMinimum")
        && v <= b
    {
        return Err(violation(path, format!("{v} is not above {b}")));
    }
    if let Some(b) = bound("exclusiveMaximum")
        && v >= b
    {
        return Err(violation(path, format!("{v} is not below {b}")));
    }
    Ok(())
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
}
