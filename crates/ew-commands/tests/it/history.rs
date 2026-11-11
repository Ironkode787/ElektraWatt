// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The undo stack (ADR-0008, ARCH §5.3): bounds, the saved marker, the redo branch
//! and marker lines, through the engine and directly on `History`.

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::commands::MoveReps;
use ew_commands::{AnyCommand, CmdError, Engine, Exposure, History, HistoryLimits, Origin, Transaction};
use ew_model::Op;
use ew_model::Rev;

/// `rep.move` of `REP_102F2` by `dx` millimetres times ten.
fn mv(dx: i64) -> MoveReps {
    MoveReps {
        routes: Vec::new(),
        connect: Vec::new(),
        items: Vec::new(),
        steps: None,
        reps: vec![REP_102F2],
        delta: pt(dx * G, 0),
        page: None,
    }
}

fn moves(e: &mut Engine, n: i64) {
    for i in 1..=n {
        e.execute(&mv(i)).expect("rep.move");
    }
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
        routes: Vec::new(),
        connect: Vec::new(),
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
// A step records the exposure of its command

#[test]
fn a_step_records_the_exposure_of_its_command() {
    let checks = AnyCommand {
        name: "project.checks.set".into(),
        params: serde_json::json!({"release_scheme": "full"}),
    };
    let script = Origin::Script { name: "drag".into() };
    // An `Edit` step below is still the script's to undo once the GUI took the `HumanOnly` one back.
    let mut e = engine(golden_model());
    e.execute(&mv(1)).unwrap();
    assert_eq!(e.history().entries()[0].exposure(), Exposure::Edit);
    e.execute(&checks).expect("checks");
    assert_eq!(e.history().entries()[1].exposure(), Exposure::HumanOnly);
    assert!(matches!(
        e.undo(&script),
        Err(CmdError::Exposure {
            exposure: Exposure::HumanOnly,
            ..
        })
    ));
    e.undo(&Origin::Gui).expect("checks undone");
    assert!(matches!(e.redo(&script), Err(CmdError::Exposure { .. })));
    e.undo(&script).expect("the move is the script's to undo");
}

/// The exposure check applies only to the step being undone or redone; a transparent marker is
/// passed over.
#[test]
fn a_transparent_human_only_marker_does_not_block_a_scripted_undo() {
    let mut e = engine(golden_model());
    e.execute(&mv(1)).unwrap();
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
