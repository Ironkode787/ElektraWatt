// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #14a, bus ports from the part (N5; WP-S1.21 §8a.2, D16; doc 06 §3.2 between rows 14 and 15).
//!
//! **Terms.** The *bus part* of a device is its first part use (in `Device.parts` order) whose part copy has a
//! `bus` section with at least one port. A *bus-port function* is a function whose FnDef copy has contact kind
//! `bus_port`. A part port is *carried* when a function of the device has a `bus_port` with the same `interface`
//! and `port_label` (compared exactly).
//!
//! **Trigger.** (i) A device of the transaction that is new, or whose bus part names another part copy than in the
//! base snapshot: placement and `box.black` from a part, `device.create`, `part.assign` in every mode, `fields.set`
//! on `parts`, a paste or a macro. (ii) A bus-port function that is new in the transaction or whose `device` changed
//! in it (`device.place` with `join_device`, `rep.join_device`), on a device with a bus part.
//!
//! **Effect.** The part's ports in list order, the first of each (`interface`, `port_label`) pair; a port whose `bus`
//! or `role` is of a newer format is left out (under trigger (i) with notice N5-c). The ports that no function of
//! the device carries are paired, in list order, with the device's port-less bus-port functions in ID order (under
//! trigger (ii) alone only the new or moved functions are candidates, so a port a drafter removed from an older
//! function stays removed); each pair gives the function `{interface, port_label, bus, role, termination}` from the
//! part, the address `not_applicable` for a passive port and `tbd` otherwise, and no nets. Surplus ports and
//! surplus functions are left as they are, without a notice. Under trigger (i) a carried port whose `bus` or `role`
//! differs from the part's keeps its value, with notice N5-b.
//!
//! N5 never changes or removes an existing `bus_port`. Idempotent: after a run every paired function carries its
//! port, so a second run pairs nothing, and its notices repeat texts that [`TxBuilder::notice`] keeps once.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::Ident;
use ew_schema::{
    AddrValue, BusPort, BusSystem, ContactKind, Device, Extra, FnDef, Function, Kind, LibCopy, Part,
    PartBusPort, PartUse, PortRole,
};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// Normaliser N5.
#[derive(Clone, Copy, Debug, Default)]
pub struct BusPorts;

/// The bus part of `parts`: the first part use whose part copy has a bus section with at least one port.
fn bus_part(tx: &TxBuilder<'_>, parts: &[PartUse]) -> Option<Arc<LibCopy<Part>>> {
    parts.iter().find_map(|u| {
        tx.get::<LibCopy<Part>>(u.part.copy)
            .filter(|c| c.item.sections.bus.as_ref().is_some_and(|b| !b.ports.is_empty()))
    })
}

/// Whether the function's FnDef copy has contact kind `bus_port`.
fn is_bus_port_function(tx: &TxBuilder<'_>, f: &Function) -> bool {
    tx.get::<LibCopy<FnDef>>(f.def)
        .is_some_and(|d| d.item.contact_kind == ContactKind::BusPort)
}

/// A port's key: its interface and port label.
type PortKey = (Ident, Option<Ident>);

fn key_of(interface: &Ident, port_label: &Option<Ident>) -> PortKey {
    (interface.clone(), port_label.clone())
}

/// A stored enum value as the file writes it (`profinet`, `controller`).
fn stored_name<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

/// The function's port made from a part port (D16).
fn seeded(port: &PartBusPort) -> BusPort {
    BusPort {
        interface: port.interface.clone(),
        port_label: port.port_label.clone(),
        bus: port.bus.clone(),
        role: port.role.clone(),
        address: if port.role == PortRole::Passive {
            AddrValue::NotApplicable
        } else {
            AddrValue::Tbd
        },
        termination: port.termination.clone(),
        logical_net: None,
        physical_net: None,
        extra: Extra::new(),
    }
}

/// The devices of trigger (i): new, or their bus part names another copy than in the base snapshot.
fn devices_with_a_new_bus_part(tx: &TxBuilder<'_>) -> BTreeSet<Id<Device>> {
    let mut out = BTreeSet::new();
    for key in tx.changed_of(Kind::Device) {
        let Some(dev) = tx.get::<Device>(key.1.cast()) else {
            continue;
        };
        let Some(now) = bus_part(tx, &dev.parts) else {
            continue;
        };
        let before = tx
            .base_row(&key)
            .and_then(|r| r.get::<Device>().cloned())
            .and_then(|b| bus_part(tx, &b.parts))
            .map(|c| c.id);
        if before != Some(now.id) {
            out.insert(dev.id);
        }
    }
    out
}

/// The bus-port functions of trigger (ii), by their device: new in the transaction, or moved to another device.
fn new_or_moved_functions(tx: &TxBuilder<'_>) -> BTreeMap<Id<Device>, BTreeSet<Id<Function>>> {
    let mut out: BTreeMap<Id<Device>, BTreeSet<Id<Function>>> = BTreeMap::new();
    for key in tx.changed_of(Kind::Function) {
        let Some(f) = tx.get::<Function>(key.1.cast()) else {
            continue;
        };
        let Some(device) = f.device else {
            continue;
        };
        let before = tx
            .base_row(&key)
            .and_then(|r| r.get::<Function>().map(|b| b.device));
        let fresh_here = match before {
            None => true,
            Some(old) => old != Some(device),
        };
        if fresh_here && f.bus_port.is_none() && is_bus_port_function(tx, &f) {
            out.entry(device).or_default().insert(f.id);
        }
    }
    out
}

/// A port as a notice names it: the device's tag, a colon and the interface, then a space and the label (`-A1:DP`).
fn port_text(tx: &TxBuilder<'_>, device: Id<Device>, key: &PortKey) -> String {
    let tag = crate::words::device(tx, device);
    match &key.1 {
        Some(label) => format!("{tag}:{} {}", key.0.as_str(), label.as_str()),
        None => format!("{tag}:{}", key.0.as_str()),
    }
}

/// N5 for one device; returns whether it gave a function a port.
fn run_device(
    tx: &mut TxBuilder<'_>,
    device: Id<Device>,
    full: bool,
    moved: Option<&BTreeSet<Id<Function>>>,
) -> Result<bool, CmdError> {
    let Some(dev) = tx.get::<Device>(device) else {
        return Ok(false);
    };
    let Some(part) = bus_part(tx, &dev.parts) else {
        return Ok(false);
    };
    let number = part.item.part_number.as_str().to_owned();
    let Some(section) = part.item.sections.bus.as_ref() else {
        return Ok(false);
    };
    // The first port of each pair; a pair whose first port is of a newer format is left out.
    let mut seen: BTreeSet<PortKey> = BTreeSet::new();
    let mut ports: Vec<(PortKey, &PartBusPort)> = Vec::new();
    for port in &section.ports {
        let key = key_of(&port.interface, &port.port_label);
        if !seen.insert(key.clone()) {
            continue;
        }
        if matches!(port.bus, BusSystem::Unknown(_)) || matches!(port.role, PortRole::Unknown(_)) {
            if full {
                tx.notice(format!(
                    "bus port {} of part {number} is of a newer format and was not applied",
                    port.interface.as_str()
                ));
            }
            continue;
        }
        ports.push((key, port));
    }
    let mut functions: Vec<Arc<Function>> = tx
        .functions_of_device(device)
        .into_iter()
        .filter_map(|f| tx.get::<Function>(f))
        .collect();
    functions.sort_by_key(|f| f.id);
    // The carried ports: the first function by ID that carries each key.
    let mut carried: BTreeMap<PortKey, BusPort> = BTreeMap::new();
    for f in &functions {
        if let Some(p) = &f.bus_port {
            carried
                .entry(key_of(&p.interface, &p.port_label))
                .or_insert_with(|| p.clone());
        }
    }
    let candidates: Vec<Id<Function>> = functions
        .iter()
        .filter(|f| f.bus_port.is_none())
        .filter(|f| full || moved.is_some_and(|m| m.contains(&f.id)))
        .filter(|f| is_bus_port_function(tx, f))
        .map(|f| f.id)
        .collect();
    let uncarried: Vec<&PartBusPort> = ports
        .iter()
        .filter(|(k, _)| !carried.contains_key(k))
        .map(|(_, p)| *p)
        .collect();
    let mut changed = false;
    for (port, function) in uncarried.into_iter().zip(candidates) {
        let value = seeded(port);
        tx.update::<Function>(function, |f| f.bus_port = Some(value))?;
        changed = true;
    }
    if full {
        for (key, port) in &ports {
            let Some(have) = carried.get(key) else {
                continue;
            };
            if have.bus != port.bus || have.role != port.role {
                let text = format!(
                    "{} keeps its bus port; part {number} declares {} {}",
                    port_text(tx, device, key),
                    stored_name(&port.bus),
                    stored_name(&port.role)
                );
                tx.notice(text);
            }
        }
    }
    Ok(changed)
}

impl Normaliser for BusPorts {
    fn id(&self) -> &'static str {
        "bus_ports"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let full = devices_with_a_new_bus_part(tx);
        let moved = new_or_moved_functions(tx);
        let devices: BTreeSet<Id<Device>> = full.iter().copied().chain(moved.keys().copied()).collect();
        let mut changed = false;
        for device in devices {
            changed |= run_device(tx, device, full.contains(&device), moved.get(&device))?;
        }
        Ok(changed)
    }
}
