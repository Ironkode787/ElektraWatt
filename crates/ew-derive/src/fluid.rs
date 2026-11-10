// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid side (doc 11 §5.1): `fluid_nets()`, the partition of the fluid CPs joined by `fluid`
//! connections and their junctions, and the tracked target of a valve (`reached_device`, `tracked_target`,
//! `tracked_process_tag`).
//!
//! A fluid net is a connected set of nodes (function CPs and junctions) joined by connections of kind `Fluid`, on
//! every page and across pages; galleries, fluid interruption points and `Short` links are 1.0's. Its ID is its
//! minimum member CP ID, as an electrical net's ([`NetId`]). A fluid line is no wire: it joins no electrical net
//! ([`crate::joins_net`] leaves it out). A CP that no fluid line reaches is in no fluid net.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_model::EndpointKey;
use ew_schema::{Connection, ConnectionKind, ContactKind, Device, DeviceKind, DeviceState, FluidPortRole};

use crate::nets::{Dsu, NetId, ends};
use crate::query::{Policy, Query};
use crate::{FunctionCps, Q, driven_device};

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

/// The position count of a module: `plc.positions.count` of the part named by `plc_module.part_use`,
/// else of the first part use whose copy has `plc.positions`; `None` for a device without (no module).
pub fn valve_positions(q: &Q, module: Id<Device>) -> Option<u16> {
    let dev = q.devices().get(module)?;
    let parts = q.parts();
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
