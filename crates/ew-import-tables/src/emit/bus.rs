// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bus tables (WP-S1.21 D11; doc 10 §2.7): `logical_nets.csv` → `logical_net`, `physical_nets.csv` →
//! `physical_net`, and `bus_ports.csv` → the `bus_port` aspect of the function its key names. A project without
//! these tables has no bus records, and its functions are written as before.
//!
//! - `logical_nets.csv`: `key`, `bus`, `name`; optional `master` (a function key: a port, or an external
//!   interruption point whose external reference names a partner outside the project; its CP 0) and `rate` (bit/s).
//! - `physical_nets.csv`: `key`, `name`, `medium`; optional `order_hint` (`;`-separated function keys).
//! - `bus_ports.csv`: `key` (the function key), `interface`, `bus`, `role`, `address` (`tbd`, `not_applicable`, or a
//!   PROFIBUS station address 0–255 on a `profibus_dp` port); optional `port_label`, `termination` (`on`, `off`),
//!   `logical_net` and `physical_net` (net keys).

use std::collections::BTreeMap;

use ew_ids::Id;
use ew_schema::{
    AddrValue, BusMedium, BusPort, BusRate, BusSystem, Endpoint, Function, LogicalNet, NetAddress, OnOff,
    PhysicalNet, PortRole, Record,
};

use super::{Cx, fresh};
use crate::ids::{record_id, source_ref};
use crate::table::Table;

/// The names of [`BusSystem`].
const BUS_SYSTEMS: &[(&str, BusSystem)] = &[
    ("profibus_dp", BusSystem::ProfibusDp),
    ("profinet", BusSystem::Profinet),
    ("ethercat", BusSystem::EtherCat),
    ("ethernet_ip", BusSystem::EthernetIp),
    ("io_link", BusSystem::IoLink),
    ("asi", BusSystem::Asi),
    ("canopen", BusSystem::CanOpen),
    ("modbus_tcp", BusSystem::ModbusTcp),
];

/// The names of [`PortRole`].
const ROLES: &[(&str, PortRole)] = &[
    ("master", PortRole::Master),
    ("slave", PortRole::Slave),
    ("controller", PortRole::Controller),
    ("device", PortRole::Device),
    ("passive", PortRole::Passive),
];

/// The names of [`BusMedium`].
const MEDIA: &[(&str, BusMedium)] = &[("copper", BusMedium::Copper), ("fibre", BusMedium::Fibre)];

/// The names of [`OnOff`].
const ON_OFF: &[(&str, OnOff)] = &[("on", OnOff::On), ("off", OnOff::Off)];

/// `logical_nets.csv` → `logical_net`; returns the IDs by key. A master that names no function drops the row.
pub(crate) fn emit_logical_nets(cx: &mut Cx<'_>, table: Option<&Table>) -> BTreeMap<String, Id<LogicalNet>> {
    let mut out = BTreeMap::new();
    let Some(table) = table else { return out };
    let mut seen = BTreeMap::<String, ()>::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let bus = r.req_choice("bus", BUS_SYSTEMS);
        let name = r.req_ident("name");
        let master = r.text("master");
        let rate = r.uint::<u32>("rate");
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(bus), Some(name)) = (bus, name) else {
            continue;
        };
        if rate == Some(0) {
            cx.malformed(row, "column `rate`: a data rate of 0 bit/s");
            continue;
        }
        let master = match master {
            None => None,
            Some(key) => match cx.functions.get(key) {
                Some(f) => Some(Endpoint::Cp(f.id, 0)),
                None => {
                    cx.unresolved(row, &format!("unknown function `{key}` in column `master`"));
                    continue;
                }
            },
        };
        seen.insert(row.key().to_owned(), ());
        let id = record_id::<LogicalNet>("logical_nets", row.key());
        out.insert(row.key().to_owned(), id);
        cx.records.push(Record::LogicalNet(LogicalNet {
            id,
            v: 1,
            bus,
            name,
            master,
            rate: rate.map(BusRate),
            source_ref: Some(source_ref("logical_nets", row.key())),
            extra: Default::default(),
        }));
    }
    out
}

/// `physical_nets.csv` → `physical_net`; returns the IDs by key. An `order_hint` entry that names no function drops
/// the row.
pub(crate) fn emit_physical_nets(
    cx: &mut Cx<'_>,
    table: Option<&Table>,
) -> BTreeMap<String, Id<PhysicalNet>> {
    let mut out = BTreeMap::new();
    let Some(table) = table else { return out };
    let mut seen = BTreeMap::<String, ()>::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let name = r.req_ident("name");
        let medium = r.req_choice("medium", MEDIA);
        let hint = r.list("order_hint");
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(name), Some(medium)) = (name, medium) else {
            continue;
        };
        let mut order_hint: Vec<Id<Function>> = Vec::new();
        let mut missing = None;
        for key in hint {
            match cx.functions.get(key) {
                Some(f) => order_hint.push(f.id),
                None => missing = Some(key),
            }
        }
        if let Some(key) = missing {
            cx.unresolved(row, &format!("unknown function `{key}` in column `order_hint`"));
            continue;
        }
        seen.insert(row.key().to_owned(), ());
        let id = record_id::<PhysicalNet>("physical_nets", row.key());
        out.insert(row.key().to_owned(), id);
        cx.records.push(Record::PhysicalNet(PhysicalNet {
            id,
            v: 1,
            name,
            medium,
            order_hint,
            source_ref: Some(source_ref("physical_nets", row.key())),
            extra: Default::default(),
        }));
    }
    out
}

/// `bus_ports.csv` → the `bus_port` aspect of the function named by the row's key (emitted before). A key that names
/// no function, an unknown net key, or a numeric address on a port that is not PROFIBUS DP drops the row.
pub(crate) fn emit_bus_ports(
    cx: &mut Cx<'_>,
    table: Option<&Table>,
    logical: &BTreeMap<String, Id<LogicalNet>>,
    physical: &BTreeMap<String, Id<PhysicalNet>>,
) {
    let Some(table) = table else { return };
    let mut seen = BTreeMap::<String, ()>::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let interface = r.req_ident("interface");
        let port_label = r.ident("port_label");
        let bus = r.req_choice("bus", BUS_SYSTEMS);
        let role = r.req_choice("role", ROLES);
        let address = r.req("address");
        let termination = r.choice("termination", ON_OFF);
        let lnet = r.text("logical_net");
        let pnet = r.text("physical_net");
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(interface), Some(bus), Some(role), Some(address)) = (interface, bus, role, address) else {
            continue;
        };
        let address = match address {
            "tbd" => AddrValue::Tbd,
            "not_applicable" => AddrValue::NotApplicable,
            n => match (n.parse::<u8>(), &bus) {
                (Ok(n), BusSystem::ProfibusDp) => AddrValue::Value(NetAddress::Profibus(n)),
                _ => {
                    cx.malformed(
                        row,
                        &format!(
                            "column `address`: `{n}` is not tbd, not_applicable or a PROFIBUS station address of a \
                             profibus_dp port"
                        ),
                    );
                    continue;
                }
            },
        };
        let logical_net = match lnet {
            None => None,
            Some(k) => match logical.get(k) {
                Some(id) => Some(*id),
                None => {
                    cx.unresolved(row, &format!("unknown logical net `{k}` in column `logical_net`"));
                    continue;
                }
            },
        };
        let physical_net = match pnet {
            None => None,
            Some(k) => match physical.get(k) {
                Some(id) => Some(*id),
                None => {
                    cx.unresolved(
                        row,
                        &format!("unknown physical net `{k}` in column `physical_net`"),
                    );
                    continue;
                }
            },
        };
        let Some(function) = cx.functions.get(row.key()).map(|f| f.id) else {
            cx.unresolved(row, &format!("unknown function `{}`", row.key()));
            continue;
        };
        let Some(record) = cx.records.iter_mut().find_map(|rec| match rec {
            Record::Function(f) if f.id == function => Some(f),
            _ => None,
        }) else {
            cx.unresolved(row, &format!("the function `{}` was dropped", row.key()));
            continue;
        };
        seen.insert(row.key().to_owned(), ());
        record.bus_port = Some(BusPort {
            interface,
            port_label,
            bus,
            role,
            address,
            termination,
            logical_net,
            physical_net,
            extra: Default::default(),
        });
    }
}
