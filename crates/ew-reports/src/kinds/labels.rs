// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `labels`: every label of the workshop (doc 13 §5.3), from `ew_derive::LabelRows`, one row per label in the order the
//! fitter applies them; `tube_labels` lists the tube markers of the same engine with the same columns.

use ew_derive::{LabelRecord, LabelRows, LabelSet, LabelTarget, Q, WireRecordSet, cp_label, short_tag};
use ew_schema::{ReportBlock, ReportKind};

use super::conn::{put, words_cell};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowKey, RowSet};

/// The labels list.
#[derive(Clone, Copy, Debug)]
pub struct Labels;

impl ReportKindDef for Labels {
    fn kind(&self) -> ReportKind {
        ReportKind::Labels
    }

    fn columns(&self) -> Vec<ColumnDef> {
        label_columns()
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        label_set_rows(q, |_| true)
    }
}

/// The columns of a label list (doc 13 §5.3): Set, For, Text 1 to 3, Copies, Marker part, Note.
pub(super) fn label_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::new("set", CellType::Text, text!("column-set")).weighted(12),
        ColumnDef::new("for", CellType::Text, text!("column-for")).weighted(18),
        ColumnDef::new("text_1", CellType::Ident, text!("column-text-1")).weighted(14),
        ColumnDef::new("text_2", CellType::Ident, text!("column-text-2")).weighted(14),
        ColumnDef::new("text_3", CellType::Ident, text!("column-text-3")).weighted(14),
        ColumnDef::new("copies", CellType::Ident, text!("column-copies")).weighted(6),
        ColumnDef::new("marker_part", CellType::Ident, text!("column-marker-part")).weighted(12),
        ColumnDef::new("note", CellType::Text, text!("column-note")).weighted(14),
    ]
}

/// The rows of the labels whose set `keep` keeps.
pub(super) fn label_set_rows(q: &Q, keep: impl Fn(LabelSet) -> bool) -> RowSet {
    let columns = label_columns();
    let records = q.get::<WireRecordSet>(());
    let tubes = q.get::<ew_derive::TubeRows>(());
    let mut set = RowSet::new(columns.clone());
    for (n, l) in q
        .get::<LabelRows>(())
        .iter()
        .enumerate()
        .filter(|(_, l)| keep(l.set))
    {
        let mut cells = vec![Cell::Empty; columns.len()];
        let mut cell = |key: &str, c: Cell| put(&columns, &mut cells, key, c);
        cell("set", words_cell(&ew_derive::text::label_set(l.set)));
        cell("for", label_for(q, l, &records, &tubes));
        for (i, key) in ["text_1", "text_2", "text_3"].into_iter().enumerate() {
            if let Some(line) = l.lines.get(i) {
                cell(key, Cell::Ident(line.clone()));
            }
        }
        cell("copies", Cell::Ident(l.copies.to_string()));
        if let Some(part) = l.marker_part.and_then(|p| q.parts().get(p)) {
            cell(
                "marker_part",
                super::parts::part_number_cell(part.item.part_number.as_str()),
            );
        }
        if !l.fits {
            cell("note", words_cell(&text!("cell-label-does-not-fit")));
        }
        let id = RowId(vec![RowKey::Index(u32::try_from(n).unwrap_or(u32::MAX))]);
        set.rows.push(Row::data(id, cells));
    }
    set
}

/// What a label is for, as the For cell prints it: "101.1 at -X1011:L1", a device's tag, "203.1 at -201U3-Q1:2".
fn label_for(
    q: &Q,
    l: &LabelRecord,
    records: &[ew_derive::WireRecord],
    tubes: &[ew_derive::TubeRow],
) -> Cell {
    match &l.target {
        LabelTarget::WireEnd(wire, end) => {
            let Some(r) = records.iter().find(|r| r.wire.id == *wire) else {
                return Cell::Empty;
            };
            let no = r.number.clone().unwrap_or_default();
            let at = cp_label(q, r.wire.ends[usize::from(*end)].cp);
            words_cell(&text!("cell-label-for", no, at))
        }
        LabelTarget::Device(d) => Cell::Ident(short_tag(q, *d)),
        LabelTarget::TubeEnd(tube, end) => {
            let Some(t) = tubes.iter().find(|t| t.id == *tube) else {
                return Cell::Empty;
            };
            let at = if *end == 0 { &t.from } else { &t.to };
            let name = ew_derive::text_in(&t.name(), None).to_owned();
            words_cell(&text!("cell-label-for", name, at.text.clone()))
        }
    }
}
