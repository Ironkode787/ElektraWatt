// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The memo backend of [`Q`]: the store a session keeps across requests and the bridge that runs an
//! `ew-derive` family as an `ew-memo` query (ADR-0002 amendment 4 item (c), ARCH §6.2 "Additions after
//! G-SP").
//!
//! - **[`MemoStore`]** holds an `ew_memo::Store` and the snapshot it was last advanced to. The session
//!   advances it after every commit with the commit's touched keys ([`MemoStore::advance`]): push
//!   invalidation over the page-level keys of `memo_keys`. [`MemoStore::query`] hands out a `Q` that
//!   memoises the `Memo` and `MemoLru` families.
//! - **The bridge** `MemoOf<F>` runs `F::run` (or `F::run_incremental`) on the reading `Q` inside a memo
//!   frame: the tracked reads of the body and its cold families go to the execution; a nested memoised family
//!   is one memo dependency. Its value is the family's value with the notes the execution pushed, so that a
//!   memo hit replays them; equivalence compares both (a changed note is a changed value). Cold (oracle mode),
//!   it evaluates `F` in a fresh cold `Q`: the cold family stays the reference.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

use ew_memo::{Cx, Durability, Equivalence, FamilyMemory, Mismatch, Oracle, Stats};
use ew_model::{AnyKey, Model};

use crate::memo_keys::{self, MemoInput, PartKey};
use crate::note::DeriveNote;
use crate::q::DeriveStore;
use crate::query::{Changes, Policy, Query};
use crate::{Cycle, Q, ReportPagesProvider};

/// A memoised value with the notes its execution pushed.
pub(crate) struct Memoised<V> {
    pub(crate) value: Arc<V>,
    pub(crate) notes: Arc<[DeriveNote]>,
}

impl<V: fmt::Debug> fmt::Debug for Memoised<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Memoised")
            .field("value", &self.value)
            .field("notes", &self.notes)
            .finish()
    }
}

/// The `ew-memo` query that memoises the `ew-derive` family `F`.
pub(crate) struct MemoOf<F>(PhantomData<F>);

/// A deterministic 128-bit fingerprint of the notes (FNV-1a over their `Debug` text).
fn notes_fingerprint(notes: &[DeriveNote]) -> u128 {
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
    let mut h: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    for n in notes {
        for b in format!("{n:?}").bytes() {
            h ^= u128::from(b);
            h = h.wrapping_mul(PRIME);
        }
        h = h.wrapping_mul(PRIME);
    }
    h
}

fn same<F: Query>(a: &Memoised<F::Value>, b: &Memoised<F::Value>) -> bool {
    (Arc::ptr_eq(&a.value, &b.value) || a.value == b.value) && a.notes == b.notes
}

fn fingerprint<F: Query>(m: &Memoised<F::Value>) -> u128 {
    F::fingerprint(&m.value) ^ notes_fingerprint(&m.notes)
}

/// One memoised (or, in oracle mode, cold) evaluation of `F`.
fn run<F: Query>(
    cx: &mut Cx<'_, Q>,
    key: &F::Key,
    previous: Option<(&Memoised<F::Value>, &ew_memo::Changes<MemoInput>)>,
) -> Memoised<F::Value> {
    let q = cx.db();
    if cx.is_cold() {
        // The reference: `F` evaluated cold, nested families too, in a fresh `Q`.
        let mut twin = q.cold_twin();
        let value = twin.get::<F>(key.clone());
        let notes: Vec<DeriveNote> = twin
            .diagnostics()
            .notes()
            .into_iter()
            .filter(|n| !matches!(n, DeriveNote::UnknownProfile { .. }))
            .collect();
        return Memoised {
            value,
            notes: notes.into(),
        };
    }
    let (value, notes) = q.memo_frame::<F>(key, || {
        if F::INCREMENTAL {
            F::run_incremental(q, key, previous.map(|(p, inner)| (&p.value, Changes { inner })))
        } else {
            F::run(q, key)
        }
    });
    Memoised {
        value,
        notes: notes.into(),
    }
}

impl<F: Query> ew_memo::Query<Q> for MemoOf<F> {
    type Key = F::Key;
    type Value = Memoised<F::Value>;
    const NAME: &'static str = F::FAMILY;
    const LRU: bool = matches!(F::POLICY, Policy::MemoLru);
    const INCREMENTAL: bool = F::INCREMENTAL;

    fn equivalence() -> Equivalence<Memoised<F::Value>> {
        if F::FINGERPRINT {
            Equivalence::Hash128(fingerprint::<F>)
        } else {
            Equivalence::Eq(same::<F>)
        }
    }

    fn bytes(value: &Memoised<F::Value>) -> usize {
        F::bytes(&value.value) + std::mem::size_of_val::<[DeriveNote]>(&value.notes)
    }

    fn execute(cx: &mut Cx<'_, Q>, key: &F::Key) -> Result<Memoised<F::Value>, ew_memo::Cycle> {
        Ok(run::<F>(cx, key, None))
    }

    fn execute_incremental(
        cx: &mut Cx<'_, Q>,
        key: &F::Key,
        previous: Option<(&Memoised<F::Value>, &ew_memo::Changes<MemoInput>)>,
    ) -> Result<Memoised<F::Value>, ew_memo::Cycle> {
        Ok(run::<F>(cx, key, previous))
    }
}

/// Reads `F(key)` through the store for `q` and replays its notes.
pub(crate) fn get<F: Query>(q: &Q, store: &DeriveStore, key: &F::Key) -> Result<Arc<F::Value>, Cycle> {
    match store.get::<MemoOf<F>>(q, key) {
        Ok(m) => {
            q.replay(&m.notes);
            Ok(Arc::clone(&m.value))
        }
        Err(_) => Err(Cycle {
            family: F::FAMILY,
            key: format!("{key:?}"),
        }),
    }
}

/// Reads `F(key)` depending on its part `part` only, and replays its notes.
// Used through `Q::get_part` by the per-potential families (WP-S1.07 next step).
#[allow(dead_code)]
pub(crate) fn get_part<F: Query>(
    q: &Q,
    store: &DeriveStore,
    key: &F::Key,
    part: PartKey,
) -> Result<Arc<F::Value>, Cycle> {
    match store.get_part::<MemoOf<F>>(q, key, &MemoInput::Part(part)) {
        Ok(m) => {
            q.replay(&m.notes);
            Ok(Arc::clone(&m.value))
        }
        Err(_) => Err(Cycle {
            family: F::FAMILY,
            key: format!("{key:?}"),
        }),
    }
}

/// The memo store of one session: an `ew-memo` store over the session's current snapshot (ADR-0002
/// decisions 3–5, amendment 4).
///
/// It holds the snapshot it was last advanced to; [`MemoStore::query`] hands out memoising `Q`s over it,
/// [`MemoStore::advance`] moves it to the next committed snapshot with push invalidation. A `Q` over any
/// other snapshot (a preview's candidate, an export fork) is cold: build it with [`Q::new`]. The report
/// engine is part of the store's configuration (FT-60), fixed by [`MemoStore::with_report_pages`].
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

    /// A memoising `Q` over the store's snapshot (request-scoped: build one per evaluation).
    pub fn query(&self) -> Q {
        Q::with_memo(&self.model.borrow(), self.store(), self.provider.clone())
    }

    /// The revision of the store's snapshot.
    pub fn rev(&self) -> ew_model::Rev {
        self.model.borrow().rev()
    }

    /// Moves the store from its snapshot to `after`, the snapshot a commit produced by putting the rows
    /// `keys` (the change feed's touched keys; the union over several commits is fine): push invalidation
    /// with the page-level keys of the touched rows before and after. A snapshot at the same revision is
    /// ignored; an older one replaces the store.
    pub fn advance(&self, after: &Model, keys: &[AnyKey]) {
        let rev = after.rev();
        let before = self.model.borrow().clone();
        if rev == before.rev() {
            return;
        }
        if rev < before.rev() {
            self.replace(after);
            return;
        }
        let (touched, durability) = memo_keys::touched(&before, after, keys);
        self.store()
            .advance_with(ew_memo::Rev(rev.0), durability, touched);
        *self.model.borrow_mut() = after.clone();
    }

    /// Moves the store to `model` without knowing what changed (an open, a recovery, a reload): every memo is
    /// verified by a dependency walk at its next read. An older or equal revision starts a new store (the old
    /// memos describe another history; `Q`s still holding the old store keep it until they are dropped).
    pub fn replace(&self, model: &Model) {
        let current = self.model.borrow().rev();
        if model.rev() > current {
            self.store()
                .advance(ew_memo::Rev(model.rev().0), Durability::Library);
        } else {
            *self.store.borrow_mut() = fresh_store(model, self.oracle.get());
        }
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

    /// Memory per family (G-SP-04): memos, values, value bytes, dependency entries.
    pub fn memory(&self) -> Vec<FamilyMemory> {
        self.store().memory()
    }

    /// The (registration key, memo) pairs of the push index.
    pub fn push_index_entries(&self) -> usize {
        self.store().push_index_entries()
    }
}
