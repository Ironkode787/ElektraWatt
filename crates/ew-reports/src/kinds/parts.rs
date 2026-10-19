// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `parts_summary` and `parts_list` (FR-REP-004 … FR-REP-006; golden 901, 901.1), and the part cells the
//! other part-based kinds share.

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{NaturalKey, natural_key};
use ew_derive::ew_schema::{Device, LibRef, OrderUnit, Part, ReportBlock};
use ew_derive::ew_units::Dec3;
use ew_derive::{BomMode, BomRows, DevicePartLines, DeviceTag, Q};

use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, LinkRef, LinkTarget, QtyUnit, Row, RowId, RowKey, RowSet};

/// The summarised parts list (EPLAN `.f02`).
///
/// Component mode by default; assembly mode when the block's `params.mode` is `"assembly"`.
#[derive(Clone, Copy, Debug)]
pub struct PartsSummary;

impl ReportKindDef for PartsSummary {
    fn name(&self) -> &'static str {
        "parts_summary"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("pos", CellType::Ident, "Pos"),
            ColumnDef::new("qty", CellType::Qty, "Qty"),
            ColumnDef::new("unit", CellType::Ident, "Unit"),
            ColumnDef::new("manufacturer", CellType::Ident, "Manufacturer"),
            ColumnDef::new("part_number", CellType::Ident, "Part number"),
            ColumnDef::new("description", CellType::Text, "Description"),
            ColumnDef::new("tags", CellType::Range, "Tags"),
            ColumnDef::new("folios", CellType::Folios, "Folios"),
        ]
    }

    fn rows(&self, q: &Q, block: &ReportBlock) -> RowSet {
        let assembly = block
            .extra
            .get("params")
            .and_then(|p| p.get("mode"))
            .and_then(|m| m.as_str())
            == Some("assembly");
        let (mode, mode_name) = if assembly {
            (BomMode::Assembly, "assembly")
        } else {
            (BomMode::Component, "component")
        };
        let mut set = RowSet::new(self.columns());
        for row in q.get::<BomRows>(mode).iter() {
            let pos = if row.configured {
                format!("A{}", row.pos)
            } else {
                row.pos.to_string()
            };
            let id = RowId(vec![
                RowKey::Entity(row.key.part.as_uuid()),
                RowKey::Name(
                    row.key
                        .variant
                        .as_ref()
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default(),
                ),
                RowKey::Name(mode_name.to_owned()),
            ]);
            let mut r = Row::data(
                id,
                vec![
                    Cell::Ident(pos),
                    qty_cell(row.qty, &row.unit),
                    Cell::Ident(unit_symbol(&row.unit)),
                    Cell::Ident(manufacturer_text(&row.manufacturer)),
                    part_number_cell(&row.part_number),
                    Cell::Text(row.description.clone()),
                    Cell::Range(row.devices.clone()),
                    Cell::Folios(row.pages.clone()),
                ],
            );
            r.sources = row.devices.iter().map(|d| d.cast()).collect();
            if let Some(d) = row.devices.first() {
                r.links = vec![LinkRef {
                    column: 6,
                    target: LinkTarget::MainRep(*d),
                }];
            }
            set.rows.push(r);
        }
        set
    }
}

/// The parts list, one row per device part line (EPLAN `.f01`).
#[derive(Clone, Copy, Debug)]
pub struct PartsList;

impl ReportKindDef for PartsList {
    fn name(&self) -> &'static str {
        "parts_list"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("device", CellType::Tag, "Device"),
            ColumnDef::new("qty", CellType::Qty, "Qty"),
            ColumnDef::new("unit", CellType::Ident, "Unit"),
            ColumnDef::new("manufacturer", CellType::Ident, "Manufacturer"),
            ColumnDef::new("part_number", CellType::Ident, "Part number"),
            ColumnDef::new("description", CellType::Text, "Description"),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut keyed: Vec<(LineKey, Row)> = Vec::new();
        for device in q.devices().ids() {
            let key = tag_key(q, device);
            for (i, line) in q.get::<DevicePartLines>(device).iter().enumerate() {
                let Some(copy) = q.parts().get(line.part) else {
                    continue;
                };
                let part = &copy.item;
                let mut r = Row::data(
                    RowId::entity(device).with(RowKey::Index(i as u32)),
                    vec![
                        Cell::Tag(device),
                        qty_cell(line.qty, &part.unit),
                        Cell::Ident(unit_symbol(&part.unit)),
                        Cell::Ident(manufacturer_text(&part.manufacturer)),
                        part_number_cell(part.part_number.as_str()),
                        Cell::Text(part.description.clone()),
                    ],
                );
                r.sources = vec![device.cast()];
                keyed.push(((key.clone(), device, i), r));
            }
        }
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        let mut set = RowSet::new(self.columns());
        set.rows = keyed.into_iter().map(|(_, r)| r).collect();
        set
    }
}

/// The sort key of a parts-list row: natural key of the full tag, device, line index.
type LineKey = (NaturalKey, Id<Device>, usize);

/// The natural key of a device's full tag (empty without a tag).
pub(super) fn tag_key(q: &Q, device: Id<Device>) -> NaturalKey {
    let tag = q.get::<DeviceTag>(device);
    natural_key(tag.as_ref().as_ref().map(|t| t.full.as_str()).unwrap_or(""))
}

/// A quantity in the part's order unit.
pub(super) fn qty_cell(qty: Dec3, unit: &OrderUnit) -> Cell {
    Cell::Qty(qty, QtyUnit::Order(unit.clone()))
}

/// The symbol of an order unit (`pc`, `m`, `set`).
pub(super) fn unit_symbol(unit: &OrderUnit) -> String {
    match unit {
        OrderUnit::Piece => "pc".to_owned(),
        OrderUnit::Metre => "m".to_owned(),
        OrderUnit::Set => "set".to_owned(),
        OrderUnit::Unknown(v) => v.as_str().unwrap_or("?").to_owned(),
    }
}

/// A readable text of a manufacturer reference: the `key` an importer keeps on the reference (the project
/// has no manufacturer table the queries can read), else the item ID.
pub(super) fn manufacturer_text(m: &LibRef) -> String {
    match m.extra.get("key").and_then(|k| k.as_str()) {
        Some(k) => k.to_owned(),
        None => m.item.as_uuid().to_string(),
    }
}

/// The part-number cell: `Tbd` when empty or "TBD".
pub(super) fn part_number_cell(part_number: &str) -> Cell {
    if part_number.is_empty() || part_number == "TBD" {
        Cell::Tbd
    } else {
        Cell::Ident(part_number.to_owned())
    }
}

/// The manufacturer, part-number and description cells of a part.
pub(super) fn part_cells(part: &Part) -> [Cell; 3] {
    [
        Cell::Ident(manufacturer_text(&part.manufacturer)),
        part_number_cell(part.part_number.as_str()),
        Cell::Text(part.description.clone()),
    ]
}
