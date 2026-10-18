// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The [`Query`] trait and the request-scoped cold cache of [`Q`](crate::Q) (ADR-0002 decisions 1–3).

use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

/// How a query family is evaluated (ADR-0002 decisions 2–3, ARCH §6.4 "Initial policy").
///
/// Phase 1 evaluates every family cold with the request-scoped cache, whatever its policy; the policy is
/// the declaration the memo engine reads once it exists (the performance spike, G-SP). Promoting or
/// demoting a family is a one-line change of its `POLICY`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Policy {
    /// Recomputed on demand; cached only within one `Q`.
    Cold,
    /// Memoised with red-green verification and early cut-off.
    Memo,
    /// Memoised and evicted by the byte-budget LRU (bulky values).
    MemoLru,
}

/// A family of derived values: a pure function `fn(q: &Q, key) -> Arc<V>` (ADR-0002 decision 1).
///
/// `run` receives only `&Q`, whose accessors are all tracked; it must not read anything else (no clock,
/// no environment, no global state), iterate no hash map into its result, use no floating point, and
/// round ties toward zero. Callers go through [`Q::get`](crate::Q::get), never `run`, so that the cache
/// and the tracker see every evaluation.
pub trait Query: 'static {
    /// The key; its order is the order of the family's memo table.
    type Key: Ord + Clone + fmt::Debug + 'static;
    /// The value, shared behind an `Arc`.
    type Value: fmt::Debug + Send + Sync + 'static;
    /// The family name, unique in the crate (see [`FAMILIES`](crate::FAMILIES)).
    const FAMILY: &'static str;
    /// The initial policy.
    const POLICY: Policy;

    /// Computes the value for `key`.
    fn run(q: &crate::Q, key: &Self::Key) -> Arc<Self::Value>;
}

/// One cache slot: running (for cycle detection) or done.
enum Slot<V> {
    Running,
    Done(Arc<V>),
}

type FamilyMap<Qy> = BTreeMap<<Qy as Query>::Key, Slot<<Qy as Query>::Value>>;

/// One family's erased map with the query type that owns it.
struct Family {
    owner: TypeId,
    map: Box<dyn Any>,
    len: fn(&dyn Any) -> usize,
}

fn erased_len<Qy: Query>(map: &dyn Any) -> usize {
    map.downcast_ref::<FamilyMap<Qy>>().map_or(0, BTreeMap::len)
}

/// The request-scoped cache: per family name, the query type that owns it and its key → slot map.
#[derive(Default)]
pub(crate) struct Cache {
    families: BTreeMap<&'static str, Family>,
}

/// What the cache knows about a key.
pub(crate) enum Lookup<V> {
    Hit(Arc<V>),
    Running,
    Miss,
}

impl Cache {
    fn family<Qy: Query>(&mut self) -> &mut FamilyMap<Qy> {
        let family = self.families.entry(Qy::FAMILY).or_insert_with(|| Family {
            owner: TypeId::of::<Qy>(),
            map: Box::new(FamilyMap::<Qy>::new()),
            len: erased_len::<Qy>,
        });
        assert!(
            family.owner == TypeId::of::<Qy>(),
            "two query types share the family name `{}`",
            Qy::FAMILY
        );
        family
            .map
            .downcast_mut::<FamilyMap<Qy>>()
            .expect("the family map has the owner's type")
    }

    pub(crate) fn lookup<Qy: Query>(&mut self, key: &Qy::Key) -> Lookup<Qy::Value> {
        match self.family::<Qy>().get(key) {
            Some(Slot::Done(v)) => Lookup::Hit(Arc::clone(v)),
            Some(Slot::Running) => Lookup::Running,
            None => Lookup::Miss,
        }
    }

    pub(crate) fn start<Qy: Query>(&mut self, key: Qy::Key) {
        self.family::<Qy>().insert(key, Slot::Running);
    }

    pub(crate) fn finish<Qy: Query>(&mut self, key: Qy::Key, value: Arc<Qy::Value>) {
        self.family::<Qy>().insert(key, Slot::Done(value));
    }

    /// The number of cached keys (running or done) of `family`.
    pub(crate) fn len_of(&self, family: &str) -> usize {
        self.families.get(family).map_or(0, |f| (f.len)(f.map.as_ref()))
    }
}
