// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The memo store: red-green verification with early cut-off, durability, the LRU, statistics, reverse
//! dependencies, cycle detection and oracle mode.

use std::any::{Any, TypeId};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Debug};
use std::num::NonZeroU32;
use std::sync::Arc;

use smallvec::SmallVec;

use crate::key::{DepKey, FamilyId, MemoRef, SlotId};
use crate::lru::Lru;
use crate::query::{Db, Equivalence, Query};
use crate::rdeps::RevDeps;
use crate::stats::{self, Event, Stats};
use crate::{Durability, Rev};

/// The recorded reads of a memo: each key with the revision observed when the query ran.
type Deps<I> = SmallVec<[(DepKey<I>, Rev); 4]>;

/// The longest `Debug` text a [`Mismatch`] or [`Cycle`] keeps of a key or value.
const DEBUG_LIMIT: usize = 1024;

/// The most dependencies a [`Mismatch`] lists.
const MISMATCH_DEPS: usize = 64;

// ---------------------------------------------------------------------------------------------------------
// Public reports

/// A dependency cycle between queries (ADR-0002 decision 4). Debug builds panic with this message instead
/// of returning it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Cycle {
    /// The reads from the outermost active query to the repeated one, as `family(key)`.
    pub path: Vec<String>,
}

impl fmt::Display for Cycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ew-memo: query cycle: {}", self.path.join(" -> "))
    }
}

impl std::error::Error for Cycle {}

/// Oracle mode: memoised reads re-run cold and compared (the "memoised equals cold" proof of WP-S1.05).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Oracle {
    /// No checks.
    #[default]
    Off,
    /// Every memoised read, at every depth, also runs its query cold.
    Every,
    /// One read in `n`, counted over all reads of the store (deterministic, no randomness).
    OneIn(NonZeroU32),
}

/// A memoised value that differed from its cold run, reported by oracle mode.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Mismatch {
    /// The query family.
    pub family: &'static str,
    /// The key, as `Debug` text (at most 1,024 bytes).
    pub key: String,
    /// The revision of the read.
    pub rev: Rev,
    /// The memoised value, as `Debug` text (at most 1,024 bytes).
    pub memoised: String,
    /// The cold value, as `Debug` text (at most 1,024 bytes), or the cycle the cold run hit.
    pub cold: String,
    /// The revision at which the memo was last confirmed valid (the read's revision, as the memo was just
    /// brought up to date).
    pub verified_at: Rev,
    /// The revision at which the memo's value last changed.
    pub changed_at: Rev,
    /// The memo's durability.
    pub durability: Durability,
    /// The memo's recorded reads in read order, as `input(<key>)@r<rev>` or `family(<key>)@r<rev>` with the
    /// revision observed (at most 64 entries, then a `…` entry).
    pub deps: Vec<String>,
}

/// The state of one memo, for tests and `--explain` output.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemoInfo<I> {
    /// The revision at which the memo was last confirmed valid.
    pub verified_at: Rev,
    /// The revision at which its value last changed (backdated on cut-off).
    pub changed_at: Rev,
    /// The minimum durability of its dependencies.
    pub durability: Durability,
    /// What it read, with the revisions observed, in read order.
    pub deps: Vec<(DepKey<I>, Rev)>,
    /// Whether the value is held (false after an LRU eviction).
    pub has_value: bool,
}

/// Memory of one query family (G-SP-04, NFR-PERF-13).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct FamilyMemory {
    /// The family name.
    pub family: &'static str,
    /// Keys with a memo.
    pub memos: usize,
    /// Memos that hold their value.
    pub values: usize,
    /// Bytes of the held values, as `Query::bytes` reports them.
    pub value_bytes: usize,
    /// Recorded dependency entries over all memos.
    pub dep_entries: usize,
    /// (key, memo) pairs of the reverse-dependency index (0 without `Query::RDEPS`).
    pub rdep_entries: usize,
}

// ---------------------------------------------------------------------------------------------------------
// Family storage

struct Memo<V, I> {
    /// `None` after an LRU eviction; the rest of the memo stays, so dependents can still be verified.
    value: Option<Arc<V>>,
    /// The fingerprint, for `Equivalence::Hash128` families.
    fingerprint: Option<u128>,
    deps: Deps<I>,
    verified_at: Rev,
    changed_at: Rev,
    durability: Durability,
    bytes: usize,
}

impl<V, I: Clone> Clone for Memo<V, I> {
    fn clone(&self) -> Self {
        Memo {
            value: self.value.clone(),
            fingerprint: self.fingerprint,
            deps: self.deps.clone(),
            verified_at: self.verified_at,
            changed_at: self.changed_at,
            durability: self.durability,
            bytes: self.bytes,
        }
    }
}

struct Slot<K, V, I> {
    key: K,
    memo: Option<Memo<V, I>>,
}

impl<K: Clone, V, I: Clone> Clone for Slot<K, V, I> {
    fn clone(&self) -> Self {
        Slot {
            key: self.key.clone(),
            memo: self.memo.clone(),
        }
    }
}

/// The memos of one family: keys interned to slots, slots in a persistent vector.
struct FamilyStore<K: Ord + Clone, V, I: Ord + Clone> {
    index: imbl::OrdMap<K, SlotId>,
    slots: imbl::Vector<Slot<K, V, I>>,
    rdeps: Option<RevDeps<I>>,
}

impl<K: Ord + Clone, V, I: Ord + Clone> Clone for FamilyStore<K, V, I> {
    fn clone(&self) -> Self {
        FamilyStore {
            index: self.index.clone(),
            slots: self.slots.clone(),
            rdeps: self.rdeps.clone(),
        }
    }
}

type AnyStorage = Box<dyn Any + Send + Sync>;

/// Brings a memo of some family up to date and returns its `changed_at` and durability.
type VerifyFn<D> = fn(&mut Store<D>, &D, SlotId) -> Result<(Rev, Durability), Cycle>;

/// A registered family: its storage and the monomorphised entry points the engine calls without knowing
/// the family's types.
struct Family<D: Db> {
    name: &'static str,
    storage: AnyStorage,
    /// Brings a memo up to date (re-executing if needed) and returns its `changed_at` and durability.
    verify: VerifyFn<D>,
    /// Drops a value (LRU eviction).
    evict: fn(&mut AnyStorage, SlotId),
    clone_storage: fn(&AnyStorage) -> AnyStorage,
    memory: fn(&AnyStorage, &'static str) -> FamilyMemory,
    /// `Debug` text of a slot's key, for cycle paths.
    describe: fn(&AnyStorage, SlotId) -> String,
}

impl<D: Db> Clone for Family<D> {
    fn clone(&self) -> Self {
        Family {
            name: self.name,
            storage: (self.clone_storage)(&self.storage),
            verify: self.verify,
            evict: self.evict,
            clone_storage: self.clone_storage,
            memory: self.memory,
            describe: self.describe,
        }
    }
}

/// One query execution in progress: what it has read so far.
struct Frame<I> {
    deps: Deps<I>,
    durability: Durability,
}

// ---------------------------------------------------------------------------------------------------------
// The store

/// The memo store of one host database type `D` (ADR-0002 decision 4, ARCH §6.2).
///
/// # Reading
///
/// [`Store::get`] answers `Q(key)` at the current revision:
///
/// 1. **Hit**: the memo was verified at the current revision.
/// 2. **Durability skip**: no input of the memo's durability or a more durable one changed since the memo
///    was verified (see [`Store::advance`]); the memo is re-stamped without looking at its dependencies.
/// 3. **Verification**: the recorded dependencies are checked in read order; an input compares its current
///    revision with the observed one, a memo dependency is first brought up to date itself (recursively,
///    re-executing it if needed) and compares its `changed_at`. If nothing changed, the memo is re-stamped.
///    The walk stops at the first changed dependency.
/// 4. **Re-execution**: otherwise (or for a new key, or for an evicted value) the query runs, recording its
///    reads. If the new value is equivalent to the previous one ([`Equivalence`]), `changed_at` keeps its old
///    revision (**early cut-off**: dependents stay green) and the previous `Arc` is kept (for both
///    equivalences, as long as the previous memo still holds its value), so pointer identity survives
///    for consumers such as GPU buffers. After an LRU eviction the re-computed value is a new `Arc`.
///
/// Reads inside a query ([`Cx::get`]) follow the same steps and record the memo as a dependency of the
/// caller.
///
/// # Snapshots
///
/// The store is built on persistent maps: [`Clone`] (or [`Store::fork`]) is O(1), and a fork evolves
/// independently, for jobs on a read-only snapshot. [`Store::adopt_if_unchanged`] takes a fork's memos back
/// when the store was not touched since the fork (ADR-0002 decision 5).
///
/// # Determinism
///
/// No hash map is iterated, no time or randomness is used; statistics, reports and reverse dependencies are
/// in a fixed order.
pub struct Store<D: Db> {
    current: Rev,
    /// Per durability level, the latest revision at which an input of that level or a more durable one
    /// changed.
    last_changed: [Rev; 3],
    families: Vec<Family<D>>,
    by_type: BTreeMap<TypeId, FamilyId>,
    /// Memos being executed or verified, outermost first (cycle detection).
    active: Vec<MemoRef>,
    /// Executions in progress, outermost first (dependency recording).
    frames: Vec<Frame<D::Input>>,
    lru: Lru,
    commit: Stats,
    total: Stats,
    oracle: Oracle,
    oracle_reads: u64,
    mismatches: Vec<Mismatch>,
    panic_on_cycle: bool,
    /// The identity of the store's current state (adopt-if-unchanged, ADR-0002 decision 5): replaced by
    /// every mutation that a fork taken before it would not see (an advance, a memo write, a new key, a
    /// budget change) whenever a fork may hold it. A fork records it as its `base_token`.
    token: Arc<()>,
    /// For a fork: the token of the store it was taken from, at the time it was taken.
    base_token: Option<Arc<()>>,
    /// For a fork: the revision of the store it was taken from, at the time it was taken.
    base_rev: Rev,
    /// For a fork: the LRU's next tick when it was taken from its parent (adopt re-touches what the parent
    /// touched since).
    lru_base_tick: u64,
}

/// A clone is an O(1) snapshot that evolves independently (see [`Store::fork`]). It starts with empty
/// statistics (`commit` and `total`: a job's statistics are its own work) and no oracle mismatches; the
/// oracle mode, the oracle read counter, the LRU (entries and budget) and the memos are copied.
impl<D: Db> Clone for Store<D> {
    fn clone(&self) -> Self {
        Store {
            current: self.current,
            last_changed: self.last_changed,
            families: self.families.clone(),
            by_type: self.by_type.clone(),
            active: Vec::new(),
            frames: Vec::new(),
            lru: self.lru.clone(),
            commit: Stats::default(),
            total: Stats::default(),
            oracle: self.oracle,
            oracle_reads: self.oracle_reads,
            mismatches: Vec::new(),
            panic_on_cycle: self.panic_on_cycle,
            // A fork is a different store: its own identity, and its parent's as the base.
            token: Arc::new(()),
            base_token: Some(Arc::clone(&self.token)),
            base_rev: self.current,
            lru_base_tick: self.lru.next_tick(),
        }
    }
}

impl<D: Db> Debug for Store<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store")
            .field("current", &self.current)
            .field(
                "families",
                &self.families.iter().map(|x| x.name).collect::<Vec<_>>(),
            )
            .field("lru_values", &self.lru.len())
            .field("lru_bytes", &self.lru.total_bytes())
            .field("oracle", &self.oracle)
            .finish_non_exhaustive()
    }
}

impl<D: Db> Default for Store<D> {
    fn default() -> Self {
        Store::new()
    }
}

impl<D: Db> Store<D> {
    /// An empty store at [`Rev::ZERO`], without LRU budget and with oracle mode off.
    pub fn new() -> Store<D> {
        Store {
            current: Rev::ZERO,
            last_changed: [Rev::ZERO; 3],
            families: Vec::new(),
            by_type: BTreeMap::new(),
            active: Vec::new(),
            frames: Vec::new(),
            lru: Lru::default(),
            commit: Stats::default(),
            total: Stats::default(),
            oracle: Oracle::Off,
            oracle_reads: 0,
            mismatches: Vec::new(),
            panic_on_cycle: cfg!(debug_assertions),
            token: Arc::new(()),
            base_token: None,
            base_rev: Rev::ZERO,
            lru_base_tick: 0,
        }
    }

    /// The current revision.
    pub fn current(&self) -> Rev {
        self.current
    }

    /// Moves the store to revision `rev` after a commit whose most durable changed input has durability
    /// `changed`.
    ///
    /// The durability rule: the store keeps, per level, the latest revision at which an input of that level
    /// *or a more durable one* changed; advancing with `changed` sets it for `changed` and every less durable
    /// level. A memo of durability `d` (the minimum over its dependencies) whose verification revision is not
    /// older than that revision of level `d` cannot have a changed dependency, so it is re-stamped without a
    /// dependency walk. Example: a symbol's geometry, which reads library copies only, stays verified while
    /// the user draws.
    ///
    /// `changed` must cover the durability of every changed input both before and after the change.
    ///
    /// The statistics of the closing commit are emitted (`tracing`, target `ew_memo::stats`, window
    /// `commit`) and reset.
    ///
    /// # Panics
    ///
    /// If `rev` is not newer than the current revision.
    pub fn advance(&mut self, rev: Rev, changed: Durability) {
        assert!(
            rev > self.current,
            "ew-memo: advance to {rev} from {}",
            self.current
        );
        // `&mut self` is held, so no query is executing: leftover state is an evaluation that a panic
        // abandoned (and the host caught).
        self.active.clear();
        self.frames.clear();
        stats::emit(self.current, "commit", &self.commit);
        self.commit.clear();
        for level in &mut self.last_changed[..=changed.index()] {
            *level = rev;
        }
        self.current = rev;
        self.mutated();
    }

    /// Reads `Q(key)` at the current revision (see the type documentation).
    ///
    /// # Errors
    ///
    /// [`Cycle`] when the evaluation reads a query that is already being evaluated (release builds; debug
    /// builds panic).
    pub fn get<Q: Query<D>>(&mut self, db: &D, key: &Q::Key) -> Result<Arc<Q::Value>, Cycle> {
        if !self.active.is_empty() || !self.frames.is_empty() {
            // Left over by a panic inside a query: the evaluation was abandoned.
            self.active.clear();
            self.frames.clear();
        }
        self.fetch::<Q>(db, key).map(|(value, _, _)| value)
    }

    /// Computes `Q(key)` cold: no memo is read or written, nested queries are computed cold too (each at
    /// most once per call). The reference for oracle mode and `ew verify-derived`.
    ///
    /// # Errors
    ///
    /// [`Cycle`] as for [`Store::get`].
    pub fn cold<Q: Query<D>>(&self, db: &D, key: &Q::Key) -> Result<Arc<Q::Value>, Cycle> {
        let mut cold = Cold {
            panic_on_cycle: self.panic_on_cycle,
            ..Cold::default()
        };
        cold_fetch::<D, Q>(&mut cold, db, key)
    }

    /// Sets oracle mode.
    pub fn set_oracle(&mut self, oracle: Oracle) {
        self.oracle = oracle;
    }

    /// The mismatches found by oracle mode since the last call, in the order found.
    pub fn take_mismatches(&mut self) -> Vec<Mismatch> {
        std::mem::take(&mut self.mismatches)
    }

    /// Sets the byte budget of the LRU over the values of `Query::LRU` families (`None`: unbounded).
    /// Values over budget are evicted at the next memo write.
    pub fn set_lru_budget(&mut self, bytes: Option<usize>) {
        self.lru.set_budget(bytes);
        self.mutated();
    }

    /// The LRU budget in bytes (`None`: unbounded).
    pub fn lru_budget(&self) -> Option<usize> {
        self.lru.budget()
    }

    /// The bytes of the values currently under the LRU.
    pub fn lru_bytes(&self) -> usize {
        self.lru.total_bytes()
    }

    /// Statistics since the last [`Store::advance`] (one commit).
    pub fn commit_stats(&self) -> &Stats {
        &self.commit
    }

    /// Statistics over the store's lifetime.
    pub fn total_stats(&self) -> &Stats {
        &self.total
    }

    /// Emits the lifetime statistics as `tracing` events (target `ew_memo::stats`, window `total`).
    pub fn emit_stats(&self) {
        stats::emit(self.current, "total", &self.total);
    }

    /// The dependency key naming the memo of `Q(key)`, if the key was ever read.
    pub fn memo_key<Q: Query<D>>(&self, key: &Q::Key) -> Option<DepKey<D::Input>> {
        let fid = *self.by_type.get(&TypeId::of::<Q>())?;
        let slot = *self.storage::<Q>(fid).index.get(key)?;
        Some(DepKey::Memo(fid, slot))
    }

    /// The reverse-dependency API (ADR-0002 amendment 1): the keys of family `Q` whose memo's recorded reads
    /// include `dep` (an input, or a memo named by [`Store::memo_key`]), in key order. Indexed for
    /// `Query::RDEPS` families, a scan otherwise.
    pub fn dependents<Q: Query<D>>(&self, dep: &DepKey<D::Input>) -> Vec<Q::Key> {
        let Some(&fid) = self.by_type.get(&TypeId::of::<Q>()) else {
            return Vec::new();
        };
        let s = self.storage::<Q>(fid);
        let mut keys: Vec<Q::Key> = match &s.rdeps {
            Some(index) => index
                .get(dep)
                .into_iter()
                .map(|slot| s.slots[slot.index()].key.clone())
                .collect(),
            None => s
                .slots
                .iter()
                .filter(|slot| {
                    slot.memo
                        .as_ref()
                        .is_some_and(|m| m.deps.iter().any(|(k, _)| k == dep))
                })
                .map(|slot| slot.key.clone())
                .collect(),
        };
        keys.sort();
        keys
    }

    /// The state of the memo of `Q(key)`, if any.
    pub fn memo_info<Q: Query<D>>(&self, key: &Q::Key) -> Option<MemoInfo<D::Input>> {
        let fid = *self.by_type.get(&TypeId::of::<Q>())?;
        let s = self.storage::<Q>(fid);
        let slot = *s.index.get(key)?;
        let m = s.slots[slot.index()].memo.as_ref()?;
        Some(MemoInfo {
            verified_at: m.verified_at,
            changed_at: m.changed_at,
            durability: m.durability,
            deps: m.deps.to_vec(),
            has_value: m.value.is_some(),
        })
    }

    /// Memory per family, in family-name order.
    pub fn memory(&self) -> Vec<FamilyMemory> {
        let mut out: Vec<FamilyMemory> = self
            .families
            .iter()
            .map(|f| (f.memory)(&f.storage, f.name))
            .collect();
        out.sort_by_key(|m| m.family);
        out
    }

    /// An O(1) snapshot that evolves independently (a job's memo store). The fork starts with empty
    /// statistics and no oracle mismatches (its statistics and findings are its own work; adopting it
    /// merges them into this store's); the oracle mode, the LRU and the memos are copied.
    pub fn fork(&self) -> Store<D> {
        self.clone()
    }

    /// Takes `fork`'s memos when `fork` was taken from this very store and neither side moved on in a way
    /// the other cannot see: this store is still pointer-equal to the fork's base (no memo written, no key
    /// added, no budget changed and no revision advanced since [`Store::fork`]; reads that only hit are
    /// fine), and the fork is still at that base revision (adopt-if-unchanged, ADR-0002 decision 5). A fork
    /// of a fork, a sibling fork taken after this store changed, or a fork that advanced is refused. Returns
    /// whether it adopted.
    ///
    /// Adopting takes the fork's memos (families and their LRU entries) and keeps this store's
    /// configuration: oracle mode, cycle behaviour, LRU budget and oracle read counter. The fork's
    /// statistics are merged into this store's, its oracle mismatches follow this store's, and the LRU
    /// recency of what this store hit since the fork is kept (those entries stay the most recent). Values
    /// over this store's budget are evicted.
    pub fn adopt_if_unchanged(&mut self, fork: Store<D>) -> bool {
        let same_base = fork
            .base_token
            .as_ref()
            .is_some_and(|base| Arc::ptr_eq(base, &self.token));
        if !same_base
            || self.current != fork.base_rev
            || fork.current != fork.base_rev
            || !self.active.is_empty()
        {
            return false;
        }
        let mut fork = fork;
        let parent_touched = self.lru.touched_since(fork.lru_base_tick);
        let budget = self.lru.budget();
        self.families = fork.families;
        self.by_type = fork.by_type;
        self.lru = fork.lru;
        for (memo, bytes) in parent_touched {
            if self.lru.contains(memo) {
                self.lru.touch(memo, bytes);
            }
        }
        self.lru.set_budget(budget);
        self.commit.merge(&fork.commit);
        self.total.merge(&fork.total);
        self.mismatches.append(&mut fork.mismatches);
        self.evict_over_budget();
        // The adopted state is a new state of this store: no earlier fork of either side has it as its base.
        self.token = Arc::new(());
        self.base_token = None;
        true
    }

    // -----------------------------------------------------------------------------------------------------
    // Families

    fn family<Q: Query<D>>(&mut self) -> FamilyId {
        if let Some(&fid) = self.by_type.get(&TypeId::of::<Q>()) {
            return fid;
        }
        let fid = FamilyId(u32::try_from(self.families.len()).expect("fewer than 2^32 families"));
        let storage: FamilyStore<Q::Key, Q::Value, D::Input> = FamilyStore {
            index: imbl::OrdMap::new(),
            slots: imbl::Vector::new(),
            rdeps: Q::RDEPS.then(RevDeps::new),
        };
        self.families.push(Family {
            name: Q::NAME,
            storage: Box::new(storage),
            verify: verify_erased::<D, Q>,
            evict: evict_erased::<Q::Key, Q::Value, D::Input>,
            clone_storage: clone_erased::<Q::Key, Q::Value, D::Input>,
            memory: memory_erased::<Q::Key, Q::Value, D::Input>,
            describe: describe_erased::<Q::Key, Q::Value, D::Input>,
        });
        self.by_type.insert(TypeId::of::<Q>(), fid);
        fid
    }

    fn storage<Q: Query<D>>(&self, fid: FamilyId) -> &FamilyStore<Q::Key, Q::Value, D::Input> {
        self.families[fid.0 as usize]
            .storage
            .downcast_ref()
            .expect("family storage of its own type")
    }

    fn storage_mut<Q: Query<D>>(&mut self, fid: FamilyId) -> &mut FamilyStore<Q::Key, Q::Value, D::Input> {
        self.families[fid.0 as usize]
            .storage
            .downcast_mut()
            .expect("family storage of its own type")
    }

    fn intern<Q: Query<D>>(&mut self, fid: FamilyId, key: &Q::Key) -> SlotId {
        if let Some(&slot) = self.storage::<Q>(fid).index.get(key) {
            return slot;
        }
        let s = self.storage_mut::<Q>(fid);
        let slot = SlotId(u32::try_from(s.slots.len()).expect("fewer than 2^32 keys per family"));
        s.slots.push_back(Slot {
            key: key.clone(),
            memo: None,
        });
        s.index.insert(key.clone(), slot);
        self.mutated();
        slot
    }

    /// Marks a mutation that a fork taken before it does not see. A fork may hold the current token as its
    /// base, so the token is replaced; only then, since a token that nobody else holds can name the new state
    /// as well (no fork can be compared with it).
    fn mutated(&mut self) {
        if Arc::strong_count(&self.token) > 1 {
            self.token = Arc::new(());
        }
    }

    /// Drops the values the LRU evicts while over budget, recording the evictions.
    fn evict_over_budget(&mut self) {
        for victim in self.lru.evict_over_budget() {
            let family = &mut self.families[victim.0.0 as usize];
            (family.evict)(&mut family.storage, victim.1);
            let name = family.name;
            self.record(name, Event::Evicted);
        }
    }

    fn record(&mut self, family: &'static str, event: Event) {
        self.commit.record(family, event);
        self.total.record(family, event);
    }

    // -----------------------------------------------------------------------------------------------------
    // Reading

    /// Reads `Q(key)` and records it as a dependency of the executing caller, if any.
    fn fetch<Q: Query<D>>(
        &mut self,
        db: &D,
        key: &Q::Key,
    ) -> Result<(Arc<Q::Value>, Rev, Durability), Cycle> {
        let fid = self.family::<Q>();
        let slot = self.intern::<Q>(fid, key);
        let (value, changed_at, durability) = self.up_to_date::<Q>(db, fid, slot, true)?;
        let value = value.expect("a value was requested");
        if let Some(frame) = self.frames.last_mut() {
            push_dep(frame, DepKey::Memo(fid, slot), changed_at, durability);
        }
        self.oracle_check::<Q>(db, key, &value, fid, slot);
        Ok((value, changed_at, durability))
    }

    /// Brings the memo of `slot` to the current revision: hit, durability skip, verification or
    /// re-execution. With `need_value`, an evicted value is re-computed.
    #[allow(clippy::type_complexity)]
    fn up_to_date<Q: Query<D>>(
        &mut self,
        db: &D,
        fid: FamilyId,
        slot: SlotId,
        need_value: bool,
    ) -> Result<(Option<Arc<Q::Value>>, Rev, Durability), Cycle> {
        let me = (fid, slot);
        if self.active.contains(&me) {
            return Err(self.cycle(me));
        }
        let current = self.current;
        let memo = self.storage::<Q>(fid).slots[slot.index()].memo.clone();
        let Some(memo) = memo else {
            return self.execute::<Q>(db, fid, slot, None, false);
        };
        if memo.verified_at == current {
            if memo.value.is_some() || !need_value {
                self.record(Q::NAME, Event::Hit);
                self.touch_lru::<Q>(fid, slot, &memo);
                return Ok((memo.value, memo.changed_at, memo.durability));
            }
            // Verified, but the value was evicted: the inputs are the same, so is the value.
            return self.execute::<Q>(db, fid, slot, Some(memo), true);
        }
        let green = if self.last_changed[memo.durability.index()] <= memo.verified_at {
            // Nothing of the memo's durability or a more durable one changed since it was verified, so no
            // dependency changed and none changed its durability (a move between levels is a change of the
            // more durable one): the memo's durability stands.
            self.record(Q::NAME, Event::DurabilitySkip);
            Some(memo.durability)
        } else {
            self.active.push(me);
            let walk = self.deps_unchanged(db, &memo.deps);
            self.active.pop();
            let green = walk?;
            if green.is_some() {
                self.record(Q::NAME, Event::Verified);
            }
            green
        };
        if let Some(durability) = green {
            self.stamp::<Q>(fid, slot, durability);
            if memo.value.is_some() || !need_value {
                self.touch_lru::<Q>(fid, slot, &memo);
                return Ok((memo.value, memo.changed_at, durability));
            }
            return self.execute::<Q>(db, fid, slot, Some(memo), true);
        }
        self.execute::<Q>(db, fid, slot, Some(memo), false)
    }

    /// Whether no recorded dependency changed since it was observed: `Some` with the minimum of the
    /// dependencies' *current* durabilities (an input's as the host reports it now, a memo's as its own
    /// verification left it; [`Durability::Library`] without dependencies), `None` at the first change.
    /// Memo dependencies are brought up to date first; the walk stops at the first change.
    ///
    /// The durability is recomputed because a dependency can keep its revision while its durability drops:
    /// a memo dependency that re-executed with a different read set and cut off, or an input that moved to a
    /// less durable level while the memo it feeds cut off. Keeping the old durability would let a later,
    /// less durable change skip the memo's verification and return a stale value.
    fn deps_unchanged(&mut self, db: &D, deps: &Deps<D::Input>) -> Result<Option<Durability>, Cycle> {
        let mut durability = Durability::Library;
        for (dep, observed) in deps {
            let (now, level) = match dep {
                DepKey::Input(input) => (db.changed_at(input), db.durability(input)),
                DepKey::Memo(fid, slot) => {
                    let verify = self.families[fid.0 as usize].verify;
                    verify(self, db, *slot)?
                }
            };
            if now != *observed {
                return Ok(None);
            }
            durability = durability.min(level);
        }
        Ok(Some(durability))
    }

    /// Re-stamps a verified memo at the current revision with the durability its verification found.
    fn stamp<Q: Query<D>>(&mut self, fid: FamilyId, slot: SlotId, durability: Durability) {
        let current = self.current;
        let s = self.storage_mut::<Q>(fid);
        if let Some(slot) = s.slots.get_mut(slot.index()) {
            if let Some(m) = slot.memo.as_mut() {
                m.verified_at = current;
                m.durability = durability;
            }
        }
        self.mutated();
    }

    fn touch_lru<Q: Query<D>>(&mut self, fid: FamilyId, slot: SlotId, memo: &Memo<Q::Value, D::Input>) {
        if Q::LRU && memo.value.is_some() {
            self.lru.touch((fid, slot), memo.bytes);
        }
    }

    /// Runs the query and stores the new memo. `previous` is the old memo; `known_same` says the inputs are
    /// unchanged (an evicted value being re-computed), so the value is equivalent by purity.
    #[allow(clippy::type_complexity)]
    fn execute<Q: Query<D>>(
        &mut self,
        db: &D,
        fid: FamilyId,
        slot: SlotId,
        previous: Option<Memo<Q::Value, D::Input>>,
        known_same: bool,
    ) -> Result<(Option<Arc<Q::Value>>, Rev, Durability), Cycle> {
        let key = self.storage::<Q>(fid).slots[slot.index()].key.clone();
        self.active.push((fid, slot));
        self.frames.push(Frame {
            deps: Deps::new(),
            durability: Durability::Library,
        });
        let result = {
            let mut cx = Cx {
                db,
                inner: Inner::Memo(self),
            };
            Q::execute(&mut cx, &key)
        };
        let frame = self.frames.pop().expect("own frame");
        self.active.pop();
        let value = result?;
        self.record(Q::NAME, Event::ReExecuted);

        let current = self.current;
        let fingerprint = match Q::equivalence() {
            Equivalence::Hash128(hash) => Some(hash(&value)),
            Equivalence::Eq(_) => None,
        };
        // Early cut-off: an equivalent value keeps its `changed_at` (and the previous `Arc`, when it is held).
        let (value, changed_at) = match previous.as_ref() {
            Some(prev) => {
                let same = match (Q::equivalence(), prev.value.as_ref()) {
                    (Equivalence::Eq(eq), Some(old)) => eq(old, &value),
                    (Equivalence::Eq(_), None) => known_same,
                    (Equivalence::Hash128(_), _) => prev.fingerprint == fingerprint,
                };
                if same {
                    self.record(Q::NAME, Event::CutOff);
                    // The old value is equivalent by definition of the cut-off: keep its identity (both
                    // equivalences). Without it (evicted), the re-computed value is the only one.
                    let arc = match &prev.value {
                        Some(old) => Arc::clone(old),
                        None => Arc::new(value),
                    };
                    (arc, prev.changed_at)
                } else {
                    (Arc::new(value), current)
                }
            }
            None => (Arc::new(value), current),
        };
        let bytes = Q::bytes(&value);
        let durability = frame.durability;
        let memo = Memo {
            value: Some(Arc::clone(&value)),
            fingerprint,
            deps: frame.deps,
            verified_at: current,
            changed_at,
            durability,
            bytes,
        };
        {
            let s = self.storage_mut::<Q>(fid);
            if let Some(index) = s.rdeps.as_mut() {
                if let Some(prev) = previous.as_ref() {
                    index.remove(slot, &prev.deps);
                }
                index.add(slot, &memo.deps);
            }
            s.slots.get_mut(slot.index()).expect("interned slot").memo = Some(memo);
        }
        self.mutated();
        if Q::LRU {
            self.lru.touch((fid, slot), bytes);
            self.evict_over_budget();
        }
        Ok((Some(value), changed_at, durability))
    }

    fn cycle(&mut self, repeated: MemoRef) -> Cycle {
        let start = self.active.iter().position(|m| *m == repeated).unwrap_or(0);
        let mut path: Vec<String> = self.active[start..]
            .iter()
            .map(|&(fid, slot)| self.describe(fid, slot))
            .collect();
        path.push(self.describe(repeated.0, repeated.1));
        let name = self.families[repeated.0.0 as usize].name;
        self.record(name, Event::Cycle);
        let cycle = Cycle { path };
        if self.panic_on_cycle {
            panic!("{cycle}");
        }
        cycle
    }

    fn describe(&self, fid: FamilyId, slot: SlotId) -> String {
        let f = &self.families[fid.0 as usize];
        format!("{}({})", f.name, (f.describe)(&f.storage, slot))
    }

    // -----------------------------------------------------------------------------------------------------
    // Oracle

    fn oracle_check<Q: Query<D>>(
        &mut self,
        db: &D,
        key: &Q::Key,
        memoised: &Arc<Q::Value>,
        fid: FamilyId,
        slot: SlotId,
    ) {
        let selected = match self.oracle {
            Oracle::Off => false,
            Oracle::Every => true,
            Oracle::OneIn(n) => {
                self.oracle_reads += 1;
                self.oracle_reads % u64::from(n.get()) == 0
            }
        };
        if !selected {
            return;
        }
        self.record(Q::NAME, Event::OracleCheck);
        let cold = self.cold::<Q>(db, key);
        let equal = match &cold {
            Ok(cold) => match Q::equivalence() {
                Equivalence::Eq(eq) => eq(memoised, cold),
                Equivalence::Hash128(hash) => hash(memoised) == hash(cold),
            },
            Err(_) => false,
        };
        if !equal {
            self.record(Q::NAME, Event::OracleMismatch);
            let (verified_at, changed_at, durability, deps) = self.memo_trail::<Q>(fid, slot);
            self.mismatches.push(Mismatch {
                family: Q::NAME,
                key: bounded(format!("{key:?}")),
                rev: self.current,
                memoised: bounded(format!("{memoised:?}")),
                cold: match cold {
                    Ok(v) => bounded(format!("{v:?}")),
                    Err(c) => bounded(c.to_string()),
                },
                verified_at,
                changed_at,
                durability,
                deps,
            });
        }
    }

    /// The state and the rendered dependency trail of a memo, for a [`Mismatch`].
    fn memo_trail<Q: Query<D>>(&self, fid: FamilyId, slot: SlotId) -> (Rev, Rev, Durability, Vec<String>) {
        let Some(m) = self.storage::<Q>(fid).slots[slot.index()].memo.as_ref() else {
            return (Rev::ZERO, Rev::ZERO, Durability::Library, Vec::new());
        };
        let mut deps: Vec<String> = m
            .deps
            .iter()
            .take(MISMATCH_DEPS)
            .map(|(dep, rev)| match dep {
                DepKey::Input(input) => bounded(format!("input({input:?})@{rev}")),
                DepKey::Memo(f, s) => format!("{}@{rev}", self.describe(*f, *s)),
            })
            .collect();
        if m.deps.len() > MISMATCH_DEPS {
            deps.push("…".to_owned());
        }
        (m.verified_at, m.changed_at, m.durability, deps)
    }

    #[cfg(test)]
    pub(crate) fn set_panic_on_cycle(&mut self, panic: bool) {
        self.panic_on_cycle = panic;
    }
}

fn push_dep<I: PartialEq>(frame: &mut Frame<I>, key: DepKey<I>, rev: Rev, durability: Durability) {
    frame.durability = frame.durability.min(durability);
    if frame.deps.last().is_some_and(|(k, r)| *k == key && *r == rev) {
        return;
    }
    frame.deps.push((key, rev));
}

/// Cuts `Debug` text to [`DEBUG_LIMIT`] bytes on a character boundary.
fn bounded(mut text: String) -> String {
    if text.len() > DEBUG_LIMIT {
        let mut end = DEBUG_LIMIT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push('…');
    }
    text
}

// ---------------------------------------------------------------------------------------------------------
// Erased entry points

fn verify_erased<D: Db, Q: Query<D>>(
    store: &mut Store<D>,
    db: &D,
    slot: SlotId,
) -> Result<(Rev, Durability), Cycle> {
    let fid = store.family::<Q>();
    store
        .up_to_date::<Q>(db, fid, slot, false)
        .map(|(_, changed_at, durability)| (changed_at, durability))
}

fn evict_erased<K, V, I>(storage: &mut AnyStorage, slot: SlotId)
where
    K: Ord + Clone + Send + Sync + 'static,
    V: Send + Sync + 'static,
    I: Ord + Clone + Send + Sync + 'static,
{
    let s: &mut FamilyStore<K, V, I> = (**storage)
        .downcast_mut()
        .expect("family storage of its own type");
    if let Some(m) = s.slots.get_mut(slot.index()).and_then(|x| x.memo.as_mut()) {
        m.value = None;
    }
}

fn clone_erased<K, V, I>(storage: &AnyStorage) -> AnyStorage
where
    K: Ord + Clone + Send + Sync + 'static,
    V: Send + Sync + 'static,
    I: Ord + Clone + Send + Sync + 'static,
{
    let s: &FamilyStore<K, V, I> = (**storage)
        .downcast_ref()
        .expect("family storage of its own type");
    Box::new(s.clone())
}

fn memory_erased<K, V, I>(storage: &AnyStorage, family: &'static str) -> FamilyMemory
where
    K: Ord + Clone + Send + Sync + 'static,
    V: Send + Sync + 'static,
    I: Ord + Clone + Send + Sync + 'static,
{
    let s: &FamilyStore<K, V, I> = (**storage)
        .downcast_ref()
        .expect("family storage of its own type");
    let mut out = FamilyMemory {
        family,
        rdep_entries: s.rdeps.as_ref().map_or(0, RevDeps::entries),
        ..FamilyMemory::default()
    };
    for m in s.slots.iter().filter_map(|x| x.memo.as_ref()) {
        out.memos += 1;
        out.dep_entries += m.deps.len();
        if m.value.is_some() {
            out.values += 1;
            out.value_bytes += m.bytes;
        }
    }
    out
}

fn describe_erased<K, V, I>(storage: &AnyStorage, slot: SlotId) -> String
where
    K: Ord + Clone + Debug + Send + Sync + 'static,
    V: Send + Sync + 'static,
    I: Ord + Clone + Send + Sync + 'static,
{
    let s: &FamilyStore<K, V, I> = (**storage)
        .downcast_ref()
        .expect("family storage of its own type");
    bounded(format!("{:?}", s.slots[slot.index()].key))
}

// ---------------------------------------------------------------------------------------------------------
// The query context

enum Inner<'a, D: Db> {
    Memo(&'a mut Store<D>),
    Cold(&'a mut Cold),
}

/// The context a query executes in: the host's database and the tracked reads.
///
/// In a memoised evaluation, [`Cx::read`] and [`Cx::get`] record dependencies; in a cold one (oracle mode,
/// [`Store::cold`]) nothing is recorded and nested queries are computed cold.
pub struct Cx<'a, D: Db> {
    db: &'a D,
    inner: Inner<'a, D>,
}

impl<D: Db> Debug for Cx<'_, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cx")
            .field("cold", &self.is_cold())
            .finish_non_exhaustive()
    }
}

impl<'a, D: Db> Cx<'a, D> {
    /// The host's database.
    pub fn db(&self) -> &'a D {
        self.db
    }

    /// Records a read of `input`. The host's tracked accessors call this for every input they read.
    pub fn read(&mut self, input: &D::Input) {
        if let Inner::Memo(store) = &mut self.inner {
            let rev = self.db.changed_at(input);
            let durability = self.db.durability(input);
            if let Some(frame) = store.frames.last_mut() {
                push_dep(frame, DepKey::Input(input.clone()), rev, durability);
            }
        }
    }

    /// Reads another query.
    ///
    /// # Errors
    ///
    /// [`Cycle`] when `Q(key)` is already being evaluated (release builds; debug builds panic).
    pub fn get<Q: Query<D>>(&mut self, key: &Q::Key) -> Result<Arc<Q::Value>, Cycle> {
        match &mut self.inner {
            Inner::Memo(store) => store.fetch::<Q>(self.db, key).map(|(value, _, _)| value),
            Inner::Cold(cold) => cold_fetch::<D, Q>(cold, self.db, key),
        }
    }

    /// Whether this is a cold evaluation.
    pub fn is_cold(&self) -> bool {
        matches!(self.inner, Inner::Cold(_))
    }
}

// ---------------------------------------------------------------------------------------------------------
// Cold evaluation

/// The state of one cold evaluation: the values computed so far and the active reads, per family type.
#[derive(Default)]
struct Cold {
    families: BTreeMap<TypeId, Box<dyn Any>>,
    /// Active reads, outermost first, as `family(key)` text (cycle paths).
    path: Vec<String>,
    panic_on_cycle: bool,
}

struct ColdFamily<K, V> {
    done: BTreeMap<K, Arc<V>>,
    active: BTreeSet<K>,
}

fn cold_family<D: Db, Q: Query<D>>(cold: &mut Cold) -> &mut ColdFamily<Q::Key, Q::Value> {
    cold.families
        .entry(TypeId::of::<Q>())
        .or_insert_with(|| {
            Box::new(ColdFamily::<Q::Key, Q::Value> {
                done: BTreeMap::new(),
                active: BTreeSet::new(),
            })
        })
        .downcast_mut()
        .expect("cold family of its own type")
}

fn cold_fetch<D: Db, Q: Query<D>>(cold: &mut Cold, db: &D, key: &Q::Key) -> Result<Arc<Q::Value>, Cycle> {
    let fam = cold_family::<D, Q>(cold);
    if let Some(v) = fam.done.get(key) {
        return Ok(Arc::clone(v));
    }
    let label = bounded(format!("{}({key:?})", Q::NAME));
    if !fam.active.insert(key.clone()) {
        let start = cold.path.iter().position(|p| *p == label).unwrap_or(0);
        let mut path = cold.path[start..].to_vec();
        path.push(label);
        let cycle = Cycle { path };
        if cold.panic_on_cycle {
            panic!("{cycle}");
        }
        return Err(cycle);
    }
    cold.path.push(label);
    let result = {
        let mut cx = Cx {
            db,
            inner: Inner::Cold(cold),
        };
        Q::execute(&mut cx, key)
    };
    cold.path.pop();
    let fam = cold_family::<D, Q>(cold);
    fam.active.remove(key);
    let value = Arc::new(result?);
    fam.done.insert(key.clone(), Arc::clone(&value));
    Ok(value)
}
