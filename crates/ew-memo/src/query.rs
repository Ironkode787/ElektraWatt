// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the host implements: its inputs ([`Db`]) and its query families ([`Query`]).

use std::fmt::{self, Debug};

use crate::{Cx, Durability, Rev};

/// The host's inputs, as the memo engine sees them: keys with a revision stamp and a durability.
///
/// The host reads its data itself (from its own snapshot, through [`Cx::db`]) and reports every read with
/// [`Cx::read`]. Its stamps must follow two rules, or memos go stale:
///
/// - `changed_at(k)` is the revision of the last change of whatever a read of `k` observes, including its
///   absence: a key whose row was deleted (or never existed) reports a revision at least as recent as the
///   deletion, for example the revision of the table's key set.
/// - every change happens in a revision the store was [advanced](crate::Store::advance) to, with a
///   durability at least as high as the changed input's durability both before and after the change (an
///   input that moves from library to drawing durability is a library change).
///
/// Oracle mode ([`Oracle`](crate::Oracle)) checks the outcome: a memo that disagrees with a cold run is
/// reported as a [`Mismatch`](crate::Mismatch).
pub trait Db {
    /// A key of an input: a row, an index key, a table, a setting.
    type Input: Clone + Ord + Debug + Send + Sync + 'static;

    /// The revision of the input's last change.
    fn changed_at(&self, input: &Self::Input) -> Rev;

    /// How rarely the input changes.
    fn durability(&self, input: &Self::Input) -> Durability;
}

/// How a query family decides that a re-executed value equals the previous one (early cut-off, ADR-0002
/// decision 4) and that a memo equals a cold run (oracle mode).
pub enum Equivalence<V> {
    /// Compare the values. The previous value is kept while the memo has one; a value evicted by the LRU
    /// cannot be compared, so its re-execution counts as a change unless its dependencies were unchanged.
    Eq(fn(&V, &V) -> bool),
    /// Compare 128-bit fingerprints computed by the host (bulky values such as display lists). The
    /// fingerprint outlives an eviction, so a re-computed evicted value still cuts off.
    Hash128(fn(&V) -> u128),
}

impl<V: Eq> Equivalence<V> {
    /// `Eq` through `V`'s `PartialEq`.
    pub fn by_eq() -> Equivalence<V> {
        Equivalence::Eq(<V as PartialEq>::eq)
    }
}

impl<V> Clone for Equivalence<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V> Copy for Equivalence<V> {}

impl<V> Debug for Equivalence<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Equivalence::Eq(_) => f.write_str("Eq"),
            Equivalence::Hash128(_) => f.write_str("Hash128"),
        }
    }
}

/// A query family: a pure function from a key to a value, over the host's inputs (ADR-0002 decision 1).
///
/// `execute` must read inputs only through `cx` (reporting each with [`Cx::read`]) and other queries only
/// through [`Cx::get`]; anything else it reads is untracked, and its memo would go stale. It must be
/// deterministic: no time, no randomness, no iteration order of hash maps.
pub trait Query<D: Db>: 'static {
    /// The key.
    type Key: Clone + Ord + Debug + Send + Sync + 'static;
    /// The value; shared as `Arc<Value>`.
    type Value: Debug + Send + Sync + 'static;

    /// The family name used in statistics, oracle reports and cycle messages.
    const NAME: &'static str;

    /// Whether the values are bulky and live under the store's byte-budget LRU.
    const LRU: bool = false;

    /// Whether the store keeps an index of the family's reverse dependencies. Without it,
    /// [`Store::dependents`](crate::Store::dependents) scans the family's memos; the answer is the same.
    const RDEPS: bool = false;

    /// How values are compared.
    fn equivalence() -> Equivalence<Self::Value>;

    /// The bytes a value occupies, as the host counts them (for the LRU budget and memory reports).
    fn bytes(_value: &Self::Value) -> usize {
        std::mem::size_of::<Self::Value>()
    }

    /// Computes the value.
    ///
    /// # Errors
    ///
    /// [`Cycle`](crate::Cycle) when a nested [`Cx::get`] detected a cycle (in release builds; debug builds
    /// panic). Propagate it with `?`.
    fn execute(cx: &mut Cx<'_, D>, key: &Self::Key) -> Result<Self::Value, crate::Cycle>;
}
