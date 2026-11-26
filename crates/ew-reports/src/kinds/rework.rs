// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rework list's rows (doc 13 §3.5). No registered kind: it compares two states, the release and now, so it is
//! made when a person asks for it and is never a page of the set.

use ew_derive::{Q, ReworkAction, ReworkItem, ReworkSubject};

use super::conn::Wires;
use super::wire_list::{WireList, wire_row};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowKey, RowSet};

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
        .enumerate()
        .map(|(n, item)| {
            let gone = matches!(
                item.action,
                ReworkAction::Pull
                    | ReworkAction::RemoveDevice
                    | ReworkAction::RemoveTerminal
                    | ReworkAction::RemoveCable
            );
            let (q, wires) = if gone {
                (base, &base_wires)
            } else {
                (now, &now_wires)
            };
            let change = if item.changes.is_empty() {
                Cell::Empty
            } else {
                words(&ew_derive::text::rework_changes(&item.changes))
            };
            let action = words(&ew_derive::text::rework_action(item.action));
            match &item.subject {
                ReworkSubject::Wire(record) => {
                    let wire = wire_row(q, wires, record);
                    let mut cells = vec![action];
                    cells.extend(picked.iter().map(|i| wire.cells[*i].clone()));
                    cells.push(change);
                    let mut row = Row::data(RowId::entity(record.wire.id), cells);
                    row.sources = wire.sources;
                    row
                }
                // A label's row names what it marks in the No. column, its device's tag.
                ReworkSubject::Label(_, target) => {
                    let mut cells = vec![action];
                    cells.extend(picked.iter().map(|i| match (list[*i].key.as_str(), target) {
                        ("no", ew_derive::LabelTarget::TerminalLevel(d, l)) => {
                            Cell::Ident(ew_derive::level_short_tag(q, *d, Some(*l)))
                        }
                        ("no", ew_derive::LabelTarget::Device(d)) => Cell::Ident(ew_derive::short_tag(q, *d)),
                        _ => Cell::Empty,
                    }));
                    cells.push(change);
                    let id = RowId(vec![
                        RowKey::Name("label".to_owned()),
                        RowKey::Index(u32::try_from(n).unwrap_or(0)),
                    ]);
                    let mut row = Row::data(id, cells);
                    if let Some(d) = target.device() {
                        row.sources = vec![d.cast()];
                    }
                    row
                }
            }
        })
        .collect();
    set
}

/// Words of `ew-derive` as a cell, in every language of its text files.
fn words(t: &ew_l10n::Text) -> Cell {
    Cell::Text(ew_derive::text::in_languages(t))
}
