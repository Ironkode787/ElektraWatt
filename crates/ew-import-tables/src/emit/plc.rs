// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC tables (doc 20 §6.2, §11.2, §11.3; doc 10 §2.2–§2.6): configuration projects, the PLC section of
//! a part, channels and device links. The station and module aspects are read with the devices
//! (`devices.rs`); the bus connections with the connections (`connect.rs`).

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_interop_api::{Code, Severity};
use ew_mltext::Ident;
use ew_schema::{
    AddrSpec, Channel, ChannelKind, ChannelSpec, ChannelState, Device, DeviceLink, DeviceLinkKind, Endpoint,
    IoClass, IoDir, IoRange, MapSpec, PlcConfigProject, PlcRole, PlcSection, PositionMap, Record,
    SolenoidChannel, StationRole,
};

use super::{Cx, fresh};
use crate::ids::{record_id, source_ref};
use crate::table::{Row, Table, parse_uint};

/// The names of [`StationRole`] in `devices.csv`.
pub(crate) const STATION_ROLES: &[(&str, StationRole)] = &[
    ("cpu", StationRole::Cpu),
    ("head_station", StationRole::HeadStation),
    ("io_device", StationRole::IoDevice),
    ("valve_terminal", StationRole::ValveTerminal),
    ("io_link_master", StationRole::IoLinkMaster),
    ("drive", StationRole::Drive),
    ("external_partner", StationRole::ExternalPartner),
];

/// The names of [`ChannelKind`].
const CHANNEL_KINDS: &[(&str, ChannelKind)] = &[
    ("di", ChannelKind::Di),
    ("do", ChannelKind::Do),
    ("ai", ChannelKind::Ai),
    ("ao", ChannelKind::Ao),
    ("counter", ChannelKind::Counter),
    ("io_link_port", ChannelKind::IoLinkPort),
    ("power_supply", ChannelKind::PowerSupply),
];

/// The names of [`ChannelState`].
const CHANNEL_STATES: &[(&str, ChannelState)] = &[
    ("spare", ChannelState::Spare),
    ("reserved", ChannelState::Reserved),
    ("deactivated", ChannelState::Deactivated),
];

// ---------------------------------------------------------------------------------------------------------
// Configuration projects

/// A `plc_config_projects.csv` row that read cleanly and awaits its CPU reference.
pub(crate) struct PendingProject<'t> {
    row: Row<'t>,
    id: Id<PlcConfigProject>,
    name: Ident,
    profile: Ident,
    cpu: Option<&'t str>,
}

/// Reads `plc_config_projects.csv` and registers the IDs of the valid rows, so that the station aspects of the
/// devices may name them. The CPU is a device and is resolved by [`emit_config_projects`] once the devices exist.
pub(crate) fn read_config_projects<'t>(cx: &mut Cx<'_>, table: Option<&'t Table>) -> Vec<PendingProject<'t>> {
    let mut out = Vec::new();
    let Some(table) = table else { return out };
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &cx.plc_projects) {
            continue;
        }
        let mut r = row.reader();
        let name = r.req_ident("name");
        let profile = r.req_ident("profile");
        let cpu = r.text("cpu");
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(name), Some(profile)) = (name, profile) else {
            continue;
        };
        let id = record_id::<PlcConfigProject>("plc_config_projects", row.key());
        cx.plc_projects.insert(row.key().to_owned(), id);
        out.push(PendingProject {
            row,
            id,
            name,
            profile,
            cpu,
        });
    }
    out
}

/// `plc_config_projects.csv` → `plc_config_project`. A project whose CPU does not resolve is dropped, and the
/// station aspects that named it are removed from their devices (a station without its project has no meaning).
pub(crate) fn emit_config_projects(cx: &mut Cx<'_>, pending: Vec<PendingProject<'_>>) {
    for p in pending {
        let cpu = match p.cpu {
            None => None,
            Some(key) => match cx.devices.get(key) {
                Some(d) => Some(d.id),
                None => {
                    cx.unresolved(p.row, &format!("unknown device `{key}` in column `cpu`"));
                    drop_project(cx, p.row.key(), p.id);
                    continue;
                }
            },
        };
        cx.records.push(Record::PlcConfigProject(PlcConfigProject {
            id: p.id,
            v: 1,
            name: p.name,
            profile: p.profile,
            cpu,
            source_ref: Some(source_ref("plc_config_projects", p.row.key())),
            extra: Default::default(),
        }));
    }
}

/// Unregisters project `key` and removes the station aspects that name it, each with a finding.
fn drop_project(cx: &mut Cx<'_>, key: &str, id: Id<PlcConfigProject>) {
    cx.plc_projects.remove(key);
    let mut removed = Vec::new();
    for r in &mut cx.records {
        let Record::Device(d) = r else { continue };
        let names_it = d
            .aspects
            .plc_station
            .as_ref()
            .is_some_and(|s| s.config_project == id);
        if names_it {
            d.aspects.plc_station = None;
            removed.push(d.source_ref.as_ref().map(|s| s.key.clone()).unwrap_or_default());
        }
    }
    for source in removed {
        let device = source.strip_prefix("devices|").unwrap_or(&source);
        cx.findings.at(
            Code::UNRESOLVED_REFERENCE,
            Severity::Error,
            "devices.csv",
            Some(device),
            None,
            format!("its configuration project `{key}` was dropped: the station aspect is removed"),
        );
    }
}

// ---------------------------------------------------------------------------------------------------------
// The PLC section of a part

/// One `position_map` item: `<solenoid>:<channel expression>` (`14:2p`), the expression stored as
/// `MapSpec::Expr` like `bit_of` (the derivation parses it).
fn parse_solenoid(item: &str) -> Result<SolenoidChannel, String> {
    let bad = || format!("`{item}` is not <solenoid>:<channel expression> (`14:2p`)");
    let (solenoid, channel) = item.split_once(':').ok_or_else(bad)?;
    let (solenoid, channel) = (solenoid.trim(), channel.trim());
    if channel.is_empty() || channel.contains(':') {
        return Err(bad());
    }
    let solenoid: Ident = solenoid.parse().map_err(|_| bad())?;
    Ok(SolenoidChannel {
        solenoid,
        channel: MapSpec::Expr(channel.to_owned()),
        extra: Default::default(),
    })
}

/// One `io` item: `<dir>:<bits>` or `<dir>:<bits>:<class>`.
fn parse_io(item: &str) -> Result<IoRange, String> {
    let mut parts = item.split(':');
    let (dir, bits, class, rest) = (parts.next(), parts.next(), parts.next(), parts.next());
    let bad = || format!("`{item}` is not <in|out>:<bits>[:<digital|analog>]");
    if rest.is_some() {
        return Err(bad());
    }
    let dir = match dir {
        Some("in") => IoDir::In,
        Some("out") => IoDir::Out,
        _ => return Err(bad()),
    };
    let bits: u32 = parse_uint(bits.ok_or_else(bad)?)?;
    let class = match class {
        None | Some("digital") => IoClass::Digital,
        Some("analog") => IoClass::Analog,
        Some(_) => return Err(bad()),
    };
    Ok(IoRange {
        dir,
        bits,
        class,
        extra: Default::default(),
    })
}

/// Reads `part_plc.csv`: the PLC section of each part, by `parts` key. The rows whose part does not exist are
/// reported by [`report_unknown_parts`] once the parts are known.
pub(crate) fn read_part_plc<'t>(
    cx: &mut Cx<'_>,
    table: Option<&'t Table>,
) -> BTreeMap<String, (Row<'t>, PlcSection)> {
    let mut out: BTreeMap<String, (Row<'t>, PlcSection)> = BTreeMap::new();
    let Some(table) = table else { return out };
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &out) {
            continue;
        }
        let mut r = row.reader();
        let role = r.req_choice(
            "role",
            &[("station", PlcRole::Station), ("module", PlcRole::Module)],
        );
        let mut io = Vec::new();
        for item in r.list("io") {
            match parse_io(item) {
                Ok(range) => io.push(range),
                Err(e) => r.fail(format!("column `io`: {e}")),
            }
        }
        let count = r.uint::<u16>("channel_count");
        let kind = r.choice("channel_kind", CHANNEL_KINDS);
        let data_type = r.ident("data_type");
        let designation = r.text("designation").map(str::to_owned);
        let bit_of = r.text("bit_of").map(|s| MapSpec::Expr(s.to_owned()));
        let pin_of = r.text("pin_of").map(|s| MapSpec::Expr(s.to_owned()));
        let unbound = r.choice("unbound", CHANNEL_STATES);
        let position_count = r.uint::<u16>("positions");
        let mut map = Vec::new();
        for item in r.list("position_map") {
            match parse_solenoid(item) {
                Ok(entry) => map.push(entry),
                Err(e) => r.fail(format!("column `position_map`: {e}")),
            }
        }
        let positions = match position_count {
            Some(count) => Some(PositionMap {
                count,
                map,
                extra: Default::default(),
            }),
            None => {
                if !map.is_empty() {
                    r.fail("column `position_map` needs `positions`");
                }
                None
            }
        };
        let channels = match (count, kind) {
            (Some(count), Some(kind)) => Some(ChannelSpec {
                count,
                kind,
                data_type,
                designation,
                extra: Default::default(),
            }),
            (Some(_), None) => {
                r.fail("column `channel_kind` is empty (required with `channel_count`)");
                None
            }
            (None, Some(_)) => {
                r.fail("column `channel_kind` needs `channel_count`");
                None
            }
            (None, None) => {
                if data_type.is_some() || designation.is_some() {
                    r.fail("columns `data_type` and `designation` need `channel_count`");
                }
                None
            }
        };
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let Some(role) = role else { continue };
        out.insert(
            row.key().to_owned(),
            (
                row,
                PlcSection {
                    role,
                    io,
                    channels,
                    bit_of,
                    pin_of,
                    unbound,
                    positions,
                    extra: Default::default(),
                },
            ),
        );
    }
    out
}

/// EW-IMP-003 for every `part_plc.csv` row that no part took.
pub(crate) fn report_unknown_parts(cx: &mut Cx<'_>, rest: BTreeMap<String, (Row<'_>, PlcSection)>) {
    for (key, (row, _)) in rest {
        cx.unresolved(row, &format!("unknown part `{key}`"));
    }
}

// ---------------------------------------------------------------------------------------------------------
// Channels

/// `channels.csv` → `channel`. The ID is `plc_channel_id(module, number)` (doc 10 §2.4, ADR-0004 amendment 2),
/// not the table identity rule; `source_ref.key` is `channels|<key>`. Two rows of the same module and number
/// are a duplicate channel: the second is dropped.
pub(crate) fn emit_channels(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    let mut seen = BTreeMap::<String, ()>::new();
    let mut numbers: BTreeSet<(Id<Device>, u16)> = BTreeSet::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let module = r.req("module");
        let number = r.req_uint::<u16>("number");
        let kind = r.req_choice("kind", CHANNEL_KINDS);
        let address = r
            .choice(
                "address",
                &[
                    ("auto", AddrSpec::Auto),
                    ("tbd", AddrSpec::Tbd),
                    ("excluded", AddrSpec::Excluded),
                ],
            )
            .unwrap_or_default();
        let state = r.choice("state", CHANNEL_STATES);
        let symbol = r.ident("symbol");
        let comment = r.mltext("comment");
        let binding = r.text("binding");
        let target = r.text("target");
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(module), Some(number), Some(kind)) = (module, number, kind) else {
            continue;
        };
        let Some(module_id) = cx.devices.get(module).map(|d| d.id) else {
            cx.unresolved(row, &format!("unknown device `{module}` in column `module`"));
            continue;
        };
        if numbers.contains(&(module_id, number)) {
            cx.malformed(
                row,
                &format!("duplicate channel: `{module}` already has channel {number}"),
            );
            continue;
        }
        let binding = match binding {
            None => None,
            Some(text) => match cx.endpoint(text) {
                Ok(Endpoint::Junction(_)) => {
                    cx.malformed(row, "column `binding` must be a CP, not a junction");
                    continue;
                }
                Ok(e) => Some(e),
                Err(m) => {
                    cx.unresolved(row, &m);
                    continue;
                }
            },
        };
        let target = match target {
            None => None,
            Some(key) => match cx.functions.get(key) {
                Some(f) => Some(f.id),
                None => {
                    cx.unresolved(row, &format!("unknown function `{key}` in column `target`"));
                    continue;
                }
            },
        };
        seen.insert(row.key().to_owned(), ());
        numbers.insert((module_id, number));
        cx.records.push(Record::Channel(Channel {
            id: ew_ids::v5::plc_channel_id(module_id, number),
            v: 1,
            module: module_id,
            number,
            designation: None,
            kind,
            address,
            state,
            symbol,
            comment,
            binding,
            target,
            source_ref: Some(source_ref("channels", row.key())),
            extra: Default::default(),
        }));
    }
}

// ---------------------------------------------------------------------------------------------------------
// Device links

/// `device_links.csv` → `device_link`.
pub(crate) fn emit_device_links(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    let mut seen = BTreeMap::<String, ()>::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let from = r.req("from");
        let to = r.req("to");
        let kind = r.req_choice(
            "kind",
            &[
                ("drives", DeviceLinkKind::Drives),
                ("linkage", DeviceLinkKind::Linkage),
            ],
        );
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(from), Some(to), Some(kind)) = (from, to, kind) else {
            continue;
        };
        let mut ends = [None, None];
        for (slot, (column, key)) in ends.iter_mut().zip([("from", from), ("to", to)]) {
            *slot = cx.devices.get(key).map(|d| d.id);
            if slot.is_none() {
                cx.unresolved(row, &format!("unknown device `{key}` in column `{column}`"));
            }
        }
        let [Some(from_id), Some(to_id)] = ends else {
            continue;
        };
        if from_id == to_id {
            cx.malformed(row, "both ends are the same device");
            continue;
        }
        seen.insert(row.key().to_owned(), ());
        cx.records.push(Record::DeviceLink(DeviceLink {
            id: record_id::<DeviceLink>("device_links", row.key()),
            v: 1,
            from: from_id,
            to: to_id,
            kind,
            source_ref: Some(source_ref("device_links", row.key())),
            extra: Default::default(),
        }));
    }
}
