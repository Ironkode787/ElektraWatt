// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The schema-driven default arm: one generator that builds a step for any registered command from the
//! command's parameter schema (`CommandRegistry::get(..).schema()`), so a new command is fuzzed without a hand arm.
//!
//! Values follow the schema: by type, within the bounds (`minimum`/`maximum`, the `format` of an unsigned or
//! sized integer, `minLength`/`maxLength`, `minItems`/`maxItems`), from `enum` and `const`, through `$ref`,
//! `oneOf`/`anyOf` (one branch) and `allOf` (merged). A string with a known `pattern` (a UUID, an identifier, a
//! decimal, a language tag, a date, a hash, a colour) gets a value of that shape, so that the step also passes
//! the typed parse after the schema check. A UUID is an ID drawn from every live record and library copy
//! ([`super::live::Live::all_ids`]); where the schema names the record kind (a `kind` member beside an `id`, as
//! in `edit.delete`'s targets) or the member is named after a kind (`page`, `reps`, `device`), the ID is of that
//! kind three times in four; one draw in sixteen is a fresh UUID that names nothing.
//!
//! The arm knows no command rule, so its steps carry [`super::step::Verdict::Either`]: the command decides, and
//! the fuzzer's invariants (no panic, all or nothing, the undo law, determinism) hold whatever it decides.
//! Memory and time stay bounded: an array gets at most three items beyond its minimum (and none beyond
//! `max(minItems, 1)` at a nesting depth of [`SHALLOW`] or more), optional members are dropped at that depth, and
//! no value nests deeper than [`MAX_DEPTH`].

use ew_commands::CommandRegistry;
use serde_json::{Map, Value, json};

use super::rng::SplitMix64;
use super::step::Step;

/// The weight of each registered command's default step among the steps (the hand arms weigh about 200
/// together).
pub const WEIGHT: usize = 1;

/// At this nesting depth or deeper, an object gets only its required members and a map no entry beyond its
/// minimum; an array gets its minimum length, except that an array of no minimum gets one item two times in three
/// (and none the third).
const SHALLOW: usize = 6;

/// The deepest value the generator builds.
const MAX_DEPTH: usize = 24;

/// One in this many IDs is a fresh UUID that names no record.
const FRESH_ID: usize = 16;

/// The page-grid unit of the golden pages in nm (three steps of the QET grid); integers are drawn as its multiples
/// part of the time, so that positions land on the grid.
const GRID_UNIT: i64 = 7_937_500;

/// The quarter turns about z as sign-normalised unit quaternions `[w, x, y, z]` in 10^-9.
const QUARTER_TURNS: [[i64; 4]; 4] = [
    [1_000_000_000, 0, 0, 0],
    [707_106_781, 0, 0, 707_106_781],
    [0, 0, 0, 1_000_000_000],
    [707_106_781, 0, 0, -707_106_781],
];

const UUID_PATTERN: &str = "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$";

/// A step of `command` from its schema, or `None` when the command is not registered.
pub fn step(
    registry: &CommandRegistry,
    command: &'static str,
    ids: &[(&'static str, Value)],
    rng: &mut SplitMix64,
) -> Option<Step> {
    params(registry, command, ids, rng).map(|p| Step::cmd(command, p))
}

/// The parameters of a step of `command` from its schema, or `None` when the command is not registered.
pub fn params(
    registry: &CommandRegistry,
    command: &str,
    ids: &[(&'static str, Value)],
    rng: &mut SplitMix64,
) -> Option<Value> {
    let schema = registry.get(command)?.schema();
    let mut generator = Gen {
        root: schema,
        ids,
        rng,
    };
    Some(generator.value(schema, None, 0))
}

/// The string values a schema allows at a path of member names (through `$ref`, arrays, `anyOf`/`oneOf` and
/// `const`/`enum`), in schema order: a list the hand arms read from the registry instead of copying it.
pub fn strings_at<'r>(registry: &'r CommandRegistry, command: &str, path: &[&str]) -> Vec<&'r str> {
    let Some(entry) = registry.get(command) else {
        return Vec::new();
    };
    let root = entry.schema();
    let mut out = Vec::new();
    collect_at(root, root, path, 0, &mut out);
    let mut seen = std::collections::BTreeSet::new();
    out.retain(|s| seen.insert(*s));
    out
}

fn collect_at<'r>(root: &'r Value, schema: &'r Value, path: &[&str], depth: usize, out: &mut Vec<&'r str>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Some(map) = schema.as_object() else {
        return;
    };
    if let Some(target) = map
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|r| deref(root, r))
    {
        collect_at(root, target, path, depth + 1, out);
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        for branch in map.get(key).and_then(Value::as_array).into_iter().flatten() {
            collect_at(root, branch, path, depth + 1, out);
        }
    }
    if let Some(items) = map.get("items") {
        collect_at(root, items, path, depth + 1, out);
    }
    match path.split_first() {
        Some((head, rest)) => {
            if let Some(sub) = map.get("properties").and_then(|p| p.get(*head)) {
                collect_at(root, sub, rest, depth + 1, out);
            }
        }
        None => {
            if let Some(Value::String(s)) = map.get("const") {
                out.push(s.as_str());
            }
            for v in map.get("enum").and_then(Value::as_array).into_iter().flatten() {
                if let Value::String(s) = v {
                    out.push(s.as_str());
                }
            }
        }
    }
}

fn deref<'s>(root: &'s Value, reference: &str) -> Option<&'s Value> {
    root.pointer(reference.strip_prefix('#')?)
}

/// The record kind a member name stands for, when it names one.
fn kind_of_member(name: &str) -> Option<&'static str> {
    Some(match name {
        "rep" | "reps" | "representation" | "representations" => "representation",
        "page" | "pages" => "page",
        "device" | "devices" | "cpu" | "module" | "station" | "cable" | "terminal" | "terminals"
        | "parent" => "device",
        "function" | "functions" => "function",
        "connection" | "connections" => "connection",
        "junction" | "junctions" => "junction",
        "conductor" | "conductors" => "conductor",
        "section" => "section",
        "location" | "node" | "nodes" => "structure_node",
        "external" => "external_ref",
        "symbol" => "symbol",
        "fndef" | "main_fndef" => "fndef",
        "form" | "frame" => "form",
        "part" | "copy" => "part",
        "revision" => "revision",
        "waiver" => "waiver",
        "item" | "items" => "page_item",
        "space" => "layout_space",
        "enclosure" => "enclosure",
        "surface" => "mounting_surface",
        "carrier" | "rail" | "duct" => "carrier",
        "placement" | "placements" => "placement",
        "channel" => "channel",
        "block" => "report_block",
        _ => return None,
    })
}

struct Gen<'a, 'r> {
    root: &'a Value,
    ids: &'a [(&'static str, Value)],
    rng: &'r mut SplitMix64,
}

impl Gen<'_, '_> {
    /// The keywords of a schema with its `$ref` target and its `allOf` parts merged in (the validator checks a
    /// `$ref`'s target and its siblings both).
    fn flatten(&self, schema: &Value, depth: usize) -> Map<String, Value> {
        let mut out = Map::new();
        let Some(map) = schema.as_object() else {
            return out;
        };
        if depth > MAX_DEPTH {
            return out;
        }
        if let Some(target) = map
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|r| deref(self.root, r))
        {
            out = self.flatten(target, depth + 1);
        }
        for part in map.get("allOf").and_then(Value::as_array).into_iter().flatten() {
            merge(&mut out, self.flatten(part, depth + 1));
        }
        let own: Map<String, Value> = map
            .iter()
            .filter(|(k, _)| !matches!(k.as_str(), "$ref" | "allOf"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        merge(&mut out, own);
        out
    }

    /// A value of `schema`; `member` is the name of the member it fills (or of the array that holds it), the hint
    /// for the kind of an ID.
    fn value(&mut self, schema: &Value, member: Option<&str>, depth: usize) -> Value {
        match schema {
            Value::Bool(true) => return self.any(),
            Value::Bool(false) => return Value::Null,
            _ => {}
        }
        if depth > MAX_DEPTH {
            return Value::Null;
        }
        let mut map = self.flatten(schema, depth);
        // One branch of `oneOf` / `anyOf`, merged into the rest of the schema.
        for key in ["oneOf", "anyOf"] {
            if let Some(Value::Array(branches)) = map.remove(key)
                && !branches.is_empty()
            {
                let branch = &branches[self.rng.below(branches.len())];
                let flat = self.flatten(branch, depth + 1);
                merge(&mut map, flat);
                // A branch may hold a `oneOf` of its own.
                return self.value(&Value::Object(map), member, depth + 1);
            }
        }
        if let Some(c) = map.get("const") {
            return c.clone();
        }
        if let Some(Value::Array(options)) = map.get("enum")
            && !options.is_empty()
        {
            return options[self.rng.below(options.len())].clone();
        }
        let ty = match map.get("type") {
            Some(Value::String(t)) => t.clone(),
            Some(Value::Array(ts)) => {
                let names: Vec<&str> = ts.iter().filter_map(Value::as_str).collect();
                // `null` (an absent optional value) one time in three.
                let non_null: Vec<&str> = names.iter().copied().filter(|t| *t != "null").collect();
                match (non_null.is_empty(), names.contains(&"null") && self.rng.chance(3)) {
                    (true, _) | (false, true) => "null".to_owned(),
                    (false, false) => non_null[self.rng.below(non_null.len())].to_owned(),
                }
            }
            _ if map.contains_key("properties") => "object".to_owned(),
            _ if map.contains_key("items") || map.contains_key("prefixItems") => "array".to_owned(),
            _ if map.contains_key("pattern") || map.contains_key("minLength") => "string".to_owned(),
            _ => return self.any(),
        };
        match ty.as_str() {
            "null" => Value::Null,
            "boolean" => Value::Bool(self.rng.chance(2)),
            "integer" | "number" => json!(self.integer(&map)),
            "string" => Value::String(self.string(&map, member)),
            "array" => self.array(&map, member, depth),
            "object" => self.object(&map, depth),
            _ => self.any(),
        }
    }

    /// A value of the schema `true`: a small JSON value.
    fn any(&mut self) -> Value {
        match self.rng.below(4) {
            0 => Value::Null,
            1 => json!(self.rng.below(10)),
            2 => Value::Bool(self.rng.chance(2)),
            _ => Value::String(format!("FZ{}", self.rng.below(100))),
        }
    }

    fn integer(&mut self, map: &Map<String, Value>) -> i64 {
        let bound = |key: &str| map.get(key).and_then(Value::as_i64);
        let (mut lo, mut hi) = match map.get("format").and_then(Value::as_str) {
            Some("uint8") => (0, i64::from(u8::MAX)),
            Some("uint16") => (0, i64::from(u16::MAX)),
            Some("uint32") => (0, i64::from(u32::MAX)),
            Some("int8") => (i64::from(i8::MIN), i64::from(i8::MAX)),
            Some("int16") => (i64::from(i16::MIN), i64::from(i16::MAX)),
            Some("int32") => (i64::from(i32::MIN), i64::from(i32::MAX)),
            Some("uint64" | "uint") => (0, ew_units::MAX_SAFE_INTEGER),
            _ => (-ew_units::MAX_SAFE_INTEGER, ew_units::MAX_SAFE_INTEGER),
        };
        if let Some(b) = bound("minimum") {
            lo = lo.max(b);
        }
        if let Some(b) = bound("exclusiveMinimum") {
            lo = lo.max(b.saturating_add(1));
        }
        if let Some(b) = bound("maximum") {
            hi = hi.min(b);
        }
        if let Some(b) = bound("exclusiveMaximum") {
            hi = hi.min(b.saturating_sub(1));
        }
        if lo > hi {
            return lo;
        }
        let clamp = |v: i64| v.clamp(lo, hi);
        match self.rng.below(20) {
            // A small count, index or quarter turn.
            0..=8 => clamp(self.rng.range(0, 8)),
            // A position on the page grid.
            9..=15 => clamp(self.rng.range(0, 60) * GRID_UNIT),
            16 => clamp(-self.rng.range(1, 8) * GRID_UNIT),
            17 => lo,
            18 => hi,
            // Anywhere in the range.
            _ => {
                let span = u64::try_from(i128::from(hi) - i128::from(lo)).unwrap_or(u64::MAX);
                let off = if span == u64::MAX {
                    self.rng.next()
                } else {
                    self.rng.next() % (span + 1)
                };
                i64::try_from(i128::from(lo) + i128::from(off)).unwrap_or(lo)
            }
        }
    }

    fn string(&mut self, map: &Map<String, Value>, member: Option<&str>) -> String {
        let pattern = map.get("pattern").and_then(Value::as_str).unwrap_or("");
        let hex = |rng: &mut SplitMix64, n: usize| -> String {
            (0..n)
                .map(|_| char::from(b"0123456789abcdef"[rng.below(16)]))
                .collect()
        };
        if pattern == UUID_PATTERN {
            return self.id(member);
        }
        if pattern.starts_with("^(?:0|-?[1-9]") {
            return ["0", "1", "2", "1.5", "2.5", "10", "0.75", "400"][self.rng.below(8)].to_owned();
        }
        if pattern.starts_with("^(?:[a-z]{2,3}") {
            return ["en", "de", "en-GB", "nl", "fr"][self.rng.below(5)].to_owned();
        }
        match pattern {
            "^[0-9]{4}-[0-9]{2}-[0-9]{2}$" => {
                return format!("2025-0{}-1{}", 1 + self.rng.below(9), self.rng.below(10));
            }
            "^[0-9a-f]{32}$" => return hex(self.rng, 32),
            "^sha256:[0-9a-f]{64}$" => return format!("sha256:{}", hex(self.rng, 64)),
            "^#[0-9a-f]{6}$" => return format!("#{}", hex(self.rng, 6)),
            _ => {}
        }
        // An identifier, or a plain string: a short word within the length bounds.
        let min = map.get("minLength").and_then(Value::as_u64).unwrap_or(0);
        let max = map
            .get("maxLength")
            .and_then(Value::as_u64)
            .unwrap_or(16)
            .max(min);
        let mut s = format!("FZ{}", self.rng.below(1000));
        let min = usize::try_from(min.min(64)).unwrap_or(0);
        let max = usize::try_from(max.min(64)).unwrap_or(16);
        while s.len() < min {
            s.push('X');
        }
        s.truncate(max);
        s
    }

    /// An ID: of the member's kind three times in four where the schema names one, a fresh UUID one time in
    /// [`FRESH_ID`], else any live record's.
    fn id(&mut self, member: Option<&str>) -> String {
        let fresh = self.rng.chance(FRESH_ID);
        let kind = member.and_then(kind_of_member).filter(|_| self.rng.below(4) != 0);
        let of_kind: Vec<&Value> = match kind {
            Some(k) => self
                .ids
                .iter()
                .filter(|(kk, _)| *kk == k)
                .map(|(_, v)| v)
                .collect(),
            None => Vec::new(),
        };
        let pick = if fresh {
            None
        } else if !of_kind.is_empty() {
            Some(of_kind[self.rng.below(of_kind.len())].clone())
        } else {
            self.rng.pick(self.ids).map(|(_, v)| v.clone())
        };
        match pick {
            Some(Value::String(s)) => s,
            _ => self.rng.uuid(),
        }
    }

    fn array(&mut self, map: &Map<String, Value>, member: Option<&str>, depth: usize) -> Value {
        let min = usize::try_from(map.get("minItems").and_then(Value::as_u64).unwrap_or(0)).unwrap_or(0);
        let max = map
            .get("maxItems")
            .and_then(Value::as_u64)
            .map_or(usize::MAX, |m| usize::try_from(m).unwrap_or(usize::MAX));
        let prefix: Vec<Value> = map
            .get("prefixItems")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let items = map.get("items").cloned();
        // The rotation `q` of a pose: the parse takes only a sign-normalised unit quaternion in 10^-9, so a quarter
        // turn about z.
        if member == Some("q") && prefix.len() == 4 {
            return json!(QUARTER_TURNS[self.rng.below(QUARTER_TURNS.len())]);
        }
        let extra = if depth >= SHALLOW { 0 } else { self.rng.below(3) };
        let mut len = (min.max(1) + extra).saturating_sub(usize::from(min == 0 && self.rng.chance(3)));
        len = len.min(max).max(min).min(min + 3);
        if matches!(items, Some(Value::Bool(false))) {
            len = len.min(prefix.len());
        }
        let unique = map.get("uniqueItems") == Some(&Value::Bool(true));
        let mut out: Vec<Value> = Vec::new();
        for i in 0..len {
            let sub = match prefix.get(i) {
                Some(s) => s.clone(),
                None => items.clone().unwrap_or(Value::Bool(true)),
            };
            let v = self.value(&sub, member, depth + 1);
            if unique && out.contains(&v) {
                continue;
            }
            out.push(v);
        }
        Value::Array(out)
    }

    fn object(&mut self, map: &Map<String, Value>, depth: usize) -> Value {
        let properties = map
            .get("properties")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let required: Vec<String> = map
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|r| r.as_str().map(str::to_owned))
            .collect();
        let mut out = Map::new();
        // The record kind first, so that an `id` beside it is of that kind.
        let mut kind_hint: Option<String> = None;
        let mut names: Vec<&String> = properties.keys().collect();
        names.sort_by_key(|n| n.as_str() != "kind");
        for name in names {
            let needed = required.contains(name);
            if !needed && (depth >= SHALLOW || self.rng.chance(2)) {
                continue;
            }
            let sub = &properties[name];
            let v = match (&kind_hint, name.as_str()) {
                (Some(k), "id") => {
                    let k = k.clone();
                    self.value_of_kind(sub, &k, depth)
                }
                _ => self.value(sub, Some(name), depth + 1),
            };
            if name == "kind"
                && let Value::String(k) = &v
            {
                kind_hint = Some(k.clone());
            }
            out.insert(name.clone(), v);
        }
        // A required member the properties do not describe (any value).
        for r in &required {
            if !out.contains_key(r) {
                let v = self.any();
                out.insert(r.clone(), v);
            }
        }
        // Map entries (`additionalProperties` with a schema), at least `minProperties`.
        let min = usize::try_from(map.get("minProperties").and_then(Value::as_u64).unwrap_or(0)).unwrap_or(0);
        if let Some(extra @ Value::Object(_)) = map.get("additionalProperties") {
            let names = map.get("propertyNames").cloned();
            let n = min.max(usize::from(depth < SHALLOW && self.rng.chance(2))).min(3);
            for _ in 0..n {
                let key = match &names {
                    Some(schema) => match self.value(schema, None, depth + 1) {
                        Value::String(s) => s,
                        other => other.to_string(),
                    },
                    None => format!("fz{}", self.rng.below(100)),
                };
                let v = self.value(extra, Some(&key), depth + 1);
                out.insert(key, v);
            }
        }
        Value::Object(out)
    }

    /// The `id` member beside a `kind`: an ID of that kind (the kind names are the `$k` names of the records).
    fn value_of_kind(&mut self, schema: &Value, kind: &str, depth: usize) -> Value {
        let of_kind: Vec<&Value> = self
            .ids
            .iter()
            .filter(|(k, _)| *k == kind)
            .map(|(_, v)| v)
            .collect();
        if !of_kind.is_empty() && self.rng.below(4) != 0 {
            return of_kind[self.rng.below(of_kind.len())].clone();
        }
        self.value(schema, None, depth + 1)
    }
}

/// Merges the keywords of `from` into `into`: `properties` are united, `required` lists joined, any other keyword
/// of `from` wins.
fn merge(into: &mut Map<String, Value>, from: Map<String, Value>) {
    for (k, v) in from {
        match (k.as_str(), into.get_mut(&k), v) {
            ("properties", Some(Value::Object(mine)), Value::Object(theirs)) => {
                for (pk, pv) in theirs {
                    mine.insert(pk, pv);
                }
            }
            ("required", Some(Value::Array(mine)), Value::Array(theirs)) => {
                for r in theirs {
                    if !mine.contains(&r) {
                        mine.push(r);
                    }
                }
            }
            (_, _, v) => {
                into.insert(k, v);
            }
        }
    }
}
