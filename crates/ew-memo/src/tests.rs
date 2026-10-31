// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Engine tests over a toy database: cells of integers with revision stamps and durabilities.

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use proptest::prelude::*;

use crate::{Counters, Cx, Cycle, Db, DepKey, Durability, Equivalence, Mismatch, Oracle, Query, Rev, Store};

/// Cells `k -> (value, changed_at, durability)`; `cycle` makes `Cyc` read itself.
#[derive(Clone, Default)]
struct Cells {
    cells: BTreeMap<u32, (i64, Rev, Durability)>,
    rev: Rev,
}

impl Db for Cells {
    type Input = u32;
    fn changed_at(&self, input: &u32) -> Rev {
        // An absent cell reads as 0, changed at revision 0 (it never changes without a `set`).
        self.cells.get(input).map_or(Rev::ZERO, |c| c.1)
    }
    fn durability(&self, input: &u32) -> Durability {
        self.cells.get(input).map_or(Durability::Drawing, |c| c.2)
    }
}

impl Cells {
    fn value(&self, k: u32) -> i64 {
        self.cells.get(&k).map_or(0, |c| c.0)
    }
    /// One commit: sets the cells and advances the store.
    fn commit(&mut self, store: &mut Store<Cells>, changes: &[(u32, i64, Durability)]) {
        self.rev = Rev(self.rev.0 + 1);
        let mut most = Durability::Drawing;
        for &(k, v, d) in changes {
            // The commit's durability covers the input's level before and after the change.
            let before = self.durability(&k);
            self.cells.insert(k, (v, self.rev, d));
            most = most.max(d).max(before);
        }
        store.advance(self.rev, most);
    }
}

/// The value of one cell.
struct Cell;
impl Query<Cells> for Cell {
    type Key = u32;
    type Value = i64;
    const NAME: &'static str = "cell";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Cells>, key: &u32) -> Result<i64, Cycle> {
        cx.read(key);
        Ok(cx.db().value(*key))
    }
}

/// The sum of the cells `0..n`.
struct Sum;
impl Query<Cells> for Sum {
    type Key = u32;
    type Value = i64;
    const NAME: &'static str = "sum";
    const RDEPS: bool = true;
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Cells>, key: &u32) -> Result<i64, Cycle> {
        let mut s = 0;
        for k in 0..*key {
            s += *cx.get::<Cell>(&k)?;
        }
        Ok(s)
    }
}

/// Whether `Sum(n)` is positive: changes rarely, the cut-off point.
struct Positive;
impl Query<Cells> for Positive {
    type Key = u32;
    type Value = bool;
    const NAME: &'static str = "positive";
    fn equivalence() -> Equivalence<bool> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Cells>, key: &u32) -> Result<bool, Cycle> {
        Ok(*cx.get::<Sum>(key)? > 0)
    }
}

/// A label that depends on `Positive(n)` only.
struct Label;
impl Query<Cells> for Label {
    type Key = u32;
    type Value = String;
    const NAME: &'static str = "label";
    fn equivalence() -> Equivalence<String> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Cells>, key: &u32) -> Result<String, Cycle> {
        let p = cx.get::<Positive>(key)?;
        Ok(format!("{key}:{}", if *p { "+" } else { "-" }))
    }
}

/// A bulky value under the LRU, compared by fingerprint: `n` copies of cell `n`.
struct Bulk;
impl Query<Cells> for Bulk {
    type Key = u32;
    type Value = Vec<i64>;
    const NAME: &'static str = "bulk";
    const LRU: bool = true;
    fn equivalence() -> Equivalence<Vec<i64>> {
        Equivalence::Hash128(|v| {
            // A deterministic toy fingerprint: 128-bit FNV-1a over the bytes of the elements. (Xor-ing a whole
            // sign-extended element per round collides, e.g. `[0; 4]` and `[-2; 4]`.)
            let mut h: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
            for x in v {
                for byte in x.to_le_bytes() {
                    h ^= u128::from(byte);
                    h = h.wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013b);
                }
            }
            h
        })
    }
    fn bytes(value: &Vec<i64>) -> usize {
        value.len() * 8
    }
    fn execute(cx: &mut Cx<'_, Cells>, key: &u32) -> Result<Vec<i64>, Cycle> {
        let v = *cx.get::<Cell>(key)?;
        Ok(vec![v; *key as usize])
    }
}

/// Reads `Cyc(k)` again when cell 1000 is non-zero: a cycle on demand.
struct Cyc;
impl Query<Cells> for Cyc {
    type Key = u32;
    type Value = i64;
    const NAME: &'static str = "cyc";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Cells>, key: &u32) -> Result<i64, Cycle> {
        if *cx.get::<Cell>(&1000)? != 0 {
            return cx.get::<Cyc>(key).map(|v| *v);
        }
        Ok(i64::from(*key))
    }
}

/// A deliberately wrong query: it reads cell 7 without reporting the read.
struct Untracked;
impl Query<Cells> for Untracked {
    type Key = ();
    type Value = i64;
    const NAME: &'static str = "untracked";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Cells>, _: &()) -> Result<i64, Cycle> {
        Ok(cx.db().value(7))
    }
}

/// A partly tracked query: reads `Cell(0)` and input 3 (tracked) and cell 7 (untracked).
struct Partial;
impl Query<Cells> for Partial {
    type Key = ();
    type Value = i64;
    const NAME: &'static str = "partial";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Cells>, _: &()) -> Result<i64, Cycle> {
        let v = *cx.get::<Cell>(&0)?;
        cx.read(&3);
        Ok(v + cx.db().value(7))
    }
}

/// Panics on demand: when cell 2000 is non-zero (the host catches the panic).
struct Boom;
impl Query<Cells> for Boom {
    type Key = ();
    type Value = i64;
    const NAME: &'static str = "boom";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Cells>, _: &()) -> Result<i64, Cycle> {
        cx.read(&2000);
        assert!(cx.db().value(2000) == 0, "boom");
        Ok(7)
    }
}

fn counters(store: &Store<Cells>, family: &str) -> Counters {
    store.commit_stats().family(family)
}

fn setup(n: u32) -> (Cells, Store<Cells>) {
    let mut db = Cells::default();
    let mut store = Store::new();
    let changes: Vec<(u32, i64, Durability)> = (0..n)
        .map(|k| (k, i64::from(k) + 1, Durability::Drawing))
        .collect();
    db.commit(&mut store, &changes);
    (db, store)
}

#[test]
fn first_read_executes_second_read_hits() {
    let (db, store) = setup(4);
    assert_eq!(*store.get::<Sum>(&db, &4).unwrap(), 1 + 2 + 3 + 4);
    let c = counters(&store, "sum");
    assert_eq!((c.re_executions, c.hits), (1, 0));
    assert_eq!(counters(&store, "cell").re_executions, 4);
    assert_eq!(*store.get::<Sum>(&db, &4).unwrap(), 10);
    assert_eq!(counters(&store, "sum").hits, 1);
    assert_eq!(
        counters(&store, "cell").re_executions,
        4,
        "nested memos are not re-read on a hit"
    );
}

#[test]
fn an_unrelated_change_verifies_without_re_execution() {
    let (mut db, mut store) = setup(4);
    store.get::<Sum>(&db, &2).unwrap();
    db.commit(&mut store, &[(3, 100, Durability::Drawing)]);
    assert_eq!(*store.get::<Sum>(&db, &2).unwrap(), 3);
    let c = counters(&store, "sum");
    assert_eq!((c.verifications, c.re_executions), (1, 0));
    assert_eq!(counters(&store, "cell").verifications, 2);
}

#[test]
fn a_changed_input_re_executes_and_bumps_changed_at() {
    let (mut db, mut store) = setup(4);
    store.get::<Sum>(&db, &4).unwrap();
    db.commit(&mut store, &[(2, 30, Durability::Drawing)]);
    assert_eq!(*store.get::<Sum>(&db, &4).unwrap(), 1 + 2 + 30 + 4);
    let info = store.memo_info::<Sum>(&4).unwrap();
    assert_eq!((info.verified_at, info.changed_at), (db.rev, db.rev));
    assert_eq!(counters(&store, "sum").re_executions, 1);
    assert_eq!(counters(&store, "sum").cut_offs, 0);
}

#[test]
fn early_cut_off_keeps_dependents_green() {
    let (mut db, mut store) = setup(4);
    assert_eq!(*store.get::<Label>(&db, &4).unwrap(), "4:+");
    let before = store.memo_info::<Positive>(&4).unwrap().changed_at;
    let label = store.get::<Label>(&db, &4).unwrap();
    // The sum changes, its sign does not: Positive re-executes and cuts off, Label only verifies.
    db.commit(&mut store, &[(0, 50, Durability::Drawing)]);
    let again = store.get::<Label>(&db, &4).unwrap();
    assert!(Arc::ptr_eq(&label, &again), "the label's Arc survives");
    assert_eq!(counters(&store, "sum").re_executions, 1);
    let p = counters(&store, "positive");
    assert_eq!((p.re_executions, p.cut_offs), (1, 1));
    let l = counters(&store, "label");
    assert_eq!((l.verifications, l.re_executions), (1, 0));
    let info = store.memo_info::<Positive>(&4).unwrap();
    assert_eq!(info.changed_at, before, "backdated");
    assert_eq!(info.verified_at, db.rev);
    // A change of sign propagates.
    db.commit(&mut store, &[(0, -100, Durability::Drawing)]);
    assert_eq!(*store.get::<Label>(&db, &4).unwrap(), "4:-");
    assert_eq!(counters(&store, "label").re_executions, 1);
}

#[test]
fn durability_skips_verification_across_less_durable_changes() {
    let mut db = Cells::default();
    let mut store = Store::new();
    db.commit(
        &mut store,
        &[
            (0, 1, Durability::Library),
            (1, 2, Durability::Library),
            (2, 3, Durability::Drawing),
        ],
    );
    store.get::<Sum>(&db, &2).unwrap();
    assert_eq!(
        store.memo_info::<Sum>(&2).unwrap().durability,
        Durability::Library
    );
    store.get::<Sum>(&db, &3).unwrap();
    assert_eq!(
        store.memo_info::<Sum>(&3).unwrap().durability,
        Durability::Drawing
    );

    // Drawing and settings changes: the library-only memo skips its dependency walk.
    db.commit(&mut store, &[(2, 30, Durability::Drawing)]);
    db.commit(&mut store, &[(9, 1, Durability::Settings)]);
    store.get::<Sum>(&db, &2).unwrap();
    let c = counters(&store, "sum");
    assert_eq!((c.durability_skips, c.verifications, c.re_executions), (1, 0, 0));
    assert_eq!(counters(&store, "cell").verifications, 0, "no walk");
    // The drawing-durable memo verifies (and re-executes: cell 2 changed).
    assert_eq!(*store.get::<Sum>(&db, &3).unwrap(), 1 + 2 + 30);

    // A library change ends the skip.
    db.commit(&mut store, &[(1, 20, Durability::Library)]);
    assert_eq!(*store.get::<Sum>(&db, &2).unwrap(), 21);
    assert_eq!(counters(&store, "sum").re_executions, 1);
}

#[test]
fn lru_evicts_over_budget_and_keeps_memo_metadata() {
    let (mut db, mut store) = setup(8);
    store.set_lru_budget(Some(10 * 8));
    store.get::<Bulk>(&db, &4).unwrap(); // 32 bytes
    store.get::<Bulk>(&db, &5).unwrap(); // 40 bytes: 72 in total
    assert_eq!(store.lru_bytes(), 72);
    store.get::<Bulk>(&db, &6).unwrap(); // 48 bytes: evicts 4 and 5 (oldest first) to reach 48
    assert_eq!(store.lru_bytes(), 48);
    assert_eq!(counters(&store, "bulk").evictions, 2);
    let info = store.memo_info::<Bulk>(&4).unwrap();
    assert!(!info.has_value);
    // Reading the evicted value re-executes; its inputs are unchanged, so changed_at stays.
    let changed = info.changed_at;
    db.commit(&mut store, &[(100, 1, Durability::Drawing)]);
    assert_eq!(*store.get::<Bulk>(&db, &4).unwrap(), vec![5; 4]);
    let c = counters(&store, "bulk");
    assert_eq!((c.verifications, c.re_executions, c.cut_offs), (1, 1, 1));
    assert_eq!(store.memo_info::<Bulk>(&4).unwrap().changed_at, changed);
    // Unbounded again: nothing more is evicted.
    store.set_lru_budget(None);
    store.get::<Bulk>(&db, &5).unwrap();
    store.get::<Bulk>(&db, &7).unwrap();
    assert!(store.memory().iter().any(|m| m.family == "bulk" && m.values >= 3));
}

#[test]
fn hash_families_cut_off_after_eviction() {
    let (mut db, mut store) = setup(8);
    store.set_lru_budget(Some(1));
    store.get::<Bulk>(&db, &3).unwrap();
    store.get::<Bulk>(&db, &2).unwrap(); // evicts Bulk(3)
    // Cell 3 changes to the value it had: the fingerprint matches, so Bulk(3) cuts off.
    db.commit(&mut store, &[(3, 4, Durability::Drawing)]);
    store.get::<Bulk>(&db, &3).unwrap();
    let c = counters(&store, "bulk");
    assert_eq!((c.re_executions, c.cut_offs), (1, 1));
}

#[test]
#[cfg_attr(not(debug_assertions), ignore = "debug builds panic on a cycle")]
#[should_panic(expected = "ew-memo: query cycle: cyc(1) -> cyc(1)")]
fn a_cycle_panics_in_debug_builds() {
    let (mut db, mut store) = setup(2);
    db.commit(&mut store, &[(1000, 1, Durability::Drawing)]);
    let _ = store.get::<Cyc>(&db, &1);
}

#[test]
fn a_cycle_is_an_error_when_not_panicking() {
    let (mut db, mut store) = setup(2);
    store.set_cycle_panics(false);
    assert_eq!(*store.get::<Cyc>(&db, &1).unwrap(), 1);
    db.commit(&mut store, &[(1000, 1, Durability::Drawing)]);
    let err = store.get::<Cyc>(&db, &1).unwrap_err();
    assert_eq!(err.path, vec!["cyc(1)".to_owned(), "cyc(1)".to_owned()]);
    assert_eq!(counters(&store, "cyc").cycles, 1);
    // The store stays usable and recovers when the cycle is gone.
    db.commit(&mut store, &[(1000, 0, Durability::Drawing)]);
    assert_eq!(*store.get::<Cyc>(&db, &1).unwrap(), 1);
    assert!(store.cold::<Cyc>(&db, &1).is_ok());
}

#[test]
fn reverse_dependencies_indexed_and_scanned_agree() {
    let (db, store) = setup(6);
    for n in 0..6 {
        store.get::<Label>(&db, &n).unwrap();
    }
    let cell2 = store.memo_key::<Cell>(&2).unwrap();
    // Sum is indexed: the sums over 0..n with n > 2 read Cell(2).
    assert_eq!(store.dependents::<Sum>(&cell2), vec![3, 4, 5]);
    // Positive is scanned; it reads Sum only.
    assert_eq!(store.dependents::<Positive>(&cell2), Vec::<u32>::new());
    let sum4 = store.memo_key::<Sum>(&4).unwrap();
    assert_eq!(store.dependents::<Positive>(&sum4), vec![4]);
    // Inputs: Cell(k) read input k.
    assert_eq!(store.dependents::<Cell>(&DepKey::Input(3)), vec![3]);
    assert_eq!(store.dependents::<Label>(&DepKey::Input(3)), Vec::<u32>::new());
}

#[test]
fn oracle_reports_an_untracked_read() {
    let (mut db, mut store) = setup(8);
    store.set_oracle(Oracle::Every);
    assert_eq!(*store.get::<Untracked>(&db, &()).unwrap(), 8);
    assert!(store.take_mismatches().is_empty());
    db.commit(&mut store, &[(7, 70, Durability::Drawing)]);
    // The memo has no dependency, so it stays at 8; the cold run sees 70.
    assert_eq!(*store.get::<Untracked>(&db, &()).unwrap(), 8);
    let m = store.take_mismatches();
    assert_eq!(m.len(), 1);
    assert_eq!(
        (m[0].family, m[0].memoised.as_str(), m[0].cold.as_str()),
        ("untracked", "8", "70")
    );
    assert_eq!(counters(&store, "untracked").oracle_mismatches, 1);
    // The memo's state at the check: re-stamped at the read's revision, value unchanged since the first run.
    assert_eq!(m[0].verified_at, db.rev);
    assert_eq!(m[0].changed_at, Rev(1));
    assert_eq!(m[0].durability, Durability::Library);
    assert!(m[0].deps.is_empty());
}

#[test]
fn oracle_reports_the_dependency_trail() {
    let (mut db, mut store) = setup(8);
    store.set_oracle(Oracle::Every);
    assert_eq!(*store.get::<Partial>(&db, &()).unwrap(), 1 + 8);
    assert!(store.take_mismatches().is_empty());
    db.commit(&mut store, &[(7, 70, Durability::Drawing)]);
    assert_eq!(
        *store.get::<Partial>(&db, &()).unwrap(),
        9,
        "the untracked read is stale"
    );
    let m = store.take_mismatches();
    assert_eq!(m.len(), 1);
    let m = &m[0];
    assert_eq!((m.family, m.key.as_str()), ("partial", "()"));
    assert_eq!((m.memoised.as_str(), m.cold.as_str()), ("9", "71"));
    assert_eq!(m.rev, db.rev);
    assert_eq!((m.verified_at, m.changed_at), (db.rev, Rev(1)));
    assert_eq!(m.durability, Durability::Drawing);
    assert_eq!(m.deps, vec!["cell(0)@r1".to_owned(), "input(3)@r1".to_owned()]);
}

#[test]
fn oracle_sampling_is_deterministic() {
    let (db, store) = setup(4);
    store.set_oracle(Oracle::OneIn(NonZeroU32::new(3).unwrap()));
    for _ in 0..9 {
        store.get::<Cell>(&db, &1).unwrap();
    }
    assert_eq!(counters(&store, "cell").oracle_checks, 3);
}

#[test]
fn a_fork_evolves_independently_and_is_adopted_if_unchanged() {
    let (db, mut store) = setup(4);
    store.get::<Sum>(&db, &2).unwrap();
    let job = store.fork();
    job.get::<Sum>(&db, &4).unwrap();
    assert!(store.memo_info::<Sum>(&4).is_none());
    assert!(store.adopt_if_unchanged(job));
    assert!(store.memo_info::<Sum>(&4).is_some());

    // A store that changed since the fork does not adopt it.
    let job = store.fork();
    job.get::<Sum>(&db, &3).unwrap();
    store.get::<Sum>(&db, &1).unwrap();
    assert!(!store.adopt_if_unchanged(job));
    assert!(store.memo_info::<Sum>(&3).is_none());
}

/// Probe A of the cycle 6 review: a fork of a fork is not adopted, even when its epoch-like position matches,
/// and a store that wrote a memo after the fork keeps it.
#[test]
fn a_fork_of_a_fork_and_a_stale_sibling_are_refused() {
    let (db, mut s) = setup(8);
    let f = s.fork();
    f.get::<Sum>(&db, &5).unwrap();
    let g = f.fork();
    s.get::<Sum>(&db, &6).unwrap();
    assert!(
        !s.adopt_if_unchanged(g),
        "a grandchild is not a fork of this store"
    );
    assert!(s.memo_info::<Sum>(&6).is_some(), "the store's own memo survives");

    // A grandchild is refused even when the store did not change at all.
    let (db, mut s) = setup(8);
    let f = s.fork();
    f.get::<Sum>(&db, &5).unwrap();
    let g = f.fork();
    assert!(!s.adopt_if_unchanged(g));
    assert!(s.memo_info::<Sum>(&5).is_none());

    // Two siblings: the first adopted changes the store, so the second is refused.
    let (db, mut s) = setup(8);
    let a = s.fork();
    let b = s.fork();
    a.get::<Sum>(&db, &3).unwrap();
    b.get::<Sum>(&db, &4).unwrap();
    assert!(s.adopt_if_unchanged(a));
    assert!(!s.adopt_if_unchanged(b));
    assert!(s.memo_info::<Sum>(&3).is_some());
    assert!(s.memo_info::<Sum>(&4).is_none());
}

/// Probe B of the cycle 6 review: a fork that advanced past its base revision is not adopted, and the
/// store's revision does not move.
#[test]
fn a_fork_that_advanced_is_refused() {
    let (db, mut s) = setup(4);
    let base = s.current();
    let f = s.fork();
    f.advance(Rev(50), Durability::Drawing);
    f.get::<Sum>(&db, &4).unwrap();
    assert!(!s.adopt_if_unchanged(f));
    assert_eq!(s.current(), base);
    assert!(s.memo_info::<Sum>(&4).is_none());
}

/// The legitimate case: a direct fork at the same revision, while the store only had hits, is adopted;
/// changing the budget, writing a memo or advancing the store after the fork refuses it.
#[test]
fn a_direct_fork_is_adopted_after_hits_only() {
    let (db, mut s) = setup(6);
    s.get::<Sum>(&db, &2).unwrap();
    let f = s.fork();
    f.get::<Sum>(&db, &5).unwrap();
    s.get::<Sum>(&db, &2).unwrap(); // a hit writes nothing
    assert!(s.adopt_if_unchanged(f));
    assert_eq!(*s.get::<Sum>(&db, &5).unwrap(), 1 + 2 + 3 + 4 + 5);
    assert_eq!(counters(&s, "sum").re_executions, 2, "Sum(5) came from the fork");

    let f = s.fork();
    f.get::<Sum>(&db, &6).unwrap();
    s.set_lru_budget(Some(1_000));
    assert!(!s.adopt_if_unchanged(f), "a budget change after the fork");

    let f = s.fork();
    f.get::<Sum>(&db, &6).unwrap();
    s.advance(Rev(s.current().0 + 1), Durability::Drawing);
    assert!(!s.adopt_if_unchanged(f), "an advance after the fork");
}

/// Reads library cell 100 (a flag); when it is set, also drawing cell 0. True when the flag is clear or
/// cell 0 is positive: its read set depends on a library input.
struct Dyn;
impl Query<Cells> for Dyn {
    type Key = ();
    type Value = bool;
    const NAME: &'static str = "dyn";
    fn equivalence() -> Equivalence<bool> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Cells>, _: &()) -> Result<bool, Cycle> {
        cx.read(&100);
        if cx.db().value(100) == 0 {
            return Ok(true);
        }
        cx.read(&0);
        Ok(cx.db().value(0) > 0)
    }
}

/// `Dyn` passed through: a memo whose only dependency is a memo.
struct Top;
impl Query<Cells> for Top {
    type Key = ();
    type Value = bool;
    const NAME: &'static str = "top";
    fn equivalence() -> Equivalence<bool> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Cells>, _: &()) -> Result<bool, Cycle> {
        Ok(*cx.get::<Dyn>(&())?)
    }
}

/// Probe 1 of the cycle 6 review: a dependency re-executes with a wider read set at a lower durability and
/// cuts off; the verified-green parent must take the lower durability, or the next drawing change skips it.
#[test]
fn a_green_memo_takes_the_durability_of_a_dependency_that_cut_off_lower() {
    let mut db = Cells::default();
    let mut store = Store::new();
    db.commit(
        &mut store,
        &[(100, 0, Durability::Library), (0, 5, Durability::Drawing)],
    );
    assert!(*store.get::<Top>(&db, &()).unwrap());
    assert_eq!(
        store.memo_info::<Top>(&()).unwrap().durability,
        Durability::Library
    );
    // The flag is set: Dyn also reads cell 0, gives the same answer and cuts off at drawing durability.
    db.commit(&mut store, &[(100, 1, Durability::Library)]);
    assert!(*store.get::<Top>(&db, &()).unwrap());
    assert_eq!(counters(&store, "dyn").cut_offs, 1);
    assert_eq!(counters(&store, "top").verifications, 1);
    assert_eq!(
        store.memo_info::<Dyn>(&()).unwrap().durability,
        Durability::Drawing
    );
    assert_eq!(
        store.memo_info::<Top>(&()).unwrap().durability,
        Durability::Drawing,
        "the green parent takes the dependency's durability"
    );
    // A drawing change of cell 0 now reaches Top.
    db.commit(&mut store, &[(0, -5, Durability::Drawing)]);
    let memo = *store.get::<Top>(&db, &()).unwrap();
    assert_eq!(memo, *store.cold::<Top>(&db, &()).unwrap());
    assert!(!memo);
}

/// Probe 2 of the cycle 6 review: an input moves from library to drawing durability with an unchanged
/// value; the memo above the cut-off must become drawing-durable.
#[test]
fn a_green_memo_takes_the_durability_of_an_input_that_moved_lower() {
    let mut db = Cells::default();
    let mut store = Store::new();
    db.commit(
        &mut store,
        &[(0, 1, Durability::Library), (1, 1, Durability::Library)],
    );
    assert_eq!(*store.get::<Sum>(&db, &2).unwrap(), 2);
    // Same value, now drawing data (a library-level commit: it covers the level before the change).
    db.commit(&mut store, &[(0, 1, Durability::Drawing)]);
    assert_eq!(*store.get::<Sum>(&db, &2).unwrap(), 2);
    assert_eq!(
        store.memo_info::<Sum>(&2).unwrap().durability,
        Durability::Drawing
    );
    db.commit(&mut store, &[(0, 41, Durability::Drawing)]);
    assert_eq!(*store.get::<Sum>(&db, &2).unwrap(), 42);
    assert_eq!(*store.cold::<Sum>(&db, &2).unwrap(), 42);
}

/// The durability `memo_info` reports is the minimum of the dependencies' current durabilities after every
/// kind of read (execution, verification, durability skip).
#[test]
fn memo_durability_is_the_minimum_of_its_dependencies_on_every_path() {
    let mut db = Cells::default();
    let mut store = Store::new();
    db.commit(
        &mut store,
        &[(0, 1, Durability::Library), (1, 1, Durability::Settings)],
    );
    store.get::<Sum>(&db, &2).unwrap(); // execution
    assert_eq!(
        store.memo_info::<Sum>(&2).unwrap().durability,
        Durability::Settings
    );
    db.commit(&mut store, &[(5, 1, Durability::Drawing)]);
    store.get::<Sum>(&db, &2).unwrap(); // durability skip
    assert_eq!(counters(&store, "sum").durability_skips, 1);
    assert_eq!(
        store.memo_info::<Sum>(&2).unwrap().durability,
        Durability::Settings
    );
    db.commit(&mut store, &[(1, 1, Durability::Library)]);
    store.get::<Sum>(&db, &2).unwrap(); // verification after a cut-off below, at a higher level
    assert_eq!(counters(&store, "sum").verifications, 1);
    assert_eq!(
        store.memo_info::<Sum>(&2).unwrap().durability,
        Durability::Library
    );
}

#[test]
fn commit_statistics_reset_on_advance() {
    let (mut db, mut store) = setup(3);
    store.get::<Sum>(&db, &3).unwrap();
    assert!(!store.commit_stats().is_empty());
    db.commit(&mut store, &[]);
    assert!(store.commit_stats().is_empty());
    assert_eq!(store.total_stats().family("sum").re_executions, 1);
}

// ---------------------------------------------------------------------------------------------------------
// Adopting a fork: configuration, statistics, recency (cycle 6 review F4)

/// A store with one oracle mismatch (`Untracked` after cell 7 changed) that has not been taken.
fn store_with_a_mismatch() -> (Cells, Store<Cells>) {
    let (mut db, mut store) = setup(8);
    store.set_oracle(Oracle::Every);
    store.get::<Untracked>(&db, &()).unwrap();
    db.commit(&mut store, &[(7, 70, Durability::Drawing)]);
    store.get::<Untracked>(&db, &()).unwrap();
    (db, store)
}

#[test]
fn oracle_mismatches_are_not_duplicated_by_a_fork_and_adopt() {
    let (_, mut store) = store_with_a_mismatch();
    let job = store.fork();
    assert!(store.adopt_if_unchanged(job));
    assert_eq!(
        store.take_mismatches().len(),
        1,
        "the parent's mismatch is reported once"
    );

    // A mismatch found in the fork is reported once, after the parent's.
    let (db, mut store) = store_with_a_mismatch();
    let job = store.fork();
    job.get::<Untracked>(&db, &()).unwrap();
    assert!(store.adopt_if_unchanged(job));
    assert_eq!(
        store.take_mismatches().len(),
        2,
        "one of the parent's, one of the fork's"
    );

    let (db, mut store) = store_with_a_mismatch();
    assert_eq!(store.take_mismatches().len(), 1);
    let job = store.fork();
    job.get::<Untracked>(&db, &()).unwrap();
    assert!(store.adopt_if_unchanged(job));
    assert_eq!(store.take_mismatches().len(), 1, "the fork's finding, once");
    assert!(store.take_mismatches().is_empty());
}

#[test]
fn a_fork_starts_with_empty_statistics_and_mismatches() {
    let (_, store) = store_with_a_mismatch();
    let job = store.fork();
    assert!(job.commit_stats().is_empty());
    assert!(job.total_stats().is_empty());
    assert!(job.take_mismatches().is_empty());
    assert!(!store.total_stats().is_empty());
    assert_eq!(store.take_mismatches().len(), 1, "the parent keeps its own");
}

#[test]
fn the_parents_configuration_survives_adopt_and_its_budget_applies() {
    let (db, mut store) = setup(8);
    store.set_oracle(Oracle::Every);
    store.set_lru_budget(Some(40));
    let job = store.fork();
    job.set_lru_budget(None);
    job.set_oracle(Oracle::Off);
    for k in [4, 5, 6] {
        job.get::<Bulk>(&db, &k).unwrap(); // 32, 40 and 48 bytes, none evicted in the fork
    }
    assert_eq!(job.lru_bytes(), 120);
    assert!(store.adopt_if_unchanged(job));
    assert_eq!(
        store.lru_budget(),
        Some(40),
        "the fork's budget change is not adopted"
    );
    // Over the parent's budget: evicted oldest first, the newest value survives alone.
    assert_eq!(store.lru_bytes(), 48);
    assert!(!store.memo_info::<Bulk>(&4).unwrap().has_value);
    assert!(!store.memo_info::<Bulk>(&5).unwrap().has_value);
    assert!(store.memo_info::<Bulk>(&6).unwrap().has_value);
    assert_eq!(counters(&store, "bulk").evictions, 2);
    // The oracle mode is the parent's: a read checks against cold.
    store.get::<Cell>(&db, &1).unwrap();
    assert_eq!(counters(&store, "cell").oracle_checks, 1);
}

#[test]
fn the_parents_hits_and_the_forks_work_both_count_after_adopt() {
    let (db, mut store) = setup(4);
    store.get::<Sum>(&db, &2).unwrap();
    let job = store.fork();
    store.get::<Sum>(&db, &2).unwrap(); // a hit after the fork
    store.get::<Sum>(&db, &2).unwrap();
    job.get::<Sum>(&db, &4).unwrap(); // Sum(4): executes; Cell(0), Cell(1) hit; Cell(2), Cell(3) execute
    assert_eq!(counters(&job, "sum").re_executions, 1);
    assert_eq!(counters(&job, "sum").hits, 0, "the fork's statistics are its own");
    assert!(store.adopt_if_unchanged(job));
    for stats in [store.commit_stats(), store.total_stats()] {
        let sum = stats.family("sum");
        assert_eq!((sum.hits, sum.re_executions), (2, 2));
        let cell = stats.family("cell");
        assert_eq!((cell.hits, cell.re_executions), (2, 4));
    }
}

#[test]
fn the_parents_hits_since_the_fork_keep_their_lru_recency_after_adopt() {
    let (db, mut store) = setup(8);
    store.set_lru_budget(Some(100));
    store.get::<Bulk>(&db, &2).unwrap(); // 16 bytes
    store.get::<Bulk>(&db, &3).unwrap(); // 24 bytes
    let job = store.fork();
    store.get::<Bulk>(&db, &2).unwrap(); // the parent hits Bulk(2): now the most recent
    job.get::<Bulk>(&db, &4).unwrap(); // 32 bytes: 72 in the fork, nothing evicted
    assert!(store.adopt_if_unchanged(job));
    assert_eq!(store.lru_bytes(), 72);
    // One more write over the budget (112 > 100): the least recent goes, which is Bulk(3), not Bulk(2).
    store.get::<Bulk>(&db, &5).unwrap(); // 40 bytes
    assert!(
        store.memo_info::<Bulk>(&2).unwrap().has_value,
        "the parent's hit survives"
    );
    assert!(!store.memo_info::<Bulk>(&3).unwrap().has_value);
    assert!(store.memo_info::<Bulk>(&4).unwrap().has_value);
    assert!(store.memo_info::<Bulk>(&5).unwrap().has_value);
}

// ---------------------------------------------------------------------------------------------------------
// Pointer identity, panics (cycle 6 review F5, F13)

#[test]
fn a_hash_family_keeps_its_arc_on_cut_off_but_not_across_an_eviction() {
    let (mut db, mut store) = setup(8);
    // Bulk(0) is always empty, whatever cell 0 holds: it re-executes and cuts off by fingerprint.
    let first = store.get::<Bulk>(&db, &0).unwrap();
    db.commit(&mut store, &[(0, 99, Durability::Drawing)]);
    let again = store.get::<Bulk>(&db, &0).unwrap();
    assert_eq!(counters(&store, "bulk").cut_offs, 1);
    assert!(Arc::ptr_eq(&first, &again), "a cut-off keeps the previous Arc");

    // Once the value was evicted there is no previous Arc: the re-computed value is a new one.
    store.set_lru_budget(Some(1));
    store.get::<Bulk>(&db, &2).unwrap(); // evicts Bulk(0)
    assert!(!store.memo_info::<Bulk>(&0).unwrap().has_value);
    db.commit(&mut store, &[(0, 5, Durability::Drawing)]);
    let after = store.get::<Bulk>(&db, &0).unwrap();
    assert_eq!(*after, *first);
    assert!(
        !Arc::ptr_eq(&first, &after),
        "expected: the old value was dropped"
    );
}

#[test]
fn advance_and_reads_work_after_a_caught_panic_in_a_query() {
    let (mut db, mut store) = setup(4);
    assert_eq!(*store.get::<Boom>(&db, &()).unwrap(), 7);
    db.commit(&mut store, &[(2000, 1, Durability::Drawing)]);
    let caught = std::panic::catch_unwind(AssertUnwindSafe(|| store.get::<Boom>(&db, &())));
    assert!(caught.is_err(), "the query panicked");
    // The abandoned evaluation does not block the next commit nor the next read.
    db.commit(&mut store, &[(2000, 0, Durability::Drawing)]);
    assert_eq!(*store.get::<Boom>(&db, &()).unwrap(), 7);
    assert_eq!(*store.get::<Sum>(&db, &4).unwrap(), 10);

    // A panic followed by an advance and no read in between.
    db.commit(&mut store, &[(2000, 1, Durability::Drawing)]);
    let caught = std::panic::catch_unwind(AssertUnwindSafe(|| store.get::<Boom>(&db, &())));
    assert!(caught.is_err());
    db.commit(&mut store, &[(1, 5, Durability::Drawing)]);
    assert_eq!(*store.get::<Cell>(&db, &1).unwrap(), 5);
}

// ---------------------------------------------------------------------------------------------------------
// Memoised equals cold

#[derive(Clone, Debug)]
enum Step {
    Set(u32, i64, u8),
    /// Re-sets a cell to its current value at another durability.
    Move(u32, u8),
    Read(u8, u32),
    Budget(Option<usize>),
    /// Takes a fork of the store, with a snapshot of the database (what a job reads).
    Fork,
    /// Reads through the held fork against its snapshot.
    ReadFork(u8, u32),
    /// Offers the held fork back.
    Adopt,
}

/// Cells `0..8` plus the library flag cell 100 that `Dyn` reads.
fn cell_key() -> impl Strategy<Value = u32> {
    prop_oneof![8 => 0u32..8, 1 => Just(100u32)]
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        3 => (cell_key(), -3i64..4, 0u8..3).prop_map(|(k, v, d)| Step::Set(k, v, d)),
        1 => (cell_key(), 0u8..3).prop_map(|(k, d)| Step::Move(k, d)),
        6 => (0u8..7, 0u32..9).prop_map(|(q, k)| Step::Read(q, k)),
        1 => prop_oneof![Just(None), (1usize..200).prop_map(Some)].prop_map(Step::Budget),
        1 => Just(Step::Fork),
        3 => (0u8..7, 0u32..9).prop_map(|(q, k)| Step::ReadFork(q, k)),
        1 => Just(Step::Adopt),
    ]
}

/// The number of query families `read` and `cold` know.
const QUERIES: u8 = 7;

fn read(store: &mut Store<Cells>, db: &Cells, q: u8, k: u32) -> String {
    match q {
        0 => format!("{:?}", store.get::<Cell>(db, &k).unwrap()),
        1 => format!("{:?}", store.get::<Sum>(db, &k).unwrap()),
        2 => format!("{:?}", store.get::<Positive>(db, &k).unwrap()),
        3 => format!("{:?}", store.get::<Label>(db, &k).unwrap()),
        4 => format!("{:?}", store.get::<Bulk>(db, &k).unwrap()),
        5 => format!("{:?}", store.get::<Dyn>(db, &()).unwrap()),
        _ => format!("{:?}", store.get::<Top>(db, &()).unwrap()),
    }
}

fn cold(store: &Store<Cells>, db: &Cells, q: u8, k: u32) -> String {
    match q {
        0 => format!("{:?}", store.cold::<Cell>(db, &k).unwrap()),
        1 => format!("{:?}", store.cold::<Sum>(db, &k).unwrap()),
        2 => format!("{:?}", store.cold::<Positive>(db, &k).unwrap()),
        3 => format!("{:?}", store.cold::<Label>(db, &k).unwrap()),
        4 => format!("{:?}", store.cold::<Bulk>(db, &k).unwrap()),
        5 => format!("{:?}", store.cold::<Dyn>(db, &()).unwrap()),
        _ => format!("{:?}", store.cold::<Top>(db, &()).unwrap()),
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// Under random input mutation sequences (any durability, durability moves, commits of one or more
    /// changes, LRU budgets, forks read against a snapshot and offered back), every memoised read equals the
    /// cold computation, the revision never decreases, a fork that missed a commit is refused, and oracle
    /// mode finds no mismatch.
    #[test]
    fn memoised_equals_cold(steps in proptest::collection::vec(step(), 1..60)) {
        let mut db = Cells::default();
        let mut store = Store::new();
        store.set_oracle(Oracle::Every);
        let mut pending: Vec<(u32, i64, Durability)> = Vec::new();
        // A fork with the database snapshot it reads, and whether the store committed since.
        let mut held: Option<(Store<Cells>, Cells)> = None;
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
                Step::Read(q, k) => {
                    if !pending.is_empty() {
                        db.commit(&mut store, &std::mem::take(&mut pending));
                        committed_since_fork = true;
                    }
                    let memo = read(&mut store, &db, q, k);
                    prop_assert_eq!(memo, cold(&store, &db, q, k));
                }
                Step::Budget(b) => store.set_lru_budget(b),
                Step::Fork => {
                    held = Some((store.fork(), db.clone()));
                    committed_since_fork = false;
                }
                Step::ReadFork(q, k) => {
                    if let Some((fork, snapshot)) = held.as_mut() {
                        let memo = read(fork, snapshot, q, k);
                        prop_assert_eq!(memo, cold(fork, snapshot, q, k));
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
                        for q in 0..QUERIES {
                            for k in [0, 3, 8] {
                                let memo = read(&mut store, &db, q, k);
                                prop_assert_eq!(memo, cold(&store, &db, q, k));
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

// ---------------------------------------------------------------------------------------------------------
// `Store::explain` (WP-S1.22 D12): read-only state and trail

/// The explanation of `q`'s key `k`, every field as text (the read-only comparisons of the property below).
fn explain(store: &Store<Cells>, q: u8, k: u32) -> String {
    match q {
        0 => format!("{:?}", store.explain::<Cell>(&k)),
        1 => format!("{:?}", store.explain::<Sum>(&k)),
        2 => format!("{:?}", store.explain::<Positive>(&k)),
        3 => format!("{:?}", store.explain::<Label>(&k)),
        4 => format!("{:?}", store.explain::<Bulk>(&k)),
        5 => format!("{:?}", store.explain::<Dyn>(&())),
        _ => format!("{:?}", store.explain::<Top>(&())),
    }
}

/// Everything a read can observe or change of a store: every memo's state, the statistics, the memory and the LRU.
fn observable(store: &Store<Cells>) -> String {
    let mut infos = Vec::new();
    for k in 0..9 {
        infos.push(format!(
            "{:?} {:?} {:?} {:?} {:?}",
            store.memo_info::<Cell>(&k),
            store.memo_info::<Sum>(&k),
            store.memo_info::<Positive>(&k),
            store.memo_info::<Label>(&k),
            store.memo_info::<Bulk>(&k)
        ));
    }
    format!(
        "{infos:?} {:?} {:?} {:?} {:?} {:?} {:?} {}",
        store.memo_info::<Dyn>(&()),
        store.memo_info::<Top>(&()),
        store.commit_stats(),
        store.total_stats(),
        store.memory(),
        store.current(),
        store.lru_bytes()
    )
}

#[test]
fn explain_of_a_key_never_read_has_no_memo() {
    let mut db = Cells::default();
    let mut store = Store::new();
    db.commit(&mut store, &[(0, 1, Durability::Drawing)]);
    let e = store.explain::<Label>(&2);
    assert_eq!(e.family, "label");
    assert_eq!(e.key, "2");
    assert_eq!(e.current, Rev(1));
    assert!(!e.memoised && !e.hit && !e.lru && !e.truncated);
    assert_eq!((e.verified_at, e.changed_at), (Rev::ZERO, Rev::ZERO));
    assert!(e.deps.is_empty());
    assert_eq!(e.lines(), Vec::<String>::new());
    // A family the store has seen, with another key read.
    let _ = store.get::<Label>(&db, &1).unwrap();
    assert!(!store.explain::<Label>(&2).memoised);
}

#[test]
fn explain_reports_the_hit_the_revisions_and_the_trail_in_read_order() {
    let mut db = Cells::default();
    let mut store = Store::new();
    db.commit(
        &mut store,
        &[(0, 1, Durability::Drawing), (1, 2, Durability::Settings)],
    );
    assert_eq!(*store.get::<Label>(&db, &2).unwrap(), "2:+");

    let label = store.explain::<Label>(&2);
    assert!(label.memoised && label.hit);
    assert_eq!((label.verified_at, label.changed_at), (Rev(1), Rev(1)));
    assert_eq!(label.durability, Durability::Drawing);
    assert_eq!(label.lines(), ["positive(2)@r1"]);
    assert_eq!(label.deps[0].memo(), Some(("positive", "2")));
    assert_eq!(label.deps[0].input(), None);
    assert_eq!(label.deps[0].observed(), Rev(1));

    let sum = store.explain::<Sum>(&2);
    assert_eq!(sum.lines(), ["cell(0)@r1", "cell(1)@r1"]);
    let cell = store.explain::<Cell>(&1);
    assert_eq!(cell.lines(), ["input(1)@r1"]);
    assert_eq!(cell.deps[0].input(), Some(&1));
    assert_eq!(cell.deps[0].memo(), None);
    assert_eq!(cell.durability, Durability::Settings);

    // A commit: the memo stays, verified at the earlier revision, so a read now is no hit.
    db.commit(&mut store, &[(0, 3, Durability::Drawing)]);
    let label = store.explain::<Label>(&2);
    assert!(label.memoised && !label.hit);
    assert_eq!((label.current, label.verified_at), (Rev(2), Rev(1)));
    // The read verifies it; `Positive` cuts off, so the label's value keeps its revision.
    let _ = store.get::<Label>(&db, &2).unwrap();
    let label = store.explain::<Label>(&2);
    assert!(label.hit);
    assert_eq!((label.verified_at, label.changed_at), (Rev(2), Rev(1)));
}

#[test]
fn explain_keeps_64_reads_and_marks_the_rest_with_an_ellipsis() {
    let mut db = Cells::default();
    let mut store = Store::new();
    db.commit(&mut store, &[(0, 1, Durability::Drawing)]);
    let _ = store.get::<Sum>(&db, &100).unwrap();
    let e = store.explain::<Sum>(&100);
    assert_eq!(e.deps.len(), 64);
    assert!(e.truncated);
    let lines = e.lines();
    assert_eq!(lines.len(), 65);
    assert_eq!(lines[0], "cell(0)@r1");
    assert_eq!(lines[63], "cell(63)@r1");
    assert_eq!(lines[64], "…");
}

#[test]
fn explain_names_an_lru_family_and_an_evicted_value_is_no_hit() {
    let mut db = Cells::default();
    let mut store = Store::new();
    db.commit(
        &mut store,
        &[(3, 5, Durability::Drawing), (4, 6, Durability::Drawing)],
    );
    let _ = store.get::<Bulk>(&db, &3).unwrap();
    let e = store.explain::<Bulk>(&3);
    assert!(e.lru && e.hit);
    // A budget that holds one of the two values evicts the older one.
    store.set_lru_budget(Some(40));
    let _ = store.get::<Bulk>(&db, &4).unwrap();
    let e = store.explain::<Bulk>(&3);
    assert!(
        e.memoised && !e.hit,
        "an evicted value is recomputed on its next read"
    );
    assert_eq!(e.lines(), ["cell(3)@r1"]);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// R4 of WP-S1.22: `explain` changes nothing. Two stores run the same random steps; one also explains every
    /// family at several keys before and after each step. Every read answers the same (and equals the cold
    /// computation), every memo state, statistic, memory figure and LRU byte count stays equal between the twins,
    /// and an explanation repeated without a step in between is the same.
    #[test]
    fn explain_is_read_only(steps in proptest::collection::vec(step(), 1..50)) {
        let mut db = Cells::default();
        let mut plain = Store::new();
        let mut explained = Store::new();
        let mut pending: Vec<(u32, i64, Durability)> = Vec::new();
        let explain_all = |store: &Store<Cells>| -> Vec<String> {
            let mut out = Vec::new();
            for q in 0..QUERIES {
                for k in [0, 3, 8] {
                    out.push(explain(store, q, k));
                }
            }
            out
        };
        for s in steps {
            let before = explain_all(&explained);
            prop_assert_eq!(&before, &explain_all(&explained));
            match s {
                Step::Set(k, v, d) => pending.push((k, v, Durability::ALL[usize::from(d)])),
                Step::Move(k, d) => pending.push((k, db.value(k), Durability::ALL[usize::from(d)])),
                Step::Read(q, k) => {
                    if !pending.is_empty() {
                        let changes = std::mem::take(&mut pending);
                        let mut twin = db.clone();
                        twin.commit(&mut plain, &changes);
                        db.commit(&mut explained, &changes);
                    }
                    let a = read(&mut plain, &db, q, k);
                    let b = read(&mut explained, &db, q, k);
                    prop_assert_eq!(&a, &b);
                    prop_assert_eq!(a, cold(&explained, &db, q, k));
                }
                Step::Budget(b) => {
                    plain.set_lru_budget(b);
                    explained.set_lru_budget(b);
                }
                Step::Fork | Step::ReadFork(..) | Step::Adopt => {}
            }
            let _ = explain_all(&explained);
            prop_assert_eq!(observable(&plain), observable(&explained));
        }
    }
}
