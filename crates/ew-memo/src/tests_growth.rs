// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Engine tests of the cycle-10 review fixes: byte accounting only where it is needed (finding 2), and bounded
//! growth (finding 3): carried dependencies a hook forgets or drops, part stamps without readers, and the
//! collection of orphan memos.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use crate::{Changes, Cx, Cycle, Db, DepKey, Durability, Equivalence, Oracle, Query, Rev, Store};

// ---------------------------------------------------------------------------------------------------------
// The toy database: pages with a value

/// An input of [`Book`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum In {
    /// One page (its value, or its absence).
    Page(u32),
    /// The set of live pages.
    Pages,
    /// Every page's content: changes with any page.
    Table,
    /// A part of a producer's value (never read as an input).
    Part(u32),
}

/// Live pages with their value and stamp.
#[derive(Clone, Default)]
struct Book {
    pages: BTreeMap<u32, (i64, Rev)>,
    /// The revision of the last change of the page set.
    pages_rev: Rev,
    /// The revision of the last change of any page.
    table_rev: Rev,
    rev: Rev,
}

impl Db for Book {
    type Input = In;

    fn changed_at(&self, input: &In) -> Rev {
        match input {
            // An absent page reports the page set's stamp, at least as recent as its deletion.
            In::Page(p) => self.pages.get(p).map_or(self.pages_rev, |x| x.1),
            In::Pages => self.pages_rev,
            In::Table => self.table_rev,
            In::Part(_) => self.rev,
        }
    }

    fn durability(&self, _: &In) -> Durability {
        Durability::Drawing
    }
}

impl Book {
    fn value(&self, p: u32) -> i64 {
        self.pages.get(&p).map_or(0, |x| x.0)
    }

    /// One commit: sets (`Some`) or deletes (`None`) pages, reporting every touched page, and the page set and
    /// table when they changed.
    fn commit(&mut self, store: &Store<Book>, changes: &[(u32, Option<i64>)]) {
        self.rev = Rev(self.rev.0 + 1);
        let mut touched = Vec::new();
        for &(p, v) in changes {
            let had = self.pages.contains_key(&p);
            match v {
                Some(v) => {
                    self.pages.insert(p, (v, self.rev));
                }
                None => {
                    self.pages.remove(&p);
                }
            }
            if had != v.is_some() {
                self.pages_rev = self.rev;
                touched.push(In::Pages);
            }
            touched.push(In::Page(p));
        }
        self.table_rev = self.rev;
        touched.push(In::Table);
        store.advance_with(self.rev, Durability::Drawing, touched);
    }
}

// ---------------------------------------------------------------------------------------------------------
// Families

/// One page's value.
struct Nets;
impl Query<Book> for Nets {
    type Key = u32;
    type Value = i64;
    const NAME: &'static str = "nets";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Book>, p: &u32) -> Result<i64, Cycle> {
        cx.read(&In::Page(*p));
        Ok(cx.db().value(*p))
    }
}

/// Every live page with its value, over `Nets` memos; incremental: re-reads the pages it was marked
/// through, the added ones, and forgets the deleted ones.
struct Partition;
impl Query<Book> for Partition {
    type Key = ();
    type Value = BTreeMap<u32, i64>;
    const NAME: &'static str = "partition";
    const INCREMENTAL: bool = true;
    fn equivalence() -> Equivalence<Self::Value> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Book>, _: &()) -> Result<Self::Value, Cycle> {
        cx.read(&In::Pages);
        let live: Vec<u32> = cx.db().pages.keys().copied().collect();
        let mut out = BTreeMap::new();
        for p in live {
            out.insert(p, *cx.get::<Nets>(&p)?);
        }
        Ok(out)
    }
    fn execute_incremental(
        cx: &mut Cx<'_, Book>,
        key: &(),
        previous: Option<(&Self::Value, &Changes<In>)>,
    ) -> Result<Self::Value, Cycle> {
        let Some((prev, changes)) = previous else {
            return Self::execute(cx, key);
        };
        let Some(via) = &changes.via else {
            cx.drop_carried();
            return Self::execute(cx, key);
        };
        let store = cx.store().expect("a memoised execution");
        let marked: BTreeSet<u32> = via.iter().filter_map(|d| store.key_of::<Nets>(d)).collect();
        cx.read(&In::Pages);
        let live: BTreeSet<u32> = cx.db().pages.keys().copied().collect();
        let mut out = prev.clone();
        for p in prev.keys().filter(|p| !live.contains(p)) {
            out.remove(p);
            if let Some(dep) = store.memo_key::<Nets>(p) {
                cx.forget(&dep);
            }
        }
        for &p in &live {
            if !prev.contains_key(&p) || marked.contains(&p) {
                out.insert(p, *cx.get::<Nets>(&p)?);
            }
        }
        Ok(out)
    }
}

/// The positive pages: an index over the table that re-reads the table's stamp and the touched pages, and
/// drops what it carried.
struct Positive;
impl Query<Book> for Positive {
    type Key = ();
    type Value = BTreeSet<u32>;
    const NAME: &'static str = "positive";
    const INCREMENTAL: bool = true;
    fn equivalence() -> Equivalence<Self::Value> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Book>, _: &()) -> Result<Self::Value, Cycle> {
        cx.read(&In::Table);
        Ok(cx
            .db()
            .pages
            .iter()
            .filter(|(_, v)| v.0 > 0)
            .map(|(p, _)| *p)
            .collect())
    }
    fn execute_incremental(
        cx: &mut Cx<'_, Book>,
        key: &(),
        previous: Option<(&Self::Value, &Changes<In>)>,
    ) -> Result<Self::Value, Cycle> {
        cx.drop_carried();
        let Some((prev, Some(touched))) = previous.map(|(p, c)| (p, c.touched.as_ref())) else {
            return Self::execute(cx, key);
        };
        cx.read(&In::Table);
        let mut out = prev.clone();
        for input in touched {
            if let In::Page(p) = input {
                cx.read(input);
                if cx.db().value(*p) > 0 {
                    out.insert(*p);
                } else {
                    out.remove(p);
                }
            }
        }
        Ok(out)
    }
}

/// Every live page with its value; a producer that declares the pages whose value changed (part
/// `In::Part(p)`).
struct Producer;
impl Query<Book> for Producer {
    type Key = ();
    type Value = BTreeMap<u32, i64>;
    const NAME: &'static str = "producer";
    const INCREMENTAL: bool = true;
    fn equivalence() -> Equivalence<Self::Value> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Book>, _: &()) -> Result<Self::Value, Cycle> {
        cx.read(&In::Table);
        Ok(cx.db().pages.iter().map(|(p, v)| (*p, v.0)).collect())
    }
    fn execute_incremental(
        cx: &mut Cx<'_, Book>,
        key: &(),
        previous: Option<(&Self::Value, &Changes<In>)>,
    ) -> Result<Self::Value, Cycle> {
        cx.drop_carried();
        let next = Self::execute(cx, key)?;
        if let Some((prev, _)) = previous {
            let keys: BTreeSet<u32> = prev.keys().chain(next.keys()).copied().collect();
            cx.changed_parts(
                keys.into_iter()
                    .filter(|p| prev.get(p) != next.get(p))
                    .map(In::Part),
            );
        }
        Ok(next)
    }
}

/// The value of page `p` read as part `In::Part(p)` of `Producer`.
struct PartReader;
impl Query<Book> for PartReader {
    type Key = u32;
    type Value = Option<i64>;
    const NAME: &'static str = "part_reader";
    fn equivalence() -> Equivalence<Self::Value> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Book>, p: &u32) -> Result<Self::Value, Cycle> {
        Ok(cx.get_part::<Producer>(&(), &In::Part(*p))?.get(p).copied())
    }
}

/// `PartReader(p)` while page 500 (the flag) exists, else nothing.
struct Top;
impl Query<Book> for Top {
    type Key = u32;
    type Value = Option<i64>;
    const NAME: &'static str = "top";
    fn equivalence() -> Equivalence<Self::Value> {
        Equivalence::by_eq()
    }
    fn execute(cx: &mut Cx<'_, Book>, p: &u32) -> Result<Self::Value, Cycle> {
        cx.read(&In::Page(FLAG));
        if cx.db().pages.contains_key(&FLAG) {
            Ok(*cx.get::<PartReader>(p)?)
        } else {
            Ok(None)
        }
    }
}

/// The flag page of [`Top`].
const FLAG: u32 = 500;

thread_local! {
    /// The calls of `Heavy::bytes` and `HeavyLru::bytes` on this thread.
    static BYTES_CALLS: Cell<usize> = const { Cell::new(0) };
}

fn bytes_calls() -> usize {
    BYTES_CALLS.with(Cell::get)
}

/// A page's value, with counted byte accounting; no LRU.
struct Heavy;
impl Query<Book> for Heavy {
    type Key = u32;
    type Value = i64;
    const NAME: &'static str = "heavy";
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn bytes(_: &i64) -> usize {
        BYTES_CALLS.with(|c| c.set(c.get() + 1));
        100
    }
    fn execute(cx: &mut Cx<'_, Book>, p: &u32) -> Result<i64, Cycle> {
        cx.read(&In::Page(*p));
        Ok(cx.db().value(*p))
    }
}

/// [`Heavy`] under the LRU.
struct HeavyLru;
impl Query<Book> for HeavyLru {
    type Key = u32;
    type Value = i64;
    const NAME: &'static str = "heavy_lru";
    const LRU: bool = true;
    fn equivalence() -> Equivalence<i64> {
        Equivalence::by_eq()
    }
    fn bytes(_: &i64) -> usize {
        BYTES_CALLS.with(|c| c.set(c.get() + 1));
        100
    }
    fn execute(cx: &mut Cx<'_, Book>, p: &u32) -> Result<i64, Cycle> {
        cx.read(&In::Page(*p));
        Ok(cx.db().value(*p))
    }
}

/// A book with pages `0..n` (values `1..`) at revision 1, and a store advanced to it.
fn setup(n: u32) -> (Book, Store<Book>) {
    let mut db = Book::default();
    let store = Store::new();
    let init: Vec<(u32, Option<i64>)> = (0..n).map(|p| (p, Some(i64::from(p) + 1))).collect();
    db.commit(&store, &init);
    (db, store)
}

fn memos(store: &Store<Book>, family: &str) -> usize {
    store
        .memory()
        .iter()
        .find(|m| m.family == family)
        .map_or(0, |m| m.memos)
}

// ---------------------------------------------------------------------------------------------------------
// Finding 2: byte accounting

/// A family without LRU never has its bytes counted on the execution path; the memory report counts them.
#[test]
fn bytes_of_a_family_without_lru_are_counted_on_demand() {
    let (mut db, store) = setup(4);
    let before = bytes_calls();
    for p in 0..4 {
        store.get::<Heavy>(&db, &p).unwrap();
    }
    for round in 0..3 {
        db.commit(&store, &[(1, Some(10 + round))]);
        store.get::<Heavy>(&db, &1).unwrap();
    }
    assert_eq!(bytes_calls(), before, "no execution counted bytes");
    let heavy = store.memory().into_iter().find(|m| m.family == "heavy").unwrap();
    assert_eq!(heavy.value_bytes, 400);
    assert_eq!(
        bytes_calls(),
        before + 4,
        "the report asked for each value's bytes"
    );
}

/// An LRU family counts bytes at every write that changes the value; a cut-off keeps the previous bytes.
#[test]
fn bytes_of_an_lru_family_are_kept_through_a_cut_off() {
    let (mut db, store) = setup(4);
    let before = bytes_calls();
    store.get::<HeavyLru>(&db, &1).unwrap();
    assert_eq!(bytes_calls(), before + 1);
    // The same value again: the re-execution cuts off and keeps the bytes.
    db.commit(&store, &[(1, Some(2))]);
    store.get::<HeavyLru>(&db, &1).unwrap();
    assert_eq!(store.commit_stats().family("heavy_lru").cut_offs, 1);
    assert_eq!(bytes_calls(), before + 1, "a cut-off does not count bytes again");
    db.commit(&store, &[(1, Some(3))]);
    store.get::<HeavyLru>(&db, &1).unwrap();
    assert_eq!(bytes_calls(), before + 2);
    assert_eq!(store.lru_bytes(), 100);
}

// ---------------------------------------------------------------------------------------------------------
// Finding 3: bounded growth

/// The review's probe on the toy engine: 300 cycles of adding and deleting a page. The incremental family
/// forgets the deleted page, its memo is collected, and nothing grows with the cycles.
#[test]
fn adding_and_deleting_a_page_300_times_does_not_grow() {
    let (mut db, store) = setup(13);
    store.set_oracle(Oracle::Every);
    store.get::<Partition>(&db, &()).unwrap();
    let deps0 = store.memo_info::<Partition>(&()).unwrap().deps.len();
    let push0 = store.push_index_entries();
    assert_eq!(deps0, 14, "the page set and 13 pages");
    assert_eq!(memos(&store, "nets"), 13);
    for i in 0..300u32 {
        let page = 1000 + i;
        db.commit(&store, &[(page, Some(7))]);
        assert_eq!(store.get::<Partition>(&db, &()).unwrap().len(), 14);
        db.commit(&store, &[(page, None)]);
        assert_eq!(store.get::<Partition>(&db, &()).unwrap().len(), 13);
    }
    let deps = store.memo_info::<Partition>(&()).unwrap().deps.len();
    let nets = memos(&store, "nets");
    let push = store.push_index_entries();
    assert!(deps <= deps0 + 1, "partition deps {deps0} -> {deps}");
    assert!(nets <= 13 + 1, "nets memos 13 -> {nets}");
    assert!(push <= push0 + 4, "push index {push0} -> {push}");
    let collected = store.total_stats().family("nets").collected;
    assert!(
        collected >= 299,
        "the deleted pages' memos are collected: {collected}"
    );
    assert!(store.take_mismatches().is_empty());
}

/// A hook that drops its carried dependencies depends on its own reads only: the touched pages of the last
/// commit and the table, not every page ever touched.
#[test]
fn dropping_the_carried_dependencies_keeps_only_the_last_reads() {
    let (mut db, store) = setup(20);
    store.set_oracle(Oracle::Every);
    store.get::<Positive>(&db, &()).unwrap();
    for i in 0..60u32 {
        let p = i % 20;
        db.commit(&store, &[(p, Some(i64::from(i % 3) - 1))]);
        let got = store.get::<Positive>(&db, &()).unwrap();
        assert_eq!(*got, *store.cold::<Positive>(&db, &()).unwrap());
        let deps = store.memo_info::<Positive>(&()).unwrap().deps;
        assert_eq!(deps.len(), 2, "the table and the one touched page: {deps:?}");
    }
    assert!(store.take_mismatches().is_empty());
}

/// A part no memo reads keeps no stamp; a read part keeps its stamp while it has a reader, and loses it when
/// its last reader is collected. Readers stay exact throughout.
#[test]
fn part_stamps_are_kept_only_for_read_parts() {
    let (mut db, store) = setup(10);
    db.commit(&store, &[(FLAG, Some(1))]);
    store.set_oracle(Oracle::Every);
    store.get::<Producer>(&db, &()).unwrap();
    let producer = store.memo_key::<Producer>(&()).unwrap();
    for v in 0..5 {
        db.commit(&store, &[(1, Some(100 + v)), (2, Some(200 + v))]);
        store.get::<Producer>(&db, &()).unwrap();
    }
    assert_eq!(store.part_stamps(&producer), 0, "no reader, no stamp");

    // A reader of part 1 (through `Top`, so that it can be dropped later).
    assert_eq!(*store.get::<Top>(&db, &1).unwrap(), Some(104));
    db.commit(&store, &[(1, Some(7)), (2, Some(8))]);
    assert_eq!(*store.get::<Top>(&db, &1).unwrap(), Some(7));
    assert_eq!(
        store.part_stamps(&producer),
        1,
        "only the read part keeps a stamp"
    );
    db.commit(&store, &[(2, Some(9))]);
    assert_eq!(*store.get::<Top>(&db, &1).unwrap(), Some(7));
    assert_eq!(store.commit_stats().family("part_reader").re_executions, 0);

    // The flag goes: `Top` stops reading `PartReader(1)`, which is collected at the next advance.
    db.commit(&store, &[(FLAG, None)]);
    assert_eq!(*store.get::<Top>(&db, &1).unwrap(), None);
    db.commit(&store, &[(3, Some(30))]);
    assert_eq!(store.commit_stats().family("part_reader").collected, 1);
    assert_eq!(
        store.part_stamps(&producer),
        0,
        "the last reader is gone, so is the stamp"
    );

    // A new reader sees the part's current value, and its next change.
    store.get::<Producer>(&db, &()).unwrap();
    assert_eq!(*store.get::<PartReader>(&db, &1).unwrap(), Some(7));
    db.commit(&store, &[(1, Some(11))]);
    assert_eq!(*store.get::<PartReader>(&db, &1).unwrap(), Some(11));
    db.commit(&store, &[(1, Some(12))]);
    store.advance(Rev(db.rev.0 + 1), Durability::Drawing);
    db.rev = Rev(db.rev.0 + 1);
    // A pull advance: the reader is verified by its part stamp.
    assert_eq!(*store.get::<PartReader>(&db, &1).unwrap(), Some(12));
    assert!(store.take_mismatches().is_empty());
}

/// An orphan the host read at the closing revision is kept; marked later and not read again, it is collected.
#[test]
fn an_orphan_read_by_the_host_is_kept_until_marked_and_unread() {
    let (mut db, store) = setup(4);
    db.commit(&store, &[(FLAG, Some(1))]);
    store.get::<Producer>(&db, &()).unwrap();
    store.get::<Top>(&db, &2).unwrap();
    db.commit(&store, &[(FLAG, None)]);
    store.get::<Top>(&db, &2).unwrap();
    // `PartReader(2)` lost its reader; the host reads it at this revision.
    store.get::<PartReader>(&db, &2).unwrap();
    db.commit(&store, &[(0, Some(5))]);
    assert_eq!(store.commit_stats().family("part_reader").collected, 0);
    assert!(store.memo_info::<PartReader>(&2).is_some());
    // Unmarked, it stays however long the host does not read it.
    db.commit(&store, &[(0, Some(6))]);
    db.commit(&store, &[(0, Some(7))]);
    assert!(store.memo_info::<PartReader>(&2).is_some());
    // Page 2 changes: the producer's re-execution marks the reader of part 2, which nobody reads any more.
    db.commit(&store, &[(2, Some(30))]);
    store.get::<Producer>(&db, &()).unwrap();
    db.commit(&store, &[(0, Some(8))]);
    assert_eq!(store.commit_stats().family("part_reader").collected, 1);
    assert!(store.memo_info::<PartReader>(&2).is_none());
    // Read again, it is computed afresh.
    assert_eq!(*store.get::<PartReader>(&db, &2).unwrap(), Some(30));
}

/// A memo only the host reads, marked by a commit and not read again, is collected at the next advance; one
/// the host reads after the commit stays.
#[test]
fn a_marked_memo_the_host_does_not_read_again_is_collected() {
    let (mut db, store) = setup(4);
    store.get::<Nets>(&db, &1).unwrap();
    store.get::<Nets>(&db, &2).unwrap();
    db.commit(&store, &[(1, None), (2, Some(9))]);
    // Page 1 is deleted: nobody reads `Nets(1)` again. `Nets(2)` is read after the commit.
    assert_eq!(*store.get::<Nets>(&db, &2).unwrap(), 9);
    db.commit(&store, &[(3, Some(1))]);
    assert_eq!(store.commit_stats().family("nets").collected, 1);
    assert!(store.memo_info::<Nets>(&1).is_none());
    assert!(store.memo_info::<Nets>(&2).is_some());
}

/// A key that is read again after its memo was collected gets a fresh memo in the same slot.
#[test]
fn a_collected_memo_is_recomputed_on_the_next_read() {
    let (mut db, store) = setup(3);
    store.get::<Partition>(&db, &()).unwrap();
    let slot = store.memo_key::<Nets>(&1).unwrap();
    db.commit(&store, &[(1, None)]);
    store.get::<Partition>(&db, &()).unwrap();
    db.commit(&store, &[(2, Some(9))]);
    assert!(store.memo_info::<Nets>(&1).is_none(), "collected");
    db.commit(&store, &[(1, Some(4))]);
    let got = store.get::<Partition>(&db, &()).unwrap();
    assert_eq!(got.get(&1), Some(&4));
    assert_eq!(store.memo_key::<Nets>(&1), Some(slot), "the slot is kept");
    let _ = DepKey::<In>::Input(In::Pages);
}
