// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `enclosure_legend`: the layout legend of a mounting surface (FR-LAY-012; doc 12 §5.4, doc 09 §2.2).

use ew_derive::layout::{LegendItem, layout_legend, page_surface};
use ew_derive::{DrawnPageOrder, PageRef, Q};
use ew_ids::Id;
use ew_schema::{AnyId, MountingSurface, OrderUnit, ReportBlock, ReportKind};

use super::conn::text_cell;
use super::parts::{manufacturer_cell, part_number_cell, qty_cell, unit_cell};
use crate::kind::{EmbeddedLayout, ReportKindDef};
use crate::rowset::{
    Cell, CellType, ColumnDef, LinkRef, LinkTarget, MissingReason, QtyUnit, Row, RowId, RowKey, RowKind,
    RowSet,
};

/// The enclosure legend: the rows of `ew_derive::layout::layout_legend` for the surface of the block's embedded page,
/// or of the layout page a legend list names (`layout_page`; the surface its `surface_view` shows): the enclosure, the
/// plate, one row per carrier (rails, then ducts, the designation empty, the length in metres as the quantity), then
/// the placements by designation, then one row per end-bracket part (the designation empty, the number of derived
/// brackets as the quantity; row ID `entity(surface)` then the part copy and its variant, source the part copy);
/// positions 1 … n. A placement's row links its tag to the placement ([`LinkTarget::Placement`]): to the device on the
/// layout, and back from the device to the row by Follow reference (doc 12 §5.4).
///
/// A block on a page without a surface view has no rows. A block that names no page (the whole-project rows, `ew export
/// report --kind enclosure_legend`) lists the legend of every plate a layout page shows, in page order, each after a
/// heading row "Layout 902" that names the plate's first layout page.
///
/// Printed: the title "LEGEND", two flow columns, each with a header
/// row, the position, DT, quantity and part number at weights 10 : 22 : 16 : 35. A quantity prints with its unit unless
/// it counts pieces ("0.47 m", "1"); the manufacturer, description and unit stay in the rows and the CSV.
#[derive(Clone, Copy, Debug)]
pub struct EnclosureLegend;

impl ReportKindDef for EnclosureLegend {
    fn kind(&self) -> ReportKind {
        ReportKind::EnclosureLegend
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("pos", CellType::Ident, text!("column-pos")).weighted(4),
            ColumnDef::new("tag", CellType::Tag, text!("column-dt")).weighted(14),
            ColumnDef::new("manufacturer", CellType::Ident, text!("column-manufacturer")).weighted(15),
            ColumnDef::new("part_number", CellType::Ident, text!("column-part-number")).weighted(17),
            ColumnDef::new("description", CellType::Text, text!("column-description")).weighted(30),
            ColumnDef::new("qty", CellType::Qty, text!("column-qty")).weighted(6),
            ColumnDef::new("unit", CellType::Ident, text!("column-unit")).weighted(4),
        ]
    }

    fn embedded_layout(&self) -> EmbeddedLayout {
        let printed = [("pos", 10), ("tag", 22), ("qty", 16), ("part_number", 35)];
        EmbeddedLayout {
            columns: printed.map(|(k, w)| (k.to_owned(), w)).to_vec(),
            title: crate::words::in_languages(&text!("title-legend")),
            headers: true,
            flow_columns: 2,
        }
    }

    fn rows(&self, q: &Q, block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        let page = block.embedded.as_ref().map(|e| e.page).or(block.layout_page);
        if let Some(page) = page {
            if let Some(surface) = page_surface(q, page) {
                legend_rows(q, surface, &mut set);
            }
            return set;
        }
        // The whole project (`ew export report --kind enclosure_legend`): every plate a layout page shows,
        // once, in page order, each after a heading that names its first layout page.
        let order = q.get::<DrawnPageOrder>(());
        let mut listed = Vec::new();
        for entry in &order.entries {
            let PageRef::Drawn(page) = entry.page else {
                continue;
            };
            let Some(surface) = page_surface(q, page).filter(|s| !listed.contains(s)) else {
                continue;
            };
            listed.push(surface);
            let mut heading = vec![Cell::Empty; set.columns.len()];
            if let Some(pos) = set.column("pos") {
                // The same English words in every language: no Dutch is written for them.
                heading[pos] = text_cell(format!("Layout {}", entry.folio));
            }
            set.rows.push(Row {
                id: RowId(vec![
                    RowKey::Name("layout".to_owned()),
                    RowKey::Entity(page.as_uuid()),
                ]),
                kind: RowKind::Heading,
                cells: heading,
                links: Vec::new(),
                sources: vec![page.cast()],
            });
            legend_rows(q, surface, &mut set);
        }
        set
    }
}

/// The rows of the legend of `surface`, pushed onto `set` (see [`EnclosureLegend`]).
fn legend_rows(q: &Q, surface: Id<MountingSurface>, set: &mut RowSet) {
    let tag = set.column("tag");
    for row in layout_legend(q, surface) {
        let (id, source): (RowId, AnyId) = match row.item {
            LegendItem::Enclosure(e) => (RowId::entity(e), e.cast()),
            LegendItem::Surface(s) => (RowId::entity(s), s.cast()),
            LegendItem::Carrier(c) => (RowId::entity(c), c.cast()),
            LegendItem::Placement(p) => (RowId::entity(p), p.cast()),
            LegendItem::Accessory(ref part) => {
                let mut id = RowId::entity(surface).with(RowKey::Entity(part.copy.as_uuid()));
                if let Some(v) = &part.variant {
                    id = id.with(RowKey::Name(v.as_str().to_owned()));
                }
                (id, part.copy.cast())
            }
        };
        let carrier = matches!(row.item, LegendItem::Carrier(_));
        let copy = row.part.as_ref().and_then(|p| ew_derive::printed_part(q, p));
        let (manufacturer, part_number, description, unit) = match copy {
            Some(copy) => (
                manufacturer_cell(q, &copy.item.manufacturer),
                part_number_cell(copy.item.part_number.as_str()),
                Cell::Text(copy.item.description.clone()),
                copy.item.unit.clone(),
            ),
            None => (Cell::Empty, Cell::Empty, Cell::Empty, OrderUnit::Piece),
        };
        // A carrier's quantity is its length, so its unit is the metre whatever the part's order unit.
        let unit = if carrier { OrderUnit::Metre } else { unit };
        // The printed legend has no Unit column: a quantity that is not a count of pieces carries its unit
        // ("0.47 m"); the CSV and XLSX write it as a number beside the Unit column (`table`).
        let qty = match row.qty {
            Some(qty) if !matches!(unit, OrderUnit::Piece) => Cell::Qty(qty, QtyUnit::Order(unit.clone())),
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
                unit_cell(&unit),
            ],
        );
        r.sources = std::iter::once(source)
            .chain(row.device.map(|d| d.cast()))
            .collect();
        if let (LegendItem::Placement(p), Some(tag)) = (&row.item, tag) {
            r.links.push(LinkRef::whole(tag, LinkTarget::Placement(*p)));
        }
        set.rows.push(r);
    }
}
