// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Engine tests over a toy database: cells of integers with revision stamps and durabilities.

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::sync::Arc;

use proptest::prelude::*;

use crate::{Counters, Cx, Cycle, Db, DepKey, Durability, Equivalence, Oracle, Query, Rev, Store};

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
            // A deterministic toy fingerprint: FNV-1a over the elements, widened.
            let mut h: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
            for x in v {
                h ^= *x as u128;
                h = h.wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013b);
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
    let (db, mut store) = setup(4);
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
    store.set_panic_on_cycle(false);
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
    let (db, mut store) = setup(6);
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
}

#[test]
fn oracle_sampling_is_deterministic() {
    let (db, mut store) = setup(4);
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
    let mut job = store.fork();
    job.get::<Sum>(&db, &4).unwrap();
    assert!(store.memo_info::<Sum>(&4).is_none());
    assert!(store.adopt_if_unchanged(job));
    assert!(store.memo_info::<Sum>(&4).is_some());

    // A store that changed since the fork does not adopt it.
    let mut job = store.fork();
    job.get::<Sum>(&db, &3).unwrap();
    store.get::<Sum>(&db, &1).unwrap();
    assert!(!store.adopt_if_unchanged(job));
    assert!(store.memo_info::<Sum>(&3).is_none());
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
// Memoised equals cold

#[derive(Clone, Debug)]
enum Step {
    Set(u32, i64, u8),
    Read(u8, u32),
    Budget(Option<usize>),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        3 => (0u32..8, -3i64..4, 0u8..3).prop_map(|(k, v, d)| Step::Set(k, v, d)),
        6 => (0u8..5, 0u32..9).prop_map(|(q, k)| Step::Read(q, k)),
        1 => prop_oneof![Just(None), (1usize..200).prop_map(Some)].prop_map(Step::Budget),
    ]
}

fn read(store: &mut Store<Cells>, db: &Cells, q: u8, k: u32) -> String {
    match q {
        0 => format!("{:?}", store.get::<Cell>(db, &k).unwrap()),
        1 => format!("{:?}", store.get::<Sum>(db, &k).unwrap()),
        2 => format!("{:?}", store.get::<Positive>(db, &k).unwrap()),
        3 => format!("{:?}", store.get::<Label>(db, &k).unwrap()),
        _ => format!("{:?}", store.get::<Bulk>(db, &k).unwrap()),
    }
}

fn cold(store: &Store<Cells>, db: &Cells, q: u8, k: u32) -> String {
    match q {
        0 => format!("{:?}", store.cold::<Cell>(db, &k).unwrap()),
        1 => format!("{:?}", store.cold::<Sum>(db, &k).unwrap()),
        2 => format!("{:?}", store.cold::<Positive>(db, &k).unwrap()),
        3 => format!("{:?}", store.cold::<Label>(db, &k).unwrap()),
        _ => format!("{:?}", store.cold::<Bulk>(db, &k).unwrap()),
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// Under random input mutation sequences (any durability, commits of one or more changes, LRU budgets),
    /// every memoised read equals the cold computation, and oracle mode finds no mismatch.
    #[test]
    fn memoised_equals_cold(steps in proptest::collection::vec(step(), 1..60)) {
        let mut db = Cells::default();
        let mut store = Store::new();
        store.set_oracle(Oracle::Every);
        let mut pending: Vec<(u32, i64, Durability)> = Vec::new();
        for s in steps {
            match s {
                Step::Set(k, v, d) => pending.push((k, v, Durability::ALL[usize::from(d)])),
                Step::Read(q, k) => {
                    if !pending.is_empty() {
                        db.commit(&mut store, &std::mem::take(&mut pending));
                    }
                    let memo = read(&mut store, &db, q, k);
                    prop_assert_eq!(memo, cold(&store, &db, q, k));
                }
                Step::Budget(b) => store.set_lru_budget(b),
            }
        }
        prop_assert_eq!(store.take_mismatches(), Vec::new());
    }
}
