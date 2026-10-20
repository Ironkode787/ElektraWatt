// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The memo store: red-green verification with early cut-off, durability, push invalidation, part
//! dependencies, the incremental re-execution hook, the LRU, statistics, reverse dependencies, cycle
//! detection and oracle mode.
//!
//! The store is re-entrant: every read takes `&self`, and no internal borrow is held while host code runs
//! (a query's `execute`, the database's `changed_at`, `durability` and `coarse`). A host whose query bodies
//! call back into the store through a shared handle (rather than through [`Cx`]) nests reads safely; the
//! store attributes them to the innermost execution.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Debug};
use std::num::NonZeroU32;
use std::sync::Arc;

use imbl::{OrdMap, OrdSet};

use crate::key::{DepKey, FamilyId, MemoRef, SlotId};
use crate::lru::Lru;
use crate::query::{Changes, Db, Equivalence, Query};
use crate::rdeps::RevDeps;
use crate::stats::{self, Event, Stats};
use crate::{Durability, Rev};

/// The longest `Debug` text a [`Mismatch`] or [`Cycle`] keeps of a key or value.
const DEBUG_LIMIT: usize = 1024;

/// The most dependencies a [`Mismatch`] lists.
const MISMATCH_DEPS: usize = 64;

/// The most revisions of touched inputs kept for incremental re-executions ([`Changes::touched`]).
const LOG_LIMIT: usize = 1024;

/// The most `via` keys collected per marked memo ([`Changes::via`]); beyond, the hook gets `None`.
const VIA_LIMIT: usize = 4096;

// ---------------------------------------------------------------------------------------------------------
// Public reports

/// A dependency cycle between queries (ADR-0002 decision 4). Debug builds panic with this message instead
/// of returning it, unless the host turned that off ([`Store::set_cycle_panics`]).
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
    /// The memo's recorded reads in read order, as `input(<key>)@r<rev>`, `family(<key>)@r<rev>` or
    /// `family(<key>)[<part>]@r<rev>` with the revision observed (at most 64 entries, then a `…` entry).
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
    /// What it read, with the revisions observed, in read order (key order for an incremental family's
    /// carried dependencies).
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
// Memos

/// The recorded reads of a memo: each key with the revision observed when the query ran.
enum Deps<I: Ord + Clone> {
    /// In read order (consecutive duplicates collapsed).
    List(Arc<[(DepKey<I>, Rev)]>),
    /// By key: the dependencies of an incremental family, carried from one execution to the next (O(1)
    /// clone, O(log n) replacement).
    Map(OrdMap<DepKey<I>, Rev>),
}

impl<I: Ord + Clone> Clone for Deps<I> {
    fn clone(&self) -> Self {
        match self {
            Deps::List(l) => Deps::List(Arc::clone(l)),
            Deps::Map(m) => Deps::Map(m.clone()),
        }
    }
}

impl<I: Ord + Clone> Deps<I> {
    fn len(&self) -> usize {
        match self {
            Deps::List(l) => l.len(),
            Deps::Map(m) => m.len(),
        }
    }

    fn iter(&self) -> Box<dyn Iterator<Item = (&DepKey<I>, Rev)> + '_> {
        match self {
            Deps::List(l) => Box::new(l.iter().map(|(k, r)| (k, *r))),
            Deps::Map(m) => Box::new(m.iter().map(|(k, r)| (k, *r))),
        }
    }

    fn contains(&self, key: &DepKey<I>) -> bool {
        match self {
            Deps::List(l) => l.iter().any(|(k, _)| k == key),
            Deps::Map(m) => m.contains_key(key),
        }
    }

    fn to_vec(&self) -> Vec<(DepKey<I>, Rev)> {
        self.iter().map(|(k, r)| (k.clone(), r)).collect()
    }
}

struct Memo<V, I: Ord + Clone> {
    /// `None` after an LRU eviction; the rest of the memo stays, so dependents can still be verified.
    value: Option<Arc<V>>,
    /// The fingerprint, for `Equivalence::Hash128` families.
    fingerprint: Option<u128>,
    deps: Deps<I>,
    /// The producers (memos whose parts this memo reads, directly or through its memo dependencies) that
    /// must be up to date before push invalidation may call this memo clean. Kept current by every
    /// verification that walks the dependencies.
    producers: Arc<[MemoRef]>,
    /// The keys this memo is registered under in the push index, sorted: the coarse key of every input read
    /// in the snapshot it was read from, the memos and the parts. Kept with the memo because a coarse key
    /// computed again in a later snapshot can differ (a row moved to another page), which would leave the
    /// old registration behind (review cycle 9, finding 5).
    reg: Arc<[DepKey<I>]>,
    verified_at: Rev,
    changed_at: Rev,
    durability: Durability,
    bytes: usize,
}

impl<V, I: Ord + Clone> Clone for Memo<V, I> {
    fn clone(&self) -> Self {
        Memo {
            value: self.value.clone(),
            fingerprint: self.fingerprint,
            deps: self.deps.clone(),
            producers: Arc::clone(&self.producers),
            reg: Arc::clone(&self.reg),
            verified_at: self.verified_at,
            changed_at: self.changed_at,
            durability: self.durability,
            bytes: self.bytes,
        }
    }
}

struct Slot<K, V, I: Ord + Clone> {
    key: K,
    memo: Option<Memo<V, I>>,
}

impl<K: Clone, V, I: Ord + Clone> Clone for Slot<K, V, I> {
    fn clone(&self) -> Self {
        Slot {
            key: self.key.clone(),
            memo: self.memo.clone(),
        }
    }
}

/// The memos of one family: keys interned to slots, slots in a persistent vector.
struct FamilyStore<K: Ord + Clone, V, I: Ord + Clone> {
    index: OrdMap<K, SlotId>,
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

/// A memo brought up to date, as its readers see it.
struct Verified {
    changed_at: Rev,
    durability: Durability,
    /// The memo's producers as its verification left them (a whole reader inherits them).
    producers: Arc<[MemoRef]>,
    volatile: bool,
}

/// What a dependency walk found when nothing changed.
struct Unchanged {
    /// The minimum of the dependencies' current durabilities.
    durability: Durability,
    /// The producers the memo must settle before push invalidation may call it clean, from the dependencies'
    /// current state: the producer of every part dependency and the producers of every memo dependency.
    producers: Arc<[MemoRef]>,
}

/// Brings a memo of some family up to date and returns its state.
type VerifyFn<D> = fn(&Store<D>, &D, SlotId) -> Result<Verified, Cycle>;

/// A registered family: its storage and the monomorphised entry points the engine calls without knowing
/// the family's types.
struct Family<D: Db> {
    name: &'static str,
    /// [`Query::INCREMENTAL`]: only these families collect `via` keys.
    incremental: bool,
    storage: AnyStorage,
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
            incremental: self.incremental,
            storage: (self.clone_storage)(&self.storage),
            verify: self.verify,
            evict: self.evict,
            clone_storage: self.clone_storage,
            memory: self.memory,
            describe: self.describe,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Executions in progress

/// What one execution has read so far.
enum FrameDeps<I: Ord + Clone> {
    List(Vec<(DepKey<I>, Rev)>),
    /// Seeded with the previous memo's dependencies (incremental execution); a new read replaces its key.
    Map(OrdMap<DepKey<I>, Rev>),
}

struct Frame<I: Ord + Clone> {
    deps: FrameDeps<I>,
    /// The keys this execution read (in `Map` mode the carried keys are already registered).
    fresh: Vec<DepKey<I>>,
    durability: Durability,
    producers: BTreeSet<MemoRef>,
    /// The parts the execution declared changed (`None`: not declared, every part may have changed).
    parts: Option<Vec<I>>,
    /// A cycle was caught below this execution: its value depends on the evaluation context.
    volatile: bool,
}

impl<I: Ord + Clone> Frame<I> {
    fn new() -> Frame<I> {
        Frame {
            deps: FrameDeps::List(Vec::new()),
            fresh: Vec::new(),
            durability: Durability::Library,
            producers: BTreeSet::new(),
            parts: None,
            volatile: false,
        }
    }

    fn carry<V>(previous: &Memo<V, I>) -> Frame<I> {
        let deps = match &previous.deps {
            Deps::Map(m) => m.clone(),
            Deps::List(l) => l.iter().cloned().collect(),
        };
        Frame {
            deps: FrameDeps::Map(deps),
            fresh: Vec::new(),
            durability: previous.durability,
            producers: previous.producers.iter().copied().collect(),
            parts: None,
            volatile: false,
        }
    }

    fn push(&mut self, key: DepKey<I>, rev: Rev, durability: Durability) {
        self.durability = self.durability.min(durability);
        match &mut self.deps {
            FrameDeps::List(l) => {
                if l.last().is_some_and(|(k, r)| *k == key && *r == rev) {
                    return;
                }
                l.push((key, rev));
            }
            FrameDeps::Map(m) => {
                if m.get(&key) == Some(&rev) && self.fresh.last() == Some(&key) {
                    return;
                }
                m.insert(key.clone(), rev);
                self.fresh.push(key);
            }
        }
    }
}

/// A pending push mark of a memo: the revision it was last marked at, and through which keys since it
/// was verified.
#[derive(Clone)]
struct Mark<I: Ord + Clone> {
    at: Rev,
    /// The keys, for an [`INCREMENTAL`](Query::INCREMENTAL) family only (no other family reads them).
    via: Option<imbl::Vector<DepKey<I>>>,
    /// The mark epoch ([`State::mark_epoch`]) at which a commit's marking walked this memo's whole readers;
    /// `None` for a mark set inside a revision (a producer's changed parts), which may land on a memo
    /// already verified at that revision.
    walked: Option<u64>,
}

/// The part stamps of a producer: a part changed at `stamps[part]`, else at `base`.
#[derive(Clone)]
struct Parts<I: Ord + Clone> {
    base: Rev,
    stamps: OrdMap<I, Rev>,
}

/// A memo as a reader receives it.
struct Got<V> {
    value: Option<Arc<V>>,
    changed_at: Rev,
    durability: Durability,
    producers: Arc<[MemoRef]>,
    volatile: bool,
}

/// How an execution runs.
enum Mode<I> {
    /// From scratch.
    Full,
    /// An evicted value whose inputs are unchanged: the value is the same by purity.
    KnownSame,
    /// Through the incremental hook with the previous value.
    Incremental(Changes<I>),
}

// ---------------------------------------------------------------------------------------------------------
// The store

/// The mutable state of a store, behind one `RefCell` (never borrowed across host code).
struct State<D: Db> {
    current: Rev,
    /// Per durability level, the latest revision at which an input of that level or a more durable one
    /// changed.
    last_changed: [Rev; 3],
    /// Memos verified at or after this revision are covered by push marks: every advance since was an
    /// [`Store::advance_with`]. A plain [`Store::advance`] moves it to its revision.
    push_from: Rev,
    /// The touched inputs of every `advance_with` since `push_from`, at most `LOG_LIMIT` revisions.
    log: imbl::Vector<(Rev, Arc<[D::Input]>)>,
    /// The revision of the newest log entry dropped (memos verified before it get no `touched`).
    log_trimmed: Rev,
    families: Vec<Family<D>>,
    by_type: BTreeMap<TypeId, FamilyId>,
    /// Memos being executed or verified, outermost first (cycle detection).
    active: Vec<MemoRef>,
    /// Executions in progress, outermost first (dependency recording).
    frames: Vec<Frame<D::Input>>,
    /// The push index: every registration key (a coarse input, a memo, a part) with the memos that read it.
    rdeps: OrdMap<DepKey<D::Input>, OrdSet<MemoRef>>,
    /// Per producer, the memos that read any of its parts.
    part_readers: OrdMap<MemoRef, OrdSet<MemoRef>>,
    /// Per producer, its part stamps (absent: every part changed with the value, at its `changed_at`).
    parts: OrdMap<MemoRef, Arc<Parts<D::Input>>>,
    /// Pending push marks.
    marks: OrdMap<MemoRef, Mark<D::Input>>,
    /// Bumped whenever a memo's pending mark is cleared without bringing every memo it was marked through up
    /// to date (a durability skip of a marked memo; an incremental execution that did not re-read every memo
    /// of its `via`). While a mark's `walked` equals it, every transitive whole reader of the marked memo
    /// still holds a pending mark, so a commit's marking need not walk them again.
    mark_epoch: u64,
    /// Test only: an incremental execution that leaves a `via` memo unread does not bump the mark epoch (proves
    /// the bump is needed).
    #[cfg(test)]
    skip_incremental_epoch_bump: bool,
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

impl<D: Db> State<D> {
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

    fn record(&mut self, family: &'static str, event: Event) {
        self.commit.record(family, event);
        self.total.record(family, event);
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

    fn describe(&self, fid: FamilyId, slot: SlotId) -> String {
        let f = &self.families[fid.0 as usize];
        format!("{}({})", f.name, (f.describe)(&f.storage, slot))
    }

    /// The stamp of `part` of the producer `producer`, whose value last changed at `changed_at`.
    fn part_stamp(&self, producer: MemoRef, part: &D::Input, changed_at: Rev) -> Rev {
        match self.parts.get(&producer) {
            Some(p) => p.stamps.get(part).copied().unwrap_or(p.base),
            None => changed_at,
        }
    }

    /// Push marking (ADR-0002 amendment 4): marks every memo registered under one of `keys`, and every memo
    /// in `direct` (with its reason), possibly changed at `rev`, then transitively every memo that reads a
    /// marked memo whole. Readers of a marked memo's parts are not marked: they settle their producers before
    /// they trust their own mark.
    ///
    /// `commit` says the marks come from a commit ([`Store::advance_with`]: no memo is verified at `rev` yet)
    /// rather than from a producer inside a revision. A memo whose pending commit mark already walked its
    /// readers in the current mark epoch is re-marked but not walked again (review cycle 9, finding 4): its
    /// transitive whole readers all still hold pending marks, so an unread page memo does not cost its whole
    /// reader closure on every commit. The `via` keys are kept for incremental families only.
    fn mark(
        &mut self,
        rev: Rev,
        commit: bool,
        keys: impl IntoIterator<Item = DepKey<D::Input>>,
        direct: impl IntoIterator<Item = (MemoRef, DepKey<D::Input>)>,
    ) {
        let mut queue: Vec<(MemoRef, DepKey<D::Input>)> = Vec::new();
        for key in keys {
            if let Some(set) = self.rdeps.get(&key) {
                queue.extend(set.iter().map(|m| (*m, key.clone())));
            }
        }
        queue.extend(direct);
        let epoch = self.mark_epoch;
        let mut visited: BTreeSet<MemoRef> = BTreeSet::new();
        while let Some((memo, via)) = queue.pop() {
            let incremental = self.families[memo.0.0 as usize].incremental;
            let mark = self.marks.entry(memo).or_insert_with(|| Mark {
                at: Rev::ZERO,
                via: incremental.then(imbl::Vector::new),
                walked: None,
            });
            mark.at = rev;
            if let Some(v) = mark.via.as_mut() {
                if v.len() >= VIA_LIMIT {
                    mark.via = None;
                } else {
                    v.push_back(via);
                }
            }
            let walked = mark.walked == Some(epoch);
            if commit {
                mark.walked = Some(epoch);
            }
            if walked || !visited.insert(memo) {
                continue;
            }
            let name = self.families[memo.0.0 as usize].name;
            self.record(name, Event::Marked);
            let key = DepKey::Memo(memo.0, memo.1);
            if let Some(set) = self.rdeps.get(&key) {
                queue.extend(set.iter().map(|m| (*m, key.clone())));
            }
        }
    }

    /// The touched inputs since revision `since` (exclusive), or `None` when the log no longer holds them.
    fn touched_since(&self, since: Rev) -> Option<Vec<D::Input>> {
        if since < self.log_trimmed {
            return None;
        }
        let mut out = Vec::new();
        for (rev, inputs) in self.log.iter().rev() {
            if *rev <= since {
                break;
            }
            out.push(Arc::clone(inputs));
        }
        Some(out.iter().rev().flat_map(|l| l.iter().cloned()).collect())
    }
}

/// Whether the test-only switch suppresses the epoch bump of an incremental execution.
#[cfg(test)]
fn skip_bump<D: Db>(st: &State<D>) -> bool {
    st.skip_incremental_epoch_bump
}

/// Whether the test-only switch suppresses the epoch bump of an incremental execution.
#[cfg(not(test))]
fn skip_bump<D: Db>(_: &State<D>) -> bool {
    false
}

/// The memo store of one host database type `D` (ADR-0002 decision 4, amendment 4, ARCH §6.2).
///
/// # Reading
///
/// [`Store::get`] answers `Q(key)` at the current revision:
///
/// 1. **Hit**: the memo was verified at the current revision (no allocation, no dependency clone).
/// 2. **Durability skip**: no input of the memo's durability or a more durable one changed since the memo
///    was verified (see [`Store::advance`]); the memo is re-stamped without looking at its dependencies.
/// 3. **Push skip**: every advance since the memo was verified reported its touched inputs
///    ([`Store::advance_with`]), the memo's producers are up to date, and no mark reached the memo since;
///    so no dependency changed and the memo is re-stamped in O(1).
/// 4. **Incremental re-execution**: a marked memo of an [`INCREMENTAL`](Query::INCREMENTAL) family runs its
///    hook with its previous value and what changed ([`Changes`]), without a dependency walk.
/// 5. **Verification**: the recorded dependencies are checked in read order; an input compares its current
///    revision with the observed one, a memo dependency is first brought up to date itself (recursively,
///    re-executing it if needed) and compares its `changed_at`, a part dependency compares the part's stamp.
///    If nothing changed, the memo is re-stamped. The walk stops at the first changed dependency.
/// 6. **Re-execution**: otherwise (or for a new key, or for an evicted value) the query runs, recording its
///    reads. If the new value is equivalent to the previous one ([`Equivalence`]), `changed_at` keeps its old
///    revision (**early cut-off**: dependents stay green) and the previous `Arc` is kept (for both
///    equivalences, as long as the previous memo still holds its value), so pointer identity survives
///    for consumers such as GPU buffers. After an LRU eviction the re-computed value is a new `Arc`.
///
/// Reads inside a query ([`Cx::get`], or [`Store::get`] called again from the query's body) follow the same
/// steps and record the memo as a dependency of the innermost executing query.
///
/// # Push invalidation
///
/// Every recorded read is registered in a reverse-dependency index under its coarse key ([`Db::coarse`]).
/// [`Store::advance_with`] marks the memos registered under a touched input, and transitively every memo that
/// reads a marked memo whole. A **part** read ([`Store::get_part`]) registers the reader under the part
/// only: the producer's re-execution declares which parts changed ([`Cx::changed_parts`]) and marks their
/// readers then, so a per-key reader of a global value is not invalidated by every change of that value.
///
/// # Volatile executions
///
/// An execution that caught a cycle below it ([`Cx::set_volatile`], or a volatile value it read) depends on
/// the context it ran in. Its value is returned but not kept, and every execution that reads it becomes
/// volatile too. So the store re-runs a volatile query on **every** read (counted as
/// [`Counters::volatile`](crate::Counters::volatile)), where a cold evaluation computes it once per call; the
/// oracle does not compare volatile values (they legitimately depend on the evaluation order). Volatility
/// only arises on inputs that break the host's acyclicity invariant.
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
    st: RefCell<State<D>>,
}

/// A clone is an O(1) snapshot that evolves independently (see [`Store::fork`]). It starts with empty
/// statistics (`commit` and `total`: a job's statistics are its own work) and no oracle mismatches; the
/// oracle mode, the oracle read counter, the LRU (entries and budget), the push index and the memos are
/// copied.
impl<D: Db> Clone for Store<D> {
    fn clone(&self) -> Self {
        let s = self.st.borrow();
        Store {
            st: RefCell::new(State {
                current: s.current,
                last_changed: s.last_changed,
                push_from: s.push_from,
                log: s.log.clone(),
                log_trimmed: s.log_trimmed,
                families: s.families.clone(),
                by_type: s.by_type.clone(),
                active: Vec::new(),
                frames: Vec::new(),
                rdeps: s.rdeps.clone(),
                part_readers: s.part_readers.clone(),
                parts: s.parts.clone(),
                marks: s.marks.clone(),
                mark_epoch: s.mark_epoch,
                #[cfg(test)]
                skip_incremental_epoch_bump: s.skip_incremental_epoch_bump,
                lru: s.lru.clone(),
                commit: Stats::default(),
                total: Stats::default(),
                oracle: s.oracle,
                oracle_reads: s.oracle_reads,
                mismatches: Vec::new(),
                panic_on_cycle: s.panic_on_cycle,
                // A fork is a different store: its own identity, and its parent's as the base.
                token: Arc::new(()),
                base_token: Some(Arc::clone(&s.token)),
                base_rev: s.current,
                lru_base_tick: s.lru.next_tick(),
            }),
        }
    }
}

impl<D: Db> Debug for Store<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.st.borrow();
        f.debug_struct("Store")
            .field("current", &s.current)
            .field("families", &s.families.iter().map(|x| x.name).collect::<Vec<_>>())
            .field("lru_values", &s.lru.len())
            .field("lru_bytes", &s.lru.total_bytes())
            .field("oracle", &s.oracle)
            .finish_non_exhaustive()
    }
}

impl<D: Db> Default for Store<D> {
    fn default() -> Self {
        Store::new()
    }
}

/// Pops the frame and the active entry of an execution that a panic in host code abandoned. Disarmed on
/// the normal path.
struct ExecGuard<'a, D: Db> {
    store: &'a Store<D>,
    frame: bool,
    armed: bool,
}

impl<D: Db> Drop for ExecGuard<'_, D> {
    fn drop(&mut self) {
        if self.armed
            && let Ok(mut st) = self.store.st.try_borrow_mut()
        {
            if self.frame {
                st.frames.pop();
            }
            st.active.pop();
        }
    }
}

impl<D: Db> Store<D> {
    /// Test only: stops incremental executions from bumping the mark epoch.
    #[cfg(test)]
    pub(crate) fn skip_incremental_epoch_bump(&self) {
        self.st.borrow_mut().skip_incremental_epoch_bump = true;
    }

    /// An empty store at [`Rev::ZERO`], without LRU budget and with oracle mode off.
    pub fn new() -> Store<D> {
        Store {
            st: RefCell::new(State {
                current: Rev::ZERO,
                last_changed: [Rev::ZERO; 3],
                push_from: Rev::ZERO,
                log: imbl::Vector::new(),
                log_trimmed: Rev::ZERO,
                families: Vec::new(),
                by_type: BTreeMap::new(),
                active: Vec::new(),
                frames: Vec::new(),
                rdeps: OrdMap::new(),
                part_readers: OrdMap::new(),
                parts: OrdMap::new(),
                marks: OrdMap::new(),
                mark_epoch: 0,
                #[cfg(test)]
                skip_incremental_epoch_bump: false,
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
            }),
        }
    }

    /// The current revision.
    pub fn current(&self) -> Rev {
        self.st.borrow().current
    }

    /// Moves the store to revision `rev` after a commit whose most durable changed input has durability
    /// `changed`, without saying which inputs changed: every memo verified before `rev` is verified by a
    /// dependency walk at its next read (pull mode).
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
    /// If `rev` is not newer than the current revision, or a query is executing.
    pub fn advance(&self, rev: Rev, changed: Durability) {
        let mut st = self.st.borrow_mut();
        Self::advance_state(&mut st, rev, changed);
        st.push_from = rev;
        st.log = imbl::Vector::new();
        st.log_trimmed = rev;
        st.marks = OrdMap::new();
    }

    /// [`Store::advance`] with the commit's touched inputs (push invalidation, ADR-0002 amendment 4): every
    /// memo registered under one of them ([`Db::coarse`]) is marked possibly changed, and transitively every
    /// memo that reads a marked memo whole; every other memo verified since the last plain `advance` is
    /// re-stamped in O(1) at its next read, without a dependency walk.
    ///
    /// `touched` must hold, for every input whose content the commit changed, its coarse key in the snapshot
    /// a memo may have read it from (see [`Db`]: a stamp that changed with equal content need not be
    /// reported). It may hold more (fine keys for incremental hooks, which see it as [`Changes::touched`]).
    ///
    /// # Panics
    ///
    /// As [`Store::advance`].
    pub fn advance_with(&self, rev: Rev, changed: Durability, touched: impl IntoIterator<Item = D::Input>) {
        let touched: Arc<[D::Input]> = touched.into_iter().collect();
        let mut st = self.st.borrow_mut();
        Self::advance_state(&mut st, rev, changed);
        st.log.push_back((rev, Arc::clone(&touched)));
        while st.log.len() > LOG_LIMIT {
            if let Some((r, _)) = st.log.pop_front() {
                st.log_trimmed = r;
            }
        }
        st.mark(rev, true, touched.iter().cloned().map(DepKey::Input), []);
    }

    fn advance_state(st: &mut State<D>, rev: Rev, changed: Durability) {
        assert!(rev > st.current, "ew-memo: advance to {rev} from {}", st.current);
        // No query can be executing during an advance: leftover state is an evaluation that a panic
        // abandoned (and the host caught).
        st.active.clear();
        st.frames.clear();
        stats::emit(st.current, "commit", &st.commit);
        st.commit.clear();
        for level in &mut st.last_changed[..=changed.index()] {
            *level = rev;
        }
        st.current = rev;
        st.mutated();
    }

    /// Reads `Q(key)` at the current revision (see the type documentation). Called from inside a query's
    /// execution (through a handle the host keeps, rather than [`Cx::get`]), the read is recorded as a
    /// dependency of that execution.
    ///
    /// # Errors
    ///
    /// [`Cycle`] when the evaluation reads a query that is already being evaluated (release builds, or with
    /// cycle panics off; debug builds panic).
    pub fn get<Q: Query<D>>(&self, db: &D, key: &Q::Key) -> Result<Arc<Q::Value>, Cycle> {
        self.fetch::<Q>(db, key)
    }

    /// Reads `P(key)` like [`Store::get`], but records a dependency on the part `part` of its value only
    /// (inside an execution): the reader is invalidated when an execution of `P(key)` declares `part`
    /// changed ([`Cx::changed_parts`]), or changes without declaring its parts, never by a change of other
    /// parts. The caller must read only what `part` names from the returned value.
    ///
    /// # Errors
    ///
    /// As [`Store::get`].
    pub fn get_part<P: Query<D>>(
        &self,
        db: &D,
        key: &P::Key,
        part: &D::Input,
    ) -> Result<Arc<P::Value>, Cycle> {
        let fid = self.family::<P>();
        let slot = self.intern::<P>(fid, key);
        let got = self.up_to_date::<P>(db, fid, slot, true)?;
        let value = got.value.clone().expect("a value was requested");
        {
            let mut st = self.st.borrow_mut();
            let stamp = st.part_stamp((fid, slot), part, got.changed_at);
            if let Some(frame) = st.frames.last_mut() {
                frame.push(DepKey::Part(fid, slot, part.clone()), stamp, got.durability);
                frame.producers.insert((fid, slot));
                frame.volatile |= got.volatile;
            }
        }
        if !got.volatile {
            self.oracle_check::<P>(db, key, &value, fid, slot);
        }
        Ok(value)
    }

    /// Records a read of `input` by the innermost executing query (what [`Cx::read`] does), for a host that
    /// calls back through its own handle. Outside an execution it does nothing.
    pub fn read(&self, db: &D, input: &D::Input) {
        if self.st.borrow().frames.is_empty() {
            return;
        }
        let rev = db.changed_at(input);
        let durability = db.durability(input);
        self.read_at(input.clone(), rev, durability);
    }

    /// [`Store::read`] with the stamp and durability the host already knows; they must be what
    /// [`Db::changed_at`] and [`Db::durability`] answer for `input` in the snapshot being read.
    pub fn read_at(&self, input: D::Input, observed: Rev, durability: Durability) {
        let mut st = self.st.borrow_mut();
        if let Some(frame) = st.frames.last_mut() {
            frame.push(DepKey::Input(input), observed, durability);
        }
    }

    /// Declares, for the innermost executing query, that the parts `parts` of its value changed (what
    /// [`Cx::changed_parts`] does).
    pub fn changed_parts(&self, parts: impl IntoIterator<Item = D::Input>) {
        let mut st = self.st.borrow_mut();
        if let Some(frame) = st.frames.last_mut() {
            frame.parts.get_or_insert_with(Vec::new).extend(parts);
        }
    }

    /// Makes the innermost executing query volatile (what [`Cx::set_volatile`] does).
    pub fn set_volatile(&self) {
        let mut st = self.st.borrow_mut();
        if let Some(frame) = st.frames.last_mut() {
            frame.volatile = true;
        }
    }

    /// Whether a query is executing (a read would be recorded as its dependency).
    pub fn executing(&self) -> bool {
        !self.st.borrow().frames.is_empty()
    }

    /// Computes `Q(key)` cold: no memo is read or written, nested queries are computed cold too (each at
    /// most once per call). The reference for oracle mode and `ew verify-derived`.
    ///
    /// # Errors
    ///
    /// [`Cycle`] as for [`Store::get`].
    pub fn cold<Q: Query<D>>(&self, db: &D, key: &Q::Key) -> Result<Arc<Q::Value>, Cycle> {
        let mut cold = Cold {
            panic_on_cycle: self.st.borrow().panic_on_cycle,
            ..Cold::default()
        };
        cold_fetch::<D, Q>(&mut cold, db, key)
    }

    /// Sets oracle mode.
    pub fn set_oracle(&self, oracle: Oracle) {
        self.st.borrow_mut().oracle = oracle;
    }

    /// Whether a cycle panics (the default in debug builds) or is returned as [`Cycle`].
    pub fn set_cycle_panics(&self, panics: bool) {
        self.st.borrow_mut().panic_on_cycle = panics;
    }

    /// The mismatches found by oracle mode since the last call, in the order found.
    pub fn take_mismatches(&self) -> Vec<Mismatch> {
        std::mem::take(&mut self.st.borrow_mut().mismatches)
    }

    /// Sets the byte budget of the LRU over the values of `Query::LRU` families (`None`: unbounded).
    /// Values over budget are evicted at the next memo write.
    pub fn set_lru_budget(&self, bytes: Option<usize>) {
        let mut st = self.st.borrow_mut();
        st.lru.set_budget(bytes);
        st.mutated();
    }

    /// The LRU budget in bytes (`None`: unbounded).
    pub fn lru_budget(&self) -> Option<usize> {
        self.st.borrow().lru.budget()
    }

    /// The bytes of the values currently under the LRU.
    pub fn lru_bytes(&self) -> usize {
        self.st.borrow().lru.total_bytes()
    }

    /// Statistics since the last advance (one commit).
    pub fn commit_stats(&self) -> Stats {
        self.st.borrow().commit.clone()
    }

    /// Statistics over the store's lifetime.
    pub fn total_stats(&self) -> Stats {
        self.st.borrow().total.clone()
    }

    /// Emits the lifetime statistics as `tracing` events (target `ew_memo::stats`, window `total`).
    pub fn emit_stats(&self) {
        let st = self.st.borrow();
        stats::emit(st.current, "total", &st.total);
    }

    /// The dependency key naming the memo of `Q(key)`, if the key was ever read.
    pub fn memo_key<Q: Query<D>>(&self, key: &Q::Key) -> Option<DepKey<D::Input>> {
        let st = self.st.borrow();
        let fid = *st.by_type.get(&TypeId::of::<Q>())?;
        let slot = *st.storage::<Q>(fid).index.get(key)?;
        Some(DepKey::Memo(fid, slot))
    }

    /// The key of family `Q` that a memo or part dependency names (as found in [`Changes::via`]); `None`
    /// for an input, or a memo of another family.
    pub fn key_of<Q: Query<D>>(&self, dep: &DepKey<D::Input>) -> Option<Q::Key> {
        let (fid, slot) = match dep {
            DepKey::Memo(f, s) | DepKey::Part(f, s, _) => (*f, *s),
            DepKey::Input(_) => return None,
        };
        let st = self.st.borrow();
        if st.by_type.get(&TypeId::of::<Q>()) != Some(&fid) {
            return None;
        }
        st.storage::<Q>(fid)
            .slots
            .get(slot.index())
            .map(|s| s.key.clone())
    }

    /// The reverse-dependency API (ADR-0002 amendment 1): the keys of family `Q` whose memo's recorded reads
    /// include `dep` (an input, or a memo named by [`Store::memo_key`]), in key order. Indexed for
    /// `Query::RDEPS` families, a scan otherwise.
    pub fn dependents<Q: Query<D>>(&self, dep: &DepKey<D::Input>) -> Vec<Q::Key> {
        let st = self.st.borrow();
        let Some(&fid) = st.by_type.get(&TypeId::of::<Q>()) else {
            return Vec::new();
        };
        let s = st.storage::<Q>(fid);
        let mut keys: Vec<Q::Key> = match &s.rdeps {
            Some(index) => index
                .get(dep)
                .into_iter()
                .map(|slot| s.slots[slot.index()].key.clone())
                .collect(),
            None => s
                .slots
                .iter()
                .filter(|slot| slot.memo.as_ref().is_some_and(|m| m.deps.contains(dep)))
                .map(|slot| slot.key.clone())
                .collect(),
        };
        keys.sort();
        keys
    }

    /// The state of the memo of `Q(key)`, if any.
    pub fn memo_info<Q: Query<D>>(&self, key: &Q::Key) -> Option<MemoInfo<D::Input>> {
        let st = self.st.borrow();
        let fid = *st.by_type.get(&TypeId::of::<Q>())?;
        let s = st.storage::<Q>(fid);
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
        let st = self.st.borrow();
        let mut out: Vec<FamilyMemory> = st
            .families
            .iter()
            .map(|f| (f.memory)(&f.storage, f.name))
            .collect();
        out.sort_by_key(|m| m.family);
        out
    }

    /// The (registration key, memo) pairs of the push index (G-SP-04: about 40 MB at XL is the budget).
    pub fn push_index_entries(&self) -> usize {
        self.st.borrow().rdeps.values().map(OrdSet::len).sum()
    }

    /// An O(1) snapshot that evolves independently (a job's memo store). The fork starts with empty
    /// statistics and no oracle mismatches (its statistics and findings are its own work; adopting it
    /// merges them into this store's); the oracle mode, the LRU, the push index and the memos are copied.
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
    /// Adopting takes the fork's memos (families, their LRU entries, the push index and marks) and keeps this
    /// store's configuration: oracle mode, cycle behaviour, LRU budget and oracle read counter. The fork's
    /// statistics are merged into this store's, its oracle mismatches follow this store's, and the LRU
    /// recency of what this store hit since the fork is kept (those entries stay the most recent). Values
    /// over this store's budget are evicted.
    pub fn adopt_if_unchanged(&mut self, fork: Store<D>) -> bool {
        let st = self.st.get_mut();
        let mut fork = fork.st.into_inner();
        let same_base = fork
            .base_token
            .as_ref()
            .is_some_and(|base| Arc::ptr_eq(base, &st.token));
        if !same_base || st.current != fork.base_rev || fork.current != fork.base_rev || !st.active.is_empty()
        {
            return false;
        }
        let parent_touched = st.lru.touched_since(fork.lru_base_tick);
        let budget = st.lru.budget();
        st.families = fork.families;
        st.by_type = fork.by_type;
        st.lru = fork.lru;
        st.rdeps = fork.rdeps;
        st.part_readers = fork.part_readers;
        st.parts = fork.parts;
        st.marks = fork.marks;
        st.mark_epoch = fork.mark_epoch;
        for (memo, bytes) in parent_touched {
            if st.lru.contains(memo) {
                st.lru.touch(memo, bytes);
            }
        }
        st.lru.set_budget(budget);
        st.commit.merge(&fork.commit);
        st.total.merge(&fork.total);
        st.mismatches.append(&mut fork.mismatches);
        st.evict_over_budget();
        // The adopted state is a new state of this store: no earlier fork of either side has it as its base.
        st.token = Arc::new(());
        st.base_token = None;
        true
    }

    /// The pending marks as `(family, holds via keys)`, in memo order (tests).
    #[cfg(test)]
    pub(crate) fn pending_marks(&self) -> Vec<(&'static str, bool)> {
        let st = self.st.borrow();
        st.marks
            .iter()
            .map(|((fid, _), m)| (st.families[fid.0 as usize].name, m.via.is_some()))
            .collect()
    }

    // -----------------------------------------------------------------------------------------------------
    // Families

    fn family<Q: Query<D>>(&self) -> FamilyId {
        let mut st = self.st.borrow_mut();
        if let Some(&fid) = st.by_type.get(&TypeId::of::<Q>()) {
            return fid;
        }
        let fid = FamilyId(u32::try_from(st.families.len()).expect("fewer than 2^32 families"));
        let storage: FamilyStore<Q::Key, Q::Value, D::Input> = FamilyStore {
            index: OrdMap::new(),
            slots: imbl::Vector::new(),
            rdeps: Q::RDEPS.then(RevDeps::new),
        };
        st.families.push(Family {
            name: Q::NAME,
            incremental: Q::INCREMENTAL,
            storage: Box::new(storage),
            verify: verify_erased::<D, Q>,
            evict: evict_erased::<Q::Key, Q::Value, D::Input>,
            clone_storage: clone_erased::<Q::Key, Q::Value, D::Input>,
            memory: memory_erased::<Q::Key, Q::Value, D::Input>,
            describe: describe_erased::<Q::Key, Q::Value, D::Input>,
        });
        st.by_type.insert(TypeId::of::<Q>(), fid);
        fid
    }

    fn intern<Q: Query<D>>(&self, fid: FamilyId, key: &Q::Key) -> SlotId {
        let mut st = self.st.borrow_mut();
        if let Some(&slot) = st.storage::<Q>(fid).index.get(key) {
            return slot;
        }
        let s = st.storage_mut::<Q>(fid);
        let slot = SlotId(u32::try_from(s.slots.len()).expect("fewer than 2^32 keys per family"));
        s.slots.push_back(Slot {
            key: key.clone(),
            memo: None,
        });
        s.index.insert(key.clone(), slot);
        st.mutated();
        slot
    }

    fn record(&self, family: &'static str, event: Event) {
        self.st.borrow_mut().record(family, event);
    }

    // -----------------------------------------------------------------------------------------------------
    // Reading

    /// Reads `Q(key)` and records it as a dependency of the innermost executing query, if any.
    fn fetch<Q: Query<D>>(&self, db: &D, key: &Q::Key) -> Result<Arc<Q::Value>, Cycle> {
        let fid = self.family::<Q>();
        let slot = self.intern::<Q>(fid, key);
        let got = self.up_to_date::<Q>(db, fid, slot, true)?;
        let value = got.value.clone().expect("a value was requested");
        {
            let mut st = self.st.borrow_mut();
            if let Some(frame) = st.frames.last_mut() {
                frame.push(DepKey::Memo(fid, slot), got.changed_at, got.durability);
                frame.producers.extend(got.producers.iter().copied());
                frame.volatile |= got.volatile;
            }
        }
        if !got.volatile {
            self.oracle_check::<Q>(db, key, &value, fid, slot);
        }
        Ok(value)
    }

    /// Brings the memo of `ref` up to date through its family's erased entry point.
    fn verify_ref(&self, db: &D, (fid, slot): MemoRef) -> Result<Verified, Cycle> {
        let verify = self.st.borrow().families[fid.0 as usize].verify;
        verify(self, db, slot)
    }

    /// Brings the memo of `slot` to the current revision: hit, durability skip, push skip, incremental
    /// re-execution, verification or re-execution. With `need_value`, an evicted value is re-computed.
    fn up_to_date<Q: Query<D>>(
        &self,
        db: &D,
        fid: FamilyId,
        slot: SlotId,
        need_value: bool,
    ) -> Result<Got<Q::Value>, Cycle> {
        let me = (fid, slot);
        let (prev, current, last_changed, push_from) = {
            let mut st = self.st.borrow_mut();
            if st.active.contains(&me) {
                drop(st);
                return Err(self.cycle(me));
            }
            let current = st.current;
            let hit = match &st.storage::<Q>(fid).slots[slot.index()].memo {
                None => None,
                Some(m) if m.verified_at == current && (m.value.is_some() || !need_value) => Some((
                    Got {
                        value: m.value.clone(),
                        changed_at: m.changed_at,
                        durability: m.durability,
                        producers: Arc::clone(&m.producers),
                        volatile: false,
                    },
                    m.bytes,
                )),
                Some(_) => None,
            };
            if let Some((got, bytes)) = hit {
                st.record(Q::NAME, Event::Hit);
                if Q::LRU && got.value.is_some() {
                    st.lru.touch(me, bytes);
                }
                return Ok(got);
            }
            let prev = st.storage::<Q>(fid).slots[slot.index()].memo.clone();
            (prev, current, st.last_changed, st.push_from)
        };
        let Some(prev) = prev else {
            return self.execute::<Q>(db, fid, slot, None, Mode::Full);
        };
        if prev.verified_at == current {
            // Verified, but the value was evicted: the inputs are the same, so is the value.
            return self.execute::<Q>(db, fid, slot, Some(prev), Mode::KnownSame);
        }
        if last_changed[prev.durability.index()] <= prev.verified_at {
            // Nothing of the memo's durability or a more durable one changed since it was verified, so no
            // dependency changed and none changed its durability (a move between levels is a change of the
            // more durable one): the memo's durability stands.
            self.record(Q::NAME, Event::DurabilitySkip);
            {
                // A pending mark is cleared without verifying what the memo was marked through: commit
                // marks walked before now no longer guarantee marked readers (`State::mark_epoch`).
                let mut st = self.st.borrow_mut();
                if st.marks.contains_key(&me) {
                    st.mark_epoch += 1;
                }
            }
            let durability = prev.durability;
            let producers = Arc::clone(&prev.producers);
            return self.green::<Q>(db, fid, slot, prev, durability, producers, need_value);
        }
        if prev.verified_at >= push_from {
            // Push mode: settle the producers first (their re-execution marks the readers of changed parts),
            // then trust the mark.
            for &p in prev.producers.iter() {
                self.verify_ref(db, p)?;
            }
            let mark = self
                .st
                .borrow()
                .marks
                .get(&me)
                .filter(|m| m.at > prev.verified_at)
                .cloned();
            match mark {
                None => {
                    // No mark: no dependency re-executed since the verification (a re-execution of a memo
                    // dependency follows its mark, which reaches this memo), so the producers stand.
                    self.record(Q::NAME, Event::PushSkip);
                    let durability = prev.durability;
                    let producers = Arc::clone(&prev.producers);
                    return self.green::<Q>(db, fid, slot, prev, durability, producers, need_value);
                }
                Some(mark) if Q::INCREMENTAL && prev.value.is_some() => {
                    let touched = self.st.borrow().touched_since(prev.verified_at);
                    let changes = Changes {
                        touched,
                        via: mark.via.map(|v| v.into_iter().collect()),
                    };
                    return self.execute::<Q>(db, fid, slot, Some(prev), Mode::Incremental(changes));
                }
                Some(_) => {}
            }
        }
        self.st.borrow_mut().active.push(me);
        let mut guard = ExecGuard {
            store: self,
            frame: false,
            armed: true,
        };
        let walk = self.deps_unchanged(db, &prev.deps, &prev.producers);
        guard.armed = false;
        self.st.borrow_mut().active.pop();
        match walk? {
            Some(Unchanged {
                durability,
                producers,
            }) => {
                self.record(Q::NAME, Event::Verified);
                self.green::<Q>(db, fid, slot, prev, durability, producers, need_value)
            }
            None => self.execute::<Q>(db, fid, slot, Some(prev), Mode::Full),
        }
    }

    /// Re-stamps a memo found unchanged with its current durability and producers, and returns it
    /// (re-computing an evicted value when one is needed).
    #[allow(clippy::too_many_arguments)]
    fn green<Q: Query<D>>(
        &self,
        db: &D,
        fid: FamilyId,
        slot: SlotId,
        mut prev: Memo<Q::Value, D::Input>,
        durability: Durability,
        producers: Arc<[MemoRef]>,
        need_value: bool,
    ) -> Result<Got<Q::Value>, Cycle> {
        {
            let mut st = self.st.borrow_mut();
            let current = st.current;
            let s = st.storage_mut::<Q>(fid);
            if let Some(m) = s.slots.get_mut(slot.index()).and_then(|x| x.memo.as_mut()) {
                m.verified_at = current;
                m.durability = durability;
                if !Arc::ptr_eq(&m.producers, &producers) {
                    m.producers = Arc::clone(&producers);
                }
            }
            st.marks.remove(&(fid, slot));
            if Q::LRU && prev.value.is_some() {
                st.lru.touch((fid, slot), prev.bytes);
            }
            st.mutated();
        }
        prev.producers = producers;
        if prev.value.is_some() || !need_value {
            return Ok(Got {
                value: prev.value,
                changed_at: prev.changed_at,
                durability,
                producers: prev.producers,
                volatile: false,
            });
        }
        self.execute::<Q>(db, fid, slot, Some(prev), Mode::KnownSame)
    }

    /// Whether no recorded dependency changed since it was observed: `Some` with the minimum of the
    /// dependencies' *current* durabilities (an input's as the host reports it now, a memo's as its own
    /// verification left it; [`Durability::Library`] without dependencies) and the dependencies' current
    /// producers, `None` at the first change. Memo dependencies are brought up to date first; the walk stops
    /// at the first change.
    ///
    /// The durability is recomputed because a dependency can keep its revision while its durability drops:
    /// a memo dependency that re-executed with a different read set and cut off, or an input that moved to a
    /// less durable level while the memo it feeds cut off. Keeping the old durability would let a later,
    /// less durable change skip the memo's verification and return a stale value.
    ///
    /// The producers are recomputed for the same reason (review cycle 9, finding 1): a memo dependency that
    /// re-executed with another producer and cut off leaves this memo green, and push invalidation must then
    /// settle the new producer, whose part changes are what reach this memo. `old` is reused when equal.
    fn deps_unchanged(
        &self,
        db: &D,
        deps: &Deps<D::Input>,
        old: &Arc<[MemoRef]>,
    ) -> Result<Option<Unchanged>, Cycle> {
        let mut durability = Durability::Library;
        let mut producers: BTreeSet<MemoRef> = BTreeSet::new();
        for (dep, observed) in deps.iter() {
            let (now, level) = match dep {
                DepKey::Input(input) => (db.changed_at(input), db.durability(input)),
                DepKey::Memo(fid, slot) => {
                    let v = self.verify_ref(db, (*fid, *slot))?;
                    if v.volatile {
                        return Ok(None);
                    }
                    producers.extend(v.producers.iter().copied());
                    (v.changed_at, v.durability)
                }
                DepKey::Part(fid, slot, part) => {
                    let v = self.verify_ref(db, (*fid, *slot))?;
                    if v.volatile {
                        return Ok(None);
                    }
                    producers.insert((*fid, *slot));
                    let stamp = self.st.borrow().part_stamp((*fid, *slot), part, v.changed_at);
                    (stamp, v.durability)
                }
            };
            if now != observed {
                return Ok(None);
            }
            durability = durability.min(level);
        }
        let producers = if producers.iter().eq(old.iter()) {
            Arc::clone(old)
        } else {
            producers.into_iter().collect()
        };
        Ok(Some(Unchanged {
            durability,
            producers,
        }))
    }

    /// Runs the query and stores the new memo (unless it is volatile).
    #[allow(clippy::too_many_lines)]
    fn execute<Q: Query<D>>(
        &self,
        db: &D,
        fid: FamilyId,
        slot: SlotId,
        prev: Option<Memo<Q::Value, D::Input>>,
        mode: Mode<D::Input>,
    ) -> Result<Got<Q::Value>, Cycle> {
        let me = (fid, slot);
        let incremental = matches!(mode, Mode::Incremental(_));
        let key = {
            let mut st = self.st.borrow_mut();
            let key = st.storage::<Q>(fid).slots[slot.index()].key.clone();
            let frame = match (&mode, &prev) {
                (Mode::Incremental(_), Some(p)) => Frame::carry(p),
                _ => Frame::new(),
            };
            st.active.push(me);
            st.frames.push(frame);
            key
        };
        let mut guard = ExecGuard {
            store: self,
            frame: true,
            armed: true,
        };
        let result = {
            let mut cx = Cx {
                db,
                inner: Inner::Memo(self),
            };
            match &mode {
                Mode::Incremental(changes) => {
                    let previous = prev
                        .as_ref()
                        .and_then(|p| p.value.as_deref())
                        .map(|v| (v, changes));
                    Q::execute_incremental(&mut cx, &key, previous)
                }
                _ if Q::INCREMENTAL => Q::execute_incremental(&mut cx, &key, None),
                _ => Q::execute(&mut cx, &key),
            }
        };
        guard.armed = false;
        let frame = {
            let mut st = self.st.borrow_mut();
            st.active.pop();
            st.frames.pop().expect("own frame")
        };
        let value = result?;
        let (current, producers) = {
            let mut st = self.st.borrow_mut();
            st.record(Q::NAME, Event::ReExecuted);
            if incremental {
                st.record(Q::NAME, Event::Incremental);
            }
            let producers: Arc<[MemoRef]> = frame.producers.iter().copied().collect();
            (st.current, producers)
        };
        if frame.volatile {
            // Context-dependent: returned, never kept; the reader becomes volatile too.
            self.record(Q::NAME, Event::Volatile);
            return Ok(Got {
                value: Some(Arc::new(value)),
                changed_at: current,
                durability: frame.durability,
                producers,
                volatile: true,
            });
        }
        let known_same = matches!(mode, Mode::KnownSame);
        let fingerprint = match Q::equivalence() {
            Equivalence::Hash128(hash) => Some(hash(&value)),
            Equivalence::Eq(_) => None,
        };
        // Early cut-off: an equivalent value keeps its `changed_at` (and the previous `Arc`, when it is held).
        let (value, changed_at, cut_off) = match prev.as_ref() {
            Some(p) => {
                let same = match (Q::equivalence(), p.value.as_ref()) {
                    (Equivalence::Eq(eq), Some(old)) => eq(old, &value),
                    (Equivalence::Eq(_), None) => known_same,
                    (Equivalence::Hash128(_), _) => p.fingerprint == fingerprint,
                };
                if same {
                    // The old value is equivalent by definition of the cut-off: keep its identity (both
                    // equivalences). Without it (evicted), the re-computed value is the only one.
                    let arc = match &p.value {
                        Some(old) => Arc::clone(old),
                        None => Arc::new(value),
                    };
                    (arc, p.changed_at, true)
                } else {
                    (Arc::new(value), current, false)
                }
            }
            None => (Arc::new(value), current, false),
        };
        let bytes = Q::bytes(&value);
        let durability = frame.durability;
        let deps = match frame.deps {
            FrameDeps::List(l) => Deps::List(Arc::from(l)),
            FrameDeps::Map(m) => Deps::Map(m),
        };
        // An incremental execution clears the memo's mark; when it did not re-read every memo it was marked
        // through, those may stay marked while this one is clean, so walked commit marks lose their guarantee.
        let unverified_via = match &mode {
            Mode::Incremental(changes) => match &changes.via {
                None => true,
                Some(via) => {
                    let read: BTreeSet<MemoRef> = frame
                        .fresh
                        .iter()
                        .filter_map(|k| match k {
                            DepKey::Memo(f, s) | DepKey::Part(f, s, _) => Some((*f, *s)),
                            DepKey::Input(_) => None,
                        })
                        .collect();
                    via.iter()
                        .any(|k| matches!(k, DepKey::Memo(f, s) if !read.contains(&(*f, *s))))
                }
            },
            _ => false,
        };
        // Registration keys (host code: computed before the state is borrowed).
        let carried = incremental;
        let new_keys: Vec<DepKey<D::Input>> = if carried {
            frame.fresh
        } else {
            deps.iter().map(|(k, _)| k.clone()).collect()
        };
        // The previous registrations as recorded (not recomputed in this snapshot, where a moved row has
        // another coarse key). A full execution replaces them; an incremental one keeps them and adds the keys
        // of its fresh reads (a carried read stays a dependency).
        let old_reg: BTreeSet<DepKey<D::Input>> = prev
            .as_ref()
            .map(|p| p.reg.iter().cloned().collect())
            .unwrap_or_default();
        let mut new_reg = registration(db, new_keys.iter());
        if carried {
            new_reg.extend(old_reg.iter().cloned());
        }
        let reg: Arc<[DepKey<D::Input>]> = match prev.as_ref() {
            Some(p) if new_reg.iter().eq(p.reg.iter()) => Arc::clone(&p.reg),
            _ => new_reg.iter().cloned().collect(),
        };
        let memo = Memo {
            value: Some(Arc::clone(&value)),
            fingerprint,
            deps,
            producers: Arc::clone(&producers),
            reg,
            verified_at: current,
            changed_at,
            durability,
            bytes,
        };
        {
            let mut st = self.st.borrow_mut();
            for key in old_reg.difference(&new_reg) {
                unregister(&mut st, key, me);
            }
            for key in new_reg.difference(&old_reg) {
                register(&mut st, key, me);
            }
            let (old_producers, new_producers) = (part_producers(&old_reg), part_producers(&new_reg));
            for p in old_producers.difference(&new_producers) {
                let empty = match st.part_readers.get_mut(p) {
                    Some(set) => {
                        set.remove(&me);
                        set.is_empty()
                    }
                    None => false,
                };
                if empty {
                    st.part_readers.remove(p);
                }
            }
            for p in new_producers {
                st.part_readers.entry(p).or_default().insert(me);
            }
            {
                let s = st.storage_mut::<Q>(fid);
                if let Some(index) = s.rdeps.as_mut() {
                    if let Some(p) = prev.as_ref() {
                        index.remove(slot, &p.deps.to_vec());
                    }
                    index.add(slot, &memo.deps.to_vec());
                }
                s.slots.get_mut(slot.index()).expect("interned slot").memo = Some(memo);
            }
            if st.marks.remove(&me).is_some() && unverified_via && !skip_bump(&st) {
                st.mark_epoch += 1;
            }
            if cut_off {
                st.record(Q::NAME, Event::CutOff);
            } else if prev.is_some() {
                // The value changed: update the part stamps and mark the readers of the changed parts.
                let prev_changed = prev.as_ref().map_or(Rev::ZERO, |p| p.changed_at);
                match frame.parts {
                    Some(declared) => {
                        let mut parts = st.parts.get(&me).map_or_else(
                            || Parts {
                                base: prev_changed,
                                stamps: OrdMap::new(),
                            },
                            |p| (**p).clone(),
                        );
                        for p in &declared {
                            parts.stamps.insert(p.clone(), current);
                        }
                        st.parts.insert(me, Arc::new(parts));
                        st.mark(
                            current,
                            false,
                            declared.into_iter().map(|p| DepKey::Part(fid, slot, p)),
                            [],
                        );
                    }
                    None => {
                        st.parts.remove(&me);
                        let readers: Vec<MemoRef> = st
                            .part_readers
                            .get(&me)
                            .map(|r| r.iter().copied().collect())
                            .unwrap_or_default();
                        let via = DepKey::Memo(fid, slot);
                        st.mark(current, false, [], readers.into_iter().map(|r| (r, via.clone())));
                    }
                }
            }
            st.mutated();
            if Q::LRU {
                st.lru.touch(me, bytes);
                st.evict_over_budget();
            }
        }
        Ok(Got {
            value: Some(value),
            changed_at,
            durability,
            producers,
            volatile: false,
        })
    }

    fn cycle(&self, repeated: MemoRef) -> Cycle {
        let (cycle, panics) = {
            let mut st = self.st.borrow_mut();
            let start = st.active.iter().position(|m| *m == repeated).unwrap_or(0);
            let mut path: Vec<String> = st.active[start..]
                .iter()
                .map(|&(fid, slot)| st.describe(fid, slot))
                .collect();
            path.push(st.describe(repeated.0, repeated.1));
            let name = st.families[repeated.0.0 as usize].name;
            st.record(name, Event::Cycle);
            (Cycle { path }, st.panic_on_cycle)
        };
        if panics {
            panic!("{cycle}");
        }
        cycle
    }

    // -----------------------------------------------------------------------------------------------------
    // Oracle

    fn oracle_check<Q: Query<D>>(
        &self,
        db: &D,
        key: &Q::Key,
        memoised: &Arc<Q::Value>,
        fid: FamilyId,
        slot: SlotId,
    ) {
        let selected = {
            let mut st = self.st.borrow_mut();
            match st.oracle {
                Oracle::Off => false,
                Oracle::Every => true,
                Oracle::OneIn(n) => {
                    st.oracle_reads += 1;
                    st.oracle_reads % u64::from(n.get()) == 0
                }
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
            let memoised_text = bounded(format!("{memoised:?}"));
            let cold_text = match cold {
                Ok(v) => bounded(format!("{v:?}")),
                Err(c) => bounded(c.to_string()),
            };
            let key_text = bounded(format!("{key:?}"));
            let mut st = self.st.borrow_mut();
            st.record(Q::NAME, Event::OracleMismatch);
            let (verified_at, changed_at, durability, deps) = memo_trail::<D, Q>(&st, fid, slot);
            let rev = st.current;
            st.mismatches.push(Mismatch {
                family: Q::NAME,
                key: key_text,
                rev,
                memoised: memoised_text,
                cold: cold_text,
                verified_at,
                changed_at,
                durability,
                deps,
            });
        }
    }
}

/// The registration keys of dependencies: an input's coarse key in `db`, a memo or a part as is.
fn registration<'a, D: Db>(
    db: &D,
    deps: impl Iterator<Item = &'a DepKey<D::Input>>,
) -> BTreeSet<DepKey<D::Input>>
where
    D::Input: 'a,
{
    deps.map(|d| match d {
        DepKey::Input(i) => DepKey::Input(db.coarse(i)),
        other => other.clone(),
    })
    .collect()
}

fn register<D: Db>(st: &mut State<D>, key: &DepKey<D::Input>, me: MemoRef) {
    st.rdeps.entry(key.clone()).or_default().insert(me);
}

fn unregister<D: Db>(st: &mut State<D>, key: &DepKey<D::Input>, me: MemoRef) {
    let empty = match st.rdeps.get_mut(key) {
        Some(set) => {
            set.remove(&me);
            set.is_empty()
        }
        None => false,
    };
    if empty {
        st.rdeps.remove(key);
    }
}

/// The producers whose parts a set of registration keys reads.
fn part_producers<I: Ord + Clone>(keys: &BTreeSet<DepKey<I>>) -> BTreeSet<MemoRef> {
    keys.iter()
        .filter_map(|k| match k {
            DepKey::Part(f, s, _) => Some((*f, *s)),
            _ => None,
        })
        .collect()
}

/// The state and the rendered dependency trail of a memo, for a [`Mismatch`].
fn memo_trail<D: Db, Q: Query<D>>(
    st: &State<D>,
    fid: FamilyId,
    slot: SlotId,
) -> (Rev, Rev, Durability, Vec<String>) {
    let Some(m) = st.storage::<Q>(fid).slots[slot.index()].memo.as_ref() else {
        return (Rev::ZERO, Rev::ZERO, Durability::Library, Vec::new());
    };
    let mut deps: Vec<String> = m
        .deps
        .iter()
        .take(MISMATCH_DEPS)
        .map(|(dep, rev)| match dep {
            DepKey::Input(input) => bounded(format!("input({input:?})@{rev}")),
            DepKey::Memo(f, s) => format!("{}@{rev}", st.describe(*f, *s)),
            DepKey::Part(f, s, p) => bounded(format!("{}[{p:?}]@{rev}", st.describe(*f, *s))),
        })
        .collect();
    if m.deps.len() > MISMATCH_DEPS {
        deps.push("…".to_owned());
    }
    (m.verified_at, m.changed_at, m.durability, deps)
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

fn verify_erased<D: Db, Q: Query<D>>(store: &Store<D>, db: &D, slot: SlotId) -> Result<Verified, Cycle> {
    let fid = store.family::<Q>();
    store.up_to_date::<Q>(db, fid, slot, false).map(|got| Verified {
        changed_at: got.changed_at,
        durability: got.durability,
        producers: got.producers,
        volatile: got.volatile,
    })
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
    Memo(&'a Store<D>),
    Cold(&'a mut Cold),
}

/// The context a query executes in: the host's database and the tracked reads.
///
/// In a memoised evaluation, [`Cx::read`], [`Cx::get`] and [`Cx::get_part`] record dependencies; in a cold
/// one (oracle mode, [`Store::cold`]) nothing is recorded and nested queries are computed cold.
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

    /// The store of a memoised evaluation (`None` in a cold one), for a host that records its reads through
    /// its own handle while the query runs.
    pub fn store(&self) -> Option<&'a Store<D>> {
        match &self.inner {
            Inner::Memo(store) => Some(store),
            Inner::Cold(_) => None,
        }
    }

    /// Records a read of `input`. The host's tracked accessors call this for every input they read.
    pub fn read(&mut self, input: &D::Input) {
        if let Inner::Memo(store) = &self.inner {
            store.read(self.db, input);
        }
    }

    /// Reads another query.
    ///
    /// # Errors
    ///
    /// [`Cycle`] when `Q(key)` is already being evaluated (release builds; debug builds panic).
    pub fn get<Q: Query<D>>(&mut self, key: &Q::Key) -> Result<Arc<Q::Value>, Cycle> {
        match &mut self.inner {
            Inner::Memo(store) => store.fetch::<Q>(self.db, key),
            Inner::Cold(cold) => cold_fetch::<D, Q>(cold, self.db, key),
        }
    }

    /// Reads another query's value and depends on its part `part` only (see [`Store::get_part`]). Cold, it
    /// is [`Cx::get`].
    ///
    /// # Errors
    ///
    /// As [`Cx::get`].
    pub fn get_part<P: Query<D>>(&mut self, key: &P::Key, part: &D::Input) -> Result<Arc<P::Value>, Cycle> {
        match &mut self.inner {
            Inner::Memo(store) => store.get_part::<P>(self.db, key, part),
            Inner::Cold(cold) => cold_fetch::<D, P>(cold, self.db, key),
        }
    }

    /// Declares that the parts `parts` of this execution's value changed (a producer's re-execution): the
    /// readers of those parts are marked. A producer that changes its value without declaring parts marks
    /// every part reader; declaring an empty list says no part changed. Cold, it does nothing.
    pub fn changed_parts(&mut self, parts: impl IntoIterator<Item = D::Input>) {
        if let Inner::Memo(store) = &self.inner {
            store.changed_parts(parts);
        }
    }

    /// Marks this execution volatile: its value depends on the evaluation context (it caught a cycle), so it
    /// is not kept and its readers become volatile too. Cold, it does nothing.
    pub fn set_volatile(&mut self) {
        if let Inner::Memo(store) = &self.inner {
            store.set_volatile();
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
