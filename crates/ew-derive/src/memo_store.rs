// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The memo backend of [`Q`]: the store a session keeps across requests and the bridge that runs an
//! `ew-derive` family as an `ew-memo` query (ADR-0002 amendment 4 item (c), ARCH §6.2 "Additions after
//! G-SP").
//!
//! - **[`MemoStore`]** holds an `ew_memo::Store` and the snapshot it was last advanced to. The session
//!   advances it after every commit with the commit's touched keys ([`MemoStore::advance`]): push
//!   invalidation over the page-level keys of `memo_keys`. [`MemoStore::query`] hands out a `Q` that
//!   memoises the `Memo` families.
//! - **The bridge** `MemoOf<F>` runs `F::run` (or `F::run_incremental`) on the reading `Q` inside a memo
//!   frame: the tracked reads of the body and its cold families go to the execution; a nested memoised family
//!   is one memo dependency. Cold (oracle mode), it evaluates `F` in a fresh cold `Q`: the cold family stays the
//!   reference.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

use ew_memo::{Cx, Durability, Equivalence, FamilyMemory, Mismatch, Oracle, Stats};
use ew_model::{AnyKey, Model};

use crate::memo_keys::{self, MemoInput, PartKey};
use crate::q::DeriveStore;
use crate::query::{Changes, Query};
use crate::{Cycle, Q, ReportPagesProvider};

/// The `ew-memo` query that memoises the `ew-derive` family `F`.
pub(crate) struct MemoOf<F>(PhantomData<F>);

fn same<F: Query>(a: &Arc<F::Value>, b: &Arc<F::Value>) -> bool {
    Arc::ptr_eq(a, b) || a == b
}

/// One memoised (or, in oracle mode, cold) evaluation of `F`.
fn run<F: Query>(
    cx: &mut Cx<'_, Q>,
    key: &F::Key,
    previous: Option<(&Arc<F::Value>, &ew_memo::Changes<MemoInput>)>,
) -> Arc<F::Value> {
    let q = cx.db();
    if cx.is_cold() {
        // The reference: `F` evaluated cold, nested families too, in a fresh `Q`.
        return q.cold_twin().get::<F>(key.clone());
    }
    q.memo_frame::<F>(key, || {
        if F::INCREMENTAL {
            F::run_incremental(q, key, previous.map(|(p, inner)| (p, Changes { inner })))
        } else {
            F::run(q, key)
        }
    })
}

impl<F: Query> ew_memo::Query<Q> for MemoOf<F> {
    type Key = F::Key;
    type Value = Arc<F::Value>;
    const NAME: &'static str = F::FAMILY;
    const INCREMENTAL: bool = F::INCREMENTAL;

    fn equivalence() -> Equivalence<Arc<F::Value>> {
        Equivalence::Eq(same::<F>)
    }

    fn execute(cx: &mut Cx<'_, Q>, key: &F::Key) -> Result<Arc<F::Value>, ew_memo::Cycle> {
        Ok(run::<F>(cx, key, None))
    }

    fn execute_incremental(
        cx: &mut Cx<'_, Q>,
        key: &F::Key,
        previous: Option<(&Arc<F::Value>, &ew_memo::Changes<MemoInput>)>,
    ) -> Result<Arc<F::Value>, ew_memo::Cycle> {
        Ok(run::<F>(cx, key, previous))
    }
}

/// Reads `F(key)` through the store for `q`.
pub(crate) fn get<F: Query>(q: &Q, store: &DeriveStore, key: &F::Key) -> Result<Arc<F::Value>, Cycle> {
    match store.get::<MemoOf<F>>(q, key) {
        Ok(m) => Ok(Arc::clone(&m)),
        Err(_) => Err(Cycle {
            family: F::FAMILY,
            key: format!("{key:?}"),
        }),
    }
}

/// Reads `F(key)` depending on its part `part` only.
pub(crate) fn get_part<F: Query>(
    q: &Q,
    store: &DeriveStore,
    key: &F::Key,
    part: PartKey,
) -> Result<Arc<F::Value>, Cycle> {
    let cycle = || Cycle {
        family: F::FAMILY,
        key: format!("{key:?}"),
    };
    let m = store
        .get_part::<MemoOf<F>>(q, key, &MemoInput::Part(part))
        .map_err(|_| cycle())?;
    Ok(Arc::clone(&m))
}

/// The memo store of one session: an `ew-memo` store over the session's current snapshot (ADR-0002
/// decisions 3–5, amendment 4).
///
/// It holds the snapshot it was last advanced to; [`MemoStore::query`] hands out memoising `Q`s over it,
/// [`MemoStore::advance`] moves it to the next committed snapshot with push invalidation. A `Q` over any
/// other snapshot (a preview's candidate, an export fork) is cold: build it with [`Q::new`]. The report
/// engine is part of the store's configuration, fixed by [`MemoStore::with_report_pages`].
///
/// A `MemoStore` is neither `Send` nor `Sync`: the `ew-memo` store is shared with the `Q`s it hands out
/// through an `Rc` and mutated through `RefCell`s. It lives on the thread that owns the session (the UI thread,
/// ARCH §2); a job on another thread derives from its own snapshot with a cold `Q`.
pub struct MemoStore {
    store: RefCell<Rc<DeriveStore>>,
    model: RefCell<Model>,
    provider: Option<Arc<dyn ReportPagesProvider>>,
    oracle: Cell<Oracle>,
}

impl fmt::Debug for MemoStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoStore")
            .field("rev", &self.model.borrow().rev())
            .field("store", &self.store.borrow())
            .finish_non_exhaustive()
    }
}

/// A fresh `ew-memo` store at the revision of `model`. Cycles are returned, never panicked, so that
/// [`Q::try_get`] keeps its contract; `Q::get` panics on them itself.
fn fresh_store(model: &Model, oracle: Oracle) -> Rc<DeriveStore> {
    let store = DeriveStore::new();
    store.set_cycle_panics(false);
    store.set_oracle(oracle);
    if model.rev().0 > 0 {
        store.advance(ew_memo::Rev(model.rev().0), Durability::Library);
    }
    Rc::new(store)
}

impl MemoStore {
    /// An empty store over `model`, without a report engine.
    pub fn new(model: &Model) -> MemoStore {
        MemoStore {
            store: RefCell::new(fresh_store(model, Oracle::Off)),
            model: RefCell::new(model.clone()),
            provider: None,
            oracle: Cell::new(Oracle::Off),
        }
    }

    /// This store with the report engine installed: [`PageOrder`](crate::PageOrder) of its `Q`s includes
    /// the generated report pages. A configuration change: the memos are dropped.
    #[must_use]
    pub fn with_report_pages(self, provider: Arc<dyn ReportPagesProvider>) -> MemoStore {
        let store = fresh_store(&self.model.borrow(), self.oracle.get());
        MemoStore {
            store: RefCell::new(store),
            provider: Some(provider),
            ..self
        }
    }

    fn store(&self) -> Rc<DeriveStore> {
        Rc::clone(&self.store.borrow())
    }

    /// An O(1) snapshot of this store that evolves independently (`ew_memo::Store::fork`): the same snapshot,
    /// report engine and oracle mode, the memos shared until either side writes. Advancing the fork does not
    /// touch this store. For a benchmark sample or a job on this thread; a fork is no more `Send` than its
    /// store.
    ///
    /// A fork only advances along its own descendants: [`MemoStore::advance`] takes a snapshot that a commit
    /// produced from the fork's current snapshot (the same history as the store it was taken from up to the
    /// fork). Moving it to another branch of history starts an empty store, as for any `MemoStore`.
    #[must_use]
    pub fn fork(&self) -> MemoStore {
        MemoStore {
            store: RefCell::new(Rc::new(self.store().fork())),
            model: RefCell::new(self.model.borrow().clone()),
            provider: self.provider.clone(),
            oracle: Cell::new(self.oracle.get()),
        }
    }

    /// A memoising `Q` over the store's snapshot (request-scoped: build one per evaluation).
    pub fn query(&self) -> Q {
        Q::with_memo(&self.model.borrow(), self.store(), self.provider.clone())
    }

    /// The revision of the store's snapshot.
    pub fn rev(&self) -> ew_model::Rev {
        self.model.borrow().rev()
    }

    /// Moves the store from its snapshot to `after`, the snapshot a commit produced from it by putting the
    /// rows `keys` (the change feed's touched keys; the union over several commits is fine): push
    /// invalidation with the page-level keys of the touched rows before and after. `after` must descend from
    /// the store's snapshot through those commits. A snapshot at the same revision is ignored, unless its asset
    /// store differs: a transaction without rows can bring an asset (a lost logo set again), which changes what
    /// the readers of [`DepKey::Assets`](crate::DepKey::Assets) answer but not the revision, and an `ew_memo` store
    /// cannot advance to an equal revision, so it starts a new, empty store at that snapshot. An older snapshot
    /// (another history) starts a new, empty store too.
    ///
    /// There is deliberately no "move to any snapshot": verifying memos of one
    /// history by pull against the stamps of another compares incomparable revisions, which can find a stale
    /// memo green. A session that opens, recovers or reloads a project builds a new `MemoStore`.
    pub fn advance(&self, after: &Model, keys: &[AnyKey]) {
        let rev = after.rev();
        let before = self.model.borrow().clone();
        if rev == before.rev() {
            // Nothing changed, unless an asset arrived without a row (the stamp of the asset store is its size).
            if after.assets_stamp() != before.assets_stamp() {
                self.restart(after);
            }
            return;
        }
        if rev < before.rev() {
            self.restart(after);
            return;
        }
        let (touched, durability) = memo_keys::touched(&before, after, keys);
        self.store()
            .advance_with(ew_memo::Rev(rev.0), durability, touched);
        *self.model.borrow_mut() = after.clone();
    }

    /// Drops every memo and starts over at `model` (the old memos describe another history; `Q`s still
    /// holding the old store keep it until they are dropped).
    fn restart(&self, model: &Model) {
        *self.store.borrow_mut() = fresh_store(model, self.oracle.get());
        *self.model.borrow_mut() = model.clone();
    }

    /// Sets oracle mode: memoised reads are re-run cold and compared (CI, debug builds, nightly).
    pub fn set_oracle(&self, oracle: Oracle) {
        self.oracle.set(oracle);
        self.store().set_oracle(oracle);
    }

    /// The oracle's mismatches since the last call.
    pub fn take_mismatches(&self) -> Vec<Mismatch> {
        self.store().take_mismatches()
    }

    /// The engine's statistics since the last advance (one commit).
    pub fn commit_stats(&self) -> Stats {
        self.store().commit_stats()
    }

    /// The engine's statistics over the store's lifetime.
    pub fn total_stats(&self) -> Stats {
        self.store().total_stats()
    }

    /// What each family holds: memos and dependency entries.
    pub fn memory(&self) -> Vec<FamilyMemory> {
        self.store().memory()
    }

    /// The (registration key, memo) pairs of the push index.
    pub fn push_index_entries(&self) -> usize {
        self.store().push_index_entries()
    }

    /// The state and the dependency trail of the memo of `F(key)` (`ew_memo::Store::explain`;
    /// `ew query --explain`): read-only, nothing is computed. A family whose policy is
    /// [`Policy::Cold`](crate::Policy::Cold) has no memo.
    pub fn explain<F: Query>(&self, key: &F::Key) -> ew_memo::Explain<MemoInput> {
        self.store().explain::<MemoOf<F>>(key)
    }
}
