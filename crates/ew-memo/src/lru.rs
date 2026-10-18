// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A byte-budget least-recently-used index over the values of bulky memos.

use imbl::OrdMap;

use crate::key::MemoRef;

/// A byte-budget LRU over the values of bulky memos (ADR-0002 decision 4). Ticks are a counter, not time.
///
/// Holds only `imbl` structures, so a clone is O(1) and independent of the original (fork semantics).
#[derive(Clone, Debug, Default)]
pub(crate) struct Lru {
    /// The byte budget; `None` is unbounded.
    budget: Option<usize>,
    /// The sum of the sizes of all entries (saturating).
    total: usize,
    /// The tick the next `touch` takes.
    next_tick: u64,
    /// The entries by tick, oldest first.
    by_tick: OrdMap<u64, MemoRef>,
    /// Each entry's tick and size.
    entries: OrdMap<MemoRef, (u64, usize)>,
}

impl Lru {
    /// Sets the byte budget (`None` is unbounded). Takes effect at the next `evict_over_budget`.
    pub(crate) fn set_budget(&mut self, budget: Option<usize>) {
        self.budget = budget;
    }

    /// The byte budget; `None` is unbounded.
    pub(crate) fn budget(&self) -> Option<usize> {
        self.budget
    }

    /// The sum of the sizes of the tracked values.
    pub(crate) fn total_bytes(&self) -> usize {
        self.total
    }

    /// The number of tracked values.
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether `memo` is tracked.
    #[cfg(test)]
    pub(crate) fn contains(&self, memo: MemoRef) -> bool {
        self.entries.contains_key(&memo)
    }

    /// Inserts `memo` or refreshes it as the most recently used, with its current size.
    pub(crate) fn touch(&mut self, memo: MemoRef, bytes: usize) {
        self.remove(memo);
        let tick = self.next_tick;
        self.next_tick += 1;
        self.by_tick.insert(tick, memo);
        self.entries.insert(memo, (tick, bytes));
        self.total = self.total.saturating_add(bytes);
    }

    /// Forgets `memo` (no-op if absent).
    pub(crate) fn remove(&mut self, memo: MemoRef) {
        if let Some((tick, bytes)) = self.entries.remove(&memo) {
            self.by_tick.remove(&tick);
            self.total = self.total.saturating_sub(bytes);
        }
    }

    /// Removes and returns the least recently used entries until the total is within the budget, but never
    /// the most recently used entry (so a single value larger than the budget survives until the next one
    /// arrives). Returns them in eviction order (oldest first). Empty when unbounded or within budget.
    pub(crate) fn evict_over_budget(&mut self) -> Vec<MemoRef> {
        let mut evicted = Vec::new();
        let Some(budget) = self.budget else {
            return evicted;
        };
        while self.total > budget && self.entries.len() > 1 {
            let Some(&(tick, memo)) = self.by_tick.get_min() else {
                break;
            };
            self.by_tick.remove(&tick);
            if let Some((_, bytes)) = self.entries.remove(&memo) {
                self.total = self.total.saturating_sub(bytes);
            }
            evicted.push(memo);
        }
        evicted
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::key::{FamilyId, SlotId};

    fn m(slot: u32) -> MemoRef {
        (FamilyId(0), SlotId(slot))
    }

    #[test]
    fn unbounded_never_evicts() {
        let mut lru = Lru::default();
        assert_eq!(lru.budget(), None);
        for i in 0..10 {
            lru.touch(m(i), 1_000_000);
        }
        assert!(lru.evict_over_budget().is_empty());
        assert_eq!(lru.len(), 10);
        assert_eq!(lru.total_bytes(), 10_000_000);
    }

    #[test]
    fn eviction_is_least_recent_first() {
        let mut lru = Lru::default();
        lru.touch(m(1), 10);
        lru.touch(m(2), 10);
        lru.touch(m(3), 10);
        lru.set_budget(Some(15));
        assert_eq!(lru.budget(), Some(15));
        assert_eq!(lru.evict_over_budget(), vec![m(1), m(2)]);
        assert_eq!(lru.len(), 1);
        assert!(lru.contains(m(3)));
        assert!(!lru.contains(m(1)));
        assert_eq!(lru.total_bytes(), 10);
        assert!(lru.evict_over_budget().is_empty());
    }

    #[test]
    fn touch_refreshes_recency_and_size() {
        let mut lru = Lru::default();
        lru.touch(m(1), 10);
        lru.touch(m(2), 10);
        lru.touch(m(1), 25);
        assert_eq!(lru.len(), 2);
        assert_eq!(lru.total_bytes(), 35);
        lru.set_budget(Some(30));
        assert_eq!(lru.evict_over_budget(), vec![m(2)]);
        assert_eq!(lru.total_bytes(), 25);
        lru.touch(m(1), 5);
        assert_eq!(lru.total_bytes(), 5);
    }

    #[test]
    fn most_recent_survives_alone_over_budget() {
        let mut lru = Lru::default();
        lru.set_budget(Some(10));
        lru.touch(m(1), 100);
        assert!(lru.evict_over_budget().is_empty());
        assert!(lru.contains(m(1)));
        lru.touch(m(2), 100);
        assert_eq!(lru.evict_over_budget(), vec![m(1)]);
        assert!(lru.contains(m(2)));
    }

    #[test]
    fn remove_then_evict_ignores_the_removed_entry() {
        let mut lru = Lru::default();
        lru.touch(m(1), 10);
        lru.touch(m(2), 10);
        lru.touch(m(3), 10);
        lru.remove(m(1));
        lru.remove(m(1));
        lru.remove(m(99));
        assert_eq!(lru.total_bytes(), 20);
        lru.set_budget(Some(10));
        assert_eq!(lru.evict_over_budget(), vec![m(2)]);
        assert!(!lru.contains(m(1)));
    }

    #[test]
    fn clone_is_independent() {
        let mut a = Lru::default();
        a.touch(m(1), 10);
        a.touch(m(2), 10);
        let b = a.clone();
        a.touch(m(3), 10);
        a.remove(m(1));
        a.set_budget(Some(5));
        assert_eq!(b.len(), 2);
        assert!(b.contains(m(1)));
        assert!(!b.contains(m(3)));
        assert_eq!(b.total_bytes(), 20);
        assert_eq!(b.budget(), None);
        assert_eq!(a.len(), 2);
    }

    #[derive(Clone, Debug)]
    enum Op {
        Touch(u32, usize),
        Remove(u32),
        Evict,
        Budget(Option<usize>),
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            4 => (0u32..8, 0usize..50).prop_map(|(s, b)| Op::Touch(s, b)),
            2 => (0u32..8).prop_map(Op::Remove),
            2 => Just(Op::Evict),
            1 => proptest::option::of(0usize..120).prop_map(Op::Budget),
        ]
    }

    /// The naive reference: entries oldest first.
    #[derive(Default)]
    struct Model {
        budget: Option<usize>,
        items: Vec<(MemoRef, usize)>,
    }

    impl Model {
        fn total(&self) -> usize {
            self.items.iter().map(|(_, b)| *b).sum()
        }
        fn evict(&mut self) -> Vec<MemoRef> {
            let mut out = Vec::new();
            if let Some(budget) = self.budget {
                while self.total() > budget && self.items.len() > 1 {
                    out.push(self.items.remove(0).0);
                }
            }
            out
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn matches_naive_model(ops in proptest::collection::vec(op(), 0..60)) {
            let mut lru = Lru::default();
            let mut model = Model::default();
            for op in ops {
                match op {
                    Op::Touch(s, b) => {
                        lru.touch(m(s), b);
                        model.items.retain(|(k, _)| *k != m(s));
                        model.items.push((m(s), b));
                    }
                    Op::Remove(s) => {
                        lru.remove(m(s));
                        model.items.retain(|(k, _)| *k != m(s));
                    }
                    Op::Evict => {
                        prop_assert_eq!(lru.evict_over_budget(), model.evict());
                    }
                    Op::Budget(b) => {
                        lru.set_budget(b);
                        model.budget = b;
                    }
                }
                prop_assert_eq!(lru.len(), model.items.len());
                prop_assert_eq!(lru.total_bytes(), model.total());
                prop_assert_eq!(lru.budget(), model.budget);
                for s in 0..8 {
                    prop_assert_eq!(lru.contains(m(s)), model.items.iter().any(|(k, _)| *k == m(s)));
                }
            }
        }
    }
}
