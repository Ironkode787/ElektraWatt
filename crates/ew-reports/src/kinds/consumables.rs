// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `consumables`: what to order of what the wires consume, beside the parts list (doc 13 §3.6, FR-MFG-049), from
//! `ew_derive::consumable_rows`.

use ew_derive::{EndForm, Q, csa_with_unit};
use ew_l10n::{List, Text, TextArg};
use ew_schema::{OrderUnit, PartRef, ReportBlock, ReportKind};

use super::conn::{measured_cell, put, wire_type_cell, words_cell};
use super::parts::{part_number_cell, qty_cell, unit_cell};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowKey, RowKind, RowSet};
use crate::words::selecting;

/// The consumables: three groups, each under a heading row, a group without rows left out.
///
/// - Wire: per wire part (else size and colour) the wires (For) and their summed lengths in metres exact to the
///   millimetre; with wires without a length "0.45 and 7 wires without a length", with none "length missing for 7
///   wires".
/// - Ferrules and lugs: per part the ends it is crimped on (a twin ferrule once), For the wires' sizes ("2 × 0.75 mm²"
///   for a twin ferrule); ends the stock has no part for under their size, "no ferrule found".
/// - Markers: per label set and marker part the labels, For the set ("Wire markers").
///
/// Row IDs: a heading `("consumables", group)`, a row `("consumables", group, n)`.
#[derive(Clone, Copy, Debug)]
pub struct Consumables;

impl ReportKindDef for Consumables {
    fn kind(&self) -> ReportKind {
        ReportKind::Consumables
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("part_number", CellType::Ident, text!("column-part-number")).weighted(14),
            ColumnDef::new("description", CellType::Text, text!("column-description")).weighted(34),
            ColumnDef::new("for", CellType::Text, text!("column-for")).weighted(16),
            ColumnDef::new("quantity", CellType::Qty, text!("column-quantity")).weighted(16),
            ColumnDef::new("unit", CellType::Ident, text!("column-unit")).weighted(5),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let columns = self.columns();
        let c = ew_derive::consumable_rows(q);
        let mut groups: Vec<(Text, Vec<Vec<Cell>>)> = Vec::new();
        let wires = c
            .wires
            .iter()
            .map(|w| {
                let (number, description) = match &w.part {
                    Some(p) => part_cells(q, p),
                    None => (Cell::Empty, wire_type_cell(q, w.csa.as_ref(), w.colour.as_ref())),
                };
                let count = |n: usize| TextArg::Number(i64::try_from(n).unwrap_or(i64::MAX));
                let metres = ew_derive::layout::metres_of_nm(w.length);
                let quantity = match (w.without_length, w.wires) {
                    (0, _) => qty_cell(metres),
                    (missing, all) if missing == all => {
                        words_cell(&selecting(key!("cell-length-missing-for")).with("count", count(missing)))
                    }
                    (missing, _) => measured_cell(
                        q,
                        &selecting(key!("cell-metres-and-missing"))
                            .with("metres", metres.to_string())
                            .with("count", count(missing)),
                    ),
                };
                cells(
                    &columns,
                    number,
                    description,
                    words_cell(&selecting(key!("cell-wires-count")).with("count", count(w.wires))),
                    quantity,
                    OrderUnit::Metre,
                )
            })
            .collect();
        groups.push((text!("cell-group-wire"), wires));
        let ends = c
            .ends
            .iter()
            .map(|e| {
                let sizes: Vec<TextArg> = e
                    .sizes
                    .iter()
                    .filter_map(csa_with_unit)
                    .map(|s| {
                        match e.form {
                            EndForm::TwinFerrule => TextArg::Text(text!("cell-twin-size", s)),
                            EndForm::Ferrule | EndForm::RingLug => TextArg::Plain(s),
                        }
                    })
                    .collect();
                let what_for = measured_cell(
                    q,
                    &Text::bare(List::new(
                        sizes,
                        text!("cell-list-comma"),
                        text!("cell-list-comma"),
                    )),
                );
                let (number, description) = match e.part {
                    Some(p) => part_cells(q, &PartRef::new(p)),
                    None => (
                        Cell::Empty,
                        words_cell(&match e.form {
                            EndForm::Ferrule => text!("cell-no-ferrule"),
                            EndForm::TwinFerrule => text!("cell-no-twin-ferrule"),
                            EndForm::RingLug => text!("cell-no-ring-lug"),
                        }),
                    ),
                };
                let count = i64::try_from(e.count).unwrap_or(i64::MAX);
                cells(
                    &columns,
                    number,
                    description,
                    what_for,
                    Cell::Number(count),
                    OrderUnit::Piece,
                )
            })
            .collect();
        groups.push((text!("cell-group-ferrules"), ends));
        let markers = c
            .markers
            .iter()
            .map(|m| {
                let (number, description) = match m.part {
                    Some(p) => part_cells(q, &PartRef::new(p)),
                    None => (Cell::Empty, words_cell(&text!("cell-no-marker"))),
                };
                let labels = i64::try_from(m.labels).unwrap_or(i64::MAX);
                cells(
                    &columns,
                    number,
                    description,
                    words_cell(&ew_derive::text::label_set(m.set)),
                    Cell::Number(labels),
                    OrderUnit::Piece,
                )
            })
            .collect();
        groups.push((text!("cell-group-markers"), markers));
        let mut set = RowSet::new(columns);
        for (g, (words, rows)) in groups.into_iter().enumerate() {
            if rows.is_empty() {
                continue;
            }
            let g = u32::try_from(g).unwrap_or(u32::MAX);
            let id = |more: Option<u32>| {
                let mut keys = vec![RowKey::Name("consumables".to_owned()), RowKey::Index(g)];
                keys.extend(more.map(RowKey::Index));
                RowId(keys)
            };
            let mut heading = vec![Cell::Empty; set.columns.len()];
            heading[0] = words_cell(&words);
            set.rows.push(Row {
                id: id(None),
                kind: RowKind::Heading,
                cells: heading,
                links: Vec::new(),
                sources: Vec::new(),
            });
            for (n, cells) in rows.into_iter().enumerate() {
                set.rows
                    .push(Row::data(id(Some(u32::try_from(n).unwrap_or(u32::MAX))), cells));
            }
        }
        set
    }
}

/// The part-number and description cells of a part use, as it prints ([`ew_derive::printed_part`]).
fn part_cells(q: &Q, part: &PartRef) -> (Cell, Cell) {
    ew_derive::printed_part(q, part).map_or((Cell::Empty, Cell::Empty), |c| {
        (
            part_number_cell(c.item.part_number.as_str()),
            Cell::Text(c.item.description.clone()),
        )
    })
}

/// A row's cells in the columns' order.
fn cells(
    columns: &[ColumnDef],
    number: Cell,
    description: Cell,
    what_for: Cell,
    qty: Cell,
    unit: OrderUnit,
) -> Vec<Cell> {
    let mut cells = vec![Cell::Empty; columns.len()];
    put(columns, &mut cells, "part_number", number);
    put(columns, &mut cells, "description", description);
    put(columns, &mut cells, "for", what_for);
    put(columns, &mut cells, "quantity", qty);
    put(columns, &mut cells, "unit", unit_cell(&unit));
    cells
}
