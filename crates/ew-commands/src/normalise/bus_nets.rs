// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #14b, bus nets from drawn bus lines (N6; doc 10 §2.7 "nets from drawn lines"; doc 06 §3.2 after
//! row 14a).
//!
//! **Trigger.** A `Bus` connection that is new or changed in the transaction, both of whose ends are CPs of functions
//! with a `bus_port` of one bus system (a newer format's bus is left alone).
//!
//! **Effect.** The ports that bus lines join are one network, as a person who draws a bus between them means. The
//! line's segment is every port of its bus system reached over `Bus` lines from its ends, through any port on the
//! way. A passive port (a bus plug, role `passive`) passes the net through and never takes one: it has no address
//! (a plug names the physical net only). Among the segment's other ports: when exactly one logical net
//! is named (and exists), the ports that name none take it; when none is named and the segment has two such ports, a
//! new `LogicalNet` of their bus system is made, named for the bus system (`PROFIBUS DP`, then `PROFIBUS DP 2` …),
//! with the CP of the segment's one `master` or `controller` port as its master, and all take it. Ports in different
//! nets are left as they are (EW-BUS-010 reports them). The physical net goes the same way over every port of the
//! segment, a plug's too (a drawn line made the logical net only, so its segments, EW-BUS-004 and the bus
//! overview saw nothing): the one the ports name, else a new `PhysicalNet` of copper named as a new logical net is,
//! which every port without one takes. N6 never changes a net a port names, so it is idempotent: after a run the
//! segment's ports name their nets and a second run changes nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::Ident;
use ew_schema::{
    BusMedium, BusPort, BusSystem, Connection, ConnectionKind, Endpoint, Extra, Function, Kind, LogicalNet,
    PhysicalNet, PortRole,
};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// Normaliser N6.
#[derive(Clone, Copy, Debug, Default)]
pub struct BusNets;

/// A port of a segment: its function's port and the CP a bus line meets it at.
type Member = (BusPort, Endpoint);

/// The port at a line's end, with its function and the end's CP.
fn end_port(tx: &TxBuilder<'_>, end: &Endpoint) -> Option<(Id<Function>, Member)> {
    let f = end.function()?;
    let port = tx.get::<Function>(f)?.bus_port.clone()?;
    Some((f, (port, end.clone())))
}

/// The segment of `line`: the ports of `bus` reached over the bus lines `all` from its ends, each with the CP of the
/// first line (in ID order, `line` first) that meets it.
fn segment(
    tx: &TxBuilder<'_>,
    all: &[Arc<Connection>],
    line: &Connection,
    bus: &BusSystem,
) -> BTreeMap<Id<Function>, Member> {
    let mut members: BTreeMap<Id<Function>, Member> = BTreeMap::new();
    let mut queue: Vec<Id<Function>> = Vec::new();
    let meet =
        |members: &mut BTreeMap<Id<Function>, Member>, queue: &mut Vec<Id<Function>>, end: &Endpoint| {
            if let Some((f, member)) = end_port(tx, end)
                && member.0.bus == *bus
                && !members.contains_key(&f)
            {
                members.insert(f, member);
                queue.push(f);
            }
        };
    meet(&mut members, &mut queue, &line.a);
    meet(&mut members, &mut queue, &line.b);
    while let Some(f) = queue.pop() {
        for other in all {
            if other.a.function() == Some(f) {
                meet(&mut members, &mut queue, &other.b);
            }
            if other.b.function() == Some(f) {
                meet(&mut members, &mut queue, &other.a);
            }
        }
    }
    members
}

/// The net a port names, when it exists.
fn net_of(tx: &TxBuilder<'_>, port: &BusPort) -> Option<Id<LogicalNet>> {
    port.logical_net.filter(|n| tx.get::<LogicalNet>(*n).is_some())
}

/// A name for a new net of `bus`: the bus system's (`PROFIBUS DP`), then numbered, unique among `taken`.
fn net_name(bus: &BusSystem, taken: &[String]) -> Result<Ident, CmdError> {
    let base = ew_derive::bus_word(bus);
    let name = std::iter::once(base.to_owned())
        .chain((2..).map(|n| format!("{base} {n}")))
        .find(|n| !taken.contains(n))
        .unwrap_or_else(|| base.to_owned());
    Ident::from_input(&name).map_err(|e| CmdError::internal(format!("bus net name: {e}")))
}

/// A new physical net of `bus`, of copper, named as [`net_name`] names it among the physical nets.
fn new_physical(tx: &mut TxBuilder<'_>, bus: &BusSystem) -> Result<Id<PhysicalNet>, CmdError> {
    let taken: Vec<String> = tx
        .iter::<PhysicalNet>()
        .into_iter()
        .map(|n| n.name.as_str().to_owned())
        .collect();
    let name = net_name(bus, &taken)?;
    let id: Id<PhysicalNet> = tx.mint();
    tx.insert(PhysicalNet {
        id,
        v: 1,
        name,
        medium: BusMedium::Copper,
        order_hint: Vec::new(),
        source_ref: None,
        extra: Extra::new(),
    })?;
    Ok(id)
}

/// A new logical net of `bus`, named for the bus system and unique among the nets, with `master`.
fn new_net(
    tx: &mut TxBuilder<'_>,
    bus: &BusSystem,
    master: Option<Endpoint>,
) -> Result<Id<LogicalNet>, CmdError> {
    let taken: Vec<String> = tx
        .iter::<LogicalNet>()
        .into_iter()
        .map(|n| n.name.as_str().to_owned())
        .collect();
    let name = net_name(bus, &taken)?;
    let id: Id<LogicalNet> = tx.mint();
    tx.insert(LogicalNet {
        id,
        v: 1,
        bus: bus.clone(),
        name,
        master,
        rate: None,
        source_ref: None,
        extra: Extra::new(),
    })?;
    Ok(id)
}

impl Normaliser for BusNets {
    fn id(&self) -> &'static str {
        "bus_nets"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let bus_line = |c: &Arc<Connection>| matches!(c.kind, ConnectionKind::Bus(_));
        let mut lines: Vec<Arc<Connection>> = tx
            .changed_of(Kind::Connection)
            .into_iter()
            .filter_map(|k| tx.get::<Connection>(k.1.cast()))
            .filter(bus_line)
            .collect();
        if lines.is_empty() {
            return Ok(false);
        }
        lines.sort_by_key(|c| c.id);
        let mut all: Vec<Arc<Connection>> = tx.iter::<Connection>().into_iter().filter(bus_line).collect();
        all.sort_by_key(|c| c.id);
        let mut done: BTreeSet<Id<Function>> = BTreeSet::new();
        let mut changed = false;
        for line in lines {
            let (Some(a), Some(b)) = (end_port(tx, &line.a), end_port(tx, &line.b)) else {
                continue;
            };
            let bus = a.1.0.bus.clone();
            if bus != b.1.0.bus || matches!(bus, BusSystem::Unknown(_)) || done.contains(&a.0) {
                continue;
            }
            let members = segment(tx, &all, &line, &bus);
            done.extend(members.keys().copied());
            // The segment's physical net, over every port, a plug's too.
            let physical_of = |tx: &TxBuilder<'_>, port: &BusPort| {
                port.physical_net.filter(|n| tx.get::<PhysicalNet>(*n).is_some())
            };
            let physical: BTreeSet<Id<PhysicalNet>> =
                members.values().filter_map(|m| physical_of(tx, &m.0)).collect();
            let pnet = match (physical.len(), physical.first()) {
                (1, Some(n)) => Some(*n),
                (0, _) if members.len() >= 2 => Some(new_physical(tx, &bus)?),
                _ => None,
            };
            if let Some(pnet) = pnet {
                let join: Vec<Id<Function>> = members
                    .iter()
                    .filter(|(_, m)| physical_of(tx, &m.0).is_none())
                    .map(|(f, _)| *f)
                    .collect();
                for f in join {
                    tx.update::<Function>(f, |x| {
                        if let Some(p) = x.bus_port.as_mut() {
                            p.physical_net = Some(pnet);
                        }
                    })?;
                    changed = true;
                }
            }
            // A plug passes the net through and takes none.
            let nodes: Vec<(Id<Function>, &Member)> = members
                .iter()
                .filter(|(_, m)| m.0.role != PortRole::Passive)
                .map(|(f, m)| (*f, m))
                .collect();
            let named: BTreeSet<Id<LogicalNet>> =
                nodes.iter().filter_map(|(_, m)| net_of(tx, &m.0)).collect();
            let net = match (named.len(), named.first()) {
                (1, Some(n)) => *n,
                (0, _) if nodes.len() >= 2 => {
                    let mut leading = nodes
                        .iter()
                        .filter(|(_, m)| matches!(m.0.role, PortRole::Master | PortRole::Controller));
                    let master = match (leading.next(), leading.next()) {
                        (Some((_, m)), None) => Some(m.1.clone()),
                        _ => None,
                    };
                    new_net(tx, &bus, master)?
                }
                _ => continue,
            };
            let join: Vec<Id<Function>> = nodes
                .iter()
                .filter(|(_, m)| net_of(tx, &m.0).is_none())
                .map(|(f, _)| *f)
                .collect();
            for f in join {
                tx.update::<Function>(f, |x| {
                    if let Some(p) = x.bus_port.as_mut() {
                        p.logical_net = Some(net);
                    }
                })?;
                changed = true;
            }
        }
        Ok(changed)
    }
}
