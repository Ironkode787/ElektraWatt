// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reverse-dependency index of one query family.

use imbl::{OrdMap, OrdSet};

use crate::Rev;
use crate::key::{DepKey, SlotId};

/// The reverse-dependency index of one query family (ADR-0002 amendment 1): for every key a memo of the
/// family read, the slots whose recorded reads include it.
///
/// Holds only `imbl` structures, so a clone is O(1) and independent of the original (fork semantics).
#[derive(Clone, Debug)]
pub(crate) struct RevDeps<I: Ord + Clone> {
    /// For each key, the slots that read it.
    by_key: OrdMap<DepKey<I>, OrdSet<SlotId>>,
}

impl<I: Ord + Clone> RevDeps<I> {
    /// An empty index.
    pub(crate) fn new() -> Self {
        RevDeps {
            by_key: OrdMap::new(),
        }
    }

    /// Records that `slot` read every key of `deps` (duplicates are fine).
    pub(crate) fn add(&mut self, slot: SlotId, deps: &[(DepKey<I>, Rev)]) {
        for (key, _) in deps {
            self.by_key.entry(key.clone()).or_default().insert(slot);
        }
    }

    /// Undoes `add` for these deps: removes `slot` from each key's set and drops empty sets.
    pub(crate) fn remove(&mut self, slot: SlotId, deps: &[(DepKey<I>, Rev)]) {
        for (key, _) in deps {
            let now_empty = match self.by_key.get_mut(key) {
                Some(slots) => {
                    slots.remove(&slot);
                    slots.is_empty()
                }
                None => false,
            };
            if now_empty {
                self.by_key.remove(key);
            }
        }
    }

    /// The slots that read `key`, ascending.
    pub(crate) fn get(&self, key: &DepKey<I>) -> Vec<SlotId> {
        self.by_key
            .get(key)
            .map(|slots| slots.iter().copied().collect())
            .unwrap_or_default()
    }

    /// The number of (key, slot) pairs, for memory reports.
    pub(crate) fn entries(&self) -> usize {
        self.by_key.values().map(OrdSet::len).sum()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use proptest::prelude::*;

    use super::*;
    use crate::key::FamilyId;

    fn inp(i: u32) -> DepKey<u32> {
        DepKey::Input(i)
    }

    fn memo(f: u32, s: u32) -> DepKey<u32> {
        DepKey::Memo(FamilyId(f), SlotId(s))
    }

    fn deps(keys: &[DepKey<u32>]) -> Vec<(DepKey<u32>, Rev)> {
        keys.iter().cloned().map(|k| (k, Rev(1))).collect()
    }

    #[test]
    fn add_get_remove() {
        let mut r = RevDeps::<u32>::new();
        assert_eq!(r.entries(), 0);
        assert!(r.get(&inp(1)).is_empty());
        let d = deps(&[inp(1), memo(0, 2)]);
        r.add(SlotId(5), &d);
        r.add(SlotId(3), &deps(&[inp(1)]));
        assert_eq!(r.get(&inp(1)), vec![SlotId(3), SlotId(5)]);
        assert_eq!(r.get(&memo(0, 2)), vec![SlotId(5)]);
        assert_eq!(r.entries(), 3);
        r.remove(SlotId(5), &d);
        assert_eq!(r.get(&inp(1)), vec![SlotId(3)]);
        assert!(r.get(&memo(0, 2)).is_empty());
        assert_eq!(r.entries(), 1);
    }

    #[test]
    fn empty_sets_are_dropped() {
        let mut r = RevDeps::<u32>::new();
        let d = deps(&[inp(1), inp(2), memo(1, 1)]);
        r.add(SlotId(0), &d);
        assert_eq!(r.entries(), 3);
        r.remove(SlotId(0), &d);
        assert_eq!(r.entries(), 0);
        assert!(r.by_key.is_empty());
        // Removing what was never added is a no-op.
        r.remove(SlotId(0), &d);
        assert!(r.by_key.is_empty());
    }

    #[test]
    fn duplicates_are_idempotent() {
        let mut r = RevDeps::<u32>::new();
        let d = deps(&[inp(1), inp(1)]);
        r.add(SlotId(2), &d);
        r.add(SlotId(2), &d);
        assert_eq!(r.entries(), 1);
        r.remove(SlotId(2), &d);
        assert_eq!(r.entries(), 0);
    }

    #[test]
    fn get_is_ascending() {
        let mut r = RevDeps::<u32>::new();
        for s in [9, 2, 7, 4, 0] {
            r.add(SlotId(s), &deps(&[inp(0)]));
        }
        let got = r.get(&inp(0));
        let mut sorted = got.clone();
        sorted.sort();
        assert_eq!(got, sorted);
        assert_eq!(got.len(), 5);
    }

    #[test]
    fn clone_is_independent() {
        let mut a = RevDeps::<u32>::new();
        a.add(SlotId(1), &deps(&[inp(1)]));
        let b = a.clone();
        a.add(SlotId(2), &deps(&[inp(1)]));
        a.remove(SlotId(1), &deps(&[inp(1)]));
        assert_eq!(b.get(&inp(1)), vec![SlotId(1)]);
        assert_eq!(a.get(&inp(1)), vec![SlotId(2)]);
    }

    fn key() -> impl Strategy<Value = DepKey<u32>> {
        prop_oneof![
            (0u32..4).prop_map(DepKey::Input),
            (0u32..2, 0u32..4).prop_map(|(f, s)| DepKey::Memo(FamilyId(f), SlotId(s))),
        ]
    }

    #[derive(Clone, Debug)]
    enum Op {
        Add(u32, Vec<DepKey<u32>>),
        Remove(u32, Vec<DepKey<u32>>),
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            (0u32..6, proptest::collection::vec(key(), 0..5)).prop_map(|(s, k)| Op::Add(s, k)),
            (0u32..6, proptest::collection::vec(key(), 0..5)).prop_map(|(s, k)| Op::Remove(s, k)),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn matches_naive_model(ops in proptest::collection::vec(op(), 0..40)) {
            let mut r = RevDeps::<u32>::new();
            let mut model: BTreeMap<DepKey<u32>, BTreeSet<SlotId>> = BTreeMap::new();
            for op in ops {
                match op {
                    Op::Add(s, keys) => {
                        r.add(SlotId(s), &deps(&keys));
                        for k in keys {
                            model.entry(k).or_default().insert(SlotId(s));
                        }
                    }
                    Op::Remove(s, keys) => {
                        r.remove(SlotId(s), &deps(&keys));
                        for k in keys {
                            if let Some(set) = model.get_mut(&k) {
                                set.remove(&SlotId(s));
                                if set.is_empty() {
                                    model.remove(&k);
                                }
                            }
                        }
                    }
                }
                prop_assert_eq!(r.entries(), model.values().map(BTreeSet::len).sum::<usize>());
                prop_assert_eq!(r.by_key.len(), model.len());
                for (k, set) in &model {
                    prop_assert_eq!(r.get(k), set.iter().copied().collect::<Vec<_>>());
                }
            }
        }
    }
}
