// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The computed `min_reader` stamp (doc 05 §4, §10.1; ADR-0011): the lowest format a reader needs is the greatest
//! format among the kinds, members and variants a project's files use, never below the format's major floor and never
//! above the format itself. The `since` tables are `ew-schema`'s ([`SINCE`]); [`file_since`] takes them as an
//! argument, so that a test can name a member of a newer format.

use ew_schema::{Kind, MemberSince, VariantSince};
use serde_json::Value;

use crate::limits::MAX_DEPTH;
use crate::path::{FilePath, PathClass, classify};
use crate::read::exceeds_depth;
use crate::version::parse;

/// The `since` tables of the computed `min_reader`: the format each kind, member and variant first appeared in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SinceTables<'a> {
    /// The format of each kind ([`ew_schema::KIND_SINCE`]); a kind without a row is at the floor.
    pub kinds: &'a [(Kind, &'static str)],
    /// The members added after their kind ([`ew_schema::MEMBER_SINCE`]).
    pub members: &'a [MemberSince],
    /// The variants added after their member ([`ew_schema::VARIANT_SINCE`]).
    pub variants: &'a [VariantSince],
}

/// The tables of this build, from `ew-schema` (re-exported here: `ew-store` has no dependency edge to `ew-schema`).
pub const SINCE: SinceTables<'static> = SinceTables {
    kinds: &ew_schema::KIND_SINCE,
    members: ew_schema::MEMBER_SINCE,
    variants: ew_schema::VARIANT_SINCE,
};

/// The major floor of a format (doc 05 §10.1): `1.0` for `1.3`; before 1.0 every step is a major, so `0.2` for `0.2`.
fn major_floor((major, minor): (u32, u32)) -> (u32, u32) {
    if major == 0 { (0, minor) } else { (major, 0) }
}

/// The `since` value of one project file for a manifest of `format`: the greatest format above `format`'s major floor
/// that a record of the file needs by its kind (`$k`), by a member [`SinceTables::members`] names, or by a variant
/// [`SinceTables::variants`] names. `None` when nothing in it is above the floor, and always for a file that is not a
/// JSON Lines record file or a library document (assets and other opaque files are at the floor), for an unreadable
/// `format`, and for lines that do not parse (they count as nothing). Pure: the same bytes and tables give the same
/// value, so a file's value can be kept beside its hash.
///
/// The table values are not limited to `format` here; [`min_reader`] limits the result. When no row of `tables` is
/// above the floor, the file is not read at all.
///
/// ```
/// use ew_format::{FilePath, SinceTables, file_since, min_reader};
/// use ew_format::ew_schema::{Kind, MemberSince};
///
/// const NEWER: &[MemberSince] = &[MemberSince { kind: Kind::Device, path: "colour", since: "1.2" }];
/// let tables = SinceTables { kinds: &[], members: NEWER, variants: &[] };
/// let shard = FilePath::new("devices/0a.jsonl").unwrap();
/// let with = br#"{"$k":"device","id":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b0a","colour":"BU"}
/// "#;
/// let without = br#"{"$k":"device","id":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b0a"}
/// "#;
/// assert_eq!(file_since("1.3", &shard, with, &tables), Some("1.2"));
/// assert_eq!(file_since("1.3", &shard, without, &tables), None);
/// assert_eq!(min_reader("1.3", [Some("1.2"), None].into_iter().flatten()), "1.2");
/// assert_eq!(min_reader("1.3", None), "1.0");
/// ```
pub fn file_since(
    format: &str,
    path: &FilePath,
    bytes: &[u8],
    tables: &SinceTables<'_>,
) -> Option<&'static str> {
    let floor = major_floor(parse(format)?);
    let above = |since: &str| parse(since).is_some_and(|v| v > floor);
    // Nothing in the tables is above the floor (every kind and member of this format): no file needs more.
    if !(tables.kinds.iter().any(|(_, s)| above(s))
        || tables.members.iter().any(|m| above(m.since))
        || tables.variants.iter().any(|v| above(v.since)))
    {
        return None;
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let greatest = |a: Option<&'static str>, b: Option<&'static str>| match (a, b) {
        (Some(x), Some(y)) => Some(if parse(y) > parse(x) { y } else { x }),
        (x, y) => x.or(y),
    };
    match classify(path.as_str()) {
        PathClass::Records(_) => bytes
            .split(|b| *b == b'\n')
            .filter_map(read_value)
            .map(|value| value_since(&value, tables, floor))
            .fold(None, greatest),
        PathClass::LibDocument { .. } => {
            read_value(bytes).and_then(|value| value_since(&value, tables, floor))
        }
        _ => None,
    }
}

/// One JSON value as the loader reads it (UTF-8, the nesting limit, repeated members rejected); `None` otherwise.
fn read_value(bytes: &[u8]) -> Option<Value> {
    let text = std::str::from_utf8(bytes).ok()?;
    if exceeds_depth(text.as_bytes(), MAX_DEPTH) {
        return None;
    }
    ew_schema::parse_value(text).ok()
}

/// The greatest `since` above `floor` that the record or document `value` needs.
fn value_since(value: &Value, tables: &SinceTables<'_>, floor: (u32, u32)) -> Option<&'static str> {
    let kind = Kind::from_name(value.get("$k")?.as_str()?)?;
    let kinds = tables.kinds.iter().filter(|(k, _)| *k == kind).map(|(_, s)| *s);
    let members = tables
        .members
        .iter()
        .filter(|m| m.kind == kind && !at_path(value, m.path).is_empty())
        .map(|m| m.since);
    let variants = tables
        .variants
        .iter()
        .filter(|v| v.kind == kind && at_path(value, v.path).iter().any(|x| is_variant(x, v.variant)))
        .map(|v| v.since);
    kinds
        .chain(members)
        .chain(variants)
        .filter_map(|since| parse(since).filter(|v| *v > floor).map(|v| (v, since)))
        .max_by_key(|(v, _)| *v)
        .map(|(_, since)| since)
}

/// The values at `path` in `value`: `a.b` descends into an object member, `a[]` into the elements of an array.
fn at_path<'v>(value: &'v Value, path: &str) -> Vec<&'v Value> {
    let mut current = vec![value];
    for segment in path.split('.') {
        let (name, array) = match segment.strip_suffix("[]") {
            Some(name) => (name, true),
            None => (segment, false),
        };
        let mut next = Vec::new();
        for v in current {
            match v.get(name) {
                Some(Value::Array(items)) if array => next.extend(items),
                Some(member) if !array => next.push(member),
                _ => {}
            }
        }
        current = next;
    }
    current
}

/// Whether `value` is the variant `name` of an externally tagged enum: the string, or an object whose one member is
/// named `name`.
fn is_variant(value: &Value, name: &str) -> bool {
    match value {
        Value::String(s) => s == name,
        Value::Object(members) => members.len() == 1 && members.contains_key(name),
        _ => false,
    }
}

/// `min_reader` for a manifest of `format` (doc 05 §4): the greatest of `files` (each the [`file_since`] value of a
/// listed file; files at the floor give none), never below `format`'s major floor and never above `format`. A value
/// that is not a format counts as nothing; an unreadable `format` is returned unchanged (the reader refuses it).
pub fn min_reader<'s>(format: &str, files: impl IntoIterator<Item = &'s str>) -> String {
    let Some(top) = parse(format) else {
        return format.to_owned();
    };
    let floor = major_floor(top);
    let greatest = files.into_iter().filter_map(parse).fold(floor, Ord::max);
    let (major, minor) = greatest.min(top);
    format!("{major}.{minor}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b0a";

    fn shard() -> FilePath {
        FilePath::new("devices/0a.jsonl").unwrap()
    }

    fn device(members: &str) -> String {
        format!("{{\"$k\":\"device\",\"id\":\"{ID}\"{members}}}\n")
    }

    #[test]
    fn the_shipped_tables_put_every_file_at_the_floor() {
        // Every kind is 0.1 and no member or variant is newer: no file of a 0.1 project needs more than 0.1.
        let bytes = device(",\"function_text\":\"x\"");
        assert_eq!(file_since("0.1", &shard(), bytes.as_bytes(), &SINCE), None);
        assert_eq!(min_reader("0.1", None), "0.1");
    }

    #[test]
    fn a_member_of_a_newer_minor_raises_min_reader_to_its_format() {
        // SG-1 (first clause), with an injected table: before 1.0 every step is a major (doc 05 §10.1), so a 0.x
        // format's floor is the format itself and `min_reader` equals it; the clause is shown on a 1.x format.
        const MEMBERS: &[MemberSince] = &[MemberSince {
            kind: Kind::Device,
            path: "terminal.colour",
            since: "1.2",
        }];
        let tables = SinceTables {
            kinds: &[],
            members: MEMBERS,
            variants: &[],
        };
        let with = format!(
            "{}{}",
            device(""),
            device(",\"terminal\":{\"strip\":\"x\",\"colour\":\"BU\"}")
        );
        let without = device(",\"terminal\":{\"strip\":\"x\"}");
        let one = file_since("1.3", &shard(), with.as_bytes(), &tables);
        assert_eq!(one, Some("1.2"));
        assert_eq!(file_since("1.3", &shard(), without.as_bytes(), &tables), None);
        assert_eq!(min_reader("1.3", [one, None].into_iter().flatten()), "1.2");
        assert_eq!(min_reader("1.3", None), "1.0", "nothing newer: the major floor");
        // Never above the format, never below the floor.
        assert_eq!(min_reader("1.3", ["1.5"]), "1.3");
        assert_eq!(min_reader("1.3", ["0.9", "x"]), "1.0");
        // Before 1.0 the floor is the format: 0.2 with a 0.2 member is 0.2, and so is 0.2 without one.
        let zero = file_since("0.1", &shard(), with.as_bytes(), &tables);
        assert_eq!(zero, Some("1.2"), "the value itself is not limited");
        assert_eq!(min_reader("0.2", zero), "0.2");
        assert_eq!(min_reader("0.2", None), "0.2");
        // An unreadable format stays as it is.
        assert_eq!(min_reader("01.1", ["1.2"]), "01.1");
        assert_eq!(file_since("01.1", &shard(), with.as_bytes(), &tables), None);
    }

    #[test]
    fn kinds_variants_and_library_documents_count() {
        const VARIANTS: &[VariantSince] = &[
            VariantSince {
                kind: Kind::Junction,
                path: "style",
                variant: "bevel",
                since: "1.1",
            },
            VariantSince {
                kind: Kind::Junction,
                path: "wiring",
                variant: "star",
                since: "1.4",
            },
        ];
        let kinds = [(Kind::Part, "1.2"), (Kind::Device, "1.0")];
        let tables = SinceTables {
            kinds: &kinds,
            members: &[],
            variants: VARIANTS,
        };
        let page = FilePath::new(format!("pages/{ID}.jsonl")).unwrap();
        let bevel = format!(
            "{{\"$k\":\"junction\",\"id\":\"{ID}\",\"style\":{{\"bevel\":{{\"orient\":\"r90\"}}}}}}\n"
        );
        let star = format!("{{\"$k\":\"junction\",\"id\":\"{ID}\",\"wiring\":\"star\"}}\n");
        let dot = format!("{{\"$k\":\"junction\",\"id\":\"{ID}\",\"style\":\"dot\"}}\n");
        assert_eq!(file_since("1.4", &page, bevel.as_bytes(), &tables), Some("1.1"));
        let both = format!("{bevel}{star}");
        assert_eq!(file_since("1.4", &page, both.as_bytes(), &tables), Some("1.4"));
        assert_eq!(file_since("1.4", &page, dot.as_bytes(), &tables), None);
        // A kind's own format; the floor (1.0) counts as nothing.
        assert_eq!(file_since("1.4", &shard(), device("").as_bytes(), &tables), None);
        let part = FilePath::new(format!("lib/parts/{ID}.json")).unwrap();
        let doc = format!("{{\n  \"$k\": \"part\",\n  \"id\": \"{ID}\"\n}}\n");
        assert_eq!(file_since("1.4", &part, doc.as_bytes(), &tables), Some("1.2"));
        // An asset or any other opaque file is at the floor; so are lines that do not parse and unknown kinds.
        let asset = FilePath::new(format!("assets/{}.svg", "0".repeat(64))).unwrap();
        assert_eq!(file_since("1.4", &asset, doc.as_bytes(), &tables), None);
        let noise = format!(
            "not json\n{{\"$k\":\"frobnicator\",\"id\":\"{ID}\",\"style\":\"bevel\"}}\n<<<<<<< ours\n"
        );
        assert_eq!(file_since("1.4", &page, noise.as_bytes(), &tables), None);
    }

    #[test]
    fn member_paths_descend_into_objects_and_arrays() {
        let value: Value =
            serde_json::from_str(r#"{"a":{"b":[{"c":1},{"d":2},{"c":3}]},"e":[1,2]}"#).unwrap();
        assert_eq!(at_path(&value, "a.b[].c"), [&Value::from(1), &Value::from(3)]);
        assert_eq!(at_path(&value, "e[]").len(), 2);
        assert!(at_path(&value, "a.b.c").is_empty(), "an array is not an object");
        assert!(at_path(&value, "a[]").is_empty(), "an object is not an array");
        assert!(at_path(&value, "x").is_empty());
        assert!(is_variant(&Value::from("star"), "star"));
        assert!(is_variant(&serde_json::json!({"star": {}}), "star"));
        assert!(!is_variant(&serde_json::json!({"star": {}, "x": 1}), "star"));
        assert!(!is_variant(&Value::from(1), "star"));
    }
}
