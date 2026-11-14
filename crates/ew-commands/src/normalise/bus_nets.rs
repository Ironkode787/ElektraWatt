// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #14b, bus nets from drawn bus lines (N6; doc 10 §2.7 "As built (persona round)"; doc 06 §3.2 after
//! row 14a).
//!
//! **Trigger.** A `Bus` connection that is new or changed in the transaction, both of whose ends are CPs of functions
//! with a `bus_port` of one bus system (a newer format's bus is left alone).
//!
//! **Effect.** The two ports are one network, as a person who draws a bus line between them means: when one port
//! names a logical net that exists and the other names none, the other takes it; when neither names one, a new
//! `LogicalNet` of their bus system is made, named for the bus system (`PROFIBUS DP`, then `PROFIBUS DP 2` …), with
//! the line's CP of a `master` or `controller` port as its master when exactly one end has such a role, and both
//! ports take it. Two ports in different nets are left as they are (EW-BUS-010 reports them). N6 never changes a
//! net a port names, so it is idempotent: after a run both ends name a net and a second run changes nothing.

use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::Ident;
use ew_schema::{
    BusPort, BusSystem, Connection, ConnectionKind, Endpoint, Extra, Function, Kind, LogicalNet, PortRole,
};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// Normaliser N6.
#[derive(Clone, Copy, Debug, Default)]
pub struct BusNets;

/// The port at a line's end, with its function and the end's CP.
fn end_port(tx: &TxBuilder<'_>, end: &Endpoint) -> Option<(Id<Function>, BusPort, Endpoint)> {
    let f = end.function()?;
    let port = tx.get::<Function>(f)?.bus_port.clone()?;
    Some((f, port, end.clone()))
}

/// The net a port names, when it exists.
fn net_of(tx: &TxBuilder<'_>, port: &BusPort) -> Option<Id<LogicalNet>> {
    port.logical_net.filter(|n| tx.get::<LogicalNet>(*n).is_some())
}

/// A new logical net of `bus`, named for the bus system and unique among the nets, with `master`.
fn new_net(tx: &mut TxBuilder<'_>, bus: &BusSystem, master: Option<Endpoint>) -> Result<Id<LogicalNet>, CmdError> {
    let base = ew_derive::bus_word(bus);
    let taken: Vec<String> = tx
        .iter::<LogicalNet>()
        .into_iter()
        .map(|n| n.name.as_str().to_owned())
        .collect();
    let name = std::iter::once(base.to_owned())
        .chain((2..).map(|n| format!("{base} {n}")))
        .find(|n| !taken.contains(n))
        .unwrap_or_else(|| base.to_owned());
    let name = Ident::from_input(&name).map_err(|e| CmdError::internal(format!("bus net name: {e}")))?;
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
        let mut lines: Vec<Arc<Connection>> = tx
            .changed_of(Kind::Connection)
            .into_iter()
            .filter_map(|k| tx.get::<Connection>(k.1.cast()))
            .filter(|c| matches!(c.kind, ConnectionKind::Bus(_)))
            .collect();
        lines.sort_by_key(|c| c.id);
        let mut changed = false;
        for line in lines {
            let (Some(a), Some(b)) = (end_port(tx, &line.a), end_port(tx, &line.b)) else {
                continue;
            };
            if a.1.bus != b.1.bus || matches!(a.1.bus, BusSystem::Unknown(_)) {
                continue;
            }
            let net = match (net_of(tx, &a.1), net_of(tx, &b.1)) {
                (Some(_), Some(_)) => continue,
                (Some(n), None) | (None, Some(n)) => n,
                (None, None) => {
                    let leads = |p: &BusPort| matches!(p.role, PortRole::Master | PortRole::Controller);
                    let master = match (leads(&a.1), leads(&b.1)) {
                        (true, false) => Some(a.2.clone()),
                        (false, true) => Some(b.2.clone()),
                        _ => None,
                    };
                    new_net(tx, &a.1.bus, master)?
                }
            };
            for (f, port, _) in [&a, &b] {
                if net_of(tx, port).is_none() {
                    tx.update::<Function>(*f, |x| {
                        if let Some(p) = x.bus_port.as_mut() {
                            p.logical_net = Some(net);
                        }
                    })?;
                    changed = true;
                }
            }
        }
        Ok(changed)
    }
}
