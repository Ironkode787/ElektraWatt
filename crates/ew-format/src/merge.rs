// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The three-way merge core (doc 05 §11.2, §11.3; ADR-0012 decisions 2 to 4; WP-S1.17 D2, D13, D14, D16, decision log
//! ID-1268).
//!
//! - **One record** ([`merge_record`]): the core that the merge driver, the conflict-marker loader and the
//!   external-change apply all call, on `serde_json::Value`. By the record table of doc 05 §11.2: a record on one side
//!   only is kept; a record deleted on one side and unchanged on the other is deleted; deleted on one side and modified
//!   on the other, the modified side is kept with a delete/modify conflict; on both sides, the record merges per
//!   top-level member, with an empty base when it was added on both.
//! - **One member** merges as one value (`atomic`: the side that changed it wins, both changed it differently is a
//!   conflict that keeps ours) unless `ew-schema`'s [`MERGE_STRATEGIES`] names its path and the member exists at the
//!   base: `object` (member by member, each atomic unless listed), `keyed` (a map by the key member, each element
//!   atomic), `set` and `max`; a strategy settles a member that both sides changed, the side that changed it alone
//!   winning as for an atomic one. A member above a listed path merges as an `object`. A record of a kind this reader
//!   does not implement merges as a whole.
//! - **Conflict records** (D14): one per target, `{"$k":"conflict","id":<target id>,"kind":<target $k>,"base":…,
//!   "ours":…,"theirs":…}`, holding only the conflicting members as partial objects, `"deleted"` for a deleted side.
//!   A conflict inside a keyed list holds the whole list: ours as merged, theirs as merged with theirs' elements.
//! - **One file** ([`merge_file`]): a record file per `($k, id)`, a library document as one record whose conflict is a
//!   top-level `$conflict` member holding theirs' document, and `manifest.json` by doc 05 §11.3. The three inputs are
//!   read under the loader's [`Limits`]; an older side is migrated with the chain the caller passes ([`MIGRATIONS`]
//!   in production); a side that does not read (a broken line, a limit, a failed step), or that is newer than this
//!   reader (its format, or a record's `$v` above its kind's), keeps the other side and adds the file-level conflict
//!   `{"$k":"conflict","id":v5(NS_CONFLICT, path),"kind":"file","path":…,"side":…,"lines":[…]}` with its lines
//!   verbatim. A base that is not empty but does not read follows D18's no-base rule. A record a side holds unchanged
//!   is written as that side's own line.
//!
//! [`MIGRATIONS`]: crate::MIGRATIONS

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use ew_schema::{
    AnyId, Date, FORMAT, Kind, KindStatus, LibDocument, MERGE_STRATEGIES, MergeOrder, MergeStrategy, Record,
    conflict_file_id,
};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::limits::Limits;
use crate::line::{jsonl_file, lib_document_text, pretty, record_line};
use crate::manifest::{FileHash, MAX_GENERATION, Manifest};
use crate::migrate::{Migration, Migrations};
use crate::order::LineKey;
use crate::path::{PathClass, classify};
use crate::read::{exceeds_depth, parse_document, parse_line, rehash_migrated_copy};
use crate::version;

/// One side of a file merge ([`merge_file`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MergeSide<'a> {
    /// The file's bytes; empty for a side without the file (git's ancestor of a file added on both sides).
    pub bytes: &'a [u8],
    /// The format of the side's project, its manifest's `format` ([`FORMAT`] for a current side). `manifest.json`
    /// states its own format, so this one is not used for it.
    pub format: &'a str,
}

impl<'a> MergeSide<'a> {
    /// A side of this reader's format.
    pub fn current(bytes: &'a [u8]) -> MergeSide<'a> {
        MergeSide {
            bytes,
            format: FORMAT,
        }
    }
}

/// The merge of one file ([`merge_file`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FileMerge {
    /// The merged file.
    pub bytes: Vec<u8>,
    /// The conflict records written (record-level and file-level, or a library document's `$conflict`); for a
    /// manifest or a file of another class, 1 when the sides could not be merged; 1 when ours is kept as it is
    /// because the merged file would pass a reader limit. 0 is a clean merge.
    pub conflicts: usize,
}

/// The merge of one record ([`merge_record`]).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RecordMerge {
    /// The merged record; `None` when it is deleted.
    pub merged: Option<Value>,
    /// Its conflict record, when the sides conflict. A `Value` holds its members sorted; [`merge_file`] writes them in
    /// D14's order.
    pub conflict: Option<Value>,
}

/// The member a save writes with the hash of the manifest it replaced (`ew-store`'s `BASE_MEMBER`): a merged
/// manifest has none (doc 05 §11.3).
const BASE: &str = "base";

/// A conflicting member: its path from the record, and its value on each side (`None`: absent).
struct Conflict {
    path: Vec<String>,
    base: Option<Value>,
    ours: Option<Value>,
    theirs: Option<Value>,
}

#[derive(Clone, Copy)]
enum Who {
    Ours,
    Theirs,
}

/// Merges one record three-way (doc 05 §11.2; WP-S1.17 D2, D13, D14): `base`, `ours` and `theirs` are the record's
/// values under one `($k, id)` key, `None` where a side does not hold it. The values are read and current already
/// (the loader's limits, the migration chain); [`merge_file`] reads a file's records so.
///
/// ```
/// use ew_format::merge_record;
/// use serde_json::json;
///
/// let base = json!({"$k":"connection","id":"5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c","overrides":{"csa":"1.5"}});
/// let ours = json!({"$k":"connection","id":"5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c","overrides":{"csa":"1.5","colour":["BU"]}});
/// let theirs = json!({"$k":"connection","id":"5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c","overrides":{"csa":"2.5"}});
/// let merged = merge_record(Some(&base), Some(&ours), Some(&theirs));
/// assert_eq!(merged.conflict, None);
/// assert_eq!(merged.merged.unwrap()["overrides"], json!({"csa":"2.5","colour":["BU"]}));
/// ```
pub fn merge_record(base: Option<&Value>, ours: Option<&Value>, theirs: Option<&Value>) -> RecordMerge {
    match (ours, theirs) {
        (None, None) => RecordMerge::default(),
        (Some(a), None) => one_sided(base, a, Who::Theirs),
        (None, Some(b)) => one_sided(base, b, Who::Ours),
        (Some(a), Some(b)) if a == b => RecordMerge {
            merged: Some(a.clone()),
            conflict: None,
        },
        (Some(a), Some(b)) => both_sides(base, a, b),
    }
}

/// A record that one side holds and the other (`deleted`) does not.
fn one_sided(base: Option<&Value>, kept: &Value, deleted: Who) -> RecordMerge {
    let Some(o) = base else {
        return RecordMerge {
            merged: Some(kept.clone()),
            conflict: None,
        };
    };
    if kept == o {
        return RecordMerge::default();
    }
    // Delete/modify: the modified side stays, and the conflict holds the members it changed.
    let names: BTreeSet<&String> = members(Some(o))
        .chain(members(Some(kept)))
        .filter(|name| o.get(name.as_str()) != kept.get(name.as_str()))
        .collect();
    let pick = |v: &Value| {
        let mut out = Map::new();
        for name in &names {
            if let Some(value) = v.get(name.as_str()) {
                out.insert((*name).clone(), value.clone());
            }
        }
        Value::Object(out)
    };
    let gone = Value::from("deleted");
    let (ours, theirs) = match deleted {
        Who::Ours => (gone, pick(kept)),
        Who::Theirs => (pick(kept), gone),
    };
    RecordMerge {
        merged: Some(kept.clone()),
        conflict: Some(conflict_record(kept, pick(o), ours, theirs)),
    }
}

/// The member names of an object value (none for anything else).
fn members(value: Option<&Value>) -> impl Iterator<Item = &String> {
    value
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|m| m.keys())
}

/// One record by D18's no-base rule, for a base that is not empty but does not read (decision log ID-1273): equal on
/// both sides it is kept; different, ours is kept with a conflict of every top-level member that differs; on one side
/// only, it is kept with a conflict whose other side is `"deleted"`, since the other side may have deleted it. The
/// base of such a conflict is empty.
fn no_base(ours: Option<&Value>, theirs: Option<&Value>) -> RecordMerge {
    let (kept, other) = match (ours, theirs) {
        (None, None) => return RecordMerge::default(),
        (Some(a), Some(b)) if a == b => {
            return RecordMerge {
                merged: Some(a.clone()),
                conflict: None,
            };
        }
        (Some(a), b) => (a, b),
        (None, Some(b)) => (b, None),
    };
    let names: BTreeSet<&String> = members(Some(kept))
        .chain(members(other))
        .filter(|name| *name != "$k" && *name != "id")
        .filter(|name| kept.get(name.as_str()) != other.and_then(|o| o.get(name.as_str())))
        .collect();
    let pick = |v: &Value| {
        let mut out = Map::new();
        for name in &names {
            if let Some(value) = v.get(name.as_str()) {
                out.insert((*name).clone(), value.clone());
            }
        }
        Value::Object(out)
    };
    let gone = Value::from("deleted");
    let (a, b) = match (ours, other) {
        (Some(_), Some(b)) => (pick(kept), pick(b)),
        (Some(_), None) => (pick(kept), gone),
        (None, _) => (gone, pick(kept)),
    };
    RecordMerge {
        merged: Some(kept.clone()),
        conflict: Some(conflict_record(kept, Value::Object(Map::new()), a, b)),
    }
}

/// A record both sides hold, differently.
fn both_sides(base: Option<&Value>, a: &Value, b: &Value) -> RecordMerge {
    let kind = a
        .get("$k")
        .and_then(Value::as_str)
        .and_then(Kind::from_name)
        .filter(|k| k.status() == KindStatus::Implemented);
    let mut conflicts = Vec::new();
    let merged = match (kind, a.as_object(), b.as_object()) {
        (Some(kind), Some(am), Some(bm)) => {
            let om = base.and_then(Value::as_object);
            let names: BTreeSet<&String> = members(base).chain(am.keys()).chain(bm.keys()).collect();
            let mut out = Map::new();
            for name in names {
                // The key members are equal on both sides.
                let merged = if name == "$k" || name == "id" {
                    am.get(name).cloned()
                } else {
                    merge_value(
                        kind,
                        std::slice::from_ref(name),
                        om.and_then(|m| m.get(name)),
                        am.get(name),
                        bm.get(name),
                        &mut conflicts,
                    )
                };
                if let Some(value) = merged {
                    out.insert(name.clone(), value);
                }
            }
            Value::Object(out)
        }
        // A kind this reader does not implement merges as a whole record (D13).
        _ => {
            if base == Some(a) {
                b.clone()
            } else if base == Some(b) {
                a.clone()
            } else {
                conflicts.push(Conflict {
                    path: Vec::new(),
                    base: base.cloned(),
                    ours: Some(a.clone()),
                    theirs: Some(b.clone()),
                });
                a.clone()
            }
        }
    };
    let conflict = (!conflicts.is_empty()).then(|| {
        let side = |pick: fn(&Conflict) -> Option<&Value>| {
            let mut out = Map::new();
            for c in &conflicts {
                insert_at(&mut out, &c.path, pick(c));
            }
            Value::Object(out)
        };
        conflict_record(
            a,
            side(|c| c.base.as_ref()),
            side(|c| c.ours.as_ref()),
            side(|c| c.theirs.as_ref()),
        )
    });
    RecordMerge {
        merged: Some(merged),
        conflict,
    }
}

/// `value` at `path` in the partial object `out`, its parents created; nothing at the leaf when `value` is absent.
/// An empty path stands for the whole record.
fn insert_at(out: &mut Map<String, Value>, path: &[String], value: Option<&Value>) {
    let Some((leaf, parents)) = path.split_last() else {
        if let Some(Value::Object(whole)) = value {
            out.extend(whole.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        return;
    };
    let mut at = out;
    for name in parents {
        let entry = at
            .entry(name.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        let Value::Object(next) = entry else {
            return;
        };
        at = next;
    }
    if let Some(value) = value {
        at.insert(leaf.clone(), value.clone());
    }
}

/// The conflict record of `target` (D14).
fn conflict_record(target: &Value, base: Value, ours: Value, theirs: Value) -> Value {
    let mut out = Map::new();
    out.insert("$k".to_owned(), Value::from("conflict"));
    out.insert("id".to_owned(), target.get("id").cloned().unwrap_or(Value::Null));
    out.insert(
        "kind".to_owned(),
        target.get("$k").cloned().unwrap_or(Value::Null),
    );
    out.insert("base".to_owned(), base);
    out.insert("ours".to_owned(), ours);
    out.insert("theirs".to_owned(), theirs);
    Value::Object(out)
}

/// The strategy of the member at `path` of a `kind` record: its row of [`MERGE_STRATEGIES`], else
/// [`MergeStrategy::Object`] when a row lies below it, else none (atomic).
fn strategy_at(kind: Kind, path: &[String]) -> Option<MergeStrategy> {
    let dotted = path.join(".");
    let rows = || MERGE_STRATEGIES.iter().filter(move |r| r.kind == kind);
    if let Some(row) = rows().find(|r| r.path == dotted) {
        return Some(row.strategy);
    }
    rows()
        .any(|r| {
            r.path
                .strip_prefix(dotted.as_str())
                .is_some_and(|rest| rest.starts_with('.'))
        })
        .then_some(MergeStrategy::Object)
}

/// One member three-way, its conflicts pushed to `out`; ours where the sides conflict.
fn merge_value(
    kind: Kind,
    path: &[String],
    o: Option<&Value>,
    a: Option<&Value>,
    b: Option<&Value>,
    out: &mut Vec<Conflict>,
) -> Option<Value> {
    if a == b || b == o {
        return a.cloned();
    }
    if a == o {
        return b.cloned();
    }
    // Both sides changed it: a strategy applies where the member exists at the base (D13).
    let merged = match o.and_then(|_| strategy_at(kind, path)) {
        Some(MergeStrategy::Object) => merge_object(kind, path, o, a, b, out),
        Some(MergeStrategy::Keyed { key, order }) => merge_keyed(path, key, order, o, a, b, out),
        Some(MergeStrategy::Set { order }) => merge_set(order, o, a, b),
        Some(MergeStrategy::Max) => merge_max(a, b),
        None => None,
    };
    if let Some(merged) = merged {
        return merged;
    }
    out.push(Conflict {
        path: path.to_vec(),
        base: o.cloned(),
        ours: a.cloned(),
        theirs: b.cloned(),
    });
    a.cloned()
}

/// An absent member as its default (an empty object or list, which the canonical form omits); `None` when a value is
/// not of that shape, so that the member merges atomically.
fn object_or_absent(value: Option<&Value>) -> Option<Option<&Map<String, Value>>> {
    match value {
        None => Some(None),
        Some(Value::Object(m)) => Some(Some(m)),
        Some(_) => None,
    }
}

fn list_or_absent(value: Option<&Value>) -> Option<&[Value]> {
    match value {
        None => Some(&[]),
        Some(Value::Array(items)) => Some(items),
        Some(_) => None,
    }
}

/// An empty merged object or list is absent when a side omitted it.
fn present(value: Value, empty: bool, a: Option<&Value>, b: Option<&Value>) -> Option<Value> {
    (!(empty && (a.is_none() || b.is_none()))).then_some(value)
}

/// `max`: the later date of the two sides; `None` (atomic) when a side's value is not a date.
fn merge_max(a: Option<&Value>, b: Option<&Value>) -> Option<Option<Value>> {
    let date = |v: Option<&Value>| match v {
        None => Some(None),
        Some(v) => serde_json::from_value::<Date>(v.clone()).ok().map(Some),
    };
    let (da, db) = (date(a)?, date(b)?);
    Some(if db > da { b.cloned() } else { a.cloned() })
}

/// `object`: member by member, each by its own row or atomic.
fn merge_object(
    kind: Kind,
    path: &[String],
    o: Option<&Value>,
    a: Option<&Value>,
    b: Option<&Value>,
    out: &mut Vec<Conflict>,
) -> Option<Option<Value>> {
    let om = o?.as_object()?;
    let am = object_or_absent(a)?;
    let bm = object_or_absent(b)?;
    let names: BTreeSet<&String> = om
        .keys()
        .chain(am.into_iter().flat_map(|m| m.keys()))
        .chain(bm.into_iter().flat_map(|m| m.keys()))
        .collect();
    let mut merged = Map::new();
    for name in names {
        let mut child = path.to_vec();
        child.push(name.clone());
        let value = merge_value(
            kind,
            &child,
            om.get(name),
            am.and_then(|m| m.get(name)),
            bm.and_then(|m| m.get(name)),
            out,
        );
        if let Some(value) = value {
            merged.insert(name.clone(), value);
        }
    }
    let empty = merged.is_empty();
    Some(present(Value::Object(merged), empty, a, b))
}

/// The elements of a keyed list by the compact JSON text of their key, in list order; `None` when an element is not an
/// object with the key or two elements share a key (the member then merges atomically, decision log ID-1268).
fn keyed_elements<'v>(items: &'v [Value], key: &str) -> Option<Vec<(String, &'v Value, &'v Value)>> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let k = item.as_object()?.get(key)?;
        let text = k.to_string();
        if !seen.insert(text.clone()) {
            return None;
        }
        out.push((text, k, item));
    }
    Some(out)
}

/// The elements of [`keyed_elements`] by key text.
fn by_key<'v>(list: &[(String, &'v Value, &'v Value)]) -> BTreeMap<String, &'v Value> {
    list.iter().map(|(text, _, e)| (text.clone(), *e)).collect()
}

/// The natural order of keys: numbers numerically, strings byte-wise, anything else by its JSON text.
fn key_order(x: &Value, y: &Value) -> Ordering {
    match (x, y) {
        (Value::Number(p), Value::Number(q)) => match (p.as_u64(), q.as_u64()) {
            (Some(p), Some(q)) => p.cmp(&q),
            _ => match (p.as_i64(), q.as_i64()) {
                (Some(p), Some(q)) => p.cmp(&q),
                _ => x.to_string().cmp(&y.to_string()),
            },
        },
        (Value::String(p), Value::String(q)) => p.as_bytes().cmp(q.as_bytes()),
        _ => x.to_string().cmp(&y.to_string()),
    }
}

/// `keyed(key)`: a map by `key`, each element atomic. Only keys that are all unsigned integers or all strings have a
/// total order ([`key_order`]); a list with other keys merges atomically (decision log ID-1273).
fn merge_keyed(
    path: &[String],
    key: &str,
    order: MergeOrder,
    o: Option<&Value>,
    a: Option<&Value>,
    b: Option<&Value>,
    out: &mut Vec<Conflict>,
) -> Option<Option<Value>> {
    let ol = keyed_elements(o?.as_array()?, key)?;
    let al = keyed_elements(list_or_absent(a)?, key)?;
    let bl = keyed_elements(list_or_absent(b)?, key)?;
    let keys_of = || ol.iter().chain(&al).chain(&bl).map(|(_, k, _)| *k);
    if !keys_of().all(Value::is_u64) && !keys_of().all(Value::is_string) {
        return None;
    }
    let (om, am, bm) = (by_key(&ol), by_key(&al), by_key(&bl));
    // Ours' order, then theirs' additions in theirs' order; the canonical order sorts by key.
    let mut seen = BTreeSet::new();
    let mut keys: Vec<(&String, &Value)> = Vec::new();
    for (text, k, _) in al.iter().chain(bl.iter()).chain(ol.iter()) {
        if seen.insert(text) {
            keys.push((text, k));
        }
    }
    if order == MergeOrder::Canonical {
        keys.sort_by(|x, y| key_order(x.1, y.1));
    }
    let (mut ours, mut theirs, mut conflicted) = (Vec::new(), Vec::new(), false);
    for (text, _) in keys {
        let (eo, ea, eb) = (om.get(text), am.get(text), bm.get(text));
        let (keep_a, keep_b) = if ea == eb || eb == eo {
            (ea, ea)
        } else if ea == eo {
            (eb, eb)
        } else {
            conflicted = true;
            (ea, eb)
        };
        ours.extend(keep_a.map(|e| (*e).clone()));
        theirs.extend(keep_b.map(|e| (*e).clone()));
    }
    let (empty_a, empty_b) = (ours.is_empty(), theirs.is_empty());
    let merged = present(Value::Array(ours), empty_a, a, b);
    if conflicted {
        out.push(Conflict {
            path: path.to_vec(),
            base: o.cloned(),
            ours: merged.clone(),
            theirs: present(Value::Array(theirs), empty_b, a, b),
        });
    }
    Some(merged)
}

/// `set`: (o ∩ a ∩ b) ∪ (a ∖ o) ∪ (b ∖ o), in ours' order then theirs' additions, or sorted by the elements' compact
/// JSON text (decision log ID-1268 ruling 2).
fn merge_set(
    order: MergeOrder,
    o: Option<&Value>,
    a: Option<&Value>,
    b: Option<&Value>,
) -> Option<Option<Value>> {
    let ol = o?.as_array()?;
    let al = list_or_absent(a)?;
    let bl = list_or_absent(b)?;
    let texts = |list: &[Value]| -> BTreeSet<String> { list.iter().map(Value::to_string).collect() };
    let (in_o, in_b) = (texts(ol), texts(bl));
    let mut seen = BTreeSet::new();
    let mut merged: Vec<(String, Value)> = Vec::new();
    for x in al {
        let text = x.to_string();
        if (!in_o.contains(&text) || in_b.contains(&text)) && seen.insert(text.clone()) {
            merged.push((text, x.clone()));
        }
    }
    for x in bl {
        let text = x.to_string();
        if !in_o.contains(&text) && seen.insert(text.clone()) {
            merged.push((text, x.clone()));
        }
    }
    if order == MergeOrder::Canonical {
        merged.sort_by(|x, y| x.0.cmp(&y.0));
    }
    let empty = merged.is_empty();
    let list = Value::Array(merged.into_iter().map(|(_, v)| v).collect());
    Some(present(list, empty, a, b))
}

// ---- files ------------------------------------------------------------------------------------------------------

/// Merges one project file three-way (doc 05 §11.2, §11.3; WP-S1.17 D2, D14, D16): the entry point of the merge driver.
/// `path` is the file's project-relative path (`/` separators) and selects the handling: a record file merges per
/// `($k, id)` ([`merge_record`]), a library document as one record, `manifest.json` by doc 05 §11.3; a file of any
/// other class keeps ours with one conflict when both sides changed it. `chain` migrates an older side ([`MIGRATIONS`]
/// in production). The result is the merged file's bytes and the number of conflicts written; it never fails, and a
/// side that cannot be merged is kept verbatim in a conflict record.
///
/// **The one loss.** The result never holds a record line longer than `limits.max_line_bytes` or nested deeper than
/// `limits.max_depth` (a library document: the document), which the loader would refuse (D14: "the file stays
/// loadable"). A conflict record nests its partial objects one level below its target, `$conflict` holds theirs'
/// document one level below ours', and a file-level record holds a side's lines in one line, so a merge can pass a
/// limit that neither side passes. Then ours is returned as it is with 1 conflict, and theirs survives only in git's
/// index (stage 3): D14's "nothing of either side is lost" and MD-2 give way for this one case (decision log ID-1273).
///
/// [`MIGRATIONS`]: crate::MIGRATIONS
pub fn merge_file(
    path: &str,
    base: MergeSide<'_>,
    ours: MergeSide<'_>,
    theirs: MergeSide<'_>,
    limits: &Limits,
    chain: &Migrations,
) -> FileMerge {
    if ours.bytes == theirs.bytes || theirs.bytes == base.bytes {
        return FileMerge {
            bytes: ours.bytes.to_vec(),
            conflicts: 0,
        };
    }
    if ours.bytes == base.bytes {
        return FileMerge {
            bytes: theirs.bytes.to_vec(),
            conflicts: 0,
        };
    }
    match classify(path) {
        PathClass::Records(_) => merge_records_file(path, base, ours, theirs, limits, chain),
        PathClass::LibDocument { .. } => merge_document(path, base, ours, theirs, limits, chain),
        PathClass::Manifest => merge_manifest(base, ours, theirs, limits, chain),
        _ => FileMerge {
            bytes: ours.bytes.to_vec(),
            conflicts: 1,
        },
    }
}

/// The migration of a side of `format` towards this reader's: none within the reader's major at or below it, the chain
/// from an older major; `None` for a format newer than the reader's, one that does not parse, or one the chain does not
/// reach (decision log ID-1268 ruling 6).
fn migration<'c>(format: &str, chain: &'c Migrations) -> Option<Migration<'c>> {
    let (side, reader) = (version::parse(format)?, version::parse(FORMAT)?);
    if side > reader {
        return None;
    }
    let from = if version::same_major(side, reader) {
        FORMAT
    } else {
        format
    };
    chain.towards(from, FORMAT).ok()
}

/// Whether a value's `$v` is above its kind's schema version (a newer record, MD-4).
fn newer_record(value: &Value) -> bool {
    let Some(kind) = value.get("$k").and_then(Value::as_str).and_then(Kind::from_name) else {
        return false;
    };
    value
        .get("$v")
        .and_then(Value::as_u64)
        .is_some_and(|v| v > u64::from(kind.schema_version()))
}

/// The `($k, id)` key of a record value.
fn record_key(value: &Value) -> Option<(String, String)> {
    let k = value.get("$k")?.as_str()?;
    let id = value.get("id")?.as_str()?.parse::<ew_schema::AnyId>().ok()?;
    Some((k.to_owned(), id.to_string()))
}

/// One record of a side as read: its value, and the side's own line when that value is the line's as written (no
/// migration step and no NFC normalisation changed it).
struct ReadRecord<'b> {
    value: Value,
    line: Option<&'b str>,
}

/// The records of one side of a record file by key, read as the loader reads them, migrated and NFC-normalised; `None`
/// when the side cannot be merged: a limit, a line that is not a record with a key, a key twice, a failed step, or a
/// newer format or record.
fn read_records<'b>(
    side: MergeSide<'b>,
    limits: &Limits,
    chain: &Migrations,
) -> Option<BTreeMap<(String, String), ReadRecord<'b>>> {
    if side.bytes.len() as u64 > limits.max_file_bytes {
        return None;
    }
    let migration = migration(side.format, chain)?;
    let data = side.bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(side.bytes);
    let body = data.strip_suffix(b"\n").unwrap_or(data);
    let mut out = BTreeMap::new();
    for (i, raw) in body.split(|b| *b == b'\n').enumerate() {
        if i >= limits.max_lines_per_file {
            return None;
        }
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        if raw.is_empty() {
            continue;
        }
        let (text, parsed) = parse_line(raw, limits).ok()?;
        let mut value = migration.record(parsed.clone()).ok()?;
        let normalised = ew_schema::normalize_nfc(&mut value);
        if newer_record(&value) {
            return None;
        }
        let line = (!normalised && value == parsed).then_some(text);
        if out
            .insert(record_key(&value)?, ReadRecord { value, line })
            .is_some()
        {
            return None;
        }
    }
    Some(out)
}

/// The canonical line of a record value: its typed form's, or its compact JSON when it does not read typed.
fn line_of(value: &Value) -> (LineKey, String) {
    let text = Record::load(value.clone())
        .ok()
        .and_then(|(record, _)| record_line(&record).ok())
        .unwrap_or_else(|| value.to_string());
    (LineKey::of_value(value), text)
}

/// The line of a merged record: the own line of a side whose record it is unchanged, so that an unknown kind or the
/// members of a newer minor keep their order (doc 05 §10.3, FR-COL-010; decision log ID-1273), else its canonical line.
fn merged_line(merged: &Value, sides: [Option<&ReadRecord<'_>>; 2]) -> (LineKey, String) {
    match sides
        .into_iter()
        .flatten()
        .filter(|side| side.value == *merged)
        .find_map(|side| side.line)
    {
        Some(line) => (LineKey::of_value(merged), line.to_owned()),
        None => line_of(merged),
    }
}

/// A record-level conflict's line (D14): its members in that order, `$k` and `id` first (doc 05 §3.1 rule 1).
#[derive(Serialize)]
struct RecordConflictLine<'a> {
    #[serde(rename = "$k")]
    k: &'a Value,
    id: &'a Value,
    kind: &'a Value,
    base: &'a Value,
    ours: &'a Value,
    theirs: &'a Value,
}

/// The line of a record-level conflict record ([`conflict_record`]), its members in D14's order: the `Value` sorts them.
fn conflict_line(conflict: &Value) -> (LineKey, String) {
    let line = RecordConflictLine {
        k: &conflict["$k"],
        id: &conflict["id"],
        kind: &conflict["kind"],
        base: &conflict["base"],
        ours: &conflict["ours"],
        theirs: &conflict["theirs"],
    };
    let text = serde_json::to_string(&line).unwrap_or_else(|_| conflict.to_string());
    (LineKey::of_value(conflict), text)
}

/// The file-level conflict record's line (D14): its members in that order, `$k` and `id` first.
#[derive(Serialize)]
struct FileConflictLine<'a> {
    #[serde(rename = "$k")]
    k: &'a str,
    id: String,
    kind: &'a str,
    path: &'a str,
    side: &'a str,
    lines: Vec<&'a str>,
}

/// The line of the file-level conflict record of `path` holding `side`'s lines verbatim (D14).
fn file_conflict(path: &str, side: &str, bytes: &[u8]) -> (LineKey, String) {
    let text = String::from_utf8_lossy(bytes);
    let body = text.strip_suffix('\n').unwrap_or(&text);
    let lines: Vec<&str> = if bytes.is_empty() {
        Vec::new()
    } else {
        body.split('\n').collect()
    };
    let id = conflict_file_id::<()>(path);
    let line = FileConflictLine {
        k: "conflict",
        id: id.to_string(),
        kind: "file",
        path,
        side,
        lines,
    };
    let key = LineKey::new("conflict", Some(AnyId::from_uuid(id.into())));
    // Strings and a list of strings always serialise.
    (key, serde_json::to_string(&line).unwrap_or_default())
}

/// Whether a line is within the loader's line limits (doc 05 §3.6): its length and its nesting.
fn within_limits(line: &str, limits: &Limits) -> bool {
    line.len() <= limits.max_line_bytes && !exceeds_depth(line.as_bytes(), limits.max_depth)
}

/// Ours as it is: the merged file would pass a reader limit (decision log ID-1273). Theirs survives in git's index.
fn ours_as_it_is(ours: MergeSide<'_>) -> FileMerge {
    FileMerge {
        bytes: ours.bytes.to_vec(),
        conflicts: 1,
    }
}

/// The records of the side that reads, each by its own line where it has one, and the file-level conflict holding the
/// other side.
fn with_failed(
    kept: &BTreeMap<(String, String), ReadRecord<'_>>,
    failed: (LineKey, String),
) -> Vec<(LineKey, String)> {
    let mut lines: Vec<(LineKey, String)> = kept
        .values()
        .map(|r| merged_line(&r.value, [Some(r), None]))
        .collect();
    lines.push(failed);
    lines
}

/// Neither side reads: ours as it is, theirs in the file-level conflict record appended to it, unless that record's
/// line would pass a reader limit.
fn hold_theirs(path: &str, ours: MergeSide<'_>, theirs: MergeSide<'_>, limits: &Limits) -> FileMerge {
    let (_, held) = file_conflict(path, "theirs", theirs.bytes);
    if !within_limits(&held, limits) {
        return ours_as_it_is(ours);
    }
    let mut bytes = ours.bytes.to_vec();
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    bytes.extend(held.bytes());
    bytes.push(b'\n');
    FileMerge { bytes, conflicts: 1 }
}

fn merge_records_file(
    path: &str,
    base: MergeSide<'_>,
    ours: MergeSide<'_>,
    theirs: MergeSide<'_>,
    limits: &Limits,
    chain: &Migrations,
) -> FileMerge {
    // An empty base has no records (a file added on both sides); one that does not read follows D18's no-base rule.
    let o = if base.bytes.is_empty() {
        Some(BTreeMap::new())
    } else {
        read_records(base, limits, chain)
    };
    let (lines, conflicts) = match (
        read_records(ours, limits, chain),
        read_records(theirs, limits, chain),
    ) {
        (Some(a), Some(b)) => {
            let keys: BTreeSet<&(String, String)> = o
                .iter()
                .flat_map(BTreeMap::keys)
                .chain(a.keys())
                .chain(b.keys())
                .collect();
            let mut lines = Vec::new();
            let mut conflicts = 0;
            for key in keys {
                let (ra, rb) = (a.get(key), b.get(key));
                let (va, vb) = (ra.map(|r| &r.value), rb.map(|r| &r.value));
                let merged = match &o {
                    Some(o) => merge_record(o.get(key).map(|r| &r.value), va, vb),
                    None => no_base(va, vb),
                };
                lines.extend(merged.merged.as_ref().map(|m| merged_line(m, [ra, rb])));
                if let Some(conflict) = &merged.conflict {
                    conflicts += 1;
                    lines.push(conflict_line(conflict));
                }
            }
            (lines, conflicts)
        }
        (Some(a), None) => (with_failed(&a, file_conflict(path, "theirs", theirs.bytes)), 1),
        (None, Some(b)) => (with_failed(&b, file_conflict(path, "ours", ours.bytes)), 1),
        (None, None) => return hold_theirs(path, ours, theirs, limits),
    };
    if !lines.iter().all(|(_, line)| within_limits(line, limits)) {
        return ours_as_it_is(ours);
    }
    FileMerge {
        bytes: jsonl_file(lines),
        conflicts,
    }
}

/// One side's library document, read as the loader reads it, migrated (its copy rehashed) and NFC-normalised; `None`
/// when it cannot be merged.
fn read_document(side: MergeSide<'_>, limits: &Limits, chain: &Migrations) -> Option<Value> {
    if side.bytes.len() as u64 > limits.max_file_bytes {
        return None;
    }
    let migration = migration(side.format, chain)?;
    let before = parse_document(side.bytes, limits).ok()?;
    let mut value = migration.record(before.clone()).ok()?;
    rehash_migrated_copy(&before, &mut value);
    ew_schema::normalize_nfc(&mut value);
    value.get("$k")?.as_str()?;
    (!newer_record(&value)).then_some(value)
}

/// The canonical text of a library document value: its typed form's, or the pretty form of its JSON.
fn document_text(value: &Value) -> Vec<u8> {
    LibDocument::load(value.clone())
        .ok()
        .filter(|(_, malformed)| malformed.is_empty())
        .and_then(|(doc, _)| lib_document_text(&doc).ok())
        .unwrap_or_else(|| pretty(&value.to_string()))
        .into_bytes()
}

/// `value` with the top-level member `$conflict` (D14).
fn with_conflict(mut value: Value, other: Value) -> Vec<u8> {
    if let Value::Object(members) = &mut value {
        members.insert("$conflict".to_owned(), other);
    }
    document_text(&value)
}

fn merge_document(
    path: &str,
    base: MergeSide<'_>,
    ours: MergeSide<'_>,
    theirs: MergeSide<'_>,
    limits: &Limits,
    chain: &Migrations,
) -> FileMerge {
    let text = |side: MergeSide<'_>| Value::from(String::from_utf8_lossy(side.bytes).into_owned());
    let (bytes, conflicts) = match (
        read_document(ours, limits, chain),
        read_document(theirs, limits, chain),
    ) {
        (Some(a), Some(b)) => {
            // An empty base: added on both sides; a base that does not read: D18's no-base rule.
            let merged = match (base.bytes.is_empty(), read_document(base, limits, chain)) {
                (false, None) => no_base(Some(&a), Some(&b)),
                (_, o) => merge_record(o.as_ref(), Some(&a), Some(&b)),
            };
            let doc = merged.merged.unwrap_or(a);
            match merged.conflict {
                None => (document_text(&doc), 0),
                Some(_) => (with_conflict(doc, b), 1),
            }
        }
        (Some(a), None) => (with_conflict(a, text(theirs)), 1),
        (None, Some(b)) => (with_conflict(b, text(ours)), 1),
        // Neither side reads: theirs is held by the file-level record when ours does not open either (D14's
        // `$conflict` needs a readable ours); an ours that opens stays as it is, since a line after it would make
        // it a syntax error (decision log ID-1273).
        (None, None) => {
            return match parse_document(ours.bytes, limits) {
                Err(fault) if fault.kind.forces_read_only() => hold_theirs(path, ours, theirs, limits),
                _ => ours_as_it_is(ours),
            };
        }
    };
    // `$conflict` holds theirs' document one level below the merged one's root.
    if exceeds_depth(&bytes, limits.max_depth) {
        return ours_as_it_is(ours);
    }
    FileMerge { bytes, conflicts }
}

/// One side's manifest, an older major migrated; `None` when it does not read.
fn read_manifest(bytes: &[u8], limits: &Limits, chain: &Migrations) -> Option<Manifest> {
    let manifest = Manifest::parse(bytes, limits).ok()?;
    let (side, reader) = (version::parse(&manifest.format)?, version::parse(FORMAT)?);
    if side < reader && !version::same_major(side, reader) {
        return chain
            .towards(&manifest.format, FORMAT)
            .ok()?
            .manifest(&manifest, limits)
            .ok();
    }
    Some(manifest)
}

/// The later of two formats; `a` when they are equal or neither parses.
fn later_format(a: &str, b: &str) -> String {
    if version::parse(b) > version::parse(a) {
        b.to_owned()
    } else {
        a.to_owned()
    }
}

/// Three-way on one entry: the side that changed it, else `both` (ours for a member, pending for a hash).
fn three_way<T: PartialEq + Clone>(
    o: Option<&T>,
    a: Option<&T>,
    b: Option<&T>,
    both: Option<T>,
) -> Option<T> {
    if a == b || b == o {
        a.cloned()
    } else if a == o {
        b.cloned()
    } else {
        both
    }
}

/// `manifest.json` (doc 05 §11.3, D16): `generation = max(a, b) + 1`, `format` and `min_reader` the later, `generator`
/// and `project` ours, `files` three-way per path with `sha256:pending` where both changed, `merged: true`, no `base`.
fn merge_manifest(
    base: MergeSide<'_>,
    ours: MergeSide<'_>,
    theirs: MergeSide<'_>,
    limits: &Limits,
    chain: &Migrations,
) -> FileMerge {
    let o = read_manifest(base.bytes, limits, chain);
    let next = |g: u64| g.saturating_add(1).min(MAX_GENERATION);
    let (merged, conflicts) = match (
        read_manifest(ours.bytes, limits, chain),
        read_manifest(theirs.bytes, limits, chain),
    ) {
        (Some(a), Some(b)) => {
            let mut m = a.clone();
            m.format = later_format(&a.format, &b.format);
            m.min_reader = later_format(&a.min_reader, &b.min_reader);
            m.generation = next(a.generation.max(b.generation));
            let paths: BTreeSet<_> = o
                .as_ref()
                .map(|m| m.files.keys())
                .into_iter()
                .flatten()
                .chain(a.files.keys())
                .chain(b.files.keys())
                .collect();
            m.files = paths
                .into_iter()
                .filter_map(|p| {
                    let ho = o.as_ref().and_then(|m| m.files.get(p));
                    three_way(ho, a.files.get(p), b.files.get(p), Some(FileHash::Pending))
                        .map(|h| (p.clone(), h))
                })
                .collect();
            m.unstable = a.unstable || b.unstable;
            let names: BTreeSet<_> = o
                .as_ref()
                .map(|m| m.extra.keys())
                .into_iter()
                .flatten()
                .chain(a.extra.keys())
                .chain(b.extra.keys())
                .collect();
            m.extra = names
                .into_iter()
                .filter_map(|k| {
                    let vo = o.as_ref().and_then(|m| m.extra.get(k));
                    let (va, vb) = (a.extra.get(k), b.extra.get(k));
                    three_way(vo, va, vb, va.cloned()).map(|v| (k.clone(), v))
                })
                .collect();
            // Two projects under one path: ours is kept, and the merge is not clean.
            let conflicts = usize::from(a.project != b.project);
            (m, conflicts)
        }
        (Some(mut m), None) | (None, Some(mut m)) => {
            m.generation = next(m.generation);
            (m, 1)
        }
        (None, None) => {
            return FileMerge {
                bytes: ours.bytes.to_vec(),
                conflicts: 1,
            };
        }
    };
    let mut merged = merged;
    merged.merged = true;
    merged.extra.remove(BASE);
    FileMerge {
        bytes: merged.to_text().into_bytes(),
        conflicts,
    }
}
