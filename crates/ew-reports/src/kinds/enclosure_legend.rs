// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `enclosure_legend`: the layout legend of a mounting surface (FR-LAY-012, WP-S4.01 D11; doc 12 §5.4, doc 09 §2.2;
//! golden 902).

use ew_derive::Q;
use ew_derive::ew_schema::{AnyId, OrderUnit, ReportBlock};
use ew_derive::layout::{LegendItem, layout_legend, page_surface};

use super::parts::{manufacturer_text, part_number_cell, qty_cell, unit_symbol};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, MissingReason, Row, RowId, RowSet};

/// The enclosure legend: the rows of `ew_derive::layout::layout_legend` for the surface of the block's embedded page
/// (the surface its `surface_view` shows): the enclosure, the plate, one row per carrier (rails, then ducts, the
/// designation empty, the length in metres as the quantity), then the placements by designation; positions 1 … n.
///
/// A block that is not embedded, or embedded on a page without a surface view, has no rows. Drawing the embedded
/// legend on folio 902 (its rect and columns) is WP-S4.01 part 4b; this kind is registered so that its rows exist and
/// are tested.
#[derive(Clone, Copy, Debug)]
pub struct EnclosureLegend;

impl ReportKindDef for EnclosureLegend {
    fn name(&self) -> &'static str {
        "enclosure_legend"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("pos", CellType::Ident, "Pos").weighted(4),
            ColumnDef::new("tag", CellType::Tag, "DT").weighted(14),
            ColumnDef::new("manufacturer", CellType::Ident, "Manufacturer").weighted(15),
            ColumnDef::new("part_number", CellType::Ident, "Part number").weighted(17),
            ColumnDef::new("description", CellType::Text, "Description").weighted(30),
            ColumnDef::new("qty", CellType::Qty, "Qty").weighted(6),
            ColumnDef::new("unit", CellType::Ident, "Unit").weighted(4),
        ]
    }

    fn rows(&self, q: &Q, block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        let Some(surface) = block.embedded.as_ref().and_then(|e| page_surface(q, e.page)) else {
            return set;
        };
        for row in layout_legend(q, surface) {
            let (id, source): (RowId, AnyId) = match row.item {
                LegendItem::Enclosure(e) => (RowId::entity(e), e.cast()),
                LegendItem::Surface(s) => (RowId::entity(s), s.cast()),
                LegendItem::Carrier(c) => (RowId::entity(c), c.cast()),
                LegendItem::Placement(p) => (RowId::entity(p), p.cast()),
            };
            let carrier = matches!(row.item, LegendItem::Carrier(_));
            let copy = row.part.as_ref().and_then(|p| q.parts().get(p.copy));
            let (manufacturer, part_number, description, unit) = match copy {
                Some(copy) => (
                    Cell::Ident(manufacturer_text(q, &copy.item.manufacturer)),
                    part_number_cell(copy.item.part_number.as_str()),
                    Cell::Text(copy.item.description.clone()),
                    copy.item.unit.clone(),
                ),
                None => (Cell::Empty, Cell::Empty, Cell::Empty, OrderUnit::Piece),
            };
            // A carrier's quantity is its length, so its unit is the metre whatever the part's order unit.
            let unit = if carrier { OrderUnit::Metre } else { unit };
            let qty = match row.qty {
                Some(qty) => qty_cell(qty),
                None => Cell::Missing(MissingReason::Length),
            };
            let mut r = Row::data(
                id,
                vec![
                    Cell::Ident(row.pos.to_string()),
                    row.device.map_or(Cell::Empty, Cell::Tag),
                    manufacturer,
                    part_number,
                    description,
                    qty,
                    Cell::Ident(unit_symbol(&unit)),
                ],
            );
            r.sources = std::iter::once(source)
                .chain(row.device.map(|d| d.cast()))
                .collect();
            set.rows.push(r);
        }
        set
    }
}
