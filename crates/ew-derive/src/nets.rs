// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page-local nets: `page_local_nets(page)` (ARCH §6.4, doc 08 §3.3).
//!
//! A local net is a connected set of drawing nodes (function CPs and junctions) of one page, joined by the
//! page's connections of kind `Wire`, `Core`, `Bus` and `Internal`. A connection named as some
//! `Bond.via` is left out (the bond cut, doc 08 §3.7); `Bridge` connections join at the potential level
//! and `Overview` and unknown kinds never join (CI-07). Every interruption point drawn on the page is a
//! node even without a connection, so that the stitch graph can pair it.
//!
//! The net ID is the minimum member CP ID (`v5(function, idx)`, doc 08 §3.4); a net without a CP (a
//! hostile junction-only cycle) takes its minimum junction ID. Union-find sets do not depend on the order
//! in which edges are applied, so the result is the same for every record order.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::{Id, v5};
use ew_model::EndpointKey;
use ew_schema::{Connection, ConnectionKind, Function, FunctionKind, Junction, Page};

use crate::query::{Policy, Query};
use crate::{Cp, FunctionCps, Q};

/// A connection point by function and index (doc 08 §2.1). Its ID is `v5(function, idx)`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CpRef {
    /// The function.
    pub function: Id<Function>,
    /// The CP index.
    pub idx: u16,
}

impl CpRef {
    /// The CP `idx` of `function`.
    pub fn new(function: Id<Function>, idx: u16) -> CpRef {
        CpRef { function, idx }
    }

    /// The derived CP ID `v5(function, idx)` (never stored).
    pub fn id(self) -> Id<Cp> {
        v5::cp_id(self.function, u32::from(self.idx))
    }

    /// The endpoint key of the CP.
    pub fn key(self) -> EndpointKey {
        EndpointKey::Cp(self.function, self.idx)
    }

    /// The CP of an endpoint key; `None` for a junction.
    pub fn of(at: EndpointKey) -> Option<CpRef> {
        match at {
            EndpointKey::Cp(f, idx) => Some(CpRef::new(f, idx)),
            EndpointKey::Junction(_) => None,
        }
    }
}

/// The ID of a net or signal: its minimum member CP ID (doc 08 §3.4).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct NetId(pub Id<Cp>);

impl NetId {
    /// The ID of a set of nodes: the minimum CP ID, else (no CP) the minimum junction ID.
    pub fn of(nodes: impl IntoIterator<Item = EndpointKey>) -> Option<NetId> {
        let mut min_cp: Option<Id<Cp>> = None;
        let mut min_junction: Option<Id<Junction>> = None;
        for n in nodes {
            match n {
                EndpointKey::Cp(f, idx) => {
                    let id = CpRef::new(f, idx).id();
                    min_cp = Some(min_cp.map_or(id, |m| m.min(id)));
                }
                EndpointKey::Junction(j) => min_junction = Some(min_junction.map_or(j, |m| m.min(j))),
            }
        }
        min_cp.or_else(|| min_junction.map(Id::cast)).map(NetId)
    }
}

/// Whether a connection of this kind joins nets (doc 08 §3.3): `Wire`, `Core`, `Bus`, `Internal`.
pub fn joins_net(kind: &ConnectionKind) -> bool {
    matches!(
        kind,
        ConnectionKind::Wire | ConnectionKind::Core(_) | ConnectionKind::Bus(_) | ConnectionKind::Internal
    )
}

/// The connections named as `Bond.via` (the bond cut, doc 08 §3.7). Reads the whole bond table.
pub(crate) fn bond_vias(q: &Q) -> BTreeSet<Id<Connection>> {
    q.bonds().iter().filter_map(|(_, b)| b.via).collect()
}

/// The endpoint keys of a connection; `None` for an endpoint of a newer format.
pub(crate) fn ends(c: &Connection) -> Option<(EndpointKey, EndpointKey)> {
    Some((EndpointKey::of(&c.a)?, EndpointKey::of(&c.b)?))
}

/// The first CP of an interruption-point function (its one CP, `ew.fn.ip` index 0).
pub(crate) fn ip_cp(q: &Q, ip: Id<Function>) -> Option<CpRef> {
    q.get::<FunctionCps>(ip).first().map(|c| CpRef::new(ip, c.idx))
}

/// One member of a local net with the connections that brought it in (its provenance); empty for an
/// interruption point or junction that is on the page but has no joining connection.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NetMember {
    /// The node.
    pub at: EndpointKey,
    /// The joining connections at the node, in ID order.
    pub via: Vec<Id<Connection>>,
}

/// One page-local net.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LocalNet {
    /// The net ID (minimum member CP ID).
    pub id: NetId,
    /// The members in node order.
    pub members: Vec<NetMember>,
    /// The joining connections of the net, in ID order.
    pub connections: Vec<Id<Connection>>,
}

impl LocalNet {
    /// The member CPs in order.
    pub fn cps(&self) -> impl Iterator<Item = CpRef> + '_ {
        self.members.iter().filter_map(|m| CpRef::of(m.at))
    }

    /// The member junctions in order.
    pub fn junctions(&self) -> impl Iterator<Item = Id<Junction>> + '_ {
        self.members.iter().filter_map(|m| match m.at {
            EndpointKey::Junction(j) => Some(j),
            EndpointKey::Cp(..) => None,
        })
    }
}

/// The value of [`PageLocalNets`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LocalNets {
    /// The page.
    pub page: Id<Page>,
    /// The nets in ID order.
    pub nets: Vec<LocalNet>,
    /// The page's connections cut as a bond's `via` (doc 08 §3.7), in ID order.
    pub cut: Vec<Id<Connection>>,
    net_of: BTreeMap<EndpointKey, usize>,
}

impl LocalNets {
    /// The net of a node of this page.
    pub fn net_of(&self, at: EndpointKey) -> Option<&LocalNet> {
        self.net_of.get(&at).map(|&i| &self.nets[i])
    }
}

/// `page_local_nets(page)`: the page's nets (doc 08 §3.3). Initial policy Memo (ARCH §6.4).
#[derive(Debug)]
pub struct PageLocalNets;

impl crate::heap::HeapBytes for LocalNets {
    fn heap_bytes(&self) -> usize {
        self.nets.heap_bytes() + self.cut.heap_bytes() + self.net_of.heap_bytes()
    }
}

impl Query for PageLocalNets {
    type Key = Id<Page>;
    type Value = LocalNets;
    const FAMILY: &'static str = "page_local_nets";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, page: &Id<Page>) -> Arc<LocalNets> {
        let vias = bond_vias(q);
        let mut dsu = Dsu::default();
        let mut via: BTreeMap<EndpointKey, BTreeSet<Id<Connection>>> = BTreeMap::new();
        let mut cut = Vec::new();
        let connections = q.connections();
        for id in q.idx().connections_by_page(*page) {
            let Some(c) = connections.get(id) else { continue };
            if c.page != Some(*page) || !joins_net(&c.kind) {
                continue;
            }
            if vias.contains(&id) {
                cut.push(id);
                continue;
            }
            let Some((a, b)) = ends(c) else { continue };
            let (ia, ib) = (dsu.node(a), dsu.node(b));
            dsu.union(ia, ib);
            via.entry(a).or_default().insert(id);
            via.entry(b).or_default().insert(id);
        }
        for j in q.idx().junctions_by_page(*page) {
            dsu.node(EndpointKey::Junction(j));
        }
        // Interruption points drawn on the page are nodes even without a connection.
        let reps = q.reps();
        let functions = q.functions();
        let mut ips = BTreeSet::new();
        for r in q.idx().reps_by_page(*page) {
            let Some(rep) = reps.get(r) else { continue };
            for f in &rep.functions {
                if functions
                    .get(*f)
                    .is_some_and(|f| matches!(f.kind, FunctionKind::Ip(_)))
                {
                    ips.insert(*f);
                }
            }
        }
        for f in ips {
            if let Some(cp) = ip_cp(q, f) {
                dsu.node(cp.key());
            }
        }
        let groups = dsu.groups();
        let mut nets: Vec<LocalNet> = groups
            .into_iter()
            .filter_map(|members| {
                let id = NetId::of(members.iter().copied())?;
                let mut conns = BTreeSet::new();
                let members = members
                    .into_iter()
                    .map(|at| {
                        let v: Vec<Id<Connection>> = via
                            .get(&at)
                            .map(|s| s.iter().copied().collect())
                            .unwrap_or_default();
                        conns.extend(v.iter().copied());
                        NetMember { at, via: v }
                    })
                    .collect();
                Some(LocalNet {
                    id,
                    members,
                    connections: conns.into_iter().collect(),
                })
            })
            .collect();
        nets.sort_by(|x, y| {
            x.id.cmp(&y.id)
                .then_with(|| x.members[0].at.cmp(&y.members[0].at))
        });
        let mut net_of = BTreeMap::new();
        for (i, n) in nets.iter().enumerate() {
            for m in &n.members {
                net_of.insert(m.at, i);
            }
        }
        cut.sort();
        Arc::new(LocalNets {
            page: *page,
            nets,
            cut,
            net_of,
        })
    }
}

/// A disjoint-set forest over keyed nodes (union by size, iterative path halving): the union-find of the
/// connectivity queries. Sets do not depend on the order of `union` calls; `groups` lists them in key order.
#[derive(Debug)]
pub(crate) struct Dsu<K: Ord + Copy> {
    index: BTreeMap<K, usize>,
    keys: Vec<K>,
    parent: Vec<usize>,
    size: Vec<usize>,
}

impl<K: Ord + Copy> Default for Dsu<K> {
    fn default() -> Self {
        Dsu {
            index: BTreeMap::new(),
            keys: Vec::new(),
            parent: Vec::new(),
            size: Vec::new(),
        }
    }
}

impl<K: Ord + Copy> Dsu<K> {
    /// The index of `key`, adding it as a singleton when new.
    pub(crate) fn node(&mut self, key: K) -> usize {
        if let Some(&i) = self.index.get(&key) {
            return i;
        }
        let i = self.keys.len();
        self.index.insert(key, i);
        self.keys.push(key);
        self.parent.push(i);
        self.size.push(1);
        i
    }

    /// The representative of `i`.
    pub(crate) fn find(&mut self, mut i: usize) -> usize {
        while self.parent[i] != i {
            self.parent[i] = self.parent[self.parent[i]];
            i = self.parent[i];
        }
        i
    }

    /// Joins the sets of `a` and `b`; whether they were apart.
    pub(crate) fn union(&mut self, a: usize, b: usize) -> bool {
        let (mut ra, mut rb) = (self.find(a), self.find(b));
        if ra == rb {
            return false;
        }
        if self.size[ra] < self.size[rb] {
            std::mem::swap(&mut ra, &mut rb);
        }
        self.parent[rb] = ra;
        self.size[ra] += self.size[rb];
        true
    }

    /// The sets, each in key order, ordered by their first key.
    pub(crate) fn groups(&mut self) -> Vec<Vec<K>> {
        let mut by_root: BTreeMap<usize, Vec<K>> = BTreeMap::new();
        let entries: Vec<(K, usize)> = self.index.iter().map(|(k, i)| (*k, *i)).collect();
        for (k, i) in entries {
            let r = self.find(i);
            by_root.entry(r).or_default().push(k);
        }
        let mut groups: Vec<Vec<K>> = by_root.into_values().collect();
        groups.sort_by(|a, b| a[0].cmp(&b[0]));
        groups
    }
}
