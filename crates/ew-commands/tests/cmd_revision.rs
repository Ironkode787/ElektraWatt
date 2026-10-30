// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Revisions (FT-165, doc 03 §18; WP-S1.18 part 1): `revision.create`, `revision.set_author`, `revision.withdraw`,
//! `fields.set` on the revision kind, `edit.delete` of a revision, the automatic open revision (normaliser
//! `revision_ensure_open`) and the open revision's notes on deleted pages; released and withdrawn revisions are
//! immutable (FR-REV-009).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    CreateRevision, DeletePages, DeleteSelection, KindName, LABEL_ORDER_RULE, NewRevisionState,
    RevisionPageNote, SetFields, SetRevisionAuthor, TargetRef, WithdrawRevision,
};
use ew_commands::{CmdError, Engine, Exposure, Origin, Submit};
use ew_ids::Id;
use ew_mltext::MlText;
use ew_model::{AnyRow, Model, Op, Rev};
use ew_schema::{Date, Extra, Kind, LabelFormat, PartyRole, Record, ReleaseRecord, Revision, RevisionState};
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
    e.undo(&ew_commands::Origin::Gui).expect("undo");
    assert_eq!(lines(e.model()), before);
    e.redo(&ew_commands::Origin::Gui).expect("redo");
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
        ..create("V1.0", NewRevisionState::Released)
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

/// The parameters take no `withdrawn` state and no unknown member, also inside a page note.
#[test]
fn create_parameters_are_closed() {
    let registry = ew_commands::CommandRegistry::builtin();
    for params in [
        json!({"label": "V1.0", "state": "withdrawn"}),
        json!({"label": "V1.0", "release": {}}),
        json!({"state": "open"}),
        json!({"label": "V1.0", "page_notes": [{"page": PAGE_101.to_string(), "cloud": true}]}),
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

/// Review ruling R4 (cycle 17), on the typed member since WP-S1.18 part 1 (FT-302): once a revision has a release
/// record (written by `revision.release` later), a historic released revision is refused; an open one is still
/// accepted.
#[test]
fn create_refuses_a_historic_release_after_a_release_record() {
    let (e, v1, _) = with_history();
    let mut released = (*revision(&e, v1)).clone();
    released.id = Id::from_uuid(uuid(0x7002));
    released.label = ident("V2.5");
    released.release = Some(ReleaseRecord {
        changed_pages: Some(Vec::new()),
        ..ReleaseRecord::default()
    });
    let v2 = e
        .model()
        .table::<Revision>()
        .iter()
        .find(|(_, r)| r.is_open())
        .map(|(_, r)| r.clone())
        .expect("V2.0");
    let model: Model = e
        .model()
        .apply_history(
            &[Op::insert(AnyRow::of(released)), Op::delete(AnyRow::Revision(v2))],
            Rev(e.model().rev().0 + 1),
        )
        .expect("a project released by ElektraWatt, nothing open");
    // Another seed: the IDs the first engine minted are taken.
    let mut e = engine_seeded(model, 0x7e57);
    let err = e
        .execute(&create("V0.9", NewRevisionState::Released))
        .unwrap_err();
    assert!(rejected(&err), "{err:?}");
    assert!(e.execute(&create("V3.0", NewRevisionState::Open)).is_ok());
}

// ---------------------------------------------------------------------------------------------------------
// fields.set on the revision kind

#[test]
fn the_open_revision_is_editable() {
    let (mut e, _, v2) = with_history();
    e.execute(&set(&[v2], "description", json!("Changes on 102")))
        .expect("description");
    e.execute(&set(&[v2], "label", json!("V2.1"))).expect("label");
    e.execute(&set(&[v2], "date", Value::Null)).expect("date reset");
    e.execute(&set(
        &[v2],
        "page_notes",
        json!([{"page": PAGE_102.to_string(), "description": "moved"}]),
    ))
    .expect("notes");
    let r = revision(&e, v2);
    assert_eq!(r.description, MlText::neutral("Changes on 102"));
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
    // EW-INV-017 holds for fields.set too.
    let label = e.execute(&set(&[v2], "label", json!("V1.0"))).unwrap_err();
    assert!(
        matches!(&label, CmdError::Model(ew_model::ModelError::Invariant { id, .. }) if *id == ew_model::REVISION_LABEL_UNIQUE),
        "{label:?}"
    );
    let missing = e
        .execute(&set(
            &[v2],
            "page_notes",
            json!([{"page": uuid(0xdead).to_string()}]),
        ))
        .unwrap_err();
    assert_eq!(invalid_at(missing), ("fields.set".into(), "/value/0/page".into()));
    // A note takes no unknown member: it would land in `PageNote.extra`.
    let unknown = e
        .execute(&set(
            &[v2],
            "page_notes",
            json!([{"page": PAGE_101.to_string(), "cloud": true}]),
        ))
        .unwrap_err();
    assert_eq!(invalid_at(unknown), ("fields.set".into(), "/value".into()));
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
    e.undo(&ew_commands::Origin::Gui).expect("undo");
    assert!(revision(&e, v2).is_open());
}

/// RL-6 (FT-303, WP-S1.18 D5): deleting a page drops the open revision's note on it in the same transaction, by
/// `edit.delete` and by `page.delete`; a released revision's note on it stays (history, not a link); undo restores
/// the note with the page.
#[test]
fn deleting_a_noted_page_drops_the_open_revision_note_and_keeps_a_released_one() {
    let mut e = engine(golden_model());
    let v1 = CreateRevision {
        page_notes: vec![note(PAGE_101)],
        ..create("V1.0", NewRevisionState::Released)
    };
    let v1: Id<Revision> = typed(e.execute(&v1).expect("V1.0").outcome.primary.expect("primary"));
    let v2 = CreateRevision {
        page_notes: vec![note(PAGE_101), note(PAGE_102)],
        ..create("V2.0", NewRevisionState::Open)
    };
    let v2: Id<Revision> = typed(e.execute(&v2).expect("V2.0").outcome.primary.expect("primary"));
    let released_before = lines(e.model())
        .into_iter()
        .find(|(k, _)| *k == (Kind::Revision, v1.cast()))
        .expect("V1.0 line");
    let noted = |e: &Engine, id| {
        revision(e, id)
            .page_notes
            .iter()
            .map(|n| n.page)
            .collect::<Vec<_>>()
    };
    let before = lines(e.model());
    let cmd = DeleteSelection {
        targets: vec![TargetRef {
            kind: KindName::Page,
            id: PAGE_101.cast(),
        }],
    };
    e.execute(&cmd).expect("the page goes");
    assert!(e.model().table::<ew_schema::Page>().get(&PAGE_101).is_none());
    assert_eq!(noted(&e, v2), [PAGE_102]);
    assert_eq!(noted(&e, v1), [PAGE_101]);
    assert!(
        lines(e.model()).contains(&released_before),
        "the released record is byte-identical"
    );
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(lines(e.model()), before);
    // page.delete is bulk: previewed, then committed.
    let preview = e
        .preview(&DeletePages {
            pages: vec![PAGE_102],
            delete_devices: false,
        })
        .expect("preview");
    e.commit(preview).expect("page.delete");
    assert_eq!(noted(&e, v2), [PAGE_101]);
    assert_eq!(noted(&e, v1), [PAGE_101]);
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
        .apply_history(&[Op::insert(AnyRow::of(w))], Rev(e.model().rev().0 + 1))
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

// ---------------------------------------------------------------------------------------------------------
// WP-S1.18 part 1: labels, the automatic open revision, the roles, withdrawal

/// A raw revision record (an older file, a merge, an import).
fn raw(id: u128, label: &str, state: RevisionState) -> Revision {
    Revision {
        id: Id::from_uuid(uuid(id)),
        v: 1,
        label: ident(label),
        date: None,
        author: None,
        checker: None,
        description: MlText::default(),
        state,
        page_notes: Vec::new(),
        withdraws: None,
        source_ref: None,
        reason: MlText::default(),
        release: None,
        sign_off: None,
        extra: Extra::new(),
    }
}

/// The mini golden model with a label format and revisions.
fn model_with(format: Option<LabelFormat>, revisions: Vec<Revision>) -> Model {
    let (mut project, mut records, _) = mini_golden_parts();
    project.settings.revision_label = format;
    records.extend(revisions.into_iter().map(Record::Revision));
    Model::from_records(project, records, library()).expect("a valid model")
}

/// A content edit: a device's function text.
fn device_edit(text: &str) -> SetFields {
    SetFields {
        kind: KindName::Device,
        targets: vec![DEV_102F2.cast()],
        path: "function_text".into(),
        value: json!(text),
    }
}

fn open_revision(e: &Engine) -> Option<std::sync::Arc<Revision>> {
    e.model()
        .table::<Revision>()
        .iter()
        .find(|(_, r)| r.is_open())
        .map(|(_, r)| r.clone())
}

/// `(rule, finding)` of a `Prevented` error.
fn prevented(err: CmdError) -> (String, String) {
    match err {
        CmdError::Prevented { rule, finding, .. } => (rule, finding),
        other => panic!("expected Prevented, got {other:?}"),
    }
}

fn withdraw(revision: Id<Revision>, reason: &str) -> WithdrawRevision {
    WithdrawRevision {
        revision,
        reason: MlText::neutral(reason),
    }
}

/// The scripted origins that `HumanOnly` commands refuse (ADR-0008 amendment 3).
fn scripted() -> [Origin; 3] {
    [
        Origin::CmdLog {
            file: "replay.ewcmd".into(),
        },
        Origin::Mcp {
            client: "agent".into(),
        },
        Origin::Cli {
            subcommand: "apply".into(),
            confirmed: false,
        },
    ]
}

/// RL-2 (EW-REL-004's prevent half, FR-REV-002): under `V{major}.{minor}` "2" is refused with the rule and the next
/// label; an open revision follows every label; a historic released revision precedes the open one.
#[test]
fn create_prevents_a_label_out_of_format_or_order() {
    let mut e = engine(golden_model());
    let (rule, finding) = prevented(e.execute(&create("2", NewRevisionState::Open)).unwrap_err());
    assert_eq!(rule, LABEL_ORDER_RULE);
    assert!(
        finding.contains("V{major}.{minor}") && finding.contains("the next label is V1.0"),
        "{finding}"
    );
    e.execute(&create("V2.0", NewRevisionState::Released))
        .expect("V2.0");
    let (rule, finding) = prevented(e.execute(&create("V1.5", NewRevisionState::Open)).unwrap_err());
    assert_eq!(rule, "EW-REL-004");
    assert!(
        finding.contains("does not follow V2.0") && finding.contains("the next label is V3.0"),
        "{finding}"
    );
    e.execute(&create("V3.0", NewRevisionState::Open))
        .expect("V3.0 follows");
    let (_, finding) = prevented(
        e.execute(&create("V3.5", NewRevisionState::Released))
            .unwrap_err(),
    );
    assert!(finding.contains("does not precede V3.0"), "{finding}");
    e.execute(&create("V0.9", NewRevisionState::Released))
        .expect("history before the open revision");
}

/// RL-2 in the other formats: `two_digit` proposes 03 after 02, `letter` J after H (I is skipped, so "I" is out of
/// the format); a format of a newer build is not judged.
#[test]
fn create_follows_the_project_label_format() {
    let mut e = engine(model_with(
        Some(LabelFormat::TwoDigit),
        vec![raw(0x7101, "02", RevisionState::Released)],
    ));
    let (_, finding) = prevented(e.execute(&create("V3.0", NewRevisionState::Open)).unwrap_err());
    assert!(finding.contains("the next label is 03"), "{finding}");
    e.execute(&create("03", NewRevisionState::Open)).expect("03");
    let mut e = engine(model_with(
        Some(LabelFormat::Letter),
        vec![raw(0x7101, "H", RevisionState::Released)],
    ));
    let (_, finding) = prevented(e.execute(&create("I", NewRevisionState::Open)).unwrap_err());
    assert!(finding.contains("the next label is J"), "{finding}");
    e.execute(&create("J", NewRevisionState::Open)).expect("J");
    let newer = LabelFormat::Unknown(json!("roman"));
    let mut e = engine(model_with(
        Some(newer),
        vec![raw(0x7101, "IV", RevisionState::Released)],
    ));
    e.execute(&create("2", NewRevisionState::Open))
        .expect("not judged");
}

/// RL-3 (D3): with V1.0 released and nothing open, a content edit opens V2.0 in the same transaction, with the ID
/// `v5(V1.0, "next")` and no author; one undo removes both, redo restores both; a further edit opens nothing more.
#[test]
fn a_content_edit_after_a_release_opens_the_next_revision() {
    let mut e = engine(golden_model());
    let v1: Id<Revision> = typed(
        e.execute(&create("V1.0", NewRevisionState::Released))
            .expect("V1.0")
            .outcome
            .primary
            .expect("primary"),
    );
    assert!(open_revision(&e).is_none(), "a revision edit is not content");
    let before = lines(e.model());
    e.execute(&device_edit("Feed")).expect("the edit");
    let open = open_revision(&e).expect("V2.0 opened");
    assert_eq!(open.label.as_str(), "V2.0");
    assert_eq!(open.id, ew_ids::v5::revision_next_id(v1));
    assert!(open.author.is_none() && open.checker.is_none() && open.page_notes.is_empty());
    let after = lines(e.model());
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(
        lines(e.model()),
        before,
        "one undo removes the edit and the revision"
    );
    e.redo(&Origin::Gui).expect("redo");
    assert_eq!(lines(e.model()), after);
    e.execute(&device_edit("Supply")).expect("a second edit");
    assert_eq!(e.model().table::<Revision>().len(), 2);
}

/// RL-3's other half: an edit of revisions only (here a withdrawal) opens nothing; comments and transmittals are not
/// content either (both reserved kinds without commands at MVP); without a released revision nothing opens; the label
/// follows the project's format.
#[test]
fn only_a_content_edit_after_a_release_opens_a_revision() {
    assert!(!ew_schema::is_content_kind(Kind::Comment) && !ew_schema::is_content_kind(Kind::Transmittal));
    assert!(ew_schema::is_content_kind(Kind::Device) && ew_schema::is_content_kind(Kind::Project));
    let mut e = engine(golden_model());
    e.execute(&device_edit("Feed")).expect("the edit");
    assert!(e.model().table::<Revision>().is_empty(), "nothing released");
    let v1: Id<Revision> = typed(
        e.execute(&create("V1.0", NewRevisionState::Released))
            .expect("V1.0")
            .outcome
            .primary
            .expect("primary"),
    );
    e.execute_with(&withdraw(v1, "Issued in error"), Submit::from(Origin::Gui))
        .expect("withdrawn");
    assert!(open_revision(&e).is_none(), "a withdrawal is not content");
    e.execute(&device_edit("Supply")).expect("the edit");
    assert_eq!(open_revision(&e).expect("opened").label.as_str(), "V2.0");
    let mut e = engine(model_with(
        Some(LabelFormat::TwoDigit),
        vec![raw(0x7101, "02", RevisionState::Released)],
    ));
    e.execute(&device_edit("Feed")).expect("the edit");
    assert_eq!(open_revision(&e).expect("opened").label.as_str(), "03");
}

/// D3's edge cases: a taken ID chains the formula deterministically; a label format of a newer build opens nothing
/// and leaves a notice.
#[test]
fn the_automatic_revision_chains_a_taken_id_and_skips_a_newer_format() {
    let v1 = raw(0x7101, "V1.0", RevisionState::Released);
    let taken = Revision {
        id: ew_ids::v5::revision_next_id(v1.id),
        ..raw(0, "V0.5", RevisionState::Released)
    };
    let chained = ew_ids::v5::revision_next_id(taken.id);
    let mut e = engine(model_with(None, vec![v1, taken]));
    e.execute(&device_edit("Feed")).expect("the edit");
    let open = open_revision(&e).expect("opened");
    assert_eq!((open.id, open.label.as_str()), (chained, "V2.0"));
    let mut e = engine(model_with(
        Some(LabelFormat::Unknown(json!("roman"))),
        vec![raw(0x7101, "IV", RevisionState::Released)],
    ));
    let outcome = e.execute(&device_edit("Feed")).expect("the edit").outcome;
    assert!(open_revision(&e).is_none());
    assert!(
        outcome
            .notices
            .iter()
            .any(|n| n.contains("no revision was opened")),
        "{:?}",
        outcome.notices
    );
}

/// RL-4 (D4, FT-402): `fields.set` refuses `author` and `checker` from every origin with D4's text; an open revision
/// takes no checker when created.
#[test]
fn the_roles_are_not_set_by_fields_set() {
    let (mut e, _, v2) = with_history();
    let before = lines(e.model());
    let confirmed = Origin::Cli {
        subcommand: "revision".into(),
        confirmed: true,
    };
    for origin in [Origin::Gui, confirmed] {
        for path in ["author", "checker"] {
            let err = e
                .execute_with(&set(&[v2], path, json!("checker")), Submit::from(origin.clone()))
                .unwrap_err();
            match err {
                CmdError::InvalidParams {
                    path: at, message, ..
                } => {
                    assert_eq!(at, "/path");
                    assert!(
                        message.contains("set by Revision ▸ Set author or by approval"),
                        "{message}"
                    );
                }
                other => panic!("{other:?}"),
            }
        }
    }
    assert_eq!(lines(e.model()), before);
    let mut e = engine(golden_model());
    let cmd = CreateRevision {
        checker: Some(PartyRole::Checker),
        ..create("V1.0", NewRevisionState::Open)
    };
    assert_eq!(
        invalid_at(e.execute(&cmd).unwrap_err()),
        ("revision.create".into(), "/checker".into())
    );
}

/// RL-4 (D4): `revision.set_author` sets the open revision's author from the GUI; it refuses a released revision,
/// a missing one and the checker's role; from a script origin it is refused with the exposure error.
#[test]
fn set_author_sets_the_open_revision_author_from_a_person_only() {
    let (mut e, v1, v2) = with_history();
    let cmd = SetRevisionAuthor {
        revision: v2,
        role: PartyRole::Author,
    };
    for origin in scripted() {
        let err = e.execute_with(&cmd, Submit::from(origin.clone())).unwrap_err();
        assert_eq!(
            err,
            CmdError::Exposure {
                command: "revision.set_author".into(),
                exposure: Exposure::HumanOnly,
                origin,
            }
        );
    }
    assert_eq!(revision(&e, v2).author, Some(PartyRole::Engineer));
    e.execute_with(&cmd, Submit::from(Origin::Gui)).expect("set");
    assert_eq!(revision(&e, v2).author, Some(PartyRole::Author));
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(revision(&e, v2).author, Some(PartyRole::Engineer));
    let released = SetRevisionAuthor {
        revision: v1,
        ..cmd.clone()
    };
    assert_eq!(
        invalid_at(e.execute(&released).unwrap_err()),
        ("revision.set_author".into(), "/revision".into())
    );
    let missing = SetRevisionAuthor {
        revision: Id::from_uuid(uuid(0xdead)),
        ..cmd
    };
    assert_eq!(
        invalid_at(e.execute(&missing).unwrap_err()),
        ("revision.set_author".into(), "/revision".into())
    );
    // An open revision with a checker (an older file): its role is not the author's.
    let open = Revision {
        checker: Some(PartyRole::Checker),
        ..raw(0x7101, "V1.0", RevisionState::Open)
    };
    let id = open.id;
    let mut e = engine(model_with(None, vec![open]));
    let eyes = SetRevisionAuthor {
        revision: id,
        role: PartyRole::Checker,
    };
    assert_eq!(
        invalid_at(e.execute(&eyes).unwrap_err()),
        ("revision.set_author".into(), "/role".into())
    );
}

/// RL-5 (D5): `revision.withdraw` adds the withdrawal record `v5(revision, "withdrawal")` with the label, `withdraws`
/// and the reason; the released record stays byte for byte; undo removes the record. Refused: an open revision, a
/// blank reason, a second withdrawal, a withdrawal record, a script origin.
#[test]
fn withdraw_adds_a_record_and_leaves_the_released_one() {
    let (mut e, v1, v2) = with_history();
    let line = |e: &Engine, id: Id<Revision>| {
        lines(e.model())
            .into_iter()
            .find(|(k, _)| *k == (Kind::Revision, id.cast()))
            .map(|(_, l)| l)
    };
    let released = line(&e, v1).expect("V1.0");
    let before = lines(e.model());
    for origin in scripted() {
        let err = e
            .execute_with(&withdraw(v1, "Issued in error"), Submit::from(origin))
            .unwrap_err();
        assert!(matches!(err, CmdError::Exposure { .. }), "{err:?}");
    }
    let outcome = e
        .execute_with(&withdraw(v1, "Issued in error"), Submit::from(Origin::Gui))
        .expect("withdrawn")
        .outcome;
    let wid: Id<Revision> = typed(outcome.primary.expect("primary"));
    assert_eq!(wid, ew_ids::v5::revision_withdrawal_id(v1));
    let w = revision(&e, wid);
    assert_eq!(w.state, RevisionState::Withdrawn);
    assert_eq!((w.label.as_str(), w.withdraws), ("V1.0", Some(v1)));
    assert_eq!(w.reason, MlText::neutral("Issued in error"));
    assert!(
        line(&e, wid)
            .expect("record")
            .contains(r#""reason":"Issued in error""#)
    );
    assert_eq!(line(&e, v1).as_deref(), Some(released.as_str()), "byte-identical");
    let err = e.execute(&withdraw(v1, "Again")).unwrap_err();
    assert!(rejected(&err), "{err:?}");
    assert_eq!(
        invalid_at(e.execute(&withdraw(wid, "Gone")).unwrap_err()),
        ("revision.withdraw".into(), "/revision".into())
    );
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(lines(e.model()), before);
    assert_eq!(
        invalid_at(e.execute(&withdraw(v2, "Open")).unwrap_err()),
        ("revision.withdraw".into(), "/revision".into())
    );
    assert_eq!(
        invalid_at(e.execute(&withdraw(v1, "  ")).unwrap_err()),
        ("revision.withdraw".into(), "/reason".into())
    );
}
