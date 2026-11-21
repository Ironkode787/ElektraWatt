// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page-local nets: `page_local_nets(page)` (ARCH §6.4, doc 08 §3.3).
//!
//! A local net is a connected set of drawing nodes (function CPs and junctions) of one page, joined by the
//! page's connections of kind `Wire`, `Core`, `Bus` and `Internal`. A connection named as some
//! `Bond.via` is left out (the bond cut, doc 08 §3.7), and so is an earth wire: a connection with one end on an earth
//! point (a pin whose role is PE or FE, a definition of that type there, or a definition or an interruption point named
//! `PE` or `FE`) and the other in a net of the page that holds a 0 V point (the same for M, L-, and a name whose head is
//! `0V`, `0VDC`, `M` or `L-`) or an interruption point that carries one in from another page ([`ZeroVoltIps`]), joined
//! without the connections to earth points. The person who drew that wire earthed the 0 V: it is the bond, and 0 V
//! stays 0 V (the drawn bond, doc 08 §3.7). `Bridge` connections
//! join at the potential level
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
use ew_schema::{Connection, ConnectionKind, Function, FunctionKind, Junction, Page, PotentialType};

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

/// What a drawing node says of the conductor it is on, for the earth wires (doc 08 §3.7, the drawn bond).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mark {
    /// A 0 V point: a pin whose role is M or L-, a potential definition of that type there, or (without a type) a
    /// definition or an interruption point named by a 0 V word ([`word_mark`]).
    ZeroVolt,
    /// An earth point: a pin whose role is PE or FE (an earth symbol, a PE terminal, a PE or FE screw), or a
    /// definition of that type there, or (without a type) a definition or an interruption point named `PE` or `FE`.
    Earth(EarthKind),
}

/// The earth an [`EarthWire`] goes to.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum EarthKind {
    /// Protective earth: the 0 V's PELV bond.
    Protective,
    /// Functional earth: the 0 V earthed for function, which is no PELV bond.
    Functional,
}

/// The mark of a type (`None` for any other).
fn type_mark(t: &PotentialType) -> Option<Mark> {
    match t {
        PotentialType::M | PotentialType::LMinus => Some(Mark::ZeroVolt),
        PotentialType::Pe => Some(Mark::Earth(EarthKind::Protective)),
        PotentialType::Fe => Some(Mark::Earth(EarthKind::Functional)),
        _ => None,
    }
}

/// The mark of a name that is a type's own word (`PE`, `FE`), as doc 08 §3.5 reads such a name; a 0 V also by the head
/// of its name, the part before a dash with something after it, spaces aside, as the supply marks' panel reads a name:
/// `0V`, `0VDC`, `M` or `L-` (`0V`, `0VDC-6`, `0 V DC`, `M-2`; doc 08 §3.7).
fn word_mark(name: &str) -> Option<Mark> {
    let word: String = name
        .to_uppercase()
        .replace('\u{2212}', "-")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let head = match word.find('-') {
        Some(i) if i > 0 && i + 1 < word.len() => &word[..i],
        _ => word.as_str(),
    };
    match (word.as_str(), head) {
        ("PE", _) => Some(Mark::Earth(EarthKind::Protective)),
        ("FE", _) => Some(Mark::Earth(EarthKind::Functional)),
        (_, "0V" | "0VDC" | "M" | "L-") => Some(Mark::ZeroVolt),
        _ => None,
    }
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
    /// The joining connections at the node, and the earth wires that end at it, in ID order.
    pub via: Vec<Id<Connection>>,
}

/// One page-local net.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LocalNet {
    /// The net ID (minimum member CP ID).
    pub id: NetId,
    /// The members in node order.
    pub members: Vec<NetMember>,
    /// The joining connections of the net, and the earth wires ([`EarthWire`]) that end in it, in ID order.
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

/// A wire drawn on the page from a 0 V point to an earth point: the bond it draws (doc 08 §3.7, "the drawn bond"),
/// a PELV bond to protective earth, or the 0 V earthed for function.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct EarthWire {
    /// The connection.
    pub via: Id<Connection>,
    /// Its end on the 0 V side.
    pub zero_volt: EndpointKey,
    /// Its end on the earth point.
    pub earth: EndpointKey,
    /// The earth it goes to.
    pub kind: EarthKind,
}

/// The value of [`PageLocalNets`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LocalNets {
    /// The page.
    pub page: Id<Page>,
    /// The nets in ID order.
    pub nets: Vec<LocalNet>,
    /// The page's connections cut as a bond's `via` or as an [`EarthWire`] (doc 08 §3.7), in ID order.
    pub cut: Vec<Id<Connection>>,
    /// The page's earth wires, in connection ID order.
    pub earth_wires: Vec<EarthWire>,
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

impl Query for PageLocalNets {
    type Key = Id<Page>;
    type Value = LocalNets;
    const FAMILY: &'static str = "page_local_nets";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, page: &Id<Page>) -> Arc<LocalNets> {
        let Base {
            mut dsu,
            mut via,
            mut cut,
            earth_ended,
            mut zero_volt,
        } = base(q, *page);
        // An interruption point that carries a 0 V in from another page is a 0 V point of this page; the set is read
        // as a cut-off point, so a 0 V drawn elsewhere reaches this page only when the points it carries changed.
        let carried = q.get_value_part::<ZeroVoltIps>(());
        zero_volt.extend(
            page_ips(q, *page)
                .into_iter()
                .filter(|f| carried.contains(f))
                .filter_map(|f| ip_cp(q, f))
                .map(CpRef::key),
        );
        // An earth wire: exactly one end on an earth point, the other end in a net (as joined without the
        // connections that wait) that holds a 0 V point. The test reads that net only, so the order of the waiting
        // connections decides nothing.
        let zero_volt_nets: BTreeSet<usize> = zero_volt
            .into_iter()
            .map(|e| {
                let i = dsu.node(e);
                dsu.find(i)
            })
            .collect();
        let mut earth_wires = Vec::new();
        let mut joined = Vec::new();
        for (id, a, b, ma, mb) in earth_ended {
            let other = match (ma, mb) {
                (Some(Mark::Earth(k)), m) if !matches!(m, Some(Mark::Earth(_))) => Some((b, a, k)),
                (m, Some(Mark::Earth(k))) if !matches!(m, Some(Mark::Earth(_))) => Some((a, b, k)),
                _ => None,
            };
            let earth_wire = other.filter(|(z, _, _)| {
                let i = dsu.node(*z);
                zero_volt_nets.contains(&dsu.find(i))
            });
            match earth_wire {
                Some((zero_volt, earth, kind)) => {
                    // Both ends stay nodes of the page, brought in by the earth wire: each side of the bond is a
                    // potential of its own even when nothing else is wired to it.
                    cut.push(id);
                    dsu.node(earth);
                    via.entry(earth).or_default().insert(id);
                    via.entry(zero_volt).or_default().insert(id);
                    earth_wires.push(EarthWire {
                        via: id,
                        zero_volt,
                        earth,
                        kind,
                    });
                }
                None => joined.push((id, a, b)),
            }
        }
        for (id, a, b) in joined {
            join(&mut dsu, &mut via, id, a, b);
        }
        for j in q.idx().junctions_by_page(*page) {
            dsu.node(EndpointKey::Junction(j));
        }
        // Interruption points drawn on the page are nodes even without a connection.
        for f in page_ips(q, *page) {
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
            earth_wires,
            net_of,
        })
    }
}

/// The connections of a node, by node.
type Via = BTreeMap<EndpointKey, BTreeSet<Id<Connection>>>;

/// A connection that ends on an earth point: its ID, its two ends and their marks.
type EarthEnded = (
    Id<Connection>,
    EndpointKey,
    EndpointKey,
    Option<Mark>,
    Option<Mark>,
);

/// A page joined without the connections that end on an earth point: the first half of [`PageLocalNets`], and what
/// [`PageIpNets`] reads of it.
struct Base {
    dsu: Dsu<EndpointKey>,
    via: Via,
    /// The bond cut (doc 08 §3.7).
    cut: Vec<Id<Connection>>,
    /// The connections with an end on an earth point, in ID order: they wait until the rest of the page is joined.
    earth_ended: Vec<EarthEnded>,
    /// The ends of the page's joining connections that are 0 V points.
    zero_volt: Vec<EndpointKey>,
}

/// Joins `a` and `b` by the connection `id`.
fn join(dsu: &mut Dsu<EndpointKey>, via: &mut Via, id: Id<Connection>, a: EndpointKey, b: EndpointKey) {
    let (ia, ib) = (dsu.node(a), dsu.node(b));
    dsu.union(ia, ib);
    via.entry(a).or_default().insert(id);
    via.entry(b).or_default().insert(id);
}

/// The page joined without the connections that end on an earth point (see [`Base`]).
fn base(q: &Q, page: Id<Page>) -> Base {
    // The bond cut (doc 08 §3.7), read as a cut-off point: a bond edit reaches this page only when the cut changed.
    let vias = q.get_value_part::<crate::index::BondVias>(());
    let mut b = Base {
        dsu: Dsu::default(),
        via: BTreeMap::new(),
        cut: Vec::new(),
        earth_ended: Vec::new(),
        zero_volt: Vec::new(),
    };
    let connections = q.connections();
    let defs = q.get::<crate::index::DefPoints>(());
    let functions = q.functions();
    let mut marks: BTreeMap<EndpointKey, Option<Mark>> = BTreeMap::new();
    let mut mark_of = |e: EndpointKey| -> Option<Mark> {
        *marks.entry(e).or_insert_with(|| {
            if let Some(cp) = CpRef::of(e)
                && let Some(m) = q
                    .get::<FunctionCps>(cp.function)
                    .iter()
                    .find(|c| c.idx == cp.idx)
                    .and_then(|c| c.potential_role.as_ref())
                    .and_then(type_mark)
            {
                return Some(m);
            }
            let by_def = defs.at(e).find_map(|d| {
                let d = q.potential_defs().get(d)?;
                match &d.ptype {
                    Some(t) => type_mark(t),
                    None => word_mark(d.name.as_str()),
                }
            });
            by_def.or_else(|| match &functions.get(CpRef::of(e)?.function)?.kind {
                FunctionKind::Ip(ip) => word_mark(ip.name.as_str()),
                _ => None,
            })
        })
    };
    for id in q.idx().connections_by_page(page) {
        let Some(c) = connections.get(id) else { continue };
        if c.page != Some(page) || !joins_net(&c.kind) {
            continue;
        }
        if vias.contains(&id) {
            b.cut.push(id);
            continue;
        }
        let Some((x, y)) = ends(c) else { continue };
        let (mx, my) = (mark_of(x), mark_of(y));
        for (e, m) in [(x, mx), (y, my)] {
            if m == Some(Mark::ZeroVolt) {
                b.zero_volt.push(e);
            }
        }
        if matches!(mx, Some(Mark::Earth(_))) || matches!(my, Some(Mark::Earth(_))) {
            b.earth_ended.push((id, x, y, mx, my));
            continue;
        }
        join(&mut b.dsu, &mut b.via, id, x, y);
    }
    b.earth_ended.sort_by_key(|e| e.0);
    b
}

/// The interruption-point functions drawn on `page`, in ID order.
fn page_ips(q: &Q, page: Id<Page>) -> BTreeSet<Id<Function>> {
    let (reps, functions) = (q.reps(), q.functions());
    q.idx()
        .reps_by_page(page)
        .filter_map(|r| reps.get(r))
        .flat_map(|rep| rep.functions.iter().copied())
        .filter(|f| {
            functions
                .get(*f)
                .is_some_and(|f| matches!(f.kind, FunctionKind::Ip(_)))
        })
        .collect()
}

/// One net of a page, as joined without the connections to earth points, that holds an interruption point.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IpNet {
    /// Whether the net holds a 0 V point.
    pub zero_volt: bool,
    /// Its interruption points, in ID order.
    pub ips: Vec<Id<Function>>,
}

/// `page_ip_nets(page)`: the nets of the page that hold an interruption point, joined without the connections that end
/// on an earth point, in the order of their first point: what [`ZeroVoltIps`] reads of a page. Memo, note-free.
#[derive(Debug)]
pub struct PageIpNets;

impl Query for PageIpNets {
    type Key = Id<Page>;
    type Value = Vec<IpNet>;
    const FAMILY: &'static str = "page_ip_nets";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, page: &Id<Page>) -> Arc<Vec<IpNet>> {
        let Base {
            mut dsu, zero_volt, ..
        } = base(q, *page);
        let zero_roots: BTreeSet<usize> = zero_volt
            .into_iter()
            .map(|e| {
                let i = dsu.node(e);
                dsu.find(i)
            })
            .collect();
        let mut by_root: BTreeMap<usize, IpNet> = BTreeMap::new();
        for f in page_ips(q, *page) {
            let Some(cp) = ip_cp(q, f) else { continue };
            let i = dsu.node(cp.key());
            let root = dsu.find(i);
            by_root
                .entry(root)
                .or_insert_with(|| IpNet {
                    zero_volt: zero_roots.contains(&root),
                    ips: Vec::new(),
                })
                .ips
                .push(f);
        }
        let mut nets: Vec<IpNet> = by_root.into_values().collect();
        nets.sort_by(|a, b| a.ips.cmp(&b.ips));
        Arc::new(nets)
    }
}

/// `zero_volt_ips()`: the interruption points that carry a 0 V (doc 08 §3.7, the drawn bond across pages): the points
/// of a net of [`PageIpNets`] that holds a 0 V point, and every point their pairs reach from there, net by net and page
/// by page. A point that carries a 0 V is a 0 V point of its page's nets, so a wire from it to an earth point is an
/// earth wire wherever the 0 V was named. The pairs are the pairing's ([`IpPairingQ`](crate::ip::IpPairingQ)), which
/// reads no net. Memo, note-free.
#[derive(Debug)]
pub struct ZeroVoltIps;

impl Query for ZeroVoltIps {
    type Key = ();
    type Value = BTreeSet<Id<Function>>;
    const FAMILY: &'static str = "zero_volt_ips";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _: &()) -> Arc<BTreeSet<Id<Function>>> {
        let pairing = q.get::<crate::ip::IpPairingQ>(());
        let mut dsu: Dsu<Id<Function>> = Dsu::default();
        let mut zero = Vec::new();
        for page in q.get::<crate::index::PageIds>(()).keys() {
            for net in q.get::<PageIpNets>(*page).iter() {
                let first = dsu.node(net.ips[0]);
                for f in &net.ips[1..] {
                    let i = dsu.node(*f);
                    dsu.union(first, i);
                }
                if net.zero_volt {
                    zero.push(first);
                }
            }
        }
        for f in pairing.ips.keys() {
            let i = dsu.node(*f);
            for partner in pairing.partners(*f) {
                let j = dsu.node(*partner);
                dsu.union(i, j);
            }
        }
        let roots: BTreeSet<usize> = zero.into_iter().map(|i| dsu.find(i)).collect();
        let mut out = BTreeSet::new();
        for g in dsu.groups() {
            let i = dsu.node(g[0]);
            if roots.contains(&dsu.find(i)) {
                out.extend(g);
            }
        }
        Arc::new(out)
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
