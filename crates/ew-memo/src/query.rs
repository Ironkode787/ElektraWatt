// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the host implements: its inputs ([`Db`]) and its query families ([`Query`]).

use std::fmt::{self, Debug};

use crate::{Cx, DepKey, Durability, Rev};

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
/// **Push invalidation** ([`Store::advance_with`](crate::Store::advance_with)) adds a third rule. Every read
/// is registered in the store's reverse-dependency index under its *coarse* key, [`Db::coarse`], computed
/// in the snapshot the read observed. The touched inputs a host reports for a commit must include, for every
/// input whose **content** the commit changed (what a read of it observes, its absence included), its coarse
/// key in that snapshot. A host whose coarse key is the input itself (the default) reports the changed
/// inputs; a host with coarser keys (a page for every row drawn on it) reports the coarse keys of the changed
/// inputs before and after the change.
///
/// The rule is about content, not stamps: an input whose stamp changed while what a read observes stayed
/// equal need not be reported. Examples: a row put again with an equal value, an absent row or index bucket
/// whose stamp follows its table's, a key set and cleared again within one commit, a coarse stamp such as
/// the snapshot's revision for data without stamps of its own. A memo that read such an input is re-stamped by
/// push invalidation and keeps its value, which is right because the content is the same; a later pull
/// verification ([`Store::advance`](crate::Store::advance)) still sees the newer stamp and re-executes, which
/// is conservative. Leaving out an input whose content changed makes memos stale, and oracle mode reports it.
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

    /// The key under which a read of `input` is registered for push invalidation, in this snapshot (see the
    /// trait documentation). The default is the input itself.
    fn coarse(&self, input: &Self::Input) -> Self::Input {
        input.clone()
    }
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

/// What changed for a memo since it was last verified, handed to an incremental re-execution
/// ([`Query::execute_incremental`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Changes<I> {
    /// The inputs the host reported touched ([`Store::advance_with`](crate::Store::advance_with)) since the
    /// memo was verified, in revision order (duplicates possible); `None` when the store no longer holds them
    /// all.
    pub touched: Option<Vec<I>>,
    /// The dependency keys through which the memo was marked possibly changed since then, in marking order:
    /// coarse inputs, memos (a dependency memo, or a memo marked on the way) and parts; `None` when the store
    /// stopped collecting them.
    pub via: Option<Vec<DepKey<I>>>,
}

/// A query family: a pure function from a key to a value, over the host's inputs (ADR-0002 decision 1).
///
/// `execute` must read inputs only through `cx` (reporting each with [`Cx::read`]) and other queries only
/// through [`Cx::get`] or [`Cx::get_part`]; anything else it reads is untracked, and its memo would go stale.
/// It must be deterministic: no time, no randomness, no iteration order of hash maps.
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

    /// Whether memoised executions go through [`Query::execute_incremental`] (ADR-0002 amendment 4: the
    /// incremental re-execution hook). Cold evaluations (oracle mode, [`Store::cold`](crate::Store::cold))
    /// always call [`Query::execute`], which stays the reference.
    const INCREMENTAL: bool = false;

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

    /// Computes the value in a memoised evaluation of an [`INCREMENTAL`](Query::INCREMENTAL) family.
    ///
    /// `previous` is the memo's previous value with what changed since it was verified, when the store
    /// holds both; the execution then starts with the previous memo's dependencies already recorded, and every
    /// dependency it reads again replaces the old entry. The hook must therefore (re-)read every dependency
    /// its new value relies on that `changes` names or that is new, and must account for every key in
    /// `changes.via`; anything it does not re-read keeps its old observed revision. A producer of parts
    /// ([`Cx::changed_parts`]) declares the parts the change touched. With `previous` `None` (the first
    /// execution, an evicted value, or changes the store does not know), the hook computes from scratch.
    ///
    /// The result must equal [`Query::execute`] on the same snapshot; oracle mode checks it.
    ///
    /// # Errors
    ///
    /// As [`Query::execute`].
    fn execute_incremental(
        cx: &mut Cx<'_, D>,
        key: &Self::Key,
        previous: Option<(&Self::Value, &Changes<D::Input>)>,
    ) -> Result<Self::Value, crate::Cycle> {
        let _ = previous;
        Self::execute(cx, key)
    }
}
