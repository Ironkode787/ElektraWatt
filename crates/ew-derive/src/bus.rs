// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bus model's derived data (doc 10 §2.7, §2.10, §3.8; WP-S1.21 D11): the segments of a physical net
//! (`bus_segments(pnet)`, algorithm A5), the members of a logical net (`net_members(lnet)`), the texts of a bus port
//! (its station address and termination, FR-BUS-004, FR-BUS-006) and the member count of a potential that a bus
//! segment carries (folio 909).
//!
//! **A5 as built.** The graph's nodes are the functions and junctions at the ends of `Bus` connections, and every
//! port function of the net. A function is one node: a connector's In, Out and device-side entries are its CPs, so
//! its through-link is the node itself, and the plug-on link is the page-less `Bus` connection between the
//! connector and the port (doc 10 §2.7). The edges are the `Bus` connections; a connection's cable
//! (`BusLine.cable`) is a member of the segment between the two nodes it joins. Two nodes joined by several
//! connections (A and B drawn as two lines) are one edge. A component belongs to every physical net that one of its
//! port functions names. Its ends are its functions with at most one neighbour, and every external interruption
//! point (an IP with an `ExternalRef`). The walk starts at the master end (the end that is the logical net's
//! master, a port or an external interruption point), else at the end with the smallest (`order_hint` position,
//! ID); it visits neighbours in that order, so a branched component (EW-BUS-006's stub, not built) still has one
//! deterministic member order. Junctions carry the line but are not members.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{
    AddrValue, BusPort, ConnectionKind, Device, Endpoint, ExternalRef, Function, FunctionKind, Junction,
    LogicalNet, NetAddress, OnOff, PhysicalNet,
};

use crate::Q;
use crate::potentials::Potential;
use crate::query::{Policy, Query};

/// A member of a bus segment, in walk order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SegmentMember {
    /// A function on the line: a bus port, a connector's port, or an external interruption point at an end.
    Function(Id<Function>),
    /// The cable of the `Bus` connections between the members before and after it (`BusLine.cable`).
    Cable(Id<Device>),
}

/// One segment of a physical net (doc 10 §3.8 A5).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct BusSegment {
    /// The members in walk order from the start end: functions and the cables between them.
    pub members: Vec<SegmentMember>,
    /// The ends: the start end first, then the others in (`order_hint` position, ID) order. Empty for a ring.
    pub ends: Vec<Id<Function>>,
    /// The ends that are external interruption points (the partner's side is outside the project).
    pub external_ends: Vec<Id<Function>>,
    /// The logical net of the segment's ports (the smallest ID when they name several; `None` when none does).
    pub logical_net: Option<Id<LogicalNet>>,
    /// Whether a node has more than two neighbours: a line bus with a branch (EW-BUS-006's stub).
    pub branched: bool,
}

impl BusSegment {
    /// The functions of the segment in walk order.
    pub fn functions(&self) -> impl Iterator<Item = Id<Function>> + '_ {
        self.members.iter().filter_map(|m| match m {
            SegmentMember::Function(f) => Some(*f),
            SegmentMember::Cable(_) => None,
        })
    }
}

/// A node of the A5 graph.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum Node {
    Function(Id<Function>),
    Junction(Id<Junction>),
}

fn node(e: &Endpoint) -> Option<Node> {
    match e {
        Endpoint::Cp(f, _) => Some(Node::Function(*f)),
        Endpoint::Junction(j) => Some(Node::Junction(*j)),
        Endpoint::Unknown(_) => None,
    }
}

/// The bus graph of the snapshot: per node, its neighbours with the cables of the edge between them.
type Graph = BTreeMap<Node, BTreeMap<Node, BTreeSet<Id<Device>>>>;

fn bus_graph(q: &Q) -> Graph {
    let mut graph: Graph = BTreeMap::new();
    for (_, c) in q.connections().iter() {
        let ConnectionKind::Bus(line) = &c.kind else {
            continue;
        };
        let (Some(a), Some(b)) = (node(&c.a), node(&c.b)) else {
            continue;
        };
        if a == b {
            continue;
        }
        for (x, y) in [(a, b), (b, a)] {
            let cables = graph.entry(x).or_default().entry(y).or_default();
            if let Some(cable) = line.cable {
                cables.insert(cable);
            }
        }
    }
    graph
}

/// Whether a function is an external interruption point (an IP whose `ExternalRef` names the other side).
fn is_external_ip(f: &Function) -> bool {
    matches!(&f.kind, FunctionKind::Ip(ip) if ip.external.is_some())
}

/// `bus_segments(pnet)`: the segments of a physical net, algorithm A5 of doc 10 §3.8 (see the module doc), in the
/// order of their start ends. Empty for a net that no port names.
#[derive(Debug)]
pub struct BusSegments;

impl Query for BusSegments {
    type Key = Id<PhysicalNet>;
    type Value = Vec<BusSegment>;
    const FAMILY: &'static str = "bus_segments";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<BusSegment>> {
        let functions = q.functions();
        let Some(pnet) = q.table::<PhysicalNet>().get(*key) else {
            return Arc::new(Vec::new());
        };
        let port = |f: Id<Function>| functions.get(f).and_then(|f| f.bus_port.clone());
        let of_net = |f: Id<Function>| port(f).is_some_and(|p| p.physical_net == Some(*key));
        let mut graph = bus_graph(q);
        // the ports of the net are nodes even without a connection
        for (id, f) in functions.iter() {
            if f.bus_port.as_ref().is_some_and(|p| p.physical_net == Some(*key)) {
                graph.entry(Node::Function(id)).or_default();
            }
        }
        let hint: BTreeMap<Id<Function>, usize> = pnet
            .order_hint
            .iter()
            .enumerate()
            .rev()
            .map(|(i, f)| (*f, i))
            .collect();
        let rank = |n: &Node| match n {
            Node::Function(f) => (hint.get(f).copied().unwrap_or(usize::MAX), 0u8, f.as_uuid()),
            Node::Junction(j) => (usize::MAX, 1u8, j.as_uuid()),
        };
        let mut seen: BTreeSet<Node> = BTreeSet::new();
        let mut out: Vec<(Node, BusSegment)> = Vec::new();
        let starts: Vec<Node> = graph.keys().copied().collect();
        for first in starts {
            if seen.contains(&first) {
                continue;
            }
            // the component of `first`
            let mut component: BTreeSet<Node> = BTreeSet::new();
            let mut stack = vec![first];
            while let Some(n) = stack.pop() {
                if component.insert(n) {
                    stack.extend(graph.get(&n).into_iter().flat_map(|m| m.keys().copied()));
                }
            }
            seen.extend(component.iter().copied());
            let fns: Vec<Id<Function>> = component
                .iter()
                .filter_map(|n| match n {
                    Node::Function(f) => Some(*f),
                    Node::Junction(_) => None,
                })
                .collect();
            if !fns.iter().any(|f| of_net(*f)) {
                continue;
            }
            let degree = |n: &Node| graph.get(n).map_or(0, BTreeMap::len);
            let external_of = |f: Id<Function>| functions.get(f).is_some_and(|f| is_external_ip(f));
            let mut ends: Vec<Id<Function>> = fns
                .iter()
                .copied()
                .filter(|f| degree(&Node::Function(*f)) <= 1 || external_of(*f))
                .collect();
            ends.sort_by_key(|f| rank(&Node::Function(*f)));
            let logical_net = fns
                .iter()
                .filter_map(|f| port(*f).and_then(|p| p.logical_net))
                .min();
            let master = logical_net
                .and_then(|n| q.table::<LogicalNet>().get(n))
                .and_then(|n| match &n.master {
                    Some(Endpoint::Cp(f, _)) => Some(*f),
                    _ => None,
                });
            if let Some(m) = master.filter(|m| ends.contains(m)) {
                ends.retain(|e| *e != m);
                ends.insert(0, m);
            }
            let start = ends.first().map(|f| Node::Function(*f)).or_else(|| {
                component
                    .iter()
                    .copied()
                    .filter(|n| matches!(n, Node::Function(_)))
                    .min_by_key(|n| rank(n))
            });
            let Some(start) = start else { continue };
            // depth-first walk from the start, neighbours by rank; a cable at its first edge
            let mut members = Vec::new();
            let mut cables_seen: BTreeSet<Id<Device>> = BTreeSet::new();
            let mut visited: BTreeSet<Node> = BTreeSet::new();
            let mut stack: Vec<(Node, Option<Node>)> = vec![(start, None)];
            while let Some((n, from)) = stack.pop() {
                if !visited.insert(n) {
                    continue;
                }
                if let Some(from) = from {
                    for cable in graph.get(&from).and_then(|m| m.get(&n)).into_iter().flatten() {
                        if cables_seen.insert(*cable) {
                            members.push(SegmentMember::Cable(*cable));
                        }
                    }
                }
                if let Node::Function(f) = n {
                    members.push(SegmentMember::Function(f));
                }
                let mut next: Vec<Node> = graph
                    .get(&n)
                    .into_iter()
                    .flat_map(|m| m.keys().copied())
                    .filter(|m| !visited.contains(m))
                    .collect();
                next.sort_by_key(|m| rank(m));
                for m in next.into_iter().rev() {
                    stack.push((m, Some(n)));
                }
            }
            let external_ends = ends.iter().copied().filter(|f| external_of(*f)).collect();
            let branched = component.iter().any(|n| degree(n) > 2);
            out.push((
                start,
                BusSegment {
                    members,
                    ends,
                    external_ends,
                    logical_net,
                    branched,
                },
            ));
        }
        out.sort_by_key(|(start, _)| rank(start));
        Arc::new(out.into_iter().map(|(_, s)| s).collect())
    }
}

/// The master of a logical net (FR-BUS-005).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NetMaster {
    /// A bus port of this project.
    Port(Id<Function>),
    /// An external interruption point: the partner is outside the project, named by the IP's `ExternalRef` (doc 20
    /// §11.1: JE-N1000-03 in +J-E-206). It is no device here, so it is in no BOM.
    External {
        /// The interruption point.
        ip: Id<Function>,
        /// Its external reference.
        external: Id<ExternalRef>,
    },
    /// An endpoint that names no function of the project (a junction, a removed function, a newer form).
    Other,
}

/// The members of a logical net: its ports in ID order and its master.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct NetMembers {
    /// The functions whose bus port names the net, in ID order.
    pub ports: Vec<Id<Function>>,
    /// The master; `None` when the net names none.
    pub master: Option<NetMaster>,
}

/// `net_members(lnet)`: the ports of a logical net and its master (doc 10 §2.10).
#[derive(Debug)]
pub struct NetMembersOf;

impl Query for NetMembersOf {
    type Key = Id<LogicalNet>;
    type Value = NetMembers;
    const FAMILY: &'static str = "net_members";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<NetMembers> {
        let Some(net) = q.table::<LogicalNet>().get(*key) else {
            return Arc::new(NetMembers::default());
        };
        let functions = q.functions();
        let ports = functions
            .iter()
            .filter(|(_, f)| f.bus_port.as_ref().is_some_and(|p| p.logical_net == Some(*key)))
            .map(|(id, _)| id)
            .collect();
        let master = net.master.as_ref().map(|m| match m {
            Endpoint::Cp(f, _) => match functions.get(*f) {
                Some(func) => match &func.kind {
                    FunctionKind::Ip(ip) => {
                        ip.external
                            .map_or(NetMaster::Other, |external| NetMaster::External {
                                ip: *f,
                                external,
                            })
                    }
                    _ if func.bus_port.is_some() => NetMaster::Port(*f),
                    _ => NetMaster::Other,
                },
                None => NetMaster::Other,
            },
            _ => NetMaster::Other,
        });
        Arc::new(NetMembers { ports, master })
    }
}

/// The station address as a drawing prints it (doc 20 §11.1, FR-BUS-006): "Node TBD" for the open item, "Node 3"
/// for a PROFIBUS address; `None` for a port without an address and for an address form of a newer format.
pub fn bus_address_text(port: &BusPort) -> Option<String> {
    match &port.address {
        AddrValue::Tbd => Some("Node TBD".to_owned()),
        AddrValue::Value(NetAddress::Profibus(n)) => Some(format!("Node {n}")),
        AddrValue::Value(NetAddress::Unknown(_)) | AddrValue::NotApplicable | AddrValue::Unknown(_) => None,
    }
}

/// The termination as a drawing prints it (FR-BUS-004: a property arrangement over `termination`): `Switch to "ON"`
/// or `Switch to "OFF"`; `None` for a port without a termination switch or with a state of a newer format.
pub fn bus_termination_text(port: &BusPort) -> Option<String> {
    match port.termination.as_ref()? {
        OnOff::On => Some("Switch to \"ON\"".to_owned()),
        OnOff::Off => Some("Switch to \"OFF\"".to_owned()),
        OnOff::Unknown(_) => None,
    }
}

/// The members of a potential as folio 909 counts them (doc 20 §7.3): the connection points of devices, a terminal as
/// one member whatever its clamps and sides, and the external interruption points; the interruption points between
/// pages are not members. A potential that a bus segment carries counts the segment's members instead of their
/// CPs (D11: the DP potential reads segments): every function and cable of each segment one of whose functions has
/// a CP on the potential, so the cable between the external end and the connector is a member (Golden A's DP row:
/// the external end, -3W1, -3X1 and -201U1:DP).
pub fn potential_member_count(q: &Q, p: &Potential) -> usize {
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    enum Member {
        Cp(ew_ids::Uuid, u16),
        Record(ew_ids::Uuid),
    }
    let on: BTreeSet<Id<Function>> = p.cps.iter().map(|c| c.function).collect();
    let mut segment_functions: BTreeSet<Id<Function>> = BTreeSet::new();
    let mut members: BTreeSet<Member> = BTreeSet::new();
    let physical: Vec<Id<PhysicalNet>> = q.table::<PhysicalNet>().ids().collect();
    for pnet in physical {
        for segment in q.get::<BusSegments>(pnet).iter() {
            if !segment.functions().any(|f| on.contains(&f)) {
                continue;
            }
            for m in &segment.members {
                match m {
                    SegmentMember::Function(f) => {
                        segment_functions.insert(*f);
                        members.insert(Member::Record(f.as_uuid()));
                    }
                    SegmentMember::Cable(c) => {
                        members.insert(Member::Record(c.as_uuid()));
                    }
                }
            }
        }
    }
    for cp in &p.cps {
        if segment_functions.contains(&cp.function) {
            continue;
        }
        let Some(f) = q.functions().get(cp.function) else {
            continue;
        };
        match (&f.kind, f.device) {
            (FunctionKind::Ip(ip), _) => {
                if ip.external.is_some() {
                    members.insert(Member::Cp(cp.function.as_uuid(), cp.idx));
                }
            }
            (_, Some(d)) if q.devices().get(d).is_some_and(|d| d.aspects.terminal.is_some()) => {
                members.insert(Member::Record(d.as_uuid()));
            }
            _ => {
                members.insert(Member::Cp(cp.function.as_uuid(), cp.idx));
            }
        }
    }
    members.len()
}

/// The printed name of a partner outside the project, "+J-E-206 / JE-N1000-03" (FR-XRF-012): how the network list
/// and the bus checks name an external master or segment end (FR-BUS-005).
pub fn external_partner_text(q: &Q, external: Id<ExternalRef>) -> String {
    crate::xref::external_text(q, external)
}
