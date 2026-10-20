// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Derived-index families of the connectivity queries (ARCH §6.3 rule 5, §6.4 `stitch_graph`; ADR-0002
//! amendment 4: "no whole-table read on an interactive path").
//!
//! The stitch graph and the partition need a few sparse sets out of large tables: the pages, the page-less
//! joining connections and every bridge, the interruption-point functions, the potential definitions, the
//! bonds and the rail-bonded terminals. Reading those tables whole would invalidate every reader on every
//! edit of any row. Each set is instead one memoised **index family** with an incremental hook:
//!
//! - **Cold** (`run`, the oracle's reference, and the first memoised execution) the index reads its table
//!   whole and selects the rows it keeps.
//! - **Incremental** (`run_incremental`) it starts from its previous value and re-selects only the rows of its
//!   table that the commits since its last verification touched ([`Changes::touched`] holds every put row's
//!   key, `memo_keys::touched`); it re-reads the table's stamp, so its dependency list stays exact. Without the
//!   touched list (an old memo, a trimmed log) it runs cold.
//!
//! An index is a persistent map ([`Index`]): an update copies only the paths it changes, and comparing two
//! versions (early cut-off, the readers' diffs) skips the subtrees they share. A row edit that changes no
//! entry leaves an equal index, which cuts off, so the readers of the index stay valid.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use ew_ids::Id;
use ew_model::{EndpointKey, Entity, RowId};
use ew_schema::PotentialDef;
use ew_schema::{Bond, BondKind, Connection, ConnectionKind, Device, Function, FunctionKind, IpSpec, Page};
use imbl::OrdMap;

use crate::Q;
use crate::heap::HeapBytes;
use crate::memo_keys::MemoInput;
use crate::nets::{ends, joins_net};
use crate::query::{Changes, Policy, Query};
use crate::tracker::DepKey;

// ---------------------------------------------------------------------------------------------------------
// The persistent map

/// A read-only persistent map, the value of an index family: in key order, O(1) to clone, and compared by a
/// diff that skips the subtrees two versions share.
#[derive(Clone, PartialEq, Eq)]
pub struct Index<K: Ord + Clone, V: Clone> {
    map: OrdMap<K, V>,
}

impl<K: Ord + Clone, V: Clone> Default for Index<K, V> {
    fn default() -> Self {
        Index { map: OrdMap::new() }
    }
}

impl<K: Ord + Clone + fmt::Debug, V: Clone + fmt::Debug> fmt::Debug for Index<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.map.iter()).finish()
    }
}

impl<K: Ord + Clone, V: Clone> Index<K, V> {
    /// The entry of `key`.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    /// Whether `key` has an entry.
    pub fn contains(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    /// The entries in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> + '_ {
        self.map.iter()
    }

    /// The keys in order.
    pub fn keys(&self) -> impl Iterator<Item = &K> + '_ {
        self.map.keys()
    }

    /// The entries whose key lies in `range`, in key order.
    pub fn range<R: std::ops::RangeBounds<K>>(&self, range: R) -> impl Iterator<Item = (&K, &V)> + '_ {
        self.map.range(range)
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether there is no entry.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl<K: Ord + Clone, V: Clone + PartialEq> Index<K, V> {
    /// Sets the entry of `key` (`None`: removes it). An equal entry is left as it is, so unchanged paths stay
    /// shared with earlier versions.
    pub(crate) fn set(&mut self, key: K, value: Option<V>) {
        match value {
            Some(v) => {
                if self.map.get(&key) != Some(&v) {
                    self.map.insert(key, v);
                }
            }
            None => {
                if self.map.contains_key(&key) {
                    self.map.remove(&key);
                }
            }
        }
    }

    /// The keys whose entries differ between `self` and `other` (added, removed or changed), in key order.
    /// Subtrees the two versions share are skipped.
    // Read by the incremental partition (the next step of WP-S1.07 part 2).
    #[allow(dead_code)]
    pub(crate) fn changed_keys(&self, other: &Index<K, V>) -> Vec<K> {
        let mut out: Vec<K> = self
            .map
            .diff(&other.map)
            .map(|d| match d {
                imbl::ordmap::DiffItem::Add(k, _)
                | imbl::ordmap::DiffItem::Remove(k, _)
                | imbl::ordmap::DiffItem::Update { old: (k, _), .. } => k.clone(),
            })
            .collect();
        out.dedup();
        out
    }
}

impl<K: Ord + Clone, V: Clone> FromIterator<(K, V)> for Index<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Index {
            map: iter.into_iter().collect(),
        }
    }
}

impl<K: Ord + Clone + HeapBytes, V: Clone + HeapBytes> HeapBytes for Index<K, V> {
    fn heap_bytes(&self) -> usize {
        // Entries plus a node overhead of two words per entry (an estimate, ARCH §6.3 rule 4).
        self.map
            .iter()
            .map(|(k, v)| {
                std::mem::size_of::<K>()
                    + std::mem::size_of::<V>()
                    + 2 * std::mem::size_of::<usize>()
                    + k.heap_bytes()
                    + v.heap_bytes()
            })
            .sum()
    }
}

impl HeapBytes for () {
    fn heap_bytes(&self) -> usize {
        0
    }
}

impl HeapBytes for BondRec {
    fn heap_bytes(&self) -> usize {
        self.kind.heap_bytes()
    }
}

impl HeapBytes for Link {
    fn heap_bytes(&self) -> usize {
        0
    }
}

impl HeapBytes for IpSpec {
    fn heap_bytes(&self) -> usize {
        self.name.heap_bytes() + self.direction.heap_bytes()
    }
}

// ---------------------------------------------------------------------------------------------------------
// The incremental table index

/// The row IDs of kind `T` among the touched inputs (`memo_keys::touched` lists `Read(Row(key))` for every
/// put row).
fn touched_rows<T: Entity>(touched: &[MemoInput]) -> BTreeSet<RowId<T>> {
    touched
        .iter()
        .filter_map(|i| match i {
            MemoInput::Read(DepKey::Row((kind, id))) if *kind == T::KIND => Some(id.cast()),
            _ => None,
        })
        .collect()
}

/// An index over the rows of `T`: the entry of a row is `select(q, id, row)`, `None` leaves the row out.
///
/// Cold (no previous value, or no touched list) it reads the table whole. Incremental it re-selects the
/// touched rows of `T` from `previous` (absent rows lose their entry) and re-reads the table's stamp (`len`),
/// the dependency the cold run recorded. `select` must read nothing but the row (and values that are equal
/// for every row of one snapshot, which a caller checks itself).
pub(crate) fn table_index<T: Entity, V: Clone + PartialEq>(
    q: &Q,
    previous: Option<(&Index<RowId<T>, V>, Changes<'_>)>,
    select: impl Fn(RowId<T>, &T) -> Option<V>,
) -> Index<RowId<T>, V> {
    let view = q.table::<T>();
    let Some((prev, touched)) = previous.and_then(|(p, c)| c.touched().map(|t| (p, t))) else {
        return view
            .iter()
            .filter_map(|(id, row)| select(id, row).map(|v| (id, v)))
            .collect();
    };
    let _ = view.len();
    let mut index = prev.clone();
    for id in touched_rows::<T>(touched) {
        let entry = view.get(id).and_then(|row| select(id, row));
        index.set(id, entry);
    }
    index
}

// ---------------------------------------------------------------------------------------------------------
// Pages

/// `page_ids()`: every page, in ID order (an incremental index of the page table; a renumber re-selects its
/// pages and cuts off).
#[derive(Debug)]
pub struct PageIds;

impl Query for PageIds {
    type Key = ();
    type Value = Index<Id<Page>, ()>;
    const FAMILY: &'static str = "page_ids";
    const POLICY: Policy = Policy::Memo;
    const INCREMENTAL: bool = true;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<Self::Value> {
        Self::run_incremental(q, &(), None)
    }

    fn run_incremental(
        q: &Q,
        _: &(),
        previous: Option<(&Arc<Self::Value>, Changes<'_>)>,
    ) -> Arc<Self::Value> {
        Arc::new(table_index::<Page, ()>(
            q,
            previous.map(|(p, c)| (p.as_ref(), c)),
            |_, _| Some(()),
        ))
    }
}

// ---------------------------------------------------------------------------------------------------------
// Bonds

/// One bond as the connectivity reads it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BondRec {
    /// The first endpoint (`None`: of a newer format).
    pub a: Option<EndpointKey>,
    /// The second endpoint.
    pub b: Option<EndpointKey>,
    /// The kind.
    pub kind: BondKind,
    /// The via connection (the bond cut, doc 08 §3.7).
    pub via: Option<Id<Connection>>,
}

/// `bond_index()`: every bond, by ID (incremental over the bond table).
#[derive(Debug)]
pub struct BondIndex;

impl Query for BondIndex {
    type Key = ();
    type Value = Index<Id<Bond>, BondRec>;
    const FAMILY: &'static str = "bond_index";
    const POLICY: Policy = Policy::Memo;
    const INCREMENTAL: bool = true;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<Self::Value> {
        Self::run_incremental(q, &(), None)
    }

    fn run_incremental(
        q: &Q,
        _: &(),
        previous: Option<(&Arc<Self::Value>, Changes<'_>)>,
    ) -> Arc<Self::Value> {
        Arc::new(table_index::<Bond, BondRec>(
            q,
            previous.map(|(p, c)| (p.as_ref(), c)),
            |_, b| {
                Some(BondRec {
                    a: EndpointKey::of(&b.a),
                    b: EndpointKey::of(&b.b),
                    kind: b.kind.clone(),
                    via: b.via,
                })
            },
        ))
    }
}

/// `bond_vias()`: the connections named as some `Bond.via` (the bond cut, doc 08 §3.7), in ID order. Read by
/// `page_local_nets` and the page-less index as a cut-off point.
#[derive(Debug)]
pub struct BondVias;

impl Query for BondVias {
    type Key = ();
    type Value = BTreeSet<Id<Connection>>;
    const FAMILY: &'static str = "bond_vias";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<Self::Value> {
        Arc::new(q.get::<BondIndex>(()).iter().filter_map(|(_, b)| b.via).collect())
    }
}

// ---------------------------------------------------------------------------------------------------------
// Page-less connections and bridges

/// A connection the stitch graph joins outside the page-local nets.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Link {
    /// The first end.
    pub a: EndpointKey,
    /// The second end.
    pub b: EndpointKey,
    /// A `Bridge` (potential level, drawn or page-less); else a page-less `Wire`, `Core`, `Bus` or `Internal`
    /// connection (net level) that no bond cuts.
    pub bridge: bool,
}

/// The value of [`PagelessLinks`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PagelessLinksV {
    /// The links by connection ID.
    pub links: Index<Id<Connection>, Link>,
    /// The links at each end: `(end, connection)` pairs.
    at: Index<(EndpointKey, Id<Connection>), ()>,
    /// The bond cut the selection used.
    cut: Arc<BTreeSet<Id<Connection>>>,
}

impl PagelessLinksV {
    /// The links with an end at `e`, in connection ID order.
    pub fn at(&self, e: EndpointKey) -> impl Iterator<Item = (Id<Connection>, &Link)> + '_ {
        let start = (e, Id::from_uuid(Default::default()));
        self.at
            .range(start..)
            .take_while(move |((end, _), _)| *end == e)
            .filter_map(|((_, c), _)| self.links.get(c).map(|l| (*c, l)))
    }

    /// Whether some link ends at `e`.
    pub fn touches(&self, e: EndpointKey) -> bool {
        self.at(e).next().is_some()
    }

    fn set(&mut self, id: Id<Connection>, link: Option<Link>) {
        if self.links.get(&id) == link.as_ref() {
            return;
        }
        if let Some(old) = self.links.get(&id).copied() {
            self.at.set((old.a, id), None);
            self.at.set((old.b, id), None);
        }
        if let Some(new) = link {
            self.at.set((new.a, id), Some(()));
            self.at.set((new.b, id), Some(()));
        }
        self.links.set(id, link);
    }
}

impl HeapBytes for PagelessLinksV {
    fn heap_bytes(&self) -> usize {
        self.links.heap_bytes() + self.at.heap_bytes() + self.cut.heap_bytes()
    }
}

/// `pageless_links()`: every `Bridge` connection and every page-less joining connection that no bond cuts
/// (doc 08 §3.3), by ID, with an index by end. Incremental over the connection table; a change of the bond
/// cut re-selects every row.
#[derive(Debug)]
pub struct PagelessLinks;

/// The link of a connection row, under the bond cut `cut`.
fn link_of(id: Id<Connection>, c: &Connection, cut: &BTreeSet<Id<Connection>>) -> Option<Link> {
    let (a, b) = ends(c)?;
    match &c.kind {
        ConnectionKind::Bridge(_) => Some(Link { a, b, bridge: true }),
        k if joins_net(k) && c.page.is_none() && !cut.contains(&id) => Some(Link { a, b, bridge: false }),
        _ => None,
    }
}

impl Query for PagelessLinks {
    type Key = ();
    type Value = PagelessLinksV;
    const FAMILY: &'static str = "pageless_links";
    const POLICY: Policy = Policy::Memo;
    const INCREMENTAL: bool = true;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<Self::Value> {
        Self::run_incremental(q, &(), None)
    }

    fn run_incremental(
        q: &Q,
        _: &(),
        previous: Option<(&Arc<Self::Value>, Changes<'_>)>,
    ) -> Arc<Self::Value> {
        let cut = q.get_value_part::<BondVias>(());
        let connections = q.table::<Connection>();
        let incremental = previous
            .filter(|(p, _)| p.cut == cut)
            .and_then(|(p, c)| c.touched().map(|t| (p, t)));
        let Some((prev, touched)) = incremental else {
            let mut v = PagelessLinksV {
                cut: Arc::clone(&cut),
                ..PagelessLinksV::default()
            };
            for (id, c) in connections.iter() {
                v.set(id, link_of(id, c, &cut));
            }
            return Arc::new(v);
        };
        let _ = connections.len();
        let mut v = (**prev).clone();
        v.cut = cut;
        for id in touched_rows::<Connection>(touched) {
            let link = connections.get(id).and_then(|c| link_of(id, c, &v.cut));
            v.set(id, link);
        }
        Arc::new(v)
    }
}

// ---------------------------------------------------------------------------------------------------------
// Interruption-point functions

/// `ip_functions()`: the specification of every interruption-point function, by ID (incremental over the
/// function table).
#[derive(Debug)]
pub struct IpFunctions;

impl Query for IpFunctions {
    type Key = ();
    type Value = Index<Id<Function>, IpSpec>;
    const FAMILY: &'static str = "ip_functions";
    const POLICY: Policy = Policy::Memo;
    const INCREMENTAL: bool = true;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<Self::Value> {
        Self::run_incremental(q, &(), None)
    }

    fn run_incremental(
        q: &Q,
        _: &(),
        previous: Option<(&Arc<Self::Value>, Changes<'_>)>,
    ) -> Arc<Self::Value> {
        Arc::new(table_index::<Function, IpSpec>(
            q,
            previous.map(|(p, c)| (p.as_ref(), c)),
            |_, f| match &f.kind {
                FunctionKind::Ip(spec) => Some(spec.clone()),
                _ => None,
            },
        ))
    }
}

// ---------------------------------------------------------------------------------------------------------
// Potential definitions

/// The value of [`DefPoints`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DefPointsV {
    /// The definition point of every potential definition with a known endpoint, by ID.
    pub defs: Index<Id<PotentialDef>, EndpointKey>,
    /// `(endpoint, definition)` pairs.
    at: Index<(EndpointKey, Id<PotentialDef>), ()>,
}

impl DefPointsV {
    /// The definitions placed at `e`, in ID order.
    pub fn at(&self, e: EndpointKey) -> impl Iterator<Item = Id<PotentialDef>> + '_ {
        self.at
            .range((e, Id::from_uuid(Default::default()))..)
            .take_while(move |((end, _), _)| *end == e)
            .map(|((_, d), _)| *d)
    }

    fn set(&mut self, id: Id<PotentialDef>, at: Option<EndpointKey>) {
        if self.defs.get(&id) == at.as_ref() {
            return;
        }
        if let Some(old) = self.defs.get(&id).copied() {
            self.at.set((old, id), None);
        }
        if let Some(new) = at {
            self.at.set((new, id), Some(()));
        }
        self.defs.set(id, at);
    }
}

impl HeapBytes for DefPointsV {
    fn heap_bytes(&self) -> usize {
        self.defs.heap_bytes() + self.at.heap_bytes()
    }
}

/// `def_points()`: where each potential definition is placed (incremental over the definition table).
#[derive(Debug)]
pub struct DefPoints;

impl Query for DefPoints {
    type Key = ();
    type Value = DefPointsV;
    const FAMILY: &'static str = "def_points";
    const POLICY: Policy = Policy::Memo;
    const INCREMENTAL: bool = true;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<Self::Value> {
        Self::run_incremental(q, &(), None)
    }

    fn run_incremental(
        q: &Q,
        _: &(),
        previous: Option<(&Arc<Self::Value>, Changes<'_>)>,
    ) -> Arc<Self::Value> {
        let defs = q.table::<PotentialDef>();
        let Some((prev, touched)) = previous.and_then(|(p, c)| c.touched().map(|t| (p, t))) else {
            let mut v = DefPointsV::default();
            for (id, d) in defs.iter() {
                v.set(id, EndpointKey::of(&d.at));
            }
            return Arc::new(v);
        };
        let _ = defs.len();
        let mut v = (**prev).clone();
        for id in touched_rows::<PotentialDef>(touched) {
            v.set(id, defs.get(id).and_then(|d| EndpointKey::of(&d.at)));
        }
        Arc::new(v)
    }
}

// ---------------------------------------------------------------------------------------------------------
// Rail-bonded terminals

/// `rail_terminals()`: the terminal devices with `rail_bonded` set, in ID order (incremental over the device
/// table).
#[derive(Debug)]
pub struct RailTerminals;

impl Query for RailTerminals {
    type Key = ();
    type Value = Index<Id<Device>, ()>;
    const FAMILY: &'static str = "rail_terminals";
    const POLICY: Policy = Policy::Memo;
    const INCREMENTAL: bool = true;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<Self::Value> {
        Self::run_incremental(q, &(), None)
    }

    fn run_incremental(
        q: &Q,
        _: &(),
        previous: Option<(&Arc<Self::Value>, Changes<'_>)>,
    ) -> Arc<Self::Value> {
        Arc::new(table_index::<Device, ()>(
            q,
            previous.map(|(p, c)| (p.as_ref(), c)),
            |_, d| {
                d.aspects
                    .terminal
                    .as_ref()
                    .is_some_and(|t| t.rail_bonded)
                    .then_some(())
            },
        ))
    }
}
