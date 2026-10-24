// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The undo stack (ADR-0008, ARCH §5.3): coalescing, contexts, bounds, the saved marker, the redo branch
//! and marker lines, through the engine and directly on `History`.

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::MoveReps;
use ew_commands::{
    AnyCommand, CmdError, Engine, Exposure, History, HistoryLimits, Origin, Submit, Transaction, TxContext,
};
use ew_model::Op;
use ew_model::Rev;
use ew_schema::Kind;

/// `rep.move` of `REP_102F2` by `dx` millimetres times ten.
fn mv(dx: i64) -> MoveReps {
    MoveReps {
        items: Vec::new(),
        steps: None,
        reps: vec![REP_102F2],
        delta: pt(dx * G, 0),
        page: None,
    }
}

fn drag(e: &mut Engine, dx: i64, key: &str) {
    e.execute_with(&mv(dx), Submit::default().coalescing(key))
        .expect("rep.move");
}

fn moves(e: &mut Engine, n: i64) {
    for i in 1..=n {
        e.execute(&mv(i)).expect("rep.move");
    }
}

#[test]
fn a_drag_coalesces_into_one_step() {
    let mut e = engine(golden_model());
    let original = lines(e.model());
    drag(&mut e, 1, "drag-1");
    drag(&mut e, 2, "drag-1");
    assert_eq!(e.history().entries().len(), 1);
    assert_eq!(e.history().entries()[0].merged(), 1);
    assert_ne!(lines(e.model()), original);
    e.undo(&Origin::Gui).expect("one undo");
    assert_eq!(lines(e.model()), original);
    assert!(!e.is_dirty());
    assert!(!e.history().can_undo());
    e.redo(&Origin::Gui).expect("redo");
    e.undo(&Origin::Gui).expect("undo again");
    assert_eq!(lines(e.model()), original);
}

#[test]
fn another_key_or_no_key_does_not_coalesce() {
    let mut e = engine(golden_model());
    drag(&mut e, 1, "drag-1");
    drag(&mut e, 2, "drag-2");
    assert_eq!(e.history().entries().len(), 2);
    assert!(e.history().entries().iter().all(|h| h.merged() == 0));

    let mut e = engine(golden_model());
    e.execute(&mv(1)).unwrap();
    e.execute(&mv(2)).unwrap();
    assert_eq!(e.history().entries().len(), 2);

    // A keyed step after an unkeyed one starts its own step, and the next keyed one joins it.
    let mut e = engine(golden_model());
    e.execute(&mv(1)).unwrap();
    drag(&mut e, 2, "drag-1");
    drag(&mut e, 3, "drag-1");
    assert_eq!(e.history().entries().len(), 2);
    assert_eq!(e.history().entries()[1].merged(), 1);
}

#[test]
fn a_coalesced_step_after_a_save_makes_the_model_dirty_again() {
    let mut e = engine(golden_model());
    drag(&mut e, 1, "drag-1");
    e.mark_saved();
    assert!(!e.is_dirty());
    drag(&mut e, 2, "drag-1");
    assert_eq!(e.history().entries().len(), 1);
    assert!(e.is_dirty());
    e.undo(&Origin::Gui).unwrap();
    assert!(
        e.is_dirty(),
        "the saved state was merged away; the original is not it"
    );
}

#[test]
fn undo_returns_the_context_of_the_first_submission() {
    let first = TxContext {
        page: Some(PAGE_102),
        selection: vec![(Kind::Representation, REP_102F2.cast())],
        ..TxContext::default()
    };
    let second = TxContext {
        page: Some(PAGE_101),
        ..TxContext::default()
    };
    let mut e = engine(golden_model());
    let submit = |context: &TxContext| Submit {
        context: context.clone(),
        ..Submit::default().coalescing("drag-1")
    };
    e.execute_with(&mv(1), submit(&first)).unwrap();
    e.execute_with(&mv(2), submit(&second)).unwrap();
    assert_eq!(e.history().entries().len(), 1);
    let undone = e.undo(&Origin::Gui).unwrap();
    assert_eq!(undone.context, first);
    assert_eq!(undone.label, "rep.move");
    let redone = e.redo(&Origin::Gui).unwrap();
    assert_eq!(redone.context, first);
}

#[test]
fn the_step_limit_drops_the_oldest_entries() {
    let limits = HistoryLimits {
        max_steps: 3,
        max_bytes: usize::MAX,
    };
    let mut e = engine_with_limits(golden_model(), limits);
    let original = lines(e.model());
    moves(&mut e, 2);
    let after_two = lines(e.model());
    e.execute(&mv(3)).unwrap();
    e.execute(&mv(4)).unwrap();
    e.execute(&mv(5)).unwrap();
    assert_eq!(e.history().entries().len(), 3);
    for _ in 0..3 {
        e.undo(&Origin::Gui).expect("three steps are kept");
    }
    assert!(matches!(e.undo(&Origin::Gui), Err(CmdError::History { .. })));
    assert_eq!(
        lines(e.model()),
        after_two,
        "the two oldest steps are gone for good"
    );
    assert_ne!(lines(e.model()), original);
}

#[test]
fn the_byte_limit_keeps_the_newest_entry() {
    let limits = HistoryLimits {
        max_steps: 500,
        max_bytes: 1,
    };
    let mut e = engine_with_limits(golden_model(), limits);
    moves(&mut e, 3);
    assert_eq!(e.history().entries().len(), 1);
    assert_eq!(e.history().cursor(), 1);
    e.undo(&Origin::Gui).expect("the newest entry is kept");
    assert!(matches!(e.undo(&Origin::Gui), Err(CmdError::History { .. })));
}

#[test]
fn a_trimmed_saved_state_leaves_the_model_dirty() {
    let limits = HistoryLimits {
        max_steps: 2,
        max_bytes: usize::MAX,
    };
    let mut e = engine_with_limits(golden_model(), limits);
    e.execute(&mv(1)).unwrap();
    e.mark_saved();
    assert!(!e.is_dirty());
    // Trimming the saved step's entry leaves its state as the floor: still reachable.
    e.execute(&mv(2)).unwrap();
    e.execute(&mv(3)).unwrap();
    assert_eq!(e.history().entries().len(), 2);
    e.undo(&Origin::Gui).unwrap();
    e.undo(&Origin::Gui).unwrap();
    assert!(!e.is_dirty(), "the floor is the saved state");
    e.redo(&Origin::Gui).unwrap();
    e.redo(&Origin::Gui).unwrap();
    // Trimming one more step makes the saved state unreachable.
    e.execute(&mv(4)).unwrap();
    assert_eq!(e.history().entries().len(), 2);
    while e.undo(&Origin::Gui).is_ok() {}
    assert!(e.is_dirty(), "the saved state is trimmed away");
    e.mark_saved();
    assert!(!e.is_dirty());
}

#[test]
fn the_initial_state_is_clean_until_trimmed() {
    let limits = HistoryLimits {
        max_steps: 2,
        max_bytes: usize::MAX,
    };
    let mut e = engine_with_limits(golden_model(), limits);
    assert!(!e.is_dirty());
    moves(&mut e, 2);
    while e.undo(&Origin::Gui).is_ok() {}
    assert!(!e.is_dirty());
    while e.redo(&Origin::Gui).is_ok() {}
    e.execute(&mv(3)).unwrap();
    while e.undo(&Origin::Gui).is_ok() {}
    assert!(e.is_dirty(), "the loaded state is no longer reachable");
}

#[test]
fn a_new_command_truncates_the_redo_branch() {
    let mut e = engine(golden_model());
    assert!(!e.history().can_undo());
    assert!(!e.history().can_redo());
    moves(&mut e, 3);
    assert!(e.history().can_undo());
    assert!(!e.history().can_redo());
    e.undo(&Origin::Gui).unwrap();
    e.undo(&Origin::Gui).unwrap();
    assert!(e.history().can_undo());
    assert!(e.history().can_redo());
    e.execute(&mv(9)).unwrap();
    assert_eq!(e.history().entries().len(), 2);
    assert!(!e.history().can_redo());
    assert!(matches!(e.redo(&Origin::Gui), Err(CmdError::History { .. })));
    assert!(e.history().can_undo());
    e.undo(&Origin::Gui).unwrap();
    e.undo(&Origin::Gui).unwrap();
    assert!(!e.history().can_undo());
    assert!(e.history().can_redo());
}

#[test]
fn a_failed_command_leaves_the_history_alone() {
    let mut e = engine(golden_model());
    moves(&mut e, 1);
    let bad = MoveReps {
        items: Vec::new(),
        steps: None,
        reps: vec![],
        delta: pt(0, 0),
        page: None,
    };
    assert!(e.execute(&bad).is_err());
    assert_eq!(e.history().entries().len(), 1);
}

// ---------------------------------------------------------------------------------------------------------
// FT-404: a coalesced step records the strictest exposure of its parts

#[test]
fn a_coalesced_step_takes_the_strictest_exposure_of_its_parts() {
    let checks = AnyCommand {
        name: "project.checks.set".into(),
        params: serde_json::json!({"release_scheme": "full"}),
    };
    let script = Origin::Script { name: "drag".into() };
    // An `Edit` drag, then a `HumanOnly` command under the same key: one step, `HumanOnly`.
    let mut e = engine(golden_model());
    drag(&mut e, 1, "k");
    assert_eq!(e.history().entries()[0].exposure(), Exposure::Edit);
    e.execute_with(&checks, Submit::default().coalescing("k"))
        .expect("checks");
    drag(&mut e, 2, "k");
    let entries = e.history().entries();
    assert_eq!((entries.len(), entries[0].merged()), (1, 2));
    assert_eq!(entries[0].exposure(), Exposure::HumanOnly);
    assert!(matches!(
        e.undo(&script),
        Err(CmdError::Exposure {
            exposure: Exposure::HumanOnly,
            ..
        })
    ));
    e.undo(&Origin::Gui).expect("the GUI undoes the merged step");
    assert!(matches!(e.redo(&script), Err(CmdError::Exposure { .. })));
    // The `HumanOnly` command first: the later `Edit` drags do not relax the step.
    let mut e = engine(golden_model());
    e.execute_with(&checks, Submit::default().coalescing("k"))
        .expect("checks");
    drag(&mut e, 1, "k");
    assert_eq!(e.history().entries().len(), 1);
    assert_eq!(e.history().entries()[0].exposure(), Exposure::HumanOnly);
    assert!(e.undo(&script).is_err());
    // An `Edit` step below is still the script's to undo once the GUI took the `HumanOnly` one back.
    let mut e = engine(golden_model());
    drag(&mut e, 1, "a");
    e.execute(&checks).expect("checks");
    assert!(e.undo(&script).is_err());
    e.undo(&Origin::Gui).expect("checks undone");
    e.undo(&script).expect("the drag is the script's to undo");
}

/// FT-404 ruling 3: the exposure check applies only to the step being undone or redone; a transparent marker is
/// passed over.
#[test]
fn a_transparent_human_only_marker_does_not_block_a_scripted_undo() {
    let mut e = engine(golden_model());
    drag(&mut e, 1, "a");
    let listed = format!("sha256:{}", "ab".repeat(32));
    let p = e
        .preview(&AnyCommand {
            name: "project.assets.purge".into(),
            params: serde_json::json!({ "assets": [listed] }),
        })
        .unwrap();
    e.commit(p).unwrap();
    let entries = e.history().entries();
    assert_eq!(entries.len(), 2);
    let marker = &entries[1];
    assert!(!marker.is_step());
    assert!(!marker.is_barrier());
    assert_eq!(marker.exposure(), Exposure::HumanOnly);
    assert_eq!(marker.op_count(), 0);
    assert_eq!(e.history().cursor(), 2);
    let script = Origin::Script { name: "s".into() };
    let restored = e.undo(&script).expect("the marker is passed over");
    assert_eq!(restored.label, "rep.move");
    assert_eq!(e.history().cursor(), 0);
    e.redo(&script).expect("redo");
    assert_eq!(e.history().cursor(), 2);
}

// Marker lines, directly on `History`

/// A real op list: the ops of a `rep.move` preview.
fn some_ops() -> Vec<Op> {
    let mut e = engine(golden_model());
    e.preview(&mv(1)).expect("preview").ops().to_vec()
}

fn tx(label: &str, ops: Vec<Op>, undoable: bool) -> Transaction {
    Transaction {
        ops,
        command: AnyCommand {
            name: label.into(),
            params: serde_json::json!({}),
        },
        origin: Origin::Gui,
        context: TxContext::default(),
        coalesce_key: None,
        undoable,
        exposure: Exposure::Edit,
        label: label.into(),
    }
}

#[test]
fn a_marker_without_ops_is_transparent() {
    let mut h = History::default();
    h.push(tx("step", some_ops(), true), Rev(1));
    h.push(tx("export", vec![], false), Rev(1));
    assert_eq!(h.entries().len(), 2);
    assert_eq!(h.cursor(), 2);
    assert_eq!(h.undo_index().unwrap(), 0, "undo passes the marker");
    h.undone(0);
    assert_eq!(h.cursor(), 0);
    assert_eq!(h.redo_index().unwrap(), 0);
    h.redone(0);
    assert_eq!(h.cursor(), 2, "redo passes the marker");
    assert!(h.can_undo());
    assert!(!h.can_redo());
}

#[test]
fn a_marker_without_ops_does_not_make_the_model_dirty() {
    let mut h = History::default();
    h.push(tx("export", vec![], false), Rev(0));
    assert!(!h.is_dirty());
    h.push(tx("step", some_ops(), true), Rev(1));
    assert!(h.is_dirty());
    h.mark_saved();
    assert!(!h.is_dirty());
    h.push(tx("export", vec![], false), Rev(1));
    assert!(!h.is_dirty());
    assert!(matches!(h.undo_index(), Ok(1)));
}

#[test]
fn a_marker_alone_has_nothing_to_undo() {
    let mut h = History::default();
    h.push(tx("export", vec![], false), Rev(0));
    assert!(matches!(h.undo_index(), Err(CmdError::History { .. })));
    assert!(!h.can_redo());
}

#[test]
fn a_marker_with_ops_is_a_barrier() {
    let mut h = History::default();
    h.push(tx("before", some_ops(), true), Rev(1));
    h.push(tx("checkpoint", some_ops(), false), Rev(2));
    h.push(tx("after", some_ops(), true), Rev(3));
    assert!(h.entries()[1].is_barrier());
    assert_eq!(h.undo_index().unwrap(), 2, "the step above the barrier undoes");
    h.undone(2);
    assert!(matches!(h.undo_index(), Err(CmdError::History { .. })));
    assert!(!h.can_undo());
    assert!(h.can_redo());
    assert_eq!(h.redo_index().unwrap(), 2);
    h.redone(2);
    assert_eq!(h.cursor(), 3);
    assert!(h.can_undo());
}

#[test]
fn a_marker_with_ops_changes_the_state() {
    let mut h = History::default();
    h.mark_saved();
    h.push(tx("checkpoint", some_ops(), false), Rev(1));
    assert!(h.is_dirty());
    h.mark_saved();
    assert!(!h.is_dirty());
}
