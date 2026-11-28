// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A page that prints each data row on two lines, one per end of a wire (doc 13 §3.1): the wire list's page, whose
//! fitter's columns do not fit one line of the sheet. Where a block prints its kind's own columns and a printed column
//! names the column of the wire's other end ([`ColumnDef::other_end`]), the pair is one column of the page: the From
//! end's cell on the first line, the To end's under it. The rows, the CSV and the Excel file keep one row per wire with
//! every printed column; only the page's table and its pagination read this.

use ew_mltext::MlText;
use ew_schema::{ReportBlock, ReportSource};

use crate::kind::Registry;
use crate::rowset::ColumnDef;

/// The weight of the End column, which prints "From" and "To".
const END_WEIGHT: u32 = 5;

/// What a column of a page that prints a row on two lines prints on one of them.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EndLine {
    /// The row's cell of this column of the row set.
    Cell(usize),
    /// These words: the End column's "From" and "To".
    Words(MlText),
    /// Nothing.
    Empty,
}

/// One column of a page that prints each data row on two lines ([`per_end`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EndColumn {
    /// Its heading.
    pub heading: MlText,
    /// Its relative width.
    pub weight: u32,
    /// What it prints on the first line and on the second.
    pub lines: [EndLine; 2],
}

/// The columns of the page of `block` when it prints each data row on two lines, the From end above the To end (see
/// the module documentation); `None` for a block that chooses its own columns, a kind without printed columns and one
/// whose printed columns pair no two ends: its page prints a row on one line.
pub fn per_end(block: &ReportBlock) -> Option<Vec<EndColumn>> {
    if !block.columns.is_empty() {
        return None;
    }
    let ReportSource::Kind(kind) = &block.source else {
        return None;
    };
    let def = Registry::builtin().get(kind)?;
    per_end_of(&def.columns(), def.printed()?)
}

/// [`per_end`] of the columns `columns` printed by the keys `keys`, in their order: a pair as one column under the
/// heading its From column names, before the first pair the End column ("From" and "To": the first pair's own
/// headings), every other column on the first line.
pub fn per_end_of(columns: &[ColumnDef], keys: &[&str]) -> Option<Vec<EndColumn>> {
    let printed: Vec<usize> = keys
        .iter()
        .filter_map(|k| columns.iter().position(|c| c.key == *k))
        .collect();
    let partner = |i: usize| -> Option<(usize, &MlText)> {
        let (key, heading) = columns.get(i)?.other_end.as_ref()?;
        let p = printed.iter().copied().find(|p| columns[*p].key == *key)?;
        Some((p, heading))
    };
    let seconds: Vec<usize> = printed
        .iter()
        .filter_map(|i| partner(*i).map(|(p, _)| p))
        .collect();
    if seconds.is_empty() {
        return None;
    }
    let mut out: Vec<EndColumn> = Vec::new();
    for &i in printed.iter().filter(|i| !seconds.contains(i)) {
        match partner(i) {
            Some((p, heading)) => {
                if !out.iter().any(|c| matches!(c.lines[0], EndLine::Words(_))) {
                    out.push(EndColumn {
                        heading: MlText::default(),
                        weight: END_WEIGHT,
                        lines: [
                            EndLine::Words(columns[i].label.clone()),
                            EndLine::Words(columns[p].label.clone()),
                        ],
                    });
                }
                out.push(EndColumn {
                    heading: heading.clone(),
                    weight: columns[i].weight.max(columns[p].weight),
                    lines: [EndLine::Cell(i), EndLine::Cell(p)],
                });
            }
            None => out.push(EndColumn {
                heading: columns[i].label.clone(),
                weight: columns[i].weight,
                lines: [EndLine::Cell(i), EndLine::Empty],
            }),
        }
    }
    Some(out)
}
