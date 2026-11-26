// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `labels`: every label of the workshop (doc 13 §5.3), from `ew_derive::LabelRows`, one row per label in the order the
//! fitter applies them; `tube_labels` lists the tube markers of the same engine with the same columns. A row is what it
//! labels ([`label_row_id`]), with that record as its source: a block's manual exclusion and opening a row's record read
//! it.

use ew_derive::{
    LabelColumn, LabelRecord, LabelRows, LabelSet, LabelTarget, Q, WireRecordSet, cp_label, short_tag,
};
use std::collections::BTreeMap;

use ew_ids::Id;
use ew_schema::{AnyId, Connection, ExportProfile, Part, ReportBlock, ReportKind};

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
    let records: BTreeMap<Id<Connection>, &ew_derive::WireRecord> =
        records.iter().map(|r| (r.wire.id, r)).collect();
    let tubes = q.get::<ew_derive::TubeRows>(());
    let tubes: BTreeMap<Id<Connection>, &ew_derive::TubeRow> = tubes.iter().map(|t| (t.id, t)).collect();
    let mut set = RowSet::new(columns.clone());
    for l in q.get::<LabelRows>(()).iter().filter(|l| keep(l.set)) {
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
        let (id, source) = label_row_id(l);
        let mut row = Row::data(id, cells);
        row.sources = vec![source];
        set.rows.push(row);
    }
    set
}

/// The identity of a label's row and its source: the record it labels, with the end of a wire or a tube or the level
/// of a terminal; a plate, which labels a device a device label labels too, says so.
fn label_row_id(l: &LabelRecord) -> (RowId, AnyId) {
    match &l.target {
        LabelTarget::WireEnd(c, end) | LabelTarget::TubeEnd(c, end) => {
            (RowId::entity(*c).with(RowKey::Index(u32::from(*end))), c.cast())
        }
        LabelTarget::Device(d) if l.set == LabelSet::Plates => {
            (RowId::entity(*d).with(RowKey::Name("plate".to_owned())), d.cast())
        }
        LabelTarget::Device(d) => (RowId::entity(*d), d.cast()),
        LabelTarget::TerminalLevel(d, level) => {
            (RowId::entity(*d).with(RowKey::Index(u32::from(*level))), d.cast())
        }
    }
}

/// What a label is for, as the For cell prints it: "101.1 at -X1011:L1", a device's tag, "203.1 at -201U3-Q1:2";
/// `records` and `tubes` by their IDs.
fn label_for(
    q: &Q,
    l: &LabelRecord,
    records: &BTreeMap<Id<Connection>, &ew_derive::WireRecord>,
    tubes: &BTreeMap<Id<Connection>, &ew_derive::TubeRow>,
) -> Cell {
    match &l.target {
        LabelTarget::WireEnd(wire, end) => {
            let Some(r) = records.get(wire) else {
                return Cell::Empty;
            };
            // A core has no number: its cable and core name it, as the wire list's No. does.
            let no = super::wire_list::wire_no(q, r).unwrap_or_default();
            let at = cp_label(q, r.wire.ends[usize::from(*end)].cp);
            words_cell(&text!("cell-label-for", no, at))
        }
        LabelTarget::Device(d) => Cell::Ident(short_tag(q, *d)),
        LabelTarget::TerminalLevel(d, level) => Cell::Ident(ew_derive::level_short_tag(q, *d, Some(*level))),
        LabelTarget::TubeEnd(tube, end) => {
            let Some(t) = tubes.get(tube) else {
                return Cell::Empty;
            };
            let at = if *end == 0 { &t.from } else { &t.to };
            words_cell(&text!("cell-label-for", t.name_arg(), at.arg()))
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
