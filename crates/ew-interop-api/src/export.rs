// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `Exporter` trait (doc 14 §2.2, FR-EXP-001, decision log ID-1016; WP-S3.09 D14): an exporter writes a read-only
//! view of a revision to a byte sink and says what it wrote.
//!
//! The view's tables are [`RowTable`]s, plain data written for the trait here, so that a table writer behind the trait
//! needs no type of a GPL crate in its signature: `ew-reports` converts its own table model to and from it.

use std::collections::BTreeMap;
use std::fmt;

use ew_schema::{Date, Record};
use sha2::{Digest, Sha256};

use crate::ReportEntry;

/// An exporter of one output format (doc 14 §2.2). Implementations are pure: `export` reads only its view and writes
/// only to its sink.
pub trait Exporter: Send + Sync {
    /// The exporter's lower-case ID: `"csv"`, `"xlsx"`.
    fn id(&self) -> &'static str;

    /// Writes the output of `view` to `sink`.
    ///
    /// # Errors
    ///
    /// [`ExportError`] when the view cannot be written in this format or the sink refuses the bytes.
    fn export(
        &self,
        view: &ExportView<'_>,
        sink: &mut dyn std::io::Write,
    ) -> Result<ExportReport, ExportError>;
}

/// What an exporter reads: one revision's records and its tables.
#[derive(Clone, PartialEq, Debug)]
pub struct ExportView<'a> {
    /// The session revision the view was taken at.
    pub rev: u64,
    /// The revision's records an exporter reads (empty for the table writers, which read `rows` only).
    pub records: &'a [Record],
    /// The tables, in the order of their keys: a writer of several tables writes them in this order.
    pub rows: &'a BTreeMap<String, RowTable>,
    /// The export's settings.
    pub context: ExportContext,
}

/// The settings of one export.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ExportContext {
    /// The export date, when the output records one.
    pub date: Option<Date>,
    /// The output language (a BCP 47 tag, such as `en-GB`).
    pub language: String,
    /// Whether personal data is left out (NFR-PRIV).
    pub omit_personal_data: bool,
}

/// What one export wrote.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExportReport {
    /// The revision the output shows ([`ExportView::rev`]).
    pub rev: u64,
    /// The outputs written, in order.
    pub outputs: Vec<OutputMeta>,
    /// Findings about the export, in the order they were made.
    pub findings: Vec<ReportEntry>,
}

/// One output of an export.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OutputMeta {
    /// The output's name (a table's or a file's name, never a path).
    pub name: String,
    /// SHA-256 of the output's bytes.
    pub sha256: [u8; 32],
    /// The media type, such as `text/csv`.
    pub media_type: String,
}

impl OutputMeta {
    /// The output `name` of `media_type` with the SHA-256 of `bytes`.
    pub fn of(name: &str, media_type: &str, bytes: &[u8]) -> OutputMeta {
        OutputMeta {
            name: name.to_owned(),
            sha256: Sha256::digest(bytes).into(),
            media_type: media_type.to_owned(),
        }
    }
}

/// Why an export wrote nothing.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ExportError {
    /// The view cannot be written in this format; the writer's sentence says why.
    Refused(String),
    /// The sink refused the bytes.
    Write(String),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Refused(why) => f.write_str(why),
            ExportError::Write(why) => write!(f, "the output could not be written: {why}"),
        }
    }
}

impl std::error::Error for ExportError {}

/// One table of an [`ExportView`]: a name, a key and typed columns and rows, each cell as its neutral text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RowTable {
    /// The name: a sheet's name in a workbook.
    pub name: String,
    /// What the table is, such as `kind:wire_list` or `sql`.
    pub key: String,
    /// The columns, in order.
    pub columns: Vec<RowColumn>,
    /// The rows, in order.
    pub rows: Vec<RowTableRow>,
}

/// One column of a [`RowTable`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RowColumn {
    /// The key.
    pub key: String,
    /// The header label, neutral language.
    pub label: String,
    /// The value type's name, such as `Text` or `Number`.
    pub kind: String,
    /// Whether a round trip may change the column's cells.
    pub editable: bool,
}

/// One row of a [`RowTable`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RowTableRow {
    /// The row's ID text; `None` for a heading row and a row without an identity.
    pub id: Option<String>,
    /// Whether the row is a heading.
    pub heading: bool,
    /// One cell per column.
    pub cells: Vec<RowCell>,
}

/// One cell of a [`RowTableRow`]: its neutral text and its kind.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RowCell {
    /// The neutral text: a number with a `.` decimal point, a date as `yyyy-mm-dd`.
    pub text: String,
    /// The kind.
    pub kind: CellKind,
}

/// The kind of a [`RowCell`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CellKind {
    /// Text.
    Text,
    /// A number.
    Number,
    /// A date.
    Date,
}
