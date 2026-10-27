// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The dependency-recording hook of [`Q`](crate::Q) (ADR-0002 decisions 1, 2 and 4).
//!
//! Every tracked accessor of `Q` reports what it read as a [`DepKey`] together with the revision stamp it
//! observed (`changed_at` of the row, index key or table; ARCH §6.2). Phase 1 is cold, so the default
//! [`ColdTracker`] drops the reports. The memo engine of `ew-memo` (after the performance spike, G-SP)
//! implements [`Tracker`] to collect `deps: SmallVec<(DepKey, observed)>` per memo; query code does not
//! change, because queries only ever call `Q`'s accessors.
//!
//! **Verifier contract.** A memo engine re-validates a value by re-reading every recorded [`DepKey`] and
//! comparing its current stamp with the recorded `observed`; a dependency is unchanged iff the current stamp
//! equals `observed`. The stamp of a `Row` is the row's `changed_at` (the table's for an absent row); of an
//! `IndexKey` the key's `changed_at` (the index's for an absent key); of a `Table` or `Index` the table's or
//! index's; of `Project` the project row's; of `Assets` the number of assets held; of `Reserved` the model's
//! revision. Stamps only grow, and a query result depends only on the recorded reads.

use std::cell::RefCell;
use std::fmt;

use ew_model::{AnyKey, EndpointKey, IndexName, Rev};
use ew_schema::{AnyId, Kind};

/// The key of one index bucket.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum IxKey {
    /// A record ID (`reps_by_page(page)` …).
    Id(AnyId),
    /// An endpoint (`connections_at_cp(f, idx)` …).
    Endpoint(EndpointKey),
}

/// One tracked read.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum DepKey {
    /// The project row (master data, settings, the pinned derivation profile).
    Project,
    /// The asset store: whether an asset is held and its reference (the format of an image). The observed
    /// revision is the number of assets the model holds ([`Model::assets_stamp`](ew_model::Model::assets_stamp)):
    /// assets are only ever added, and one can arrive in a transaction that changes no row the reader read.
    Assets,
    /// One row, present or absent. For an absent row the observed revision is the table's.
    Row(AnyKey),
    /// A whole table (iteration, length).
    Table(Kind),
    /// One bucket of a base index. For an absent key the observed revision is the index's.
    IndexKey(IndexName, IxKey),
    /// A whole base index (iteration, range reads).
    Index(IndexName),
    /// The records of reserved kinds (revisions, …), which carry no revision stamps: the observed
    /// revision is the model's.
    Reserved,
}

/// Receives the reads of a [`Q`](crate::Q).
///
/// `enter` and `exit` bracket the evaluation of one query, so that an engine can attribute the reads in
/// between to that query (nested evaluations nest). Every value a query obtains from another query,
/// cached or fresh, is also reported as [`Tracker::query_read`].
pub trait Tracker {
    /// A tracked read of `dep`, which carried revision `observed`.
    fn read(&self, dep: DepKey, observed: Rev);

    /// The evaluation of `family` for `key` starts.
    fn enter(&self, family: &'static str, key: &dyn fmt::Debug) {
        let _ = (family, key);
    }

    /// The evaluation of `family` ends.
    fn exit(&self, family: &'static str) {
        let _ = family;
    }

    /// The value of `family` for `key` was read (by a query or by the caller of `Q::get`).
    fn query_read(&self, family: &'static str, key: &dyn fmt::Debug) {
        let _ = (family, key);
    }
}

/// The phase-1 tracker: records nothing (ADR-0002 decision 2).
#[derive(Clone, Copy, Debug, Default)]
pub struct ColdTracker;

impl Tracker for ColdTracker {
    fn read(&self, _dep: DepKey, _observed: Rev) {}
}

/// One read recorded by [`RecordingTracker`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Recorded {
    /// The innermost query evaluating at the time of the read; `None` outside any query.
    pub query: Option<&'static str>,
    /// What was read.
    pub dep: DepKey,
    /// The revision it carried.
    pub observed: Rev,
}

/// A tracker that keeps every read, for tests and `--explain` style diagnostics.
#[derive(Debug, Default)]
pub struct RecordingTracker {
    reads: RefCell<Vec<Recorded>>,
    stack: RefCell<Vec<&'static str>>,
    evaluated: RefCell<Vec<(&'static str, String)>>,
}

impl RecordingTracker {
    /// An empty recorder.
    pub fn new() -> RecordingTracker {
        RecordingTracker::default()
    }

    /// The reads so far, in order.
    pub fn reads(&self) -> Vec<Recorded> {
        self.reads.borrow().clone()
    }

    /// The query evaluations so far (family and `Debug` key), in start order.
    pub fn evaluated(&self) -> Vec<(&'static str, String)> {
        self.evaluated.borrow().clone()
    }
}

impl Tracker for RecordingTracker {
    fn read(&self, dep: DepKey, observed: Rev) {
        let query = self.stack.borrow().last().copied();
        self.reads.borrow_mut().push(Recorded { query, dep, observed });
    }

    fn enter(&self, family: &'static str, key: &dyn fmt::Debug) {
        self.stack.borrow_mut().push(family);
        self.evaluated.borrow_mut().push((family, format!("{key:?}")));
    }

    fn exit(&self, family: &'static str) {
        let top = self.stack.borrow_mut().pop();
        debug_assert_eq!(top, Some(family), "unbalanced query evaluation");
    }
}

/// Lets a shared recorder be handed to a `Q` while the caller keeps a handle on it.
impl<T: Tracker + ?Sized> Tracker for std::rc::Rc<T> {
    fn read(&self, dep: DepKey, observed: Rev) {
        (**self).read(dep, observed);
    }

    fn enter(&self, family: &'static str, key: &dyn fmt::Debug) {
        (**self).enter(family, key);
    }

    fn exit(&self, family: &'static str) {
        (**self).exit(family);
    }

    fn query_read(&self, family: &'static str, key: &dyn fmt::Debug) {
        (**self).query_read(family, key);
    }
}
