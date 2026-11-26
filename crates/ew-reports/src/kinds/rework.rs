// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rework list's rows (doc 13 §3.5). No registered kind: it compares two states, the release and now, so it is
//! made when a person asks for it and is never a page of the set.

use ew_derive::{Q, ReworkAction, ReworkItem};

use super::conn::Wires;
use super::wire_list::{WireList, wire_row};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowSet};

/// The wire list's columns a rework row prints, in this order, as far as the wire list has them.
const WIRE_COLUMNS: [&str; 12] = [
    "no",
    "from",
    "from_cp",
    "to",
    "to_cp",
    "colour",
    "cross_section",
    "length",
    "from_end",
    "to_end",
    "from_marker",
    "to_marker",
];

/// The rows of the rework list between the release `base` and `now` (`ew_derive::rework`): Action, the wire list's
/// columns of [`WIRE_COLUMNS`] with their cells (a pulled wire as the release had it, every other row as it is now)
/// and Change, the row's changes in words.
pub fn rework_rows(base: &Q, now: &Q, items: &[ReworkItem]) -> RowSet {
    let list = WireList.columns();
    let picked: Vec<usize> = WIRE_COLUMNS
        .iter()
        .filter_map(|key| list.iter().position(|c| c.key == *key))
        .collect();
    let mut columns = vec![ColumnDef::new("action", CellType::Text, text!("column-action")).weighted(10)];
    columns.extend(picked.iter().map(|i| list[*i].clone()));
    columns.push(ColumnDef::new("change", CellType::Text, text!("column-change")).weighted(30));
    let (base_wires, now_wires) = (Wires::of(base), Wires::of(now));
    let mut set = RowSet::new(columns);
    set.rows = items
        .iter()
        .map(|item| {
            let (q, wires) = match item.action {
                ReworkAction::Pull => (base, &base_wires),
                _ => (now, &now_wires),
            };
            let wire = wire_row(q, wires, &item.wire);
            let mut cells = vec![words(&ew_derive::text::rework_action(item.action))];
            cells.extend(picked.iter().map(|i| wire.cells[*i].clone()));
            cells.push(if item.changes.is_empty() {
                Cell::Empty
            } else {
                words(&ew_derive::text::rework_changes(&item.changes))
            });
            let mut row = Row::data(RowId::entity(item.wire.wire.id), cells);
            row.sources = wire.sources;
            row
        })
        .collect();
    set
}

/// Words of `ew-derive` as a cell, in every language of its text files.
fn words(t: &ew_l10n::Text) -> Cell {
    Cell::Text(ew_derive::text::in_languages(t))
}
