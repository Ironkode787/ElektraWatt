// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A query that panics leaves no state behind (ADR-0002 decision 4): after `catch_unwind` the slot is gone
//! and the tracker stack is balanced.

mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use common::Fx;
use ew_derive::{DepKey, Policy, Q, Query, RecordingTracker};

static OUTER_ARMED: AtomicBool = AtomicBool::new(false);
static INNER_ARMED: AtomicBool = AtomicBool::new(false);
static PLAIN_ARMED: AtomicBool = AtomicBool::new(false);

/// Panics on its first evaluation (while `PLAIN_ARMED`), then returns the key plus one.
struct Plain;

impl Query for Plain {
    type Key = u32;
    type Value = u32;
    const FAMILY: &'static str = "unwind_plain";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &u32) -> Arc<u32> {
        let _ = q.project();
        assert!(!PLAIN_ARMED.swap(false, Ordering::SeqCst), "boom");
        Arc::new(key + 1)
    }
}

struct Inner;

impl Query for Inner {
    type Key = u32;
    type Value = u32;
    const FAMILY: &'static str = "unwind_inner";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &u32) -> Arc<u32> {
        let _ = q.project();
        assert!(!INNER_ARMED.swap(false, Ordering::SeqCst), "inner boom");
        Arc::new(key * 2)
    }
}

struct Outer;

impl Query for Outer {
    type Key = u32;
    type Value = u32;
    const FAMILY: &'static str = "unwind_outer";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &u32) -> Arc<u32> {
        assert!(!OUTER_ARMED.swap(false, Ordering::SeqCst), "outer boom");
        Arc::new(*q.get::<Inner>(*key) + 1)
    }
}

fn model() -> ew_model::Model {
    Fx::new().build()
}

#[test]
fn a_panicking_query_can_be_evaluated_again() {
    let m = model();
    let rec = Rc::new(RecordingTracker::new());
    let mut q = Q::with_tracker(&m, Box::new(Rc::clone(&rec)));
    PLAIN_ARMED.store(true, Ordering::SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| q.get::<Plain>(7))).is_err());
    assert_eq!(q.diagnostics().cached("unwind_plain"), 0);
    // No false cycle: the key evaluates again and returns the value.
    assert_eq!(*q.get::<Plain>(7), 8);
    assert_eq!(q.diagnostics().cached("unwind_plain"), 1);
    assert!(q.diagnostics().notes().is_empty());
    // The tracker stack is balanced: a read at top level is attributed to no query.
    let before = rec.reads().len();
    let _ = q.project();
    let reads = rec.reads();
    assert_eq!(reads.len(), before + 1);
    let last = reads.last().unwrap();
    assert_eq!(last.dep, DepKey::Project);
    assert_eq!(last.query, None);
}

#[test]
fn a_panicking_inner_query_unwinds_both_levels() {
    let m = model();
    let rec = Rc::new(RecordingTracker::new());
    let mut q = Q::with_tracker(&m, Box::new(Rc::clone(&rec)));
    INNER_ARMED.store(true, Ordering::SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| q.get::<Outer>(5))).is_err());
    assert_eq!(q.diagnostics().cached("unwind_outer"), 0);
    assert_eq!(q.diagnostics().cached("unwind_inner"), 0);
    assert_eq!(*q.get::<Outer>(5), 11);
    assert_eq!(q.diagnostics().cached("unwind_outer"), 1);
    assert_eq!(q.diagnostics().cached("unwind_inner"), 1);
    assert!(q.diagnostics().notes().is_empty());
    let before = rec.reads().len();
    let _ = q.project();
    assert_eq!(rec.reads()[before].query, None);
}

#[test]
fn a_panicking_outer_query_before_its_inner_read() {
    let m = model();
    let mut q = Q::new(&m);
    OUTER_ARMED.store(true, Ordering::SeqCst);
    assert!(catch_unwind(AssertUnwindSafe(|| q.get::<Outer>(3))).is_err());
    assert_eq!(q.diagnostics().cached("unwind_outer"), 0);
    assert_eq!(*q.get::<Outer>(3), 7);
}
