// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `plc_overview`: the PLC overview, one row per channel of a station (FR-PLC-022, FR-REP-019; doc 10 §5;
//! golden 908, 908.1, 908.2), and the PLC overview box of a module or station embedded on a page
//! (FR-PLC-052).

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{MlText, NaturalKey, natural_key};
use ew_derive::ew_schema::{ChannelKind, Device, ReportBlock};
use ew_derive::{ChannelAddr, PageRef, PlcRow, PlcRows, Q, RepAnchorCell};

use super::conn::{put, tag_short};
use crate::kind::{EmbeddedLayout, ReportKindDef};
use crate::rowset::{Cell, CellType, ColumnDef, LinkRef, LinkTarget, Row, RowId, RowKey, RowSet};

/// The PLC overview.
///
/// The selection is the block's manual selection (`manual.include`): its devices that carry a `plc_station` aspect
/// give every row of that station, its devices that carry a `plc_module` aspect give the rows of that module (the PLC
/// overview box of a module, FR-PLC-052); a selection that names neither prints every station. Stations print in
/// natural order of their tags, each with its rows in `plc_rows` order. Rows come from `plc_rows(station)`
/// (`ew-derive`): the channels of every module in slot order, then by channel number, in every state
/// (doc 10 §5). The columns are those of doc 10 §5: module, part, address (rendered by the configuration
/// project's profile; "TBD" for an open item, empty when excluded), channel number, designation, kind,
/// symbol (manual only before the templates of 1.0), function text (the target's, else the tracked process
/// tag), process tag, target, state, and the frame cell of the bound coil's representation; the cell links
/// to the bound CP, the coil's position (doc 10 §5, doc 20 §11.3).
#[derive(Clone, Copy, Debug)]
pub struct PlcOverview;

impl ReportKindDef for PlcOverview {
    fn name(&self) -> &'static str {
        "plc_overview"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("module", CellType::Ident, "Module").weighted(9),
            ColumnDef::new("part", CellType::Ident, "Part").weighted(8),
            ColumnDef::new("address", CellType::Address, "Address").weighted(8),
            ColumnDef::new("channel", CellType::Number, "Channel").weighted(8),
            ColumnDef::new("designation", CellType::Ident, "Designation").weighted(10),
            ColumnDef::new("kind", CellType::Text, "Kind").weighted(5),
            ColumnDef::new("symbol", CellType::Ident, "Symbol").weighted(9),
            ColumnDef::new("function", CellType::Text, "Function text").weighted(12),
            ColumnDef::new("process_tag", CellType::Ident, "Process tag").weighted(11),
            ColumnDef::new("target", CellType::Ident, "Target").weighted(11),
            ColumnDef::new("state", CellType::Text, "State").weighted(10),
            ColumnDef::new("page", CellType::CellRef, "Page").weighted(8),
        ]
    }

    fn rows(&self, q: &Q, block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        let selection = Selection::of(q, block);
        for station in selection.stations(q) {
            let whole = selection.whole_station(station);
            for r in q.get::<PlcRows>(station).iter() {
                if whole || selection.modules.contains(&r.module) {
                    let row = plc_row(q, &set.columns, r);
                    set.rows.push(row);
                }
            }
        }
        set
    }

    /// The PLC overview box (FR-PLC-052, doc 10 §5): the channel's number, address, symbol, function text and state
    /// and the cross-reference to its coil's representation, under the title "PLC OVERVIEW" and a header row, in one
    /// flow column. The weights are the I/O list's for the same columns (tenths of a millimetre of the widest text at
    /// the 1.8 mm cap height, plus 0.6 mm); the page column's is estimated the same way from its widest Golden A cell
    /// (`203-D7`), since its header is shorter than the I/O list's "Target page".
    fn embedded_layout(&self) -> EmbeddedLayout {
        EmbeddedLayout {
            columns: [
                ("channel", 114),
                ("address", 112),
                ("symbol", 104),
                ("function", 171),
                ("state", 154),
                ("page", 96),
            ]
            .into_iter()
            .map(|(k, w)| (k.to_owned(), w))
            .collect(),
            title: "PLC OVERVIEW".to_owned(),
            headers: true,
            flow_columns: 1,
        }
    }
}

/// The manual selection of a PLC overview block: its stations and its modules (see [`PlcOverview`]).
struct Selection {
    stations: Vec<Id<Device>>,
    modules: Vec<Id<Device>>,
}

impl Selection {
    fn of(q: &Q, block: &ReportBlock) -> Self {
        let devices = q.devices();
        let mut out = Selection {
            stations: Vec::new(),
            modules: Vec::new(),
        };
        for a in block.manual.include.as_slice() {
            let id = Id::<Device>::from_uuid(a.as_uuid());
            match devices.get(id) {
                Some(d) if d.aspects.plc_station.is_some() => out.stations.push(id),
                Some(d) if d.aspects.plc_module.is_some() => out.modules.push(id),
                _ => {}
            }
        }
        out
    }

    /// Whether the selection names neither a station nor a module (every station prints).
    fn is_empty(&self) -> bool {
        self.stations.is_empty() && self.modules.is_empty()
    }

    /// Whether every row of `station` prints.
    fn whole_station(&self, station: Id<Device>) -> bool {
        self.is_empty() || self.stations.contains(&station)
    }

    /// The stations to read, in natural order of their tags: the selected ones, every station when modules are
    /// selected (a module's rows sit under its station's), else every station.
    fn stations(&self, q: &Q) -> Vec<Id<Device>> {
        let stations = if self.modules.is_empty() && !self.stations.is_empty() {
            self.stations.clone()
        } else {
            ew_derive::stations(q)
        };
        sorted_by_tag(q, stations)
    }
}

/// `stations` in natural order of their tags, without repeats.
fn sorted_by_tag(q: &Q, stations: Vec<Id<Device>>) -> Vec<Id<Device>> {
    let mut keyed: Vec<(NaturalKey, Id<Device>)> = stations
        .into_iter()
        .map(|s| (natural_key(&tag_short(q, s)), s))
        .collect();
    keyed.sort();
    keyed.dedup();
    keyed.into_iter().map(|(_, s)| s).collect()
}

/// A channel kind as the PLC overview and the I/O list print it.
pub(super) fn kind_name(k: &ChannelKind) -> &'static str {
    match k {
        ChannelKind::Di => "DI",
        ChannelKind::Do => "DO",
        ChannelKind::Ai => "AI",
        ChannelKind::Ao => "AO",
        ChannelKind::Counter => "counter",
        ChannelKind::IoLinkPort => "IO-Link",
        ChannelKind::PowerSupply => "supply",
        ChannelKind::Unknown(_) => "other",
    }
}

fn ident(text: Option<&str>) -> Cell {
    match text.filter(|t| !t.is_empty()) {
        Some(t) => Cell::Ident(t.to_owned()),
        None => Cell::Empty,
    }
}

fn plc_row(q: &Q, columns: &[ColumnDef], r: &PlcRow) -> Row {
    let mut cells = vec![Cell::Empty; columns.len()];
    put(columns, &mut cells, "module", Cell::Ident(tag_short(q, r.module)));
    if let Some(part) = r.part.and_then(|p| q.parts().get(p)) {
        put(
            columns,
            &mut cells,
            "part",
            Cell::Ident(part.item.part_number.as_str().to_owned()),
        );
    }
    put(columns, &mut cells, "address", address_cell(&r.address));
    put(columns, &mut cells, "channel", Cell::Number(i64::from(r.number)));
    put(
        columns,
        &mut cells,
        "designation",
        ident(r.text.designation.as_deref()),
    );
    put(
        columns,
        &mut cells,
        "kind",
        Cell::Text(MlText::neutral(kind_name(&r.kind))),
    );
    put(columns, &mut cells, "symbol", ident(r.text.symbol.as_deref()));
    if let Some(t) = &r.text.function_text {
        put(columns, &mut cells, "function", Cell::Text(t.clone()));
    }
    put(
        columns,
        &mut cells,
        "process_tag",
        ident(r.text.process_tag.as_deref()),
    );
    if let Some(d) = r.target_device {
        put(columns, &mut cells, "target", Cell::Ident(tag_short(q, d)));
    }
    put(
        columns,
        &mut cells,
        "state",
        Cell::Text(MlText::neutral(r.state.name())),
    );
    let links = page_cell(q, columns, &mut cells, r);
    let mut row = Row::data(
        RowId::entity(r.channel).with(RowKey::Name("channel".to_owned())),
        cells,
    );
    row.links = links;
    row
}

/// A channel's address cell as the PLC overview and the I/O list print it: the address rendered by the
/// configuration project's profile, "TBD" for an open item, empty when excluded or unresolved.
pub(super) fn address_cell(address: &ChannelAddr) -> Cell {
    match address {
        ChannelAddr::At { text, .. } => Cell::Address(text.clone()),
        ChannelAddr::Tbd => Cell::Tbd,
        ChannelAddr::Excluded | ChannelAddr::Unresolved => Cell::Empty,
    }
}

/// Sets the `page` cell of a channel row, the frame cell of the bound coil's representation (or of the target), and
/// returns the row's link to the bound CP, the coil's position (doc 10 §5): shared by the PLC overview and the I/O
/// list.
pub(super) fn page_cell(q: &Q, columns: &[ColumnDef], cells: &mut [Cell], r: &PlcRow) -> Vec<LinkRef> {
    let page_col = columns.iter().position(|c| c.key == "page");
    let mut links = Vec::new();
    if let Some(rep) = r.rep {
        let cell = q.get::<RepAnchorCell>(rep).as_ref().clone();
        if let (Some(cell), Some(page)) = (cell, q.reps().get(rep).map(|x| x.page)) {
            put(columns, cells, "page", Cell::CellRef(PageRef::Drawn(page), cell));
            if let (Some(col), Some(cp)) = (page_col, r.cp) {
                links.push(LinkRef::whole(
                    col,
                    LinkTarget::Cp {
                        function: cp.function,
                        idx: cp.idx,
                    },
                ));
            }
        }
    }
    links
}
