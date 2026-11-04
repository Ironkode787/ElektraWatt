// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser N-FL-4 `drives_link` (doc 06 §3.2 row 16, doc 11 §4.2, WP-S1.20 D15, FR-FLU-029): the first line from a
//! valve's working port to an actuator links the valve to the device it drives.
//!
//! - **Trigger.** A `fluid` connection new in the transaction.
//! - **The net.** The fluid net of that line in the transaction's state: the CPs reached through `fluid` connections and
//!   their junctions. A CP's port role is its FnDef CP's (the CP of the same index, the rule of `function_cps`).
//! - **Effect.** For each valve with a working port (`working2`, `working4`) of a function on a FnDef of contact kind
//!   `valve` in the net, and no `drives` link of its own: when the net holds a `consumer` port of exactly one device
//!   outside the valve and its parent chain, it adds `DeviceLink {from: valve, to: that device, kind: drives}` with the
//!   ID `v5(valve, "drives" ␟ device)`, with the notice `<valve> now drives <device>`; with two or more such devices it
//!   adds no link and says so. An existing link is the user's and stays.
//!
//! Idempotent: the valve has its link after the first run. Reads records only (the transaction-read rule, ID-525).

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_model::EndpointKey;
use ew_schema::{
    Connection, ConnectionKind, ContactKind, Device, DeviceLink, DeviceLinkKind, Extra, FluidPortRole, FnDef,
    Function, Kind, LibCopy,
};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;
use crate::words;

/// Normaliser N-FL-4.
#[derive(Clone, Copy, Debug, Default)]
pub struct DrivesLink;

/// The ID of the `drives` link a line adds: `v5(valve, "drives" ␟ device)` (WP-S1.20 D15).
fn link_id(valve: Id<Device>, device: Id<Device>) -> Id<DeviceLink> {
    let name = format!("drives\u{1F}{}", device.as_uuid().hyphenated());
    Id::from_uuid(ew_ids::Uuid::new_v5(&valve.as_uuid(), name.as_bytes()))
}

/// The CPs of the fluid net of `line` in the transaction: breadth first through `fluid` connections and junctions.
fn net_cps(tx: &TxBuilder<'_>, line: &Connection) -> BTreeSet<(Id<Function>, u16)> {
    let mut seen: BTreeSet<EndpointKey> = BTreeSet::new();
    let mut queue: Vec<EndpointKey> = [&line.a, &line.b]
        .into_iter()
        .filter_map(EndpointKey::of)
        .collect();
    while let Some(node) = queue.pop() {
        if !seen.insert(node) {
            continue;
        }
        for c in tx.connections_at(node) {
            let Some(c) = tx.get::<Connection>(c) else {
                continue;
            };
            if !matches!(c.kind, ConnectionKind::Fluid(_)) {
                continue;
            }
            queue.extend([&c.a, &c.b].into_iter().filter_map(EndpointKey::of));
        }
    }
    seen.into_iter()
        .filter_map(|k| match k {
            EndpointKey::Cp(f, idx) => Some((f, idx)),
            EndpointKey::Junction(_) => None,
        })
        .collect()
}

/// The port role of CP `idx` of a function and whether the function is a valve (its FnDef's contact kind).
fn role_of(tx: &TxBuilder<'_>, f: Id<Function>, idx: u16) -> Option<(Id<Device>, FluidPortRole, bool)> {
    let function = tx.get::<Function>(f)?;
    let device = function.device?;
    let def = tx.get::<LibCopy<FnDef>>(function.def)?;
    let role = def.item.cps.iter().find(|c| c.idx == idx)?.port_role.clone()?;
    Some((device, role, def.item.contact_kind == ContactKind::Valve))
}

/// The device and its parent chain in the transaction.
fn chain(tx: &TxBuilder<'_>, device: Id<Device>) -> Vec<Id<Device>> {
    let mut out = vec![device];
    let mut at = device;
    while let Some(p) = tx.get::<Device>(at).and_then(|d| d.parent) {
        if out.contains(&p) {
            break;
        }
        out.push(p);
        at = p;
    }
    out
}

/// Devices in words: "-A", "-A and -B", "-A, -B and -C".
fn listed(tx: &TxBuilder<'_>, devices: &[Id<Device>]) -> String {
    let names: Vec<String> = devices.iter().map(|d| words::device(tx, *d).to_string()).collect();
    match names.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        Some((last, _)) => last.clone(),
        None => String::new(),
    }
}

impl Normaliser for DrivesLink {
    fn id(&self) -> &'static str {
        "drives_link"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let lines: Vec<Connection> = tx
            .changed_of(Kind::Connection)
            .into_iter()
            .filter(|k| tx.is_new(k))
            .filter_map(|k| tx.get::<Connection>(k.1.cast()))
            .filter(|c| matches!(c.kind, ConnectionKind::Fluid(_)))
            .map(|c| (*c).clone())
            .collect();
        let mut changed = false;
        for line in lines {
            let mut valves: BTreeSet<Id<Device>> = BTreeSet::new();
            let mut consumers: BTreeSet<Id<Device>> = BTreeSet::new();
            for (f, idx) in net_cps(tx, &line) {
                match role_of(tx, f, idx) {
                    Some((d, FluidPortRole::Working2 | FluidPortRole::Working4, true)) => {
                        valves.insert(d);
                    }
                    Some((d, FluidPortRole::Consumer, _)) => {
                        consumers.insert(d);
                    }
                    _ => {}
                }
            }
            for valve in valves {
                let linked = tx
                    .iter::<DeviceLink>()
                    .into_iter()
                    .any(|l| l.from == valve && l.kind == DeviceLinkKind::Drives);
                if linked {
                    continue;
                }
                let held = chain(tx, valve);
                let driven: Vec<Id<Device>> =
                    consumers.iter().copied().filter(|d| !held.contains(d)).collect();
                match driven.as_slice() {
                    [] => {}
                    [device] => {
                        tx.insert(DeviceLink {
                            id: link_id(valve, *device),
                            v: 1,
                            from: valve,
                            to: *device,
                            kind: DeviceLinkKind::Drives,
                            source_ref: None,
                            extra: Extra::new(),
                        })?;
                        tx.notice(format!(
                            "{} now drives {}",
                            words::device(tx, valve),
                            words::device(tx, *device)
                        ));
                        changed = true;
                    }
                    several => tx.notice(format!(
                        "the working-port lines of {} reach {}; link the driven device by hand",
                        words::device(tx, valve),
                        listed(tx, several)
                    )),
                }
            }
        }
        Ok(changed)
    }
}
