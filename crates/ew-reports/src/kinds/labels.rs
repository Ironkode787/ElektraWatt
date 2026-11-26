// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `labels`: every label of the workshop (doc 13 §5.3), from `ew_derive::LabelRows`, one row per label in the order the
//! fitter applies them; `tube_labels` lists the tube markers of the same engine with the same columns.

use ew_derive::{
    LabelColumn, LabelRecord, LabelRows, LabelSet, LabelTarget, Q, WireRecordSet, cp_label, short_tag,
};
use ew_ids::Id;
use ew_schema::{ExportProfile, Part, ReportBlock, ReportKind};

use super::conn::{put, words_cell};
use crate::kind::ReportKindDef;
use crate::render::{RenderOptions, render};
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

/// The columns of a label list (doc 13 §5.3), one per [`LabelColumn`]: Set, For, Text 1 to 3, Copies, Marker part,
/// Note.
pub(super) fn label_columns() -> Vec<ColumnDef> {
    LabelColumn::ALL
        .into_iter()
        .map(|c| {
            let (cell, heading, weight) = match c {
                LabelColumn::Set => (CellType::Text, text!("column-set"), 12),
                LabelColumn::For => (CellType::Text, text!("column-for"), 18),
                LabelColumn::Text1 => (CellType::Ident, text!("column-text-1"), 14),
                LabelColumn::Text2 => (CellType::Ident, text!("column-text-2"), 14),
                LabelColumn::Text3 => (CellType::Ident, text!("column-text-3"), 14),
                LabelColumn::Copies => (CellType::Ident, text!("column-copies"), 6),
                LabelColumn::MarkerPart => (CellType::Ident, text!("column-marker-part"), 12),
                LabelColumn::Note => (CellType::Text, text!("column-note"), 14),
            };
            ColumnDef::new(c.key(), cell, heading).weighted(weight)
        })
        .collect()
}

/// The heading of the labels list's column `c` in the language `lang` (`None`: English), as a form names it.
pub fn label_column_words(c: LabelColumn, lang: Option<&ew_mltext::LangTag>) -> String {
    label_columns()
        .into_iter()
        .find(|d| d.key == c.key())
        .map(|d| ew_derive::text_in(&d.label, lang).to_owned())
        .unwrap_or_default()
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
        cell(
            LabelColumn::Set.key(),
            words_cell(&ew_derive::text::label_set(l.set)),
        );
        cell(LabelColumn::For.key(), label_for(q, l, &records, &tubes));
        for (i, c) in [LabelColumn::Text1, LabelColumn::Text2, LabelColumn::Text3]
            .into_iter()
            .enumerate()
        {
            if let Some(line) = l.lines.get(i) {
                cell(c.key(), Cell::Ident(line.clone()));
            }
        }
        cell(LabelColumn::Copies.key(), Cell::Ident(l.copies.to_string()));
        if let Some(part) = l.marker_part.and_then(|p| q.parts().get(p)) {
            cell(
                LabelColumn::MarkerPart.key(),
                super::parts::part_number_cell(part.item.part_number.as_str()),
            );
        }
        if !l.fits {
            cell(
                LabelColumn::Note.key(),
                words_cell(&text!("cell-label-does-not-fit")),
            );
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

/// One label file (doc 13 §5.4): the labels of one set printed on one marker part, in the export profile's columns.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LabelFile {
    /// The label set.
    pub set: LabelSet,
    /// The marker part they are printed on; `None`: the stock has none for the set.
    pub marker_part: Option<Id<Part>>,
    /// The profile's headings.
    pub header: Vec<String>,
    /// One row per label, its cells in the profile's columns as the list prints them in the project's language.
    pub rows: Vec<Vec<String>>,
}

/// The label files of the labels `keep` keeps, one per label set and marker part, in the order of their first label;
/// a profile column whose key the `labels` list does not have stays empty.
pub fn label_files(q: &Q, profile: &ExportProfile, keep: impl Fn(&LabelRecord) -> bool) -> Vec<LabelFile> {
    let set = label_set_rows(q, |_| true);
    let options = RenderOptions {
        lang: ew_derive::print_language(q),
        ..RenderOptions::default()
    };
    let at: Vec<Option<usize>> = profile
        .columns
        .iter()
        .map(|c| set.columns.iter().position(|d| d.key == c.key))
        .collect();
    let header: Vec<String> = profile.columns.iter().map(|c| c.heading.clone()).collect();
    let mut files: Vec<LabelFile> = Vec::new();
    for (label, row) in q.get::<LabelRows>(()).iter().zip(&set.rows) {
        if !keep(label) {
            continue;
        }
        let cells = at
            .iter()
            .map(|i| i.map(|i| render(q, &row.cells[i], &options)).unwrap_or_default())
            .collect();
        match files
            .iter_mut()
            .find(|f| f.set == label.set && f.marker_part == label.marker_part)
        {
            Some(f) => f.rows.push(cells),
            None => files.push(LabelFile {
                set: label.set,
                marker_part: label.marker_part,
                header: header.clone(),
                rows: vec![cells],
            }),
        }
    }
    files
}
