// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC and bus lists of 1.0 (doc 10 §5): `plc_card_list` (FR-REP-045), `network_address_list` and
//! `bus_cable_list` (FR-BUS-008), `bus_overview` (FR-REP-020, FR-BUS-007: the generated bus topology overview, one
//! block per project that lists every logical net's segments), `iolink_port_list` (FR-BUS-014) and `safety_io_list`
//! (FR-PLC-041). Every row comes from the derived PLC and bus values of `ew-derive`; nothing is stored.

use std::collections::BTreeMap;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{MlText, natural_key};
use ew_derive::ew_plcaddr::{Area, PlcAddress, Purpose, Size};
use ew_derive::ew_schema::{
    AddrValue, BusSystem, Channel, ConnectionKind, Device, Endpoint, Function, IoDir, LengthSource,
    LogicalNet, NetAddress, OnOff, PhysicalNet, PlcConfigProject, PortClass, PortMode, PortRole, ReportBlock,
};
use ew_derive::{
    BusSegments, ChannelStates, ChannelTargets, EffectiveState, FunctionCps, ModuleOrder, ModuleStartOf,
    NetMaster, NetMembersOf, Q, SegmentMember, Slot, Start, external_partner_text, iolink_device, module_io,
    module_part, net_address_text, port_windows, profile_of, stations,
};

use super::conn::{put, tag_short};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowKey, RowSet};

fn ident(text: &str) -> Cell {
    if text.is_empty() {
        Cell::Empty
    } else {
        Cell::Ident(text.to_owned())
    }
}

fn text(t: &str) -> Cell {
    Cell::Text(MlText::neutral(t))
}

/// The configuration projects of the project's stations, in station order, each once.
fn cfgs(q: &Q) -> Vec<Id<PlcConfigProject>> {
    let mut out = Vec::new();
    for s in stations(q) {
        if let Some(c) = q
            .devices()
            .get(s)
            .and_then(|d| d.aspects.plc_station.as_ref().map(|p| p.config_project))
        {
            if !out.contains(&c) {
                out.push(c);
            }
        }
    }
    out
}

/// A slot as the lists print it: `n` positional, `rack.slot` or `rack.slot.subslot`.
fn slot_text(slot: Slot) -> String {
    match slot {
        Slot::Positional(n) => n.to_string(),
        Slot::Rack {
            rack,
            slot,
            subslot: None,
        } => format!("{rack}.{slot}"),
        Slot::Rack {
            rack,
            slot,
            subslot: Some(s),
        } => format!("{rack}.{slot}.{s}"),
        Slot::Unresolved => String::new(),
    }
}

/// The logical nets in natural order of their names.
fn logical_nets(q: &Q) -> Vec<Id<LogicalNet>> {
    let table = q.table::<LogicalNet>();
    let mut nets: Vec<_> = table
        .iter()
        .map(|(id, n)| (natural_key(n.name.as_str()), id))
        .collect();
    nets.sort();
    nets.into_iter().map(|(_, id)| id).collect()
}

fn bus_word(b: &BusSystem) -> &'static str {
    match b {
        BusSystem::ProfibusDp => "PROFIBUS DP",
        BusSystem::Profinet => "PROFINET",
        BusSystem::EtherCat => "EtherCAT",
        BusSystem::EthernetIp => "EtherNet/IP",
        BusSystem::IoLink => "IO-Link",
        BusSystem::Asi => "AS-i",
        BusSystem::CanOpen => "CANopen",
        BusSystem::ModbusTcp => "Modbus TCP",
        BusSystem::Unknown(_) => "other",
    }
}

/// A cable's length as the lists print it: metres with three decimals at most (`45 m`), else empty.
fn length_cell(q: &Q, cable: Id<Device>) -> Cell {
    match q
        .devices()
        .get(cable)
        .and_then(|d| d.aspects.cable.as_ref().map(|c| c.length.clone()))
    {
        Some(LengthSource::Manual(nm)) => {
            let mm = nm.0 / 1_000_000;
            let text = if mm % 1000 == 0 {
                format!("{} m", mm / 1000)
            } else {
                format!("{}.{:03} m", mm / 1000, mm % 1000)
                    .trim_end_matches('0')
                    .to_owned()
            };
            text_cell(&text)
        }
        Some(LengthSource::FromPart) => text_cell("from part"),
        _ => Cell::Empty,
    }
}

fn text_cell(t: &str) -> Cell {
    Cell::Text(MlText::neutral(t))
}

/// A port's function as the bus lists name it: the device's tag and the interface (`-201U1:DP`), a connector by its
/// tag alone.
fn port_label(q: &Q, f: Id<Function>) -> String {
    let Some(func) = q.functions().get(f) else {
        return String::new();
    };
    match (&func.bus_port, func.device) {
        (Some(p), Some(d)) if p.role == PortRole::Passive => tag_short(q, d),
        (Some(p), Some(d)) => format!("{}:{}", tag_short(q, d), p.interface),
        (Some(p), None) => p.interface.to_string(),
        (None, _) => match &func.kind {
            ew_derive::ew_schema::FunctionKind::Ip(ip) => ip.name.to_string(),
            _ => String::new(),
        },
    }
}

// ---------------------------------------------------------------------------------------------------------
// plc_card_list

/// `plc_card_list` (FR-REP-045, doc 10 §5): one row per module of every station, in allocation order: station, slot,
/// module, part, start and length (bytes) per direction, and the channel counts per state.
#[derive(Clone, Copy, Debug)]
pub struct PlcCardList;

impl ReportKindDef for PlcCardList {
    fn name(&self) -> &'static str {
        "plc_card_list"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("station", CellType::Ident, "Station").weighted(10),
            ColumnDef::new("slot", CellType::Ident, "Slot").weighted(5),
            ColumnDef::new("module", CellType::Ident, "Module").weighted(10),
            ColumnDef::new("part", CellType::Ident, "Part").weighted(10),
            ColumnDef::new("input_start", CellType::Address, "Input start").weighted(8),
            ColumnDef::new("input_length", CellType::Number, "Input bytes").weighted(7),
            ColumnDef::new("output_start", CellType::Address, "Output start").weighted(8),
            ColumnDef::new("output_length", CellType::Number, "Output bytes").weighted(7),
            ColumnDef::new("used", CellType::Number, "Used").weighted(5),
            ColumnDef::new("spare", CellType::Number, "Spare").weighted(5),
            ColumnDef::new("reserved", CellType::Number, "Reserved").weighted(6),
            ColumnDef::new("deactivated", CellType::Number, "Deactivated").weighted(7),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        for cfg in cfgs(q) {
            let profile = profile_of(q, cfg).filter(|p| p.absolute);
            for m in q.get::<ModuleOrder>(cfg).iter() {
                let mut cells = vec![Cell::Empty; set.columns.len()];
                let c = &set.columns;
                put(c, &mut cells, "station", ident(&tag_short(q, m.station)));
                put(c, &mut cells, "slot", ident(&slot_text(m.slot)));
                put(c, &mut cells, "module", ident(&tag_short(q, m.module)));
                if let Some((copy, _)) = module_part(q, m.module) {
                    put(c, &mut cells, "part", ident(copy.item.part_number.as_str()));
                }
                let starts = *q.get::<ModuleStartOf>(m.module);
                for r in module_io(q, m.module) {
                    let (key_start, key_len, area, start) = match r.dir {
                        IoDir::In => (
                            "input_start",
                            "input_length",
                            Area::I,
                            starts.and_then(|s| s.input),
                        ),
                        IoDir::Out => (
                            "output_start",
                            "output_length",
                            Area::Q,
                            starts.and_then(|s| s.output),
                        ),
                        IoDir::Unknown(_) => continue,
                    };
                    put(
                        c,
                        &mut cells,
                        key_len,
                        Cell::Number(i64::from(r.bits.div_ceil(8))),
                    );
                    let cell = match (start, &profile) {
                        (Some(Start::At(b)), Some(p)) => {
                            Cell::Address(p.render(&PlcAddress::unit(area, Size::Byte, b), Purpose::Drawing))
                        }
                        (Some(Start::Tbd), _) => Cell::Tbd,
                        _ => Cell::Empty,
                    };
                    put(c, &mut cells, key_start, cell);
                }
                let mut counts: BTreeMap<EffectiveState, i64> = BTreeMap::new();
                for (_, s) in q.get::<ChannelStates>(m.module).iter() {
                    *counts.entry(*s).or_default() += 1;
                }
                for (key, state) in [
                    ("used", EffectiveState::Used),
                    ("spare", EffectiveState::Spare),
                    ("reserved", EffectiveState::Reserved),
                    ("deactivated", EffectiveState::Deactivated),
                ] {
                    put(
                        c,
                        &mut cells,
                        key,
                        Cell::Number(counts.get(&state).copied().unwrap_or(0)),
                    );
                }
                set.rows.push(Row::data(
                    RowId::entity(m.module).with(RowKey::Name("module".to_owned())),
                    cells,
                ));
            }
        }
        set
    }
}

// ---------------------------------------------------------------------------------------------------------
// network_address_list

/// `network_address_list` (FR-BUS-008, doc 10 §5): per logical net in name order, an external partner named as its
/// master first, then every port with an address in ID order: logical net, station, device type (its part), address
/// or PROFINET name, IP, mask, master flag, rate, configuration project and the `Tbd` flag. Golden A has two rows:
/// JE-N1000-03 (master, external) and -201U1 (slave, `Tbd`).
#[derive(Clone, Copy, Debug)]
pub struct NetworkAddressList;

impl ReportKindDef for NetworkAddressList {
    fn name(&self) -> &'static str {
        "network_address_list"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("net", CellType::Ident, "Logical net").weighted(10),
            ColumnDef::new("station", CellType::Ident, "Station").weighted(12),
            ColumnDef::new("device_type", CellType::Ident, "Device type").weighted(10),
            ColumnDef::new("address", CellType::Ident, "Address or name").weighted(10),
            ColumnDef::new("ip", CellType::Ident, "IP address").weighted(9),
            ColumnDef::new("mask", CellType::Ident, "Subnet mask").weighted(9),
            ColumnDef::new("master", CellType::Bool, "Master").weighted(5),
            ColumnDef::new("rate", CellType::Number, "Rate (bit/s)").weighted(8),
            ColumnDef::new("config_project", CellType::Ident, "Configuration project").weighted(9),
            ColumnDef::new("tbd", CellType::Bool, "TBD").weighted(4),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        let functions = q.functions();
        for net in logical_nets(q) {
            let Some(n) = q.table::<LogicalNet>().get(net).cloned() else {
                continue;
            };
            let members = q.get::<NetMembersOf>(net);
            let rate = n.rate.map_or(Cell::Empty, |r| Cell::Number(i64::from(r.0)));
            if let Some(NetMaster::External { ip, external }) = members.master {
                let mut cells = vec![Cell::Empty; set.columns.len()];
                let c = &set.columns;
                put(c, &mut cells, "net", ident(n.name.as_str()));
                put(
                    c,
                    &mut cells,
                    "station",
                    ident(&external_partner_text(q, external)),
                );
                put(c, &mut cells, "master", Cell::Bool(true));
                put(c, &mut cells, "rate", rate.clone());
                put(c, &mut cells, "tbd", Cell::Bool(false));
                set.rows.push(Row::data(
                    RowId::entity(ip).with(RowKey::Name("master".to_owned())),
                    cells,
                ));
            }
            for f in &members.ports {
                let Some(func) = functions.get(*f) else {
                    continue;
                };
                let Some(port) = func.bus_port.as_ref() else {
                    continue;
                };
                if matches!(port.address, AddrValue::NotApplicable) {
                    continue;
                }
                let mut cells = vec![Cell::Empty; set.columns.len()];
                let c = &set.columns;
                put(c, &mut cells, "net", ident(n.name.as_str()));
                if let Some(d) = func.device {
                    put(c, &mut cells, "station", ident(&tag_short(q, d)));
                    let part = q.devices().get(d).and_then(|dev| {
                        dev.parts
                            .first()
                            .and_then(|u| q.parts().get(u.part.copy).map(|p| p.item.part_number.to_string()))
                    });
                    put(c, &mut cells, "device_type", ident(&part.unwrap_or_default()));
                    let cfg = ew_derive::station_of(q, d)
                        .and_then(|s| {
                            q.devices()
                                .get(s)
                                .and_then(|x| x.aspects.plc_station.as_ref().map(|p| p.config_project))
                        })
                        .and_then(|c| q.table::<PlcConfigProject>().get(c).map(|c| c.name.to_string()));
                    put(c, &mut cells, "config_project", ident(&cfg.unwrap_or_default()));
                }
                match &port.address {
                    AddrValue::Tbd => put(c, &mut cells, "address", Cell::Tbd),
                    AddrValue::Value(NetAddress::Profinet(p)) => {
                        put(c, &mut cells, "address", ident(p.name.as_str()));
                        put(
                            c,
                            &mut cells,
                            "ip",
                            ident(&p.ip.map(|ip| ip.to_string()).unwrap_or_default()),
                        );
                        put(
                            c,
                            &mut cells,
                            "mask",
                            ident(&p.mask.map(|m| m.to_string()).unwrap_or_default()),
                        );
                    }
                    AddrValue::Value(a) => put(
                        c,
                        &mut cells,
                        "address",
                        ident(&net_address_text(a).unwrap_or_default()),
                    ),
                    _ => {}
                }
                let master = matches!(members.master, Some(NetMaster::Port(m)) if m == *f)
                    || matches!(port.role, PortRole::Master | PortRole::Controller);
                put(c, &mut cells, "master", Cell::Bool(master));
                put(c, &mut cells, "rate", rate.clone());
                put(c, &mut cells, "tbd", Cell::Bool(port.address == AddrValue::Tbd));
                set.rows.push(Row::data(
                    RowId::entity(*f).with(RowKey::Name("port".to_owned())),
                    cells,
                ));
            }
        }
        set
    }
}

// ---------------------------------------------------------------------------------------------------------
// bus_overview

/// `bus_overview` (FR-REP-020, FR-BUS-007, doc 10 §3.6, §5): the generated bus topology overview, per logical net in
/// name order (segments without a net last), then per member of each physical net's segments in walk order from the
/// master end: position, station, port, address, connector, termination, cable, cable length and the external flag. A
/// cable is the member between the two functions it joins.
#[derive(Clone, Copy, Debug)]
pub struct BusOverview;

impl ReportKindDef for BusOverview {
    fn name(&self) -> &'static str {
        "bus_overview"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("net", CellType::Ident, "Logical net").weighted(10),
            ColumnDef::new("position", CellType::Number, "Position").weighted(6),
            ColumnDef::new("station", CellType::Ident, "Station").weighted(12),
            ColumnDef::new("port", CellType::Ident, "Port").weighted(8),
            ColumnDef::new("address", CellType::Ident, "Address").weighted(10),
            ColumnDef::new("connector", CellType::Ident, "Connector").weighted(9),
            ColumnDef::new("termination", CellType::Text, "Termination").weighted(8),
            ColumnDef::new("cable", CellType::Ident, "Cable").weighted(8),
            ColumnDef::new("length", CellType::Text, "Length").weighted(7),
            ColumnDef::new("external", CellType::Bool, "External").weighted(6),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        let functions = q.functions();
        // segments by the name of their logical net
        let mut segments: Vec<(ew_derive::ew_mltext::NaturalKey, String, ew_derive::BusSegment)> = Vec::new();
        let pnets: Vec<Id<PhysicalNet>> = q.table::<PhysicalNet>().iter().map(|(id, _)| id).collect();
        for pnet in pnets {
            for s in q.get::<BusSegments>(pnet).iter() {
                let name = s
                    .logical_net
                    .and_then(|n| q.table::<LogicalNet>().get(n).map(|n| n.name.to_string()))
                    .unwrap_or_default();
                let key = natural_key(if name.is_empty() { "\u{10FFFF}" } else { &name });
                segments.push((key, name, s.clone()));
            }
        }
        segments.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, net, segment) in segments {
            let mut position = 0i64;
            for m in &segment.members {
                let mut cells = vec![Cell::Empty; set.columns.len()];
                let c = &set.columns;
                put(c, &mut cells, "net", ident(&net));
                let row_id = match m {
                    SegmentMember::Cable(cable) => {
                        put(c, &mut cells, "cable", ident(&tag_short(q, *cable)));
                        put(c, &mut cells, "length", length_cell(q, *cable));
                        RowId::entity(*cable)
                    }
                    SegmentMember::Function(f) => {
                        position += 1;
                        put(c, &mut cells, "position", Cell::Number(position));
                        let func = functions.get(*f);
                        let external = segment.external_ends.contains(f);
                        put(c, &mut cells, "external", Cell::Bool(external));
                        match (func.and_then(|x| x.bus_port.clone()), func.and_then(|x| x.device)) {
                            (Some(p), Some(d)) if p.role == PortRole::Passive => {
                                put(c, &mut cells, "connector", ident(&tag_short(q, d)));
                                put(c, &mut cells, "termination", termination(&p.termination));
                            }
                            (Some(p), device) => {
                                if let Some(d) = device {
                                    put(c, &mut cells, "station", ident(&tag_short(q, d)));
                                }
                                put(c, &mut cells, "port", ident(p.interface.as_str()));
                                let address = match &p.address {
                                    AddrValue::Tbd => Cell::Tbd,
                                    AddrValue::Value(a) => ident(&net_address_text(a).unwrap_or_default()),
                                    _ => Cell::Empty,
                                };
                                put(c, &mut cells, "address", address);
                                put(c, &mut cells, "termination", termination(&p.termination));
                            }
                            (None, _) => {
                                let partner = func.and_then(|x| match &x.kind {
                                    ew_derive::ew_schema::FunctionKind::Ip(ip) => ip.external,
                                    _ => None,
                                });
                                let name = partner
                                    .map_or_else(|| port_label(q, *f), |e| external_partner_text(q, e));
                                put(c, &mut cells, "station", ident(&name));
                            }
                        }
                        RowId::entity(*f)
                    }
                };
                set.rows
                    .push(Row::data(row_id.with(RowKey::Name("member".to_owned())), cells));
            }
        }
        set
    }
}

fn termination(t: &Option<OnOff>) -> Cell {
    match t {
        Some(OnOff::On) => text("on"),
        Some(OnOff::Off) => text("off"),
        _ => Cell::Empty,
    }
}

// ---------------------------------------------------------------------------------------------------------
// bus_cable_list

/// The two functions a `Bus` connection joins.
type Ends = (Id<Function>, Id<Function>);

/// `bus_cable_list` (FR-BUS-008, doc 10 §5): one row per cable of a `Bus` connection, in natural order of its tag:
/// cable, bus system, part, length, the two ends and the physical net (segment) of its ports.
#[derive(Clone, Copy, Debug)]
pub struct BusCableList;

impl ReportKindDef for BusCableList {
    fn name(&self) -> &'static str {
        "bus_cable_list"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("cable", CellType::Ident, "Cable").weighted(9),
            ColumnDef::new("bus", CellType::Text, "Bus system").weighted(10),
            ColumnDef::new("part", CellType::Ident, "Part").weighted(10),
            ColumnDef::new("length", CellType::Text, "Length").weighted(7),
            ColumnDef::new("from", CellType::Ident, "From").weighted(12),
            ColumnDef::new("to", CellType::Ident, "To").weighted(12),
            ColumnDef::new("segment", CellType::Ident, "Segment").weighted(9),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        let functions = q.functions();
        let mut by_cable: BTreeMap<Id<Device>, Vec<Ends>> = BTreeMap::new();
        for (_, conn) in q.connections().iter() {
            let ConnectionKind::Bus(line) = &conn.kind else {
                continue;
            };
            let (Some(cable), Endpoint::Cp(a, _), Endpoint::Cp(b, _)) = (line.cable, &conn.a, &conn.b) else {
                continue;
            };
            by_cable.entry(cable).or_default().push((*a, *b));
        }
        let mut cables: Vec<_> = by_cable
            .into_iter()
            .map(|(c, ends)| (natural_key(&tag_short(q, c)), c, ends))
            .collect();
        cables.sort();
        for (_, cable, mut ends) in cables {
            ends.sort();
            let Some(&(a, b)) = ends.first() else {
                continue;
            };
            let port = [a, b]
                .into_iter()
                .find_map(|f| functions.get(f).and_then(|x| x.bus_port.clone()));
            let mut cells = vec![Cell::Empty; set.columns.len()];
            let c = &set.columns;
            put(c, &mut cells, "cable", ident(&tag_short(q, cable)));
            if let Some(p) = &port {
                put(c, &mut cells, "bus", text(bus_word(&p.bus)));
                if let Some(net) = p
                    .physical_net
                    .and_then(|n| q.table::<PhysicalNet>().get(n).map(|n| n.name.to_string()))
                {
                    put(c, &mut cells, "segment", ident(&net));
                }
            }
            let part = q.devices().get(cable).and_then(|d| {
                d.parts
                    .first()
                    .and_then(|u| q.parts().get(u.part.copy).map(|p| p.item.part_number.to_string()))
            });
            put(c, &mut cells, "part", ident(&part.unwrap_or_default()));
            put(c, &mut cells, "length", length_cell(q, cable));
            let end = |f: Id<Function>| {
                functions
                    .get(f)
                    .and_then(|x| match &x.kind {
                        ew_derive::ew_schema::FunctionKind::Ip(ip) => {
                            ip.external.map(|e| external_partner_text(q, e))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| port_label(q, f))
            };
            put(c, &mut cells, "from", ident(&end(a)));
            put(c, &mut cells, "to", ident(&end(b)));
            set.rows.push(Row::data(
                RowId::entity(cable).with(RowKey::Name("cable".to_owned())),
                cells,
            ));
        }
        set
    }
}

// ---------------------------------------------------------------------------------------------------------
// iolink_port_list

/// `iolink_port_list` (FR-BUS-014, doc 10 §5): one row per used port of every IO-Link master (a port with a connected
/// device), in allocation and port order: master, port (1-based), mode, port class, device, vendor and device ID, the
/// process data in and out (bits) and the cable at the port's pin.
#[derive(Clone, Copy, Debug)]
pub struct IolinkPortList;

impl ReportKindDef for IolinkPortList {
    fn name(&self) -> &'static str {
        "iolink_port_list"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("master", CellType::Ident, "Master").weighted(10),
            ColumnDef::new("port", CellType::Number, "Port").weighted(5),
            ColumnDef::new("mode", CellType::Text, "Mode").weighted(8),
            ColumnDef::new("class", CellType::Ident, "Class").weighted(5),
            ColumnDef::new("device", CellType::Ident, "Device").weighted(10),
            ColumnDef::new("vendor_id", CellType::Number, "Vendor ID").weighted(7),
            ColumnDef::new("device_id", CellType::Number, "Device ID").weighted(7),
            ColumnDef::new("input_bits", CellType::Number, "Input bits").weighted(6),
            ColumnDef::new("output_bits", CellType::Number, "Output bits").weighted(6),
            ColumnDef::new("cable", CellType::Ident, "Cable").weighted(8),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        let channels = q.table::<Channel>();
        for cfg in cfgs(q) {
            for m in q.get::<ModuleOrder>(cfg).iter() {
                let Some(windows) = port_windows(q, m.module) else {
                    continue;
                };
                let class = module_part(q, m.module).and_then(|(_, s)| s.iolink.map(|w| w.class));
                let targets = q.get::<ChannelTargets>(m.module);
                for w in &windows {
                    let Some(device) = targets
                        .iter()
                        .find(|t| t.channel == w.channel)
                        .and_then(|t| t.device)
                    else {
                        continue;
                    };
                    let mut cells = vec![Cell::Empty; set.columns.len()];
                    let c = &set.columns;
                    put(c, &mut cells, "master", ident(&tag_short(q, m.module)));
                    put(c, &mut cells, "port", Cell::Number(i64::from(w.number) + 1));
                    let mode = match w.mode {
                        PortMode::IoLink => "IO-Link",
                        PortMode::Di => "DI",
                        PortMode::Dq => "DQ",
                        PortMode::Deactivated => "deactivated",
                        PortMode::Unknown(_) => "other",
                    };
                    put(c, &mut cells, "mode", text(mode));
                    let class_text = match class {
                        Some(PortClass::A) => "A",
                        Some(PortClass::B) => "B",
                        _ => "",
                    };
                    put(c, &mut cells, "class", ident(class_text));
                    put(c, &mut cells, "device", ident(&tag_short(q, device)));
                    if let Some(d) = iolink_device(q, device) {
                        if let Some(v) = d.vendor_id {
                            put(c, &mut cells, "vendor_id", Cell::Number(i64::from(v)));
                        }
                        if let Some(v) = d.device_id {
                            put(c, &mut cells, "device_id", Cell::Number(i64::from(v)));
                        }
                        put(c, &mut cells, "input_bits", Cell::Number(i64::from(d.input_bits)));
                        put(
                            c,
                            &mut cells,
                            "output_bits",
                            Cell::Number(i64::from(d.output_bits)),
                        );
                    }
                    let cable = match channels.get(w.channel).and_then(|ch| ch.binding.clone()) {
                        Some(Endpoint::Cp(f, idx)) => q.idx().connections_at_cp(f, idx).find_map(|conn| {
                            match q.connections().get(conn).map(|x| x.kind.clone()) {
                                Some(ConnectionKind::Core(core)) => q.conductors().get(core).map(|c| c.cable),
                                _ => None,
                            }
                        }),
                        _ => None,
                    };
                    if let Some(cable) = cable {
                        put(c, &mut cells, "cable", ident(&tag_short(q, cable)));
                    }
                    set.rows.push(Row::data(
                        RowId::entity(w.channel).with(RowKey::Name("port".to_owned())),
                        cells,
                    ));
                }
            }
        }
        set
    }
}

// ---------------------------------------------------------------------------------------------------------
// safety_io_list

/// `safety_io_list` (FR-PLC-041, doc 10 §5): one row per safety channel, in allocation and channel order: the module's
/// F-destination and F-source addresses, module, channel, its pair, the target device, the requirement field and the
/// test-pulse source.
#[derive(Clone, Copy, Debug)]
pub struct SafetyIoList;

impl ReportKindDef for SafetyIoList {
    fn name(&self) -> &'static str {
        "safety_io_list"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("f_dest", CellType::Number, "F-destination").weighted(7),
            ColumnDef::new("f_source", CellType::Number, "F-source").weighted(7),
            ColumnDef::new("module", CellType::Ident, "Module").weighted(10),
            ColumnDef::new("channel", CellType::Number, "Channel").weighted(6),
            ColumnDef::new("pair", CellType::Number, "Pair").weighted(5),
            ColumnDef::new("target", CellType::Ident, "Sensor or actuator").weighted(12),
            ColumnDef::new("requirement", CellType::Ident, "Requirement").weighted(8),
            ColumnDef::new("test_pulse", CellType::Ident, "Test pulse").weighted(10),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        let channels = q.table::<Channel>();
        for cfg in cfgs(q) {
            for m in q.get::<ModuleOrder>(cfg).iter() {
                let module = q
                    .devices()
                    .get(m.module)
                    .and_then(|d| d.aspects.plc_module.clone());
                let f_module = module.and_then(|p| p.safety);
                let targets = q.get::<ChannelTargets>(m.module);
                for t in targets.iter() {
                    let Some(ch) = channels.get(t.channel) else {
                        continue;
                    };
                    let Some(s) = ch.safety.as_ref() else {
                        continue;
                    };
                    let mut cells = vec![Cell::Empty; set.columns.len()];
                    let c = &set.columns;
                    if let Some(f) = &f_module {
                        if let Some(a) = f.f_dest {
                            put(c, &mut cells, "f_dest", Cell::Number(i64::from(a)));
                        }
                        if let Some(a) = f.f_source {
                            put(c, &mut cells, "f_source", Cell::Number(i64::from(a)));
                        }
                    }
                    put(c, &mut cells, "module", ident(&tag_short(q, m.module)));
                    put(c, &mut cells, "channel", Cell::Number(i64::from(ch.number)));
                    if let Some(p) = s.pair.and_then(|p| channels.get(p)) {
                        put(c, &mut cells, "pair", Cell::Number(i64::from(p.number)));
                    }
                    if let Some(d) = t.device {
                        put(c, &mut cells, "target", ident(&tag_short(q, d)));
                    }
                    if let Some(r) = &s.requirement {
                        put(c, &mut cells, "requirement", ident(r.as_str()));
                    }
                    if let Some(Endpoint::Cp(f, idx)) = &s.test_pulse {
                        let pin = q
                            .get::<FunctionCps>(*f)
                            .iter()
                            .find(|cp| cp.idx == *idx)
                            .map(|cp| cp.designation.clone())
                            .unwrap_or_default();
                        let device = q
                            .functions()
                            .get(*f)
                            .and_then(|x| x.device)
                            .map(|d| tag_short(q, d))
                            .unwrap_or_default();
                        put(c, &mut cells, "test_pulse", ident(&format!("{device}:{pin}")));
                    }
                    set.rows.push(Row::data(
                        RowId::entity(ch.id).with(RowKey::Name("channel".to_owned())),
                        cells,
                    ));
                }
            }
        }
        set
    }
}
