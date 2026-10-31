// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The JSON results of the `ew` subcommands (doc 17 §3.1, FR-EXT-024; WP-S1.22 part 1, D3 and D5).
//!
//! Every subcommand that prints a result takes `--json`: stdout is then one JSON object whose first member is
//! `"$schema": "ew.<command>-result/1"` (the command's words joined by `-`). Each result is a type here (or, for
//! `ew check` and `ew sql`, in the crate that builds it: [`CheckResult`], [`SqlResult`]); the type is the schema, so
//! the printed object and the published file `bin/ew/schemas/<name>.schema.json` cannot drift. [`RESULTS`] lists
//! every result; [`schema_files`] renders the published directory, [`schema_text`] one file of it.
//!
//! [`to_text`] prints a result: two-space indentation, members in byte-wise key order (`$schema` sorts first), LF and
//! a final newline, so one result always prints to the same bytes.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use ew_checks::report::CheckResult;
pub use ew_sqlview::SqlResult;

/// One published result schema: its name and the subcommand that prints it.
#[derive(Clone, Copy, Debug)]
pub struct ResultSchema {
    /// The schema name, `ew.<command>-result/<n>`.
    pub name: &'static str,
    /// The subcommand path that prints it, words joined by a space (`export pdf`).
    pub command: &'static str,
    schema: fn() -> Value,
}

impl ResultSchema {
    /// The JSON Schema document (draft 2020-12) of the result: the type's schema with the name as its `title` and
    /// as the `const` of `$schema`.
    pub fn schema(&self) -> Value {
        let mut schema = (self.schema)();
        if let Value::Object(map) = &mut schema {
            map.insert("title".into(), Value::String(self.name.into()));
            if let Some(Value::Object(props)) = map.get_mut("properties") {
                props.insert(
                    "$schema".into(),
                    serde_json::json!({ "const": self.name, "description": "The name of this schema." }),
                );
            }
        }
        schema
    }

    /// The published file name: `ew.check-result/1` → `ew.check-result-1.schema.json`.
    pub fn file_name(&self) -> String {
        file_name(self.name)
    }
}

/// The published file name of the result schema `name` (`ew.check-result/1` → `ew.check-result-1.schema.json`).
pub fn file_name(name: &str) -> String {
    format!("{}.schema.json", name.replace('/', "-"))
}

fn schema_of<T: JsonSchema>() -> Value {
    SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<T>()
        .to_value()
}

/// `ew import qet --json`.
pub const IMPORT_QET: &str = "ew.import-qet-result/1";
/// `ew import csv --json`.
pub const IMPORT_CSV: &str = "ew.import-csv-result/1";
/// `ew fmt --json`.
pub const FMT: &str = "ew.fmt-result/1";
/// `ew recover --json`.
pub const RECOVER: &str = "ew.recover-result/1";
/// `ew import parts --json`.
pub const IMPORT_PARTS: &str = "ew.import-parts-result/1";
/// `ew check --json`.
pub const CHECK: &str = ew_checks::report::JSON_SCHEMA;
/// `ew sql --json`.
pub const SQL: &str = ew_sqlview::SQL_RESULT_SCHEMA;
/// `ew sql dump --json`.
pub const SQL_DUMP: &str = "ew.sql-dump-result/1";
/// `ew export cables --json`.
pub const EXPORT_CABLES: &str = "ew.export-cables-result/1";
/// `ew export bom --json`.
pub const EXPORT_BOM: &str = "ew.export-bom-result/1";
/// `ew export pdf --json`.
pub const EXPORT_PDF: &str = "ew.export-pdf-result/1";
/// `ew export svg --json`.
pub const EXPORT_SVG: &str = "ew.export-svg-result/1";
/// `ew export png --json`.
pub const EXPORT_PNG: &str = "ew.export-png-result/1";
/// `ew export wires --json`.
pub const EXPORT_WIRES: &str = "ew.export-wires-result/1";
/// `ew export terminals --json`.
pub const EXPORT_TERMINALS: &str = "ew.export-terminals-result/1";
/// `ew export report --json`.
pub const EXPORT_REPORT: &str = "ew.export-report-result/1";
/// `ew render --json`.
pub const RENDER: &str = "ew.render-result/1";
/// `ew apply --json`.
pub const APPLY: &str = "ew.apply-result/1";
/// `ew verify-derived --json`.
pub const VERIFY_DERIVED: &str = "ew.verify-derived-result/1";
/// `ew lib list --json`.
pub const LIB_LIST: &str = "ew.lib-list-result/1";
/// `ew lib search --json`.
pub const LIB_SEARCH: &str = "ew.lib-search-result/1";
/// `ew lib index --rebuild --json`.
pub const LIB_INDEX: &str = "ew.lib-index-result/1";
/// `ew lib create --json`.
pub const LIB_CREATE: &str = "ew.lib-create-result/1";
/// `ew lib import parts --json`.
pub const LIB_IMPORT_PARTS: &str = "ew.lib-import-parts-result/1";
/// `ew commands --json`.
pub const COMMANDS: &str = "ew.commands-result/1";
/// `ew query --json`.
pub const QUERY: &str = "ew.query-result/1";
/// `ew queries --json`.
pub const QUERIES: &str = "ew.queries-result/1";

/// Every published result schema, in name order.
pub const RESULTS: &[ResultSchema] = &[
    ResultSchema {
        name: APPLY,
        command: "apply",
        schema: schema_of::<ApplyResult>,
    },
    ResultSchema {
        name: CHECK,
        command: "check",
        schema: schema_of::<CheckResult>,
    },
    ResultSchema {
        name: COMMANDS,
        command: "commands",
        schema: schema_of::<CommandsResult>,
    },
    ResultSchema {
        name: EXPORT_BOM,
        command: "export bom",
        schema: schema_of::<ExportRowsResult>,
    },
    ResultSchema {
        name: EXPORT_CABLES,
        command: "export cables",
        schema: schema_of::<ExportRowsResult>,
    },
    ResultSchema {
        name: EXPORT_PDF,
        command: "export pdf",
        schema: schema_of::<ExportPdfResult>,
    },
    ResultSchema {
        name: EXPORT_PNG,
        command: "export png",
        schema: schema_of::<ExportPngResult>,
    },
    ResultSchema {
        name: EXPORT_REPORT,
        command: "export report",
        schema: schema_of::<ExportReportResult>,
    },
    ResultSchema {
        name: EXPORT_SVG,
        command: "export svg",
        schema: schema_of::<ExportSvgResult>,
    },
    ResultSchema {
        name: EXPORT_TERMINALS,
        command: "export terminals",
        schema: schema_of::<ExportTerminalsResult>,
    },
    ResultSchema {
        name: EXPORT_WIRES,
        command: "export wires",
        schema: schema_of::<ExportRowsResult>,
    },
    ResultSchema {
        name: FMT,
        command: "fmt",
        schema: schema_of::<FmtResult>,
    },
    ResultSchema {
        name: IMPORT_CSV,
        command: "import csv",
        schema: schema_of::<ImportCsvResult>,
    },
    ResultSchema {
        name: IMPORT_PARTS,
        command: "import parts",
        schema: schema_of::<ImportPartsResult>,
    },
    ResultSchema {
        name: IMPORT_QET,
        command: "import qet",
        schema: schema_of::<ImportQetResult>,
    },
    ResultSchema {
        name: LIB_CREATE,
        command: "lib create",
        schema: schema_of::<LibCreateResult>,
    },
    ResultSchema {
        name: LIB_IMPORT_PARTS,
        command: "lib import parts",
        schema: schema_of::<LibImportPartsResult>,
    },
    ResultSchema {
        name: LIB_INDEX,
        command: "lib index",
        schema: schema_of::<LibIndexResult>,
    },
    ResultSchema {
        name: LIB_LIST,
        command: "lib list",
        schema: schema_of::<LibListResult>,
    },
    ResultSchema {
        name: LIB_SEARCH,
        command: "lib search",
        schema: schema_of::<LibSearchResult>,
    },
    ResultSchema {
        name: QUERIES,
        command: "queries",
        schema: schema_of::<QueriesResult>,
    },
    ResultSchema {
        name: QUERY,
        command: "query",
        schema: schema_of::<QueryResult>,
    },
    ResultSchema {
        name: RECOVER,
        command: "recover",
        schema: schema_of::<RecoverResult>,
    },
    ResultSchema {
        name: RENDER,
        command: "render",
        schema: schema_of::<RenderResult>,
    },
    ResultSchema {
        name: SQL_DUMP,
        command: "sql dump",
        schema: schema_of::<SqlDumpResult>,
    },
    ResultSchema {
        name: SQL,
        command: "sql",
        schema: schema_of::<SqlResult>,
    },
    ResultSchema {
        name: VERIFY_DERIVED,
        command: "verify-derived",
        schema: schema_of::<VerifyDerivedResult>,
    },
];

/// The published result schema named `name`.
pub fn result(name: &str) -> Option<&'static ResultSchema> {
    RESULTS.iter().find(|r| r.name == name)
}

/// The canonical text of a JSON value: two-space indentation, members in byte-wise key order, LF, a final newline.
fn render(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
    text.push('\n');
    text
}

/// The text of the published schema file of `name` ([`schema_files`]), or `None` for a name that is not a result.
pub fn schema_text(name: &str) -> Option<String> {
    result(name).map(|r| render(&r.schema()))
}

/// The files of `bin/ew/schemas/`: one `<name>.schema.json` per result and the catalogue `index.json`
/// (`{"results": [{"command", "name", "schema"}]}`), as `(file name, contents)` sorted by file name.
pub fn schema_files() -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = RESULTS
        .iter()
        .map(|r| (r.file_name(), render(&r.schema())))
        .collect();
    let index: Vec<Value> = RESULTS
        .iter()
        .map(|r| serde_json::json!({ "name": r.name, "command": r.command, "schema": r.file_name() }))
        .collect();
    files.push((
        "index.json".into(),
        render(&serde_json::json!({ "results": index })),
    ));
    files.sort();
    files
}

/// A result as `ew … --json` prints it, through a JSON value: two-space indentation, members in byte-wise key order,
/// LF and a final newline.
pub fn to_text<T: Serialize>(result: &T) -> String {
    render(&serde_json::to_value(result).unwrap_or(Value::Null))
}

// ---------------------------------------------------------------------------------------------------------
// The result types

/// `ew import qet --json` ([`IMPORT_QET`]): what the import found; the migration report itself goes to
/// `--report-out`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportQetResult {
    /// [`IMPORT_QET`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The imported file's name (never its path).
    pub source: String,
    /// The records imported.
    pub counts: QetCounts,
    /// The findings of the migration report.
    pub findings: u32,
    /// The connections the import could not keep.
    pub lost_connections: u32,
    /// The findings of the lenient checks run on the import.
    pub lenient_findings: u32,
    /// The pages in page order of the derived model; `null` when the model check failed.
    pub derived_pages: Option<usize>,
    /// Why the model check failed; `null` when it passed (exit 1 otherwise).
    pub model_check_error: Option<String>,
    /// The file the migration report was written to (`--report-out`); `null` without one.
    pub report_out: Option<String>,
}

/// The record counts of an [`ImportQetResult`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QetCounts {
    /// Pages.
    pub pages: usize,
    /// Devices (strips and terminals included).
    pub devices: usize,
    /// Terminal strips.
    pub strips: usize,
    /// Terminals.
    pub terminals: usize,
    /// Functions.
    pub functions: usize,
    /// Representations.
    pub representations: usize,
    /// Connections.
    pub connections: usize,
    /// Junctions.
    pub junctions: usize,
}

/// `ew import parts --json` ([`IMPORT_PARTS`]): the part copies created and skipped; with `--map`, the row counts and
/// the report of the mapped import.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportPartsResult {
    /// [`IMPORT_PARTS`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The table's name.
    pub source: String,
    /// The project-local part copies created.
    pub created: usize,
    /// The parts skipped (the project already has their key).
    pub skipped: usize,
    /// The row counts of a mapped import (`--map`); `null` for the golden dialect.
    pub rows: Option<RowCounts>,
    /// The report of a mapped import, one entry per finding; empty for the golden dialect.
    pub report: Vec<ReportRow>,
}

/// The row counts of a mapped parts import.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RowCounts {
    /// New parts.
    pub new: usize,
    /// Updated parts.
    pub updated: usize,
    /// Unchanged parts.
    pub unchanged: usize,
    /// Duplicate rows.
    pub duplicate: usize,
    /// Rejected rows.
    pub rejected: usize,
}

/// The severity of a [`ReportRow`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReportSeverity {
    /// Info.
    Info,
    /// Warning.
    Warning,
    /// Error.
    Error,
}

/// One finding of a mapped parts import.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportRow {
    /// The finding code (`EW-IMP-002`).
    pub code: String,
    /// The severity.
    pub severity: ReportSeverity,
    /// The table line, where the finding has one.
    pub line: Option<u64>,
    /// The message.
    pub message: String,
}

/// `ew sql dump --json` ([`SQL_DUMP`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SqlDumpResult {
    /// [`SQL_DUMP`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The SQLite file written.
    pub out: String,
}

/// `ew export cables|bom --json` ([`EXPORT_CABLES`], [`EXPORT_BOM`]): the CSV in the result, or the file it went to.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportRowsResult {
    /// [`EXPORT_CABLES`], [`EXPORT_BOM`] or [`EXPORT_WIRES`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The rows (data and heading rows, the header line excluded).
    pub rows: usize,
    /// The file the CSV was written to (`--out`); `null` without one.
    pub out: Option<String>,
    /// The CSV text when there is no `--out`; `null` with one.
    pub csv: Option<String>,
}

/// `ew export pdf --json` ([`EXPORT_PDF`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportPdfResult {
    /// [`EXPORT_PDF`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The PDF file written.
    pub out: String,
    /// The pages.
    pub pages: usize,
    /// The links (cross-references and outline targets).
    pub links: usize,
    /// The page findings (unresolved wire ends, unknown styles, overflowing table texts).
    pub findings: usize,
}

/// `ew export svg --json` ([`EXPORT_SVG`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportSvgResult {
    /// [`EXPORT_SVG`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The directory written.
    pub out: String,
    /// The files written, one `<folio>.svg` per page, in page order.
    pub files: Vec<String>,
    /// The page findings.
    pub findings: usize,
}

/// `ew export terminals --json` ([`EXPORT_TERMINALS`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportTerminalsResult {
    /// [`EXPORT_TERMINALS`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The rows (data and heading rows, a strip's heading row included; the header line excluded).
    pub rows: usize,
    /// The printed tags of the strips written, in output order.
    pub strips: Vec<String>,
    /// The file the CSV was written to (`--out`); `null` without one.
    pub out: Option<String>,
    /// The CSV text when there is no `--out`; `null` with one.
    pub csv: Option<String>,
}

/// `ew export report --json` ([`EXPORT_REPORT`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportReportResult {
    /// [`EXPORT_REPORT`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The report block's ID.
    pub block: String,
    /// The block's report kind (`plc_overview`, ...).
    pub kind: String,
    /// `csv` or `pdf`.
    pub format: String,
    /// For CSV: the rows (data and heading rows, the header line excluded); `null` for PDF.
    pub rows: Option<usize>,
    /// For PDF: the folio labels of the block's pages written, in page order; `null` for CSV.
    pub folios: Option<Vec<String>>,
    /// The file written (`--out`); `null` for CSV on stdout.
    pub out: Option<String>,
    /// The CSV text when it goes to stdout; else `null`.
    pub csv: Option<String>,
}

/// `ew export png --json` ([`EXPORT_PNG`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportPngResult {
    /// [`EXPORT_PNG`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The directory written.
    pub out: String,
    /// The resolution in dots per inch.
    pub dpi: u32,
    /// The files written, one `<folio>.png` per page, in page order.
    pub files: Vec<String>,
    /// The page findings.
    pub findings: usize,
}

/// `ew render --json` ([`RENDER`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderResult {
    /// [`RENDER`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The folio label of the page.
    pub page: String,
    /// The PNG file written.
    pub out: String,
    /// The resolution in dots per inch.
    pub dpi: u32,
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
    /// The page findings.
    pub findings: usize,
}

/// `ew apply --json` ([`APPLY`]): the replay's counts, and with `--preview` the changes.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyResult {
    /// [`APPLY`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The lines (transactions) of the log.
    pub lines: usize,
    /// The lines that committed.
    pub committed: usize,
    /// Whether the project was saved.
    pub saved: bool,
    /// The changes of `--preview`; `null` without it.
    pub preview: Option<ApplyChanges>,
}

/// The changes `ew apply --preview` reports.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyChanges {
    /// The changed records.
    pub records: Vec<RecordChange>,
    /// The changed tags.
    pub tags: Vec<DerivedRowChange>,
    /// The changed cross-references.
    pub xrefs: Vec<DerivedRowChange>,
}

/// One changed record.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordChange {
    /// `Created`, `Updated` or `Deleted`.
    pub change: String,
    /// The record kind.
    pub kind: String,
    /// The record ID.
    pub id: String,
    /// The changed fields of an update.
    pub fields: Vec<String>,
}

/// One changed derived value (a tag or a cross-reference).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DerivedRowChange {
    /// What changed.
    pub key: String,
    /// The value before.
    pub before: String,
    /// The value after.
    pub after: String,
}

/// `ew verify-derived --json` ([`VERIFY_DERIVED`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerifyDerivedResult {
    /// [`VERIFY_DERIVED`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The keys read per query family (one pass).
    pub families: BTreeMap<String, usize>,
    /// The keys read in one pass.
    pub keys: usize,
    /// The keys whose memoised value differs from the cold one.
    pub differences: Vec<String>,
    /// What the memo oracle found.
    pub oracle: Vec<OracleMismatch>,
}

/// One mismatch of the memo oracle.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OracleMismatch {
    /// The query family.
    pub family: String,
    /// The key, as `Debug` text.
    pub key: String,
    /// The memoised value, as `Debug` text.
    pub memoised: String,
    /// The cold value, as `Debug` text.
    pub cold: String,
}

/// `ew lib list --json` ([`LIB_LIST`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibListResult {
    /// [`LIB_LIST`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The libraries of the four tiers.
    pub libraries: Vec<LibraryRow>,
}

/// One library of [`LibListResult`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibraryRow {
    /// The library ID.
    pub id: String,
    /// The human key.
    pub key: String,
    /// The display name.
    pub name: String,
    /// The tier (`system`, `community`, `company`, `user`).
    pub tier: String,
    /// The library's version.
    pub version: String,
    /// The default licence of its items.
    pub licence: String,
    /// Whether it accepts writes.
    pub writable: bool,
    /// The manifest generation.
    pub generation: u64,
    /// The index state (`fresh`, `rebuilt`, `failed: …`).
    pub index: String,
    /// The directory; `null` for a compiled-in library.
    pub path: Option<String>,
    /// Files and lines skipped, and other notes.
    pub findings: Vec<String>,
}

/// `ew lib search --json` ([`LIB_SEARCH`]).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibSearchResult {
    /// [`LIB_SEARCH`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The hits, parts first.
    pub hits: Vec<LibHit>,
}

/// One hit of [`LibSearchResult`]: a part or another library item.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum LibHit {
    /// A part (`kind` is `part`).
    Part(PartHitRow),
    /// A symbol, FnDef, form, macro or manufacturer.
    Item(ItemHitRow),
}

/// A part hit.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PartHitRow {
    /// `part`.
    pub kind: String,
    /// The library ID.
    pub library: String,
    /// The library's key.
    pub library_key: String,
    /// The item ID.
    pub item: String,
    /// The item's version.
    pub version: String,
    /// The library's tier.
    pub tier: String,
    /// The part number, verbatim.
    pub part_number: String,
    /// The manufacturer's name.
    pub manufacturer: String,
    /// The description in every language, joined by " / ".
    pub description: String,
    /// The part kind.
    pub part_kind: String,
    /// The item status (`released`, or `{"deprecated": {"successor": …}}`).
    pub status: Value,
    /// 0 (no text), 1 exact part number, 2 prefix, 3 trigram, 4 full text.
    pub rank_tier: u8,
}

/// A hit of another library item.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ItemHitRow {
    /// `symbol`, `fndef`, `form`, `macro` or `manufacturer`.
    pub kind: String,
    /// The library ID.
    pub library: String,
    /// The library's key.
    pub library_key: String,
    /// The item ID.
    pub item: String,
    /// The item's version.
    pub version: String,
    /// The library's tier.
    pub tier: String,
    /// The human key, empty when none.
    pub key: String,
    /// The name in every language, joined by " / ".
    pub name: String,
    /// The item status.
    pub status: Value,
    /// 0 (no text), 1 exact key, 2 key prefix, 4 full text.
    pub rank_tier: u8,
}

/// `ew lib index --rebuild --json` ([`LIB_INDEX`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibIndexResult {
    /// [`LIB_INDEX`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The parts indexed.
    pub parts: u64,
    /// The other items indexed.
    pub items: u64,
}

/// `ew lib create --json` ([`LIB_CREATE`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibCreateResult {
    /// [`LIB_CREATE`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The new library's key.
    pub key: String,
    /// The new library's ID.
    pub id: String,
    /// Its directory.
    pub dir: String,
    /// Whether it lies in a library tier (else pass it as `--company`).
    pub listed: bool,
}

/// `ew lib import parts --json` ([`LIB_IMPORT_PARTS`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibImportPartsResult {
    /// [`LIB_IMPORT_PARTS`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The target library's key.
    pub library: String,
    /// The table's name.
    pub source: String,
    /// The row counts.
    pub rows: RowCounts,
    /// The report, one entry per finding.
    pub report: Vec<ReportRow>,
    /// What the library write did; `null` when there was nothing to write.
    pub written: Option<LibWrite>,
}

/// The library write of [`LibImportPartsResult`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibWrite {
    /// Items new to the library.
    pub added: usize,
    /// Items written at a new version.
    pub updated: usize,
    /// Items unchanged.
    pub unchanged: usize,
    /// The library's generation after the write.
    pub generation: u64,
}

/// `ew commands --json` ([`COMMANDS`]): the command registry, the entries of `crates/ew-commands/schemas/index.json`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommandsResult {
    /// [`COMMANDS`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The commands in name order.
    pub commands: Vec<CommandEntry>,
}

/// One command of [`CommandsResult`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommandEntry {
    /// The canonical name (`fields.set`).
    pub name: String,
    /// The other names it answers to.
    pub aliases: Vec<String>,
    /// The version of its parameters.
    pub params_version: u32,
    /// `interactive` or `batch`.
    pub cost: String,
    /// The exposure class (`edit`, `human_only`, …).
    pub exposure: String,
    /// The confirmation it asks for (`none`, …).
    pub confirmation: String,
    /// Whether it is undoable.
    pub undoable: bool,
    /// `preview` before 1.0.
    pub stability: String,
    /// The parameter schema's file (`ew schema <name>` prints it).
    pub schema: String,
}

/// `ew import csv --json` ([`IMPORT_CSV`]): the tables read, the import report and whether the new project was written.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportCsvResult {
    /// [`IMPORT_CSV`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The table files read, by name, in name order.
    pub tables: Vec<String>,
    /// The import mode: `strict` (the default) or `lenient`.
    pub mode: ImportCsvMode,
    /// The new project's directory, as given (`--out`).
    pub out: String,
    /// Whether the project was written (`false`: `--strict` refused the error findings).
    pub written: bool,
    /// The records the import made.
    pub records: usize,
    /// The library documents the import embedded.
    pub documents: usize,
    /// The import report, one entry per finding, in the report's order.
    pub report: Vec<TableReportRow>,
}

/// The mode of `ew import csv`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ImportCsvMode {
    /// A row the dialect refuses is dropped with an error finding, and the project is not written.
    Strict,
    /// Foreign colour codes are kept with a warning; the project is written with its findings.
    Lenient,
}

/// One finding of a table import.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TableReportRow {
    /// The finding code (`EW-IMP-002`).
    pub code: String,
    /// The severity.
    pub severity: ReportSeverity,
    /// The table file, where the finding has one.
    pub file: Option<String>,
    /// The table line, where the finding has one.
    pub line: Option<u64>,
    /// The message.
    pub message: String,
}

/// `ew fmt --json` ([`FMT`]): the files not in canonical form (with `--check`: those a format would rewrite; without:
/// those it rewrote), and the lines kept verbatim.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FmtResult {
    /// [`FMT`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// Whether this was `--check` (nothing is written).
    pub check: bool,
    /// The files not in canonical form, in path order.
    pub files: Vec<FmtFile>,
    /// The lines (or files) kept verbatim as the open reported them: quarantined lines, malformed values, unknown
    /// kinds, members or files.
    pub kept: Vec<KeptRow>,
    /// The generation on disk afterwards.
    pub generation: u64,
}

/// One file of [`FmtResult`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FmtFile {
    /// The file, relative to the project directory.
    pub file: String,
    /// Whether nothing is left in it and it is removed (its records belong in other files).
    pub removed: bool,
}

/// One kept line of [`FmtResult`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeptRow {
    /// The file, relative to the project directory.
    pub file: String,
    /// The 1-based line; `null` for a whole file.
    pub line: Option<u32>,
    /// The finding's rule (`EW-CORE-007`).
    pub code: String,
    /// Why it is kept.
    pub reason: String,
}

/// `ew recover --json` ([`RECOVER`]): the crashed session's journal that was summarised, and whether it was replayed.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoverResult {
    /// [`RECOVER`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// What ran: `summary` (no flag) or `replay` (`--replay`).
    pub action: RecoverAction,
    /// The journal of the newest crashed session; `null` when there is none.
    pub journal: Option<JournalSummary>,
    /// Whether its transactions were replayed and saved.
    pub replayed: bool,
    /// The generation on disk afterwards.
    pub generation: u64,
}

/// What `ew recover` did.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RecoverAction {
    /// The summary only.
    Summary,
    /// The replay and save.
    Replay,
}

/// The summary of a crashed session's journal.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JournalSummary {
    /// Its unsaved transactions.
    pub transactions: usize,
    /// The command of each transaction, in journal order.
    pub labels: Vec<String>,
    /// Whether the project on disk is still the journal's base (the replay cannot collide).
    pub base_matches: bool,
    /// How many further crashed sessions' journals remain after this one (each is offered by its own `ew recover`).
    pub further: usize,
}

/// `ew query --json` ([`QUERY`]): the answer of one public query (WP-S1.22 D11, D12).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryResult {
    /// [`QUERY`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The query's name (`device_tag`).
    pub query: String,
    /// The key as given (`tag:-101F1`, `-`).
    pub key: String,
    /// The session's revision the answer was computed at.
    pub rev: u64,
    /// The query's output object; its schema is the query's `output` in `ew queries --json`.
    pub output: Value,
    /// The explanation (`--explain`); `null` without it.
    pub explain: Option<QueryExplain>,
}

/// The explanation of a query's value (`ew query --explain`, D12): the memo's policy and state, its dependencies and
/// the property row's provenance. No wall-clock time: revisions are the session's.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryExplain {
    /// The memo family explained: the query's own, or the outermost memoised family of a composed query.
    pub family: String,
    /// The policy line (`Memo · device_tag · verified at rev 1 (miss) · changed at rev 1`).
    pub policy: String,
    /// Whether the memo held the value, verified, before the query ran; `null` for a family without a memo.
    pub hit: Option<bool>,
    /// The revision at which the memo was last confirmed valid; `null` without a memo.
    pub verified_at: Option<u64>,
    /// The revision at which its value last changed; `null` without a memo.
    pub changed_at: Option<u64>,
    /// The memo's dependencies in read order, in words (at most 64, then `…`): memos by family and described key,
    /// inputs by record kind and tag, folio or ID, each with the revision observed.
    pub depends_on: Vec<String>,
    /// The `via` of the property-grid row the query backs (`device_tag`, `cell_of`); `null` otherwise.
    pub provenance: Option<String>,
}

/// `ew queries --json` ([`QUERIES`]): the query registry. Its text is the committed snapshot
/// `crates/ew-api/api/queries.json` (D13).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueriesResult {
    /// [`QUERIES`].
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The queries in name order.
    pub queries: Vec<QueryEntry>,
}

/// One query of [`QueriesResult`].
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryEntry {
    /// The name (`device_tag`).
    pub name: String,
    /// The grammar of its key (`-` for none).
    pub key: String,
    /// What it answers.
    pub description: String,
    /// Its golden example: a key on Golden A.
    pub example: String,
    /// The version of its output type.
    pub output_version: u32,
    /// The JSON Schema (draft 2020-12) of its output object.
    pub output: Value,
}

/// The query registry as [`QueriesResult`] ([`crate::queries::REGISTRY`], in name order).
pub fn queries() -> QueriesResult {
    QueriesResult {
        schema: QUERIES.to_owned(),
        queries: crate::queries::REGISTRY.iter().map(|q| q.entry()).collect(),
    }
}

/// The command registry as [`CommandsResult`]: the entries of the built-in registry's `index.json`, in its order.
pub fn commands() -> CommandsResult {
    let files = ew_commands::CommandRegistry::builtin().schema_files();
    let index = files
        .iter()
        .find(|(name, _)| name == "index.json")
        .map(|(_, text)| text.as_str())
        .unwrap_or("{}");
    #[derive(Deserialize)]
    struct Index {
        commands: Vec<CommandEntry>,
    }
    let commands = serde_json::from_str::<Index>(index).map_or_else(|_| Vec::new(), |i| i.commands);
    CommandsResult {
        schema: COMMANDS.to_owned(),
        commands,
    }
}

/// The parameter schema file of the command `name` (or an alias), as `crates/ew-commands/schemas/` holds it.
pub fn command_schema_text(name: &str) -> Option<String> {
    let registry = ew_commands::CommandRegistry::builtin();
    let canonical = registry.get(name)?.meta().name;
    let file = format!("{canonical}.schema.json");
    registry
        .schema_files()
        .into_iter()
        .find(|(n, _)| *n == file)
        .map(|(_, text)| text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_sorted_distinct_and_well_formed() {
        let names: Vec<&str> = RESULTS.iter().map(|r| r.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted);
        for r in RESULTS {
            let words = r.command.replace(' ', "-");
            assert_eq!(r.name, format!("ew.{words}-result/1"), "{}", r.command);
        }
    }

    #[test]
    fn every_schema_names_itself_and_requires_its_name() {
        for r in RESULTS {
            let s = r.schema();
            assert_eq!(s["title"], r.name);
            assert_eq!(s["$schema"], "https://json-schema.org/draft/2020-12/schema");
            assert_eq!(s["properties"]["$schema"]["const"], r.name);
            let required = s["required"].as_array().expect("required members");
            assert!(required.iter().any(|m| m == "$schema"), "{}", r.name);
        }
        assert_eq!(file_name(CHECK), "ew.check-result-1.schema.json");
    }

    #[test]
    fn to_text_sorts_members_with_the_schema_first() {
        let r = SqlDumpResult {
            schema: SQL_DUMP.into(),
            out: "p.sqlite".into(),
        };
        assert_eq!(
            to_text(&r),
            "{\n  \"$schema\": \"ew.sql-dump-result/1\",\n  \"out\": \"p.sqlite\"\n}\n"
        );
    }

    #[test]
    fn commands_lists_the_registry_and_schemas_resolve_aliases() {
        let c = commands();
        let names: Vec<&str> = ew_commands::CommandRegistry::builtin().names().collect();
        let listed: Vec<&str> = c.commands.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(listed, names);
        assert!(command_schema_text("fields.set").is_some());
        assert!(command_schema_text("no.such").is_none());
    }
}
