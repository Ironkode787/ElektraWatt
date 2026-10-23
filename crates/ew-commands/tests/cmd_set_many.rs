// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `fields.set_many` (WP-S2.05 part 2, acceptance check FR1): every rejection at its pointer, the equivalence with a
//! sequence of `fields.set` on the same records, one undo step, and the bulk cost above 500 targets.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    CreateRevision, KindName, NewRevisionState, SET_MANY_MAX_EDITS, SetFields, SetManyFields,
};
use ew_commands::{CmdError, Command, Cost};
use ew_ids::Id;
use ew_mltext::MlText;
use ew_schema::{AnyId, Device, Revision};
use serde_json::{Value, json};

fn set(kind: KindName, targets: &[AnyId], path: &str, value: Value) -> SetFields {
    SetFields {
        kind,
        targets: targets.to_vec(),
        path: path.into(),
        value,
    }
}

fn dev(d: Id<Device>) -> AnyId {
    d.cast()
}

fn many(edits: Vec<SetFields>) -> SetManyFields {
    SetManyFields { edits }
}

/// `(command, pointer)` of an `InvalidParams` error.
fn invalid_at(err: CmdError) -> (String, String) {
    match err {
        CmdError::InvalidParams { command, path, .. } => (command, path),
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

#[test]
fn one_value_per_target_in_one_transaction_and_one_undo_step() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let cmd = many(vec![
        set(
            KindName::Device,
            &[dev(DEV_101F1)],
            "function_text",
            json!("Spare"),
        ),
        set(
            KindName::Device,
            &[dev(DEV_102F2), dev(DEV_102G1)],
            "function_text",
            json!("Reserve"),
        ),
        set(KindName::Page, &[PAGE_101.cast()], "title", json!("Main")),
    ]);
    e.execute(&cmd).unwrap();
    let text = |d: Id<Device>| e.model().table::<Device>().get(&d).unwrap().function_text.clone();
    assert_eq!(text(DEV_101F1), MlText::neutral("Spare"));
    assert_eq!(text(DEV_102F2), MlText::neutral("Reserve"));
    assert_eq!(text(DEV_102G1), MlText::neutral("Reserve"));
    assert_eq!(e.history().entries().len(), 1, "one undo step");
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before, "undo restores every record");
}

/// FR1: the records after one `fields.set_many` equal those after the same entries as `fields.set` commands.
#[test]
fn equals_a_sequence_of_fields_set() {
    let edits = vec![
        set(KindName::Device, &[dev(DEV_101F1)], "function_text", json!("A")),
        set(KindName::Device, &[dev(DEV_102F2)], "tag.counter", json!(9)),
        set(
            KindName::Connection,
            &[CON_W22.cast()],
            "overrides.csa",
            json!("1.5"),
        ),
        set(KindName::Device, &[dev(DEV_101F1)], "process_tag", json!("P-1")),
        set(
            KindName::Device,
            &[dev(DEV_X1031)],
            "strip.label_mode",
            json!("designation"),
        ),
    ];
    let mut one = engine(golden_model());
    one.execute(&many(edits.clone())).unwrap();
    let mut seq = engine(golden_model());
    for e in &edits {
        seq.execute(e).unwrap();
    }
    assert_eq!(lines(one.model()), lines(seq.model()));
}

#[test]
fn no_entry_and_too_many_entries_are_refused_at_edits() {
    let mut e = engine(golden_model());
    let err = e.execute(&many(Vec::new())).unwrap_err();
    assert_eq!(invalid_at(err), ("fields.set_many".into(), "/edits".into()));
    let one = set(KindName::Device, &[dev(DEV_101F1)], "function_text", json!("x"));
    // 10,001 targets: a bulk command, so it runs as a preview.
    let err = e.preview(&many(vec![one; SET_MANY_MAX_EDITS + 1])).unwrap_err();
    assert_eq!(invalid_at(err), ("fields.set_many".into(), "/edits".into()));
    assert_eq!(e.model().rev().0, 0, "nothing committed");
}

#[test]
fn a_pair_named_twice_is_refused_at_the_later_entry() {
    let mut e = engine(golden_model());
    let err = e
        .execute(&many(vec![
            set(
                KindName::Device,
                &[dev(DEV_101F1), dev(DEV_102F2)],
                "function_text",
                json!("a"),
            ),
            set(KindName::Device, &[dev(DEV_102G1)], "process_tag", json!("P")),
            set(
                KindName::Device,
                &[dev(DEV_102G1), dev(DEV_102F2)],
                "function_text",
                json!("b"),
            ),
        ]))
        .unwrap_err();
    assert_eq!(
        invalid_at(err),
        ("fields.set_many".into(), "/edits/2/targets/1".into())
    );
    // The same target with another path is two pairs, and is accepted.
    e.execute(&many(vec![
        set(KindName::Device, &[dev(DEV_101F1)], "function_text", json!("a")),
        set(KindName::Device, &[dev(DEV_101F1)], "process_tag", json!("P")),
    ]))
    .unwrap();
}

/// Every rejection of `fields.set` comes back with its pointer below `/edits/<i>`.
#[test]
fn an_entry_rejected_as_fields_set_would_be_carries_the_prefixed_pointer() {
    let mut e = engine(golden_model());
    let ok = set(KindName::Device, &[dev(DEV_101F1)], "function_text", json!("x"));
    let cases: Vec<(SetFields, &str)> = vec![
        (
            set(KindName::Device, &[], "function_text", json!("x")),
            "/edits/1/targets",
        ),
        (
            set(
                KindName::Device,
                &[dev(DEV_102F2), dev(DEV_102F2)],
                "function_text",
                json!("x"),
            ),
            "/edits/1/targets/1",
        ),
        (
            set(KindName::Device, &[dev(DEV_102F2)], "parent", json!(null)),
            "/edits/1/path",
        ),
        (
            set(KindName::Junction, &[J5.cast()], "pos", json!(null)),
            "/edits/1/path",
        ),
        (
            set(KindName::Device, &[dev(DEV_102F2)], "tag.counter", json!("nine")),
            "/edits/1/value",
        ),
        (
            set(
                KindName::Device,
                &[dev(DEV_102F2)],
                "strip.label_mode",
                json!("designation"),
            ),
            "/edits/1/path",
        ),
    ];
    for (bad, pointer) in cases {
        let err = e.execute(&many(vec![ok.clone(), bad.clone()])).unwrap_err();
        assert_eq!(
            invalid_at(err),
            ("fields.set_many".into(), pointer.into()),
            "{bad:?}"
        );
    }
    // A missing record is `NotFound`, as for `fields.set`.
    let ghost: AnyId = Id::<Device>::from_uuid(uuid(0xdead)).cast();
    let err = e
        .execute(&many(vec![
            ok,
            set(KindName::Device, &[ghost], "function_text", json!("x")),
        ]))
        .unwrap_err();
    assert!(matches!(err, CmdError::NotFound { .. }), "{err:?}");
    assert_eq!(e.model().rev().0, 0, "nothing committed");
}

/// A released revision is immutable (FR-REV-009) in an entry too: refused at its target below `/edits/<i>`.
#[test]
fn a_closed_revision_in_an_entry_is_refused_at_its_target() {
    let mut e = engine(golden_model());
    let released = CreateRevision {
        label: ident("V1.0"),
        state: NewRevisionState::Released,
        date: Some(ew_schema::Date::new(2025, 3, 3).unwrap()),
        author: None,
        checker: None,
        description: MlText::neutral("Initial issue"),
        page_notes: Vec::new(),
    };
    let v1: Id<Revision> = typed(e.execute(&released).unwrap().outcome.primary.unwrap());
    let err = e
        .execute(&many(vec![
            set(KindName::Device, &[dev(DEV_101F1)], "function_text", json!("x")),
            set(
                KindName::Revision,
                &[v1.cast()],
                "description",
                json!("rewritten"),
            ),
        ]))
        .unwrap_err();
    assert_eq!(
        invalid_at(err),
        ("fields.set_many".into(), "/edits/1/targets/0".into())
    );
}

/// D2's tag-collision refusal applies to an entry as to `fields.set`: G1 takes F2's letter and counter on page 102.
#[test]
fn a_counter_collision_is_refused_as_fields_set_refuses_it() {
    let model = golden_model();
    let counter = model
        .table::<Device>()
        .get(&DEV_102F2)
        .unwrap()
        .tag
        .counter
        .unwrap_or(1);
    let edits = vec![
        set(KindName::Device, &[dev(DEV_102G1)], "tag.letter", json!("F")),
        set(KindName::Device, &[dev(DEV_102G1)], "tag.counter", json!(counter)),
    ];
    let mut seq = engine(golden_model());
    let oracle: Result<(), CmdError> = edits.iter().try_for_each(|c| seq.execute(c).map(|_| ()));
    let mut e = engine(model);
    let got = e.execute(&many(edits)).map(|_| ());
    match (&oracle, &got) {
        (Err(CmdError::Rejected { reason: a, .. }), Err(CmdError::Rejected { reason: b, .. })) => {
            assert_eq!(a, b);
            assert!(a.contains("next free counter"), "{a}");
        }
        (Ok(()), Ok(())) => {}
        _ => panic!("fields.set gave {oracle:?}, fields.set_many {got:?}"),
    }
    assert!(oracle.is_err(), "the fixture collides: {oracle:?}");
}

#[test]
fn the_cost_is_bulk_above_500_targets_in_total() {
    let model = golden_model();
    let targets = |n: u128, base: u128| -> Vec<AnyId> {
        (0..n)
            .map(|i| Id::<Device>::from_uuid(uuid(base + i)).cast())
            .collect()
    };
    let at = |n: u128| {
        many(vec![
            set(
                KindName::Device,
                &targets(n / 2, 0x1_0000),
                "function_text",
                json!("x"),
            ),
            set(
                KindName::Device,
                &targets(n - n / 2, 0x2_0000),
                "process_tag",
                json!("P"),
            ),
        ])
    };
    assert_eq!(Command::cost(&at(500), &model), Cost::Interactive);
    assert_eq!(Command::cost(&at(501), &model), Cost::Bulk);
}
