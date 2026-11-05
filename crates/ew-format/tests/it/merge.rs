// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The three-way merge core (WP-S1.17 part 4a: D2, D13, D14, D16; decision log ID-1268): MD-1 (each row of the record
//! table and each strategy), MD-2 (the properties), MD-3 (typed inputs merge typed, over Golden A's records), MD-4 (a
//! side that does not read or is newer), the manifest merge and AC-41's page half on W20. Golden A is only read.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ew_format::ew_schema::{FORMAT, Kind, KindStatus, LibDocument, Record};
use ew_format::{
    FilePath, FormatStep, Limits, MANIFEST, MIGRATIONS, Manifest, MergeSide, MigrationError, Migrations,
    PathClass, classify, load, merge_file, merge_record,
};
use proptest::prelude::*;
use serde_json::{Map, Value, json};

const ID: &str = "5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c";

/// A record of `kind` with the ID [`ID`] and `members`.
fn rec(kind: &str, members: Value) -> Value {
    let mut out = json!({"$k": kind, "id": ID});
    let (Value::Object(out_members), Value::Object(members)) = (&mut out, members) else {
        panic!("members are an object");
    };
    out_members.extend(members);
    out
}

/// The merge as compact texts: the merged record and the conflict record.
fn merged(o: Option<&Value>, a: Option<&Value>, b: Option<&Value>) -> (Option<String>, Option<String>) {
    let m = merge_record(o, a, b);
    (m.merged.map(|v| v.to_string()), m.conflict.map(|v| v.to_string()))
}

fn text(v: &Value) -> Option<String> {
    Some(v.to_string())
}

// ---- MD-1: the record table -------------------------------------------------------------------------------------

#[test]
fn md1_a_record_on_one_side_only_is_kept() {
    let a = rec("connection", json!({"page": "p"}));
    assert_eq!(merged(None, Some(&a), None), (text(&a), None));
    assert_eq!(merged(None, None, Some(&a)), (text(&a), None));
    assert_eq!(merged(None, Some(&a), Some(&a)), (text(&a), None));
}

#[test]
fn md1_a_record_added_differently_on_both_sides_merges_per_member_with_an_empty_base() {
    let a = rec("connection", json!({"page": "p", "overrides": {"csa": "1.5"}}));
    let b = rec("connection", json!({"number": 7, "overrides": {"csa": "2.5"}}));
    let (merged, conflict) = merged(None, Some(&a), Some(&b));
    assert_eq!(
        merged.as_deref(),
        Some(
            r#"{"$k":"connection","id":"5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c","number":7,"overrides":{"csa":"1.5"},"page":"p"}"#
        )
    );
    assert_eq!(
        conflict.as_deref(),
        Some(
            r#"{"$k":"conflict","base":{},"id":"5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c","kind":"connection","ours":{"overrides":{"csa":"1.5"}},"theirs":{"overrides":{"csa":"2.5"}}}"#
        )
    );
}

#[test]
fn md1_a_record_deleted_on_one_side_and_unchanged_on_the_other_is_deleted() {
    let o = rec("connection", json!({"page": "p"}));
    assert_eq!(merged(Some(&o), None, None), (None, None));
    assert_eq!(merged(Some(&o), None, Some(&o)), (None, None));
    assert_eq!(merged(Some(&o), Some(&o), None), (None, None));
}

#[test]
fn md1_delete_modify_keeps_the_modified_side_with_a_conflict_of_its_changes() {
    let o = rec("connection", json!({"page": "p", "overrides": {"csa": "1.5"}}));
    let b = rec(
        "connection",
        json!({"page": "p", "overrides": {"csa": "2.5"}, "number": 3}),
    );
    assert_eq!(
        merged(Some(&o), None, Some(&b)),
        (
            text(&b),
            Some(r#"{"$k":"conflict","base":{"overrides":{"csa":"1.5"}},"id":"5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c","kind":"connection","ours":"deleted","theirs":{"number":3,"overrides":{"csa":"2.5"}}}"#.to_owned())
        )
    );
    assert_eq!(
        merged(Some(&o), Some(&b), None),
        (
            text(&b),
            Some(r#"{"$k":"conflict","base":{"overrides":{"csa":"1.5"}},"id":"5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c","kind":"connection","ours":{"number":3,"overrides":{"csa":"2.5"}},"theirs":"deleted"}"#.to_owned())
        )
    );
}

#[test]
fn md1_disjoint_member_edits_on_both_sides_merge_cleanly() {
    let o = rec("connection", json!({"page": "p", "number": 1}));
    let a = rec("connection", json!({"page": "q", "number": 1}));
    let b = rec("connection", json!({"page": "p"}));
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (text(&rec("connection", json!({"page": "q"}))), None)
    );
}

#[test]
fn md1_an_atomic_member_changed_on_both_sides_conflicts_and_keeps_ours() {
    let o = rec("connection", json!({"page": "p"}));
    let a = rec("connection", json!({"page": "q"}));
    let b = rec("connection", json!({}));
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&a),
            Some(r#"{"$k":"conflict","base":{"page":"p"},"id":"5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c","kind":"connection","ours":{"page":"q"},"theirs":{}}"#.to_owned())
        )
    );
}

// ---- MD-1: the strategies ---------------------------------------------------------------------------------------

#[test]
fn md1_object_merges_the_overrides_member_by_member() {
    // AC-41's clean variant (D29): A adds colour BU, B sets csa 2.5.
    let o = rec(
        "connection",
        json!({"overrides": {"csa": "1.5", "show_marker": true}}),
    );
    let a = rec(
        "connection",
        json!({"overrides": {"csa": "1.5", "colour": ["BU"], "show_marker": true}}),
    );
    let b = rec(
        "connection",
        json!({"overrides": {"csa": "2.5", "show_marker": true}}),
    );
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&rec(
                "connection",
                json!({"overrides": {"csa": "2.5", "colour": ["BU"], "show_marker": true}})
            )),
            None
        )
    );
}

#[test]
fn md1_object_conflict_holds_only_the_conflicting_member() {
    // AC-41's conflict variant: B also adds colour WH.
    let o = rec(
        "connection",
        json!({"overrides": {"csa": "1.5", "show_marker": true}}),
    );
    let a = rec(
        "connection",
        json!({"overrides": {"csa": "1.5", "colour": ["BU"], "show_marker": true}}),
    );
    let b = rec(
        "connection",
        json!({"overrides": {"csa": "2.5", "colour": ["WH"], "show_marker": true}}),
    );
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&rec("connection", json!({"overrides": {"csa": "2.5", "colour": ["BU"], "show_marker": true}}))),
            Some(r#"{"$k":"conflict","base":{"overrides":{}},"id":"5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c","kind":"connection","ours":{"overrides":{"colour":["BU"]}},"theirs":{"overrides":{"colour":["WH"]}}}"#.to_owned())
        )
    );
}

#[test]
fn md1_an_object_absent_on_one_side_counts_as_empty() {
    let o = rec("connection", json!({"overrides": {"csa": "1.5"}}));
    let a = rec("connection", json!({}));
    let b = rec(
        "connection",
        json!({"overrides": {"csa": "1.5", "colour": ["BU"]}}),
    );
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&rec("connection", json!({"overrides": {"colour": ["BU"]}}))),
            None
        )
    );
}

#[test]
fn md1_an_object_strategy_needs_the_member_at_the_base() {
    let o = rec("connection", json!({}));
    let a = rec("connection", json!({"overrides": {"csa": "1.5"}}));
    let b = rec("connection", json!({"overrides": {"colour": ["BU"]}}));
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&a),
            Some(r#"{"$k":"conflict","base":{},"id":"5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c","kind":"connection","ours":{"overrides":{"csa":"1.5"}},"theirs":{"overrides":{"colour":["BU"]}}}"#.to_owned())
        )
    );
}

#[test]
fn md1_below_an_object_members_are_atomic() {
    let o = rec("connection", json!({"overrides": {"flags": {"shielded": true}}}));
    let a = rec(
        "connection",
        json!({"overrides": {"flags": {"shielded": true, "twisted": true}}}),
    );
    let b = rec("connection", json!({"overrides": {"flags": {}}}));
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&a),
            Some(r#"{"$k":"conflict","base":{"overrides":{"flags":{"shielded":true}}},"id":"5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c","kind":"connection","ours":{"overrides":{"flags":{"shielded":true,"twisted":true}}},"theirs":{"overrides":{"flags":{}}}}"#.to_owned())
        )
    );
}

#[test]
fn md1_an_enum_changed_to_different_variants_is_one_conflict_without_a_union() {
    let o = rec("connection", json!({"kind": {"bridge": {"group": 1}}}));
    let a = rec(
        "connection",
        json!({"kind": {"core": "0192f3a4-5b6c-7d8e-9f01-23456789abcd"}}),
    );
    let b = rec("connection", json!({"kind": {"bridge": {"group": 2}}}));
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&a),
            Some(r#"{"$k":"conflict","base":{"kind":{"bridge":{"group":1}}},"id":"5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c","kind":"connection","ours":{"kind":{"core":"0192f3a4-5b6c-7d8e-9f01-23456789abcd"}},"theirs":{"kind":{"bridge":{"group":2}}}}"#.to_owned())
        )
    );
}

#[test]
fn md1_keyed_cp_overrides_merge_by_idx_in_numeric_order() {
    let o = rec(
        "function",
        json!({"cp_overrides": [{"idx": 1, "alias": "A"}, {"idx": 9, "alias": "B"}]}),
    );
    let a = rec(
        "function",
        json!({"cp_overrides": [{"idx": 1, "alias": "A2"}, {"idx": 9, "alias": "B"}]}),
    );
    let b = rec(
        "function",
        json!({"cp_overrides": [{"idx": 1, "alias": "A"}, {"idx": 10, "alias": "C"}]}),
    );
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&rec(
                "function",
                json!({"cp_overrides": [{"idx": 1, "alias": "A2"}, {"idx": 10, "alias": "C"}]})
            )),
            None
        )
    );
}

#[test]
fn md1_a_keyed_conflict_holds_ours_and_theirs_lists_as_merged() {
    let o = rec(
        "function",
        json!({"cp_overrides": [{"idx": 1, "alias": "A"}, {"idx": 9, "alias": "B"}]}),
    );
    let a = rec(
        "function",
        json!({"cp_overrides": [{"idx": 1, "alias": "A2"}, {"idx": 9, "alias": "B"}]}),
    );
    let b = rec(
        "function",
        json!({"cp_overrides": [{"idx": 1, "alias": "A3"}, {"idx": 9, "alias": "B"}, {"idx": 10, "alias": "C"}]}),
    );
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&rec(
                "function",
                json!({"cp_overrides": [{"idx": 1, "alias": "A2"}, {"idx": 9, "alias": "B"}, {"idx": 10, "alias": "C"}]})
            )),
            Some(r#"{"$k":"conflict","base":{"cp_overrides":[{"alias":"A","idx":1},{"alias":"B","idx":9}]},"id":"5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c","kind":"function","ours":{"cp_overrides":[{"alias":"A2","idx":1},{"alias":"B","idx":9},{"alias":"C","idx":10}]},"theirs":{"cp_overrides":[{"alias":"A3","idx":1},{"alias":"B","idx":9},{"alias":"C","idx":10}]}}"#.to_owned())
        )
    );
}

#[test]
fn md1_a_keyed_list_whose_keys_mix_integers_and_other_numbers_merges_atomically() {
    // 40 elements, more than a sort orders by insertion alone: `idx` 1 to 20 and 1.5 to 20.5, which no order sorts both
    // as numbers and as text (10 < 2.5 by text, 2.5 < 3 by number). The keys are neither all unsigned integers nor
    // all strings, so the list merges as one value (decision log ID-1273).
    let list = |changed: (usize, &str)| {
        let mut items = Vec::new();
        for i in 1..=20u32 {
            items.push(json!({"idx": i, "alias": "A"}));
            items.push(json!({"idx": f64::from(i) + 0.5, "alias": "B"}));
        }
        items[changed.0]["alias"] = json!(changed.1);
        Value::Array(items)
    };
    let (lo, la, lb) = (list((0, "A")), list((0, "A2")), list((2, "A3")));
    let o = rec("function", json!({"cp_overrides": lo}));
    let a = rec("function", json!({"cp_overrides": la}));
    let b = rec("function", json!({"cp_overrides": lb}));
    let m = std::panic::catch_unwind(|| merge_record(Some(&o), Some(&a), Some(&b))).expect("no panic");
    assert_eq!(m.merged, Some(a));
    assert_eq!(
        m.conflict,
        Some(json!({
            "$k": "conflict", "id": ID, "kind": "function",
            "base": {"cp_overrides": lo}, "ours": {"cp_overrides": la}, "theirs": {"cp_overrides": lb}
        }))
    );
}

#[test]
fn md1_keyed_tech_data_merges_by_key_in_byte_order() {
    let o = rec(
        "device",
        json!({"tech_data": [{"key": "curve", "value": "B"}, {"key": "i_out", "value": "6 A"}]}),
    );
    let a = rec(
        "device",
        json!({"tech_data": [{"key": "curve", "value": "B"}, {"key": "i_out", "value": "6 A"}, {"key": "u_n", "value": "230 V"}]}),
    );
    let b = rec(
        "device",
        json!({"tech_data": [{"key": "Z", "value": "1"}, {"key": "curve", "value": "C"}, {"key": "i_out", "value": "6 A"}]}),
    );
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&rec(
                "device",
                json!({"tech_data": [{"key": "Z", "value": "1"}, {"key": "curve", "value": "C"}, {"key": "i_out", "value": "6 A"}, {"key": "u_n", "value": "230 V"}]})
            )),
            None
        )
    );
}

#[test]
fn md1_a_part_s_keyed_tech_data_merges_by_key() {
    // D13's `keyed(key)` names `tech_data` without a kind: a part's list merges as a device's does.
    let o = rec("part", json!({"tech_data": [{"key": "curve", "value": "B"}]}));
    let a = rec(
        "part",
        json!({"tech_data": [{"key": "curve", "value": "B"}, {"key": "u_n", "value": "230 V"}]}),
    );
    let b = rec("part", json!({"tech_data": [{"key": "curve", "value": "C"}]}));
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&rec(
                "part",
                json!({"tech_data": [{"key": "curve", "value": "C"}, {"key": "u_n", "value": "230 V"}]})
            )),
            None
        )
    );
}

#[test]
fn md1_a_device_s_tag_and_the_project_s_settings_merge_member_by_member() {
    // MD-1's "each row": without its row, either member would merge atomically and conflict here.
    for (kind, member) in [("device", "tag"), ("project", "settings")] {
        let side = |x: u8, y: u8| rec(kind, json!({member: {"x": x, "y": y}}));
        assert_eq!(
            merged(Some(&side(1, 1)), Some(&side(2, 1)), Some(&side(1, 3))),
            (text(&side(2, 3)), None),
            "{kind}.{member}"
        );
    }
}

#[test]
fn md1_keyed_parties_merge_by_role_in_ours_order_then_theirs_additions() {
    let o = rec("project", json!({"parties": [{"role": "customer", "name": "c"}]}));
    let a = rec(
        "project",
        json!({"parties": [{"role": "engineer", "name": "e"}, {"role": "customer", "name": "c2"}]}),
    );
    let b = rec(
        "project",
        json!({"parties": [{"role": "customer", "name": "c"}, {"role": "author", "name": "w"}]}),
    );
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&rec(
                "project",
                json!({"parties": [{"role": "engineer", "name": "e"}, {"role": "customer", "name": "c2"}, {"role": "author", "name": "w"}]})
            )),
            None
        )
    );
}

#[test]
fn md1_two_parties_of_one_role_merge_the_member_atomically() {
    let o = rec("project", json!({"parties": [{"role": "customer", "name": "c"}]}));
    let a = rec(
        "project",
        json!({"parties": [{"role": "customer", "name": "c"}, {"role": "customer", "name": "d"}]}),
    );
    let b = rec(
        "project",
        json!({"parties": [{"role": "customer", "name": "c"}, {"role": "author", "name": "w"}]}),
    );
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&a),
            Some(r#"{"$k":"conflict","base":{"parties":[{"name":"c","role":"customer"}]},"id":"5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c","kind":"project","ours":{"parties":[{"name":"c","role":"customer"},{"name":"d","role":"customer"}]},"theirs":{"parties":[{"name":"c","role":"customer"},{"name":"w","role":"author"}]}}"#.to_owned())
        )
    );
}

#[test]
fn md1_a_set_keeps_both_sides_additions_less_either_sides_removals_sorted() {
    let o = rec("structure_node", json!({"aliases": ["JE2416", "M"]}));
    let a = rec("structure_node", json!({"aliases": ["JE2416", "M", "ZZ"]}));
    let b = rec("structure_node", json!({"aliases": ["AA", "M"]}));
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&rec("structure_node", json!({"aliases": ["AA", "M", "ZZ"]}))),
            None
        )
    );
}

#[test]
fn md1_a_set_of_a_plain_list_keeps_ours_order_then_theirs_additions() {
    let o = rec("manufacturer", json!({"aliases": ["Schneider", "Merlin"]}));
    let a = rec("manufacturer", json!({"aliases": ["Schneider", "Merlin", "SE"]}));
    let b = rec("manufacturer", json!({"aliases": ["Telemecanique", "Schneider"]}));
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&rec(
                "manufacturer",
                json!({"aliases": ["Schneider", "SE", "Telemecanique"]})
            )),
            None
        )
    );
}

#[test]
fn md1_max_takes_the_later_edited_date_and_its_object_merges_beside_it() {
    let o = rec(
        "project",
        json!({"master": {"description": "x", "edited": "2025-04-14"}}),
    );
    let a = rec(
        "project",
        json!({"master": {"description": "x", "edited": "2025-06-01"}}),
    );
    let b = rec(
        "project",
        json!({"master": {"description": "y", "edited": "2025-05-02"}}),
    );
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&b)),
        (
            text(&rec(
                "project",
                json!({"master": {"description": "y", "edited": "2025-06-01"}})
            )),
            None
        )
    );
    assert_eq!(
        merged(Some(&o), Some(&b), Some(&a)),
        (
            text(&rec(
                "project",
                json!({"master": {"description": "y", "edited": "2025-06-01"}})
            )),
            None
        )
    );
    // A value that is not a date merges atomically.
    let c = rec(
        "project",
        json!({"master": {"description": "x", "edited": "soon"}}),
    );
    assert_eq!(
        merged(Some(&o), Some(&a), Some(&c)),
        (
            text(&a),
            Some(r#"{"$k":"conflict","base":{"master":{"edited":"2025-04-14"}},"id":"5e07c3b9-1a4d-7f82-a6c3-90d8e2b71f4c","kind":"project","ours":{"master":{"edited":"2025-06-01"}},"theirs":{"master":{"edited":"soon"}}}"#.to_owned())
        )
    );
}

#[test]
fn md1_a_record_of_a_kind_this_reader_does_not_implement_merges_as_a_whole() {
    for kind in ["frobnicator", "comment"] {
        let o = rec(kind, json!({"x": 1, "y": 1}));
        let a = rec(kind, json!({"x": 2, "y": 1}));
        let b = rec(kind, json!({"x": 1, "y": 2}));
        let (merged_text, conflict) = merged(Some(&o), Some(&a), Some(&b));
        assert_eq!(merged_text, text(&a));
        assert_eq!(
            conflict,
            Some(format!(
                r#"{{"$k":"conflict","base":{o},"id":"{ID}","kind":"{kind}","ours":{a},"theirs":{b}}}"#
            ))
        );
        // One side's change alone is taken whole.
        assert_eq!(merged(Some(&o), Some(&o), Some(&b)), (text(&b), None));
    }
}

// ---- MD-2: properties ------------------------------------------------------------------------------------------

fn member(values: &[Value]) -> BoxedStrategy<Option<Value>> {
    prop::option::of(prop::sample::select(values.to_vec())).boxed()
}

fn put(record: &mut Value, name: &str, value: Option<Value>) {
    if let (Some(value), Value::Object(members)) = (value, record) {
        members.insert(name.to_owned(), value);
    }
}

fn connection() -> BoxedStrategy<Value> {
    let overrides = prop::option::of((
        member(&[json!("1.5"), json!("2.5")]),
        member(&[json!(["BU"]), json!(["WH"])]),
        member(&[json!(true)]),
        member(&[
            json!({"shielded": true}),
            json!({"shielded": true, "twisted": true}),
        ]),
    ));
    (member(&[json!(1), json!(2)]), overrides)
        .prop_map(|(number, overrides)| {
            let mut r = rec("connection", json!({}));
            put(&mut r, "number", number);
            if let Some((csa, colour, show, flags)) = overrides {
                let mut o = json!({});
                put(&mut o, "csa", csa);
                put(&mut o, "colour", colour);
                put(&mut o, "show_marker", show);
                put(&mut o, "flags", flags);
                put(&mut r, "overrides", Some(o));
            }
            r
        })
        .boxed()
}

fn function() -> BoxedStrategy<Value> {
    let cps = prop::collection::btree_map(0u16..3, prop::sample::select(vec!["A", "B"]), 0..3);
    (member(&[json!("t1"), json!("t2")]), prop::option::of(cps))
        .prop_map(|(text, cps)| {
            let mut r = rec("function", json!({}));
            put(&mut r, "function_text", text);
            let cps = cps.map(|cps| {
                Value::Array(
                    cps.into_iter()
                        .map(|(idx, alias)| json!({"idx": idx, "alias": alias}))
                        .collect(),
                )
            });
            put(&mut r, "cp_overrides", cps);
            r
        })
        .boxed()
}

fn project() -> BoxedStrategy<Value> {
    let master = prop::option::of((
        member(&[json!("2025-04-14"), json!("2025-05-02"), json!("2025-06-01")]),
        member(&[json!("p"), json!("q")]),
    ));
    let party = (
        prop::sample::select(vec!["customer", "author"]),
        prop::sample::select(vec!["n1", "n2"]),
    )
        .prop_map(|(role, name)| json!({"role": role, "name": name}));
    let parties = prop::option::of(prop::collection::vec(party, 0..3));
    (master, parties)
        .prop_map(|(master, parties)| {
            let mut r = rec("project", json!({}));
            if let Some((edited, description)) = master {
                let mut m = json!({});
                put(&mut m, "edited", edited);
                put(&mut m, "description", description);
                put(&mut r, "master", Some(m));
            }
            put(&mut r, "parties", parties.map(Value::Array));
            r
        })
        .boxed()
}

fn structure_node() -> BoxedStrategy<Value> {
    let aliases = prop::collection::btree_set(prop::sample::select(vec!["X", "Y", "Z"]), 0..3);
    (member(&[json!("c1"), json!("c2")]), prop::option::of(aliases))
        .prop_map(|(code, aliases)| {
            let mut r = rec("structure_node", json!({}));
            put(&mut r, "code", code);
            put(&mut r, "aliases", aliases.map(|a| json!(a)));
            r
        })
        .boxed()
}

/// Three records of one kind (or a side without it), from small domains so that sides often agree.
fn sides() -> impl Strategy<Value = (Option<Value>, Option<Value>, Option<Value>)> {
    (0..4usize).prop_flat_map(|k| {
        let one = move || match k {
            0 => connection(),
            1 => function(),
            2 => project(),
            _ => structure_node(),
        };
        let side = move || prop::option::weighted(0.9, one());
        (side(), side(), side())
    })
}

/// The value at `path` (`None` when absent on the way).
fn at<'v>(v: Option<&'v Value>, path: &[&str]) -> Option<&'v Value> {
    path.iter().try_fold(v?, |v, name| v.get(*name))
}

/// What a leaf of the generated kinds is merged as.
enum Leaf {
    Atomic(&'static [&'static str]),
    Keyed(&'static [&'static str], &'static str),
    Set(&'static [&'static str]),
    Max(&'static [&'static str]),
}

fn leaves(kind: &str) -> &'static [Leaf] {
    match kind {
        "connection" => &[
            Leaf::Atomic(&["number"]),
            Leaf::Atomic(&["overrides", "csa"]),
            Leaf::Atomic(&["overrides", "colour"]),
            Leaf::Atomic(&["overrides", "show_marker"]),
            Leaf::Atomic(&["overrides", "flags"]),
        ],
        "function" => &[
            Leaf::Atomic(&["function_text"]),
            Leaf::Keyed(&["cp_overrides"], "idx"),
        ],
        "project" => &[
            Leaf::Atomic(&["master", "description"]),
            Leaf::Max(&["master", "edited"]),
            Leaf::Keyed(&["parties"], "role"),
        ],
        _ => &[Leaf::Atomic(&["code"]), Leaf::Set(&["aliases"])],
    }
}

/// Whether the conflict record covers `path` with ours' value `a` and theirs' value `b`.
fn covers(conflict: Option<&Value>, path: &[&str], a: Option<&Value>, b: Option<&Value>) -> bool {
    conflict.is_some_and(|c| {
        (a.is_some() || b.is_some()) && at(Some(&c["ours"]), path) == a && at(Some(&c["theirs"]), path) == b
    })
}

/// The elements of a keyed list by key text, `None` when two share a key.
fn by_key(list: Option<&Value>, key: &str) -> Option<BTreeMap<String, Value>> {
    let mut out = BTreeMap::new();
    for e in list.and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        if out.insert(e[key].to_string(), e.clone()).is_some() {
            return None;
        }
    }
    Some(out)
}

/// The texts of a list's elements.
fn elements(list: Option<&Value>) -> BTreeSet<String> {
    list.and_then(Value::as_array)
        .map(|l| l.iter().map(Value::to_string).collect())
        .unwrap_or_default()
}

/// Every value a side changed is in the result or in the conflict record (MD-2), by the leaf's strategy.
fn assert_kept(o: &Value, a: &Value, b: &Value, r: &Value, c: Option<&Value>) {
    let kind = a["$k"].as_str().unwrap_or_default();
    for leaf in leaves(kind) {
        match leaf {
            Leaf::Atomic(p) => {
                let (vo, va, vb, vr) = (at(Some(o), p), at(Some(a), p), at(Some(b), p), at(Some(r), p));
                for v in [va, vb] {
                    if v != vo {
                        assert!(vr == v || covers(c, p, va, vb), "{p:?} lost {v:?}");
                    }
                }
            }
            Leaf::Max(p) => {
                let (vo, va, vb, vr) = (at(Some(o), p), at(Some(a), p), at(Some(b), p), at(Some(r), p));
                if vo.is_some() && va != vo && vb != vo {
                    let later = |v: Option<&Value>| v.and_then(Value::as_str).map(str::to_owned);
                    assert_eq!(later(vr), later(va).max(later(vb)), "{p:?}");
                } else {
                    for v in [va, vb] {
                        if v != vo {
                            assert!(vr == v || covers(c, p, va, vb), "{p:?} lost {v:?}");
                        }
                    }
                }
            }
            Leaf::Keyed(p, key) => {
                let lists = [at(Some(o), p), at(Some(a), p), at(Some(b), p), at(Some(r), p)];
                let maps: Option<Vec<_>> = lists.iter().map(|l| by_key(*l, key)).collect();
                let Some(maps) = maps else {
                    // Two elements of one key: the list is atomic.
                    let (va, vb) = (lists[1], lists[2]);
                    for v in [va, vb] {
                        if v != lists[0] {
                            assert!(lists[3] == v || covers(c, p, va, vb), "{p:?} lost {v:?}");
                        }
                    }
                    continue;
                };
                let in_conflict = |side: &str| c.and_then(|c| by_key(at(Some(&c[side]), p), key));
                let keys: BTreeSet<&String> = maps.iter().flat_map(|m| m.keys()).collect();
                for k in keys {
                    let (eo, ea, eb, er) = (maps[0].get(k), maps[1].get(k), maps[2].get(k), maps[3].get(k));
                    for e in [ea, eb] {
                        if e != eo {
                            let held = in_conflict("ours").is_some_and(|m| m.get(k) == ea)
                                && in_conflict("theirs").is_some_and(|m| m.get(k) == eb);
                            assert!(er == e || held, "{p:?}[{k}] lost {e:?}");
                        }
                    }
                }
            }
            Leaf::Set(p) => {
                let (lo, la, lb, lr) = (at(Some(o), p), at(Some(a), p), at(Some(b), p), at(Some(r), p));
                if lo.is_none() {
                    for v in [la, lb] {
                        if v != lo {
                            assert!(lr == v || covers(c, p, la, lb), "{p:?} lost {v:?}");
                        }
                    }
                    continue;
                }
                let (eo, ea, eb, er) = (elements(lo), elements(la), elements(lb), elements(lr));
                for added in ea.difference(&eo).chain(eb.difference(&eo)) {
                    assert!(er.contains(added), "{p:?} lost the addition {added}");
                }
                for removed in eo.difference(&ea).chain(eo.difference(&eb)) {
                    assert!(!er.contains(removed), "{p:?} kept the removal {removed}");
                }
            }
        }
    }
}

/// `value` with every `parties` list sorted: a plain list keeps ours' order (decision log ID-1268 ruling 3), so the
/// two side orders compare up to it.
fn unordered(value: Option<Value>) -> Option<Value> {
    fn walk(v: &mut Value) {
        match v {
            Value::Object(members) => {
                for (name, member) in members.iter_mut() {
                    if let (true, Value::Array(items)) = (name == "parties", &mut *member) {
                        items.sort_by_key(Value::to_string);
                    }
                    walk(member);
                }
            }
            Value::Array(items) => items.iter_mut().for_each(walk),
            _ => {}
        }
    }
    value.map(|mut v| {
        walk(&mut v);
        v
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// MD-2 on `merge_record`. Not on `merge_file`'s one loss, where a merged file would pass a reader limit and ours is
    /// kept as it is (`a_merge_that_would_pass_a_reader_limit_keeps_ours_as_it_is`, decision log ID-1273).
    #[test]
    fn md2_merge_properties((o, a, b) in sides()) {
        let (o, a, b) = (o.as_ref(), a.as_ref(), b.as_ref());
        // merge(o,a,a) = a and merge(o,o,b) = b.
        prop_assert_eq!(merge_record(o, a, a).merged, a.cloned());
        prop_assert_eq!(merge_record(o, a, a).conflict, None);
        prop_assert_eq!(merge_record(o, o, b).merged, b.cloned());
        prop_assert_eq!(merge_record(o, o, b).conflict, None);
        // merge(o,a,b) equals merge(o,b,a) up to the side names.
        let ab = merge_record(o, a, b);
        let ba = merge_record(o, b, a);
        match (&ab.conflict, &ba.conflict) {
            (None, None) => prop_assert_eq!(unordered(ab.merged.clone()), unordered(ba.merged.clone())),
            (Some(x), Some(y)) => {
                prop_assert_eq!(&x["id"], &y["id"]);
                prop_assert_eq!(&x["kind"], &y["kind"]);
                prop_assert_eq!(unordered(Some(x["base"].clone())), unordered(Some(y["base"].clone())));
                prop_assert_eq!(unordered(Some(x["ours"].clone())), unordered(Some(y["theirs"].clone())));
                prop_assert_eq!(unordered(Some(x["theirs"].clone())), unordered(Some(y["ours"].clone())));
            }
            _ => prop_assert!(false, "a conflict one way only: {:?} / {:?}", ab.conflict, ba.conflict),
        }
        // Every value of a and b is in the result or in the conflict record.
        if let (Some(o), Some(a), Some(b), Some(r)) = (o, a, b, ab.merged.as_ref()) {
            assert_kept(o, a, b, r, ab.conflict.as_ref());
        }
        if let (Some(o), None, Some(b)) = (o, a, b) {
            if o != b {
                prop_assert_eq!(ab.merged.as_ref(), Some(b));
                prop_assert_eq!(&ab.conflict.as_ref().map(|c| c["ours"].clone()), &Some(json!("deleted")));
            }
        }
    }
}

// ---- MD-3: typed inputs merge typed, over Golden A ---------------------------------------------------------------

/// Every file of `golden/J-E-2416.ew`, read only, by project-relative path.
fn golden() -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .expect("below the base")
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(&path).expect("a readable file"));
            }
        }
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
    let mut out = BTreeMap::new();
    walk(&dir, &dir, &mut out);
    assert!(!out.is_empty(), "the golden fixture {} is missing", dir.display());
    out
}

fn typed(value: &Value) -> bool {
    match value["$k"].as_str().and_then(Kind::from_name) {
        Some(kind) if kind.is_library() => LibDocument::from_value(value.clone()).is_ok(),
        _ => Record::from_value(value.clone()).is_ok(),
    }
}

/// `r` with the members of `s` chosen by `pick` (by their index among the names), recursing into objects both hold.
fn mix(r: &Value, s: &Value, pick: &dyn Fn(usize) -> bool) -> Value {
    let (Value::Object(rm), Value::Object(sm)) = (r, s) else {
        return s.clone();
    };
    let names: BTreeSet<&String> = rm.keys().chain(sm.keys()).collect();
    let mut out = Map::new();
    for (i, name) in names.into_iter().enumerate() {
        let keep = name == "$k" || name == "id" || !pick(i);
        let value = match (rm.get(name), sm.get(name)) {
            (r, _) if keep => r.cloned(),
            (Some(rv @ Value::Object(_)), Some(sv @ Value::Object(_))) => Some(mix(rv, sv, pick)),
            (_, s) => s.cloned(),
        };
        if let Some(value) = value {
            out.insert(name.clone(), value);
        }
    }
    Value::Object(out)
}

#[test]
fn md3_typed_records_and_documents_of_golden_a_merge_typed() {
    let mut by_kind: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for (path, bytes) in golden() {
        let text = String::from_utf8(bytes).expect("UTF-8");
        let values: Vec<Value> = match classify(&path) {
            PathClass::Records(_) => text
                .lines()
                .map(|l| serde_json::from_str(l).expect("JSON"))
                .collect(),
            PathClass::LibDocument { .. } => vec![serde_json::from_str(&text).expect("JSON")],
            _ => continue,
        };
        for value in values {
            let kind = value["$k"].as_str().and_then(Kind::from_name);
            if kind.is_some_and(|k| k.status() == KindStatus::Implemented) && typed(&value) {
                let name = value["$k"].as_str().unwrap_or_default().to_owned();
                by_kind.entry(name).or_default().push(value);
            }
        }
    }
    let (mut merges, mut failures) = (0, BTreeSet::new());
    for (kind, records) in &by_kind {
        let n = records.len();
        for (i, r) in records.iter().enumerate() {
            let (s, t) = (&records[(i + 1) % n], &records[(i + 2) % n]);
            let even = |i: usize| i % 2 == 0;
            let odd = |i: usize| i % 2 == 1;
            let all = |_: usize| true;
            for (a, b) in [
                (mix(r, s, &even), mix(r, t, &odd)),
                (mix(r, s, &all), mix(r, t, &all)),
                (mix(r, s, &odd), mix(r, t, &all)),
            ] {
                if !typed(&a) || !typed(&b) {
                    continue;
                }
                merges += 1;
                let m = merge_record(Some(r), Some(&a), Some(&b));
                if !m.merged.as_ref().is_some_and(typed) {
                    failures.insert(kind.clone());
                }
            }
        }
    }
    assert!(merges > 0);
    assert_eq!(
        failures,
        BTreeSet::new(),
        "kinds whose merged record does not read typed"
    );
}

#[test]
fn md3_the_project_merges_typed_through_its_strategies() {
    let files = golden();
    let text = String::from_utf8(files["project.jsonl"].clone()).expect("UTF-8");
    let line = text.lines().next().expect("the project line");
    let project: Value = serde_json::from_str(line).expect("JSON");
    let mut a = project.clone();
    a["master"]["edited"] = json!("2025-06-01");
    a["parties"]
        .as_array_mut()
        .expect("parties")
        .insert(0, json!({"role": "author", "name": "w"}));
    let mut b = project.clone();
    b["master"]["edited"] = json!("2025-05-02");
    b["master"]["description"] = json!("Expansion");
    b["parties"][0]["name"] = json!("c2");
    let m = merge_record(Some(&project), Some(&a), Some(&b));
    assert_eq!(m.conflict, None);
    let merged = m.merged.expect("merged");
    assert!(typed(&merged));
    assert_eq!(merged["master"]["edited"], json!("2025-06-01"));
    assert_eq!(merged["master"]["description"], json!("Expansion"));
    assert_eq!(merged["parties"][0], json!({"role": "author", "name": "w"}));
    assert_eq!(merged["parties"][1]["name"], json!("c2"));
}

// ---- files: AC-41's page half, MD-4 ----------------------------------------------------------------------------

const PAGE: &str = "pages/4aa43308-d0d3-57ee-be0c-d9383f429f50.jsonl";
/// `v5(NS_CONFLICT, PAGE)`, computed independently.
const PAGE_CONFLICT: &str = "c801da2a-0f8c-5452-8203-791712d85dcd";
const W20_OVERRIDES: &str = r#""overrides":{"csa":"1.5","show_marker":true}"#;

/// Golden A's page-102 file and the file with W20's overrides replaced.
fn page_with(overrides: &str) -> (String, String) {
    let base = String::from_utf8(golden()[PAGE].clone()).expect("UTF-8");
    assert_eq!(base.matches(W20_OVERRIDES).count(), 6);
    let w20 = base
        .lines()
        .find(|l| l.contains("connections|W20"))
        .expect("W20")
        .to_owned();
    let changed = base.replace(&w20, &w20.replace(W20_OVERRIDES, overrides));
    (base, changed)
}

fn merge_page(o: &str, a: &str, b: &str) -> (String, usize) {
    let m = merge_file(
        PAGE,
        MergeSide::current(o.as_bytes()),
        MergeSide::current(a.as_bytes()),
        MergeSide::current(b.as_bytes()),
        &Limits::default(),
        &MIGRATIONS,
    );
    (String::from_utf8(m.bytes).expect("UTF-8"), m.conflicts)
}

/// The page file loaded alone: its records, kept lines and findings.
fn load_page(bytes: &str) -> ew_format::Loaded {
    let files = BTreeMap::from([(FilePath::new(PAGE).expect("a path"), bytes.as_bytes().to_vec())]);
    load(None, &files, &Limits::default())
}

#[test]
fn ac41_w20_merges_cleanly_to_csa_2_5_and_colour_bu() {
    let (base, ours) = page_with(r#""overrides":{"csa":"1.5","colour":["BU"],"show_marker":true}"#);
    let (_, theirs) = page_with(r#""overrides":{"csa":"2.5","show_marker":true}"#);
    let (_, expected) = page_with(r#""overrides":{"csa":"2.5","colour":["BU"],"show_marker":true}"#);
    assert_eq!(merge_page(&base, &ours, &theirs), (expected.clone(), 0));
    assert_eq!(merge_page(&base, &theirs, &ours), (expected, 0));
}

#[test]
fn ac41_w20_conflict_variant_writes_one_conflict_record_and_keeps_bu() {
    let (base, ours) = page_with(r#""overrides":{"csa":"1.5","colour":["BU"],"show_marker":true}"#);
    let (_, theirs) = page_with(r#""overrides":{"csa":"2.5","colour":["WH"],"show_marker":true}"#);
    let (_, kept) = page_with(r#""overrides":{"csa":"2.5","colour":["BU"],"show_marker":true}"#);
    // D14's member order: `$k`, `id`, `kind`, `base`, `ours`, `theirs` (decision log ID-1273).
    let conflict = r#"{"$k":"conflict","id":"1c5963b7-6c34-53d6-8fa6-9d02d4629711","kind":"connection","base":{"overrides":{}},"ours":{"overrides":{"colour":["BU"]}},"theirs":{"overrides":{"colour":["WH"]}}}"#;
    let (merged, conflicts) = merge_page(&base, &ours, &theirs);
    assert_eq!(merged, format!("{kept}{conflict}\n"));
    assert_eq!(conflicts, 1);
    // The file opens editable with nothing reported: the conflict is a typed record now, written back as it was read
    // (WP-S1.17 part 5, decision log ID-1279).
    let loaded = load_page(&merged);
    assert_eq!(loaded.findings, []);
    assert!(!loaded.read_only);
    assert!(loaded.kept.is_empty());
    let conflicts: Vec<String> = loaded
        .records
        .iter()
        .filter(|r| matches!(r, Record::Conflict(_)))
        .map(|r| ew_format::record_line(r).expect("a line"))
        .collect();
    assert_eq!(conflicts, [conflict.to_owned()]);
    // MD-3: every merged record reads typed.
    for line in merged.lines().filter(|l| !l.starts_with(r#"{"$k":"conflict""#)) {
        assert!(typed(&serde_json::from_str(line).expect("JSON")), "{line}");
    }
}

/// The file-level conflict line of PAGE holding `side`'s `text` (D14), members in D14's order (decision log ID-1273).
fn file_conflict(side: &str, text: &str) -> String {
    let lines: Vec<&str> = text.strip_suffix('\n').unwrap_or(text).split('\n').collect();
    format!(
        r#"{{"$k":"conflict","id":"{PAGE_CONFLICT}","kind":"file","path":"{PAGE}","side":"{side}","lines":{}}}"#,
        serde_json::to_string(&lines).expect("JSON")
    )
}

#[test]
fn md4_a_side_that_does_not_parse_keeps_the_other_with_one_file_level_conflict() {
    let (base, ours) = page_with(r#""overrides":{"csa":"1.5","colour":["BU"],"show_marker":true}"#);
    let broken = format!("{base}{{\"$k\":\"connection\",\r\n");
    assert_eq!(
        merge_page(&base, &ours, &broken),
        (format!("{ours}{}\n", file_conflict("theirs", &broken)), 1)
    );
    assert_eq!(
        merge_page(&base, &broken, &ours),
        (format!("{ours}{}\n", file_conflict("ours", &broken)), 1)
    );
    // The result opens: the file-level record is a typed conflict record (decision log ID-1279).
    let (merged, _) = merge_page(&base, &ours, &broken);
    let loaded = load_page(&merged);
    assert_eq!(loaded.findings, []);
    assert!(loaded.kept.is_empty());
    let held: Vec<String> = loaded
        .records
        .iter()
        .filter(|r| matches!(r, Record::Conflict(_)))
        .map(|r| ew_format::record_line(r).expect("a line"))
        .collect();
    assert_eq!(held, [file_conflict("theirs", &broken)]);
}

// ---- conflict markers, D18 (WP-S1.17 part 5, decision log ID-1279) ---------------------------------------------------

/// Golden A's page-102 file with W20's line replaced by a marker hunk: ours, the diff3 base when `base` is given,
/// theirs; each side `W20_OVERRIDES` replaced by its own.
fn page_with_hunk(ours: &str, base: Option<&str>, theirs: &str) -> String {
    let (page, _) = page_with(W20_OVERRIDES);
    let w20 = page
        .lines()
        .find(|l| l.contains("connections|W20"))
        .expect("W20")
        .to_owned();
    let side = |overrides: &str| w20.replace(W20_OVERRIDES, overrides);
    let mut hunk = format!("<<<<<<< ours\n{}\n", side(ours));
    if let Some(b) = base {
        hunk.push_str(&format!("||||||| base\n{}\n", side(b)));
    }
    hunk.push_str(&format!("=======\n{}\n>>>>>>> theirs", side(theirs)));
    page.replace(&w20, &hunk)
}

/// The record lines of the loaded page that are conflict records, and the line of W20.
fn conflicts_and_w20(loaded: &ew_format::Loaded) -> (Vec<String>, String) {
    let line = |r: &Record| ew_format::record_line(r).expect("a line");
    let conflicts = loaded
        .records
        .iter()
        .filter(|r| matches!(r, Record::Conflict(_)))
        .map(line)
        .collect();
    let w20 = loaded
        .records
        .iter()
        .map(line)
        .find(|l| l.contains("connections|W20"))
        .expect("W20");
    (conflicts, w20)
}

/// The finding of a file the reader rewrites at the next save.
fn rewritten() -> ew_format::Finding {
    ew_format::Finding::new(
        ew_format::FindingKind::NonCanonical,
        Some(FilePath::new(PAGE).expect("a path")),
        None,
        "not in canonical form; rewritten at the next save",
    )
}

/// CR-2: a diff3 hunk whose sides are whole record lines merges through `merge_record`: `csa` (theirs only) merges
/// cleanly, `colour` (both) keeps ours with one conflict record, the same as the merge driver's (MD-6); the file opens
/// editable and is rewritten canonically.
#[test]
fn cr2_diff3_markers_on_w20_merge_to_one_conflict_record_and_the_clean_parts() {
    let file = page_with_hunk(
        r#""overrides":{"csa":"1.5","colour":["BU"],"show_marker":true}"#,
        Some(W20_OVERRIDES),
        r#""overrides":{"csa":"2.5","colour":["WH"],"show_marker":true}"#,
    );
    let (_, kept) = page_with(r#""overrides":{"csa":"2.5","colour":["BU"],"show_marker":true}"#);
    let kept_w20 = kept.lines().find(|l| l.contains("connections|W20")).expect("W20");
    let loaded = load_page(&file);
    assert_eq!(loaded.findings, [rewritten()]);
    assert!(!loaded.read_only);
    assert!(loaded.kept.is_empty());
    assert!(loaded.rewrite.contains(&FilePath::new(PAGE).expect("a path")));
    let conflict = r#"{"$k":"conflict","id":"1c5963b7-6c34-53d6-8fa6-9d02d4629711","kind":"connection","base":{"overrides":{}},"ours":{"overrides":{"colour":["BU"]}},"theirs":{"overrides":{"colour":["WH"]}}}"#;
    assert_eq!(
        conflicts_and_w20(&loaded),
        (vec![conflict.to_owned()], kept_w20.to_owned())
    );
}

/// A record-level conflict whose target is not in the project is kept verbatim and reported (EW-CORE-007), the file
/// editable.
#[test]
fn a_conflict_record_without_its_target_is_kept_verbatim_and_reported() {
    let (page, _) = page_with(W20_OVERRIDES);
    let conflict = r#"{"$k":"conflict","id":"00000000-0000-7000-8000-00000000c0f1","kind":"connection","base":{},"ours":{"overrides":{"colour":["BU"]}},"theirs":{"overrides":{"colour":["WH"]}}}"#;
    let file = format!("{page}{conflict}\n");
    let loaded = load_page(&file);
    let line = u32::try_from(file.lines().count()).expect("a line number");
    assert_eq!(
        loaded.findings,
        [ew_format::Finding::new(
            ew_format::FindingKind::SchemaViolation,
            Some(FilePath::new(PAGE).expect("a path")),
            Some(line),
            "a conflict record whose target, a record of the kind \"connection\", is not in the project; kept verbatim",
        )]
    );
    assert!(!loaded.read_only);
    let kept: Vec<(u32, &[u8])> = loaded.kept.iter().map(|k| (k.line, k.text.as_slice())).collect();
    assert_eq!(kept, [(line, conflict.as_bytes())]);
    assert!(!loaded.records.iter().any(|r| matches!(r, Record::Conflict(_))));
}

/// CR-2: without a base the hunk takes D18's no-base rule: ours kept, one conflict holding each member that differs
/// whole, base `{}`.
#[test]
fn cr2_markers_without_a_base_keep_ours_with_a_conflict_of_every_differing_member() {
    let ours = r#""overrides":{"csa":"1.5","colour":["BU"],"show_marker":true}"#;
    let file = page_with_hunk(
        ours,
        None,
        r#""overrides":{"csa":"2.5","colour":["WH"],"show_marker":true}"#,
    );
    let (_, kept) = page_with(ours);
    let kept_w20 = kept.lines().find(|l| l.contains("connections|W20")).expect("W20");
    let loaded = load_page(&file);
    assert_eq!(loaded.findings, [rewritten()]);
    assert!(!loaded.read_only);
    let conflict = r#"{"$k":"conflict","id":"1c5963b7-6c34-53d6-8fa6-9d02d4629711","kind":"connection","base":{},"ours":{"overrides":{"colour":["BU"],"csa":"1.5","show_marker":true}},"theirs":{"overrides":{"colour":["WH"],"csa":"2.5","show_marker":true}}}"#;
    assert_eq!(
        conflicts_and_w20(&loaded),
        (vec![conflict.to_owned()], kept_w20.to_owned())
    );
}

/// CR-2: a side line that does not parse (torn) leaves the markers as they are: EW-CORE-006 on each marker line, the
/// lines read as before (the torn one EW-CORE-005, W20 of both sides EW-CORE-009), read-only.
#[test]
fn cr2_a_torn_line_in_a_hunk_keeps_the_file_ew_core_006_and_read_only() {
    let file = page_with_hunk(
        r#""overrides":{"csa":"1.5","colour":["BU"],"show_marker":true}"#,
        Some(W20_OVERRIDES),
        r#""overrides":{"csa":"2.5","colour":["WH"],"show_marker":true}"#,
    )
    .replace(">>>>>>> theirs", "{\"$k\":\"connection\",\n>>>>>>> theirs");
    let loaded = load_page(&file);
    assert!(loaded.read_only);
    let codes: Vec<Option<&str>> = loaded.findings.iter().map(ew_format::Finding::code).collect();
    assert_eq!(
        codes,
        [
            Some("EW-CORE-005"),
            Some("EW-CORE-006"),
            Some("EW-CORE-006"),
            Some("EW-CORE-006"),
            Some("EW-CORE-006"),
            Some("EW-CORE-009"),
            Some("EW-CORE-009"),
            None
        ]
    );
    assert!(!loaded.records.iter().any(|r| matches!(r, Record::Conflict(_))));
}

#[test]
fn md4_a_side_with_a_newer_record_or_format_keeps_the_other_side() {
    let (base, ours) = page_with(r#""overrides":{"csa":"1.5","colour":["BU"],"show_marker":true}"#);
    let (_, newer) = page_with(r#""$v":2,"overrides":{"csa":"2.5","show_marker":true}"#);
    assert_eq!(
        merge_page(&base, &ours, &newer),
        (format!("{ours}{}\n", file_conflict("theirs", &newer)), 1)
    );
    // A side of a format above the reader's.
    let (_, theirs) = page_with(r#""overrides":{"csa":"2.5","show_marker":true}"#);
    let m = merge_file(
        PAGE,
        MergeSide::current(base.as_bytes()),
        MergeSide::current(ours.as_bytes()),
        MergeSide {
            bytes: theirs.as_bytes(),
            format: "0.2",
        },
        &Limits::default(),
        &MIGRATIONS,
    );
    assert_eq!(FORMAT, "0.1", "the test's newer format is above the reader's");
    assert_eq!(
        (String::from_utf8(m.bytes).expect("UTF-8"), m.conflicts),
        (format!("{ours}{}\n", file_conflict("theirs", &theirs)), 1)
    );
}

#[test]
fn md4_a_side_above_a_reader_limit_keeps_the_other_side() {
    let (base, ours) = page_with(r#""overrides":{"csa":"1.5","colour":["BU"],"show_marker":true}"#);
    let deep = format!("{}1{}", "[".repeat(80), "]".repeat(80));
    let (_, theirs) = page_with(&format!(
        r#""overrides":{{"csa":"2.5","show_marker":true}},"x":{deep}"#
    ));
    assert_eq!(
        merge_page(&base, &ours, &theirs),
        (format!("{ours}{}\n", file_conflict("theirs", &theirs)), 1)
    );
    let limits = Limits {
        max_line_bytes: 64,
        ..Limits::default()
    };
    let m = merge_file(
        PAGE,
        MergeSide::current(base.as_bytes()),
        MergeSide::current(ours.as_bytes()),
        MergeSide::current(b"{\"$k\":\"page\"}\n"),
        &limits,
        &MIGRATIONS,
    );
    // Neither side reads (ours is above the line limit, theirs has no ID): ours stays as it is, and the file-level
    // record that would hold theirs is above the line limit too, so it is not written (decision log ID-1273).
    assert_eq!(
        (String::from_utf8(m.bytes).expect("UTF-8"), m.conflicts),
        (ours, 1)
    );
}

#[test]
fn neither_side_parsing_keeps_ours_and_holds_theirs() {
    let m = merge_file(
        PAGE,
        MergeSide::current(b""),
        MergeSide::current(b"<<<<<<< ours"),
        MergeSide::current(b"not json\n"),
        &Limits::default(),
        &MIGRATIONS,
    );
    assert_eq!(
        (String::from_utf8(m.bytes).expect("UTF-8"), m.conflicts),
        (
            format!("<<<<<<< ours\n{}\n", file_conflict("theirs", "not json\n")),
            1
        )
    );
}

/// A bond record's line with the ID `0190d8a4-6d3e-7c41-8f00-00000000000<n>` and `members`, as compact JSON.
fn bond(n: u8, members: Value) -> String {
    let mut out = json!({"$k": "bond", "id": format!("0190d8a4-6d3e-7c41-8f00-00000000000{n}")});
    if let (Value::Object(out), Value::Object(members)) = (&mut out, members) {
        out.extend(members);
    }
    out.to_string()
}

#[test]
fn a_base_that_does_not_read_follows_the_no_base_rule() {
    // A committed marker line makes the base unreadable (doc 05 §11.4). D18's no-base rule holds, never an empty base
    // (decision log ID-1273): a record equal on both sides is kept; one that differs keeps ours with a conflict of
    // every member that differs; one on one side only is kept with a conflict whose other side is deleted.
    let base = "<<<<<<< ours\n";
    let ours = [
        bond(1, json!({"number": 1})),
        bond(2, json!({"number": 1, "page": "p"})),
    ];
    let theirs = [
        bond(1, json!({"number": 1})),
        bond(2, json!({"number": 2})),
        bond(3, json!({"number": 3})),
    ];
    let m = merge_file(
        "waivers.jsonl",
        MergeSide::current(base.as_bytes()),
        MergeSide::current(format!("{}\n", ours.join("\n")).as_bytes()),
        MergeSide::current(format!("{}\n", theirs.join("\n")).as_bytes()),
        &Limits::default(),
        &MIGRATIONS,
    );
    let conflict = |n: u8, ours: &str, theirs: &str| {
        format!(
            r#"{{"$k":"conflict","id":"0190d8a4-6d3e-7c41-8f00-00000000000{n}","kind":"bond","base":{{}},"ours":{ours},"theirs":{theirs}}}"#
        )
    };
    let expected = [
        bond(1, json!({"number": 1})),
        bond(2, json!({"number": 1, "page": "p"})),
        bond(3, json!({"number": 3})),
        conflict(2, r#"{"number":1,"page":"p"}"#, r#"{"number":2}"#),
        conflict(3, r#""deleted""#, r#"{"number":3}"#),
    ];
    assert_eq!(
        (String::from_utf8(m.bytes).expect("UTF-8"), m.conflicts),
        (format!("{}\n", expected.join("\n")), 2)
    );

    // A library document: theirs deleted a member that ours holds; with no base to tell, ours is kept and `$conflict`
    // holds theirs' document.
    let path = "lib/manufacturers/221fd06e-3263-5cab-bbb9-27aee71c8d79.json";
    let mut a: Value = serde_json::from_slice(&golden()[path]).expect("JSON");
    a["aliases"] = json!(["Schneider"]);
    let mut b = a.clone();
    b.as_object_mut().expect("an object").remove("aliases");
    let doc = |v: &Value| ew_format::pretty(&v.to_string());
    let m = merge_file(
        path,
        MergeSide::current(base.as_bytes()),
        MergeSide::current(doc(&a).as_bytes()),
        MergeSide::current(doc(&b).as_bytes()),
        &Limits::default(),
        &MIGRATIONS,
    );
    assert_eq!(m.conflicts, 1);
    let mut merged: Value = serde_json::from_slice(&m.bytes).expect("JSON");
    let held = merged
        .as_object_mut()
        .and_then(|m| m.remove("$conflict"))
        .expect("$conflict");
    assert_eq!((merged, held), (a, b));
}

#[test]
fn a_library_document_neither_side_reads_keeps_ours_and_holds_theirs() {
    // D14's `$conflict` needs a readable ours: theirs goes in the file-level record, as for other files (decision log
    // ID-1273).
    let path = "lib/manufacturers/221fd06e-3263-5cab-bbb9-27aee71c8d79.json";
    let id = ew_format::ew_schema::conflict_file_id::<()>(path);
    let ours = "{\n  \"$k\": \"manufacturer\",\n<<<<<<< ours\n";
    let merge = |ours: &str| {
        merge_file(
            path,
            MergeSide::current(b"{}\n"),
            MergeSide::current(ours.as_bytes()),
            MergeSide::current(b"not json\n"),
            &Limits::default(),
            &MIGRATIONS,
        )
    };
    let m = merge(ours);
    assert_eq!(
        (String::from_utf8(m.bytes).expect("UTF-8"), m.conflicts),
        (
            format!(
                r#"{ours}{{"$k":"conflict","id":"{id}","kind":"file","path":"{path}","side":"theirs","lines":["not json"]}}
"#
            ),
            1
        )
    );
    // An ours that opens (JSON, but no document this reader merges) stays as it is: a line after it would make it a
    // syntax error.
    let m = merge("[]\n");
    assert_eq!((m.bytes, m.conflicts), (b"[]\n".to_vec(), 1));
}

#[test]
fn a_record_unchanged_on_its_side_is_written_as_that_side_s_own_line() {
    // FR-COL-010, doc 05 §10.3 (decision log ID-1273): J5.feed's line with `page` before `a` (out of schema order),
    // untouched on every side, and a record of an unknown kind with its members out of byte order are written as
    // ours' lines; W11, changed on theirs only (a member of a newer minor, first), as theirs' line.
    let (base, ours) = page_with(r#""overrides":{"csa":"1.5","colour":["BU"],"show_marker":true}"#);
    let (_, theirs) = page_with(r#""overrides":{"csa":"2.5","show_marker":true}"#);
    let (_, expected) = page_with(r#""overrides":{"csa":"2.5","colour":["BU"],"show_marker":true}"#);
    let line = |key: &str| {
        base.lines()
            .find(|l| l.contains(key))
            .unwrap_or_else(|| panic!("{key}"))
            .to_owned()
    };
    let (j5, w11) = (line("connections|J5.feed"), line("connections|W11\""));
    let page = r#","page":"4aa43308-d0d3-57ee-be0c-d9383f429f50""#;
    let at = j5.find(r#","a":"#).expect("a");
    let j5_moved = format!("{}{page}{}", &j5[..at], j5[at..].replacen(page, "", 1));
    let w11_newer = w11.replacen(r#","a":"#, r#","zz_newer":1,"a":"#, 1);
    let unknown = r#"{"$k":"frobnicator","id":"0190d8a4-6d3e-7c41-8f00-000000000001","z":1,"a":2}"#;
    let side = |text: &str, w11: &str| {
        format!(
            "{}{unknown}\n",
            text.replace(&j5, &j5_moved)
                .replace(&line("connections|W11\""), w11)
        )
    };
    let m = merge_page(&side(&base, &w11), &side(&ours, &w11), &side(&theirs, &w11_newer));
    assert_eq!(m, (side(&expected, &w11_newer), 0));
}

#[test]
fn a_merge_that_would_pass_a_reader_limit_keeps_ours_as_it_is() {
    // D14's loadable file over its "nothing lost" (decision log ID-1273): the result is ours as it is, with 1 conflict.
    let merge = |path: &str, sides: [&str; 3], limits: &Limits| {
        let m = merge_file(
            path,
            MergeSide::current(sides[0].as_bytes()),
            MergeSide::current(sides[1].as_bytes()),
            MergeSide::current(sides[2].as_bytes()),
            limits,
            &MIGRATIONS,
        );
        (String::from_utf8(m.bytes).expect("UTF-8"), m.conflicts)
    };
    // Theirs, a 100-byte line that does not parse, under a 64-byte line limit: the file-level record holding it would
    // be one line above the limit.
    let lines = Limits {
        max_line_bytes: 64,
        ..Limits::default()
    };
    let ours = format!("{}\n", bond(1, json!({})));
    let theirs = format!("{}\n", "x".repeat(100));
    assert_eq!(
        merge("waivers.jsonl", ["", &ours, &theirs], &lines),
        (ours.clone(), 1)
    );
    // A record 4 deep under a depth limit of 4, changed at its leaf on both sides: its conflict record nests the leaf
    // one level deeper.
    let depth = Limits {
        max_depth: 4,
        ..Limits::default()
    };
    let leaf = |n: u8| bond(1, json!({"x": {"y": {"z": [n]}}}));
    let [o, a, b] = [1, 2, 3].map(|n| format!("{}\n", leaf(n)));
    assert_eq!(merge("waivers.jsonl", [&o, &a, &b], &depth), (a.clone(), 1));
    // A library document 4 deep: `$conflict` holds theirs' document one level below ours'.
    let path = "lib/manufacturers/221fd06e-3263-5cab-bbb9-27aee71c8d79.json";
    let doc = |n: u8| {
        ew_format::pretty(
            &json!({"$k": "manufacturer", "id": "221fd06e-3263-5cab-bbb9-27aee71c8d79", "x": {"y": {"z": [n]}}})
                .to_string(),
        )
    };
    let [o, a, b] = [1, 2, 3].map(doc);
    assert_eq!(merge(path, [&o, &a, &b], &depth), (a.clone(), 1));
}

fn same(value: Value) -> Result<Value, MigrationError> {
    Ok(value)
}

/// The test chain's record step from "0.0": the member `nr` became `number`.
fn rename_nr(mut value: Value) -> Result<Value, MigrationError> {
    if let Value::Object(members) = &mut value {
        if let Some(nr) = members.remove("nr") {
            members.insert("number".to_owned(), nr);
        }
    }
    Ok(value)
}

static TEST_CHAIN: Migrations = Migrations {
    kinds: &[],
    formats: &[FormatStep {
        from: "0.0",
        to: "0.1",
        manifest: same,
        record: rename_nr,
    }],
};

#[test]
fn an_older_side_is_migrated_before_the_merge() {
    let o = format!("{}\n", rec("bond", json!({"number": 1})));
    let a = format!("{}\n", rec("bond", json!({"number": 1, "page": "p"})));
    let b = format!("{}\n", rec("bond", json!({"nr": 2})));
    let older = MergeSide {
        bytes: b.as_bytes(),
        format: "0.0",
    };
    let path = "waivers.jsonl";
    let m = merge_file(
        path,
        MergeSide::current(o.as_bytes()),
        MergeSide::current(a.as_bytes()),
        older,
        &Limits::default(),
        &TEST_CHAIN,
    );
    assert_eq!(
        (String::from_utf8(m.bytes).expect("UTF-8"), m.conflicts),
        (format!("{}\n", rec("bond", json!({"number": 2, "page": "p"}))), 0)
    );
    // The shipped chain has no step from "0.0": that side keeps the other one.
    let m = merge_file(
        path,
        MergeSide::current(o.as_bytes()),
        MergeSide::current(a.as_bytes()),
        older,
        &Limits::default(),
        &MIGRATIONS,
    );
    assert_eq!(m.conflicts, 1);
}

// ---- library documents ------------------------------------------------------------------------------------------

#[test]
fn a_library_document_merges_as_one_record_and_a_conflict_holds_theirs_document() {
    let path = "lib/manufacturers/221fd06e-3263-5cab-bbb9-27aee71c8d79.json";
    let base: Value = serde_json::from_slice(&golden()[path]).expect("JSON");
    let side = |aliases: Value, name: &str| {
        let mut v = base.clone();
        v["aliases"] = aliases;
        v["name"] = json!(name);
        v
    };
    let o = side(json!(["Schneider"]), "Siemens");
    let a = side(json!(["Schneider", "SE"]), "Siemens");
    let b = side(json!(["Telemecanique", "Schneider"]), "Siemens AG");
    let doc = |v: &Value| ew_format::pretty(&v.to_string());
    let merge = |a: &Value, b: &Value| {
        merge_file(
            path,
            MergeSide::current(doc(&o).as_bytes()),
            MergeSide::current(doc(a).as_bytes()),
            MergeSide::current(doc(b).as_bytes()),
            &Limits::default(),
            &MIGRATIONS,
        )
    };
    let m = merge(&a, &b);
    let expected = side(json!(["Schneider", "SE", "Telemecanique"]), "Siemens AG");
    let canonical = |v: &Value| {
        ew_format::lib_document_text(&LibDocument::from_value(v.clone()).expect("typed")).expect("text")
    };
    assert_eq!(
        (String::from_utf8(m.bytes).expect("UTF-8"), m.conflicts),
        (canonical(&expected), 0)
    );
    // Both change the name: ours stays, `$conflict` holds theirs' document.
    let c = side(json!(["Schneider", "SE"]), "Siemens Energy");
    let m = merge(&c, &b);
    assert_eq!(m.conflicts, 1);
    let merged: Value = serde_json::from_slice(&m.bytes).expect("JSON");
    let mut without = merged.clone();
    let held = without
        .as_object_mut()
        .and_then(|m| m.remove("$conflict"))
        .expect("$conflict");
    assert_eq!(held, b);
    assert_eq!(
        without,
        side(json!(["Schneider", "SE", "Telemecanique"]), "Siemens Energy")
    );
    assert!(LibDocument::load(merged).is_ok());
}

// ---- manifest.json (D16) -----------------------------------------------------------------------------------------

const PROJECT: &str = "d76f5765-c456-5393-935c-4cec32a87b01";

fn hash(n: u8) -> String {
    format!("sha256:{}", format!("{n:02x}").repeat(32))
}

fn manifest(generation: u64, files: &[(&str, u8)], extra: &str) -> String {
    let files: Vec<String> = files
        .iter()
        .map(|(p, h)| format!("    \"{p}\": \"{}\"", hash(*h)))
        .collect();
    format!(
        "{{\n  \"format\": \"0.1\",\n  \"min_reader\": \"0.1\",\n  \"generator\": \"ElektraWatt 0.0.{generation}\",\n  \"project\": \"{PROJECT}\",\n  \"generation\": {generation},\n  \"files\": {{\n{}\n  }}{extra}\n}}\n",
        files.join(",\n")
    )
}

fn merge_manifests(o: &str, a: &str, b: &str) -> (String, usize) {
    let m = merge_file(
        MANIFEST,
        MergeSide::current(o.as_bytes()),
        MergeSide::current(a.as_bytes()),
        MergeSide::current(b.as_bytes()),
        &Limits::default(),
        &MIGRATIONS,
    );
    (String::from_utf8(m.bytes).expect("UTF-8"), m.conflicts)
}

#[test]
fn the_manifest_merges_by_max_plus_one_with_pending_where_both_changed() {
    let base_member = |n: u8| format!(",\n  \"base\": \"{}\"", hash(n));
    let o = manifest(
        3,
        &[("a.jsonl", 1), ("b.jsonl", 2), ("c.jsonl", 3), ("d.jsonl", 4)],
        "",
    );
    let a = manifest(
        5,
        &[
            ("a.jsonl", 11),
            ("b.jsonl", 2),
            ("c.jsonl", 3),
            ("d.jsonl", 4),
            ("e.jsonl", 5),
        ],
        &base_member(9),
    );
    let b = manifest(
        4,
        &[("a.jsonl", 12), ("b.jsonl", 22), ("c.jsonl", 3)],
        &format!("{},\n  \"newer\": 1", base_member(8)),
    );
    let expected = format!(
        "{{\n  \"format\": \"0.1\",\n  \"min_reader\": \"0.1\",\n  \"generator\": \"ElektraWatt 0.0.5\",\n  \"project\": \"{PROJECT}\",\n  \"generation\": 6,\n  \"files\": {{\n    \"a.jsonl\": \"sha256:pending\",\n    \"b.jsonl\": \"{}\",\n    \"c.jsonl\": \"{}\",\n    \"e.jsonl\": \"{}\"\n  }},\n  \"merged\": true,\n  \"newer\": 1\n}}\n",
        hash(22),
        hash(3),
        hash(5)
    );
    assert_eq!(merge_manifests(&o, &a, &b), (expected, 0));
    // The result reads as a merged manifest.
    let merged =
        Manifest::parse(merge_manifests(&o, &a, &b).0.as_bytes(), &Limits::default()).expect("readable");
    assert!(merged.merged);
    assert_eq!(merged.extra.get("base"), None);
}

/// FT-1235 (WP-S6.08 D4): a side that names a format newer than this reader's, theirs, ours or the ancestor, is not
/// merged: ours as it is, not clean, so the merge never takes a format this reader cannot read.
#[test]
fn a_manifest_side_of_a_newer_format_keeps_ours_as_it_is() {
    let newer = |m: String| m.replace("\"0.1\"", "\"0.2\"");
    let o = manifest(1, &[("a.jsonl", 1)], "");
    let a = manifest(2, &[("a.jsonl", 2)], "");
    let b = manifest(3, &[("a.jsonl", 1)], "");
    assert_eq!(merge_manifests(&o, &a, &newer(b.clone())), (a.clone(), 1));
    assert_eq!(merge_manifests(&newer(o.clone()), &a, &b), (a.clone(), 1));
    assert_eq!(merge_manifests(&o, &newer(a.clone()), &b), (newer(a), 1));
}

#[test]
fn a_manifest_side_that_does_not_read_keeps_the_other_and_is_not_clean() {
    let o = manifest(1, &[("a.jsonl", 1)], "");
    let a = manifest(2, &[("a.jsonl", 2)], ",\n  \"base\": \"x\"");
    let b = "{\n<<<<<<< ours\n}\n";
    let (merged, conflicts) = merge_manifests(&o, &a, b);
    assert_eq!(conflicts, 1);
    assert_eq!(
        merged,
        format!(
            "{{\n  \"format\": \"0.1\",\n  \"min_reader\": \"0.1\",\n  \"generator\": \"ElektraWatt 0.0.2\",\n  \"project\": \"{PROJECT}\",\n  \"generation\": 3,\n  \"files\": {{\n    \"a.jsonl\": \"{}\"\n  }},\n  \"merged\": true\n}}\n",
            hash(2)
        )
    );
}

#[test]
fn a_file_of_another_class_keeps_ours_when_both_sides_changed_it() {
    let path = "assets/logo.png";
    let m = merge_file(
        path,
        MergeSide::current(b"o"),
        MergeSide::current(b"a"),
        MergeSide::current(b"b"),
        &Limits::default(),
        &MIGRATIONS,
    );
    assert_eq!((m.bytes, m.conflicts), (b"a".to_vec(), 1));
    let m = merge_file(
        path,
        MergeSide::current(b"o"),
        MergeSide::current(b"o"),
        MergeSide::current(b"b"),
        &Limits::default(),
        &MIGRATIONS,
    );
    assert_eq!((m.bytes, m.conflicts), (b"b".to_vec(), 0));
}

// ---- cycle 50's FT batch (decision log ID-1312) ------------------------------------------------------------------

/// `merge_file` on bytes: the merged bytes and the conflicts.
fn merge_bytes(path: &str, sides: [&[u8]; 3], limits: &Limits) -> (Vec<u8>, usize) {
    let m = merge_file(
        path,
        MergeSide::current(sides[0]),
        MergeSide::current(sides[1]),
        MergeSide::current(sides[2]),
        limits,
        &MIGRATIONS,
    );
    (m.bytes, m.conflicts)
}

#[test]
fn a_side_that_is_not_utf8_is_a_binary_conflict_that_keeps_the_side_that_reads() {
    // FT-1234: never held lossily; the side that reads is kept and the other in git's index, as for a binary file
    // (doc 05 §11.2: B when A is the failing side; C50-p4#3).
    let ours = format!("{}\n", bond(1, json!({"number": 1})));
    let not_utf8 = b"{\"$k\":\"bond\",\"name\":\"\xff\"}\n".as_slice();
    let base = format!("{}\n", bond(1, json!({})));
    let limits = Limits::default();
    assert_eq!(
        merge_bytes(
            "waivers.jsonl",
            [base.as_bytes(), ours.as_bytes(), not_utf8],
            &limits
        ),
        (ours.clone().into_bytes(), 1)
    );
    assert_eq!(
        merge_bytes(
            "waivers.jsonl",
            [base.as_bytes(), not_utf8, ours.as_bytes()],
            &limits
        ),
        (ours.clone().into_bytes(), 1)
    );
    // Neither reads: ours as it is.
    assert_eq!(
        merge_bytes("waivers.jsonl", [base.as_bytes(), not_utf8, b"x\n"], &limits),
        (not_utf8.to_vec(), 1)
    );
    let path = "lib/manufacturers/221fd06e-3263-5cab-bbb9-27aee71c8d79.json";
    let doc = golden()[path].clone();
    assert_eq!(
        merge_bytes(path, [b"{}\n", &doc, not_utf8], &limits),
        (doc.clone(), 1)
    );
    assert_eq!(
        merge_bytes(path, [b"{}\n", not_utf8, &doc], &limits),
        (doc.clone(), 1)
    );
}

/// C50-p4#3: ours does not read and theirs holds a `$conflict`: theirs is kept, ours' text held at the end of its
/// `$conflict`'s chain.
#[test]
fn an_unreadable_ours_is_held_below_theirs_conflict() {
    let path = "lib/manufacturers/221fd06e-3263-5cab-bbb9-27aee71c8d79.json";
    let base: Value = serde_json::from_slice(&golden()[path]).expect("JSON");
    let mut b = base.clone();
    b["name"] = json!("Siemens AG");
    let mut earlier = base.clone();
    earlier["name"] = json!("Siemens Old");
    b["$conflict"] = earlier.clone();
    let doc = |v: &Value| ew_format::pretty(&v.to_string());
    let ours = "not json\n";
    let (bytes, conflicts) = merge_bytes(
        path,
        [doc(&base).as_bytes(), ours.as_bytes(), doc(&b).as_bytes()],
        &Limits::default(),
    );
    assert_eq!(conflicts, 1);
    let merged: Value = serde_json::from_slice(&bytes).expect("JSON");
    let mut expected = b.clone();
    earlier["$conflict"] = json!(ours);
    expected["$conflict"] = earlier;
    assert_eq!(merged, expected);
}

#[test]
fn a_second_conflict_on_a_target_joins_its_conflict_record() {
    // FT-1233: one conflict record per target (D14). Ours and theirs hold the record of an earlier conflict on
    // `number`; both now change `page`: one record holds both, never a second line with the same ID.
    let earlier = r#"{"$k":"conflict","id":"0190d8a4-6d3e-7c41-8f00-000000000001","kind":"bond","base":{"number":0},"ours":{"number":1},"theirs":{"number":2}}"#;
    let side = |page: &str| format!("{}\n{earlier}\n", bond(1, json!({"number": 1, "page": page})));
    let (o, a, b) = (side("p"), side("q"), side("r"));
    let joined = r#"{"$k":"conflict","id":"0190d8a4-6d3e-7c41-8f00-000000000001","kind":"bond","base":{"number":0,"page":"p"},"ours":{"number":1,"page":"q"},"theirs":{"number":2,"page":"r"}}"#;
    let (bytes, conflicts) = merge_bytes(
        "waivers.jsonl",
        [o.as_bytes(), a.as_bytes(), b.as_bytes()],
        &Limits::default(),
    );
    assert_eq!(
        (String::from_utf8(bytes).expect("UTF-8"), conflicts),
        (
            format!("{}\n{joined}\n", bond(1, json!({"number": 1, "page": "q"}))),
            1
        )
    );
    // The sides added different conflict records of one target, on different members: one record holds both.
    let conflict = |ours: &str, theirs: &str| {
        format!(
            r#"{{"$k":"conflict","id":"0190d8a4-6d3e-7c41-8f00-000000000001","kind":"bond","base":{{}},"ours":{ours},"theirs":{theirs}}}"#
        )
    };
    let o = format!("{}\n", bond(1, json!({})));
    let a = format!("{o}{}\n", conflict(r#"{"number":1}"#, r#"{"number":2}"#));
    let b = format!("{o}{}\n", conflict(r#"{"page":"q"}"#, r#"{"page":"r"}"#));
    let (bytes, conflicts) = merge_bytes(
        "waivers.jsonl",
        [o.as_bytes(), a.as_bytes(), b.as_bytes()],
        &Limits::default(),
    );
    assert_eq!(
        (String::from_utf8(bytes).expect("UTF-8"), conflicts),
        (
            format!(
                "{o}{}\n",
                conflict(r#"{"number":1,"page":"q"}"#, r#"{"number":2,"page":"r"}"#)
            ),
            1
        )
    );
    // On one member with different values: ours' record stays as it is and the file is conflicted, so that git keeps
    // theirs in its index; nothing pending is dropped (D14; decision log ID-1312, C50-p4#1).
    let b = format!("{o}{}\n", conflict(r#"{"number":1}"#, r#"{"number":3}"#));
    let (bytes, conflicts) = merge_bytes(
        "waivers.jsonl",
        [o.as_bytes(), a.as_bytes(), b.as_bytes()],
        &Limits::default(),
    );
    assert_eq!(
        (String::from_utf8(bytes).expect("UTF-8"), conflicts),
        (a.clone(), 1)
    );
}

/// C50-p4#1: ours holds an unresolved conflict on `number` (1 against 2); a later merge brings theirs' `number` to 3.
/// The pending 2 is not replaced: ours' record stays as it is, W's ours value with it, and the file is conflicted.
#[test]
fn a_new_conflict_on_a_member_already_in_conflict_keeps_the_pending_one() {
    let pending = r#"{"$k":"conflict","id":"0190d8a4-6d3e-7c41-8f00-000000000001","kind":"bond","base":{"number":0},"ours":{"number":1},"theirs":{"number":2}}"#;
    let o = format!("{}\n", bond(1, json!({"number": 0})));
    let a = format!("{}\n{pending}\n", bond(1, json!({"number": 1})));
    let b = format!("{}\n", bond(1, json!({"number": 3})));
    let (bytes, conflicts) = merge_bytes(
        "waivers.jsonl",
        [o.as_bytes(), a.as_bytes(), b.as_bytes()],
        &Limits::default(),
    );
    assert_eq!((String::from_utf8(bytes).expect("UTF-8"), conflicts), (a, 1));
}

/// C50-p4#2: one side resolved and deleted the conflict record C of X, the other changed C; X conflicts in the same
/// merge. C's own delete/modify keeps the changed C and raises no conflict of kind `conflict`, and X's new conflict
/// joins it: one record of kind `bond`.
#[test]
fn a_conflict_record_deleted_on_one_side_keeps_the_changed_one_and_x_s_conflict() {
    let record = |theirs: &str| {
        format!(
            r#"{{"$k":"conflict","id":"0190d8a4-6d3e-7c41-8f00-000000000001","kind":"bond","base":{{"page":"p"}},"ours":{{"page":"q"}},"theirs":{{"page":"{theirs}"}}}}"#
        )
    };
    let o = format!("{}\n{}\n", bond(1, json!({"number": 1})), record("r"));
    let a = format!("{}\n{}\n", bond(1, json!({"number": 5})), record("s"));
    let b = format!("{}\n", bond(1, json!({"number": 2})));
    let joined = r#"{"$k":"conflict","id":"0190d8a4-6d3e-7c41-8f00-000000000001","kind":"bond","base":{"number":1,"page":"p"},"ours":{"number":5,"page":"q"},"theirs":{"number":2,"page":"s"}}"#;
    let (bytes, conflicts) = merge_bytes(
        "waivers.jsonl",
        [o.as_bytes(), a.as_bytes(), b.as_bytes()],
        &Limits::default(),
    );
    assert_eq!(
        (String::from_utf8(bytes).expect("UTF-8"), conflicts),
        (format!("{}\n{joined}\n", bond(1, json!({"number": 5}))), 1)
    );
}

#[test]
fn a_library_document_conflict_keeps_the_earlier_conflict_below_theirs() {
    // FT-1233: ours holds the `$conflict` of an earlier merge; both now change the name. `$conflict` holds theirs'
    // document, and the earlier conflict at the end of its chain.
    let path = "lib/manufacturers/221fd06e-3263-5cab-bbb9-27aee71c8d79.json";
    let base: Value = serde_json::from_slice(&golden()[path]).expect("JSON");
    let named = |name: &str| {
        let mut v = base.clone();
        v["name"] = json!(name);
        v
    };
    let earlier = named("Siemens Old");
    let mut a = named("Siemens Energy");
    a["$conflict"] = earlier.clone();
    let b = named("Siemens AG");
    let doc = |v: &Value| ew_format::pretty(&v.to_string());
    let (bytes, conflicts) = merge_bytes(
        path,
        [doc(&base).as_bytes(), doc(&a).as_bytes(), doc(&b).as_bytes()],
        &Limits::default(),
    );
    assert_eq!(conflicts, 1);
    let merged: Value = serde_json::from_slice(&bytes).expect("JSON");
    let mut held = b.clone();
    held["$conflict"] = earlier;
    let mut expected = named("Siemens Energy");
    expected["$conflict"] = held;
    assert_eq!(merged, expected);
    assert!(LibDocument::load(merged).is_ok());
}

#[test]
fn a_manifest_base_that_does_not_read_makes_a_path_on_one_side_only_a_conflict() {
    // FT-1245: markers a web merge committed (doc 05 §11.4) cannot tell theirs' delete from ours' add.
    let o = "{\n<<<<<<< ours\n}\n";
    let a = manifest(2, &[("a.jsonl", 1), ("b.jsonl", 2)], "");
    let b = manifest(3, &[("a.jsonl", 3)], "");
    let (merged, conflicts) = merge_manifests(o, &a, &b);
    let merged = Manifest::parse(merged.as_bytes(), &Limits::default()).expect("readable");
    let files: Vec<&str> = merged.files.keys().map(|p| p.as_str()).collect();
    assert_eq!((files, conflicts), (vec!["a.jsonl", "b.jsonl"], 1));
    // Both sides list the same paths: nothing is on one side only.
    let b = manifest(3, &[("a.jsonl", 3), ("b.jsonl", 2)], "");
    assert_eq!(merge_manifests(o, &a, &b).1, 0);
}

#[test]
fn a_merge_that_would_pass_a_file_limit_keeps_ours_as_it_is() {
    // FT-1246: the record file's size and line count, and the manifest's size and file count.
    let o = format!("{}\n", bond(1, json!({})));
    let a = format!("{o}{}\n", bond(2, json!({})));
    let b = format!("{o}{}\n", bond(3, json!({})));
    let sides = [o.as_bytes(), a.as_bytes(), b.as_bytes()];
    let lines = Limits {
        max_lines_per_file: 2,
        ..Limits::default()
    };
    assert_eq!(
        merge_bytes("waivers.jsonl", sides, &lines),
        (a.clone().into_bytes(), 1)
    );
    let size = Limits {
        max_file_bytes: a.len() as u64,
        ..Limits::default()
    };
    assert_eq!(
        merge_bytes("waivers.jsonl", sides, &size),
        (a.clone().into_bytes(), 1)
    );
    assert_eq!(merge_bytes("waivers.jsonl", sides, &Limits::default()).1, 0);

    let o = manifest(1, &[("a.jsonl", 1)], "");
    let a = manifest(2, &[("a.jsonl", 1), ("b.jsonl", 2)], "");
    let b = manifest(3, &[("a.jsonl", 1), ("c.jsonl", 3)], "");
    let sides = [o.as_bytes(), a.as_bytes(), b.as_bytes()];
    let files = Limits {
        max_files: 2,
        ..Limits::default()
    };
    assert_eq!(merge_bytes(MANIFEST, sides, &files), (a.clone().into_bytes(), 1));
    let size = Limits {
        max_manifest_bytes: b.len() as u64,
        ..Limits::default()
    };
    assert_eq!(merge_bytes(MANIFEST, sides, &size), (a.clone().into_bytes(), 1));
    assert_eq!(merge_bytes(MANIFEST, sides, &Limits::default()).1, 0);
}
