// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The [`Query`] trait and the request-scoped cold cache of [`Q`](crate::Q) (ADR-0002 decisions 1–3).

use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

/// How a query family is evaluated (ADR-0002 decisions 2–3, ARCH §6.4 "Initial policy").
///
/// A `Q` without a memo store evaluates every family cold with its request-scoped cache. A `Q` from a
/// [`MemoStore`](crate::MemoStore) memoises the `Memo` and `MemoLru` families in the store across requests
/// (ADR-0002 amendment 4); promoting or demoting a family is a one-line change of its `POLICY`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Policy {
    /// Recomputed on demand; cached only within one `Q` (and within one memo execution).
    Cold,
    /// Memoised with red-green verification, push invalidation and early cut-off.
    Memo,
    /// Memoised and evicted by the byte-budget LRU (bulky values).
    MemoLru,
}

/// What changed for an incremental family since its memo was last verified ([`Query::run_incremental`]):
/// the touched inputs of the commits in between and the dependencies through which the memo was marked.
#[derive(Clone, Copy, Debug)]
pub struct Changes<'a> {
    pub(crate) inner: &'a ew_memo::Changes<crate::MemoInput>,
}

impl<'a> Changes<'a> {
    /// The inputs the commits since the last verification touched (fine reads and their coarse keys,
    /// [`MemoInput`](crate::MemoInput)), in revision order; `None` when the store no longer holds them.
    pub fn touched(&self) -> Option<&'a [crate::MemoInput]> {
        self.inner.touched.as_deref()
    }

    /// The dependencies through which the memo was marked possibly changed; `None` when unknown. Map a
    /// memo or part entry to its family's key with [`Q::via_key`](crate::Q::via_key).
    pub fn via(&self) -> Option<&'a [ew_memo::DepKey<crate::MemoInput>]> {
        self.inner.via.as_deref()
    }
}

/// A family of derived values: a pure function `fn(q: &Q, key) -> Arc<V>` (ADR-0002 decision 1).
///
/// `run` receives only `&Q`, whose accessors are all tracked; it must not read anything else (no clock,
/// no environment, no global state), iterate no hash map into its result, use no floating point, and
/// round ties toward zero. Callers go through [`Q::get`](crate::Q::get), never `run`, so that the cache,
/// the memo store and the tracker see every evaluation.
///
/// Values are compared (`Eq`) for early cut-off and by the oracle, which checks that a memoised value equals
/// a cold `run` (ADR-0002 decision 4).
pub trait Query: 'static {
    /// The key; its order is the order of the family's memo table.
    type Key: Ord + Clone + fmt::Debug + Send + Sync + 'static;
    /// The value, shared behind an `Arc`.
    type Value: fmt::Debug + Eq + Send + Sync + 'static;
    /// The family name, unique in the crate (see [`FAMILIES`](crate::FAMILIES)).
    const FAMILY: &'static str;
    /// The initial policy.
    const POLICY: Policy;

    /// Whether memoised evaluations go through [`Query::run_incremental`] (ADR-0002 amendment 4).
    const INCREMENTAL: bool = false;

    /// Whether early cut-off and the oracle compare [`Query::fingerprint`]s (128 bits) instead of the values:
    /// for bulky values whose comparison would cost as much as the change.
    const FINGERPRINT: bool = false;

    /// Computes the value for `key`: the cold reference.
    fn run(q: &crate::Q, key: &Self::Key) -> Arc<Self::Value>;

    /// Computes the value in a memoised evaluation of an [`INCREMENTAL`](Query::INCREMENTAL) family, from the
    /// previous value and what changed since (`None`: from scratch). It starts with the previous memo's
    /// dependencies recorded; it must re-read every dependency its new value relies on that `changes` names,
    /// account for every entry of [`Changes::via`], and declare the parts it changed when it is a producer.
    /// The result must equal [`Query::run`]; the oracle checks it.
    fn run_incremental(
        q: &crate::Q,
        key: &Self::Key,
        previous: Option<(&Arc<Self::Value>, Changes<'_>)>,
    ) -> Arc<Self::Value> {
        let _ = previous;
        Self::run(q, key)
    }

    /// The bytes a value occupies (the memory gate, NFR-PERF-13; ADR-0002 amendment 4 item (d)).
    fn bytes(_value: &Self::Value) -> usize {
        std::mem::size_of::<Self::Value>()
    }

    /// The 128-bit fingerprint of a value, for [`FINGERPRINT`](Query::FINGERPRINT) families: equal values have
    /// equal fingerprints, and different ones differ with overwhelming probability.
    fn fingerprint(_value: &Self::Value) -> u128 {
        0
    }
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

    /// Removes the slot of `key` (a running evaluation that unwound).
    pub(crate) fn abort<Qy: Query>(&mut self, key: &Qy::Key) {
        self.family::<Qy>().remove(key);
    }

    /// The number of cached keys (running or done) of `family`.
    pub(crate) fn len_of(&self, family: &str) -> usize {
        self.families.get(family).map_or(0, |f| (f.len)(f.map.as_ref()))
    }
}
