// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `device_list`: the device list (FR-REP-008; WP-S3.06 D5).

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{NaturalKey, natural_key};
use ew_derive::ew_schema::{Device, ReportBlock};
use ew_derive::{
    DevicePartLines, DeviceTag, LineSource, MainRep, PageRef, Q, RepAnchorCell, StructureOf, tech_data_text,
};

use super::conn::{is_listed_device, put, text_cell};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowSet};

/// The device list.
///
/// One row per device that is not a terminal (the device rows of `structure_overview`, one shared predicate), in the
/// natural order of the full tag, ties by ID; row ID `entity(device)`, sources the device. The kind does not group:
/// doc 09's grouping by location is a block's `group` over the `location` column.
///
/// - `tag`: the full designation (`=P01+J-E-2416-101F1`).
/// - `function_text`: the device's function text, empty when none.
/// - `parts`: the part numbers of the device's own part uses (`LineSource::Device` lines only, no strip accessory,
///   bridge, configuration or carrier line), each once, in line order, joined by ", ".
/// - `tech_data`: the technical-data text (`ew_derive::tech_data_text`).
/// - `location`: the location designation, as `structure_overview` prints it.
/// - `main_rep`: the frame cell of the device's main representation (`MainRep`, `RepAnchorCell`), empty when the
///   device has none.
#[derive(Clone, Copy, Debug)]
pub struct DeviceList;

impl ReportKindDef for DeviceList {
    fn name(&self) -> &'static str {
        "device_list"
    }

    /// The weights are millimetres of the house table's 260.96 mm: each column holds its widest Golden A text (the
    /// header or a cell of the 68 rows: "=P01+J-E-2416-UC1-plate", "Function text", "NEBU-G78W5-K-2-N-LE5",
    /// "Technical data", "Location (+)", "Main representation") at the house cap height, about 1.5 mm a character on
    /// folio 910, with room to spare; no text of Golden A shrinks (the brief's stop 1).
    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("tag", CellType::Ident, "Tag").weighted(52),
            ColumnDef::new("function_text", CellType::Text, "Function text").weighted(50),
            ColumnDef::new("parts", CellType::Ident, "Parts").weighted(46),
            ColumnDef::new("tech_data", CellType::Text, "Technical data").weighted(40),
            ColumnDef::new("location", CellType::Ident, "Location (+)").weighted(31),
            ColumnDef::new("main_rep", CellType::CellRef, "Main representation").weighted(42),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let columns = self.columns();
        let mut keyed: Vec<((NaturalKey, Id<Device>), Row)> = Vec::new();
        for id in q.devices().ids() {
            let Some(dev) = q.devices().get(id) else { continue };
            if !is_listed_device(dev) {
                continue;
            }
            let full = q
                .get::<DeviceTag>(id)
                .as_ref()
                .as_ref()
                .map(|t| t.full.clone())
                .unwrap_or_default();
            let mut cells = vec![Cell::Empty; columns.len()];
            put(&columns, &mut cells, "tag", ident(&full));
            if !dev.function_text.is_empty() {
                put(
                    &columns,
                    &mut cells,
                    "function_text",
                    Cell::Text(dev.function_text.clone()),
                );
            }
            put(&columns, &mut cells, "parts", ident(&parts_text(q, id)));
            put(
                &columns,
                &mut cells,
                "tech_data",
                text_cell(tech_data_text(q, id)),
            );
            let location = q
                .get::<StructureOf>(id)
                .designation(|a| a.as_str() == "location")
                .to_string();
            put(&columns, &mut cells, "location", ident(&location));
            if let Some(cell) = main_rep_cell(q, id) {
                put(&columns, &mut cells, "main_rep", cell);
            }
            let mut r = Row::data(RowId::entity(id), cells);
            r.sources = vec![id.cast()];
            keyed.push(((natural_key(&full), id), r));
        }
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        let mut set = RowSet::new(columns);
        set.rows = keyed.into_iter().map(|(_, r)| r).collect();
        set
    }
}

/// The part numbers of the device's own part uses (`LineSource::Device`), each once, in line order, joined by ", ".
fn parts_text(q: &Q, device: Id<Device>) -> String {
    let parts = q.parts();
    let mut numbers: Vec<String> = Vec::new();
    for line in q
        .get::<DevicePartLines>(device)
        .iter()
        .filter(|l| l.source == LineSource::Device)
    {
        let Some(copy) = parts.get(line.part) else {
            continue;
        };
        let number = copy.item.part_number.as_str();
        if !numbers.iter().any(|n| n == number) {
            numbers.push(number.to_owned());
        }
    }
    numbers.join(", ")
}

/// The frame cell of the device's main representation (`101-B8`); `None` without one or without a cell.
fn main_rep_cell(q: &Q, device: Id<Device>) -> Option<Cell> {
    let rep = (*q.get::<MainRep>(device))?;
    let page = q.reps().get(rep)?.page;
    let cell = q.get::<RepAnchorCell>(rep).as_ref().clone()?;
    Some(Cell::CellRef(PageRef::Drawn(page), cell))
}

fn ident(text: &str) -> Cell {
    if text.is_empty() {
        Cell::Empty
    } else {
        Cell::Ident(text.to_owned())
    }
}
