// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid side (doc 11 §5.1, §5.2): `galleries(vt)`, the derived manifold galleries of a valve terminal;
//! `fluid_nets()`, the partition of the fluid CPs; `fluid_tubes()`, the physical tubes; and the tracked target of a
//! valve (`reached_device`, `tracked_target`, `tracked_process_tag`).
//!
//! A fluid net is a connected set of nodes (function CPs and junctions) joined by connections of kind `Fluid`, on
//! every page and across pages, by the gallery edges of every valve terminal, by the pairs of fluid interruption
//! points (an interruption point whose CP is a fluid port, FR-FLU-030) and by the `short` internal links of the
//! FnDefs of the functions it reaches (fittings, distribution blocks, bulkheads). Its ID is its minimum member CP ID,
//! as an electrical net's ([`NetId`]). A fluid line is no wire: it joins no electrical net ([`crate::joins_net`]
//! leaves it out). A CP that neither a fluid line nor a gallery reaches is in no fluid net.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_model::EndpointKey;
use ew_schema::{
    Connection, ConnectionKind, ContactKind, Device, DeviceKind, DeviceState, Discipline, FluidPortRole,
    Function, FunctionKind, InternalKind, IpDirection, StationRole,
};

use crate::nets::{CpRef, Dsu, NetId, ends};
use crate::query::{Policy, Query};
use crate::{FunctionCps, IpPairingQ, Q, driven_device};

/// Whether a discipline is a fluid one: pneumatic, vacuum or hydraulic.
pub fn is_fluid(d: &Discipline) -> bool {
    matches!(
        d,
        Discipline::Pneumatic | Discipline::Vacuum | Discipline::Hydraulic
    )
}

/// Whether a device is a valve terminal: a PLC station of role `valve_terminal` (doc 11 §3.2, FR-DEV-024).
pub fn is_valve_terminal(q: &Q, device: Id<Device>) -> bool {
    q.devices().get(device).is_some_and(|d| {
        d.aspects
            .plc_station
            .as_ref()
            .is_some_and(|s| s.role == StationRole::ValveTerminal)
    })
}

/// The functions of a device on a FnDef of contact kind `valve`, in ID order.
pub fn valve_functions(q: &Q, device: Id<Device>) -> Vec<Id<Function>> {
    let functions = q.functions();
    let fndefs = q.fndefs();
    let mut out: Vec<Id<Function>> = q
        .idx()
        .functions_of_device(device)
        .filter(|f| {
            functions
                .get(*f)
                .and_then(|f| fndefs.get(f.def))
                .is_some_and(|d| d.item.contact_kind == ContactKind::Valve)
        })
        .collect();
    out.sort();
    out
}

/// The children of a device ordered by position, then ID (a child without a position last).
fn ordered_children(q: &Q, device: Id<Device>) -> Vec<Id<Device>> {
    let devices = q.devices();
    let mut out: Vec<(u32, Id<Device>)> = q
        .idx()
        .device_children(device)
        .map(|c| (devices.get(c).and_then(|d| d.position).unwrap_or(u32::MAX), c))
        .collect();
    out.sort();
    out.into_iter().map(|(_, c)| c).collect()
}

/// The galleries a manifold joins, in this order (doc 11 §5.2). The pilot gallery of an external pilot supply is not
/// derived: the `valve_terminal` aspect that would name it is not typed (WP-S1.25).
pub const GALLERIES: [FluidPortRole; 5] = [
    FluidPortRole::Supply1,
    FluidPortRole::Exhaust3,
    FluidPortRole::Exhaust5,
    FluidPortRole::PilotExhaust82,
    FluidPortRole::PilotExhaust84,
];

/// One segment of one gallery of a valve terminal: the CPs it joins, in manifold order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GallerySegment {
    /// The gallery, named by the role of the valve ports it joins.
    pub gallery: FluidPortRole,
    /// The joined CPs: the valves' ports of the gallery's role and the tapping ports of supply plates.
    pub members: Vec<EndpointKey>,
}

/// `galleries(vt)` (doc 11 §5.2, FR-FLU-026): the gallery segments of a valve terminal.
///
/// The slots are the terminal's children in position order, each followed by its own children in position order
/// (a module and its valves). Per gallery, a slot whose part's fluid section has an entry for the gallery with
/// `separates` closes the segment before it; a slot with a valve function joins that function's ports of the
/// gallery's role; a slot whose entry has a `tap` joins its port of that designation; every other slot (a module
/// without fluid data, the CPX block) is transparent. Empty segments are left out. Empty for a device that is no
/// valve terminal ([`is_valve_terminal`]). Policy Memo.
#[derive(Debug)]
pub struct Galleries;

impl Query for Galleries {
    type Key = Id<Device>;
    type Value = Vec<GallerySegment>;
    const FAMILY: &'static str = "galleries";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Id<Device>) -> Arc<Vec<GallerySegment>> {
        if !is_valve_terminal(q, *key) {
            return Arc::new(Vec::new());
        }
        let mut slots: Vec<Id<Device>> = Vec::new();
        for c in ordered_children(q, *key) {
            slots.push(c);
            slots.extend(ordered_children(q, c));
        }
        let devices = q.devices();
        let parts = q.parts();
        let mut out = Vec::new();
        for g in GALLERIES {
            let mut segment: Vec<EndpointKey> = Vec::new();
            let mut close = |segment: &mut Vec<EndpointKey>| {
                if !segment.is_empty() {
                    out.push(GallerySegment {
                        gallery: g.clone(),
                        members: std::mem::take(segment),
                    });
                }
                segment.clear();
            };
            for s in &slots {
                let entry = devices.get(*s).and_then(|d| {
                    d.parts.iter().find_map(|u| {
                        let fluid = parts.get(u.part.copy)?.item.sections.fluid.as_ref()?;
                        fluid.galleries.iter().find(|e| e.gallery == g).cloned()
                    })
                });
                if entry.as_ref().is_some_and(|e| e.separates) {
                    close(&mut segment);
                }
                let valves = valve_functions(q, *s);
                if !valves.is_empty() {
                    for f in valves {
                        for cp in q.get::<FunctionCps>(f).iter() {
                            if cp.port_role.as_ref() == Some(&g) {
                                segment.push(EndpointKey::Cp(f, cp.idx));
                            }
                        }
                    }
                } else if let Some(tap) = entry.as_ref().and_then(|e| e.tap.as_ref()) {
                    let mut functions: Vec<Id<Function>> = q.idx().functions_of_device(*s).collect();
                    functions.sort();
                    if let Some(at) = functions.iter().find_map(|f| {
                        q.get::<FunctionCps>(*f)
                            .iter()
                            .find(|c| c.designation == tap.as_str() && is_fluid(&c.discipline))
                            .map(|c| EndpointKey::Cp(*f, c.idx))
                    }) {
                        segment.push(at);
                    }
                }
            }
            close(&mut segment);
        }
        Arc::new(out)
    }
}

/// The interruption point of a fluid port: a function of kind `ip` whose CP is a fluid port.
fn fluid_ip_cp(q: &Q, f: Id<Function>) -> Option<CpRef> {
    let func = q.functions().get(f)?;
    if !matches!(func.kind, FunctionKind::Ip(_)) {
        return None;
    }
    let cps = q.get::<FunctionCps>(f);
    let cp = cps.first()?;
    is_fluid(&cp.discipline).then(|| CpRef::new(f, cp.idx))
}

/// One fluid net.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FluidNet {
    /// The net ID (minimum member CP ID).
    pub id: NetId,
    /// The members in node order.
    pub members: Vec<EndpointKey>,
    /// The fluid connections of the net, in ID order.
    pub connections: Vec<Id<Connection>>,
}

/// The value of [`FluidNets`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FluidPartition {
    /// The nets in ID order.
    pub nets: Vec<FluidNet>,
    net_of: BTreeMap<EndpointKey, usize>,
}

impl FluidPartition {
    /// The fluid net of a node; `None` when no fluid line reaches it.
    pub fn net_of(&self, at: EndpointKey) -> Option<&FluidNet> {
        self.net_of.get(&at).map(|&i| &self.nets[i])
    }
}

/// `fluid_nets()`: the fluid partition of the project (doc 11 §5.1). Policy Memo.
#[derive(Debug)]
pub struct FluidNets;

impl Query for FluidNets {
    type Key = ();
    type Value = FluidPartition;
    const FAMILY: &'static str = "fluid_nets";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _: &()) -> Arc<FluidPartition> {
        let mut dsu = Dsu::default();
        let mut via: BTreeMap<EndpointKey, BTreeSet<Id<Connection>>> = BTreeMap::new();
        for (id, c) in q.connections().iter() {
            if !matches!(c.kind, ConnectionKind::Fluid(_)) {
                continue;
            }
            let Some((a, b)) = ends(c) else { continue };
            let (ia, ib) = (dsu.node(a), dsu.node(b));
            dsu.union(ia, ib);
            via.entry(a).or_default().insert(id);
            via.entry(b).or_default().insert(id);
        }
        // The gallery edges of every valve terminal: one star per segment.
        let mut terminals: Vec<Id<Device>> = q.devices().ids().filter(|d| is_valve_terminal(q, *d)).collect();
        terminals.sort();
        for vt in terminals {
            for segment in q.get::<Galleries>(vt).iter() {
                let Some((first, rest)) = segment.members.split_first() else {
                    continue;
                };
                let root = dsu.node(*first);
                for m in rest {
                    let i = dsu.node(*m);
                    dsu.union(root, i);
                }
            }
        }
        // The pairs of fluid interruption points that a line reaches.
        let reached: BTreeSet<Id<Function>> = via
            .keys()
            .filter_map(|k| match k {
                EndpointKey::Cp(f, _) => Some(*f),
                EndpointKey::Junction(_) => None,
            })
            .collect();
        let pairing = q.get::<IpPairingQ>(());
        for f in &reached {
            let Some(cp) = fluid_ip_cp(q, *f) else { continue };
            for p in pairing.partners(*f) {
                if let Some(other) = fluid_ip_cp(q, *p) {
                    let (a, b) = (
                        dsu.node(EndpointKey::Cp(cp.function, cp.idx)),
                        dsu.node(EndpointKey::Cp(other.function, other.idx)),
                    );
                    dsu.union(a, b);
                }
            }
        }
        // The `short` internal links of the FnDefs of the functions a line reaches.
        let functions = q.functions();
        let fndefs = q.fndefs();
        for f in &reached {
            let Some(def) = functions.get(*f).and_then(|x| fndefs.get(x.def)) else {
                continue;
            };
            let cps = q.get::<FunctionCps>(*f);
            let fluid_cp = |i: u16| cps.iter().any(|c| c.idx == i && is_fluid(&c.discipline));
            for link in &def.item.internal {
                if link.kind == InternalKind::Short && fluid_cp(link.a) && fluid_cp(link.b) {
                    let (a, b) = (
                        dsu.node(EndpointKey::Cp(*f, link.a)),
                        dsu.node(EndpointKey::Cp(*f, link.b)),
                    );
                    dsu.union(a, b);
                }
            }
        }
        let mut nets: Vec<FluidNet> = dsu
            .groups()
            .into_iter()
            .filter_map(|members| {
                let id = NetId::of(members.iter().copied())?;
                let connections: BTreeSet<Id<Connection>> = members
                    .iter()
                    .flat_map(|m| via.get(m).into_iter().flatten().copied())
                    .collect();
                Some(FluidNet {
                    id,
                    members,
                    connections: connections.into_iter().collect(),
                })
            })
            .collect();
        nets.sort_by(|x, y| x.id.cmp(&y.id).then_with(|| x.members[0].cmp(&y.members[0])));
        let mut net_of = BTreeMap::new();
        for (i, n) in nets.iter().enumerate() {
            for m in &n.members {
                net_of.insert(*m, i);
            }
        }
        Arc::new(FluidPartition { nets, net_of })
    }
}

/// The device and its parent chain (doc 11 §3.3: a valve never drives the terminal that holds it); a cycle
/// (EW-INV-009) ends the walk.
pub fn parent_chain(q: &Q, device: Id<Device>) -> Vec<Id<Device>> {
    let devices = q.devices();
    let mut out = vec![device];
    let mut at = device;
    while let Some(p) = devices.get(at).and_then(|d| d.parent) {
        if out.contains(&p) {
            break;
        }
        out.push(p);
        at = p;
    }
    out
}

/// `reached_device(valve)` (doc 11 §3.3): the single device, other than the valve and its parent chain,
/// with a CP in a fluid net of one of the valve's working ports (`working2`, `working4`); `None` when there is no
/// such device or more than one. Cold.
pub fn reached_device(q: &Q, valve: Id<Device>) -> Option<Id<Device>> {
    let chain = parent_chain(q, valve);
    let functions = q.functions();
    let nets = q.get::<FluidNets>(());
    let mut reached: Vec<Id<Device>> = Vec::new();
    for f in q.idx().functions_of_device(valve) {
        for cp in q.get::<FunctionCps>(f).iter() {
            if !matches!(
                cp.port_role,
                Some(FluidPortRole::Working2 | FluidPortRole::Working4)
            ) {
                continue;
            }
            let Some(net) = nets.net_of(EndpointKey::Cp(f, cp.idx)) else {
                continue;
            };
            for m in &net.members {
                let EndpointKey::Cp(g, _) = m else { continue };
                let Some(d) = functions.get(*g).and_then(|g| g.device) else {
                    continue;
                };
                if !chain.contains(&d) && !reached.contains(&d) {
                    reached.push(d);
                }
            }
        }
    }
    match reached.as_slice() {
        [one] => Some(*one),
        _ => None,
    }
}

/// `tracked_target(valve)` (doc 11 §3.3): the device the valve's `drives` link names
/// ([`driven_device`]), else its [`reached_device`], else none.
pub fn tracked_target(q: &Q, valve: Id<Device>) -> Option<Id<Device>> {
    driven_device(q, valve).or_else(|| reached_device(q, valve))
}

/// What one valve position of a module holds (doc 11 §5.6).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ValveSlot {
    /// A child with a function on a FnDef of contact kind `valve`, in its state.
    Valve {
        /// The valve.
        device: Id<Device>,
        /// Its state.
        state: DeviceState,
    },
    /// A child of kind `blanking_plate`.
    Blank {
        /// The plate.
        device: Id<Device>,
    },
    /// Nothing.
    Empty,
}

/// The value of [`ValveSlots`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ValveSlotsV {
    /// The position count n (`plc.positions.count`).
    pub count: u16,
    /// Position p is `slots[p - 1]`, for p in 1 … n.
    pub slots: Vec<ValveSlot>,
    /// The children at a position that another child (of lower ID) holds: (position, child), in that order.
    pub second: Vec<(u32, Id<Device>)>,
    /// The children at a position outside 1 … n: (position, child), in that order.
    pub outside: Vec<(u32, Id<Device>)>,
}

/// The position count of a module: the `fluid.positions` of the first part use whose copy has it, else
/// `plc.positions.count` of the part named by `plc_module.part_use`, else of the first part use whose copy has
/// `plc.positions`; `None` for a device without (no module).
pub fn valve_positions(q: &Q, module: Id<Device>) -> Option<u16> {
    let dev = q.devices().get(module)?;
    let parts = q.parts();
    if let Some(n) = dev
        .parts
        .iter()
        .find_map(|u| parts.get(u.part.copy)?.item.sections.fluid.as_ref()?.positions)
    {
        return Some(n);
    }
    let count = |i: usize| {
        let copy = parts.get(dev.parts.get(i)?.part.copy)?;
        Some(copy.item.sections.plc.as_ref()?.positions.as_ref()?.count)
    };
    match dev.aspects.plc_module.as_ref().and_then(|m| m.part_use) {
        Some(i) => count(usize::from(i)),
        None => (0..dev.parts.len()).find_map(count),
    }
}

/// `valve_slots(module)` (doc 11 §5.6): the occupancy of each position 1 … n of a module, n its
/// [`valve_positions`]: a [`ValveSlot::Valve`] (of two valve or plate children at a position, the lower ID holds it), a
/// [`ValveSlot::Blank`] or [`ValveSlot::Empty`]; a child that is neither a valve nor a plate leaves its position empty.
/// The second children at a position and the children outside 1 … n are listed apart (EW-FL-013). Empty for a device
/// that is no module. Policy Memo.
#[derive(Debug)]
pub struct ValveSlots;

impl Query for ValveSlots {
    type Key = Id<Device>;
    type Value = ValveSlotsV;
    const FAMILY: &'static str = "valve_slots";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Id<Device>) -> Arc<ValveSlotsV> {
        let Some(count) = valve_positions(q, *key) else {
            return Arc::new(ValveSlotsV::default());
        };
        let devices = q.devices();
        let functions = q.functions();
        let fndefs = q.fndefs();
        let is_valve = |d: Id<Device>| {
            q.idx().functions_of_device(d).any(|f| {
                functions
                    .get(f)
                    .and_then(|f| fndefs.get(f.def))
                    .is_some_and(|def| def.item.contact_kind == ContactKind::Valve)
            })
        };
        let mut children: Vec<Id<Device>> = q.idx().device_children(*key).collect();
        children.sort();
        let mut out = ValveSlotsV {
            count,
            slots: vec![ValveSlot::Empty; usize::from(count)],
            second: Vec::new(),
            outside: Vec::new(),
        };
        let mut held = vec![false; usize::from(count)];
        for c in children {
            let Some(d) = devices.get(c) else { continue };
            let Some(p) = d.position else { continue };
            let Some(i) = usize::try_from(p)
                .ok()
                .filter(|i| (1..=usize::from(count)).contains(i))
            else {
                out.outside.push((p, c));
                continue;
            };
            let slot = if d.kind == Some(DeviceKind::BlankingPlate) {
                ValveSlot::Blank { device: c }
            } else if is_valve(c) {
                ValveSlot::Valve {
                    device: c,
                    state: d.state.clone(),
                }
            } else {
                continue;
            };
            if held[i - 1] {
                out.second.push((p, c));
            } else {
                held[i - 1] = true;
                out.slots[i - 1] = slot;
            }
        }
        out.second.sort();
        out.outside.sort();
        Arc::new(out)
    }
}

/// `tracked_process_tag(valve)` (doc 11 §3.3): the process tag of the [`tracked_target`], else the
/// valve's own; `None` when neither has one.
pub fn tracked_process_tag(q: &Q, valve: Id<Device>) -> Option<String> {
    let devices = q.devices();
    let tag = |d: Id<Device>| {
        devices
            .get(d)
            .and_then(|d| d.process_tag.as_ref())
            .map(|t| t.as_str().to_owned())
    };
    tracked_target(q, valve).and_then(tag).or_else(|| tag(valve))
}

/// One physical tube (doc 11 §5.1, FR-FLU-031): a fluid connection, or the connections joined through pairs of fluid
/// interruption points; every drawn fluid junction is a tee, so each of its branches is its own tube.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FluidTube {
    /// The tube ID: the connection at a target-side interruption point, else the lowest connection ID.
    pub id: Id<Connection>,
    /// The connections it is made of, in ID order.
    pub constituents: Vec<Id<Connection>>,
    /// Its two ends: CPs or junctions (tees), never a paired interruption point.
    pub ends: [EndpointKey; 2],
}

/// `fluid_tubes()` (doc 11 §5.1): the physical tubes of the project, in ID order. A fluid interruption point joins
/// the connection at it with the connection at its one partner when each has exactly one fluid connection; any other
/// interruption point is a tube end. Policy Memo.
#[derive(Debug)]
pub struct FluidTubes;

impl Query for FluidTubes {
    type Key = ();
    type Value = Vec<FluidTube>;
    const FAMILY: &'static str = "fluid_tubes";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _: &()) -> Arc<Vec<FluidTube>> {
        let mut lines: BTreeMap<Id<Connection>, (EndpointKey, EndpointKey)> = BTreeMap::new();
        let mut at: BTreeMap<EndpointKey, Vec<Id<Connection>>> = BTreeMap::new();
        for (id, c) in q.connections().iter() {
            if !matches!(c.kind, ConnectionKind::Fluid(_)) {
                continue;
            }
            let Some((a, b)) = ends(c) else { continue };
            lines.insert(id, (a, b));
            at.entry(a).or_default().push(id);
            at.entry(b).or_default().push(id);
        }
        let pairing = q.get::<IpPairingQ>(());
        // The single line at a fluid interruption point, when it has exactly one.
        let single = |f: Id<Function>| -> Option<(EndpointKey, Id<Connection>)> {
            let cp = fluid_ip_cp(q, f)?;
            let key = EndpointKey::Cp(cp.function, cp.idx);
            match at.get(&key).map(Vec::as_slice) {
                Some([one]) => Some((key, *one)),
                _ => None,
            }
        };
        let mut dsu: Dsu<Id<Connection>> = Dsu::default();
        let mut joined: BTreeSet<EndpointKey> = BTreeSet::new();
        let mut target_side: BTreeSet<Id<Connection>> = BTreeSet::new();
        for id in lines.keys() {
            dsu.node(*id);
        }
        for f in pairing.ips.keys() {
            let Some((key, line)) = single(*f) else { continue };
            let partners = pairing.partners(*f);
            let [p] = partners else { continue };
            let Some((pkey, pline)) = single(*p) else { continue };
            let (a, b) = (dsu.node(line), dsu.node(pline));
            dsu.union(a, b);
            joined.insert(key);
            joined.insert(pkey);
            let target = q.functions().get(*f).is_some_and(
                |x| matches!(&x.kind, FunctionKind::Ip(ip) if ip.direction == IpDirection::Target),
            );
            if target {
                target_side.insert(line);
            }
        }
        let mut out: Vec<FluidTube> = dsu
            .groups()
            .into_iter()
            .filter_map(|mut constituents| {
                constituents.sort();
                let mut free: Vec<EndpointKey> = constituents
                    .iter()
                    .filter_map(|c| lines.get(c))
                    .flat_map(|(a, b)| [*a, *b])
                    .filter(|e| !joined.contains(e))
                    .collect();
                if free.len() != 2 {
                    // A cycle through interruption points: the first line's ends.
                    let (a, b) = lines.get(constituents.first()?)?;
                    free = vec![*a, *b];
                }
                let id = constituents
                    .iter()
                    .find(|c| target_side.contains(c))
                    .copied()
                    .unwrap_or(constituents[0]);
                Some(FluidTube {
                    id,
                    constituents,
                    ends: [free[0], free[1]],
                })
            })
            .collect();
        out.sort_by_key(|t| t.id);
        Arc::new(out)
    }
}
