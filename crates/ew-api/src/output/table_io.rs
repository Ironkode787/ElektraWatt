// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The table round trip (WP-S3.08 part 2; D8, D9, D11; decision log ID-1174): a navigator table as a [`Table`]
//! ([`Session::table_export`]), and a round-trip file read back against the project's table of the same key
//! ([`Session::table_import`]), its changed cells compiled by table mode's own rule ([`super::table_edit::compile`]) into
//! one `fields.set_many` ([`Session::apply_table_import`]).
//!
//! **Tables and keys** (D8, ID-1174 ruling 4): `nav:<word>` for the twelve table navigators ([`TABLE_NAVIGATORS`]),
//! `nav:strips/<strip uuid>` for one strip's rows; the report tables of `ew export` (`block:<uuid>`, `kind:<name>`,
//! `kind:parts_summary/<mode>`); `sql` is never imported. A navigator table is every table row in node order with every
//! column; its rows' IDs are the node keys.
//!
//! **What is editable** (D9): a navigator cell exactly when table mode's [`cell_reason_key`] finds none, except that a
//! strip-editor cell and a potential's defining cell are "edit in the window"; `io_list`'s six columns of
//! [`IO_LIST_ROUTES`] on a channel's row; every other report cell is "derived".
//!
//! **The merge** (D11): the file's rows match the table's by `id` (`kind:io_list` also by station, slot and channel);
//! with an intact `_ew_base` each cell is three-way (only the file changed it: applied; only EW: kept, EW-IMP-302; both,
//! differently: EW-IMP-304, EW's value unless `take_file`), else two-way against the current value. A read-only cell
//! edited is EW-IMP-301, an unknown ID EW-IMP-303, a number a spreadsheet wrote for a text (`1.10` read `1.1`)
//! EW-IMP-305. Each applied cell is parsed as the grid parses it ([`Session::parse_field_input`]). The import is pure
//! over the bytes and the session: it reads no file, clock or network, and creates or deletes no record (D13).

use std::collections::BTreeMap;
use std::fmt;

use ew_derive::DeviceTag;
use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::Device;
use ew_import_catalog::SourceFormat;
use ew_import_catalog::ew_interop_api::ImportError;
use ew_import_catalog::sheet::{read_rows, read_sheet, sheet_names};
use ew_reports::csv::Preset;
use ew_reports::table::{Table, TableCell, TableColumn, TableRow};
use ew_reports::xlsx::{BASE_DIGEST_ROW, BASE_SHEET, EW_FORMAT, EW_SHEET, rows_text};
use ew_reports::{CellType, RowId, RowKey};
use serde_json::{Value, json};

use super::navigators::{NavKind, NavNode, NavRole, NavTable, Uuid};
use super::rows::{RowsError, sha256_hex};
use super::table_edit::{
    CellValue, Gesture, GestureCell, GestureKind, MemberBase, cell_reason_key, compile, define_point,
    mark_conflicts, potential_input, row_targets, strip_route,
};
use super::{InputError, RowValue};
use crate::{AnyCommand, CmdError, Receipt, Session, Submit};

/// The navigators whose tables a round trip exports (D8, ID-1174 ruling 4), in [`NavKind::ALL`]'s order: not the
/// macros, libraries or messages.
pub const TABLE_NAVIGATORS: [NavKind; 12] = [
    NavKind::Pages,
    NavKind::Devices,
    NavKind::Connections,
    NavKind::Strips,
    NavKind::Cables,
    NavKind::Plc,
    NavKind::Potentials,
    NavKind::Ips,
    NavKind::Structure,
    NavKind::Parts,
    NavKind::Blocks,
    NavKind::Unplaced,
];

/// The editable columns of `io_list` (D9): (column key, route kind, route path), the PLC navigator's routes, the target
/// being the row's channel.
pub const IO_LIST_ROUTES: [(&str, &str, &str); 6] = [
    ("designation", "channel", "designation"),
    ("address", "channel", "address"),
    ("symbol", "channel", "symbol"),
    ("state", "channel", "state"),
    ("target", "channel", "target"),
    ("comment", "channel", "comment"),
];

/// The most changed cells one import applies (`fields.set_many`'s bound).
pub const MAX_IMPORT_CELLS: usize = ew_commands::commands::SET_MANY_MAX_EDITS;

/// Why a table cannot be named or built (D8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TableError {
    /// No table navigator has this word.
    UnknownNavigator(String),
    /// A strip given for another navigator than `strips`.
    StripElsewhere,
    /// No strip of the Terminal strips navigator has this tag (the tags there are).
    UnknownStrip(String, Vec<String>),
    /// More than one strip prints this tag (their full tags).
    AmbiguousStrip(String, Vec<String>),
    /// The key names no table (the key).
    UnknownTable(String),
    /// `sql`: an `ew sql` result is never imported (D8).
    Sql,
    /// The rows of a report table do not derive (the report layer's sentence).
    Rows(String),
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableError::UnknownNavigator(word) => {
                let words: Vec<&str> = TABLE_NAVIGATORS.iter().map(|k| k.word()).collect();
                write!(
                    f,
                    "unknown navigator `{word}` (the table navigators are: {})",
                    words.join(", ")
                )
            }
            TableError::StripElsewhere => f.write_str("`--strip` applies to `export table strips`"),
            TableError::UnknownStrip(tag, tags) => write!(
                f,
                "no terminal strip `{tag}` (the strips are: {})",
                tags.join(", ")
            ),
            TableError::AmbiguousStrip(tag, fulls) => write!(
                f,
                "more than one terminal strip has the tag `{tag}` ({}); name it by its full tag",
                fulls.join(", ")
            ),
            TableError::UnknownTable(key) => write!(
                f,
                "no table `{key}`: a table is `nav:<navigator>`, `nav:strips/<strip>`, `block:<block>` or \
                 `kind:<kind>`, as `ew export` names it"
            ),
            TableError::Sql => f.write_str(
                "an `ew sql` result is never imported: import a table that `ew export table` or `ew export report` \
                 wrote",
            ),
            TableError::Rows(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for TableError {}

impl From<RowsError> for TableError {
    fn from(e: RowsError) -> TableError {
        TableError::Rows(e.to_string())
    }
}

/// How a round-trip file is read (D11).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ImportTableOptions {
    /// The table's key (`--table`): needed for a CSV and for a workbook whose `_ew` names none.
    pub table: Option<String>,
    /// The CSV preset the file was written in (`--preset`): `rfc4180` keeps a leading apostrophe; `excel-nl` reads a
    /// decimal comma in a number column. `None`: `excel-nl` for a `;`-separated file, else `excel-en`.
    pub preset: Option<Preset>,
    /// `--take-file`: a cell changed in both takes the file's value (EW-IMP-304).
    pub take_file: bool,
}

/// Why a round-trip file is refused as a whole (D11).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TableImportError {
    /// The reader refused the bytes.
    Read(String),
    /// A CSV without `--table`.
    CsvNoTable,
    /// A workbook whose `_ew` names no table, without `--table`.
    NoTable,
    /// The `_ew` sheet is of another format (its `format`).
    Format(String),
    /// The file comes from another project (its project's ID).
    OtherProject(String),
    /// The table cannot be built.
    Table(TableError),
    /// The file has no `id` column, and the table is not `kind:io_list`.
    NoIdColumn,
    /// More changed cells than one import applies (their number).
    TooManyCells(usize),
    /// The read panicked on the file named (WP-S6.04 D15, [`guard_table_read`]).
    Panicked(String),
}

impl fmt::Display for TableImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableImportError::Read(why) => write!(f, "the file cannot be read: {why}"),
            TableImportError::CsvNoTable => f.write_str("a CSV names no table: give `--table <key>`"),
            TableImportError::NoTable => f.write_str("the workbook names no table: give `--table <key>`"),
            TableImportError::Format(format) => write!(
                f,
                "the workbook is of the format `{format}`, not `{EW_FORMAT}`: export the table again"
            ),
            TableImportError::OtherProject(id) => {
                write!(f, "the file was exported from another project ({id})")
            }
            TableImportError::Table(e) => e.fmt(f),
            TableImportError::NoIdColumn => f.write_str(
                "the file has no `id` column, by which its rows are matched: export the table as XLSX, or as CSV \
                 with `--preset excel-nl`, `excel-en` or `rfc4180`, and keep the column",
            ),
            TableImportError::TooManyCells(n) => write!(
                f,
                "{n} changed cells are more than the 10,000 one import applies; split the file"
            ),
            TableImportError::Panicked(file) => write!(
                f,
                "The table import stopped on an internal error while reading {file}; nothing was changed."
            ),
        }
    }
}

impl std::error::Error for TableImportError {}

/// Runs `read`, a table round trip's read of the file `file` ([`Session::table_import`]), inside `catch_unwind`
/// (WP-S6.04 D15, NFR-REL-12): a panic becomes [`TableImportError::Panicked`] with the file's name. The read is pure
/// over the bytes and the session (`&self`), so a panic leaves the model and the files as they were. The window and
/// `ew import table` call the read through it.
///
/// # Errors
///
/// `read`'s [`TableImportError`], or [`TableImportError::Panicked`].
pub fn guard_table_read<T>(
    file: &str,
    read: impl FnOnce() -> Result<T, TableImportError>,
) -> Result<T, TableImportError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(read))
        .unwrap_or_else(|_| Err(TableImportError::Panicked(file.to_owned())))
}

/// What became of one cell of the file (D11).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellOutcome {
    /// Changed only in the file: applied.
    Applied,
    /// Changed only in EW since the export: EW's value kept (EW-IMP-302).
    Kept,
    /// Changed in both to different values (EW-IMP-304): EW's value kept, or the file's taken with `take_file`.
    Conflict {
        /// Whether the file's value is taken.
        taken: bool,
    },
    /// A read-only cell edited (EW-IMP-301): not applied.
    ReadOnly,
    /// The file's value does not parse, or sets a record another row of the file sets differently: not applied.
    Refused,
}

impl CellOutcome {
    /// The outcome's word (`applied`, `kept`, `conflict`, `taken`, `read-only`, `refused`).
    pub fn word(self) -> &'static str {
        match self {
            CellOutcome::Applied => "applied",
            CellOutcome::Kept => "kept",
            CellOutcome::Conflict { taken: false } => "conflict",
            CellOutcome::Conflict { taken: true } => "taken",
            CellOutcome::ReadOnly => "read-only",
            CellOutcome::Refused => "refused",
        }
    }

    /// Whether the cell goes into the command.
    pub fn applies(self) -> bool {
        matches!(self, CellOutcome::Applied | CellOutcome::Conflict { taken: true })
    }
}

/// One cell of the preview: changed in the file, in EW, or both (D11).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportCell {
    /// The row's ID.
    pub row: String,
    /// The row's label (`-X1031:3`).
    pub row_label: String,
    /// The column's key.
    pub column: String,
    /// The column's label.
    pub column_label: String,
    /// The value as exported (`_ew_base`), `None` without an intact base.
    pub base: Option<String>,
    /// The project's value now.
    pub ew: String,
    /// The file's value.
    pub file: String,
    /// What became of it.
    pub outcome: CellOutcome,
    /// Why a read-only or refused cell is not applied: the read-only reason's words, the parser's text, or the row of
    /// the file that sets the same record differently.
    pub reason: Option<String>,
}

/// One finding of a round trip (doc 15, EW-IMP-301 to -305).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportFinding {
    /// The rule (`EW-IMP-301`).
    pub rule: &'static str,
    /// The row's ID.
    pub row: String,
    /// The column's key, for a cell's finding.
    pub column: Option<String>,
    /// The text key of a read-only cell's reason (EW-IMP-301), for a front-end's words.
    pub reason_key: Option<&'static str>,
    /// The finding as a sentence.
    pub message: String,
}

/// The counts of a round trip's preview (the window's card, part 3).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ImportCounts {
    /// Cells applied (a conflict taken from the file included).
    pub changed: usize,
    /// Cells kept because only EW changed them.
    pub kept: usize,
    /// Cells changed in both.
    pub conflicts: usize,
    /// Cells refused: read-only, unparsable or clashing.
    pub refused: usize,
    /// Rows of the file with an unknown ID.
    pub unknown: usize,
    /// Rows of the table missing from the file.
    pub missing: usize,
}

/// A round-trip file read against the project (D11): the preview and the compiled command.
#[derive(Clone, PartialEq, Debug)]
pub struct TableImport {
    /// The table's key.
    pub table: String,
    /// The session revision it was read against.
    pub rev: u64,
    /// Whether the cells were compared three-way (an intact `_ew_base`).
    pub three_way: bool,
    /// Sentences about the file as a whole (an edited `_ew_base`).
    pub notices: Vec<String>,
    /// The cells, in file row order, then column order.
    pub cells: Vec<ImportCell>,
    /// The findings, in file row order.
    pub findings: Vec<ImportFinding>,
    /// The labels of the table's rows that the file does not have (listed, never deleted).
    pub missing: Vec<String>,
    /// The one `fields.set_many` of the applied cells; `None` when no cell applies.
    pub command: Option<AnyCommand>,
}

impl TableImport {
    /// The preview's counts.
    pub fn counts(&self) -> ImportCounts {
        let mut c = ImportCounts {
            unknown: self.findings.iter().filter(|f| f.rule == "EW-IMP-303").count(),
            missing: self.missing.len(),
            ..ImportCounts::default()
        };
        for cell in &self.cells {
            match cell.outcome {
                CellOutcome::Applied => c.changed += 1,
                CellOutcome::Kept => c.kept += 1,
                CellOutcome::Conflict { taken } => {
                    c.conflicts += 1;
                    if taken {
                        c.changed += 1;
                    }
                }
                CellOutcome::ReadOnly | CellOutcome::Refused => c.refused += 1,
            }
        }
        c
    }

    /// The command as a command log (`.ewcmd`, doc 17 §3.5) of one line, which `ew apply` replays; `None` without one.
    pub fn command_log(&self) -> Option<String> {
        let cmd = self.command.as_ref()?;
        let header =
            json!({"$schema": crate::CMDLOG_SCHEMA, "label": format!("ew import table {}", self.table)});
        let line = json!({"cmd": cmd.name, "params": cmd.params});
        Some(format!("{header}\n{line}\n"))
    }
}

/// Why a round trip's command was not committed.
#[derive(Clone, PartialEq, Debug)]
pub enum TableApplyError {
    /// The project changed since the file was read (the revision then and now).
    Stale(u64, u64),
    /// The command failed.
    Command(CmdError),
}

impl fmt::Display for TableApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableApplyError::Stale(..) => {
                f.write_str("the project changed since the file was read: import the file again")
            }
            TableApplyError::Command(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for TableApplyError {}

/// A table's key, parsed (D8).
enum TableKey {
    Nav(NavKind),
    NavStrip(Uuid),
    Report,
}

fn parse_key(key: &str) -> Result<TableKey, TableError> {
    if key == "sql" {
        return Err(TableError::Sql);
    }
    if let Some(rest) = key.strip_prefix("nav:") {
        if let Some(strip) = rest.strip_prefix("strips/") {
            return strip
                .parse::<Uuid>()
                .map(TableKey::NavStrip)
                .map_err(|_| TableError::UnknownTable(key.to_owned()));
        }
        return NavKind::from_word(rest)
            .filter(|k| TABLE_NAVIGATORS.contains(k))
            .map(TableKey::Nav)
            .ok_or_else(|| TableError::UnknownNavigator(rest.to_owned()));
    }
    if key.starts_with("block:") || key.starts_with("kind:") {
        return Ok(TableKey::Report);
    }
    Err(TableError::UnknownTable(key.to_owned()))
}

/// A table with what the merge reads beside it: for a navigator table its [`NavTable`] and, per row, the node.
struct Current {
    table: Table,
    nav: Option<NavTable>,
    nodes: Vec<u32>,
}

impl Session {
    /// The key of a navigator's table (D8): `nav:<word>`, or with `strip` (a printed or full tag) `nav:strips/<uuid>`.
    ///
    /// # Errors
    ///
    /// [`TableError::UnknownNavigator`] for a word that is not a table navigator's; [`TableError::StripElsewhere`] for
    /// a strip of another navigator; [`TableError::UnknownStrip`] or
    /// [`TableError::AmbiguousStrip`] for a strip tag that names no strip or two.
    pub fn navigator_table_key(&self, word: &str, strip: Option<&str>) -> Result<String, TableError> {
        let kind = NavKind::from_word(word)
            .filter(|k| TABLE_NAVIGATORS.contains(k))
            .ok_or_else(|| TableError::UnknownNavigator(word.to_owned()))?;
        let Some(tag) = strip else {
            return Ok(format!("nav:{}", kind.word()));
        };
        if kind != NavKind::Strips {
            return Err(TableError::StripElsewhere);
        }
        let table = self.navigator_table(NavKind::Strips);
        let q = self.query();
        let strips: Vec<(String, String, Uuid)> = table
            .nodes
            .iter()
            .filter(|n| n.role == NavRole::Strip)
            .filter_map(|n| {
                let id = *n.records.first()?;
                let full = q
                    .get::<DeviceTag>(Id::<Device>::from_uuid(id))
                    .as_ref()
                    .as_ref()
                    .map(|t| t.full.clone())
                    .unwrap_or_default();
                Some((n.label.clone(), full, id))
            })
            .collect();
        let tags: Vec<(String, String)> = strips.iter().map(|(s, f, _)| (s.clone(), f.clone())).collect();
        match super::rows::pick_strip(tag, &tags) {
            Ok(Some(i)) => Ok(format!("nav:strips/{}", strips[i].2)),
            Ok(None) => Err(TableError::UnknownStrip(
                tag.to_owned(),
                strips.into_iter().map(|(s, _, _)| s).collect(),
            )),
            Err(RowsError::AmbiguousStrip(tag, fulls)) => Err(TableError::AmbiguousStrip(tag, fulls)),
            Err(e) => Err(e.into()),
        }
    }

    /// The table of `key` (D8): a navigator's table rows in node order with every column (a column is editable when
    /// it has a `fields.set` route), or a report table as `ew export` writes it (`io_list`'s round-trip columns
    /// editable, D9).
    ///
    /// # Errors
    ///
    /// [`TableError`] for a key that names no table, `sql`, or report rows that do not derive.
    pub fn table_export(&self, key: &str) -> Result<Table, TableError> {
        self.current_table(key).map(|c| c.table)
    }

    fn current_table(&self, key: &str) -> Result<Current, TableError> {
        let (kind, strip) = match parse_key(key)? {
            TableKey::Report => {
                let table = super::rows::report_table(&self.query(), key)?;
                return Ok(Current {
                    table,
                    nav: None,
                    nodes: Vec::new(),
                });
            }
            TableKey::Nav(kind) => (kind, None),
            TableKey::NavStrip(strip) => (NavKind::Strips, Some(strip)),
        };
        let nav = self.navigator_table(kind);
        let strip_node = match strip {
            None => None,
            Some(id) => Some(
                nav.nodes
                    .iter()
                    .position(|n| n.role == NavRole::Strip && n.records.first() == Some(&id))
                    .ok_or_else(|| TableError::UnknownTable(key.to_owned()))?,
            ),
        };
        let nodes: Vec<u32> = (0..nav.nodes.len())
            .filter(|&i| {
                let n = &nav.nodes[i];
                n.table_row && strip_node.is_none_or(|s| n.parent == u32::try_from(s).ok())
            })
            .map(|i| u32::try_from(i).unwrap_or(u32::MAX))
            .collect();
        let name = match strip_node {
            Some(s) => nav.nodes[s].label.clone(),
            None => kind.word().to_owned(),
        };
        let table = Table {
            name,
            key: key.to_owned(),
            columns: nav
                .columns
                .iter()
                .map(|c| TableColumn {
                    key: c.key.clone(),
                    label: c.label.clone(),
                    cell: CellType::Text,
                    editable: !c.derived && c.route.is_some(),
                })
                .collect(),
            rows: nodes
                .iter()
                .map(|&i| {
                    let n = &nav.nodes[i as usize];
                    TableRow {
                        id: Some(n.key.clone()),
                        heading: false,
                        cells: (0..nav.columns.len())
                            .map(|c| TableCell::Text(n.cells.get(c).cloned().unwrap_or_default()))
                            .collect(),
                    }
                })
                .collect(),
        };
        Ok(Current {
            table,
            nav: Some(nav),
            nodes,
        })
    }

    /// Reads a round-trip file against the project's table of the same key (D11): the preview of every changed cell
    /// with its outcome, the findings EW-IMP-301 to -305, the table's rows missing from the file, and the one
    /// `fields.set_many` of the applied cells. Nothing is committed ([`Session::apply_table_import`] does).
    ///
    /// # Errors
    ///
    /// [`TableImportError`]: unreadable bytes, a CSV or a workbook without a table, a workbook of another format or
    /// project, a table that cannot be built, a file without its `id` column, or more changed cells than one import
    /// applies.
    pub fn table_import(
        &self,
        bytes: &[u8],
        options: &ImportTableOptions,
    ) -> Result<TableImport, TableImportError> {
        let limits = ew_import_catalog::table_limits();
        let read = |e: ImportError| TableImportError::Read(e.to_string());
        // The reader's size bound first, before any other refusal reads the bytes (RT-6).
        if bytes.len() as u64 > limits.max_input_bytes {
            return Err(read(ImportError::LimitExceeded {
                what: "input size".into(),
                limit: limits.max_input_bytes,
            }));
        }
        let format = SourceFormat::detect(bytes);
        let mut notices = Vec::new();
        let (key, sheet, base) = match format {
            SourceFormat::Csv => {
                let key = options.table.clone().ok_or(TableImportError::CsvNoTable)?;
                (key, read_sheet(bytes, format, None, &limits).map_err(read)?, None)
            }
            SourceFormat::Xlsx => {
                let names = sheet_names(bytes, &limits).map_err(read)?;
                let ew: Vec<Vec<String>> = if names.iter().any(|n| n == EW_SHEET) {
                    read_rows(bytes, format, EW_SHEET, &limits).map_err(read)?
                } else {
                    Vec::new()
                };
                let value = |k: &str| {
                    ew.iter()
                        .find(|r| r.first().is_some_and(|c| c == k))
                        .and_then(|r| r.get(1).cloned())
                };
                if let Some(f) = value("format").filter(|f| f != EW_FORMAT) {
                    return Err(TableImportError::Format(f));
                }
                let project = self.query().project().id.to_string();
                if let Some(p) = value("project").filter(|p| *p != project) {
                    return Err(TableImportError::OtherProject(p));
                }
                let key = value("table")
                    .filter(|t| !t.is_empty())
                    .or_else(|| options.table.clone())
                    .ok_or(TableImportError::NoTable)?;
                let first = names.first().map(String::as_str);
                let sheet = read_sheet(bytes, format, first, &limits).map_err(read)?;
                let base = if names.iter().any(|n| n == BASE_SHEET) {
                    let rows = read_rows(bytes, format, BASE_SHEET, &limits).map_err(read)?;
                    let base = intact_base(&rows);
                    if base.is_none() {
                        notices.push(
                            "the `_ew_base` sheet of the workbook was changed, so each cell is compared with the \
                             project's value only (two-way)"
                                .to_owned(),
                        );
                    }
                    base
                } else {
                    None
                };
                (key, sheet, base)
            }
        };
        let current = self.current_table(&key).map_err(TableImportError::Table)?;
        let preset = match (format, options.preset) {
            (SourceFormat::Xlsx, _) => None,
            (SourceFormat::Csv, Some(p)) => Some(p),
            (SourceFormat::Csv, None) => Some(if semicolons(bytes) {
                Preset::ExcelNl
            } else {
                Preset::ExcelEn
            }),
        };
        let merge = Merge {
            session: self,
            current: &current,
            key: &key,
            base: base.as_ref(),
            preset,
            take_file: options.take_file,
        };
        let mut import = merge.run(&sheet)?;
        import.notices = notices;
        Ok(import)
    }

    /// Commits a round trip's command (D11): one `fields.set_many`, one transaction and one undo step; `None` when no
    /// cell applies.
    ///
    /// # Errors
    ///
    /// [`TableApplyError::Stale`] when the project changed since the file was read; [`TableApplyError::Command`] when
    /// the command fails.
    pub fn apply_table_import(
        &mut self,
        import: &TableImport,
        submit: Submit,
    ) -> Result<Option<Receipt>, TableApplyError> {
        let now = self.world().rev.0;
        if now != import.rev {
            return Err(TableApplyError::Stale(import.rev, now));
        }
        let Some(cmd) = &import.command else {
            return Ok(None);
        };
        let preview = self.preview_with(cmd, submit).map_err(TableApplyError::Command)?;
        self.commit(preview).map(Some).map_err(TableApplyError::Command)
    }

    /// A target's stored member and the project's source language, for a composed leaf or a language text (one
    /// property sheet of that record, as the grid reads it).
    fn member_base(&self, kind: &str, target: Uuid, member: &str) -> MemberBase {
        let sheet = self.property_sheet(&[target]);
        let key = format!("{kind}/{member}");
        let value = sheet
            .sections
            .iter()
            .find_map(|s| s.rows.iter().find(|r| r.key == key))
            .and_then(|r| match &r.value {
                RowValue::One { json, .. } => Some(json.clone()),
                _ => None,
            });
        let language = sheet
            .languages
            .first()
            .cloned()
            .or_else(|| match &value {
                Some(Value::Object(o)) => o.keys().next().cloned(),
                _ => None,
            })
            .unwrap_or_else(|| "en".to_owned());
        MemberBase { value, language }
    }
}

/// Whether the header line of a CSV has more semicolons than commas outside quotes (the reader's separator rule).
fn semicolons(bytes: &[u8]) -> bool {
    let (mut commas, mut semis, mut quoted) = (0usize, 0usize, false);
    for &b in bytes {
        match b {
            b'"' => quoted = !quoted,
            b',' if !quoted => commas += 1,
            b';' if !quoted => semis += 1,
            b'\n' | b'\r' if !quoted => break,
            _ => {}
        }
    }
    semis > commas
}

/// The base of an `_ew_base` sheet read whole: its header's column keys, and per row ID the cells under them; `None`
/// when the digest row is missing or the digest does not match the rows (an edited base).
#[derive(Clone, PartialEq, Eq, Debug)]
struct Base {
    columns: Vec<String>,
    rows: BTreeMap<String, Vec<String>>,
}

fn intact_base(rows: &[Vec<String>]) -> Option<Base> {
    let (last, body) = rows.split_last()?;
    if last.first().map(String::as_str) != Some(BASE_DIGEST_ROW) {
        return None;
    }
    let digest = last.get(1)?;
    let header = body.first()?;
    let width = header.len();
    // A row read back ends at its last cell: pad it to the header's width, as the writer's text has every cell.
    let padded: Vec<Vec<String>> = body
        .iter()
        .map(|r| {
            let mut r = r.clone();
            r.resize(width.max(r.len()), String::new());
            r
        })
        .collect();
    if sha256_hex(rows_text(&padded).as_bytes()) != *digest {
        return None;
    }
    Some(Base {
        columns: header.iter().skip(1).cloned().collect(),
        rows: padded
            .iter()
            .skip(1)
            .filter_map(|r| Some((r.first()?.clone(), r.iter().skip(1).cloned().collect())))
            .collect(),
    })
}

/// The merge of one file against one table (D11).
struct Merge<'a> {
    session: &'a Session,
    current: &'a Current,
    key: &'a str,
    base: Option<&'a Base>,
    /// The CSV preset; `None` for a workbook.
    preset: Option<Preset>,
    take_file: bool,
}

/// One cell the merge compiles: its preview index and the gesture cell.
struct Pending {
    at: usize,
    cell: GestureCell,
}

impl Merge<'_> {
    fn run(&self, sheet: &ew_import_catalog::sheet::SheetTable) -> Result<TableImport, TableImportError> {
        let table = &self.current.table;
        let io_list = self.key == "kind:io_list";
        let id_col = sheet.headers.iter().position(|h| h == "id");
        if id_col.is_none() && !io_list {
            return Err(TableImportError::NoIdColumn);
        }
        // Each file column to the table's column of its label (the k-th one of a label the table has twice).
        let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
        let columns: Vec<Option<usize>> = sheet
            .headers
            .iter()
            .enumerate()
            .map(|(i, h)| {
                if Some(i) == id_col {
                    return None;
                }
                let k = seen.entry(h.as_str()).or_insert(0);
                let found = table
                    .columns
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.label == *h)
                    .nth(*k)
                    .map(|(c, _)| c);
                *k += 1;
                found
            })
            .collect();
        let by_id: BTreeMap<&str, usize> = table
            .rows
            .iter()
            .enumerate()
            .filter_map(|(i, r)| Some((r.id.as_deref()?, i)))
            .collect();
        let point_of = |cells: &dyn Fn(&str) -> Option<String>| -> Option<(String, String, String)> {
            Some((cells("station")?, cells("slot")?, cells("channel")?))
        };
        let by_point: BTreeMap<(String, String, String), usize> = if io_list {
            let col = |k: &str| table.columns.iter().position(|c| c.key == k);
            table
                .rows
                .iter()
                .enumerate()
                .filter_map(|(i, r)| {
                    let get = |k: &str| -> Option<String> { Some(r.cells.get(col(k)?)?.text().to_owned()) };
                    Some((point_of(&get)?, i))
                })
                .collect()
        } else {
            BTreeMap::new()
        };
        let mut import = TableImport {
            table: self.key.to_owned(),
            rev: self.session.world().rev.0,
            three_way: self.base.is_some(),
            notices: Vec::new(),
            cells: Vec::new(),
            findings: Vec::new(),
            missing: Vec::new(),
            command: None,
        };
        let mut matched = vec![false; table.rows.len()];
        let mut pending: Vec<Pending> = Vec::new();
        for (n, file_row) in sheet.rows.iter().enumerate() {
            let id = id_col.map_or("", |c| file_row.get(c).map_or("", |s| s.trim()));
            let row = if id.is_empty() {
                // A heading row; for `kind:io_list` a row matched by (station, slot, channel).
                let get = |k: &str| -> Option<String> {
                    let c = table.columns.iter().position(|x| x.key == k)?;
                    let f = columns.iter().position(|x| *x == Some(c))?;
                    Some(self.text(file_row.get(f).map_or("", String::as_str), &table.columns[c]))
                };
                match point_of(&get).and_then(|p| by_point.get(&p).copied()) {
                    Some(r) if io_list => r,
                    _ => continue,
                }
            } else {
                match by_id.get(id) {
                    Some(r) => *r,
                    None => {
                        import.findings.push(ImportFinding {
                            rule: "EW-IMP-303",
                            row: id.to_owned(),
                            column: None,
                            reason_key: None,
                            message: format!(
                                "the row {id} of the file is not in the project's table (deleted since the export?): \
                                 nothing of it is imported"
                            ),
                        });
                        continue;
                    }
                }
            };
            matched[row] = true;
            for (f, c) in columns.iter().enumerate() {
                let Some(c) = *c else { continue };
                let raw = file_row.get(f).map_or("", String::as_str);
                self.cell(&mut import, &mut pending, n + 1, row, c, raw);
            }
        }
        import.missing = (0..table.rows.len())
            .filter(|&r| !matched[r] && !table.rows[r].heading && table.rows[r].id.is_some())
            .map(|r| self.row_label(r))
            .collect();
        self.compile(&mut import, pending)?;
        Ok(import)
    }

    /// The file's text of a cell in `column` as the table writes it: a CSV's guard apostrophe dropped (not in
    /// `rfc4180`), an `excel-nl` decimal comma read as a point in a number column.
    fn text(&self, raw: &str, column: &TableColumn) -> String {
        let Some(preset) = self.preset else {
            return raw.to_owned();
        };
        let mut t = raw.to_owned();
        if preset != Preset::Rfc4180
            && let Some(rest) = raw.strip_prefix('\'')
            && rest.starts_with(['=', '+', '-', '@', '\t', '\r'])
        {
            t = rest.to_owned();
        }
        if preset == Preset::ExcelNl && matches!(column.cell, CellType::Number | CellType::Qty) {
            t = t.replace(',', ".");
        }
        t
    }

    /// The label of table row `r`: a terminal's `<strip>:<terminal>`, another navigator row's label, an I/O row's
    /// module and channel, another report row's first cell.
    fn row_label(&self, r: usize) -> String {
        let table = &self.current.table;
        let row = &table.rows[r];
        if let (Some(nav), Some(&i)) = (&self.current.nav, self.current.nodes.get(r)) {
            let node = &nav.nodes[i as usize];
            if node.role == NavRole::Terminal {
                let strip = node.cells.first().map_or("", String::as_str);
                return format!("{strip}:{}", node.label);
            }
            return node.label.clone();
        }
        let cell = |k: &str| {
            table
                .columns
                .iter()
                .position(|c| c.key == k)
                .and_then(|c| row.cells.get(c))
                .map_or("", TableCell::text)
        };
        if self.key == "kind:io_list" {
            return format!("{} channel {}", cell("module"), cell("channel"));
        }
        row.cells.first().map_or("", TableCell::text).to_owned()
    }

    /// The route and targets of an editable cell (row `r`, column `c`), or the text key of why it is read-only in a
    /// round trip (D9).
    fn route(&self, r: usize, c: usize) -> Result<((String, String), Vec<Uuid>), &'static str> {
        let table = &self.current.table;
        if let (Some(nav), Some(&i)) = (&self.current.nav, self.current.nodes.get(r)) {
            let node: &NavNode = &nav.nodes[i as usize];
            if strip_route(nav, node, c).is_some() {
                return Err(ew_commands::text_key!("table-reason-window"));
            }
            if let Some(reason) = cell_reason_key(nav, node, c) {
                return Err(reason);
            }
            let Some((kind, path)) = nav.columns[c].route.clone() else {
                return Err(ew_commands::text_key!("table-reason-no-command"));
            };
            let targets = row_targets(node, &kind);
            if targets.is_empty() || define_point(node, &kind, &path).is_some() {
                // A potential's defining cell (WP-S1.16 D4) is defined in the window.
                return Err(ew_commands::text_key!("table-reason-window"));
            }
            return Ok(((kind, path), targets));
        }
        let column = &table.columns[c];
        let route = IO_LIST_ROUTES
            .iter()
            .find(|(k, _, _)| self.key == "kind:io_list" && *k == column.key);
        let channel = table.rows[r]
            .id
            .as_deref()
            .and_then(|id| RowId::parse(id).ok())
            .and_then(|id| match id.0.first() {
                Some(RowKey::Entity(u)) => Some(*u),
                _ => None,
            });
        match (route, channel) {
            (Some((_, kind, path)), Some(channel)) => {
                Ok((((*kind).to_owned(), (*path).to_owned()), vec![channel]))
            }
            (Some(_), None) => Err(ew_commands::text_key!("table-no-record")),
            (None, _) => Err(ew_commands::text_key!("table-reason-derived")),
        }
    }

    /// One cell of a matched row: its preview line and finding, and the gesture cell when it applies.
    fn cell(
        &self,
        import: &mut TableImport,
        pending: &mut Vec<Pending>,
        file_row: usize,
        r: usize,
        c: usize,
        raw: &str,
    ) {
        let table = &self.current.table;
        let column = &table.columns[c];
        let row = &table.rows[r];
        let id = row.id.clone().unwrap_or_default();
        let ew = row.cells.get(c).map_or("", TableCell::text).to_owned();
        let file = self.text(raw, column);
        let base: Option<String> = self.base.and_then(|b| {
            let at = b.columns.iter().position(|k| *k == column.key)?;
            b.rows
                .get(&id)
                .map(|cells| cells.get(at).cloned().unwrap_or_default())
        });
        let reference = base.as_deref().unwrap_or(&ew);
        let file_changed = file.trim() != reference.trim();
        let ew_changed = base.as_deref().is_some_and(|b| b.trim() != ew.trim());
        if !file_changed && !ew_changed {
            return;
        }
        let row_label = self.row_label(r);
        let what = format!("{} of {row_label}", column.label);
        let mut line = ImportCell {
            row: id.clone(),
            row_label,
            column: column.key.clone(),
            column_label: column.label.clone(),
            base: base.clone(),
            ew: ew.clone(),
            file: file.clone(),
            outcome: CellOutcome::Applied,
            reason: None,
        };
        let finding = |rule: &'static str, reason_key: Option<&'static str>, message: String| ImportFinding {
            rule,
            row: id.clone(),
            column: Some(column.key.clone()),
            reason_key,
            message,
        };
        if !file_changed {
            line.outcome = CellOutcome::Kept;
            import.findings.push(finding(
                "EW-IMP-302",
                None,
                format!("{what} was changed in the project since the export: the project's value is kept"),
            ));
            import.cells.push(line);
            return;
        }
        if ew_changed && file.trim() == ew.trim() {
            // Changed in both to the same value: nothing to do.
            return;
        }
        let route = match self.route(r, c) {
            Ok(route) => route,
            Err(reason) => {
                line.outcome = CellOutcome::ReadOnly;
                line.reason = Some(reason_words(reason).to_owned());
                import.findings.push(finding(
                    "EW-IMP-301",
                    Some(reason),
                    format!(
                        "{what} is read-only in a round trip ({}): the file's value is not imported",
                        reason_words(reason)
                    ),
                ));
                import.cells.push(line);
                return;
            }
        };
        if is_lossy(&file, reference) {
            import.findings.push(finding(
                "EW-IMP-305",
                None,
                format!(
                    "{what} reads \"{file}\" where the export wrote \"{reference}\": a spreadsheet stored the text as a \
                     number; \"{file}\" is imported"
                ),
            ));
        }
        if ew_changed {
            line.outcome = CellOutcome::Conflict {
                taken: self.take_file,
            };
            let kept = if self.take_file {
                "the file's value is taken"
            } else {
                "the project's value is kept (`--take-file` takes the file's)"
            };
            import.findings.push(finding(
                "EW-IMP-304",
                None,
                format!("{what} was changed in the project and in the file: {kept}"),
            ));
            if !self.take_file {
                import.cells.push(line);
                return;
            }
        }
        let ((kind, path), targets) = route;
        let (input, extra) = potential_input(&kind, &path, &file);
        let value = match self.session.parse_field_input(&kind, &path, &input) {
            Ok(parsed) => CellValue::Valid(parsed.value),
            Err(e @ (InputError::Invalid(_) | InputError::NotOneOf { .. })) => {
                CellValue::Invalid(e.text().unwrap_or_default())
            }
            Err(InputError::UnknownPart(typed)) => CellValue::Invalid(format!(
                "no part copy of the project has the part number `{typed}`"
            )),
            Err(InputError::Ambiguous { choices }) => {
                let texts: Vec<String> = choices.iter().map(|(t, _)| format!("\"{t}\"")).collect();
                CellValue::Invalid(format!("it reads as {}; write one", texts.join(" or ")))
            }
        };
        if let CellValue::Invalid(why) = &value {
            line.outcome = CellOutcome::Refused;
            line.reason = Some(why.clone());
            import.cells.push(line);
            return;
        }
        let at = import.cells.len();
        import.cells.push(line.clone());
        pending.push(Pending {
            at,
            cell: GestureCell {
                row: id,
                row_label: line.row_label,
                row_number: file_row,
                column: column.key.clone(),
                column_label: column.label.clone(),
                text: file,
                route: Some((kind, path)),
                targets,
                value,
                conflict: None,
                dropped: false,
                define: None,
                extra: extra.into_iter().collect(),
                define_clash: false,
                define_unnamed: false,
            },
        });
    }

    /// Compiles the applicable cells into one `fields.set_many` (D10's rule); a cell that sets a record an earlier row
    /// of the file sets to another value is refused.
    fn compile(&self, import: &mut TableImport, pending: Vec<Pending>) -> Result<(), TableImportError> {
        if pending.len() > MAX_IMPORT_CELLS {
            return Err(TableImportError::TooManyCells(pending.len()));
        }
        let ats: Vec<usize> = pending.iter().map(|p| p.at).collect();
        let mut gesture = Gesture {
            kind: GestureKind::Paste,
            nav: self.current.nav.as_ref().map_or(NavKind::Plc, |n| n.kind),
            cells: pending.into_iter().map(|p| p.cell).collect(),
            ignored: Vec::new(),
        };
        mark_conflicts(&mut gesture);
        for (cell, at) in gesture.cells.iter().zip(&ats) {
            if let Some(n) = cell.conflict {
                let line = &mut import.cells[*at];
                line.outcome = CellOutcome::Refused;
                line.reason = Some(format!(
                    "row {n} of the file sets the same record to another value"
                ));
            }
        }
        let compiled = compile(&gesture, &mut |k, t, m| self.session.member_base(k, t, m));
        import.command = compiled.map(|c| {
            let params = if c.name == "fields.set" {
                json!({ "edits": [c.params] })
            } else {
                c.params
            };
            AnyCommand {
                name: "fields.set_many".to_owned(),
                params,
            }
        });
        Ok(())
    }
}

/// Whether the file's text is a number a spreadsheet made of the exported text (EW-IMP-305): both read as the same
/// number, and the texts differ (`1.10` read `1.1`).
fn is_lossy(file: &str, exported: &str) -> bool {
    let number = |t: &str| t.trim().parse::<f64>().ok().filter(|f| f.is_finite());
    file.trim() != exported.trim() && number(file).is_some() && number(file) == number(exported)
}

/// The English words of a read-only reason key, as the CLI prints a round trip's findings: the key's text of the
/// window's en-GB locale file.
pub fn reason_words(key: &str) -> &'static str {
    ew_commands::text_english(key).unwrap_or("no command yet")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// D11: a CSV's guard apostrophe is dropped before `=+-@`, tab and CR only, and never in `rfc4180`; the lossy rule.
    #[test]
    fn the_csv_guard_and_the_lossy_rule() {
        assert!(is_lossy("1.1", "1.10"));
        assert!(!is_lossy("1.2", "1.10"));
        assert!(!is_lossy("x", "1.10"));
        assert!(!is_lossy("1.1", "1.1"));
        assert!(semicolons(b"id;a,b;c\n"));
        assert!(!semicolons(b"id,\"a;b;c\"\n"));
    }

    /// D5: an `_ew_base` sheet whose digest matches its rows is intact, an edited one is not.
    #[test]
    fn an_edited_base_is_detected() {
        let rows = vec![
            vec!["id".to_owned(), "label".to_owned(), "state".to_owned()],
            vec!["e:1".to_owned(), "Pump".to_owned()],
        ];
        let digest = sha256_hex(b"id\tlabel\tstate\ne:1\tPump\t\n");
        let mut sheet = rows.clone();
        sheet.push(vec![BASE_DIGEST_ROW.to_owned(), digest.clone()]);
        let base = intact_base(&sheet).unwrap();
        assert_eq!(base.columns, ["label", "state"]);
        assert_eq!(base.rows["e:1"], ["Pump", ""]);
        sheet[1][1] = "Fan".to_owned();
        assert_eq!(intact_base(&sheet), None);
        assert_eq!(intact_base(&rows), None, "no digest row");
    }
}
