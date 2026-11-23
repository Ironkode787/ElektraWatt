// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The dependency-recording hook of [`Q`](crate::Q) (ADR-0002 decisions 1, 2 and 4).
//!
//! Every tracked accessor of `Q` reports what it read as a [`DepKey`] together with the revision stamp it
//! observed (`changed_at` of the row, index key or table; ARCH §6.2). A `Q` without a recorder drops the
//! reports; one with a [`RecordingTracker`] keeps them for the tests. The memo engine of `ew-memo` receives the same reads
//! through the memo store a `Q` carries, beside this hook; query code calls only `Q`'s accessors either way.
//!
//! **Verifier contract.** A memo engine re-validates a value by re-reading every recorded [`DepKey`] and
//! comparing its current stamp with the recorded `observed`; a dependency is unchanged iff the current stamp
//! equals `observed`. The stamp of a `Row` is the row's `changed_at` (the table's for an absent row); of an
//! `IndexKey` the key's `changed_at` (the index's for an absent key); of a `Table` or `Index` the table's or
//! index's; of `Project` the project row's; of `Assets` the number of assets held; of `Reserved` the model's
//! revision. Stamps only grow, and a query result depends only on the recorded reads.

use std::cell::RefCell;
use std::fmt;

use ew_model::{AnyKey, IndexName, IxKey, Rev};
use ew_schema::Kind;

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

/// The optional recorder of a [`Q`](crate::Q) (`Q::with_recorder`): it keeps every read, with the query evaluating
/// at the time, and every query evaluation, for the tests and `--explain` style diagnostics. `enter` and `exit`
/// bracket the evaluation of one query, so that the reads in between are attributed to it (nested evaluations nest).
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

    /// A tracked read of `dep`, which carried revision `observed`.
    pub(crate) fn read(&self, dep: DepKey, observed: Rev) {
        let query = self.stack.borrow().last().copied();
        self.reads.borrow_mut().push(Recorded { query, dep, observed });
    }

    /// The evaluation of `family` for `key` starts.
    pub(crate) fn enter(&self, family: &'static str, key: &dyn fmt::Debug) {
        self.stack.borrow_mut().push(family);
        self.evaluated.borrow_mut().push((family, format!("{key:?}")));
    }

    /// The evaluation of `family` ends.
    pub(crate) fn exit(&self, family: &'static str) {
        let top = self.stack.borrow_mut().pop();
        debug_assert_eq!(top, Some(family), "unbalanced query evaluation");
    }
}
