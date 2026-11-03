// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Conflict records in the editing layer (WP-S1.17 part 5, D19; decision log ID-1279): `merge.resolve` takes ours or
//! theirs in one undo step, and `edit.delete` removes a record's conflict in the same transaction (CR-4).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::{AnyCommand, CmdError, Engine, Origin};
use ew_model::{AnyKey, Model};
use ew_schema::{Conflict, Connection, Kind, Record};
use serde_json::{Value, json};

fn any(name: &str, params: Value) -> AnyCommand {
    AnyCommand {
        name: name.to_owned(),
        params,
    }
}

fn resolve(take: &str) -> AnyCommand {
    any(
        "merge.resolve",
        json!({"choices": [{"conflict": CON_W29.to_string(), "take": take}]}),
    )
}

fn record(v: Value) -> Record {
    Record::from_value(v.clone()).unwrap_or_else(|e| panic!("{e}: {v}"))
}

/// Sets the overrides of connection `id` among `records`.
fn set_overrides(records: &mut [Record], id: ew_ids::Id<Connection>, overrides: Value) {
    for r in records {
        if let Record::Connection(c) = r {
            if c.id == id {
                let mut v = serde_json::to_value(&*c).expect("a connection serialises");
                v["$k"] = json!("connection");
                v["overrides"] = overrides.clone();
                *r = record(v);
            }
        }
    }
}

/// The mini golden model with W29's colour BU and a conflict on it: `ours` and `theirs` as given.
fn model_with_conflict(ours: Value, theirs: Value) -> Model {
    let (p, mut records, _) = mini_golden_parts();
    set_overrides(&mut records, CON_W29, json!({"colour": ["BU"]}));
    records.push(record(json!({
        "$k": "conflict", "id": CON_W29.to_string(), "kind": "connection",
        "base": {}, "ours": ours, "theirs": theirs
    })));
    Model::from_records(p, records, library()).expect("the model is valid")
}

fn colour_conflict() -> Model {
    model_with_conflict(
        json!({"overrides": {"colour": ["BU"]}}),
        json!({"overrides": {"colour": ["WH"]}}),
    )
}

fn w29(e: &Engine) -> Option<Value> {
    e.model()
        .table::<Connection>()
        .get(&CON_W29)
        .map(|c| serde_json::to_value(&**c).expect("a connection serialises"))
}

fn conflicts(e: &Engine) -> usize {
    e.model().table::<Conflict>().iter().count()
}

/// Theirs writes the conflicting member into W29 and removes the conflict; one undo restores both.
#[test]
fn taking_theirs_writes_the_member_and_removes_the_conflict_in_one_undo_step() {
    let mut e = engine(colour_conflict());
    let before = lines(e.model());
    e.execute(&resolve("theirs")).expect("theirs is taken");
    assert_eq!(w29(&e).expect("W29")["overrides"], json!({"colour": ["WH"]}));
    assert_eq!(conflicts(&e), 0);
    e.undo(&Origin::Gui).expect("one undo step");
    assert_eq!(lines(e.model()), before);
}

/// Ours keeps W29 as it is and removes the conflict.
#[test]
fn taking_ours_keeps_the_record_and_removes_the_conflict() {
    let mut e = engine(colour_conflict());
    e.execute(&resolve("ours")).expect("ours is taken");
    assert_eq!(w29(&e).expect("W29")["overrides"], json!({"colour": ["BU"]}));
    assert_eq!(conflicts(&e), 0);
}

/// A delete/modify conflict: the side that deleted the record, taken, deletes it with its conflict; the side that
/// changed it keeps it.
#[test]
fn taking_the_deleting_side_deletes_the_record_with_its_conflict() {
    let deleted = || model_with_conflict(json!({"overrides": {"colour": ["BU"]}}), json!("deleted"));
    let mut e = engine(deleted());
    let before = lines(e.model());
    e.execute(&resolve("theirs")).expect("theirs is taken");
    assert_eq!(w29(&e), None);
    assert_eq!(conflicts(&e), 0);
    e.undo(&Origin::Gui).expect("one undo step");
    assert_eq!(lines(e.model()), before);

    let mut e = engine(deleted());
    e.execute(&resolve("ours")).expect("ours is taken");
    assert_eq!(w29(&e).expect("W29")["overrides"], json!({"colour": ["BU"]}));
    assert_eq!(conflicts(&e), 0);
}

/// CR-4: deleting the record removes its conflict in the same transaction; one undo restores both.
#[test]
fn cr4_deleting_a_record_removes_its_conflict_and_undo_restores_both() {
    let mut e = engine(colour_conflict());
    let before = lines(e.model());
    e.execute(&any(
        "edit.delete",
        json!({"targets": [{"kind": "connection", "id": CON_W29.to_string()}]}),
    ))
    .expect("the delete commits");
    assert_eq!(w29(&e), None);
    assert_eq!(conflicts(&e), 0);
    e.undo(&Origin::Gui).expect("one undo step");
    assert_eq!(lines(e.model()), before);
}

/// The refusals: no choice, a conflict that is not there, a conflict named twice; nothing changes.
#[test]
fn resolve_refuses_no_choice_an_unknown_conflict_and_a_repeated_one() {
    let mut e = engine(colour_conflict());
    let before = lines(e.model());
    let unknown = CON_W27.to_string();
    let w29 = CON_W29.to_string();
    let cases = [
        (
            json!({"choices": []}),
            "/choices",
            "no conflict chosen; name at least one",
        ),
        (
            json!({"choices": [{"conflict": unknown, "take": "ours"}]}),
            "/choices/0/conflict",
            "names no conflict record of the project",
        ),
        (
            json!({"choices": [{"conflict": w29, "take": "ours"}, {"conflict": w29, "take": "theirs"}]}),
            "/choices/1/conflict",
            "an earlier choice names this conflict already",
        ),
    ];
    for (params, path, message) in cases {
        assert_eq!(
            e.execute(&any("merge.resolve", params)).unwrap_err(),
            CmdError::invalid_params("merge.resolve", path, message)
        );
    }
    assert_eq!(lines(e.model()), before);
}

/// The lines of the records of page 102's file (doc 05 §2.2: the page, the records naming it, and the interruption
/// points without a device whose representation is on it), `edit` applied to each record's JSON (`None` leaves it
/// out).
fn page_102_lines(model: &Model, edit: impl Fn(AnyKey, Value) -> Option<Value>) -> Vec<String> {
    let page = json!(PAGE_102.to_string());
    let all: Vec<(AnyKey, Value)> = lines(model)
        .into_iter()
        .map(|(key, line)| (key, serde_json::from_str(&line).expect("JSON")))
        .collect();
    let on_page = |v: &Value| v.get("page") == Some(&page);
    let drawn: Vec<Value> = all
        .iter()
        .filter(|(key, v)| key.0 == Kind::Representation && on_page(v))
        .flat_map(|(_, v)| v["functions"].as_array().into_iter().flatten().cloned())
        .collect();
    all.into_iter()
        .filter_map(|(key, mut v)| {
            let ip_here = key.0 == Kind::Function && v.get("device").is_none() && drawn.contains(&v["id"]);
            if !(on_page(&v) || ip_here || key == (Kind::Page, PAGE_102.cast())) {
                return None;
            }
            v["$k"] = json!(key.0.name());
            edit(key, v)
        })
        .map(|v| v.to_string())
        .collect()
}

/// A file-level conflict: taking the side whose lines it holds replaces the records of the file with them (one
/// changed, one left out and so deleted); taking the other side removes only the conflict.
#[test]
fn a_file_level_conflict_taken_replaces_the_records_of_its_file() {
    let base = golden_model();
    let path = format!("pages/{PAGE_102}.jsonl");
    let held = page_102_lines(&base, |key, mut v| {
        if key == (Kind::Connection, CON_W29.cast()) {
            return None;
        }
        if key == (Kind::Connection, CON_W27.cast()) {
            v["overrides"] = json!({"colour": ["WH"]});
        }
        Some(v)
    });
    let conflict_id = "00000000-0000-7000-8000-00000000c0f1";
    let (p, mut records, _) = mini_golden_parts();
    records.push(record(json!({
        "$k": "conflict", "id": conflict_id, "kind": "file", "path": path, "side": "theirs", "lines": held
    })));
    let model = Model::from_records(p, records, library()).expect("the model is valid");
    let choose = |take: &str| {
        any(
            "merge.resolve",
            json!({"choices": [{"conflict": conflict_id, "take": take}]}),
        )
    };

    let mut e = engine(model.clone());
    e.execute(&choose("ours")).expect("ours is taken");
    assert_eq!(lines(e.model()), lines(&base));

    let mut e = engine(model);
    e.execute(&choose("theirs")).expect("theirs is taken");
    // Expected: W27 white, then W29 deleted through the delete plan.
    let (p, mut records, _) = mini_golden_parts();
    set_overrides(&mut records, CON_W27, json!({"colour": ["WH"]}));
    let mut expected = engine(Model::from_records(p, records, library()).expect("the model is valid"));
    expected
        .execute(&any(
            "edit.delete",
            json!({"targets": [{"kind": "connection", "id": CON_W29.to_string()}]}),
        ))
        .expect("W29 deletes");
    assert_eq!(lines(e.model()), lines(expected.model()));
}
