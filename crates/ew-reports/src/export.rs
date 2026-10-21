// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The row sources of the export commands `ew export cables` and `ew export bom` (doc 17 §3.3, FR-EXT-003,
//! FR-EXP-005): the rows of the project's report block of the matching kind, as a folio of that block prints
//! them, written by the one CSV writer ([`crate::csv::rows_csv`]).
//!
//! The block is the project's page-oriented block of the kind with the lowest first folio (ties by ID): the
//! cable overview (`cable_overview_diagrams`, golden folio 906) for the cable list, the summarised parts list
//! (`parts_summary`, golden folio 901) for the bill of materials. The block supplies its manual selection,
//! sort and grouping; the BOM mode is the export's, whatever the block's own `params.mode` says. A project
//! without such a block cannot export (no second row source exists).

use std::fmt;
use std::sync::Arc;

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::ReportBlock;
use ew_derive::{BomMode, Q};

use crate::csv::rows_csv;
use crate::kind::kind_name;
use crate::kinds::PartsSummary;
use crate::queries::{BlockRows, arranged};
use crate::rowset::{RowSet, RowSetStatus};

/// What an export writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Export {
    /// The cable list: the rows of the cable overview (doc 20 §10).
    Cables,
    /// The bill of materials in a mode: `Component` is the parts list (doc 20 §14.1), `Assembly` the
    /// assembly list (§14.2).
    Bom(BomMode),
}

impl Export {
    /// The report kind whose block supplies the rows.
    pub fn kind(self) -> &'static str {
        match self {
            Export::Cables => "cable_overview_diagrams",
            Export::Bom(_) => "parts_summary",
        }
    }
}

/// Why an export could not produce rows.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ExportError {
    /// The project has no page-oriented report block of the kind.
    NoBlock(&'static str),
    /// The block's row source does not derive (a kind without records yet, a query cycle).
    NotDerived(&'static str, String),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::NoBlock(kind) => write!(f, "the project has no report block of kind `{kind}`"),
            ExportError::NotDerived(kind, why) => {
                write!(f, "the rows of the `{kind}` block are not derived ({why})")
            }
        }
    }
}

impl std::error::Error for ExportError {}

/// The project's page-oriented block of `kind`: the lowest first folio, then the lowest ID.
fn block_of_kind(q: &Q, kind: &str) -> Option<(Id<ReportBlock>, Arc<ReportBlock>)> {
    q.report_blocks()
        .iter()
        .filter(|(_, b)| b.embedded.is_none())
        .filter(|(_, b)| match &b.source {
            ew_derive::ew_schema::ReportSource::Kind(k) => kind_name(k) == Some(kind),
            _ => false,
        })
        .min_by_key(|(id, b)| (b.first_folio.unwrap_or(u32::MAX), *id))
        .map(|(id, b)| (id, Arc::clone(b)))
}

fn why(set: &RowSet) -> String {
    match &set.status {
        RowSetStatus::NotDerived(reason) => reason.to_string(),
        RowSetStatus::Derived => "derived".to_owned(),
    }
}

/// The rows an export writes, after the block's arrangement.
///
/// # Errors
///
/// [`ExportError`] when the project has no block of the kind or the block does not derive.
pub fn export_rows(q: &Q, what: Export) -> Result<RowSet, ExportError> {
    let kind = what.kind();
    let (id, block) = block_of_kind(q, kind).ok_or(ExportError::NoBlock(kind))?;
    let rows = match what {
        Export::Cables => (*q.get::<BlockRows>(id)).clone(),
        Export::Bom(mode) => arranged(q, &block, PartsSummary.rows_in_mode(q, mode)),
    };
    if rows.is_derived() {
        Ok(rows)
    } else {
        Err(ExportError::NotDerived(kind, why(&rows)))
    }
}

/// The CSV text of an export and the number of rows in it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Exported {
    /// The CSV: a header line, then one line per row (a field may hold a quoted line break).
    pub csv: String,
    /// The number of rows (data and heading rows, the header excluded).
    pub rows: usize,
}

/// The CSV text of an export (`ew export … --format csv`) with its row count.
///
/// # Errors
///
/// As [`export_rows`].
pub fn export(q: &Q, what: Export) -> Result<Exported, ExportError> {
    export_rows(q, what).map(|rows| Exported {
        csv: rows_csv(q, &rows),
        rows: rows.rows.len(),
    })
}

/// The CSV text of an export (`ew export … --format csv`).
///
/// # Errors
///
/// As [`export_rows`].
pub fn export_csv(q: &Q, what: Export) -> Result<String, ExportError> {
    export(q, what).map(|e| e.csv)
}
