// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `cut_list`: everything the fitter cuts to length (doc 13 §9.3, FR-MFG-030), from `ew_derive::cut_rows`; and
//! `tube_cut_list`, a block of a project made before the cut list, which prints the cut list's Tubes group.

use ew_derive::layout::{CarrierLabel, CarrierLabelKind};
use ew_derive::{CutFor, CutGroup, CutRow, CutWhat, Q, colour_text, short_tag};
use ew_l10n::{List, Text, TextArg};
use ew_schema::{ReportBlock, ReportKind};
use ew_units::Dec3;

use super::conn::{measured_cell, put, wire_type_cell, words_cell};
use super::parts::part_number_cell;
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, MissingReason, QtyUnit, Row, RowId, RowKey, RowKind, RowSet};
use crate::words::selecting;

/// The columns of the cut list: Part number, Description, For, Pieces, Length mm, Total m.
fn cut_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::new("part_number", CellType::Ident, text!("column-part-number")).weighted(14),
        ColumnDef::new("description", CellType::Text, text!("column-description")).weighted(30),
        ColumnDef::new("for", CellType::Text, text!("column-for")).weighted(30),
        ColumnDef::new("pieces", CellType::Number, text!("column-pieces")).weighted(6),
        ColumnDef::new("piece_length", CellType::Qty, text!("column-length-mm")).weighted(8),
        ColumnDef::new("total", CellType::Qty, text!("column-total-m")).weighted(8),
    ]
}

/// The cut list: per group of `cut_rows` (Rails, Ducts, Cables, Wires, Tubes) a heading row and its rows, a group
/// without rows left out. A row prints the part's number and description (a wire without a part its size and colour, a
/// tube without one its material, size and colour), what it is for, the pieces, the piece length in millimetres and the
/// total in metres; pieces without a length print "length missing" and no total. Row IDs: a heading `("cut", group)`,
/// a row `("cut", group, n)`.
#[derive(Clone, Copy, Debug)]
pub struct CutList;

impl ReportKindDef for CutList {
    fn kind(&self) -> ReportKind {
        ReportKind::CutList
    }

    fn workshop(&self) -> bool {
        true
    }

    fn columns(&self) -> Vec<ColumnDef> {
        cut_columns()
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        for (group, rows) in ew_derive::cut_rows(q) {
            if rows.is_empty() {
                continue;
            }
            let mut cells = vec![Cell::Empty; set.columns.len()];
            cells[0] = words_cell(&group_words(group));
            set.rows.push(Row {
                id: group_id(group),
                kind: RowKind::Heading,
                cells,
                links: Vec::new(),
                sources: Vec::new(),
            });
            for (n, r) in rows.iter().enumerate() {
                set.rows.push(cut_row(q, &set.columns, group, n, r));
            }
        }
        set
    }
}

/// The tube cut list of a project made before the cut list (doc 11 §8.2): the cut list's Tubes group, under the block's
/// own title. Report block… does not offer it: the cut list holds its rows.
#[derive(Clone, Copy, Debug)]
pub struct TubeCutList;

impl ReportKindDef for TubeCutList {
    fn kind(&self) -> ReportKind {
        ReportKind::TubeCutList
    }

    /// The cut list's columns, each answering the key a block of the tube cut list of 0.3.5 stored for it
    /// (`ColumnDef::former`): the tube part's number, the material, size and colour in the description, the pieces,
    /// the total length.
    fn columns(&self) -> Vec<ColumnDef> {
        cut_columns()
            .into_iter()
            .map(|c| match c.key.as_str() {
                "part_number" => c.formerly(&["part"]),
                "description" => c.formerly(&["material", "size", "colour"]),
                "pieces" => c.formerly(&["count"]),
                "total" => c.formerly(&["length"]),
                _ => c,
            })
            .collect()
    }

    /// The Tubes group's rows. Each keeps the identity its row had in the tube cut list of 0.3.5, its first tube, and
    /// names no other source: a block's choice of rows made then names that tube alone, and with every tube of the row
    /// as its sources the row would no longer be left out by it.
    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        for (group, rows) in ew_derive::cut_rows(q) {
            if group == CutGroup::Tubes {
                for (n, r) in rows.iter().enumerate() {
                    let mut row = cut_row(q, &set.columns, group, n, r);
                    row.sources.clear();
                    set.rows.push(row);
                }
            }
        }
        set
    }

    fn offered(&self) -> bool {
        false
    }
}

/// A group's heading word.
fn group_words(group: CutGroup) -> Text {
    match group {
        CutGroup::Rails => text!("cell-group-rails"),
        CutGroup::Ducts => text!("cell-group-ducts"),
        CutGroup::Cables => text!("cell-group-cables"),
        CutGroup::Wires => text!("cell-group-wires"),
        CutGroup::Tubes => text!("cell-group-tubes"),
    }
}

/// The row ID of a group's heading.
fn group_id(group: CutGroup) -> RowId {
    RowId(vec![RowKey::Name("cut".to_owned()), RowKey::Index(group as u32)])
}

/// A carrier by its label in words ("Rail 1", in Dutch "Kabelgoot 1" for a duct).
fn carrier_words(label: CarrierLabel) -> TextArg {
    let kind = match label.kind {
        CarrierLabelKind::Rail => "rail",
        CarrierLabelKind::Duct => "duct",
    };
    TextArg::Text(
        selecting(key!("cell-carrier"))
            .with("kind", kind)
            .with("n", TextArg::Number(i64::from(label.n))),
    )
}

/// One row of the cut list.
fn cut_row(q: &Q, columns: &[ColumnDef], group: CutGroup, n: usize, r: &CutRow) -> Row {
    let mut cells = vec![Cell::Empty; columns.len()];
    let mut set = |key: &str, cell: Cell| put(columns, &mut cells, key, cell);
    match &r.what {
        CutWhat::Part(p) => match ew_derive::printed_part(q, p) {
            Some(copy) => {
                set("part_number", part_number_cell(copy.item.part_number.as_str()));
                set("description", Cell::Text(copy.item.description.clone()));
            }
            None => set("part_number", Cell::Missing(MissingReason::Part)),
        },
        CutWhat::Wire { csa, colour } => set("description", wire_type_cell(q, csa.as_ref(), colour.as_ref())),
        CutWhat::Tube {
            material,
            size,
            colour,
        } => {
            let text = [
                material.clone(),
                size.clone(),
                colour.as_ref().map(colour_text).unwrap_or_default(),
            ]
            .into_iter()
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
            set("description", measured_cell(q, &Text::bare(text)));
        }
        CutWhat::Unknown => set("part_number", Cell::Missing(MissingReason::Part)),
    }
    let what_for = match &r.what_for {
        CutFor::Carriers(labels) => {
            let items: Vec<TextArg> = labels.iter().copied().map(carrier_words).collect();
            words_cell(&Text::bare(List::new(
                items,
                text!("cell-list-comma"),
                text!("cell-list-comma"),
            )))
        }
        CutFor::Cable(c) => Cell::ident(&short_tag(q, *c)),
        CutFor::Names(names) => Cell::ident(names),
    };
    set("for", what_for);
    set(
        "pieces",
        Cell::Number(i64::try_from(r.pieces).unwrap_or(i64::MAX)),
    );
    match r.length {
        Some(nm) => {
            set(
                "piece_length",
                Cell::stale_if(
                    Cell::Qty(Dec3(i64::try_from(nm / 1_000).unwrap_or(i64::MAX)), QtyUnit::None),
                    r.stale,
                ),
            );
            let pieces = i128::try_from(r.pieces).unwrap_or(i128::MAX);
            set(
                "total",
                Cell::stale_if(
                    Cell::Qty(
                        ew_derive::layout::metres_of_nm(nm.saturating_mul(pieces)),
                        QtyUnit::None,
                    ),
                    r.stale,
                ),
            );
        }
        None => set("piece_length", Cell::Missing(MissingReason::Length)),
    }
    // The row is its first record's (a duct's row and its cover's, of one duct, by their parts), and names every
    // record it is cut for.
    let id = match (r.sources.first(), &r.what) {
        (Some(first), CutWhat::Part(p)) if group == CutGroup::Ducts => {
            RowId::entity(*first).with(RowKey::Entity(p.copy.as_uuid()))
        }
        (Some(first), _) => RowId::entity(*first),
        (None, _) => {
            let mut id = group_id(group);
            id.0.push(RowKey::Index(u32::try_from(n).unwrap_or(u32::MAX)));
            id
        }
    };
    let mut row = Row::data(id, cells);
    row.sources.clone_from(&r.sources);
    row
}
