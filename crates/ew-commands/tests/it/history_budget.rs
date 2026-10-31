// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The memory bound and the compaction of the undo stack (F8, F15): the budget counts only the rows the
//! history alone keeps alive, estimated without serialising, and a coalesced step keeps one op per key.

// A wall-clock timer of this test, asserted only with EW_PERF_ASSERT (the timing rule of doc 18 §3.16); no timestamp reaches a compared result.
#![allow(clippy::disallowed_methods)]

use crate::common;

use std::collections::BTreeSet;
use std::time::Instant;

use common::fixture::*;
use common::*;
use ew_commands::commands::{KindName, MoveReps, SetFields};
use ew_commands::{Engine, HistoryLimits, Origin, Submit};
use ew_ids::Id;
use ew_model::AnyKey;
use ew_schema::Device;
use serde_json::json;

fn unlimited() -> HistoryLimits {
    HistoryLimits {
        max_steps: usize::MAX,
        max_bytes: usize::MAX,
    }
}

fn move_rep(dx: i64) -> MoveReps {
    MoveReps {
        items: Vec::new(),
        steps: None,
        reps: vec![REP_102F2],
        delta: pt(dx, 0),
        page: None,
    }
}

/// A text of `len` bytes whose content depends on `seed`, so that two texts differ.
fn text(len: usize, seed: u8) -> String {
    (0..len)
        .map(|i| char::from(b'a' + ((i as u8).wrapping_add(seed)) % 26))
        .collect()
}

fn set_text(devices: &[Id<Device>], text: &str) -> SetFields {
    SetFields {
        kind: KindName::Device,
        targets: devices.iter().map(|d| d.cast()).collect(),
        path: "function_text".into(),
        value: json!(text),
        copies: Vec::new(),
    }
}

#[test]
fn coalesced_moves_compact_to_one_op_per_key() {
    const N: usize = 2_000;
    // Alternating steps keep the representation near its start; both are multiples of `G`.
    let delta = |i: usize| if i % 2 == 0 { G } else { -G };

    // The reference: every move as its own step, to know which keys the moves touch.
    let mut plain = engine_with_limits(golden_model(), unlimited());
    for i in 0..N {
        plain.execute(&move_rep(delta(i))).expect("rep.move");
    }
    let keys: BTreeSet<AnyKey> = plain
        .history()
        .entries()
        .iter()
        .flat_map(|e| e.transaction().ops.iter().map(|op| op.key()))
        .collect();
    assert!(!keys.is_empty());

    let mut e = engine_with_limits(golden_model(), unlimited());
    let original = lines(e.model());
    // Timing acquisition, reporting and the release budget are opt-in.
    let started = std::env::var_os("EW_PERF_ASSERT").is_some().then(Instant::now);
    for i in 0..N {
        e.execute_with(&move_rep(delta(i)), Submit::from(Origin::Gui).coalescing("drag"))
            .expect("rep.move");
    }
    if let Some(started) = started {
        let elapsed = started.elapsed();
        eprintln!("{N} coalesced moves: {elapsed:?}");
        if !cfg!(debug_assertions) {
            assert!(
                elapsed.as_secs_f64() < 1.0,
                "{N} coalesced moves took {elapsed:?}"
            );
        }
    }

    assert_eq!(e.history().entries().len(), 1);
    let step = &e.history().entries()[0];
    assert_eq!(step.merged() as usize, N - 1);
    assert_eq!(step.op_count(), keys.len());
    let op_keys: Vec<AnyKey> = step.transaction().ops.iter().map(|op| op.key()).collect();
    assert!(
        op_keys.windows(2).all(|w| w[0] < w[1]),
        "one op per key, in key order"
    );
    assert_eq!(op_keys.into_iter().collect::<BTreeSet<_>>(), keys);
    assert_eq!(lines(e.model()), lines(plain.model()));

    e.undo(&Origin::Gui).expect("one undo");
    assert_eq!(lines(e.model()), original);
    e.redo(&Origin::Gui).expect("redo");
    assert_eq!(lines(e.model()), lines(plain.model()));
}

#[test]
fn the_budget_holds_after_many_steps() {
    const BUDGET: usize = 64 * 1024;
    const N: usize = 500;
    let mut e = engine_with_limits(
        golden_model(),
        HistoryLimits {
            max_steps: 10_000,
            max_bytes: BUDGET,
        },
    );
    for i in 0..N {
        let t = text(1_000, (i % 2) as u8);
        e.execute(&set_text(&[DEV_102F2], &t)).expect("fields.set");
        let h = e.history();
        assert!(
            h.bytes() <= BUDGET || h.entries().len() == 1,
            "step {i}: {} bytes in {} entries",
            h.bytes(),
            h.entries().len()
        );
    }
    let h = e.history();
    assert!(h.entries().len() < N, "{} entries are left", h.entries().len());
    assert!(h.entries().len() > 1, "the budget keeps a number of steps");
    assert!(h.bytes() > 0);
    // A step larger than the budget stays, alone.
    let mut e = engine_with_limits(
        golden_model(),
        HistoryLimits {
            max_steps: 10_000,
            max_bytes: 100,
        },
    );
    e.execute(&set_text(&[DEV_102F2], &text(1_000, 0))).unwrap();
    e.execute(&set_text(&[DEV_102F2], &text(1_000, 1))).unwrap();
    assert_eq!(e.history().entries().len(), 1);
    assert!(e.history().bytes() > 100);
    e.undo(&Origin::Gui)
        .expect("the single oversized step is undoable");
}

#[test]
fn accounting_is_within_2x_of_a_measured_step() {
    // Method. A counting `#[global_allocator]` would need `unsafe`, which the workspace forbids
    // (`unsafe_code = "forbid"`, NFR-SEC-04), so the reference is the known payload of the step instead of
    // an allocator reading. The history keeps one step (`max_steps: 1`). Step 1 gives every fixture device
    // a 1 MB text X; step 2 replaces them by a text Y. After step 2 the history alone keeps the eight rows
    // that hold X (the `before` rows of step 2: the model has Y), and the heap those rows own is at least
    // the eight texts (the `Device` structs and `Arc` headers come on top, a few KB each). The accounted
    // size must be within a factor of two of that payload, and it must not count the Y rows the live model
    // shares.
    let devices = [
        DEV_101F1, DEV_102G1, DEV_102F2, DEV_X1031, DEV_T1, DEV_T2, DEV_T3, DEV_3W1,
    ];
    const LEN: usize = 1_000_000;
    let payload = devices.len() * LEN;

    let mut e: Engine = engine_with_limits(
        golden_model(),
        HistoryLimits {
            max_steps: 1,
            max_bytes: usize::MAX,
        },
    );
    e.execute(&set_text(&devices, &text(LEN, 0))).expect("step 1");
    e.execute(&set_text(&devices, &text(LEN, 1))).expect("step 2");
    assert_eq!(e.history().entries().len(), 1);
    let accounted = e.history().bytes();
    eprintln!("payload {payload} bytes, accounted {accounted} bytes");
    assert!(
        payload / 2 <= accounted && accounted <= 2 * payload,
        "payload {payload}, accounted {accounted}"
    );
    // The step's `after` rows hold the same amount again but are the live model's: not counted.
    let step = &e.history().entries()[0];
    assert_eq!(accounted, step.bytes());
    assert!(step.after_bytes() >= payload);
}

#[test]
fn undo_counts_the_after_rows_of_the_redo_branch() {
    let mut e = engine_with_limits(golden_model(), unlimited());
    e.execute(&set_text(&[DEV_102F2], &text(10_000, 0)))
        .expect("fields.set");
    let applied = e.history().bytes();
    let step = &e.history().entries()[0];
    let after = step.after_bytes();
    assert!(after >= 10_000, "the new row holds the text ({after})");
    assert_eq!(
        applied,
        step.bytes(),
        "an applied step counts its before rows only"
    );

    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(e.history().bytes(), applied + after);
    e.redo(&Origin::Gui).expect("redo");
    assert_eq!(e.history().bytes(), applied);

    // Dropping the redo branch gives its bytes back.
    e.undo(&Origin::Gui).expect("undo");
    e.execute(&set_text(&[DEV_102F2], "short")).expect("fields.set");
    let h = e.history();
    assert_eq!(h.entries().len(), 1);
    assert_eq!(h.bytes(), h.entries()[0].bytes());
}
