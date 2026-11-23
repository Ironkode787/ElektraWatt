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
    AddrValue, BusPort, BusSystem, Channel, ConnectionKind, Device, Endpoint, Function, IoDir, LengthSource,
    LogicalNet, NetAddress, OnOff, Part, PhysicalNet, PlcConfigProject, PortClass, PortMode, PortRole,
    ReportBlock,
};
use ew_derive::{
    BusSegments, ChannelStates, ChannelTargets, EffectiveState, FunctionCps, ModuleOrder, ModuleStartOf,
    NetMaster, NetMembersOf, Q, SegmentMember, Start, bus_word, external_partner_text, iolink_device,
    module_io, module_part, net_address_text, port_windows, profile_of, short_tag, stations,
};

use super::conn::{put, words_cell};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowKey, RowSet};

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

/// A cable's length as the lists print it: metres with three decimals at most (`45 m`), else empty.
fn length_cell(q: &Q, cable: Id<Device>) -> Cell {
    match q
        .devices()
        .get(cable)
        .and_then(|d| d.aspects.cable.as_ref().map(|c| c.length.clone()))
    {
        Some(LengthSource::Manual(nm)) => text(&format!("{} m", ew_derive::ew_units::Dec3(nm.0 / 1_000_000))),
        Some(LengthSource::FromPart) => words_cell("from part", "uit artikel"),
        _ => Cell::Empty,
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
            ColumnDef::new("station", CellType::Ident, "Station", "Station").weighted(10),
            ColumnDef::new("slot", CellType::Ident, "Slot", "Slot").weighted(5),
            ColumnDef::new("module", CellType::Ident, "Module", "Module").weighted(10),
            ColumnDef::new("part", CellType::Ident, "Part", "Artikel").weighted(10),
            ColumnDef::new("input_start", CellType::Address, "Input start", "Begin ingangen").weighted(8),
            ColumnDef::new("input_length", CellType::Number, "Input bytes", "Ingangsbytes").weighted(7),
            ColumnDef::new(
                "output_start",
                CellType::Address,
                "Output start",
                "Begin uitgangen",
            )
            .weighted(8),
            ColumnDef::new("output_length", CellType::Number, "Output bytes", "Uitgangsbytes").weighted(7),
            ColumnDef::new("used", CellType::Number, "Used", "Gebruikt").weighted(5),
            ColumnDef::new("spare", CellType::Number, "Spare", "Reserve").weighted(5),
            ColumnDef::new("reserved", CellType::Number, "Reserved", "Gereserveerd").weighted(6),
            ColumnDef::new("deactivated", CellType::Number, "Deactivated", "Gedeactiveerd").weighted(7),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        for cfg in cfgs(q) {
            let profile = profile_of(q, cfg).filter(|p| p.absolute);
            for m in q.get::<ModuleOrder>(cfg).iter() {
                let mut cells = vec![Cell::Empty; set.columns.len()];
                let c = &set.columns;
                put(c, &mut cells, "station", Cell::ident(&short_tag(q, m.station)));
                put(
                    c,
                    &mut cells,
                    "slot",
                    Cell::ident(&ew_derive::slot_text(m.slot).unwrap_or_default()),
                );
                put(c, &mut cells, "module", Cell::ident(&short_tag(q, m.module)));
                if let Some((copy, _)) = module_part(q, m.module) {
                    put(c, &mut cells, "part", Cell::ident(copy.item.part_number.as_str()));
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
/// master first, then every port with an address in ID order; then every port with an address that names no logical
/// net, in ID order, with an empty net: logical net, station, device type (its part), address or PROFINET name, IP,
/// mask, master flag, rate, configuration project and the `Tbd` flag.
#[derive(Clone, Copy, Debug)]
pub struct NetworkAddressList;

impl ReportKindDef for NetworkAddressList {
    fn name(&self) -> &'static str {
        "network_address_list"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("net", CellType::Ident, "Logical net", "Logisch net").weighted(10),
            ColumnDef::new("station", CellType::Ident, "Station", "Station").weighted(12),
            ColumnDef::new("device_type", CellType::Ident, "Device type", "Apparaattype").weighted(10),
            ColumnDef::new("address", CellType::Ident, "Address or name", "Adres of naam").weighted(10),
            ColumnDef::new("ip", CellType::Ident, "IP address", "IP-adres").weighted(9),
            ColumnDef::new("mask", CellType::Ident, "Subnet mask", "Subnetmasker").weighted(9),
            ColumnDef::new("master", CellType::Bool, "Master", "Master").weighted(5),
            ColumnDef::new("rate", CellType::Number, "Rate (bit/s)", "Snelheid (bit/s)").weighted(8),
            ColumnDef::new(
                "config_project",
                CellType::Ident,
                "Configuration project",
                "Configuratieproject",
            )
            .weighted(9),
            ColumnDef::new("tbd", CellType::Bool, "TBD", "N.t.b.").weighted(4),
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
                put(c, &mut cells, "net", Cell::ident(n.name.as_str()));
                put(
                    c,
                    &mut cells,
                    "station",
                    Cell::ident(&external_partner_text(q, external)),
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
                let Some(port) = functions.get(*f).and_then(|func| func.bus_port.clone()) else {
                    continue;
                };
                let master = matches!(members.master, Some(NetMaster::Port(m)) if m == *f);
                if let Some(row) = port_row(
                    q,
                    &set.columns,
                    *f,
                    &port,
                    Some(n.name.as_str()),
                    master,
                    rate.clone(),
                ) {
                    set.rows.push(row);
                }
            }
        }
        // FR-BUS-008: a port with an address that names no logical net is listed too, after the nets, in ID order
        for (f, func) in functions.iter() {
            let Some(port) = func.bus_port.as_ref() else {
                continue;
            };
            if port
                .logical_net
                .is_some_and(|n| q.table::<LogicalNet>().contains(n))
            {
                continue;
            }
            if let Some(row) = port_row(q, &set.columns, f, port, None, false, Cell::Empty) {
                set.rows.push(row);
            }
        }
        set
    }
}

/// One port's row of the network address list: `None` for a port without an address (`NotApplicable`). `master` is
/// whether its net names it as master; a port of role master or controller is one too.
fn port_row(
    q: &Q,
    columns: &[ColumnDef],
    f: Id<Function>,
    port: &BusPort,
    net: Option<&str>,
    master: bool,
    rate: Cell,
) -> Option<Row> {
    if matches!(port.address, AddrValue::NotApplicable) {
        return None;
    }
    let mut cells = vec![Cell::Empty; columns.len()];
    let c = columns;
    if let Some(net) = net {
        put(c, &mut cells, "net", Cell::ident(net));
    }
    if let Some(d) = q.functions().get(f).and_then(|func| func.device) {
        put(c, &mut cells, "station", Cell::ident(&short_tag(q, d)));
        let part = q.devices().get(d).and_then(|dev| {
            dev.parts
                .first()
                .and_then(|u| q.parts().get(u.part.copy).map(|p| p.item.part_number.to_string()))
        });
        put(
            c,
            &mut cells,
            "device_type",
            Cell::ident(&part.unwrap_or_default()),
        );
        let cfg = ew_derive::station_of(q, d)
            .and_then(|s| {
                q.devices()
                    .get(s)
                    .and_then(|x| x.aspects.plc_station.as_ref().map(|p| p.config_project))
            })
            .and_then(|c| q.table::<PlcConfigProject>().get(c).map(|c| c.name.to_string()));
        put(
            c,
            &mut cells,
            "config_project",
            Cell::ident(&cfg.unwrap_or_default()),
        );
    }
    match &port.address {
        AddrValue::Tbd => put(c, &mut cells, "address", Cell::Tbd),
        AddrValue::Value(NetAddress::Profinet(p)) => {
            put(c, &mut cells, "address", Cell::ident(p.name.as_str()));
            put(
                c,
                &mut cells,
                "ip",
                Cell::ident(&p.ip.map(|ip| ip.to_string()).unwrap_or_default()),
            );
            put(
                c,
                &mut cells,
                "mask",
                Cell::ident(&p.mask.map(|m| m.to_string()).unwrap_or_default()),
            );
        }
        AddrValue::Value(a) => put(
            c,
            &mut cells,
            "address",
            Cell::ident(&net_address_text(a).unwrap_or_default()),
        ),
        _ => {}
    }
    let master = master || matches!(port.role, PortRole::Master | PortRole::Controller);
    put(c, &mut cells, "master", Cell::Bool(master));
    put(c, &mut cells, "rate", rate);
    put(c, &mut cells, "tbd", Cell::Bool(port.address == AddrValue::Tbd));
    Some(Row::data(
        RowId::entity(f).with(RowKey::Name("port".to_owned())),
        cells,
    ))
}

// ---------------------------------------------------------------------------------------------------------
// bus_overview

/// `bus_overview` (FR-REP-020, FR-BUS-007, doc 10 §3.6, §5): the generated bus topology overview, per logical net in
/// name order (segments without a net last), then per member of each physical net's segments in walk order from the
/// master end: position, station, port, address, connector, termination, cable, cable length and the external flag. A
/// cable is the member between the two functions it joins. A passive port is a connector, but for a switch (a passive
/// interface whose part counts its sockets, doc 10 §2.7), which stands as a station with its port and no address.
#[derive(Clone, Copy, Debug)]
pub struct BusOverview;

impl ReportKindDef for BusOverview {
    fn name(&self) -> &'static str {
        "bus_overview"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("net", CellType::Ident, "Logical net", "Logisch net").weighted(10),
            ColumnDef::new("position", CellType::Number, "Position", "Positie").weighted(6),
            ColumnDef::new("station", CellType::Ident, "Station", "Station").weighted(12),
            ColumnDef::new("port", CellType::Ident, "Port", "Poort").weighted(8),
            ColumnDef::new("address", CellType::Ident, "Address", "Adres").weighted(10),
            ColumnDef::new("connector", CellType::Ident, "Connector", "Connector").weighted(9),
            ColumnDef::new("termination", CellType::Text, "Termination", "Afsluiting").weighted(8),
            ColumnDef::new("cable", CellType::Ident, "Cable", "Kabel").weighted(8),
            ColumnDef::new("length", CellType::Text, "Length", "Lengte").weighted(7),
            ColumnDef::new("external", CellType::Bool, "External", "Extern").weighted(6),
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
                put(c, &mut cells, "net", Cell::ident(&net));
                let row_id = match m {
                    SegmentMember::Cable(cable) => {
                        put(c, &mut cells, "cable", Cell::ident(&short_tag(q, *cable)));
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
                            (Some(p), Some(d)) if p.role == PortRole::Passive && !switch_port(q, d, &p) => {
                                put(c, &mut cells, "connector", Cell::ident(&short_tag(q, d)));
                                put(c, &mut cells, "termination", termination(&p.termination));
                            }
                            (Some(p), device) => {
                                if let Some(d) = device {
                                    put(c, &mut cells, "station", Cell::ident(&short_tag(q, d)));
                                }
                                put(c, &mut cells, "port", Cell::ident(p.interface.as_str()));
                                let address = match &p.address {
                                    AddrValue::Tbd => Cell::Tbd,
                                    AddrValue::Value(a) => {
                                        Cell::ident(&net_address_text(a).unwrap_or_default())
                                    }
                                    _ => Cell::Empty,
                                };
                                put(c, &mut cells, "address", address);
                                put(c, &mut cells, "termination", termination(&p.termination));
                            }
                            (None, _) => {
                                put(
                                    c,
                                    &mut cells,
                                    "station",
                                    Cell::ident(&ew_derive::bus_end_label(q, *f)),
                                );
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

/// Whether port `p` of device `d` is a switch's: a part of the device declares its interface with a socket count.
fn switch_port(q: &Q, d: Id<Device>, p: &ew_derive::ew_schema::BusPort) -> bool {
    let parts = q.parts();
    q.devices().get(d).is_some_and(|dev| {
        dev.parts.iter().any(|u| {
            parts
                .get(u.part.copy)
                .and_then(|c| c.item.sections.bus.as_ref())
                .is_some_and(|b| {
                    b.ports.iter().any(|x| {
                        x.interface == p.interface && x.port_label == p.port_label && x.ports.is_some()
                    })
                })
        })
    })
}

fn termination(t: &Option<OnOff>) -> Cell {
    match t {
        Some(OnOff::On) => words_cell("on", "aan"),
        Some(OnOff::Off) => words_cell("off", "uit"),
        _ => Cell::Empty,
    }
}

// ---------------------------------------------------------------------------------------------------------
// bus_cable_list

/// The two functions a `Bus` connection joins, each with the part copy of the plug at that end.
type Ends = ((Id<Function>, Option<Id<Part>>), (Id<Function>, Option<Id<Part>>));

/// `bus_cable_list` (FR-BUS-008, doc 10 §5): one row per cable of a `Bus` connection, in natural order of its tag:
/// cable, bus system, part, length, the two ends (a plug at an end named after it, `-101A1:DP · plug EW-GEN-PLG-1`)
/// and the physical net (segment) of its ports.
#[derive(Clone, Copy, Debug)]
pub struct BusCableList;

impl ReportKindDef for BusCableList {
    fn name(&self) -> &'static str {
        "bus_cable_list"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("cable", CellType::Ident, "Cable", "Kabel").weighted(9),
            ColumnDef::new("bus", CellType::Text, "Bus system", "Bussysteem").weighted(10),
            ColumnDef::new("part", CellType::Ident, "Part", "Artikel").weighted(10),
            ColumnDef::new("length", CellType::Text, "Length", "Lengte").weighted(7),
            ColumnDef::new("from", CellType::Ident, "From", "Van").weighted(12),
            ColumnDef::new("to", CellType::Ident, "To", "Naar").weighted(12),
            ColumnDef::new("segment", CellType::Ident, "Segment", "Segment").weighted(9),
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
            let plug = |p: &Option<Box<ew_derive::ew_schema::BusPlug>>| p.as_ref().map(|p| p.part.copy);
            by_cable
                .entry(cable)
                .or_default()
                .push(((*a, plug(&line.plug_a)), (*b, plug(&line.plug_b))));
        }
        let mut cables: Vec<_> = by_cable
            .into_iter()
            .map(|(c, ends)| (natural_key(&short_tag(q, c)), c, ends))
            .collect();
        cables.sort();
        for (_, cable, mut ends) in cables {
            ends.sort();
            let Some(&((a, plug_a), (b, plug_b))) = ends.first() else {
                continue;
            };
            let port = [a, b]
                .into_iter()
                .find_map(|f| functions.get(f).and_then(|x| x.bus_port.clone()));
            let mut cells = vec![Cell::Empty; set.columns.len()];
            let c = &set.columns;
            put(c, &mut cells, "cable", Cell::ident(&short_tag(q, cable)));
            if let Some(p) = &port {
                let bus = match &p.bus {
                    BusSystem::Unknown(_) => words_cell(bus_word(&p.bus), "overig"),
                    known => text(bus_word(known)),
                };
                put(c, &mut cells, "bus", bus);
                if let Some(net) = p
                    .physical_net
                    .and_then(|n| q.table::<PhysicalNet>().get(n).map(|n| n.name.to_string()))
                {
                    put(c, &mut cells, "segment", Cell::ident(&net));
                }
            }
            let part = q.devices().get(cable).and_then(|d| {
                d.parts
                    .first()
                    .and_then(|u| q.parts().get(u.part.copy).map(|p| p.item.part_number.to_string()))
            });
            put(c, &mut cells, "part", Cell::ident(&part.unwrap_or_default()));
            put(c, &mut cells, "length", length_cell(q, cable));
            let end = |f: Id<Function>, plug: Option<Id<Part>>| {
                let words = ew_derive::bus_end_label(q, f);
                match plug.and_then(|p| q.parts().get(p)) {
                    Some(p) => words_cell(
                        &format!("{words} \u{b7} plug {}", p.item.part_number),
                        &format!("{words} \u{b7} stekker {}", p.item.part_number),
                    ),
                    None => Cell::ident(&words),
                }
            };
            put(c, &mut cells, "from", end(a, plug_a));
            put(c, &mut cells, "to", end(b, plug_b));
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
            ColumnDef::new("master", CellType::Ident, "Master", "Master").weighted(10),
            ColumnDef::new("port", CellType::Number, "Port", "Poort").weighted(5),
            ColumnDef::new("mode", CellType::Text, "Mode", "Modus").weighted(8),
            ColumnDef::new("class", CellType::Ident, "Class", "Klasse").weighted(5),
            ColumnDef::new("device", CellType::Ident, "Device", "Apparaat").weighted(10),
            ColumnDef::new("vendor_id", CellType::Number, "Vendor ID", "Fabrikant-ID").weighted(7),
            ColumnDef::new("device_id", CellType::Number, "Device ID", "Apparaat-ID").weighted(7),
            ColumnDef::new("input_bits", CellType::Number, "Input bits", "Ingangsbits").weighted(6),
            ColumnDef::new("output_bits", CellType::Number, "Output bits", "Uitgangsbits").weighted(6),
            ColumnDef::new("cable", CellType::Ident, "Cable", "Kabel").weighted(8),
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
                    put(c, &mut cells, "master", Cell::ident(&short_tag(q, m.module)));
                    put(c, &mut cells, "port", Cell::Number(i64::from(w.number) + 1));
                    let mode = match w.mode {
                        PortMode::IoLink => text("IO-Link"),
                        PortMode::Di => text("DI"),
                        PortMode::Dq => text("DQ"),
                        PortMode::Deactivated => words_cell("deactivated", "gedeactiveerd"),
                        PortMode::Unknown(_) => words_cell("other", "overig"),
                    };
                    put(c, &mut cells, "mode", mode);
                    let class_text = match class {
                        Some(PortClass::A) => "A",
                        Some(PortClass::B) => "B",
                        _ => "",
                    };
                    put(c, &mut cells, "class", Cell::ident(class_text));
                    put(c, &mut cells, "device", Cell::ident(&short_tag(q, device)));
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
                        put(c, &mut cells, "cable", Cell::ident(&short_tag(q, cable)));
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
            ColumnDef::new("f_dest", CellType::Number, "F-destination", "F-bestemming").weighted(7),
            ColumnDef::new("f_source", CellType::Number, "F-source", "F-bron").weighted(7),
            ColumnDef::new("module", CellType::Ident, "Module", "Module").weighted(10),
            ColumnDef::new("channel", CellType::Number, "Channel", "Kanaal").weighted(6),
            ColumnDef::new("pair", CellType::Number, "Pair", "Paar").weighted(5),
            ColumnDef::new(
                "target",
                CellType::Ident,
                "Sensor or actuator",
                "Sensor of actuator",
            )
            .weighted(12),
            ColumnDef::new("requirement", CellType::Ident, "Requirement", "Eis").weighted(8),
            ColumnDef::new("test_pulse", CellType::Ident, "Test pulse", "Testpuls").weighted(10),
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
                    put(c, &mut cells, "module", Cell::ident(&short_tag(q, m.module)));
                    put(c, &mut cells, "channel", Cell::Number(i64::from(ch.number)));
                    if let Some(p) = s.pair.and_then(|p| channels.get(p)) {
                        put(c, &mut cells, "pair", Cell::Number(i64::from(p.number)));
                    }
                    if let Some(d) = t.device {
                        put(c, &mut cells, "target", Cell::ident(&short_tag(q, d)));
                    }
                    if let Some(r) = &s.requirement {
                        put(c, &mut cells, "requirement", Cell::ident(r.as_str()));
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
                            .map(|d| short_tag(q, d))
                            .unwrap_or_default();
                        put(
                            c,
                            &mut cells,
                            "test_pulse",
                            Cell::ident(&format!("{device}:{pin}")),
                        );
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
