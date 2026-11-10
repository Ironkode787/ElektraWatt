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
use ew_api::{BomMode, CmdLog, Export};

mod apply;
mod assets;
mod checks;
mod collab;
mod diff;
mod export;
mod import;
mod layout;
mod library;
mod merge_driver;
mod merge_resolve;
mod open;
mod print;
mod project_files;
mod query;
mod revision;
mod route;
mod sql;
mod symbol;
#[cfg(test)]
mod tests;
mod tree;

use apply::*;
use assets::*;
use checks::*;
use export::*;
use import::*;
use library::*;
use open::*;
use print::*;
use revision::*;
use sql::*;

/// A parsed command line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Command {
    /// Portable symbol authoring for people and automation.
    Symbol(symbol::SymbolArgs),
    /// Import a QElectroTech project.
    ImportQet(ImportQetArgs),
    /// `ew import parts`.
    ImportParts(ImportPartsArgs),
    /// `ew import csv`: golden-dialect tables as a new project (WP-S1.22 part 3, FT-27).
    ImportCsv(ImportCsvArgs),
    /// `ew import table`: a round-trip table read back and its changed cells applied (WP-S3.08 D11).
    ImportTable(ImportTableArgs),
    /// Check a project.
    Check(CheckArgs),
    /// Run a read-only query on the SQL projection.
    Sql(SqlArgs),
    /// Write the SQL projection as a SQLite file.
    SqlDump(SqlDumpArgs),
    /// Print the SQL schema.
    SqlSchema,
    /// Write a report's rows as CSV.
    Export(ExportArgs),
    /// `ew export table`: a navigator's table as CSV or XLSX for a round trip (WP-S3.08 D7, D8).
    ExportTable(ExportTableArgs),
    /// Compare the memoised derived data with a cold recomputation.
    VerifyDerived(VerifyArgs),
    /// Write the pages as PDF, SVG or PNG.
    ExportPages(ExportPagesArgs),
    /// `ew print`: the print set through an adapter, or to a file (WP-S3.07 part 2).
    Print(PrintArgs),
    /// `ew render --page <folio> --png`: one page as PNG (WP-S1.22 D7).
    Render(RenderArgs),
    /// `ew export ewz`: the project's saved generation as an `.ewz` (WP-S1.17 D8).
    ExportEwz(ExportEwzArgs),
    /// `ew extract`: an `.ewz` into a new project directory (WP-S1.17 D9).
    Extract(ExtractArgs),
    /// Replay a command log.
    Apply(ApplyArgs),
    /// `ew fmt`: the project files in canonical form (WP-S1.22 D9).
    Fmt(FmtArgs),
    /// `ew recover`: a crashed session's journal (WP-S1.22 D10, FT-252).
    Recover(RecoverArgs),
    /// `ew upgrade`: a project of an older format converted after a backup (WP-S1.17 D11).
    Upgrade(UpgradeArgs),
    /// `ew update-copies`: the damaged library copies updated from their libraries (FT-1499).
    UpdateCopies(UpdateCopiesArgs),
    /// Name the project's check schemes (`project checks set`, a human-only command).
    ChecksSet(ChecksSetArgs),
    /// Delete the asset files nothing names (`project assets purge`).
    AssetsPurge(AssetsPurgeArgs),
    /// `ew lib`: list, search, reindex and create libraries.
    Lib(LibArgs),
    /// `ew revision approve|release|withdraw` (WP-S1.18 parts 3 and 4).
    Revision(RevisionArgs),
    /// `ew commands [--json]`: the command registry (WP-S1.22 D6).
    Commands {
        /// Print the result as JSON (`--json`).
        json: bool,
    },
    /// `ew schema <name>`: a command's parameter schema or a result schema (WP-S1.22 D6).
    Schema(String),
    /// `ew query <name> <key> [--explain]`: one public query (WP-S1.22 D11, D12).
    Query(query::QueryArgs),
    /// `ew queries [--json]`: the query registry (WP-S1.22 D11).
    Queries {
        /// Print the result as JSON (`--json`).
        json: bool,
    },
    /// `ew layout [--page <folio>] [--json]`: the layout measures (WP-S2.19 part 1, ID-1498).
    Layout(layout::LayoutArgs),
    /// `ew route [--page <folio>] [--selection <ids>] [--dry-run] [--json]`: the page router (WP-S2.19 D13).
    Route(route::RouteArgs),
    /// `ew merge resolve`: the conflict records a merge left, resolved (WP-S1.17 D19).
    MergeResolve(merge_resolve::MergeResolveArgs),
    /// `ew diff` (WP-S1.17 D26, D27).
    Diff(diff::DiffArgs),
    /// `ew git setup` (WP-S1.17 D28).
    GitSetup(collab::GitSetupArgs),
    /// `ew merge-driver %O %A %B %L %P`: the merge driver for project files (WP-S1.17 D15).
    MergeDriver(merge_driver::MergeDriverArgs),
}

/// What `ew revision` does.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RevisionAction {
    /// `revision approve --as <role>`: the checker's sign-off on the open revision.
    Approve {
        /// The checker's party role (`--as`).
        role: String,
    },
    /// `revision release <label> [--archive-dir <dir>]`: the atomic release with the as-built archive (WP-S1.18
    /// part 4), or settling an incomplete one.
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

/// What `ew lib` does.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LibAction {
    /// `lib list`.
    List,
    /// `lib search`.
    Search {
        /// The typed text.
        text: String,
        /// The kinds (`--kind`); parts when empty.
        kinds: Vec<String>,
        /// Include deprecated and withdrawn items (`--all`).
        all: bool,
        /// The most hits (`--limit`, 0: all).
        limit: u32,
    },
    /// `lib index --rebuild`.
    IndexRebuild,
    /// `lib create`.
    Create {
        /// The new library's directory.
        dir: PathBuf,
        /// `--key`.
        key: String,
        /// `--name`.
        name: String,
        /// `--licence`.
        licence: String,
    },
    /// `lib import parts`.
    ImportParts {
        /// The target library's key or ID.
        library: String,
        /// The parts table.
        file: PathBuf,
        /// The column map (`--map`).
        map: PathBuf,
        /// The import date (`--as-of`).
        as_of: ew_api::catalog::Date,
        /// Write changed parts as the next patch version (`--bump`).
        bump: bool,
        /// Take over a stale library lock (`--take-over`).
        take_over: bool,
    },
}

/// The arguments of `ew lib`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LibArgs {
    /// The subcommand.
    pub action: LibAction,
    /// The company library paths (`--company`, repeatable).
    pub company: Vec<PathBuf>,
    /// Print JSON (`--json`).
    pub json: bool,
}

/// The arguments of `ew fmt`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FmtArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// List the files a format would rewrite and write nothing (`--check`, a reader).
    pub check: bool,
    /// Discard the broken lines (EW-CORE-005 at line level) after an `.ewz` backup (`--repair`; WP-S1.17, decision log
    /// ID-1185); never with `check`.
    pub repair: bool,
    /// Print the result as JSON (`--json`).
    pub json: bool,
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

/// The arguments of `ew recover`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RecoverArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// What to do.
    pub action: RecoverAction,
    /// Print the result as JSON (`--json`; not with `--discard`).
    pub json: bool,
    /// `--yes` (with `--replay`): confirm the upgrade of a project in an older format without the typed word (FT-1336).
    pub yes: bool,
}

/// The arguments of `ew upgrade` (WP-S1.17 D11, decision log ID-1221 ruling 6).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UpgradeArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// `--check`: say whether an upgrade is needed and write nothing.
    pub check: bool,
    /// `--yes`: confirm without the typed word.
    pub yes: bool,
}

/// The arguments of `ew update-copies` (FT-1499, decision log ID-1486).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UpdateCopiesArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The company libraries (`--company`).
    pub company: Vec<PathBuf>,
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

/// The arguments of `ew project assets purge`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AssetsPurgeArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// List what would go and change nothing.
    pub preview: bool,
}

/// The arguments of `ew project checks set`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ChecksSetArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The online scheme (`--active`).
    pub active: Option<String>,
    /// The release scheme (`--release`).
    pub release: Option<String>,
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
}

/// The arguments of `ew render`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RenderArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The folio label (`--page`).
    pub page: String,
    /// The resolution (`--dpi`, default 150).
    pub dpi: u32,
    /// The PNG file (`--out`); `None`: `<folio>.png` in the current directory.
    pub out: Option<PathBuf>,
    /// Print the result as JSON (`--json`).
    pub json: bool,
}

/// The arguments of `ew export ewz` (WP-S1.17 D8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExportEwzArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The `.ewz` file (`--out`).
    pub out: PathBuf,
    /// Print the result as JSON (`--json`).
    pub json: bool,
}

/// The arguments of `ew extract` (WP-S1.17 D9).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExtractArgs {
    /// The `.ewz` file.
    pub file: PathBuf,
    /// The new project directory (`--into`); `None`: `<stem>.ew` beside the archive (doc 05 §12).
    pub into: Option<PathBuf>,
    /// Print the result as JSON (`--json`).
    pub json: bool,
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

/// The arguments of `ew print`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PrintArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The page selection (`--pages`, `--sections`).
    pub pages: PageSelection,
    /// The paper (`--paper`).
    pub paper: ew_api::output::print::Paper,
    /// Scale each page to the paper (`--fit`).
    pub fit: bool,
    /// Black and grey (`--mono`).
    pub mono: bool,
    /// The printer (`--printer`).
    pub printer: Option<String>,
    /// The adapter (`--via`, its platform default resolved).
    pub via: PrintVia,
    /// The print set's file (`--out`); `None`: printed.
    pub out: Option<PathBuf>,
    /// Print the result as JSON (`--json`).
    pub json: bool,
}

/// The arguments of `ew apply`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ApplyArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The command log.
    pub log: PathBuf,
    /// Replay on a copy and print the changes (`--preview`).
    pub preview: bool,
    /// Save only when every line commits (`--one-step`).
    pub one_step: bool,
    /// Print the result as JSON (`--json`).
    pub json: bool,
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
    /// The whole-project rows of a report kind (`export report --kind <kind>`, CSV only; WP-S3.06 D4).
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
    /// How the rows are written (`--format`, `--preset`, `--bom`, `--line-end`; WP-S3.08 D6, D7).
    pub format: RowFormat,
}

/// The arguments of `ew export table` (WP-S3.08 D7, D8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExportTableArgs {
    /// The navigator's word (`strips`).
    pub navigator: String,
    /// One strip of `strips`, by its tag (`--strip`).
    pub strip: Option<String>,
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The output file (`--out`); `None`: stdout.
    pub out: Option<PathBuf>,
    /// Print the result as JSON (`--json`).
    pub json: bool,
    /// How the table is written (`--format`, `--preset`, `--bom`, `--line-end`).
    pub format: RowFormat,
}

/// The arguments of `ew import table` (WP-S3.08 D7, D11).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportTableArgs {
    /// The round-trip file.
    pub file: PathBuf,
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The table's key (`--table`).
    pub table: Option<String>,
    /// The CSV's preset (`--preset`); `None`: from its separator.
    pub preset: Option<Preset>,
    /// `--take-file`.
    pub take_file: bool,
    /// `--apply`: commit and save.
    pub apply: bool,
    /// Print the result as JSON (`--json`).
    pub json: bool,
}

/// How a row export writes its rows (WP-S3.08 D7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowFormat {
    /// CSV in a preset (the default: the golden dialect `ew`).
    Csv(CsvOptions),
    /// An XLSX workbook (`--format xlsx`, with `--out`).
    Xlsx,
}

impl Default for RowFormat {
    fn default() -> RowFormat {
        RowFormat::Csv(CsvOptions::default())
    }
}

/// The arguments of `ew verify-derived`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VerifyArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// Print the result as JSON (`--json`).
    pub json: bool,
}

/// The result format of `ew sql`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SqlFormat {
    /// CSV: the golden dialect, or a preset (`--preset`).
    Csv,
    /// JSON (`ew.sql-result/1`).
    Json,
    /// An XLSX workbook (`--format xlsx`, with `--out`; FT-218).
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

/// The arguments of `ew sql`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SqlArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The query.
    pub query: SqlSource,
    /// The result format (`--format`).
    pub format: SqlFormat,
    /// The deadline in seconds (`--timeout`).
    pub timeout: Option<u64>,
    /// The output file (`--out`).
    pub out: Option<PathBuf>,
    /// The CSV preset and its overrides (`--preset`, `--bom`, `--line-end`; WP-S3.08 D6).
    pub csv: CsvOptions,
}

/// The arguments of `ew sql dump`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SqlDumpArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The SQLite file to write.
    pub out: PathBuf,
    /// Print the result as JSON (`--json`).
    pub json: bool,
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

/// The arguments of `ew check`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CheckArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The scheme name (`--scheme`); `None`: the project's scheme for the mode, else `pre-delivery`.
    pub scheme: Option<String>,
    /// The mode (`--mode`).
    pub mode: CheckMode,
    /// The output format (`--format`).
    pub format: CheckFormat,
    /// The output file (`--out`).
    pub out: Option<PathBuf>,
    /// The language of the messages (`--lang`, a shipped language; default en-GB).
    pub lang: String,
    /// Only the project-integrity check (`--integrity`, FT-191; WP-S1.17 D29).
    pub integrity: bool,
    /// With `integrity`: the project counts as just merged (`--after-merge`).
    pub after_merge: bool,
}

/// The arguments of `ew import qet`.
#[derive(clap::Args, Clone, PartialEq, Eq, Debug)]
pub struct ImportQetArgs {
    /// The QElectroTech project file.
    #[arg(value_name = "file.qet")]
    pub file: PathBuf,
    /// The starter content directory.
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
    /// The parts table: `parts.csv` of the golden dialect, or with `--map` a supplier's CSV or XLSX table.
    #[arg(value_name = "table")]
    pub file: PathBuf,
    /// The project directory.
    #[arg(long, value_name = "dir", allow_hyphen_values = true)]
    pub project: Option<PathBuf>,
    /// Print the result as JSON (`ew.import-parts-result/1`).
    #[arg(long, overrides_with = "json")]
    pub json: bool,
    /// The column map (a TOML file): a supplier's table instead of the golden dialect.
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

/// Parses the arguments after the program name through the derive tree ([`tree::Cli`]).
///
/// # Errors
///
/// clap's error: a usage error (exit 2), or the help or the version (exit 0), to print as clap renders it.
pub fn parse<I, T>(args: I) -> Result<Command, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    use clap::Parser as _;
    let line = std::iter::once(std::ffi::OsString::from("ew")).chain(args.into_iter().map(Into::into));
    tree::Cli::try_parse_from(line)?.into_command()
}

/// The exit code of [`parse`]'s error `e` for the arguments `args` after the program name: clap's (2 for a usage
/// error, 0 for the help and the version), but 129 for a usage error of `merge-driver`, since git reads any code up to
/// 128 from a merge driver as "conflicts written" (ADR-0012 amendment 2; decision log ID-1273).
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

/// Runs `cmd` and returns the exit code (0 success, 1 model check failed or `--strict` violated, 2 usage, I/O and
/// import errors).
pub fn run(cmd: Command) -> i32 {
    match cmd {
        Command::Symbol(a) => symbol::run(a),
        Command::ImportQet(a) => run_import_qet(&a),
        Command::ImportParts(a) => run_import_parts(&a),
        Command::ImportCsv(a) => run_import_csv(&a),
        Command::ImportTable(a) => run_import_table(&a),
        Command::Check(a) => run_check(&a),
        Command::Sql(a) => run_sql(&a),
        Command::SqlDump(a) => run_sql_dump(&a),
        Command::Export(a) => run_export(&a),
        Command::ExportTable(a) => run_export_nav_table(&a),
        Command::VerifyDerived(a) => run_verify_derived(&a),
        Command::ExportPages(a) => run_export_pages(&a),
        Command::Print(a) => run_print(&a),
        Command::Render(a) => run_render(&a),
        Command::ExportEwz(a) => run_export_ewz(&a),
        Command::Extract(a) => run_extract(&a),
        Command::Apply(a) => run_apply(&a),
        Command::Fmt(a) => project_files::run_fmt(&a),
        Command::Recover(a) => project_files::run_recover(&a),
        Command::Upgrade(a) => project_files::run_upgrade(&a),
        Command::UpdateCopies(a) => run_update_copies(&a),
        Command::ChecksSet(a) => run_checks_set(&a),
        Command::AssetsPurge(a) => run_assets_purge(&a),
        Command::Lib(a) => run_lib(&a),
        Command::Revision(a) => run_revision(&a),
        Command::Commands { json } => run_commands(json),
        Command::Schema(name) => run_schema(&name),
        Command::Query(a) => query::run_query(&a),
        Command::Queries { json } => query::run_queries(json),
        Command::Layout(a) => layout::run_layout(&a),
        Command::Route(a) => route::run_route(&a),
        Command::MergeResolve(a) => merge_resolve::run_merge_resolve(&a),
        Command::Diff(a) => diff::run_diff(&a),
        Command::GitSetup(a) => collab::run_git_setup(&a),
        Command::MergeDriver(a) => merge_driver::run_merge_driver(&a),
        Command::SqlSchema => {
            print!("{}", ew_api::sql::schema::schema_text());
            0
        }
    }
}

/// A debug build panics here when `EW_DEBUG_PANIC` names `site` (`sql`: `ew sql`'s statement; `sql-write`: the writing
/// of its result; `table-read`: `ew import table`'s read), inside the panic guard it tests (WP-S6.04 D15, SE-9: the guard at its call site); a release build
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

/// Writes `bytes` to `<path>.tmp` and renames it to `path`, so a failed export leaves no partial file (doc 09
/// section 4.3).
fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = sibling(path, ".tmp");
    write_synced(&tmp, bytes)
        .and_then(|()| std::fs::rename(&tmp, path))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
}

/// `path` with `suffix` appended to its last component.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Writes `bytes` to `path` and flushes them to the disk before returning.
fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

fn json_text(v: serde_json::Result<serde_json::Value>) -> String {
    match v {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

/// `n` with the singular or plural noun: `1 file`, `2 files`.
fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn write_report(path: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(path, text)
}
