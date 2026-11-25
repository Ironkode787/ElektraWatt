// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Argument parsing and the commands of the `ew` CLI.

use std::path::{Path, PathBuf};

use ew_api::checks::{CheckMode, report, scheme};
use ew_api::cli_results as results;
use ew_api::import::Severity;
use ew_api::import::migration::ReportFormat;
use ew_api::import::{StarterLibrary, import_qet};
use ew_api::output::{
    CsvOptions, ExportOptions, PageSelection, Parallelism, Preset, RowExport, RowsFormat, RowsOutput,
    date_from_epoch,
};
use ew_api::{BomMode, CmdLog};

mod apply;
mod assets;
mod checks;
mod collab;
mod diff;
mod dxf;
mod eplan;
mod export;
mod import;
mod layout;
mod library;
mod merge_driver;
mod merge_resolve;
mod open;
mod plc;
mod print;
mod project_files;
mod query;
mod revision;
mod route;
mod sql;
mod symbol;
#[cfg(test)]
mod tests;
pub(crate) mod tree;

use apply::*;
use assets::*;
use checks::*;
use export::*;
use import::*;
use library::*;
use open::*;
use plc::*;
use print::*;
use revision::*;
use sql::*;
use tree::*;

use crate::catalogue::{Exit, HUMAN_EXITS, OK, ROW_EXPORT_ERRORS, USAGE_ETC, exit};

/// What `ew revision` does.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RevisionAction {
    /// `revision approve --as <role>`: the checker's sign-off on the open revision.
    Approve {
        /// The checker's party role (`--as`).
        role: String,
    },
    /// `revision release <label> [--archive-dir <dir>]`: the atomic release with the as-built archive,
    /// or settling an incomplete one.
    Release {
        /// The open revision's label.
        label: String,
        /// The archive root (`--archive-dir`); `None`: the default root beside the project directory.
        archive_dir: Option<PathBuf>,
    },
    /// `revision withdraw <label> --reason <text>`.
    Withdraw {
        /// The released revision's label.
        label: String,
        /// Why (`--reason`).
        reason: String,
    },
}

/// The arguments of `ew revision`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RevisionArgs {
    /// The subcommand.
    pub action: RevisionAction,
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
}

/// What `ew recover` does with the journal of a crashed session.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecoverAction {
    /// No flag: print its summary.
    Summary,
    /// `--replay`: replay its transactions and save.
    Replay,
    /// `--discard`: delete it after the word typed on a controlling terminal.
    Discard,
}

/// The arguments of `ew import csv`.
#[derive(clap::Args, Clone, PartialEq, Eq, Debug)]
pub struct ImportCsvArgs {
    /// The directory of the tables.
    #[arg(value_name = "tables-dir")]
    pub tables: PathBuf,
    /// The new project's directory: absent or empty.
    #[arg(long, value_name = "dir", allow_hyphen_values = true, overrides_with = "out")]
    pub out: PathBuf,
    /// The starter content directory (default: `EW_CONTENT_DIR`).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        overrides_with = "content"
    )]
    pub content: Option<PathBuf>,
    /// Write nothing when the import makes error findings (the default; not with `--lenient`).
    #[arg(long, overrides_with = "strict", conflicts_with = "lenient")]
    pub strict: bool,
    /// Keep foreign colour codes with a warning and write the project with its findings (else strict).
    #[arg(long, overrides_with = "lenient")]
    pub lenient: bool,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub json: bool,
}

/// The page format of `ew export pdf|svg|png`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageFormat {
    /// One PDF file (`export pdf`).
    Pdf,
    /// One SVG file per page in a directory (`export svg`).
    Svg,
    /// One PNG file per page in a directory (`export png`).
    Png,
    /// One DXF file per page in a directory (`export dxf`).
    Dxf,
}

/// The arguments of `ew export pdf|svg|png`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExportPagesArgs {
    /// PDF or SVG.
    pub format: PageFormat,
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The PDF file or the SVG or PNG directory (`--out`).
    pub out: PathBuf,
    /// The page selection (`--pages`).
    pub pages: PageSelection,
    /// The threads of the page phase (`--threads`); `None`: the machine's parallelism.
    pub threads: Option<usize>,
    /// The resolution of `export png` (`--dpi`, default 150); unused for PDF and SVG.
    pub dpi: u32,
    /// The DXF layer names of `export dxf` (`--layer`); unused for the other formats.
    pub dxf_layers: ew_api::output::DxfLayers,
    /// Whether the hyperlinks of `export dxf` name the pages' `.dwg` files (`--dwg-links`); unused for the other formats.
    pub dxf_dwg_links: bool,
    /// Print the pages' underlays (`--underlays`, FR-IMP-030).
    pub underlays: bool,
    /// Print the result as JSON (`--json`).
    pub json: bool,
}

/// How `ew print` hands the print set to the OS (`--via`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PrintVia {
    /// The CUPS command `lp`.
    Lp,
    /// The default PDF application.
    Viewer,
    /// The Windows print dialog, or the printer `--printer` names, through GDI (Windows only).
    #[cfg(windows)]
    Windows,
}

/// What `ew export` writes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ExportKind {
    /// The cable list (`export cables`).
    Cables,
    /// The bill of materials in a mode (`export bom --mode`).
    Bom(BomMode),
    /// The wire list (`export wires`).
    Wires,
    /// The terminal diagrams (`export terminals`), one strip (`--strip`) or every strip.
    Terminals(Option<String>),
    /// The rows of a block (`export report --format csv`), or its pages (`--format pdf`).
    Report {
        /// `--format pdf`.
        pdf: bool,
    },
    /// The whole-project rows of a report kind (`export report --kind <kind>`, CSV only).
    ReportKind(String),
}

/// The arguments of `ew export cables|bom|wires|terminals|report`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExportArgs {
    /// What to write.
    pub kind: ExportKind,
    /// The report block (`--block`, its ID or first folio); `None`: the default block of the kind.
    pub block: Option<String>,
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The output file (`--out`); `None`: stdout.
    pub out: Option<PathBuf>,
    /// Print the result as JSON (`--json`).
    pub json: bool,
    /// How the rows are written (`--format`, `--preset`, `--bom`, `--line-end`).
    pub format: RowFormat,
}

/// How a row export writes its rows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowFormat {
    /// CSV in a preset (the default: the `ew` dialect).
    Csv(CsvOptions),
    /// An XLSX workbook (`--format xlsx`, with `--out`).
    Xlsx,
}

impl Default for RowFormat {
    fn default() -> RowFormat {
        RowFormat::Csv(CsvOptions::default())
    }
}

/// The result format of `ew sql`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SqlFormat {
    /// CSV: the `ew` dialect, or a preset (`--preset`).
    Csv,
    /// JSON (`ew.sql-result/1`).
    Json,
    /// An XLSX workbook (`--format xlsx`, with `--out`).
    Xlsx,
}

/// Where the query text of `ew sql` comes from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SqlSource {
    /// The query on the command line.
    Text(String),
    /// A file holding the query (`--file`).
    File(PathBuf),
}

/// The output format of `ew check`.
#[derive(clap::ValueEnum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CheckFormat {
    /// One line per finding.
    Text,
    /// Markdown.
    #[value(name = "md")]
    Markdown,
    /// JSON (`ew.check-result/1`).
    Json,
}

/// The arguments of `ew import qet`.
#[derive(clap::Args, Clone, PartialEq, Eq, Debug)]
pub struct ImportQetArgs {
    /// The QElectroTech project file.
    #[arg(
        value_name = "file.qet",
        required_unless_present = "batch",
        conflicts_with = "batch"
    )]
    pub file: Option<PathBuf>,
    /// Import every `.qet` of this directory as a new project of `--out`, with a report each and a summary.
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        overrides_with = "batch",
        requires = "out",
        conflicts_with_all = ["into", "json", "report", "report_out"]
    )]
    pub batch: Option<PathBuf>,
    /// The starter content directory (default: `EW_CONTENT_DIR`, else the documents compiled into the program).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        overrides_with = "content"
    )]
    pub content: Option<PathBuf>,
    /// The migration report's format: md, csv or json.
    #[arg(long, value_name = "format", allow_hyphen_values = true, overrides_with = "report", value_parser = tree::report)]
    pub report: Option<ReportFormat>,
    /// The migration report's file (default: stdout).
    #[arg(
        long,
        value_name = "file",
        allow_hyphen_values = true,
        overrides_with = "report_out"
    )]
    pub report_out: Option<PathBuf>,
    /// Exit 1 on lost connections or error findings, and write no project.
    #[arg(long, overrides_with = "strict")]
    pub strict: bool,
    /// Print the result as JSON (`ew.import-qet-result/1`).
    #[arg(long, overrides_with = "json")]
    pub json: bool,
    /// Write the imported project into this directory (absent or empty); without it nothing is written.
    #[arg(long, value_name = "dir", allow_hyphen_values = true, overrides_with = "out")]
    pub out: Option<PathBuf>,
    /// Re-import the file into its earlier import in this project: prints what would change and writes nothing.
    #[arg(
        long,
        value_name = "project",
        allow_hyphen_values = true,
        overrides_with = "into",
        conflicts_with_all = ["out", "json", "report", "report_out"]
    )]
    pub into: Option<PathBuf>,
    /// With `--into`: commit the re-import and save the project.
    #[arg(long, overrides_with = "apply", requires = "into")]
    pub apply: bool,
}

/// The arguments of `ew import parts`.
#[derive(clap::Args, Clone, PartialEq, Eq, Debug)]
pub struct ImportPartsArgs {
    /// The parts table: `parts.csv` of the `ew` dialect, or with `--map` a supplier's CSV or XLSX table.
    #[arg(value_name = "table")]
    pub file: PathBuf,
    /// The project directory.
    #[arg(long, value_name = "dir", allow_hyphen_values = true)]
    pub project: Option<PathBuf>,
    /// Print the result as JSON (`ew.import-parts-result/1`).
    #[arg(long, overrides_with = "json")]
    pub json: bool,
    /// The column map (a TOML file): a supplier's table instead of the `ew` dialect.
    #[arg(
        long,
        value_name = "map.toml",
        allow_hyphen_values = true,
        overrides_with = "map"
    )]
    pub map: Option<PathBuf>,
    /// The parts' import date, YYYY-MM-DD (with `--map`).
    #[arg(long, value_name = "date", allow_hyphen_values = true, overrides_with = "as_of", value_parser = tree::date, requires = "map")]
    pub as_of: Option<ew_api::catalog::Date>,
    /// A company library whose manufacturers resolve the names (with `--map`; repeatable).
    #[arg(long, value_name = "dir", allow_hyphen_values = true, requires = "map")]
    pub company: Vec<PathBuf>,
}

/// Parses the arguments after the program name through the derive tree ([`tree::Cli`]) and checks the rules of the
/// subcommand's line that the tree's relations cannot say ([`tree::Leaf::check`]).
///
/// # Errors
///
/// clap's error: a usage error (exit 2), or the help or the version (exit 0), to print as clap renders it.
pub fn parse<I, T>(args: I) -> Result<Top, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    use clap::Parser as _;
    let line = std::iter::once(std::ffi::OsString::from("ew")).chain(args.into_iter().map(Into::into));
    let cli = tree::Cli::try_parse_from(line)?;
    cli.command.check()?;
    Ok(cli.command)
}

/// The exit code of [`parse`]'s error `e` for the arguments `args` after the program name: clap's (2 for a usage
/// error, 0 for the help and the version), but 129 for a usage error of `merge-driver`, since git reads any code up to
/// 128 from a merge driver as "conflicts written" (ADR-0012 amendment 2).
pub fn error_exit_code(args: &[std::ffi::OsString], e: &clap::Error) -> i32 {
    if e.use_stderr() && args.first().is_some_and(|word| word == "merge-driver") {
        merge_driver::INTERNAL
    } else {
        e.exit_code()
    }
}

/// The clap command of the derive tree: the source of the CLI catalogue's paths and flags.
#[cfg_attr(not(test), allow(dead_code))]
pub fn command() -> clap::Command {
    use clap::CommandFactory as _;
    tree::Cli::command()
}

/// The CLI catalogue's entries: every subcommand path of the tree in declaration order, its flags read from clap's
/// command and its exits and result from its [`tree::Leaf`].
#[cfg_attr(not(test), allow(dead_code))]
pub fn entries() -> Vec<crate::catalogue::Entry> {
    let mut cmd = command();
    cmd.build();
    let mut out = Vec::new();
    Top::entries(&cmd, &mut Vec::new(), &mut out);
    out
}

/// Runs the subcommand of a parsed line and returns its exit code (0 success, 1 model check failed or `--strict`
/// violated, 2 usage, I/O and import errors).
pub fn run(cmd: Top) -> i32 {
    match cmd {
        Top::Symbol(a) => symbol::run(a),
        Top::Import(i) => match i {
            Import::Qet(a) => run_import_qet(&a),
            Import::Parts(a) => run_import_parts(&a),
            Import::Csv(a) => run_import_csv(&a),
            Import::Table(a) => run_import_table(&a),
            Import::Dxf(a) => dxf::run_import_dxf(&a),
            Import::Pdf(a) => dxf::run_import_pdf(&a),
            Import::Dwg(a) => dxf::run_import_dwg(&a),
            Import::PlcAml(a) => run_plc_import(results::ImportPlcResult::AML, &a.plc),
            Import::PlcXlsx(a) => run_plc_import(results::ImportPlcResult::XLSX, &a.plc),
            Import::InstrumentList(a) => run_instrument_list(&a),
            Import::Edz(a) | Import::PartsXml(a) => eplan::run_import_eplan_parts(&a),
            Import::EplanReport(a) => eplan::run_import_eplan_report(&a),
        },
        Top::Check(a) => run_check(&a),
        Top::Sql(a) => match &a.sub {
            Some(SqlSub::Dump(d)) => run_sql_dump(d),
            Some(SqlSub::Schema(_)) => {
                print!("{}", ew_api::sql::schema::schema_text());
                0
            }
            None => run_sql(&a),
        },
        Top::Export(e) => match e {
            ExportCli::Cables(a) => run_rows(a.rows.export("cables", ExportKind::Cables)),
            ExportCli::Bom(a) => run_rows(a.export()),
            ExportCli::Wires(a) => run_rows(a.rows.export("wires", ExportKind::Wires)),
            ExportCli::Terminals(a) => run_rows(a.export()),
            ExportCli::Report(a) => run_rows(a.export()),
            ExportCli::Table(a) => run_export_nav_table(&a),
            ExportCli::Pdf(a) => run_pages(a.pages.export(PageFormat::Pdf)),
            ExportCli::Svg(a) => run_pages(a.pages.export(PageFormat::Svg)),
            ExportCli::Dxf(a) => run_pages(a.export()),
            ExportCli::Ewz(a) => run_export_ewz(&a),
            ExportCli::Png(a) => run_pages(a.export()),
            ExportCli::PlcAml(a) => run_plc_export("plc-aml", &a.plc),
            ExportCli::PlcXlsx(a) => run_plc_export("plc-xlsx", &a.plc),
            ExportCli::Plcopen(a) => run_plc_export("plcopen", &a.plc),
        },
        Top::Plc(PlcCli::Status(a)) => run_plc_status(&a),
        Top::Extract(a) => run_extract(&a),
        Top::Print(a) => run_print(&a),
        Top::Render(a) => run_render(&a),
        Top::Apply(a) => run_apply(&a),
        Top::Fmt(a) => project_files::run_fmt(&a),
        Top::Recover(a) => project_files::run_recover(&a),
        Top::Upgrade(a) => project_files::run_upgrade(&a),
        Top::UpdateCopies(a) => run_update_copies(&a),
        Top::Project(ProjectCli::Checks(ChecksCli::Set(a))) => run_checks_set(&a),
        Top::Project(ProjectCli::Assets(AssetsCli::Purge(a))) => run_assets_purge(&a),
        Top::Revision(r) => run_revision(&match r {
            RevisionCli::Approve(a) => a.revision(),
            RevisionCli::Release(a) => a.revision(),
            RevisionCli::Withdraw(a) => a.revision(),
        }),
        Top::VerifyDerived(a) => run_verify_derived(&a),
        Top::VerifyMigration(a) => run_verify_migration(&a),
        Top::Query(a) => query::run_query(&a),
        Top::Queries(a) => query::run_queries(a.json),
        Top::Layout(a) => layout::run_layout(&a),
        Top::Route(a) => route::run_route(&a),
        Top::Lib(a) => run_lib(&a),
        Top::Commands(a) => run_commands(a.json),
        Top::Schema(a) => run_schema(&a.name),
        Top::Merge(MergeCli::Resolve(a)) => merge_resolve::run_merge_resolve(&a),
        Top::MergeDriver(a) => merge_driver::run_merge_driver(&a),
        Top::Diff(a) => diff::run_diff(&a),
        Top::Git(GitCli::Setup(a)) => collab::run_git_setup(&a),
    }
}

/// Runs a row export (`export cables|bom|wires|terminals|report`) of its checked line.
fn run_rows(line: Result<ExportArgs, clap::Error>) -> i32 {
    match line {
        Ok(a) => run_export(&a),
        Err(e) => usage(&e),
    }
}

/// Runs a page export (`export pdf|svg|png|dxf`) of its checked line.
fn run_pages(line: Result<ExportPagesArgs, clap::Error>) -> i32 {
    match line {
        Ok(a) => run_export_pages(&a),
        Err(e) => usage(&e),
    }
}

/// A debug build panics here when `EW_DEBUG_PANIC` names `site` (`sql`: `ew sql`'s statement; `sql-write`: the writing
/// of its result; `table-read`: `ew import table`'s read), inside the panic guard it tests (the guard at its call site); a release build
/// compiles none of it.
// The program boundary of a debug build's test fault, read at the site it fails; a release build has no such read.
#[allow(
    clippy::disallowed_methods,
    reason = "a debug build's test fault, read where it fails"
)]
fn debug_panic(site: &str) {
    #[cfg(debug_assertions)]
    if std::env::var_os("EW_DEBUG_PANIC").is_some_and(|v| v == site) {
        panic!("forced panic (EW_DEBUG_PANIC={site})");
    }
    #[cfg(not(debug_assertions))]
    let _ = site;
}

/// Reads at most `limit + 1` bytes of `path`, so a file over the reader's limit is refused by the reader's own text
/// without being read whole.
fn read_bounded(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Writes `bytes` to `path` all or nothing ([`ew_api::file::write_replacing`]), so a failed export leaves no
/// partial file (doc 09 section 4.3).
fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    ew_api::file::write_replacing(path, bytes)
}

fn json_text(v: serde_json::Result<serde_json::Value>) -> String {
    match v {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

/// `error: ` and `text`: a refusal of `ew`, for a writer a caller injects or a caller that prints it. With
/// [`warning_line`] and [`notice_line`] the one place the prefix of such a line is spelt.
fn error_line(text: impl std::fmt::Display) -> String {
    format!("error: {text}")
}

/// `warning: ` and `text`.
fn warning_line(text: impl std::fmt::Display) -> String {
    format!("warning: {text}")
}

/// `notice: ` and `text`.
fn notice_line(text: impl std::fmt::Display) -> String {
    format!("notice: {text}")
}

/// Prints [`error_line`] on stderr.
fn error(text: impl std::fmt::Display) {
    eprintln!("{}", error_line(text));
}

/// Prints [`warning_line`] on stderr.
fn warning(text: impl std::fmt::Display) {
    eprintln!("{}", warning_line(text));
}

/// Prints [`notice_line`] on stderr.
fn notice(text: impl std::fmt::Display) {
    eprintln!("{}", notice_line(text));
}

/// `n` with the singular or plural noun: `1 file`, `2 files`.
fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn write_report(path: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(path, text)
}
