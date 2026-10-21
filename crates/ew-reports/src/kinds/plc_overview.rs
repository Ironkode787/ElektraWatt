// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `plc_overview`: the PLC overview, one row per channel of a station (FR-PLC-022, FR-REP-019; doc 10 §5;
//! golden 908, 908.1, 908.2).

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{MlText, NaturalKey, natural_key};
use ew_derive::ew_schema::{ChannelKind, Device, ReportBlock};
use ew_derive::{ChannelAddr, PageRef, PlcRow, PlcRows, Q, RepAnchorCell};

use super::conn::{put, tag_short};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, LinkRef, LinkTarget, Row, RowId, RowKey, RowSet};

/// The PLC overview.
///
/// The stations are the devices of the block's manual selection (`manual.include`) that carry a
/// `plc_station` aspect, else every station in natural order of its tag. Rows come from `plc_rows(station)`
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
        for station in stations_of(q, block) {
            for r in q.get::<PlcRows>(station).iter() {
                let row = plc_row(q, &set.columns, r);
                set.rows.push(row);
            }
        }
        set
    }
}

fn stations_of(q: &Q, block: &ReportBlock) -> Vec<Id<Device>> {
    let is_station = |d: Id<Device>| {
        q.devices()
            .get(d)
            .is_some_and(|d| d.aspects.plc_station.is_some())
    };
    let selected: Vec<Id<Device>> = block
        .manual
        .include
        .as_slice()
        .iter()
        .map(|a| Id::<Device>::from_uuid(a.as_uuid()))
        .filter(|d| is_station(*d))
        .collect();
    let stations = if selected.is_empty() {
        ew_derive::stations(q)
    } else {
        selected
    };
    let mut keyed: Vec<(NaturalKey, Id<Device>)> = stations
        .into_iter()
        .map(|s| (natural_key(&tag_short(q, s)), s))
        .collect();
    keyed.sort();
    keyed.dedup();
    keyed.into_iter().map(|(_, s)| s).collect()
}

fn kind_name(k: &ChannelKind) -> &'static str {
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
    let address = match &r.address {
        ChannelAddr::At { text, .. } => Cell::Address(text.clone()),
        ChannelAddr::Tbd => Cell::Tbd,
        ChannelAddr::Excluded | ChannelAddr::Unresolved => Cell::Empty,
    };
    put(columns, &mut cells, "address", address);
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
    let page_col = columns.iter().position(|c| c.key == "page");
    let mut links = Vec::new();
    if let Some(rep) = r.rep {
        let cell = q.get::<RepAnchorCell>(rep).as_ref().clone();
        if let (Some(cell), Some(page)) = (cell, q.reps().get(rep).map(|x| x.page)) {
            put(
                columns,
                &mut cells,
                "page",
                Cell::CellRef(PageRef::Drawn(page), cell),
            );
            // The row links to the coil's position (doc 10 §5): the bound CP.
            if let (Some(col), Some(cp)) = (page_col, r.cp) {
                links.push(LinkRef {
                    column: u16::try_from(col).unwrap_or(u16::MAX),
                    target: LinkTarget::Cp {
                        function: cp.function,
                        idx: cp.idx,
                    },
                });
            }
        }
    }
    let mut row = Row::data(
        RowId::entity(r.channel).with(RowKey::Name("channel".to_owned())),
        cells,
    );
    row.links = links;
    row
}
