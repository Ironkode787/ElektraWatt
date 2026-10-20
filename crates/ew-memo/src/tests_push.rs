// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Engine tests of the ADR-0002 amendment 4 additions: push invalidation over coarse keys, part
//! dependencies, the incremental re-execution hook, volatile executions and re-entrant reads.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

use proptest::prelude::*;

use crate::{
    Changes, Counters, Cx, Cycle, Db, DepKey, Durability, Equivalence, Mismatch, Oracle, Query, Rev, Store,
};

// ---------------------------------------------------------------------------------------------------------
// The toy database: cells grouped by tens

/// The number of groups `Groups` sums (cells `0..40`).
const GROUPS: u32 = 4;

/// Cells `k -> (value, changed_at, durability)`; the coarse key of a cell is its group's key.
#[derive(Clone, Default)]
struct Grid {
    cells: BTreeMap<u32, (i64, Rev, Durability)>,
    rev: Rev,
}

impl Db for Grid {
    type Input = u32;

    fn changed_at(&self, input: &u32) -> Rev {
        self.cells.get(input).map_or(Rev::ZERO, |c| c.1)
    }

    fn durability(&self, input: &u32) -> Durability {
        self.cells.get(input).map_or(Durability::Drawing, |c| c.2)
    }

    fn coarse(&self, input: &u32) -> u32 {
        coarse_key(*input)
    }
}

/// The group key of cell `k` (cells `0..10` -> 1000, `10..20` -> 1001, ...); keys from 1000 on map to themselves.
fn coarse_key(k: u32) -> u32 {
    if k < 1000 { 1000 + k / 10 } else { k }
}

/// The part key of group `g` in `Groups`.
fn part_key(g: u32) -> u32 {
    2000 + g
}

type Change = (u32, i64, Durability);

fn set(k: u32, v: i64) -> Change {
    (k, v, Durability::Drawing)
}

impl Grid {
    fn value(&self, k: u32) -> i64 {
        self.cells.get(&k).map_or(0, |c| c.0)
    }

    /// Bumps the revision and writes the cells; returns the most durable level of the commit.
    fn apply(&mut self, changes: &[Change]) -> Durability {
        self.rev = Rev(self.rev.0 + 1);
        let mut most = Durability::Drawing;
        for &(k, v, d) in changes {
            let before = self.durability(&k);
            self.cells.insert(k, (v, self.rev, d));
            most = most.max(d).max(before);
        }
        most
    }

    /// A push commit: reports every changed cell and its group key.
    fn commit_push(&mut self, store: &Store<Grid>, changes: &[Change]) {
        let most = self.apply(changes);
        let touched: Vec<u32> = changes.iter().flat_map(|&(k, _, _)| [k, coarse_key(k)]).collect();
        store.advance_with(self.rev, most, touched);
    }

    /// A push commit that reports only the group keys.
    fn commit_coarse_only(&mut self, store: &Store<Grid>, changes: &[Change]) {
        let most = self.apply(changes);
        let touched: Vec<u32> = changes.iter().map(|&(k, _, _)| coarse_key(k)).collect();
        store.advance_with(self.rev, most, touched);
    }

    /// A pull commit: a plain advance.
    fn commit_pull(&mut self, store: &Store<Grid>, changes: &[Change]) {
        let most = self.apply(changes);
        store.advance(self.rev, most);
    }
}

/// A store at revision 1 over cells `0..50` holding `k % 7`.
fn setup() -> (Grid, Store<Grid>) {
    let mut db = Grid::default();
    let store = Store::new();
    let init: Vec<Change> = (0..50).map(|k| set(k, i64::from(k % 7))).collect();
    db.commit_push(&store, &init);
    (db, store)
}

fn counters(store: &Store<Grid>, family: &str) -> Counters {
    store.commit_stats().family(family)
}

// ---------------------------------------------------------------------------------------------------------
// Families

/// The value of one cell.
struct Cell;
impl Query<Grid> for Cell {
    type Key = u32;
    type Value = i64;
    const NAME: &'static str = "cell";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, key: &u32) -> Result<i64, Cycle> {
        cx.read(key);
        Ok(cx.db().value(*key))
    }
}

/// The sum of the cells `0..n`, over `Cell` memos.
struct Sum;
impl Query<Grid> for Sum {
    type Key = u32;
    type Value = i64;
    const NAME: &'static str = "sum";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, key: &u32) -> Result<i64, Cycle> {
        let mut s = 0;
        for k in 0..*key {
            s += *cx.get::<Cell>(&k)?;
        }
        Ok(s)
    }
}

/// What one run of the incremental hook saw and did.
#[derive(Clone, Debug)]
struct HookRun {
    had_previous: bool,
    recomputed: usize,
    touched: Option<Vec<u32>>,
    via: Option<Vec<DepKey<u32>>>,
}

thread_local! {
    static HOOK_RUNS: RefCell<Vec<HookRun>> = const { RefCell::new(Vec::new()) };
}

fn clear_hook_runs() {
    HOOK_RUNS.with(|r| r.borrow_mut().clear());
}

fn hook_runs() -> Vec<HookRun> {
    HOOK_RUNS.with(|r| r.borrow().clone())
}

/// The sum of the ten cells of group `g`, reading each.
fn group_sum(cx: &mut Cx<'_, Grid>, g: u32) -> i64 {
    let mut s = 0;
    for k in g * 10..g * 10 + 10 {
        cx.read(&k);
        s += cx.db().value(k);
    }
    s
}

/// Every group, from scratch (the reference).
fn groups_full(cx: &mut Cx<'_, Grid>) -> BTreeMap<u32, i64> {
    let mut out = BTreeMap::new();
    for g in 0..GROUPS {
        out.insert(g, group_sum(cx, g));
    }
    out
}

/// The incremental hook shared by `Groups` and `GroupsLru`: recomputes the groups of the touched fine cells,
/// and everything when it cannot tell (no previous value, no touched list, or a group key without a fine key).
fn groups_incremental(
    cx: &mut Cx<'_, Grid>,
    previous: Option<(&BTreeMap<u32, i64>, &Changes<u32>)>,
) -> BTreeMap<u32, i64> {
    let plan = previous.and_then(|(prev, changes)| {
        let touched = changes.touched.as_ref()?;
        let mut fine = BTreeSet::new();
        let mut coarse = BTreeSet::new();
        for &k in touched {
            if k < 1000 {
                if k / 10 < GROUPS {
                    fine.insert(k / 10);
                }
            } else if (1000..1000 + GROUPS).contains(&k) {
                coarse.insert(k - 1000);
            }
        }
        coarse.is_subset(&fine).then_some((prev, fine))
    });
    let log = |recomputed: usize| HookRun {
        had_previous: previous.is_some(),
        recomputed,
        touched: previous.and_then(|(_, c)| c.touched.clone()),
        via: previous.and_then(|(_, c)| c.via.clone()),
    };
    let (value, run) = match plan {
        Some((prev, groups)) => {
            let mut value = prev.clone();
            let mut changed = Vec::new();
            for &g in &groups {
                let s = group_sum(cx, g);
                if value.insert(g, s) != Some(s) {
                    changed.push(part_key(g));
                }
            }
            cx.changed_parts(changed);
            (value, log(groups.len()))
        }
        None => (groups_full(cx), log(GROUPS as usize)),
    };
    HOOK_RUNS.with(|r| r.borrow_mut().push(run));
    value
}

/// The sum of every group over the cells `0..40`; the incremental producer of parts.
struct Groups;
impl Query<Grid> for Groups {
    type Key = ();
    type Value = BTreeMap<u32, i64>;
    const NAME: &'static str = "groups";
    const INCREMENTAL: bool = true;
    fn equivalence() -> Equivalence<Self::Value> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, _: &()) -> Result<Self::Value, Cycle> {
        Ok(groups_full(cx))
    }
    fn execute_incremental(
        cx: &mut Cx<'_, Grid>,
        _: &(),
        previous: Option<(&Self::Value, &Changes<u32>)>,
    ) -> Result<Self::Value, Cycle> {
        Ok(groups_incremental(cx, previous))
    }
}

/// `Groups` under the LRU, with several keys that share the one body.
struct GroupsLru;
impl Query<Grid> for GroupsLru {
    type Key = u8;
    type Value = BTreeMap<u32, i64>;
    const NAME: &'static str = "groups_lru";
    const LRU: bool = true;
    const INCREMENTAL: bool = true;
    fn equivalence() -> Equivalence<Self::Value> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, _: &u8) -> Result<Self::Value, Cycle> {
        Ok(groups_full(cx))
    }
    fn execute_incremental(
        cx: &mut Cx<'_, Grid>,
        _: &u8,
        previous: Option<(&Self::Value, &Changes<u32>)>,
    ) -> Result<Self::Value, Cycle> {
        Ok(groups_incremental(cx, previous))
    }
}

/// One group's sum, reading only its part of `Groups`.
struct GroupOf;
impl Query<Grid> for GroupOf {
    type Key = u32;
    type Value = i64;
    const NAME: &'static str = "group_of";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, g: &u32) -> Result<i64, Cycle> {
        let groups = cx.get_part::<Groups>(&(), &part_key(*g))?;
        Ok(groups.get(g).copied().unwrap_or(0))
    }
}

/// The sum of all groups, reading `Groups` whole.
struct Total;
impl Query<Grid> for Total {
    type Key = ();
    type Value = i64;
    const NAME: &'static str = "total";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, _: &()) -> Result<i64, Cycle> {
        Ok(cx.get::<Groups>(&())?.values().sum())
    }
}

/// Cell `k` plus cell `k + 1`, reading through the store handle (host-driven recording) when memoised.
struct Reenter;
impl Query<Grid> for Reenter {
    type Key = u32;
    type Value = i64;
    const NAME: &'static str = "reenter";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, k: &u32) -> Result<i64, Cycle> {
        let db = cx.db();
        if cx.is_cold() {
            cx.read(k);
            let next = *cx.get::<Cell>(&(k + 1))?;
            return Ok(db.value(*k) + next);
        }
        let store = cx.store().expect("memoised");
        store.read(db, k);
        let next = *store.get::<Cell>(db, &(k + 1))?;
        Ok(db.value(*k) + next)
    }
}

/// `VB(k) + 1`; the other half of a cycle that `VB` catches.
struct VA;
impl Query<Grid> for VA {
    type Key = u32;
    type Value = i64;
    const NAME: &'static str = "va";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, k: &u32) -> Result<i64, Cycle> {
        Ok(*cx.get::<VB>(k)? + 1)
    }
}

/// `VA(k)`, or 0 and volatile when reading `VA(k)` closed a cycle.
struct VB;
impl Query<Grid> for VB {
    type Key = u32;
    type Value = i64;
    const NAME: &'static str = "vb";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, k: &u32) -> Result<i64, Cycle> {
        match cx.get::<VA>(k) {
            Ok(v) => Ok(*v),
            Err(_) => {
                cx.set_volatile();
                Ok(0)
            }
        }
    }
}

/// The part key of half `h` (0 or 1) of a `Prod` value.
fn half_key(h: u32) -> u32 {
    3000 + h
}

/// `Prod(k)`: the sums of the two halves of group `k` (cells `10k..10k + 5` and `10k + 5..10k + 10`), parts
/// [`half_key`] 0 and 1. Not incremental: a changed value declares no parts, so every part reader is marked.
struct Prod;
impl Query<Grid> for Prod {
    type Key = u32;
    type Value = [i64; 2];
    const NAME: &'static str = "prod";
    fn equivalence() -> Equivalence<[i64; 2]> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, k: &u32) -> Result<[i64; 2], Cycle> {
        let mut halves = [0, 0];
        for c in k * 10..k * 10 + 10 {
            cx.read(&c);
            halves[usize::from(c % 10 >= 5)] += cx.db().value(c);
        }
        Ok(halves)
    }
}

/// The cell that chooses `Mid`'s producer.
const CHOOSER: u32 = 44;

/// The parity of half 0 of `Prod(2)` when cell [`CHOOSER`] is odd, of `Prod(1)` otherwise: a reader whose
/// producer depends on data, read through a part; the parity makes producer switches and cut-offs frequent.
struct Mid;
impl Query<Grid> for Mid {
    type Key = ();
    type Value = i64;
    const NAME: &'static str = "mid";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, _: &()) -> Result<i64, Cycle> {
        cx.read(&CHOOSER);
        let producer = if cx.db().value(CHOOSER).rem_euclid(2) == 1 {
            2
        } else {
            1
        };
        let halves = cx.get_part::<Prod>(&producer, &half_key(0))?;
        Ok(halves[0].rem_euclid(2))
    }
}

/// Ten times `Mid`, reading it whole: its producers are `Mid`'s.
struct Top;
impl Query<Grid> for Top {
    type Key = ();
    type Value = i64;
    const NAME: &'static str = "top";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, _: &()) -> Result<i64, Cycle> {
        Ok(*cx.get::<Mid>(&())? * 10)
    }
}

/// Twice cell `k`, over its `Cell` memo (so at the cell's durability).
struct Twice;
impl Query<Grid> for Twice {
    type Key = u32;
    type Value = i64;
    const NAME: &'static str = "twice";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, k: &u32) -> Result<i64, Cycle> {
        Ok(2 * *cx.get::<Cell>(k)?)
    }
}

/// Cell 0 when cell [`CHOOSER`] is odd, else 0, over `Cell` memos.
fn pick(cx: &mut Cx<'_, Grid>) -> Result<i64, Cycle> {
    if cx.get::<Cell>(&CHOOSER)?.rem_euclid(2) == 1 {
        Ok(*cx.get::<Cell>(&0)?)
    } else {
        Ok(0)
    }
}

/// [`pick`] as an incremental family: the hook re-reads only what its new value needs, so a `via` entry
/// naming `Cell(0)` is accounted for without a read while the chooser is even (a marked memo dependency left
/// marked by an incremental execution).
struct Pick;
impl Query<Grid> for Pick {
    type Key = ();
    type Value = i64;
    const NAME: &'static str = "pick";
    const INCREMENTAL: bool = true;
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, _: &()) -> Result<i64, Cycle> {
        pick(cx)
    }
    fn execute_incremental(
        cx: &mut Cx<'_, Grid>,
        _: &(),
        _previous: Option<(&i64, &Changes<u32>)>,
    ) -> Result<i64, Cycle> {
        pick(cx)
    }
}

// ---------------------------------------------------------------------------------------------------------
// Helpers over the families

/// Asserts that the memoised read equals the cold one.
fn assert_cold<Q: Query<Grid>>(store: &Store<Grid>, db: &Grid, key: &Q::Key)
where
    Q::Value: PartialEq,
{
    let memo = store.get::<Q>(db, key).unwrap();
    let cold = store.cold::<Q>(db, key).unwrap();
    assert_eq!(*memo, *cold, "{}({key:?})", Q::NAME);
}

/// Reads and checks `Sum(39)`, `GroupOf(0..4)` and `Total`.
fn read_all(store: &Store<Grid>, db: &Grid) {
    assert_cold::<Sum>(store, db, &39);
    for g in 0..GROUPS {
        assert_cold::<GroupOf>(store, db, &g);
    }
    assert_cold::<Total>(store, db, &());
}

// ---------------------------------------------------------------------------------------------------------
// Tests

#[test]
fn push_skip_without_walk() {
    let (mut db, store) = setup();
    read_all(&store, &db);
    clear_hook_runs();

    db.commit_push(&store, &[set(25, 100)]);
    read_all(&store, &db);

    let gof = counters(&store, "group_of");
    assert_eq!((gof.push_skips, gof.re_executions, gof.verifications), (3, 1, 0));
    let groups = counters(&store, "groups");
    assert_eq!((groups.incremental, groups.re_executions), (1, 1));
    let runs = hook_runs();
    assert_eq!(runs.len(), 1);
    assert!(runs[0].had_previous);
    assert_eq!(runs[0].recomputed, 1, "only group 2 was recomputed");
    assert_eq!(counters(&store, "total").re_executions, 1);
    // `Cell(25)` re-executes; the other cells of group 2 are marked and verified; the rest are push skips.
    let cell = counters(&store, "cell");
    assert_eq!(
        (cell.re_executions, cell.verifications, cell.push_skips),
        (1, 9, 29)
    );
    assert_eq!(counters(&store, "sum").re_executions, 1);
}

/// Review cycle 9, finding 1: a reader whose memo dependency switched producer and cut off must settle the new
/// producer, or a change of the new producer's part is never pushed to it.
#[test]
fn a_switched_producer_is_settled_after_a_cut_off() {
    let (mut db, store) = setup();
    // Cell 44 holds 2: `Mid` reads half 0 of `Prod(1)` (3 + 4 + 5 + 6 + 0, even).
    assert_eq!(*store.get::<Top>(&db, &()).unwrap(), 0);

    // Switch to `Prod(2)` (6 + 0 + 1 + 2 + 3, even): `Mid` re-executes and cuts off, `Top` is verified green.
    db.commit_push(&store, &[set(CHOOSER, 1)]);
    assert_eq!(*store.get::<Top>(&db, &()).unwrap(), 0);
    assert_eq!(counters(&store, "mid").cut_offs, 1);
    assert_eq!(counters(&store, "top").verifications, 1);

    // Half 0 of the new producer turns odd: `Top` must see it.
    db.commit_push(&store, &[set(20, 7)]);
    assert_cold::<Top>(&store, &db, &());
    assert_eq!(*store.get::<Top>(&db, &()).unwrap(), 10);
    assert_eq!(counters(&store, "top").re_executions, 1);

    // The old producer no longer reaches `Top`.
    db.commit_push(&store, &[set(10, 4)]);
    assert_cold::<Top>(&store, &db, &());
    let top = counters(&store, "top");
    assert_eq!((top.push_skips, top.re_executions), (1, 0));
}

/// Review cycle 9, finding 4: only incremental families collect `via` keys.
#[test]
fn via_is_kept_for_incremental_families_only() {
    let (mut db, store) = setup();
    read_all(&store, &db);
    assert_cold::<Groups>(&store, &db, &());
    db.commit_push(&store, &[set(25, 100)]);
    let marks = store.pending_marks();
    assert!(marks.iter().any(|(f, _)| *f == "cell"), "{marks:?}");
    for (family, via) in &marks {
        assert_eq!(*via, *family == "groups", "{family}: {marks:?}");
    }
}

/// Review cycle 9, finding 4: a commit does not walk again the readers of a memo whose pending mark already
/// walked them; the values stay right.
#[test]
fn a_marked_memo_is_not_walked_again() {
    let (mut db, store) = setup();
    read_all(&store, &db);
    db.commit_push(&store, &[set(25, 100)]);
    let first = store.commit_stats().total().marked;
    assert!(first > 0);
    // Same group, nothing read in between: every memo it reaches is already marked and walked.
    db.commit_push(&store, &[set(26, 100)]);
    assert_eq!(store.commit_stats().total().marked, 0);
    read_all(&store, &db);
    assert_cold::<Cell>(&store, &db, &26);
    // Another group marks afresh.
    db.commit_push(&store, &[set(5, 100)]);
    assert!(store.commit_stats().total().marked > 0);
    read_all(&store, &db);
}

/// Review cycle 9, finding 4: a durability skip clears a reader's mark while its dependency stays marked;
/// the next commit that reaches the dependency must walk its readers again.
#[test]
fn a_durability_skip_keeps_marking_sound() {
    let (mut db, store) = setup();
    db.commit_push(&store, &[(30, 5, Durability::Library)]);
    assert_cold::<Twice>(&store, &db, &30);
    // A drawing change in the same group marks `Cell(30)` and `Twice(30)` through the group key; nothing of
    // their library durability changed, so `Twice(30)` is re-stamped and `Cell(30)` stays marked.
    db.commit_push(&store, &[set(31, 1)]);
    assert_cold::<Twice>(&store, &db, &30);
    assert_eq!(counters(&store, "twice").durability_skips, 1);
    // A library change of the still-marked cell.
    db.commit_push(&store, &[(30, 7, Durability::Library)]);
    assert_cold::<Twice>(&store, &db, &30);
    assert_eq!(*store.get::<Twice>(&db, &30).unwrap(), 14);
}

/// The cell `Noisy` returns.
const SIGNAL: u32 = 0;
/// The cell `Noisy` reads but ignores.
const NOISE: u32 = 1;

/// Reads [`NOISE`] (ignoring its value) and returns [`SIGNAL`].
struct Noisy;
impl Query<Grid> for Noisy {
    type Key = ();
    type Value = i64;
    const NAME: &'static str = "noisy";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, _: &()) -> Result<i64, Cycle> {
        cx.read(&NOISE);
        cx.read(&SIGNAL);
        Ok(cx.db().value(SIGNAL))
    }
}

/// An incremental family over `Noisy` whose hook knows that a change of [`NOISE`] alone cannot change its
/// value, and so leaves `Noisy` (named in its `via`) unread.
struct Follow;
impl Query<Grid> for Follow {
    type Key = ();
    type Value = i64;
    const NAME: &'static str = "follow";
    const INCREMENTAL: bool = true;
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, _: &()) -> Result<i64, Cycle> {
        Ok(*cx.get::<Noisy>(&())?)
    }
    fn execute_incremental(
        cx: &mut Cx<'_, Grid>,
        key: &(),
        previous: Option<(&i64, &Changes<u32>)>,
    ) -> Result<i64, Cycle> {
        if let Some((value, changes)) = previous
            && let Some(touched) = &changes.touched
            && touched.iter().any(|k| *k < 1000)
            && touched.iter().filter(|k| **k < 1000).all(|k| *k == NOISE)
        {
            return Ok(*value);
        }
        Self::execute(cx, key)
    }
}

/// A whole reader of the incremental `Follow`.
struct Reader;
impl Query<Grid> for Reader {
    type Key = ();
    type Value = i64;
    const NAME: &'static str = "follow_reader";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Grid>, _: &()) -> Result<i64, Cycle> {
        Ok(*cx.get::<Follow>(&())?)
    }
}

/// Runs the unread-`via` scenario; returns the memoised and the cold value of `Reader` at the end.
fn run_unread_via_scenario(bump: bool) -> (i64, i64) {
    let (mut db, store) = setup();
    if !bump {
        store.skip_incremental_epoch_bump();
    }
    db.commit_push(&store, &[set(SIGNAL, 3)]);
    assert_eq!(*store.get::<Reader>(&db, &()).unwrap(), 3);

    // Noise only: `Follow`'s hook keeps its value and leaves `Noisy` unread.
    db.commit_push(&store, &[set(NOISE, 100)]);
    assert_eq!(*store.get::<Reader>(&db, &()).unwrap(), 3);
    assert_eq!(counters(&store, "follow").incremental, 1);
    assert_eq!(counters(&store, "noisy").re_executions, 0);

    // A real change of the signal.
    db.commit_push(&store, &[set(SIGNAL, 9)]);
    (
        *store.get::<Reader>(&db, &()).unwrap(),
        *store.cold::<Reader>(&db, &()).unwrap(),
    )
}

/// Review cycle 9: an incremental hook that leaves its `via` memo unread; a later real change must still
/// reach a reader of the family. With the epoch bump disabled the reader is stale.
#[test]
fn an_unread_via_memo_still_reaches_its_readers() {
    assert_eq!(run_unread_via_scenario(true), (9, 9));
    // Proof that the scenario exercises the bump: without it the memoised reader keeps the old value.
    let (memo, cold) = run_unread_via_scenario(false);
    assert_eq!((memo, cold), (3, 9));
}

/// Rows `k -> (value, page, changed_at)` whose coarse key is their page (`100 + page`); a row can move page.
#[derive(Default)]
struct Rows {
    rows: BTreeMap<u32, (i64, u32, Rev)>,
    rev: Rev,
}

impl Db for Rows {
    type Input = u32;
    fn changed_at(&self, k: &u32) -> Rev {
        self.rows.get(k).map_or(Rev::ZERO, |r| r.2)
    }
    fn durability(&self, _: &u32) -> Durability {
        Durability::Drawing
    }
    fn coarse(&self, k: &u32) -> u32 {
        if *k >= 100 {
            *k
        } else {
            self.rows.get(k).map_or(100, |r| 100 + r.1)
        }
    }
}

impl Rows {
    /// Puts row `k` on `page` and reports it with its page keys before and after.
    fn put(&mut self, store: &Store<Rows>, k: u32, value: i64, page: u32) {
        let before = self.coarse(&k);
        self.rev = Rev(self.rev.0 + 1);
        self.rows.insert(k, (value, page, self.rev));
        let after = self.coarse(&k);
        store.advance_with(self.rev, Durability::Drawing, [k, before, after]);
    }
}

/// The value of a row.
struct RowValue;
impl Query<Rows> for RowValue {
    type Key = u32;
    type Value = i64;
    const NAME: &'static str = "row_value";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Rows>, k: &u32) -> Result<i64, Cycle> {
        cx.read(k);
        Ok(cx.db().rows.get(k).map_or(0, |r| r.0))
    }
}

/// Review cycle 9, finding 5: a re-execution after a row moved page drops the old page's registration (the
/// registrations are kept with the memo, not recomputed in the new snapshot).
#[test]
fn a_moved_row_leaves_no_registration_behind() {
    let mut db = Rows::default();
    let store = Store::new();
    db.put(&store, 1, 5, 1);
    assert_eq!(*store.get::<RowValue>(&db, &1).unwrap(), 5);
    assert_eq!(store.push_index_entries(), 1);
    db.put(&store, 1, 6, 2);
    assert_eq!(*store.get::<RowValue>(&db, &1).unwrap(), 6);
    assert_eq!(
        store.push_index_entries(),
        1,
        "the old page's registration is dropped"
    );
    // A change on the old page no longer marks the memo.
    db.put(&store, 2, 0, 1);
    assert_eq!(*store.get::<RowValue>(&db, &1).unwrap(), 6);
    let c = store.commit_stats().family("row_value");
    assert_eq!((c.marked, c.push_skips), (0, 1));
}

#[test]
fn untouched_cell_counts_as_push_skip() {
    let (mut db, store) = setup();
    assert_cold::<Cell>(&store, &db, &3);
    assert_cold::<Cell>(&store, &db, &25);
    db.commit_push(&store, &[set(25, 100)]);
    assert_cold::<Cell>(&store, &db, &3);
    assert_cold::<Cell>(&store, &db, &25);
    let cell = counters(&store, "cell");
    assert_eq!(
        (cell.push_skips, cell.re_executions, cell.verifications),
        (1, 1, 0)
    );
    assert_eq!(cell.marked, 1, "only the cell of the touched group is marked");
}

#[test]
fn unchanged_group_sum_marks_no_part_reader() {
    let (mut db, store) = setup();
    read_all(&store, &db);
    clear_hook_runs();
    let before = store.memo_info::<Groups>(&()).unwrap();

    // Cells 10 and 11 hold 3 and 4: swap them, so group 1 keeps its sum.
    db.commit_push(&store, &[set(10, 4), set(11, 3)]);
    read_all(&store, &db);

    let groups = counters(&store, "groups");
    assert_eq!(
        (groups.re_executions, groups.incremental, groups.cut_offs),
        (1, 1, 1)
    );
    assert_eq!(
        store.memo_info::<Groups>(&()).unwrap().changed_at,
        before.changed_at
    );
    assert_eq!(hook_runs().len(), 1);
    assert_eq!(hook_runs()[0].recomputed, 1);
    let gof = counters(&store, "group_of");
    assert_eq!(
        (gof.push_skips, gof.re_executions),
        (4, 0),
        "no part was declared changed"
    );
    let total = counters(&store, "total");
    assert_eq!((total.re_executions, total.verifications), (0, 1));
}

#[test]
fn coarse_keys_alone_mark() {
    let (mut db, store) = setup();
    read_all(&store, &db);
    assert_cold::<Groups>(&store, &db, &());
    clear_hook_runs();

    db.commit_coarse_only(&store, &[set(25, 100)]);
    read_all(&store, &db);
    assert_cold::<Cell>(&store, &db, &25);
    assert_cold::<Groups>(&store, &db, &());

    let runs = hook_runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].touched, Some(vec![coarse_key(25)]));
    assert_eq!(
        runs[0].recomputed, GROUPS as usize,
        "no fine key: everything is recomputed"
    );
    assert_eq!(counters(&store, "cell").re_executions, 1);
    assert_eq!(counters(&store, "sum").re_executions, 1);
    assert_eq!(counters(&store, "total").re_executions, 1);
    // The full recomputation declares no parts, so every part reader of a changed value re-executes.
    assert_eq!(counters(&store, "group_of").re_executions, 4);

    // A group key of a group the hook does not read does not mark it.
    clear_hook_runs();
    db.commit_coarse_only(&store, &[set(45, 9)]);
    read_all(&store, &db);
    assert_cold::<Groups>(&store, &db, &());
    assert!(hook_runs().is_empty());
    assert_eq!(counters(&store, "groups").push_skips, 1);
}

#[test]
fn pull_mode_after_plain_advance() {
    let (mut db, store) = setup();
    read_all(&store, &db);
    clear_hook_runs();

    db.commit_pull(&store, &[set(25, 100)]);
    read_all(&store, &db);
    let all = store.commit_stats().total();
    assert_eq!(all.push_skips, 0);
    // The only marks are the producer's own: `Groups` changed without declaring parts, so its four part
    // readers are marked.
    assert_eq!(all.marked, 4);
    assert_eq!(counters(&store, "group_of").marked, 4);
    assert!(all.verifications > 0 && all.re_executions > 0);
    // Without push marks the hook runs from scratch, as a non-incremental re-execution.
    assert_eq!(counters(&store, "groups").incremental, 0);
    let runs = hook_runs();
    assert_eq!(runs.len(), 1);
    assert!(!runs[0].had_previous);

    // Memos verified after the plain advance are covered by the next push commit.
    clear_hook_runs();
    db.commit_push(&store, &[set(5, 100)]);
    read_all(&store, &db);
    assert_cold::<Cell>(&store, &db, &45);
    let all = store.commit_stats().total();
    assert!(all.push_skips > 0);
    assert_eq!(counters(&store, "groups").incremental, 1);
    assert_eq!(counters(&store, "group_of").push_skips, 3);
    assert_eq!(hook_runs().len(), 1);
    assert_eq!(hook_runs()[0].recomputed, 1);
}

#[test]
fn incremental_without_previous_value_is_full() {
    let (mut db, store) = setup();
    store.set_lru_budget(Some(1));
    assert_cold::<GroupsLru>(&store, &db, &0);
    assert_cold::<GroupsLru>(&store, &db, &1); // evicts key 0
    assert!(!store.memo_info::<GroupsLru>(&0).unwrap().has_value);
    assert!(store.memo_info::<GroupsLru>(&1).unwrap().has_value);
    clear_hook_runs();

    db.commit_push(&store, &[set(12, 100)]);
    assert_cold::<GroupsLru>(&store, &db, &1);
    assert_cold::<GroupsLru>(&store, &db, &0);

    let runs = hook_runs();
    assert_eq!(runs.len(), 2);
    assert!(
        runs[0].had_previous && runs[0].recomputed == 1,
        "a held value: incremental"
    );
    assert!(
        !runs[1].had_previous && runs[1].recomputed == GROUPS as usize,
        "an evicted value: full"
    );
    assert_eq!(counters(&store, "groups_lru").incremental, 1);
}

#[test]
fn changes_name_touched_and_via() {
    let (mut db, store) = setup();
    assert_cold::<Groups>(&store, &db, &());
    clear_hook_runs();

    db.commit_push(&store, &[set(12, 100)]);
    assert_cold::<Groups>(&store, &db, &());

    let runs = hook_runs();
    assert_eq!(runs.len(), 1);
    let touched = runs[0].touched.as_ref().expect("touched is known");
    assert!(touched.contains(&12) && touched.contains(&1001), "{touched:?}");
    let via = runs[0].via.as_ref().expect("via is known");
    assert!(via.contains(&DepKey::Input(1001)), "{via:?}");
}

#[test]
fn reentrant_reads_are_recorded() {
    let (mut db, store) = setup();
    assert_cold::<Reenter>(&store, &db, &5);
    let info = store.memo_info::<Reenter>(&5).unwrap();
    let dep_keys: Vec<_> = info.deps.iter().map(|(k, _)| k.clone()).collect();
    assert!(dep_keys.contains(&DepKey::Input(5)), "{dep_keys:?}");
    let cell6 = store.memo_key::<Cell>(&6).unwrap();
    assert!(dep_keys.contains(&cell6), "{dep_keys:?}");

    db.commit_push(&store, &[set(6, 100)]);
    assert_cold::<Reenter>(&store, &db, &5);
    assert_eq!(*store.get::<Reenter>(&db, &5).unwrap(), 5 + 100);
    assert_eq!(counters(&store, "reenter").re_executions, 1);

    db.commit_push(&store, &[set(50, 100)]);
    assert_cold::<Reenter>(&store, &db, &5);
    let c = counters(&store, "reenter");
    assert_eq!((c.push_skips, c.re_executions), (1, 0));
}

#[test]
fn volatile_values_are_not_kept() {
    let (db, store) = setup();
    store.set_cycle_panics(false);
    assert_cold::<VA>(&store, &db, &1);
    assert_cold::<VB>(&store, &db, &1);
    assert!(store.memo_info::<VA>(&1).is_none());
    assert!(store.memo_info::<VB>(&1).is_none());
    let (va, vb) = (store.total_stats().family("va"), store.total_stats().family("vb"));
    // Review cycle 9, finding 11: the executions that caught the cycle are counted as volatile.
    assert!(va.volatile >= 1 && vb.volatile >= 1, "{va:?} {vb:?}");
    assert!(va.volatile <= va.re_executions && vb.volatile <= vb.re_executions);

    assert_cold::<VA>(&store, &db, &1);
    assert_cold::<VB>(&store, &db, &1);
    assert!(store.memo_info::<VA>(&1).is_none());
    assert!(store.memo_info::<VB>(&1).is_none());
    let (va2, vb2) = (store.total_stats().family("va"), store.total_stats().family("vb"));
    assert!(va2.re_executions > va.re_executions, "{va:?} -> {va2:?}");
    assert!(vb2.re_executions > vb.re_executions, "{vb:?} -> {vb2:?}");
    assert!(va2.volatile > va.volatile && vb2.volatile > vb.volatile);
    assert_eq!((va2.hits, vb2.hits), (0, 0));
}

#[test]
fn key_of_maps_via_entries() {
    let (db, store) = setup();
    read_all(&store, &db);
    let cell7 = store.memo_key::<Cell>(&7).unwrap();
    assert_eq!(store.key_of::<Cell>(&cell7), Some(7));
    assert_eq!(store.key_of::<Cell>(&DepKey::Input(7)), None);
    let sum = store.memo_key::<Sum>(&39).unwrap();
    assert_eq!(store.key_of::<Sum>(&sum), Some(39));
    assert_eq!(store.key_of::<Cell>(&sum), None, "a memo of another family");
    assert_eq!(store.key_of::<Sum>(&cell7), None);

    // A part dependency names its producer's memo.
    let info = store.memo_info::<GroupOf>(&2).unwrap();
    let (part, _) = info
        .deps
        .iter()
        .find(|(k, _)| matches!(k, DepKey::Part(..)))
        .expect("a part dependency");
    assert_eq!(store.key_of::<Groups>(part), Some(()));
    assert_eq!(store.key_of::<Cell>(part), None);
}

#[test]
fn fork_keeps_push_state() {
    let (mut db, mut store) = setup();
    read_all(&store, &db);

    // A fork that reads a new key and is adopted while the parent did not move.
    let fork = store.fork();
    assert_cold::<Cell>(&fork, &db, &44);
    assert!(store.adopt_if_unchanged(fork));
    assert!(store.memo_key::<Cell>(&44).is_some());

    // A fork on its own snapshot of the database, taken before the parent advances.
    let mut fork_db = db.clone();
    let fork = store.fork();
    db.commit_push(&store, &[set(25, 100)]);
    read_all(&store, &db);
    assert_eq!(counters(&store, "group_of").push_skips, 3);

    // The fork still reads its own snapshot, and moves on with push commits of its own.
    read_all(&fork, &fork_db);
    assert_eq!(
        *fork.get::<Total>(&fork_db, &()).unwrap(),
        (0..40).map(|k| i64::from(k % 7)).sum::<i64>()
    );
    fork_db.commit_push(&fork, &[set(3, 50)]);
    clear_hook_runs();
    read_all(&fork, &fork_db);
    assert_eq!(counters(&fork, "groups").incremental, 1);
    assert_eq!(counters(&fork, "group_of").push_skips, 3);
    assert_eq!(hook_runs().len(), 1);

    // The parent advanced after the fork: adopting is refused, and neither side is disturbed.
    assert!(!store.adopt_if_unchanged(fork));
    read_all(&store, &db);
    assert_eq!(*store.get::<Cell>(&db, &25).unwrap(), 100);
}

// ---------------------------------------------------------------------------------------------------------
// Push equals cold

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Style {
    Push,
    CoarseOnly,
    Pull,
}

#[derive(Clone, Debug)]
enum Step {
    Set(u32, i64, u8),
    /// Re-sets a cell to its current value at another durability.
    Move(u32, u8),
    Read(u8, u32),
    Commit(Style),
    Fork,
    ReadFork(u8, u32),
    Adopt,
}

fn style() -> impl Strategy<Value = Style> {
    prop_oneof![Just(Style::Push), Just(Style::CoarseOnly), Just(Style::Pull)]
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        4 => (0u32..45, -3i64..4, 0u8..3).prop_map(|(k, v, d)| Step::Set(k, v, d)),
        // The chooser of `Mid`, and the halves 0 of its two producers, more often than other cells.
        1 => (-3i64..4, 0u8..3).prop_map(|(v, d)| Step::Set(CHOOSER, v, d)),
        1 => (prop_oneof![10u32..15, 20u32..25], -3i64..4).prop_map(|(k, v)| Step::Set(k, v, 0)),
        1 => (0u32..45, 0u8..3).prop_map(|(k, d)| Step::Move(k, d)),
        6 => (0u8..FAMILIES, 0u32..45).prop_map(|(q, k)| Step::Read(q, k)),
        2 => style().prop_map(Step::Commit),
        1 => Just(Step::Fork),
        3 => (0u8..FAMILIES, 0u32..45).prop_map(|(q, k)| Step::ReadFork(q, k)),
        1 => Just(Step::Adopt),
    ]
}

const FAMILIES: u8 = 11;

fn debug<T: Debug>(value: &T) -> String {
    format!("{value:?}")
}

fn read_family(store: &Store<Grid>, db: &Grid, q: u8, k: u32) -> String {
    match q {
        0 => debug(&store.get::<Cell>(db, &k).unwrap()),
        1 => debug(&store.get::<Sum>(db, &k).unwrap()),
        2 => debug(&store.get::<GroupOf>(db, &(k % 4)).unwrap()),
        3 => debug(&store.get::<Total>(db, &()).unwrap()),
        4 => debug(&store.get::<Groups>(db, &()).unwrap()),
        5 => debug(&store.get::<Reenter>(db, &k.min(44)).unwrap()),
        6 => debug(&store.get::<Prod>(db, &(1 + k % 2)).unwrap()),
        7 => debug(&store.get::<Mid>(db, &()).unwrap()),
        8 => debug(&store.get::<Top>(db, &()).unwrap()),
        9 => debug(&store.get::<Twice>(db, &k).unwrap()),
        _ => debug(&store.get::<Pick>(db, &()).unwrap()),
    }
}

fn cold_family(store: &Store<Grid>, db: &Grid, q: u8, k: u32) -> String {
    match q {
        0 => debug(&store.cold::<Cell>(db, &k).unwrap()),
        1 => debug(&store.cold::<Sum>(db, &k).unwrap()),
        2 => debug(&store.cold::<GroupOf>(db, &(k % 4)).unwrap()),
        3 => debug(&store.cold::<Total>(db, &()).unwrap()),
        4 => debug(&store.cold::<Groups>(db, &()).unwrap()),
        5 => debug(&store.cold::<Reenter>(db, &k.min(44)).unwrap()),
        6 => debug(&store.cold::<Prod>(db, &(1 + k % 2)).unwrap()),
        7 => debug(&store.cold::<Mid>(db, &()).unwrap()),
        8 => debug(&store.cold::<Top>(db, &()).unwrap()),
        9 => debug(&store.cold::<Twice>(db, &k).unwrap()),
        _ => debug(&store.cold::<Pick>(db, &()).unwrap()),
    }
}

fn commit_styled(db: &mut Grid, store: &Store<Grid>, style: Style, changes: &[Change]) {
    match style {
        Style::Push => db.commit_push(store, changes),
        Style::CoarseOnly => db.commit_coarse_only(store, changes),
        Style::Pull => db.commit_pull(store, changes),
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// Under random sequences of changes (any durability, durability moves) committed by push, by group keys
    /// only, or by a plain advance, with forks read against a snapshot and offered back, every memoised read
    /// equals the cold one, oracle mode finds no mismatch and the revision never decreases. The families
    /// include a reader whose producer depends on data (`Mid`, chosen by cell 44, read through a part) and a
    /// whole reader of it (`Top`), so a producer switch with a cut-off is exercised (review cycle 9).
    #[test]
    fn push_equals_cold(steps in proptest::collection::vec(step(), 1..60)) {
        let mut db = Grid::default();
        let mut store = Store::new();
        store.set_oracle(Oracle::Every);
        let mut pending: Vec<Change> = Vec::new();
        let mut last_style = Style::Push;
        let mut held: Option<(Store<Grid>, Grid)> = None;
        let mut committed_since_fork = false;
        let mut last_current = store.current();
        for s in steps {
            match s {
                Step::Set(k, v, d) => pending.push((k, v, Durability::ALL[usize::from(d)])),
                Step::Move(k, d) => {
                    let value = pending
                        .iter()
                        .rev()
                        .find(|(pk, _, _)| *pk == k)
                        .map_or_else(|| db.value(k), |(_, v, _)| *v);
                    pending.push((k, value, Durability::ALL[usize::from(d)]));
                }
                Step::Commit(style) => {
                    last_style = style;
                    commit_styled(&mut db, &store, style, &std::mem::take(&mut pending));
                    committed_since_fork = true;
                }
                Step::Read(q, k) => {
                    if !pending.is_empty() {
                        commit_styled(&mut db, &store, last_style, &std::mem::take(&mut pending));
                        committed_since_fork = true;
                    }
                    let memo = read_family(&store, &db, q, k);
                    prop_assert_eq!(memo, cold_family(&store, &db, q, k));
                }
                Step::Fork => {
                    held = Some((store.fork(), db.clone()));
                    committed_since_fork = false;
                }
                Step::ReadFork(q, k) => {
                    if let Some((fork, snapshot)) = held.as_mut() {
                        let memo = read_family(fork, snapshot, q, k);
                        prop_assert_eq!(memo, cold_family(fork, snapshot, q, k));
                        prop_assert_eq!(fork.take_mismatches(), Vec::<Mismatch>::new());
                    }
                }
                Step::Adopt => {
                    if let Some((fork, _)) = held.take() {
                        let before = store.current();
                        let adopted = store.adopt_if_unchanged(fork);
                        prop_assert_eq!(store.current(), before, "adopt does not move the revision");
                        if committed_since_fork {
                            prop_assert!(!adopted, "a fork that missed a commit is refused");
                        }
                        for q in 0..FAMILIES {
                            for k in [0, 13, 44] {
                                let memo = read_family(&store, &db, q, k);
                                prop_assert_eq!(memo, cold_family(&store, &db, q, k));
                            }
                        }
                    }
                }
            }
            prop_assert!(store.current() >= last_current, "the revision never decreases");
            last_current = store.current();
        }
        prop_assert_eq!(store.take_mismatches(), Vec::<Mismatch>::new());
    }
}
