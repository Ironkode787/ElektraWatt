// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Revisions (FT-165, doc 03 §18): `revision.create`, `fields.set` on the revision kind and `edit.delete`
//! of a revision; released and withdrawn revisions are immutable (FR-REV-009).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    CreateRevision, DeleteSelection, KindName, NewRevisionState, RevisionPageNote, SetFields, TargetRef,
};
use ew_commands::{CmdError, Engine};
use ew_ids::Id;
use ew_mltext::MlText;
use ew_model::{AnyRow, Model, Op, Rev};
use ew_schema::{Date, PartyRole, Revision, RevisionState};
use serde_json::{Value, json};

fn create(label: &str, state: NewRevisionState) -> CreateRevision {
    CreateRevision {
        label: ident(label),
        state,
        date: None,
        author: None,
        checker: None,
        description: MlText::default(),
        page_notes: Vec::new(),
    }
}

fn note(page: Id<ew_schema::Page>) -> RevisionPageNote {
    RevisionPageNote {
        page,
        description: MlText::default(),
    }
}

fn revision(e: &Engine, id: Id<Revision>) -> std::sync::Arc<Revision> {
    e.model().table::<Revision>().get(&id).expect("revision").clone()
}

fn set(targets: &[Id<Revision>], path: &str, value: Value) -> SetFields {
    SetFields {
        kind: KindName::Revision,
        targets: targets.iter().map(|t| t.cast()).collect(),
        path: path.into(),
        value,
    }
}

fn delete(id: Id<Revision>) -> DeleteSelection {
    DeleteSelection {
        targets: vec![TargetRef {
            kind: KindName::Revision,
            id: id.cast(),
        }],
    }
}

/// `(command, path)` of an `InvalidParams` error.
fn invalid_at(err: CmdError) -> (String, String) {
    match err {
        CmdError::InvalidParams { command, path, .. } => (command, path),
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

fn rejected(err: &CmdError) -> bool {
    matches!(err, CmdError::Rejected { .. })
}

/// Golden A's history through commands: V1.0 historic released, V2.0 open with two page notes.
fn with_history() -> (Engine, Id<Revision>, Id<Revision>) {
    let mut e = engine(golden_model());
    let v1 = CreateRevision {
        date: Some(Date::new(2025, 3, 3).unwrap()),
        description: MlText::neutral("Initial issue"),
        ..create("V1.0", NewRevisionState::Released)
    };
    let v1 = typed(e.execute(&v1).expect("V1.0").outcome.primary.expect("primary"));
    let v2 = CreateRevision {
        date: Some(Date::new(2025, 4, 14).unwrap()),
        author: Some(PartyRole::Engineer),
        description: MlText::neutral("Changes on 101 and 106"),
        page_notes: vec![note(PAGE_101), note(PAGE_102)],
        ..create("V2.0", NewRevisionState::Open)
    };
    let v2 = typed(e.execute(&v2).expect("V2.0").outcome.primary.expect("primary"));
    (e, v1, v2)
}

// ---------------------------------------------------------------------------------------------------------
// revision.create

#[test]
fn create_writes_an_open_and_a_historic_released_revision() {
    let (e, v1, v2) = with_history();
    let r1 = revision(&e, v1);
    assert_eq!(r1.state, RevisionState::Released);
    assert_eq!(r1.label.as_str(), "V1.0");
    assert!(r1.extra.is_empty() && r1.source_ref.is_none() && r1.page_notes.is_empty());
    let r2 = revision(&e, v2);
    assert!(r2.is_open());
    assert_eq!(r2.author, Some(PartyRole::Engineer));
    assert_eq!(
        r2.page_notes.iter().map(|n| n.page).collect::<Vec<_>>(),
        [PAGE_101, PAGE_102]
    );
    assert_eq!(e.model().table::<Revision>().len(), 2);
}

/// The default state is `open`; one undo removes the revision again, redo restores it byte for byte.
#[test]
fn create_is_undoable() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let id: Id<Revision> = typed(
        e.execute(&serde_json::from_value::<CreateRevision>(json!({"label": "V1.0"})).unwrap())
            .expect("created")
            .outcome
            .primary
            .expect("primary"),
    );
    assert!(revision(&e, id).is_open());
    let after = lines(e.model());
    e.undo().expect("undo");
    assert_eq!(lines(e.model()), before);
    e.redo().expect("redo");
    assert_eq!(lines(e.model()), after);
}

#[test]
fn create_refuses_a_second_open_revision() {
    let (mut e, _, _) = with_history();
    let err = e.execute(&create("V3.0", NewRevisionState::Open)).unwrap_err();
    assert!(rejected(&err), "{err:?}");
    // A further historic released revision is fine.
    assert!(e.execute(&create("V0.9", NewRevisionState::Released)).is_ok());
}

#[test]
fn create_refuses_a_label_in_use() {
    let (mut e, _, _) = with_history();
    let err = e
        .execute(&create("V1.0", NewRevisionState::Released))
        .unwrap_err();
    assert!(rejected(&err), "{err:?}");
}

#[test]
fn create_refuses_author_as_checker() {
    let mut e = engine(golden_model());
    let cmd = CreateRevision {
        author: Some(PartyRole::Engineer),
        checker: Some(PartyRole::Engineer),
        ..create("V1.0", NewRevisionState::Open)
    };
    assert_eq!(
        invalid_at(e.execute(&cmd).unwrap_err()),
        ("revision.create".into(), "/checker".into())
    );
    let ok = CreateRevision {
        checker: Some(PartyRole::Checker),
        ..cmd
    };
    assert!(e.execute(&ok).is_ok());
}

#[test]
fn create_refuses_a_missing_page_or_a_page_noted_twice() {
    let mut e = engine(golden_model());
    let missing = CreateRevision {
        page_notes: vec![note(PAGE_101), note(Id::from_uuid(uuid(0xdead)))],
        ..create("V1.0", NewRevisionState::Open)
    };
    assert_eq!(
        invalid_at(e.execute(&missing).unwrap_err()),
        ("revision.create".into(), "/page_notes/1/page".into())
    );
    let twice = CreateRevision {
        page_notes: vec![note(PAGE_101), note(PAGE_101)],
        ..create("V1.0", NewRevisionState::Open)
    };
    assert_eq!(
        invalid_at(e.execute(&twice).unwrap_err()),
        ("revision.create".into(), "/page_notes/1".into())
    );
}

/// The parameters take no `withdrawn` state and no unknown member.
#[test]
fn create_parameters_are_closed() {
    let registry = ew_commands::CommandRegistry::builtin();
    for params in [
        json!({"label": "V1.0", "state": "withdrawn"}),
        json!({"label": "V1.0", "release": {}}),
        json!({"state": "open"}),
    ] {
        let cmd = ew_commands::AnyCommand {
            name: "revision.create".into(),
            params,
        };
        assert!(
            matches!(registry.resolve(&cmd), Err(CmdError::InvalidParams { .. })),
            "accepted {:?}",
            cmd.params
        );
    }
}

// ---------------------------------------------------------------------------------------------------------
// fields.set on the revision kind

#[test]
fn the_open_revision_is_editable() {
    let (mut e, _, v2) = with_history();
    e.execute(&set(&[v2], "checker", json!("checker")))
        .expect("checker");
    e.execute(&set(&[v2], "label", json!("V2.1"))).expect("label");
    e.execute(&set(&[v2], "date", Value::Null)).expect("date reset");
    e.execute(&set(
        &[v2],
        "page_notes",
        json!([{"page": PAGE_102.to_string(), "description": "moved"}]),
    ))
    .expect("notes");
    let r = revision(&e, v2);
    assert_eq!(r.checker, Some(PartyRole::Checker));
    assert_eq!(r.label.as_str(), "V2.1");
    assert!(r.date.is_none());
    assert_eq!(r.page_notes.len(), 1);
    assert!(!r.page_notes[0].description.is_empty());
}

/// A released revision is immutable (FR-REV-009): refused at its target, nothing changes.
#[test]
fn a_released_revision_is_not_editable() {
    let (mut e, v1, v2) = with_history();
    let before = lines(e.model());
    let err = e
        .execute(&set(&[v2, v1], "description", json!("rewritten")))
        .unwrap_err();
    assert_eq!(invalid_at(err), ("fields.set".into(), "/targets/1".into()));
    assert_eq!(lines(e.model()), before);
}

#[test]
fn the_state_is_not_settable_and_invariants_hold() {
    let (mut e, _, v2) = with_history();
    let (_, path) = invalid_at(e.execute(&set(&[v2], "state", json!("released"))).unwrap_err());
    assert_eq!(path, "/path");
    // EW-INV-017 and EW-INV-018 hold for fields.set too.
    let label = e.execute(&set(&[v2], "label", json!("V1.0"))).unwrap_err();
    assert!(
        matches!(&label, CmdError::Model(ew_model::ModelError::Invariant { id, .. }) if *id == ew_model::REVISION_LABEL_UNIQUE),
        "{label:?}"
    );
    let eyes = e.execute(&set(&[v2], "checker", json!("engineer"))).unwrap_err();
    assert!(
        matches!(&eyes, CmdError::Model(ew_model::ModelError::Invariant { id, .. }) if *id == ew_model::REVISION_FOUR_EYES),
        "{eyes:?}"
    );
    let missing = e
        .execute(&set(
            &[v2],
            "page_notes",
            json!([{"page": uuid(0xdead).to_string()}]),
        ))
        .unwrap_err();
    assert_eq!(invalid_at(missing), ("fields.set".into(), "/value/0/page".into()));
}

// ---------------------------------------------------------------------------------------------------------
// edit.delete of a revision

#[test]
fn the_open_revision_can_be_deleted_a_released_one_cannot() {
    let (mut e, v1, v2) = with_history();
    let err = e.execute(&delete(v1)).unwrap_err();
    assert_eq!(invalid_at(err), ("edit.delete".into(), "/targets/0".into()));
    e.execute(&delete(v2)).expect("the open revision goes");
    assert!(e.model().table::<Revision>().get(&v2).is_none());
    e.undo().expect("undo");
    assert!(revision(&e, v2).is_open());
}

/// Deleting a page leaves a revision's note on it: a note is history, not a link.
#[test]
fn deleting_a_noted_page_keeps_the_note() {
    let (mut e, _, v2) = with_history();
    let cmd = DeleteSelection {
        targets: vec![TargetRef {
            kind: KindName::Page,
            id: PAGE_101.cast(),
        }],
    };
    e.execute(&cmd).expect("the page goes");
    assert!(e.model().table::<ew_schema::Page>().get(&PAGE_101).is_none());
    assert_eq!(revision(&e, v2).page_notes.len(), 2);
}

/// A withdrawal record (written by `revision.withdraw` later; here a raw record) is immutable too and may
/// repeat the label of the revision it withdraws.
#[test]
fn a_withdrawn_revision_is_not_editable() {
    let (e, v1, _) = with_history();
    let w = Revision {
        id: Id::from_uuid(uuid(0x7001)),
        state: RevisionState::Withdrawn,
        ..(*revision(&e, v1)).clone()
    };
    let wid = w.id;
    let model: Model = e
        .model()
        .apply(&[Op::insert(AnyRow::of(w))], Rev(e.model().rev().0 + 1))
        .expect("a withdrawal record");
    let mut e = engine(model);
    let err = e.execute(&set(&[wid], "description", json!("x"))).unwrap_err();
    assert_eq!(invalid_at(err), ("fields.set".into(), "/targets/0".into()));
    let err = e.execute(&delete(wid)).unwrap_err();
    assert_eq!(invalid_at(err), ("edit.delete".into(), "/targets/0".into()));
    assert_eq!(
        e.model().table::<Revision>().get(&wid).map(|r| r.state.clone()),
        Some(RevisionState::Withdrawn)
    );
}
