// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Physical wires: `physical_wires_of(potential)` (ARCH §6.4, doc 08 §3.8, doc 03 §9.2).
//!
//! # Extraction
//!
//! The extraction scope is the *bond closure* of the potential: the potential and every potential linked to
//! it by a bond with a `via` connection, transitively (a bond wire is one wire seen from both sides). Its
//! drawing graph holds the `Wire` and `Core` connections inside the scope and the interruption-point pair
//! links. Rings are broken first: connections are applied in ascending ID order over a union-find that holds
//! the pair links, so a connection that closes a cycle is the highest ID of that cycle; it forms no wire
//! (EW-POT-012). The graph is then a forest and every walk terminates.
//!
//! Hubs are the junctions and the IPs of a star group with more than two members. An arm is walked through
//! transparent IPs (one drawn connection, one pair link) to a physical end or another hub. Hub trees are
//! rooted at the hub whose feed arm ends at a CP, resolved with `resolve` of doc 08 §3.8 (with an explicit
//! stack: no recursion depth depends on model content), and every remaining hub-free path between two
//! physical ends is one wire.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::sync::Arc;

use ew_ids::Id;
use ew_model::EndpointKey;
use ew_schema::{
    Bond, Conductor, Connection, ConnectionKind, Function, IpDirection, Junction, JunctionStyle,
    JunctionWiring, Page, WireOverrides,
};

use crate::ip::{IpPairing, IpPairingQ};
use crate::memo_keys::PartKey;
use crate::nets::{CpRef, Dsu, ends};
use crate::note::DeriveNote;
use crate::pages::PageOrderV;
use crate::potentials::{Partition, PotentialId, PotentialPartition};
use crate::query::{Policy, Query};
use crate::stitch::PairMode;
use crate::{DrawnPageOrder, PageRef, Q};

/// What a wire end is.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum EndKind {
    /// A CP of a device function (terminals included).
    Device,
    /// An external connection point (`IpSpec.external`); field wiring (doc 08 §12 item 6).
    External,
    /// An interruption point without a partner: the wire is open-ended (doc 08 §12 item 5).
    Open,
}

/// One end of a physical wire.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct WireEnd {
    /// The CP (an IP CP for `External` and `Open`).
    pub cp: CpRef,
    /// What it is.
    pub kind: EndKind,
}

/// One physical CP-to-CP wire (doc 08 §3.8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PhysicalWire {
    /// The wire ID: the ID of its carrier connection (the target-side constituent).
    pub id: Id<Connection>,
    /// The two ends; `ends[0]` is the end the constituents start from.
    pub ends: [WireEnd; 2],
    /// The constituent connections in order from `ends[0]` (a shared star feed arm included).
    pub constituents: Vec<Id<Connection>>,
    /// The interruption points passed, in path order.
    pub ips: Vec<Id<Function>>,
    /// The junctions passed (hubs), in path order.
    pub junctions: Vec<Id<Junction>>,
    /// The conductor when a constituent is `Core(c)`.
    pub core: Option<Id<Conductor>>,
    /// The bond whose `via` is a constituent (a bond wire, "0V ⇄ PE").
    pub bond: Option<Id<Bond>>,
    /// The potentials of the two ends, deduplicated, ascending (two for a bond wire).
    pub potentials: Vec<PotentialId>,
    /// The pages of the constituents, in page order (the folio column).
    pub pages: Vec<Id<Page>>,
    /// The level-7 values (CDP): the carrier's overrides, members the carrier leaves unset taken from
    /// the other constituents in constituent order.
    pub overrides: WireOverrides,
    /// For each level-7 value of `overrides`, the constituent connection it comes from.
    pub override_sources: OverrideSources,
    /// Whether a constituent asks for a wire marker (`overrides.show_marker`).
    pub marker: bool,
}

/// The constituent connection that supplies each level-7 value of a [`PhysicalWire`] (`None`: unset): the
/// carrier for the values it sets, else the first other constituent, in constituent order, that sets it
/// (a shared star feed arm, the other leg of an interruption-point pair). The provenance
/// `PropSource::Override` names this connection.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct OverrideSources {
    /// Cross-section.
    pub csa: Option<Id<Connection>>,
    /// Colour.
    pub colour: Option<Id<Connection>>,
    /// Wire part.
    pub wire_part: Option<Id<Connection>>,
    /// Insulation type.
    pub insulation: Option<Id<Connection>>,
    /// Wire function.
    pub function: Option<Id<Connection>>,
    /// Length.
    pub length: Option<Id<Connection>>,
    /// The shielded flag (the first constituent that sets it).
    pub shielded: Option<Id<Connection>>,
    /// The twisted flag (the first constituent that sets it).
    pub twisted: Option<Id<Connection>>,
}

/// The value of [`PhysicalWiresOf`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PhysicalWires {
    /// The wires in ID order.
    pub wires: Vec<PhysicalWire>,
    /// The notes of this derivation (also pushed to the `Q`).
    pub notes: Vec<DeriveNote>,
}

impl PhysicalWires {
    /// The wire with this ID.
    pub fn get(&self, id: Id<Connection>) -> Option<&PhysicalWire> {
        self.wires
            .binary_search_by_key(&id, |w| w.id)
            .ok()
            .map(|i| &self.wires[i])
    }
}

/// `physical_wires_of(potential)` (doc 08 §3.8). Initial policy Memo (ARCH §6.4).
#[derive(Debug)]
pub struct PhysicalWiresOf;

impl Query for PhysicalWiresOf {
    type Key = PotentialId;
    type Value = PhysicalWires;
    const FAMILY: &'static str = "physical_wires_of";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, key: &PotentialId) -> Arc<PhysicalWires> {
        // The partition's parts this wire list depends on (WP-S1.07 part 2): the potential, the bonds, every
        // potential of the bond closure, and the pairing of every interruption point in the scope.
        let partition = q.get_part::<PotentialPartition>((), PartKey::Potential(*key));
        let Some(own) = partition.get(*key) else {
            return Arc::new(PhysicalWires::default());
        };
        // The bond records, and the bonds at every potential of the bond closure (below).
        let _ = q.get_part::<PotentialPartition>((), PartKey::Bonds);
        let mut notes: BTreeSet<DeriveNote> = BTreeSet::new();

        // 1. Extraction scope: the bond closure of the potential.
        let closure = bond_closure(&partition, *key);
        let mut scope: BTreeSet<EndpointKey> = BTreeSet::new();
        for id in &closure {
            let _ = q.get_part::<PotentialPartition>((), PartKey::Potential(*id));
            let _ = q.get_part::<PotentialPartition>((), PartKey::BondsOf(*id));
            let Some(p) = partition.get(*id) else { continue };
            for f in &p.ips {
                let _ = q.get_part::<IpPairingQ>((), PartKey::Ip(*f));
            }
            scope.extend(p.cps.iter().map(|c| c.key()));
            scope.extend(p.junctions.iter().map(|j| EndpointKey::Junction(*j)));
        }

        // The pairing the partition was built from (the current one); the parts above cover what is read of it.
        let ip = partition.ip_pairing();

        // 2. The drawing graph: `Wire` and `Core` connections inside the scope, and the IP pair links.
        let connections = q.connections();
        let mut candidates: BTreeSet<Id<Connection>> = BTreeSet::new();
        for n in &scope {
            candidates.extend(q.idx().connections_at(*n));
        }
        let mut edges: BTreeMap<Id<Connection>, (EndpointKey, EndpointKey)> = BTreeMap::new();
        for id in candidates {
            let Some(c) = connections.get(id) else { continue };
            if !matches!(c.kind, ConnectionKind::Wire | ConnectionKind::Core(_)) {
                continue;
            }
            let Some((a, b)) = ends(c) else { continue };
            if scope.contains(&a) && scope.contains(&b) {
                edges.insert(id, (a, b));
            }
        }
        let mut pair_set: BTreeMap<CpRef, BTreeSet<CpRef>> = BTreeMap::new();
        for n in &scope {
            let Some(cp) = CpRef::of(*n) else { continue };
            let Some(info) = ip.ips.get(&cp.function) else {
                continue;
            };
            if info.cp != Some(cp) {
                continue;
            }
            for partner in ip.partners(cp.function) {
                let Some(pcp) = ip.ips.get(partner).and_then(|i| i.cp) else {
                    continue;
                };
                if pcp != cp && scope.contains(&pcp.key()) {
                    pair_set.entry(cp).or_default().insert(pcp);
                }
            }
        }
        let pairs: BTreeMap<CpRef, Vec<CpRef>> = pair_set
            .into_iter()
            .map(|(cp, set)| (cp, set.into_iter().collect()))
            .collect();

        // 9. Rings. The connections are applied in ascending ID order, so a connection that closes a cycle
        // (pair links count as edges) is the highest ID of that cycle. It forms no wire.
        let mut dsu: Dsu<EndpointKey> = Dsu::default();
        for n in &scope {
            dsu.node(*n);
        }
        for (a, ps) in &pairs {
            for p in ps {
                let (x, y) = (dsu.node(a.key()), dsu.node(p.key()));
                dsu.union(x, y);
            }
        }
        let mut broken: Vec<Id<Connection>> = Vec::new();
        for (id, (a, b)) in &edges {
            let (x, y) = (dsu.node(*a), dsu.node(*b));
            if !dsu.union(x, y) {
                broken.push(*id);
            }
        }
        for id in &broken {
            edges.remove(id);
            notes.insert(DeriveNote::Connectivity {
                code: "EW-POT-012",
                subject: id.to_string(),
                detail:
                    "the connection closes a ring in the drawing graph; it is the highest ID of the ring \
                         and forms no physical wire"
                        .to_owned(),
            });
        }

        // 3.-5. Hubs and their arms.
        let mut at: BTreeMap<EndpointKey, Vec<Id<Connection>>> = BTreeMap::new();
        for (id, (a, b)) in &edges {
            at.entry(*a).or_default().push(*id);
            at.entry(*b).or_default().push(*id);
        }
        let mut drawing = Drawing {
            ip,
            edges,
            at,
            pairs,
            hub_of_cp: BTreeMap::new(),
            hub_of_junction: BTreeMap::new(),
            hubs: Vec::new(),
        };
        let specs = drawing.hub_specs(q, &scope, &mut notes);
        drawing.build_hubs(specs);

        // 6.-7. Resolve the hub trees.
        let mut raw: Vec<RawWire> = Vec::new();
        drawing.resolve_hubs(&mut raw, &mut notes);

        // 8. Hub-free paths.
        drawing.hub_free_wires(&mut raw);

        // 10.-11. Fields; only wires that touch the potential itself.
        let page_order = q.get::<DrawnPageOrder>(());
        let mut bond_of_via: BTreeMap<Id<Connection>, Id<Bond>> = BTreeMap::new();
        for b in partition.bonds() {
            if let Some(v) = b.via {
                bond_of_via.entry(v).or_insert(b.bond);
            }
        }
        let own_nodes: BTreeSet<EndpointKey> = own
            .cps
            .iter()
            .map(|c| c.key())
            .chain(own.junctions.iter().map(|j| EndpointKey::Junction(*j)))
            .collect();
        let mut wires: Vec<PhysicalWire> = Vec::new();
        for w in raw {
            let touches = w.conns.iter().any(|c| {
                drawing
                    .edges
                    .get(c)
                    .is_some_and(|(a, b)| own_nodes.contains(a) || own_nodes.contains(b))
            });
            if !touches {
                continue;
            }
            let ctx = FinishCtx {
                q,
                partition: &partition,
                page_order: &page_order,
                bond_of_via: &bond_of_via,
            };
            if let Some(wire) = ctx.finish(w, &mut notes) {
                wires.push(wire);
            }
        }
        wires.sort_by_key(|w| w.id);
        wires.dedup_by_key(|w| w.id);

        let notes: Vec<DeriveNote> = notes.into_iter().collect();
        for n in &notes {
            q.note(n.clone());
        }
        Arc::new(PhysicalWires { wires, notes })
    }
}

/// The potential, plus every potential linked to it by a bond with a `via` connection, transitively.
fn bond_closure(partition: &Partition, key: PotentialId) -> BTreeSet<PotentialId> {
    let mut set = BTreeSet::from([key]);
    loop {
        let mut grew = false;
        for b in partition.bonds() {
            if b.via.is_none() {
                continue;
            }
            let (Some(x), Some(y)) = (b.a, b.b) else { continue };
            if set.contains(&x) != set.contains(&y) {
                set.insert(x);
                set.insert(y);
                grew = true;
            }
        }
        if !grew {
            return set;
        }
    }
}

/// A hub: a junction, or the IPs of a star group with more than two members.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum HubKey {
    Junction(Id<Junction>),
    /// A star IP group, by its source function.
    Ip(Id<Function>),
}

impl fmt::Display for HubKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HubKey::Junction(j) => write!(f, "{j}"),
            HubKey::Ip(i) => write!(f, "{i}"),
        }
    }
}

/// A walked piece of the drawing graph: connections in walk order, with the IPs and junctions passed.
#[derive(Clone, Default, Debug)]
struct Path {
    conns: Vec<Id<Connection>>,
    ips: Vec<Id<Function>>,
    junctions: Vec<Id<Junction>>,
}

impl Path {
    fn append(&mut self, other: &Path) {
        self.conns.extend_from_slice(&other.conns);
        self.ips.extend_from_slice(&other.ips);
        self.junctions.extend_from_slice(&other.junctions);
    }

    /// Appends `other` walked backwards.
    fn append_reversed(&mut self, other: &Path) {
        self.conns.extend(other.conns.iter().rev());
        self.ips.extend(other.ips.iter().rev());
        self.junctions.extend(other.junctions.iter().rev());
    }

    fn reversed(&self) -> Path {
        let mut p = Path::default();
        p.append_reversed(self);
        p
    }

    /// Records that the path passes `hub`.
    fn pass(&mut self, hub: HubKey) {
        if let HubKey::Junction(j) = hub {
            self.junctions.push(j);
        }
    }
}

/// Where a walk ended.
#[derive(Clone, Copy, Debug)]
enum Reached {
    /// A physical end.
    End(CpRef, EndKind),
    /// A hub (index into [`Drawing::hubs`]).
    Hub(usize),
    /// Nothing usable (cannot happen on a forest; kept so that no model content can panic).
    Dead,
}

/// How a CP behaves in a walk.
enum Class {
    Hub(usize),
    /// A transparent IP, with its pair partner.
    Transparent(CpRef),
    End(EndKind),
}

struct Arm {
    /// From the hub: the connections and the IPs passed (a hub member IP included at either end).
    path: Path,
    reached: Reached,
}

struct HubSpec {
    key: HubKey,
    star: bool,
    starts: Vec<(EndpointKey, Id<Connection>)>,
}

struct Hub {
    key: HubKey,
    star: bool,
    /// Arm 0 is the feed arm as authored.
    arms: Vec<Arm>,
}

/// One wire before its fields are derived.
struct RawWire {
    start: (CpRef, EndKind),
    end: (CpRef, EndKind),
    conns: Vec<Id<Connection>>,
    ips: Vec<Id<Function>>,
    junctions: Vec<Id<Junction>>,
    carrier: Id<Connection>,
}

/// A CP reached while resolving a hub, with the path back to the hub being resolved (that hub's junction is
/// the last junction of the path).
#[derive(Clone)]
struct Cur {
    cp: CpRef,
    kind: EndKind,
    path: Path,
}

struct Frame {
    hub: usize,
    entering: Cur,
    cur: Cur,
    advanced: bool,
    next: usize,
    pending: Option<usize>,
}

impl Frame {
    fn new(hub: usize, entering: Cur) -> Frame {
        Frame {
            hub,
            cur: entering.clone(),
            entering,
            advanced: false,
            next: 0,
            pending: None,
        }
    }
}

struct Drawing<'a> {
    ip: &'a IpPairing,
    edges: BTreeMap<Id<Connection>, (EndpointKey, EndpointKey)>,
    at: BTreeMap<EndpointKey, Vec<Id<Connection>>>,
    pairs: BTreeMap<CpRef, Vec<CpRef>>,
    hub_of_cp: BTreeMap<CpRef, usize>,
    hub_of_junction: BTreeMap<Id<Junction>, usize>,
    hubs: Vec<Hub>,
}

impl Drawing<'_> {
    fn drawn_at(&self, at: EndpointKey) -> &[Id<Connection>] {
        self.at.get(&at).map_or(&[], Vec::as_slice)
    }

    fn other_end(&self, c: Id<Connection>, from: EndpointKey) -> Option<EndpointKey> {
        let (a, b) = self.edges.get(&c)?;
        Some(if *a == from { *b } else { *a })
    }

    fn class_of(&self, cp: CpRef) -> Class {
        if let Some(&h) = self.hub_of_cp.get(&cp) {
            return Class::Hub(h);
        }
        let Some(info) = self.ip.ips.get(&cp.function) else {
            return Class::End(EndKind::Device);
        };
        if info.external {
            return Class::End(EndKind::External);
        }
        let links = self.pairs.get(&cp).map_or(&[][..], Vec::as_slice);
        match (links, self.drawn_at(cp.key()).len()) {
            ([p], 1) => Class::Transparent(*p),
            _ => Class::End(EndKind::Open),
        }
    }

    fn next_to_target(&self, c: Id<Connection>) -> bool {
        self.edges.get(&c).is_some_and(|(a, b)| {
            [a, b]
                .into_iter()
                .filter_map(|k| CpRef::of(*k))
                .filter_map(|cp| self.ip.ips.get(&cp.function))
                .any(|i| i.direction == IpDirection::Target)
        })
    }

    /// The carrier of a hub-free path (doc 08 §3.8): the end constituent next to a Target IP; if both or
    /// neither are, the end constituent with the smaller ID; a one-connection path is its own carrier. (A
    /// wire resolved at a hub is carried by its arm's connection that touches the end CP instead.)
    fn carrier(&self, conns: &[Id<Connection>]) -> Option<Id<Connection>> {
        let (first, last) = (*conns.first()?, *conns.last()?);
        if first == last {
            return Some(first);
        }
        Some(match (self.next_to_target(first), self.next_to_target(last)) {
            (true, false) => first,
            (false, true) => last,
            _ => first.min(last),
        })
    }

    /// The junction hubs of the scope (arms: the sequence, then connections missing from it in ID order)
    /// and the star IP hubs. A star group of more than two members in scope whose source has no single
    /// drawn connection is no hub (its wires end open) and is noted (EW-POT-011).
    fn hub_specs(
        &mut self,
        q: &Q,
        scope: &BTreeSet<EndpointKey>,
        notes: &mut BTreeSet<DeriveNote>,
    ) -> Vec<HubSpec> {
        let mut specs: Vec<HubSpec> = Vec::new();
        let junctions = q.junctions();
        for n in scope {
            let EndpointKey::Junction(j) = *n else { continue };
            let drawn: Vec<Id<Connection>> = self.drawn_at(*n).to_vec();
            if drawn.is_empty() {
                continue;
            }
            let row = junctions.get(j);
            let mut seen: BTreeSet<Id<Connection>> = BTreeSet::new();
            let mut starts = Vec::new();
            if let Some(row) = row {
                for c in &row.sequence {
                    let on_junction = self.edges.get(c).is_some_and(|(a, b)| a == n || b == n);
                    if on_junction && seen.insert(*c) {
                        starts.push((*n, *c));
                    }
                }
            }
            for c in drawn {
                if seen.insert(c) {
                    starts.push((*n, c));
                }
            }
            // Absent or unknown wiring: the style's default (Dot: star, Bevel: chain, unknown: star).
            let star = match row.map(|r| (r.wiring.as_ref(), &r.style)) {
                Some((Some(JunctionWiring::Star), _)) => true,
                Some((Some(JunctionWiring::Chain), _)) => false,
                Some((_, JunctionStyle::Bevel(_))) => false,
                _ => true,
            };
            self.hub_of_junction.insert(j, specs.len());
            specs.push(HubSpec {
                key: HubKey::Junction(j),
                star,
                starts,
            });
        }
        // The star groups with a point in the scope, in group order (a group whose source is outside the scope
        // is no hub of this scope). Scanning only the scope's points keeps the hub specification linear in the
        // potential (WP-S1.07 part 2: it scanned every group of the project per potential).
        let groups: BTreeSet<usize> = scope
            .iter()
            .filter_map(|n| CpRef::of(*n))
            .filter_map(|cp| self.ip.ips.get(&cp.function).filter(|i| i.cp == Some(cp)))
            .filter_map(|i| i.group)
            .collect();
        for group in groups.into_iter().filter_map(|g| self.ip.groups.get(g)) {
            if group.mode != PairMode::Star {
                continue;
            }
            let Some(source) = group.source else { continue };
            let Some(scp) = self.ip.ips.get(&source).and_then(|i| i.cp) else {
                continue;
            };
            if !scope.contains(&scp.key()) {
                continue;
            }
            let mut members = vec![scp];
            for p in self.ip.partners(source) {
                if let Some(pcp) = self.ip.ips.get(p).and_then(|i| i.cp)
                    && pcp != scp
                    && scope.contains(&pcp.key())
                {
                    members.push(pcp);
                }
            }
            if members.len() <= 2 {
                continue;
            }
            // The feed arm is the one drawn connection of the source; otherwise the group is no hub.
            let [feed] = self.drawn_at(scp.key()) else {
                notes.insert(DeriveNote::Connectivity {
                    code: "EW-POT-011",
                    subject: source.to_string(),
                    detail: format!(
                        "the star source of interruption point group {} has {} drawn connections (one \
                         expected); the group is no hub and its wires end open",
                        group.name,
                        self.drawn_at(scp.key()).len()
                    ),
                });
                continue;
            };
            let mut starts = vec![(scp.key(), *feed)];
            for m in &members[1..] {
                starts.extend(self.drawn_at(m.key()).iter().map(|c| (m.key(), *c)));
            }
            let h = specs.len();
            for m in &members {
                self.hub_of_cp.insert(*m, h);
            }
            specs.push(HubSpec {
                key: HubKey::Ip(source),
                star: true,
                starts,
            });
        }
        specs
    }

    fn build_hubs(&mut self, specs: Vec<HubSpec>) {
        let mut hubs = Vec::with_capacity(specs.len());
        for spec in specs {
            let arms = spec
                .starts
                .iter()
                .map(|(from, c)| {
                    let (path, reached) = self.walk(*from, *c);
                    Arm { path, reached }
                })
                .collect();
            hubs.push(Hub {
                key: spec.key,
                star: spec.star,
                arms,
            });
        }
        self.hubs = hubs;
    }

    /// Walks from the node `start` along `first` through transparent IPs to a physical end or a hub.
    fn walk(&self, start: EndpointKey, first: Id<Connection>) -> (Path, Reached) {
        let mut path = Path::default();
        if let Some(cp) = CpRef::of(start)
            && self.ip.ips.contains_key(&cp.function)
        {
            path.ips.push(cp.function);
        }
        let mut seen: BTreeSet<Id<Connection>> = BTreeSet::new();
        let (mut from, mut conn) = (start, first);
        loop {
            if !seen.insert(conn) {
                return (path, Reached::Dead);
            }
            path.conns.push(conn);
            let Some(next) = self.other_end(conn, from) else {
                return (path, Reached::Dead);
            };
            let cp = match next {
                EndpointKey::Junction(j) => {
                    let reached = self
                        .hub_of_junction
                        .get(&j)
                        .map_or(Reached::Dead, |h| Reached::Hub(*h));
                    return (path, reached);
                }
                EndpointKey::Cp(f, idx) => CpRef::new(f, idx),
            };
            match self.class_of(cp) {
                Class::Hub(h) => {
                    path.ips.push(cp.function);
                    return (path, Reached::Hub(h));
                }
                Class::End(kind) => return (path, Reached::End(cp, kind)),
                Class::Transparent(partner) => {
                    path.ips.push(cp.function);
                    path.ips.push(partner.function);
                    match self.class_of(partner) {
                        Class::Transparent(_) => {
                            let Some(&only) = self.drawn_at(partner.key()).first() else {
                                return (path, Reached::Dead);
                            };
                            from = partner.key();
                            conn = only;
                        }
                        Class::End(kind) => return (path, Reached::End(partner, kind)),
                        Class::Hub(_) => return (path, Reached::Dead),
                    }
                }
            }
        }
    }

    /// Roots every hub tree, resolves it (doc 08 §3.8 `resolve`) and notes ambiguous feeds (EW-POT-011).
    fn resolve_hubs(&self, out: &mut Vec<RawWire>, notes: &mut BTreeSet<DeriveNote>) {
        let n = self.hubs.len();
        // links[h]: (arm of h, hub it reaches, the arm of that hub on the same segment).
        let mut links: Vec<Vec<(usize, usize, Option<usize>)>> = vec![Vec::new(); n];
        let mut dsu: Dsu<usize> = Dsu::default();
        for (h, hub) in self.hubs.iter().enumerate() {
            dsu.node(h);
            for (i, arm) in hub.arms.iter().enumerate() {
                let Reached::Hub(h2) = arm.reached else { continue };
                if h2 >= n {
                    continue;
                }
                let last = arm.path.conns.last();
                let k = self.hubs[h2]
                    .arms
                    .iter()
                    .position(|a| last.is_some() && a.path.conns.first() == last);
                links[h].push((i, h2, k));
                let (x, y) = (dsu.node(h), dsu.node(h2));
                dsu.union(x, y);
            }
        }
        let end_arm = |h: usize| {
            self.hubs[h]
                .arms
                .iter()
                .position(|a| matches!(a.reached, Reached::End(..)))
        };
        let mut feed: Vec<usize> = vec![0; n];
        let mut done: BTreeSet<usize> = BTreeSet::new();
        for comp in dsu.groups() {
            let roots: Vec<usize> = comp
                .iter()
                .copied()
                .filter(|&h| {
                    matches!(
                        self.hubs[h].arms.first().map(|a| a.reached),
                        Some(Reached::End(..))
                    )
                })
                .collect();
            let bad = comp.iter().any(|&h| {
                links[h].iter().any(|&(i, _, k)| match k {
                    Some(k) => (i == 0) == (k == 0),
                    None => true,
                })
            });
            let consistent = !bad && roots.len() == 1;
            if !consistent {
                let subject = comp
                    .first()
                    .map_or_else(String::new, |&h| self.hubs[h].key.to_string());
                notes.insert(DeriveNote::Connectivity {
                    code: "EW-POT-011",
                    subject,
                    detail: "an arm between two junctions is the feed arm of both or of neither, or the \
                             junctions have no root; the tree is rooted at the lowest junction ID"
                        .to_owned(),
                });
            }
            // The unique root of a consistent tree, else the lowest hubs that have an arm ending at a CP.
            let mut candidates: VecDeque<usize> = if consistent {
                roots.iter().copied().collect()
            } else {
                comp.iter().copied().filter(|&h| end_arm(h).is_some()).collect()
            };
            while let Some(root) = candidates.pop_front() {
                if done.contains(&root) {
                    continue;
                }
                let Some(root_feed) = (if consistent { Some(0) } else { end_arm(root) }) else {
                    continue;
                };
                let Some(Reached::End(cp, kind)) = self.hubs[root].arms.get(root_feed).map(|a| a.reached)
                else {
                    continue;
                };
                // Orient the tree away from the root: the feed arm of a child is its arm back to the parent.
                feed[root] = root_feed;
                let mut tree = vec![root];
                let mut seen: BTreeSet<usize> = BTreeSet::from([root]);
                let mut qi = 0;
                while let Some(&h) = tree.get(qi) {
                    qi += 1;
                    for &(i, h2, k) in &links[h] {
                        if i != feed[h]
                            && let Some(k) = k
                            && !done.contains(&h2)
                            && seen.insert(h2)
                        {
                            feed[h2] = k;
                            tree.push(h2);
                        }
                    }
                }
                let mut path = self.hubs[root].arms[root_feed].path.reversed();
                path.pass(self.hubs[root].key);
                done.insert(root);
                self.resolve(root, &feed, Cur { cp, kind, path }, &mut done, out);
            }
        }
    }

    /// `resolve(h, entering)` of doc 08 §3.8 over an explicit stack. A `Cur` carries the path from its CP back
    /// to the hub being resolved, so that the constituents of a wire are the path that brought the start CP
    /// to the hub followed by the arm path to the end.
    fn resolve(
        &self,
        root: usize,
        feed: &[usize],
        entering: Cur,
        done: &mut BTreeSet<usize>,
        out: &mut Vec<RawWire>,
    ) {
        let mut stack = vec![Frame::new(root, entering)];
        // The value a finished child returns to its parent: `Some(last CP)` after a chain advanced.
        let mut returned: Option<Option<Cur>> = None;
        while let Some(top) = stack.last_mut() {
            let hub = &self.hubs[top.hub];
            if let Some(ret) = returned.take()
                && let Some(i) = top.pending.take()
                && !hub.star
                && let Some(last) = ret
            {
                let mut path = last.path;
                path.append_reversed(&hub.arms[i].path);
                path.pass(hub.key);
                top.cur = Cur {
                    cp: last.cp,
                    kind: last.kind,
                    path,
                };
                top.advanced = true;
            }
            let mut i = top.next;
            if feed.get(top.hub) == Some(&i) {
                i += 1;
            }
            let Some(arm) = hub.arms.get(i) else {
                let result = (!hub.star && top.advanced).then(|| top.cur.clone());
                stack.pop();
                returned = Some(result);
                continue;
            };
            top.next = i + 1;
            let base = if hub.star { &top.entering } else { &top.cur };
            let mut push: Option<Frame> = None;
            match arm.reached {
                Reached::End(c, kind) => {
                    // Doc 08 §3.8: "carrier = connection of a that touches c". The arm is walked from the
                    // hub, so its last connection is the one that touches the end CP `c`; the IP directions
                    // play no part on a hub arm (review decision D1).
                    if let Some(&carrier) = arm.path.conns.last() {
                        let mut full = base.path.clone();
                        full.append(&arm.path);
                        out.push(RawWire {
                            start: (base.cp, base.kind),
                            end: (c, kind),
                            conns: full.conns,
                            ips: full.ips,
                            junctions: full.junctions,
                            carrier,
                        });
                    }
                    if !hub.star {
                        let mut path = arm.path.reversed();
                        path.pass(hub.key);
                        top.cur = Cur { cp: c, kind, path };
                        top.advanced = true;
                    }
                }
                Reached::Hub(h2) => {
                    if let Some(h2_hub) = self.hubs.get(h2)
                        && done.insert(h2)
                    {
                        let mut path = base.path.clone();
                        path.append(&arm.path);
                        path.pass(h2_hub.key);
                        let sub = Cur {
                            cp: base.cp,
                            kind: base.kind,
                            path,
                        };
                        top.pending = Some(i);
                        push = Some(Frame::new(h2, sub));
                    }
                }
                Reached::Dead => {}
            }
            if let Some(f) = push {
                stack.push(f);
            }
        }
    }

    /// Every path between two physical ends that passes no hub is one wire.
    fn hub_free_wires(&self, out: &mut Vec<RawWire>) {
        let mut used: BTreeSet<Id<Connection>> = BTreeSet::new();
        for (node, conns) in &self.at {
            let Some(cp) = CpRef::of(*node) else { continue };
            let Class::End(kind) = self.class_of(cp) else {
                continue;
            };
            for c in conns {
                if used.contains(c) {
                    continue;
                }
                let (path, reached) = self.walk(*node, *c);
                let Reached::End(cp2, kind2) = reached else {
                    continue;
                };
                let Some(carrier) = self.carrier(&path.conns) else {
                    continue;
                };
                used.extend(path.conns.iter().copied());
                out.push(RawWire {
                    start: (cp, kind),
                    end: (cp2, kind2),
                    conns: path.conns,
                    ips: path.ips,
                    junctions: path.junctions,
                    carrier,
                });
            }
        }
    }
}

/// What deriving the fields of a wire reads.
struct FinishCtx<'a> {
    q: &'a Q,
    partition: &'a Partition,
    page_order: &'a PageOrderV,
    bond_of_via: &'a BTreeMap<Id<Connection>, Id<Bond>>,
}

impl FinishCtx<'_> {
    /// The fields of one wire (doc 08 §3.8, doc 03 §9.2).
    fn finish(&self, w: RawWire, notes: &mut BTreeSet<DeriveNote>) -> Option<PhysicalWire> {
        let connections = self.q.connections();
        let rows: Vec<(Id<Connection>, &Arc<Connection>)> = w
            .conns
            .iter()
            .filter_map(|id| connections.get(*id).map(|c| (*id, c)))
            .collect();
        let carrier_row = rows.iter().find(|(id, _)| *id == w.carrier).map(|(_, c)| *c)?;
        let carrier = w.carrier;
        let note14 = |notes: &mut BTreeSet<DeriveNote>, detail: String| {
            notes.insert(DeriveNote::Connectivity {
                code: "EW-POT-014",
                subject: carrier.to_string(),
                detail,
            });
        };

        // The conductor: the carrier's, else the first in constituent order.
        let cores: Vec<(Id<Connection>, Id<Conductor>)> = rows
            .iter()
            .filter_map(|(id, c)| match &c.kind {
                ConnectionKind::Core(k) => Some((*id, *k)),
                _ => None,
            })
            .collect();
        let core = match &carrier_row.kind {
            ConnectionKind::Core(k) => Some(*k),
            _ => cores.first().map(|(_, k)| *k),
        };
        if let Some(core) = core {
            for (id, k) in &cores {
                if *k != core {
                    note14(
                        notes,
                        format!("constituent {id} names conductor {k}; conductor {core} applies"),
                    );
                }
            }
        }

        // Level-7 values: the carrier's, members it leaves unset from the other constituents in order.
        let mut overrides: WireOverrides = carrier_row.overrides.clone();
        let carrier_sets = |on: bool| on.then_some(carrier);
        let mut sources = OverrideSources {
            csa: carrier_sets(overrides.csa.is_some()),
            colour: carrier_sets(overrides.colour.is_some()),
            wire_part: carrier_sets(overrides.wire_part.is_some()),
            insulation: carrier_sets(overrides.insulation.is_some()),
            function: carrier_sets(overrides.function.is_some()),
            length: carrier_sets(overrides.length.is_some()),
            shielded: carrier_sets(overrides.flags.shielded),
            twisted: carrier_sets(overrides.flags.twisted),
        };
        let mut conflict: Option<Id<Connection>> = None;
        for (id, c) in rows.iter().filter(|(id, _)| *id != carrier) {
            let o = &c.overrides;
            macro_rules! merge {
                ($($f:ident),*) => {$(
                    match (&overrides.$f, &o.$f) {
                        (None, Some(v)) => {
                            overrides.$f = Some(v.clone());
                            sources.$f = Some(*id);
                        }
                        (Some(cur), Some(v)) if cur != v => conflict = conflict.or(Some(*id)),
                        _ => {}
                    }
                )*};
            }
            merge!(csa, colour, wire_part, insulation, function, length);
            if o.flags.shielded && !overrides.flags.shielded {
                overrides.flags.shielded = true;
                sources.shielded = Some(*id);
            }
            if o.flags.twisted && !overrides.flags.twisted {
                overrides.flags.twisted = true;
                sources.twisted = Some(*id);
            }
        }
        if let Some(id) = conflict {
            note14(
                notes,
                format!(
                    "constituent {id} overrides a value that the wire already has; the first value applies"
                ),
            );
        }
        let marker = rows.iter().any(|(_, c)| c.overrides.show_marker);

        let bond = w.conns.iter().find_map(|c| self.bond_of_via.get(c).copied());
        let mut potentials: Vec<PotentialId> = [w.start.0, w.end.0]
            .iter()
            .filter_map(|cp| self.partition.potential_at(cp.key()).map(|p| p.id))
            .collect();
        potentials.sort();
        potentials.dedup();
        let mut pages: Vec<Id<Page>> = rows.iter().filter_map(|(_, c)| c.page).collect();
        pages.sort_by_key(|p| {
            (
                self.page_order.position(PageRef::Drawn(*p)).unwrap_or(usize::MAX),
                *p,
            )
        });
        pages.dedup();

        let end = |(cp, kind): (CpRef, EndKind)| WireEnd { cp, kind };
        Some(PhysicalWire {
            id: carrier,
            ends: [end(w.start), end(w.end)],
            constituents: w.conns,
            ips: w.ips,
            junctions: w.junctions,
            core,
            bond,
            potentials,
            pages,
            overrides,
            override_sources: sources,
            marker,
        })
    }
}
