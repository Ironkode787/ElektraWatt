// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Per-session dense indexes for IDs.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use uuid::Uuid;

use crate::Id;

/// A dense per-session index of an [`Id<T>`], handed out by an [`Interner<T>`] in first-come order
/// (0, 1, 2, …), for union-find arrays and other hot structures.
///
/// **Never persisted or exported** (ADR-0004): the same ID gets different indexes in different sessions,
/// so the type has no serde implementation.
pub struct Dense<T> {
    index: u32,
    kind: PhantomData<fn() -> T>,
}

impl<T> Dense<T> {
    /// The index.
    pub const fn index(self) -> u32 {
        self.index
    }

    /// The index as `usize`, for indexing vectors.
    pub const fn as_usize(self) -> usize {
        self.index as usize
    }
}

impl<T> fmt::Debug for Dense<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Dense({})", self.index)
    }
}

impl<T> Clone for Dense<T> {
    fn clone(&self) -> Dense<T> {
        *self
    }
}

impl<T> Copy for Dense<T> {}

impl<T> PartialEq for Dense<T> {
    fn eq(&self, other: &Dense<T>) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for Dense<T> {}

impl<T> PartialOrd for Dense<T> {
    fn partial_cmp(&self, other: &Dense<T>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Dense<T> {
    fn cmp(&self, other: &Dense<T>) -> Ordering {
        self.index.cmp(&other.index)
    }
}

impl<T> Hash for Dense<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

/// Interns [`Id<T>`] values to dense [`Dense<T>`] indexes for one session (ADR-0004).
///
/// Lookups use a `BTreeMap` (the determinism policy of doc 18 §2.2 bans hash maps), resolution a
/// `Vec`. Indexes are assigned in first-intern order and never reused.
///
/// ```
/// use ew_ids::{Id, Interner};
///
/// struct Device;
/// let a: Id<Device> = Id::new_v7();
/// let b: Id<Device> = Id::new_v7();
/// let mut interner = Interner::new();
/// let da = interner.intern(a);
/// assert_eq!(da.index(), 0);
/// assert_eq!(interner.intern(b).index(), 1);
/// assert_eq!(interner.intern(a), da);
/// assert_eq!(interner.resolve(da), a);
/// assert_eq!(interner.len(), 2);
/// ```
pub struct Interner<T> {
    by_uuid: BTreeMap<Uuid, u32>,
    ids: Vec<Id<T>>,
}

impl<T> Interner<T> {
    /// An empty interner.
    pub const fn new() -> Interner<T> {
        Interner {
            by_uuid: BTreeMap::new(),
            ids: Vec::new(),
        }
    }

    /// The dense index of `id`, assigning the next free one on first sight.
    ///
    /// # Panics
    ///
    /// If more than 2³² distinct IDs are interned in one session.
    pub fn intern(&mut self, id: Id<T>) -> Dense<T> {
        let next = self.ids.len();
        let index = *self
            .by_uuid
            .entry(id.as_uuid())
            .or_insert_with(|| match u32::try_from(next) {
                Ok(i) => i,
                Err(_) => panic!("more than 2^32 IDs interned in one session"),
            });
        if index as usize == next {
            self.ids.push(id);
        }
        Dense {
            index,
            kind: PhantomData,
        }
    }

    /// The dense index of `id`, if it was interned.
    pub fn get(&self, id: Id<T>) -> Option<Dense<T>> {
        self.by_uuid.get(&id.as_uuid()).map(|&index| Dense {
            index,
            kind: PhantomData,
        })
    }

    /// The ID behind `dense`, or `None` if its index is outside this interner's range.
    ///
    /// A `Dense` carries no interner identity: an index from a different interner that happens to be in
    /// range is not detected and resolves to an unrelated ID of this one. Callers must only pass indices
    /// this interner produced.
    pub fn try_resolve(&self, dense: Dense<T>) -> Option<Id<T>> {
        self.ids.get(dense.as_usize()).copied()
    }

    /// The ID behind `dense`.
    ///
    /// # Panics
    ///
    /// If `dense` did not come from this interner (its index is out of range).
    pub fn resolve(&self, dense: Dense<T>) -> Id<T> {
        match self.try_resolve(dense) {
            Some(id) => id,
            None => panic!("{dense:?} does not belong to this interner"),
        }
    }

    /// The number of interned IDs.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Whether no ID has been interned.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The interned IDs in index order.
    pub fn ids(&self) -> &[Id<T>] {
        &self.ids
    }
}

impl<T> Default for Interner<T> {
    fn default() -> Interner<T> {
        Interner::new()
    }
}

impl<T> Clone for Interner<T> {
    fn clone(&self) -> Interner<T> {
        Interner {
            by_uuid: self.by_uuid.clone(),
            ids: self.ids.clone(),
        }
    }
}

impl<T> fmt::Debug for Interner<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.ids.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    struct K;

    fn id(n: u128) -> Id<K> {
        Id::from_uuid(Uuid::from_u128(n))
    }

    #[test]
    fn interns_in_first_come_order() {
        let mut i = Interner::new();
        assert!(i.is_empty());
        let d9 = i.intern(id(9));
        let d1 = i.intern(id(1));
        assert_eq!((d9.index(), d1.index()), (0, 1));
        assert_eq!(i.intern(id(9)), d9);
        assert_eq!(i.len(), 2);
        assert_eq!(i.get(id(1)), Some(d1));
        assert_eq!(i.get(id(5)), None);
        assert_eq!(i.resolve(d1), id(1));
        assert_eq!(i.ids(), &[id(9), id(1)]);
        assert_eq!(format!("{d1:?}"), "Dense(1)");
        assert_eq!(d1.as_usize(), 1);
    }

    #[test]
    fn foreign_dense_values_are_detected() {
        let mut a = Interner::new();
        let b: Interner<K> = Interner::default();
        let d = a.intern(id(1));
        assert_eq!(b.try_resolve(d), None);
        assert!(std::panic::catch_unwind(|| b.resolve(d)).is_err());
    }

    proptest! {
        #[test]
        fn intern_resolve_round_trip(ns in prop::collection::vec(0u128..50, 0..200)) {
            let mut i = Interner::new();
            let dense: Vec<Dense<K>> = ns.iter().map(|&n| i.intern(id(n))).collect();
            for (&n, &d) in ns.iter().zip(&dense) {
                prop_assert_eq!(i.resolve(d), id(n));
                prop_assert_eq!(i.get(id(n)), Some(d));
            }
            let distinct: std::collections::BTreeSet<u128> = ns.iter().copied().collect();
            prop_assert_eq!(i.len(), distinct.len());
            // Indexes are exactly 0..len.
            let mut idx: Vec<u32> = dense.iter().map(|d| d.index()).collect();
            idx.sort_unstable();
            idx.dedup();
            prop_assert_eq!(idx, (0..i.len() as u32).collect::<Vec<_>>());
        }
    }
}
