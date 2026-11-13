// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `io_list`: the I/O assignment list, one row per channel of each station's modules (FR-PLC-023; doc 10 §5).

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{AddrValue, Channel, Device, PlcConfigProject, ReportBlock};
use ew_derive::{EffectiveState, FunctionCps, PlcRow, PlcRows, Q, Slot};

use super::conn::{put, tag_short};
use super::plc_overview::{address_cell, kind_name, page_cell};
use crate::kind::{ParamDef, ParamKind, ParamValue, ReportKindDef, kind_params};
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowKey, RowSet};

/// The block parameter that lists the deactivated channels too.
const INCLUDE_DEACTIVATED: &str = "include_deactivated";

/// The I/O list.
///
/// - **Stations:** the devices of the block's manual selection (`manual.include`) that carry a `plc_station` aspect,
///   else every station; in the order of `ew_derive::stations` (configuration project, station order, ID; doc
///   10 §5).
/// - **Rows:** `plc_rows(station)`: the channels of every module in slot order, then by channel number. A channel
///   whose effective state is `deactivated` is left out unless the block parameter `params.include_deactivated` is
///   `true` (FR-PLC-023: "the same 28 rows ... 56 with the deactivated filter off"), read through `kind_params`:
///   a value that is not a boolean gives the default, `false`, and one `MalformedShim` note naming the
///   block and the key.
/// - **Columns:** the 20 columns of FR-PLC-023 this build fills (passed terminals, signal
///   range and the safety address are added with their writers): configuration project, CPU, station, bus address
///   (the station's first bus port with an address: "TBD" for the open item, the PROFIBUS station number), slot
///   (`n` positional, `rack.slot` or `rack.slot.subslot`), module, part, channel number, designation, kind, address,
///   symbol, data type (the part's channel spec), comment, function text, process tag, target, target pin (the bound
///   CP's designation), target page (the frame cell of the bound coil's representation, linked to the bound CP, as
///   the PLC overview) and state.
#[derive(Clone, Copy, Debug)]
pub struct IoList;

impl ReportKindDef for IoList {
    fn name(&self) -> &'static str {
        "io_list"
    }

    /// The weights are the printed width, in tenths of a millimetre, that the widest Golden A text of each column
    /// (the header or a cell of the 56 rows) takes at the 1.8 mm cap-height floor, plus 0.6 mm: the 20 columns fit the
    /// house table's 260.96 mm without an overflow (the brief's stop 1; 248.3 mm at the floor).
    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("config_project", CellType::Ident, "Configuration project").weighted(269),
            ColumnDef::new("cpu", CellType::Ident, "CPU").weighted(66),
            ColumnDef::new("station", CellType::Ident, "Station").weighted(101),
            ColumnDef::new("bus_address", CellType::Ident, "Bus address").weighted(161),
            ColumnDef::new("slot", CellType::Ident, "Slot").weighted(61),
            ColumnDef::new("module", CellType::Ident, "Module").weighted(106),
            ColumnDef::new("part", CellType::Ident, "Part").weighted(103),
            ColumnDef::new("channel", CellType::Number, "Channel").weighted(114),
            ColumnDef::new("designation", CellType::Ident, "Designation").weighted(159),
            ColumnDef::new("kind", CellType::Text, "Kind").weighted(70),
            ColumnDef::new("address", CellType::Address, "Address").weighted(112),
            ColumnDef::new("symbol", CellType::Ident, "Symbol").weighted(104),
            ColumnDef::new("data_type", CellType::Ident, "Data type").weighted(131),
            ColumnDef::new("comment", CellType::Text, "Comment").weighted(134),
            ColumnDef::new("function", CellType::Text, "Function text").weighted(171),
            ColumnDef::new("process_tag", CellType::Ident, "Process tag").weighted(153),
            ColumnDef::new("target", CellType::Ident, "Target").weighted(143),
            ColumnDef::new("target_pin", CellType::Ident, "Target pin").weighted(136),
            ColumnDef::new("page", CellType::CellRef, "Target page").weighted(158),
            ColumnDef::new("state", CellType::Text, "State").weighted(154),
        ]
    }

    /// `include_deactivated`: a boolean, `false` by default.
    fn params(&self) -> &'static [ParamDef] {
        &[ParamDef {
            key: INCLUDE_DEACTIVATED,
            kind: ParamKind::Bool,
            default: ParamValue::Bool(false),
        }]
    }

    fn rows(&self, q: &Q, block: &ReportBlock) -> RowSet {
        let include_deactivated = kind_params(q, block).bool(INCLUDE_DEACTIVATED);
        let mut set = RowSet::new(self.columns());
        for station in stations_of(q, block) {
            let head = Station::of(q, station);
            for r in q.get::<PlcRows>(station).iter() {
                if r.state == EffectiveState::Deactivated && !include_deactivated {
                    continue;
                }
                let row = io_row(q, &set.columns, &head, r);
                set.rows.push(row);
            }
        }
        set
    }
}

/// The stations of the block: the selected stations, else all, in `ew_derive::stations` order.
fn stations_of(q: &Q, block: &ReportBlock) -> Vec<Id<Device>> {
    let all = ew_derive::stations(q);
    let selected: Vec<Id<Device>> = block
        .manual
        .include
        .as_slice()
        .iter()
        .map(|a| Id::<Device>::from_uuid(a.as_uuid()))
        .collect();
    if selected.iter().any(|s| all.contains(s)) {
        all.into_iter().filter(|s| selected.contains(s)).collect()
    } else {
        all
    }
}

/// The cells every row of one station repeats.
struct Station {
    config_project: Cell,
    cpu: Cell,
    station: Cell,
    bus_address: Cell,
}

impl Station {
    fn of(q: &Q, station: Id<Device>) -> Station {
        let cfg = q
            .devices()
            .get(station)
            .and_then(|d| d.aspects.plc_station.as_ref().map(|s| s.config_project))
            .and_then(|c| q.table::<PlcConfigProject>().get(c).cloned());
        let config_project = cfg
            .as_ref()
            .map_or(Cell::Empty, |c| Cell::Ident(c.name.as_str().to_owned()));
        let cpu = cfg
            .as_ref()
            .and_then(|c| c.cpu)
            .map_or(Cell::Empty, |d| ident(&tag_short(q, d)));
        Station {
            config_project,
            cpu,
            station: ident(&tag_short(q, station)),
            bus_address: bus_address(q, station),
        }
    }
}

/// The station's bus address: the first port (by function ID) of the station device with an address; "TBD" for the
/// open item, else the address as reports print it (`ew_derive::net_address_text`: "3", "cpx-1 192.168.0.10").
fn bus_address(q: &Q, station: Id<Device>) -> Cell {
    let mut functions: Vec<_> = q.idx().functions_of_device(station).collect();
    functions.sort();
    let functions_table = q.functions();
    for f in functions {
        let Some(port) = functions_table.get(f).and_then(|f| f.bus_port.clone()) else {
            continue;
        };
        match port.address {
            AddrValue::Tbd => return Cell::Tbd,
            AddrValue::Value(a) => {
                if let Some(text) = ew_derive::net_address_text(&a) {
                    return Cell::Ident(text);
                }
            }
            AddrValue::NotApplicable | AddrValue::Unknown(_) => {}
        }
    }
    Cell::Empty
}

fn ident(text: &str) -> Cell {
    if text.is_empty() {
        Cell::Empty
    } else {
        Cell::Ident(text.to_owned())
    }
}

/// The slot as the list prints it: `n` for a positional slot, `rack.slot` or `rack.slot.subslot`.
fn slot_text(slot: Slot) -> Option<String> {
    match slot {
        Slot::Positional(n) => Some(n.to_string()),
        Slot::Rack {
            rack,
            slot,
            subslot: None,
        } => Some(format!("{rack}.{slot}")),
        Slot::Rack {
            rack,
            slot,
            subslot: Some(sub),
        } => Some(format!("{rack}.{slot}.{sub}")),
        Slot::Unresolved => None,
    }
}

fn io_row(q: &Q, columns: &[ColumnDef], head: &Station, r: &PlcRow) -> Row {
    let mut cells = vec![Cell::Empty; columns.len()];
    put(columns, &mut cells, "config_project", head.config_project.clone());
    put(columns, &mut cells, "cpu", head.cpu.clone());
    put(columns, &mut cells, "station", head.station.clone());
    put(columns, &mut cells, "bus_address", head.bus_address.clone());
    if let Some(slot) = slot_text(r.slot) {
        put(columns, &mut cells, "slot", Cell::Ident(slot));
    }
    put(columns, &mut cells, "module", ident(&tag_short(q, r.module)));
    let section = ew_derive::module_part(q, r.module);
    if let Some((copy, _)) = &section {
        put(
            columns,
            &mut cells,
            "part",
            Cell::Ident(copy.item.part_number.as_str().to_owned()),
        );
    }
    put(columns, &mut cells, "channel", Cell::Number(i64::from(r.number)));
    put(
        columns,
        &mut cells,
        "designation",
        ident(r.text.designation.as_deref().unwrap_or_default()),
    );
    put(
        columns,
        &mut cells,
        "kind",
        Cell::Text(MlText::neutral(kind_name(&r.kind))),
    );
    put(columns, &mut cells, "address", address_cell(&r.address));
    put(
        columns,
        &mut cells,
        "symbol",
        ident(r.text.symbol.as_deref().unwrap_or_default()),
    );
    if let Some(t) = section
        .as_ref()
        .and_then(|(_, s)| s.channels.as_ref())
        .and_then(|c| c.data_type.as_ref())
    {
        put(columns, &mut cells, "data_type", ident(t.as_str()));
    }
    if let Some(comment) = q
        .table::<Channel>()
        .get(r.channel)
        .map(|c| c.comment.clone())
        .filter(|c| !c.is_empty())
    {
        put(columns, &mut cells, "comment", Cell::Text(comment));
    }
    if let Some(t) = &r.text.function_text {
        put(columns, &mut cells, "function", Cell::Text(t.clone()));
    }
    put(
        columns,
        &mut cells,
        "process_tag",
        ident(r.text.process_tag.as_deref().unwrap_or_default()),
    );
    if let Some(d) = r.target_device {
        put(columns, &mut cells, "target", ident(&tag_short(q, d)));
    }
    if let Some(cp) = r.cp {
        let pin = q
            .get::<FunctionCps>(cp.function)
            .iter()
            .find(|c| c.idx == cp.idx)
            .map(|c| c.designation.clone())
            .unwrap_or_default();
        put(columns, &mut cells, "target_pin", ident(&pin));
    }
    let links = page_cell(q, columns, &mut cells, r);
    put(
        columns,
        &mut cells,
        "state",
        Cell::Text(MlText::neutral(r.state.name())),
    );
    let mut row = Row::data(
        RowId::entity(r.channel).with(RowKey::Name("channel".to_owned())),
        cells,
    );
    row.links = links;
    row
}
