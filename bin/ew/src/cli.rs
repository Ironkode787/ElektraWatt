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

mod collab;
mod diff;
mod merge_driver;
mod merge_resolve;
mod project_files;
mod query;
mod tree;

/// A parsed command line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Command {
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
        Command::ChecksSet(a) => run_checks_set(&a),
        Command::AssetsPurge(a) => run_assets_purge(&a),
        Command::Lib(a) => run_lib(&a),
        Command::Revision(a) => run_revision(&a),
        Command::Commands { json } => run_commands(json),
        Command::Schema(name) => run_schema(&name),
        Command::Query(a) => query::run_query(&a),
        Command::Queries { json } => query::run_queries(json),
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

/// The nearest ancestor of `start` (itself included) that holds `manifest.json` (doc 17 §3.1).
fn find_project(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|d| d.join("manifest.json").is_file())
        .map(Path::to_path_buf)
}

/// The explicit project directory, else the one found from the current directory, for every subcommand but the readers
/// ([`reader_dir`]): an `.ewz` is refused with D9's text (WP-S1.17; it opens read-only).
fn project_dir(explicit: Option<&PathBuf>) -> Option<PathBuf> {
    let dir = reader_dir(explicit)?;
    if ew_api::ewz::is_ewz(&ew_api::OsVfs, &dir) {
        eprintln!("{}", ewz_refusal(&dir));
        return None;
    }
    Some(dir)
}

/// The refusal of a subcommand that is not a reader, given an `.ewz` as its project (WP-S1.17 D9).
fn ewz_refusal(path: &Path) -> String {
    format!("error: {}: {}", path.display(), ew_api::ewz::READ_ONLY)
}

/// The explicit project, else the directory found from the current directory, for a reader (`check`, `sql`, the row
/// and page exports, `export table`, `print`, `render`, `verify-derived`, `query`, `fmt --check`, `upgrade --check`):
/// an `.ewz` is taken as it is and opened read-only from memory by `Session::open_reader` (WP-S1.17 D9, ID-1175 ruling
/// 5, ID-1230 ruling 5).
fn reader_dir(explicit: Option<&PathBuf>) -> Option<PathBuf> {
    match explicit {
        Some(d) => Some(d.clone()),
        None => {
            let found = std::env::current_dir().ok().and_then(|cwd| find_project(&cwd));
            if found.is_none() {
                eprintln!("error: no project directory given and none found above the current directory");
            }
            found
        }
    }
}

/// Opens the project read-only and builds its SQL projection (exit code 2 on failure).
fn open_projection(project: Option<&PathBuf>) -> Result<ew_api::sql::Projection, i32> {
    open_projection_and_session(project).map(|(projection, _)| projection)
}

/// [`open_projection`] with the session it was built from (the metadata of `ew sql --format xlsx`).
fn open_projection_and_session(
    project: Option<&PathBuf>,
) -> Result<(ew_api::sql::Projection, ew_api::Session), i32> {
    let dir = reader_dir(project).ok_or(2)?;
    let (session, opened) = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "sql") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return Err(2);
        }
    };
    let report = session
        .check(&ew_api::checks::PRE_DELIVERY, CheckMode::Release)
        .with_integrity(ew_api::integrity_items(&opened));
    let projection = session.sql_projection(Some(&report)).map_err(|e| {
        eprintln!("error: {e}");
        2
    })?;
    Ok((projection, session))
}

fn run_sql(a: &SqlArgs) -> i32 {
    let sql = match &a.query {
        SqlSource::Text(t) => t.clone(),
        SqlSource::File(f) => match std::fs::read_to_string(f) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("error: {}: {e}", f.display());
                return 2;
            }
        },
    };
    let (projection, session) = match open_projection_and_session(a.project.as_ref()) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let mut profile = ew_api::sql::Profile::EW_SQL;
    if let Some(n) = a.timeout {
        profile = profile.with_deadline(std::time::Duration::from_secs(n));
    }
    // FT-1379: the result's writing, which a hostile SELECT drives as much as the statement, runs inside the guard
    // too (the workbook's in `write_sql_xlsx`).
    let (set, text) = match guarded_query(|| {
        debug_panic("sql");
        let set = projection.reader(profile).and_then(|r| r.query(&sql))?;
        debug_panic("sql-write");
        let text = match a.format {
            SqlFormat::Csv if a.csv.preset == Preset::Ew => Some(ew_api::output::restyle(
                &set.to_csv(),
                a.csv.writes_bom(),
                a.csv.writes_line_end(),
            )),
            SqlFormat::Csv => Some(ew_api::output::table_csv(
                &ew_api::output::sql_table(&set),
                &a.csv,
            )),
            SqlFormat::Json => Some(format!("{:#}\n", set.to_json())),
            SqlFormat::Xlsx => None,
        };
        Ok((set, text))
    }) {
        Ok(s) => s,
        Err(line) => {
            eprintln!("{line}");
            return 2;
        }
    };
    let Some(text) = text else {
        return write_sql_xlsx(a, &session, &set);
    };
    match &a.out {
        Some(path) => match write_report(path, &text) {
            Ok(()) => println!("result written to {}", path.display()),
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
        },
        None => {
            print!("{text}");
            eprintln!("{} rows", set.rows.len());
        }
    }
    0
}

/// S1 of WP-S6.04 (D15): `ew sql`'s line when the statement panicked.
const QUERY_PANICKED: &str = "error: the query stopped on an internal error; nothing was changed";

/// Runs `ew sql`'s statement, `run`, inside `catch_unwind` (WP-S6.04 D15, NFR-REL-12): its result, or the line to print
/// (`error: <the SQL error>`, or [`QUERY_PANICKED`] when it panicked). The query reads a projection image only, so a
/// panic changes nothing.
fn guarded_query<T>(run: impl FnOnce() -> Result<T, ew_api::sql::SqlError>) -> Result<T, String> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(Ok(t)) => Ok(t),
        Ok(Err(e)) => Err(format!("error: {e}")),
        Err(_) => Err(QUERY_PANICKED.to_owned()),
    }
}

/// A debug build panics here when `EW_DEBUG_PANIC` names `site` (`sql`: `ew sql`'s statement; `sql-write`: the writing
/// of its result; `table-read`: `ew import table`'s read), inside the panic guard it tests (WP-S6.04 D15, SE-9: the guard at its call site); a release build
/// compiles none of it.
// The program boundary of a debug build's test fault, read at the site it fails; a release build has no such read.
#[allow(clippy::disallowed_methods)]
fn debug_panic(site: &str) {
    #[cfg(debug_assertions)]
    if std::env::var_os("EW_DEBUG_PANIC").is_some_and(|v| v == site) {
        panic!("forced panic (EW_DEBUG_PANIC={site})");
    }
    #[cfg(not(debug_assertions))]
    let _ = site;
}

/// `ew import table`'s line for a refused file: `error: ` and the refusal with its first letter in lower case, as every
/// `ew` error line (S2 of WP-S6.04 for a read that panicked; ruling of C51-p1#2).
fn import_table_error_line(e: &ew_api::output::TableImportError) -> String {
    let text = e.to_string();
    let mut chars = text.chars();
    let first = chars
        .next()
        .map(|c| c.to_lowercase().collect::<String>())
        .unwrap_or_default();
    format!("error: {first}{}", chars.as_str())
}

/// `ew sql --format xlsx --out <file>` (FT-218): the result as the one sheet `sql` of a workbook, written atomically.
fn write_sql_xlsx(a: &SqlArgs, session: &ew_api::Session, set: &ew_api::sql::ResultSet) -> i32 {
    let Some(path) = &a.out else {
        eprintln!("error: {XLSX_NEEDS_OUT}");
        return 2;
    };
    let date = match source_date() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let meta = session.workbook_meta(date);
    // FT-1379: the workbook is written from the result inside the statement's guard.
    let bytes = match guarded_query(|| {
        debug_panic("sql-write");
        Ok(ew_api::output::xlsx(&[ew_api::output::sql_table(set)], &meta))
    }) {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => {
            eprintln!("error: {e}");
            return 2;
        }
        Err(line) => {
            eprintln!("{line}");
            return 2;
        }
    };
    if let Err(e) = write_atomically(path, &bytes) {
        eprintln!("error: {}: {e}", path.display());
        return 2;
    }
    println!("result written to {}", path.display());
    0
}

/// The refusal of `--format xlsx` without `--out` (WP-S3.08 D7).
pub const XLSX_NEEDS_OUT: &str = "`--format xlsx` writes a file: give `--out <file.xlsx>`";

/// The message of a refused export (FT-465). A project without the report block of the export's kind has no row source:
/// the export is the rows of that block (doc 17 §3.3), so it is refused, not empty, and the message names the block, the
/// export and where the block comes from. A block that does not derive keeps the report layer's own text.
fn export_refusal(what: Export, e: &ew_api::ExportError) -> String {
    match e {
        ew_api::ExportError::NoBlock(kind) => {
            let (export, made) = match what {
                Export::Cables => (
                    "the cable list",
                    "the company-standard wizard creates it as folio 906",
                ),
                Export::Bom(_) => (
                    "the bill of materials",
                    "the company-standard wizard creates it as folio 901",
                ),
            };
            format!(
                "the project has no report block of kind `{kind}`: {export} is the rows of that block ({made}; a project from the built-in template has none), so there is nothing to export"
            )
        }
        other => other.to_string(),
    }
}

fn run_export(a: &ExportArgs) -> i32 {
    let Some(dir) = reader_dir(a.project.as_ref()) else {
        return 2;
    };
    let (session, _) = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "export") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let options = match a.format {
        RowFormat::Csv(options) if options.preset == Preset::Ew => options,
        _ if a.kind == (ExportKind::Report { pdf: true }) => CsvOptions::default(),
        _ => return run_export_table(a, &session),
    };
    let rev = session.world().rev.0;
    let rows = match (&a.kind, &a.block) {
        // The default blocks of `cables` and `bom`: the report layer's export, as before `--block`.
        (ExportKind::Cables, None) | (ExportKind::Bom(_), None) => {
            let what = match a.kind {
                ExportKind::Bom(mode) => Export::Bom(mode),
                _ => Export::Cables,
            };
            match session.export(what) {
                Ok(e) => (e.csv, e.rows, Vec::new()),
                Err(e) => {
                    eprintln!("error: {}", export_refusal(what, &e));
                    return 2;
                }
            }
        }
        (ExportKind::Report { pdf: true }, Some(block)) => return run_export_report_pdf(a, &session, block),
        (kind, block) => {
            let what = match kind {
                ExportKind::Cables => RowExport::Cables,
                ExportKind::Bom(mode) => RowExport::Bom(*mode),
                ExportKind::Wires => RowExport::Wires,
                ExportKind::Terminals(strip) => RowExport::Terminals { strip: strip.clone() },
                ExportKind::Report { .. } => RowExport::Report,
                ExportKind::ReportKind(name) => RowExport::Kind(name.clone()),
            };
            match ew_api::output::row_export(&session.query(), &what, block.as_deref()) {
                Ok(r) => {
                    for notice in &r.notices {
                        eprintln!("{notice}");
                    }
                    if let ExportKind::Report { .. } | ExportKind::ReportKind(_) = kind {
                        return finish_export_report_csv(a, &r, &options, rev);
                    }
                    (r.csv, r.rows, r.strips)
                }
                Err(e) => {
                    eprintln!("error: {}", rows_refusal(&e));
                    return 2;
                }
            }
        }
    };
    let (csv, count, strips) = rows;
    let csv = restyle(&csv, &options);
    if write_rows(a, &csv, count) != 0 {
        return 2;
    }
    if a.json {
        let summary = RowsSummary {
            rows: count,
            strips,
            out: a.out.as_ref().map(|p| p.display().to_string()),
            csv: a.out.is_none().then_some(csv),
            format: "csv",
            preset: Some(options.preset.name()),
            rev,
        };
        print!("{}", rows_json(&a.kind, summary));
    }
    0
}

/// `csv` (the golden dialect) with the byte-order mark and line end of `options` (WP-S3.08 D6).
fn restyle(csv: &str, options: &CsvOptions) -> String {
    ew_api::output::restyle(csv, options.writes_bom(), options.writes_line_end())
}

/// What the `--json` result of `export cables|bom|wires|terminals` says (WP-S3.08 TX-7).
struct RowsSummary {
    rows: usize,
    strips: Vec<String>,
    out: Option<String>,
    csv: Option<String>,
    format: &'static str,
    preset: Option<&'static str>,
    rev: u64,
}

/// The `--json` result of a row export of `kind` other than `report`.
fn rows_json(kind: &ExportKind, s: RowsSummary) -> String {
    match kind {
        ExportKind::Terminals(_) => results::to_text(&results::ExportTerminalsResult {
            schema: results::EXPORT_TERMINALS.to_owned(),
            rows: s.rows,
            strips: s.strips,
            out: s.out,
            csv: s.csv,
            format: Some(s.format.to_owned()),
            preset: s.preset.map(str::to_owned),
            rev: Some(s.rev),
        }),
        kind => results::to_text(&results::ExportRowsResult {
            schema: match kind {
                ExportKind::Cables => results::EXPORT_CABLES,
                ExportKind::Bom(_) => results::EXPORT_BOM,
                _ => results::EXPORT_WIRES,
            }
            .to_owned(),
            rows: s.rows,
            out: s.out,
            csv: s.csv,
            format: Some(s.format.to_owned()),
            preset: s.preset.map(str::to_owned),
            rev: Some(s.rev),
        }),
    }
}

/// A row export in XLSX or a CSV preset other than `ew` (WP-S3.08 D6, D7): [`ew_api::output::row_output`] over the
/// row choice of the golden path, the file written atomically (`<file>.tmp`, synced, renamed).
fn run_export_table(a: &ExportArgs, session: &ew_api::Session) -> i32 {
    let what = match &a.kind {
        ExportKind::Cables => RowExport::Cables,
        ExportKind::Bom(mode) => RowExport::Bom(*mode),
        ExportKind::Wires => RowExport::Wires,
        ExportKind::Terminals(strip) => RowExport::Terminals { strip: strip.clone() },
        ExportKind::Report { .. } => RowExport::Report,
        ExportKind::ReportKind(name) => RowExport::Kind(name.clone()),
    };
    let (format, preset) = match a.format {
        RowFormat::Csv(options) => (RowsFormat::Csv(options), Some(options.preset.name())),
        RowFormat::Xlsx => {
            let date = match source_date() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            (RowsFormat::Xlsx(session.workbook_meta(date)), None)
        }
    };
    let file = match ew_api::output::row_output(&session.query(), &what, a.block.as_deref(), &format) {
        Ok(f) => f,
        Err(e) => {
            // `cables` and `bom` without `--block` refuse a project without the block as the golden path does.
            let text = match (&e, &a.kind, &a.block) {
                (ew_api::output::RowsError::NoBlock(kind), ExportKind::Cables, None) => {
                    export_refusal(Export::Cables, &ew_api::ExportError::NoBlock(kind))
                }
                (ew_api::output::RowsError::NoBlock(kind), ExportKind::Bom(mode), None) => {
                    export_refusal(Export::Bom(*mode), &ew_api::ExportError::NoBlock(kind))
                }
                (e, _, _) => rows_refusal(e),
            };
            eprintln!("error: {text}");
            return 2;
        }
    };
    for notice in &file.notices {
        eprintln!("{notice}");
    }
    let (bytes, text, format_name) = match file.output {
        RowsOutput::Csv(csv) => (csv.clone().into_bytes(), Some(csv), "csv"),
        RowsOutput::Xlsx(bytes) => (bytes, None, "xlsx"),
    };
    match &a.out {
        Some(path) => {
            if let Err(e) = write_atomically(path, &bytes) {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
            if !a.json {
                println!("export written to {}", path.display());
            }
        }
        // `--format xlsx` needs `--out` (the lowering refuses it without); a CSV goes to stdout.
        None => match &text {
            Some(csv) if !a.json => {
                print!("{csv}");
                eprintln!("{} rows", file.rows);
            }
            Some(_) => {}
            None => {
                eprintln!("error: {XLSX_NEEDS_OUT}");
                return 2;
            }
        },
    }
    if a.json {
        let out = a.out.as_ref().map(|p| p.display().to_string());
        let csv = if a.out.is_none() { text } else { None };
        let rev = session.world().rev.0;
        let json = match &a.kind {
            ExportKind::Report { .. } | ExportKind::ReportKind(_) => {
                results::to_text(&results::ExportReportResult {
                    schema: results::EXPORT_REPORT.to_owned(),
                    block: file.blocks.first().cloned().unwrap_or_default(),
                    kind: file.kind.clone(),
                    format: format_name.to_owned(),
                    rows: Some(file.rows),
                    folios: None,
                    out,
                    csv,
                    preset: preset.map(str::to_owned),
                    rev: Some(rev),
                })
            }
            kind => rows_json(
                kind,
                RowsSummary {
                    rows: file.rows,
                    strips: file.strips,
                    out,
                    csv,
                    format: format_name,
                    preset,
                    rev,
                },
            ),
        };
        print!("{json}");
    }
    0
}

/// `ew export table` (WP-S3.08 D7, D8): a navigator's table unfiltered, as CSV in a preset or as a workbook with the
/// round-trip sheets (`_ew`, and `_ew_base` with its digest), the file written atomically.
fn run_export_nav_table(a: &ExportTableArgs) -> i32 {
    let Some(dir) = reader_dir(a.project.as_ref()) else {
        return 2;
    };
    let (session, _) = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "export") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let table = match session
        .navigator_table_key(&a.navigator, a.strip.as_deref())
        .and_then(|key| session.table_export(&key))
    {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let (bytes, text, format, preset) = match a.format {
        RowFormat::Csv(options) => {
            let exporter = ew_api::output::CsvExporter { options };
            let csv = match ew_api::output::export_tables(
                &exporter,
                session.world().rev.0,
                std::slice::from_ref(&table),
            ) {
                Ok((bytes, _)) => String::from_utf8(bytes).unwrap_or_default(),
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            (
                csv.clone().into_bytes(),
                Some(csv),
                "csv",
                Some(options.preset.name()),
            )
        }
        RowFormat::Xlsx => {
            let date = match source_date() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            let tables = std::slice::from_ref(&table);
            let meta = ew_api::output::with_base_digest(&session.workbook_meta(date), tables);
            let rev = meta.rev;
            match ew_api::output::export_tables(&ew_api::output::XlsxExporter { meta }, rev, tables) {
                Ok((b, _)) => (b, None, "xlsx", None),
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            }
        }
    };
    match (&a.out, &text) {
        (Some(path), _) => {
            if let Err(e) = write_atomically(path, &bytes) {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
            if !a.json {
                println!("export written to {}", path.display());
            }
        }
        (None, Some(csv)) if !a.json => {
            print!("{csv}");
            eprintln!("{} rows", table.rows.len());
        }
        (None, Some(_)) => {}
        // The lowering refuses `--format xlsx` without `--out`.
        (None, None) => {
            eprintln!("error: {XLSX_NEEDS_OUT}");
            return 2;
        }
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ExportTableResult {
                schema: results::EXPORT_TABLE.to_owned(),
                table: table.key.clone(),
                format: format.to_owned(),
                preset: preset.map(str::to_owned),
                rows: table.rows.len(),
                editable: table
                    .columns
                    .iter()
                    .filter(|c| c.editable)
                    .map(|c| c.key.clone())
                    .collect(),
                out: a.out.as_ref().map(|p| p.display().to_string()),
                csv: if a.out.is_none() { text } else { None },
                rev: session.world().rev.0,
            })
        );
    }
    0
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

/// `ew import table` (WP-S3.08 D11): the preview of a round-trip file against the project, and with `--apply` its one
/// `fields.set_many` committed and the project saved.
fn run_import_table(a: &ImportTableArgs) -> i32 {
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let bytes = match read_bounded(&a.file, ew_api::catalog::table_limits().max_input_bytes) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {}: {e}", a.file.display());
            return 2;
        }
    };
    let options = ew_api::output::ImportTableOptions {
        table: a.table.clone(),
        preset: a.preset,
        take_file: a.take_file,
    };
    let origin = ew_api::Origin::Cli {
        subcommand: "import table".to_owned(),
        confirmed: false,
    };
    let mut session = if a.apply {
        match open_for_writing(&dir, origin.clone()) {
            Ok(s) => s,
            Err(code) => return code,
        }
    } else {
        match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "import table") {
            Ok((s, _)) => s,
            Err(e) => {
                eprintln!("error: {}: {e}", dir.display());
                return 2;
            }
        }
    };
    let file = a.file.file_name().map_or_else(
        || a.file.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let import = match ew_api::output::guard_table_read(&file, || {
        debug_panic("table-read");
        session.table_import(&bytes, &options)
    }) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("{}", import_table_error_line(&e));
            session.close();
            return 2;
        }
    };
    let mut applied = false;
    if a.apply && import.command.is_some() {
        if let Err(e) = session.apply_table_import(&import, ew_api::Submit::from(origin)) {
            eprintln!("error: {e}");
            session.close();
            return 2;
        }
        if let Err(e) = session
            .save()
            .map(|r| warn_unreleased(r.unstable, &mut std::io::stderr()))
        {
            eprintln!("error: the save failed: {e:?}");
            session.close();
            return 2;
        }
        applied = true;
    }
    session.close();
    if a.json {
        print!("{}", import_table_json(&import, applied));
    } else {
        print!("{}", import_table_text(&import, a.apply, applied));
    }
    0
}

/// The text preview of `ew import table`: one line per changed cell, the notices, the findings, the missing rows and
/// the counts.
fn import_table_text(import: &ew_api::output::TableImport, apply: bool, applied: bool) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for notice in &import.notices {
        let _ = writeln!(out, "{notice}");
    }
    for cell in &import.cells {
        let base = cell
            .base
            .as_ref()
            .map_or_else(String::new, |b| format!("exported \"{b}\", "));
        let reason = cell
            .reason
            .as_ref()
            .map_or_else(String::new, |r| format!(" ({r})"));
        let _ = writeln!(
            out,
            "{}, {}: {base}project \"{}\", file \"{}\": {}{reason}",
            cell.row_label,
            cell.column_label,
            cell.ew,
            cell.file,
            cell.outcome.word()
        );
    }
    for finding in &import.findings {
        let _ = writeln!(out, "{}: {}", finding.rule, finding.message);
    }
    if !import.missing.is_empty() {
        let _ = writeln!(out, "not in the file (kept): {}", import.missing.join(", "));
    }
    let c = import.counts();
    let end = match (apply, applied) {
        (_, true) => "applied in one step; saved",
        (true, false) => "nothing to apply",
        (false, false) if import.command.is_some() => {
            "a preview: nothing changed (give `--apply` to apply it)"
        }
        (false, false) => "a preview: nothing to apply",
    };
    let _ = writeln!(
        out,
        "{} changed, {} kept, {} conflicts, {} refused, {} unknown, {} missing; {end}",
        c.changed, c.kept, c.conflicts, c.refused, c.unknown, c.missing
    );
    out
}

/// The `--json` result of `ew import table` ([`results::ImportTableResult`]).
fn import_table_json(import: &ew_api::output::TableImport, applied: bool) -> String {
    let c = import.counts();
    results::to_text(&results::ImportTableResult {
        schema: results::IMPORT_TABLE.to_owned(),
        table: import.table.clone(),
        three_way: import.three_way,
        notices: import.notices.clone(),
        changed: c.changed,
        kept: c.kept,
        conflicts: c.conflicts,
        refused: c.refused,
        unknown: c.unknown,
        cells: import
            .cells
            .iter()
            .map(|cell| results::ImportTableCell {
                row: cell.row.clone(),
                label: cell.row_label.clone(),
                column: cell.column.clone(),
                column_label: cell.column_label.clone(),
                base: cell.base.clone(),
                ew: cell.ew.clone(),
                file: cell.file.clone(),
                outcome: cell.outcome.word().to_owned(),
                reason: cell.reason.clone(),
            })
            .collect(),
        findings: import
            .findings
            .iter()
            .map(|f| results::ImportTableFinding {
                rule: f.rule.to_owned(),
                row: f.row.clone(),
                column: f.column.clone(),
                message: f.message.clone(),
            })
            .collect(),
        missing: import.missing.clone(),
        command: import
            .command
            .as_ref()
            .map(|cmd| serde_json::json!({"cmd": cmd.name, "params": cmd.params})),
        applied,
        rev: import.rev,
    })
}

/// Writes the CSV of a row export to `--out` or stdout (nothing on stdout with `--json`): 0, or 2 on a write error.
fn write_rows(a: &ExportArgs, csv: &str, rows: usize) -> i32 {
    match &a.out {
        Some(path) => match write_report(path, csv) {
            Ok(()) if a.json => {}
            Ok(()) => println!("export written to {}", path.display()),
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
        },
        None if a.json => {}
        None => {
            print!("{csv}");
            eprintln!("{rows} rows");
        }
    }
    0
}

/// The message of a refused row export of a chosen block (WP-S1.22 D8): as [`export_refusal`] for a project without
/// the block of the kind, else the report layer's own text.
fn rows_refusal(e: &ew_api::output::RowsError) -> String {
    match e {
        ew_api::output::RowsError::NoBlock(kind) => {
            let export = match *kind {
                "wire_list" => "the wire list",
                "terminal_diagram" => "the terminal diagram",
                "cable_overview_diagrams" => "the cable list",
                "parts_summary" => "the bill of materials",
                _ => "the export",
            };
            format!(
                "the project has no report block of kind `{kind}`: {export} is the rows of that block (a project from the built-in template has none), so there is nothing to export"
            )
        }
        // WP-S3.06 D4: each refusal of the export by kind is its own whole text.
        e @ (ew_api::output::RowsError::KindAndBlock
        | ew_api::output::RowsError::KindPages
        | ew_api::output::RowsError::UnknownKind(..)
        | ew_api::output::RowsError::KindOneStrip) => e.to_string(),
        other => other.to_string(),
    }
}

/// `ew export report --format csv`: the rows of the block (or of the kind, `--kind`), then the `--json` result; its
/// `block` is empty for `--kind` (the orchestrator's ruling 2 of cycle 42).
fn finish_export_report_csv(
    a: &ExportArgs,
    r: &ew_api::output::RowsCsv,
    options: &CsvOptions,
    rev: u64,
) -> i32 {
    let csv = restyle(&r.csv, options);
    if write_rows(a, &csv, r.rows) != 0 {
        return 2;
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ExportReportResult {
                schema: results::EXPORT_REPORT.to_owned(),
                block: r.blocks.first().cloned().unwrap_or_default(),
                kind: r.kind.clone(),
                format: "csv".to_owned(),
                rows: Some(r.rows),
                folios: None,
                out: a.out.as_ref().map(|p| p.display().to_string()),
                csv: a.out.is_none().then(|| csv.clone()),
                preset: Some(options.preset.name().to_owned()),
                rev: Some(rev),
            })
        );
    }
    0
}

/// `ew export report --format pdf`: the page export restricted to the block's folios (WP-S1.22 D8).
fn run_export_report_pdf(a: &ExportArgs, session: &ew_api::Session, block: &str) -> i32 {
    let Some(out) = &a.out else {
        eprintln!("error: `export report --format pdf` needs `--out <file.pdf>`");
        return 2;
    };
    let q = session.query();
    let found = ew_api::output::find_block(&q, block)
        .and_then(|(id, b)| ew_api::output::report_folios(&q, block).map(|f| (id, b, f)));
    let (id, b, folios) = match found {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let date = match source_date() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let options = ExportOptions {
        pages: PageSelection::Folios(folios.clone()),
        parallelism: Parallelism::threads(
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
        ),
        monochrome: false,
        date,
    };
    let pdf = match session.export_pdf(&options) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    if let Err(e) = write_atomically(out, &pdf.bytes) {
        eprintln!("error: {}: {e}", out.display());
        return 2;
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ExportReportResult {
                schema: results::EXPORT_REPORT.to_owned(),
                block: id.to_string(),
                kind: ew_api::output::block_kind(&b).to_owned(),
                format: "pdf".to_owned(),
                rows: None,
                folios: Some(folios),
                out: Some(out.display().to_string()),
                csv: None,
                preset: None,
                rev: Some(session.world().rev.0),
            })
        );
    } else {
        println!(
            "PDF written to {}: {} pages ({})",
            out.display(),
            pdf.pages,
            folios.join(", ")
        );
    }
    if !pdf.findings.is_empty() {
        eprintln!(
            "{} page findings (unresolved wire ends, unknown styles, overflowing table texts)",
            pdf.findings.len()
        );
    }
    0
}

// The program boundary: the one environment input of the exports, and there is no wall-clock fallback (no
// `SOURCE_DATE_EPOCH` means no creation date), so the same input and variable give the same bytes (doc 18 §3.5).
#[allow(clippy::disallowed_methods)]
fn source_date_epoch() -> Option<std::ffi::OsString> {
    std::env::var_os("SOURCE_DATE_EPOCH").filter(|v| !v.is_empty())
}

// The program boundary: `$APPIMAGE` and `$APPDIR`, the AppImage this `ew` may run from (ID-1187's Linux package) and its
// mount, read once and passed in, so that `ew git setup` makes git run the AppImage and never its transient mount
// (WP-S1.17 D28, C49-p9#5, C49-p10#2).
#[allow(clippy::disallowed_methods)]
fn appimage() -> [Option<std::path::PathBuf>; 2] {
    ["APPIMAGE", "APPDIR"].map(|name| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(std::path::PathBuf::from)
    })
}

/// The creation date of `SOURCE_DATE_EPOCH` (doc 17 section 3.1, FR-OUT-026); `Ok(None)` when it is not set.
fn source_date() -> Result<Option<ew_api::output::PdfDate>, String> {
    let Some(v) = source_date_epoch() else {
        return Ok(None);
    };
    let text = v.to_string_lossy();
    text.trim()
        .parse::<i64>()
        .ok()
        .and_then(date_from_epoch)
        .map(Some)
        .ok_or_else(|| format!("SOURCE_DATE_EPOCH is not a date in seconds since 1970: `{text}`"))
}

/// `ew export ewz` (WP-S1.17 D8, ID-1175 ruling 5): the project's saved generation, read back from the disk and
/// verified, packed into `--out` ([`write_atomically`]). The project opens as a reader (no lock); an `.ewz` given as
/// the project is refused ([`project_dir`]).
fn run_export_ewz(a: &ExportEwzArgs) -> i32 {
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let (session, _) = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "export ewz") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let packed = match session.export_ewz() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    if let Err(e) = write_atomically(&a.out, &packed.bytes) {
        eprintln!("error: {}: {e}", a.out.display());
        return 2;
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ExportEwzResult {
                schema: results::EXPORT_EWZ.to_owned(),
                out: a.out.display().to_string(),
                generation: packed.generation,
                files: packed.files,
                bytes: packed.bytes.len() as u64,
            })
        );
    } else {
        println!(
            ".ewz written to {}: generation {}, {}",
            a.out.display(),
            packed.generation,
            plural(packed.files, "file", "files")
        );
    }
    0
}

/// `ew extract <file.ewz> [--into <dir>]` (WP-S1.17 D9, ID-1175 ruling 4): the archive read whole and checked before
/// anything is written, then extracted into a new directory, all or nothing (`ew_api::ewz::extract`); without `--into`
/// the directory is `<stem>.ew` beside the archive.
fn run_extract(a: &ExtractArgs) -> i32 {
    let vfs = ew_api::OsVfs;
    let archive = match ew_api::ewz::read_file(&vfs, &a.file) {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "error: {}: the .ewz is refused: {e}; nothing was written",
                a.file.display()
            );
            return 2;
        }
    };
    let Some(into) = a.into.clone().or_else(|| ew_api::ewz::default_target(&a.file)) else {
        eprintln!("error: {}: not an .ewz file name", a.file.display());
        return 2;
    };
    if let Err(e) = ew_api::ewz::extract(&vfs, &archive, &into) {
        eprintln!("error: {e}");
        return 2;
    }
    let files = archive.entries.len();
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ExtractResult {
                schema: results::EXTRACT.to_owned(),
                into: into.display().to_string(),
                generation: archive.generation(),
                files,
            })
        );
    } else {
        println!(
            "extracted into {}: generation {}, {}",
            into.display(),
            archive.generation(),
            plural(files, "file", "files")
        );
    }
    0
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

/// Writes an SVG set into the directory `out` as a whole ([`write_file_set`]).
fn write_svg_set(out: &Path, files: &[ew_api::output::SvgFile]) -> Result<(), String> {
    let files: Vec<(&str, &[u8])> = files
        .iter()
        .map(|f| (f.name.as_str(), f.text.as_bytes()))
        .collect();
    write_file_set(out, "svg", "an SVG", &files)
}

/// Writes a set of files with the extension `ext` (`svg`, `png`; `a_file` names one: `an SVG`) into the directory
/// `out` as a whole: the files go to a fresh sibling directory first, which then replaces `out`, so `out` holds exactly
/// this export (no stale page of an earlier export) and a failure leaves the earlier set as it was. A directory that
/// holds anything but `.<ext>` files is refused, so no other file is ever removed (`ew export svg`, `ew export png`).
fn write_file_set(out: &Path, ext: &str, a_file: &str, files: &[(&str, &[u8])]) -> Result<(), String> {
    let label = a_file.rsplit(' ').next().unwrap_or(a_file);
    let at = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    if out.exists() {
        let entries = std::fs::read_dir(out).map_err(|e| at(out, e))?;
        for e in entries {
            let path = e.map_err(|e| at(out, e))?.path();
            if !path.is_file() || path.extension().is_none_or(|x| x != ext) {
                return Err(format!(
                    "{}: the directory holds {}, which is not {a_file} file; choose an empty or {label}-only directory",
                    out.display(),
                    path.file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                ));
            }
        }
    }
    let tmp = sibling(out, &format!(".tmp-{}", std::process::id()));
    let old = sibling(out, &format!(".old-{}", std::process::id()));
    let written = std::fs::create_dir_all(&tmp)
        .map_err(|e| at(&tmp, e))
        .and_then(|()| {
            files.iter().try_for_each(|(name, bytes)| {
                let path = tmp.join(name);
                write_synced(&path, bytes).map_err(|e| at(&path, e))
            })
        });
    if let Err(e) = written {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    let had = out.exists();
    if had && let Err(e) = std::fs::rename(out, &old) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(at(out, e));
    }
    if let Err(e) = std::fs::rename(&tmp, out) {
        if had {
            let _ = std::fs::rename(&old, out);
        }
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(at(out, e));
    }
    if had {
        let _ = std::fs::remove_dir_all(&old);
    }
    Ok(())
}

/// `ew print` (WP-S3.07 D9): the print set of the selection, written to `--out` or handed to the adapter.
fn run_print(a: &PrintArgs) -> i32 {
    use ew_api::output::print::{Fit, PrintOptions};
    let Some(dir) = reader_dir(a.project.as_ref()) else {
        return 2;
    };
    let date = match source_date() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let (session, _) = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "print") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let options = PrintOptions {
        pages: a.pages.clone(),
        paper: a.paper,
        fit: if a.fit { Fit::ToPaper } else { Fit::Actual },
        monochrome: a.mono,
        date,
        parallelism: Parallelism::threads(
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
        ),
    };
    let set = match session.print_set(&options) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let folios: Vec<String> = set.pages.iter().map(|p| p.label.clone()).collect();
    let scale = (0..set.pages.len()).map(|i| set.percent(i)).min().unwrap_or(100);
    let notices: Vec<String> = set.notices.iter().map(ToString::to_string).collect();
    for n in &notices {
        eprintln!("notice: {n}");
    }
    let result = |adapter: &str, out: Option<String>| results::PrintResult {
        schema: results::PRINT.to_owned(),
        adapter: adapter.to_owned(),
        printer: a.printer.clone().unwrap_or_default(),
        pages: folios.clone(),
        paper: a.paper.name().to_owned(),
        scale,
        notices: notices.clone(),
        out,
    };
    if let Some(out) = &a.out {
        if let Err(e) = write_atomically(out, &set.pdf) {
            eprintln!("error: {}: {e}", out.display());
            return 2;
        }
        if a.json {
            print!(
                "{}",
                results::to_text(&result("file", Some(out.display().to_string())))
            );
        }
        return 0;
    }
    // The job's title and the viewer's file stem: the project directory's name without its extension.
    let title = dir
        .file_stem()
        .map_or_else(|| "project".to_owned(), |n| n.to_string_lossy().into_owned());
    let job = ew_print::PrintJob {
        title: title.clone(),
        pdf: set.pdf,
        pages: set.pages,
        fonts: ew_api::output::font_bytes().clone(),
        printer: a.printer.clone(),
        monochrome: a.mono,
        cancel: ew_print::PrintCancel::default(),
    };
    let adapter: Box<dyn ew_print::PrintAdapter> = match a.via {
        PrintVia::Lp => Box::new(ew_print::Lp::new(std::env::temp_dir())),
        PrintVia::Viewer => Box::new(ew_print::Viewer::new(std::env::temp_dir(), &title)),
        // FT-1157 (ID-1417): Ctrl+C aborts the document at the next band instead of ending the process mid-job.
        #[cfg(windows)]
        PrintVia::Windows => {
            ew_print::ctrl_c_cancels(&job.cancel);
            Box::new(ew_print::Windows::new())
        }
    };
    let outcome = match adapter.print(&job) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let file = match &outcome {
        ew_print::PrintOutcome::Opened { file } => Some(file.display().to_string()),
        ew_print::PrintOutcome::Spooled { .. } => None,
    };
    if a.json {
        print!("{}", results::to_text(&result(adapter.name(), file)));
    } else {
        match &outcome {
            ew_print::PrintOutcome::Spooled { printer, message } => {
                println!(
                    "{}",
                    spooled_line(folios.len(), printer.as_deref(), adapter.name())
                );
                if !message.is_empty() {
                    println!("{message}");
                }
            }
            ew_print::PrintOutcome::Opened { file } => println!(
                "{} pages opened in the PDF viewer from {}; print them from there",
                folios.len(),
                file.display()
            ),
        }
    }
    0
}

/// What `ew print` says when a spooler took `pages` pages: the printer (`None`: the default one) and the way, `lp` or
/// the Windows print spooler (`--via windows`).
fn spooled_line(pages: usize, printer: Option<&str>, adapter: &str) -> String {
    let to = printer.map_or_else(|| "the default printer".to_owned(), |p| format!("printer {p}"));
    let through = if adapter == "lp" {
        "`lp`"
    } else {
        "the Windows print spooler"
    };
    format!("{pages} pages sent to {to} through {through}")
}

fn run_export_pages(a: &ExportPagesArgs) -> i32 {
    let Some(dir) = reader_dir(a.project.as_ref()) else {
        return 2;
    };
    let date = match source_date() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let (session, _) = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "export") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let threads = a
        .threads
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get));
    let options = ExportOptions {
        pages: a.pages.clone(),
        parallelism: Parallelism::threads(threads),
        monochrome: false,
        date,
    };
    let (written, findings) = match a.format {
        PageFormat::Pdf => {
            let pdf = match session.export_pdf(&options) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            if let Err(e) = write_atomically(&a.out, &pdf.bytes) {
                eprintln!("error: {}: {e}", a.out.display());
                return 2;
            }
            if a.json {
                print!(
                    "{}",
                    results::to_text(&results::ExportPdfResult {
                        schema: results::EXPORT_PDF.to_owned(),
                        out: a.out.display().to_string(),
                        pages: pdf.pages,
                        links: pdf.report.links,
                        findings: pdf.findings.len(),
                    })
                );
            }
            (
                format!(
                    "PDF written to {}: {} pages, {} links",
                    a.out.display(),
                    pdf.pages,
                    pdf.report.links
                ),
                pdf.findings.len(),
            )
        }
        PageFormat::Svg => {
            let svg = match session.export_svg(&options) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            if let Err(e) = write_svg_set(&a.out, &svg.files) {
                eprintln!("error: {e}");
                return 2;
            }
            if a.json {
                print!(
                    "{}",
                    results::to_text(&results::ExportSvgResult {
                        schema: results::EXPORT_SVG.to_owned(),
                        out: a.out.display().to_string(),
                        files: svg.files.iter().map(|f| f.name.clone()).collect(),
                        findings: svg.findings.len(),
                    })
                );
            }
            (
                format!("SVG written to {}: {} files", a.out.display(), svg.files.len()),
                svg.findings.len(),
            )
        }
        PageFormat::Png => {
            let png = match session.export_png(&options, a.dpi) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            let files: Vec<(&str, &[u8])> = png
                .files
                .iter()
                .map(|f| (f.name.as_str(), f.bytes.as_slice()))
                .collect();
            if let Err(e) = write_file_set(&a.out, "png", "a PNG", &files) {
                eprintln!("error: {e}");
                return 2;
            }
            if a.json {
                print!(
                    "{}",
                    results::to_text(&results::ExportPngResult {
                        schema: results::EXPORT_PNG.to_owned(),
                        out: a.out.display().to_string(),
                        dpi: a.dpi,
                        files: png.files.iter().map(|f| f.name.clone()).collect(),
                        findings: png.findings.len(),
                    })
                );
            }
            (
                format!(
                    "PNG written to {}: {} files at {} dpi",
                    a.out.display(),
                    png.files.len(),
                    a.dpi
                ),
                png.findings.len(),
            )
        }
    };
    if !a.json {
        println!("{written}");
    }
    if findings > 0 {
        eprintln!("{findings} page findings (unresolved wire ends, unknown styles, overflowing table texts)");
    }
    0
}

/// `ew render --page <folio> --png` (WP-S1.22 D7, FR-EXT-004): one page as PNG, to `--out` or `<folio>.png` in the
/// current directory, never to stdout.
fn run_render(a: &RenderArgs) -> i32 {
    let Some(dir) = reader_dir(a.project.as_ref()) else {
        return 2;
    };
    let (session, _) = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "render") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let png = match session.render_png(&a.page, a.dpi) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let Some(file) = png.files.first() else {
        eprintln!("error: the page order has no folio `{}`", a.page);
        return 2;
    };
    let out = a.out.clone().unwrap_or_else(|| PathBuf::from(&file.name));
    if let Err(e) = write_atomically(&out, &file.bytes) {
        eprintln!("error: {}: {e}", out.display());
        return 2;
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::RenderResult {
                schema: results::RENDER.to_owned(),
                page: file.folio.clone(),
                out: out.display().to_string(),
                dpi: a.dpi,
                width: file.width,
                height: file.height,
                findings: png.findings.len(),
            })
        );
    } else {
        println!(
            "PNG written to {}: folio {}, {} x {} pixels at {} dpi",
            out.display(),
            file.folio,
            file.width,
            file.height,
            a.dpi
        );
    }
    if !png.findings.is_empty() {
        eprintln!(
            "{} page findings (unresolved wire ends, unknown styles, overflowing table texts)",
            png.findings.len()
        );
    }
    0
}

fn run_apply(a: &ApplyArgs) -> i32 {
    let text = match std::fs::read_to_string(&a.log) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {}: {e}", a.log.display());
            return 2;
        }
    };
    let log = match CmdLog::parse(&text) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: {e}");
            return i32::from(e.exit_code());
        }
    };
    // The origin names the log by its file name, never its path.
    let file = a.log.file_name().map_or_else(
        || a.log.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    if a.preview {
        return run_apply_preview(&dir, &log, &file, a.json);
    }
    let print_result = |committed: usize, saved: bool| {
        if a.json {
            print!(
                "{}",
                results::to_text(&results::ApplyResult {
                    schema: results::APPLY.to_owned(),
                    lines: log.len(),
                    committed,
                    saved,
                    preview: None,
                })
            );
        }
    };
    let origin = ew_api::Origin::Cli {
        subcommand: "apply".to_owned(),
        confirmed: false,
    };
    // WP-S7.03 D8: a log that pins an address profile reads the libraries of `ew lib` (the content's system
    // libraries, the user's), so that a line naming a library's profile embeds it first; any other log reads none.
    let tiers = log
        .commands()
        .any(|c| c.name == "plc.set_profile")
        .then(|| {
            let system: Vec<PathBuf> = content_dir().map(|d| d.join("libraries")).into_iter().collect();
            ew_api::library::LibraryTiers::for_user(system, Vec::new())
        })
        .flatten();
    let store = ew_api::StoreOptions::new(env!("CARGO_PKG_VERSION"));
    let mut session = match open_for_writing_from(&dir, origin, store, tiers) {
        Ok((s, _)) => s,
        Err(code) => return code,
    };
    let result = session.run_script(&log, &file);
    let (code, committed) = match &result {
        Ok(receipts) => (0, receipts.len()),
        Err(e) => {
            eprintln!("error: {e}");
            (i32::from(e.exit_code()), e.committed())
        }
    };
    if code != 0 && (a.one_step || committed == 0) {
        if a.one_step && committed > 0 {
            eprintln!("--one-step: nothing saved ({committed} lines were replayed and discarded)");
        }
        session.close();
        print_result(committed, false);
        return code;
    }
    match session.save() {
        Ok(saved) => {
            warn_unreleased(saved.unstable, &mut std::io::stderr());
            if a.json {
                print_result(committed, true);
            } else {
                println!("applied {committed} of {} lines; saved", log.len());
            }
            session.close();
            code
        }
        Err(e) => {
            eprintln!("error: the save failed: {e:?}");
            session.close();
            2
        }
    }
}

/// Opens the project in `dir` for writing with `origin`: refused (exit 2) when it opens read-only or a crashed
/// session left unsaved changes in its journal, which a new save would hide (doc 05 section 8.3).
fn open_for_writing(dir: &Path, origin: ew_api::Origin) -> Result<ew_api::Session, i32> {
    open_for_writing_with(dir, origin, ew_api::StoreOptions::new(env!("CARGO_PKG_VERSION")))
}

/// [`open_for_writing`] with the store options given.
fn open_for_writing_with(
    dir: &Path,
    origin: ew_api::Origin,
    store: ew_api::StoreOptions,
) -> Result<ew_api::Session, i32> {
    open_for_writing_items(dir, origin, store).map(|(session, _)| session)
}

/// [`open_for_writing_with`] that also returns the integrity findings of opening (EW-CORE-001 … 019), which the release
/// gate joins as `ew check` does.
fn open_for_writing_items(
    dir: &Path,
    origin: ew_api::Origin,
    store: ew_api::StoreOptions,
) -> Result<(ew_api::Session, Vec<ew_api::checks::IntegrityItem>), i32> {
    open_for_writing_from(dir, origin, store, None)
}

/// [`open_for_writing_items`] with the session's library tiers: `ew apply`'s, so that a `plc.set_profile` of a
/// library's address profile embeds it first (WP-S7.03 D8).
fn open_for_writing_from(
    dir: &Path,
    origin: ew_api::Origin,
    store: ew_api::StoreOptions,
    libraries: Option<ew_api::library::LibraryTiers>,
) -> Result<(ew_api::Session, Vec<ew_api::checks::IntegrityItem>), i32> {
    let (session, opened) = open_editing_from(dir, origin, store, ew_api::Recovery::Keep, libraries)?;
    if opened.recovery.is_some() {
        eprintln!("{}", journal_refusal(dir));
        session.close();
        return Err(2);
    }
    if let Some(from) = &opened.migrated_from {
        eprintln!("error: {}", older_format(dir, from));
        session.close();
        return Err(2);
    }
    let items = ew_api::integrity_items(&opened);
    Ok((session, items))
}

/// Opens the project in `dir` for editing with `origin`, `recovery` saying what happens to a crashed session's
/// journal: refused (exit 2, [`read_only_refusal`]) when the open went read-only, which an editing open does silently
/// when another session holds the lock (ruling 2 of cycle 41: a writer never acts read-only). A repair open
/// ([`ew_api::StoreOptions::repair`]) that integrity findings made read-only is let through: `ew fmt --repair` names a
/// crashed session's journal first and then the finding (WP-S1.17, decision log ID-1185 rulings 3 and 4).
fn open_editing(
    dir: &Path,
    origin: ew_api::Origin,
    store: ew_api::StoreOptions,
    recovery: ew_api::Recovery,
) -> Result<(ew_api::Session, ew_api::OpenReport), i32> {
    open_editing_from(dir, origin, store, recovery, None)
}

/// [`open_editing`] with the session's library tiers (`None`: the compiled-in libraries only).
fn open_editing_from(
    dir: &Path,
    origin: ew_api::Origin,
    store: ew_api::StoreOptions,
    recovery: ew_api::Recovery,
    libraries: Option<ew_api::library::LibraryTiers>,
) -> Result<(ew_api::Session, ew_api::OpenReport), i32> {
    open_editing_to(dir, origin, store, recovery, libraries, &mut std::io::stderr())
}

/// [`open_editing_from`] with its refusals written to `errors` (`ew recover`'s injected writer, FT-1394).
fn open_editing_to(
    dir: &Path,
    origin: ew_api::Origin,
    store: ew_api::StoreOptions,
    recovery: ew_api::Recovery,
    libraries: Option<ew_api::library::LibraryTiers>,
    errors: &mut dyn std::io::Write,
) -> Result<(ew_api::Session, ew_api::OpenReport), i32> {
    let repair = store.repair;
    let options = ew_api::SessionOptions {
        origin,
        libraries,
        ..ew_api::SessionOptions::default()
    };
    let (session, opened) = match ew_api::Session::open(dir, store, options, recovery) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(errors, "error: {}: {e}", dir.display());
            return Err(2);
        }
    };
    match &opened.access {
        ew_api::Access::ReadOnly(ew_api::ReadOnlyReason::Integrity) if repair => {}
        ew_api::Access::ReadOnly(reason) => {
            // FT-1383 (ID-1418): a damaged library copy a device uses is named, with the way out.
            let refusal = match (reason, session.damaged_copy()) {
                (
                    ew_api::ReadOnlyReason::Integrity,
                    Some(ew_api::DamagedCopy {
                        device: Some(device),
                        part,
                        ..
                    }),
                ) => {
                    let copy = match part {
                        Some((part, library)) => format!("the library copy of part {part} from {library}"),
                        None => "the library copy of its part".to_owned(),
                    };
                    format!(
                        "error: {}: the project opens read-only: the device {device} uses {copy}, which is damaged; \
                         update the copy from its library first; nothing was written",
                        dir.display()
                    )
                }
                // FT-1500 (ID-1458): a copy no device names, by its own words and the record that names it.
                (ew_api::ReadOnlyReason::Integrity, Some(damaged)) => format!(
                    "error: {}: the project opens read-only: {} is damaged; update the copy from its library first; \
                     nothing was written",
                    dir.display(),
                    damaged.words
                ),
                _ => read_only_refusal(dir, reason),
            };
            let _ = writeln!(errors, "{refusal}");
            session.close();
            return Err(2);
        }
        ew_api::Access::Edit => {}
    }
    Ok((session, opened))
}

/// The refusal of every writer (`apply`, `fmt`, `recover`, `import parts`, the human-only subcommands) whose open went
/// read-only (ruling 2 of cycle 41: `ew recover` refuses a held lock with `ew apply`'s text). The reason is in words,
/// those the window uses, with no session ID, process ID or `@` (FT-877; the ruling WP-S1.22 §10.2 rule 3 needs for a
/// changed text is ruling 1 of the cycle-42 FT batch).
fn read_only_refusal(dir: &Path, reason: &ew_api::ReadOnlyReason) -> String {
    format!(
        "error: {}: the project opens read-only: {}; nothing was written",
        dir.display(),
        read_only_words(reason)
    )
}

/// A read-only reason in words. Text from the lock file and the system passes through [`printable`].
fn read_only_words(reason: &ew_api::ReadOnlyReason) -> String {
    use ew_api::ReadOnlyReason as R;
    match reason {
        R::Requested => "it was opened read-only on request".to_owned(),
        R::LockedBy(info) => match &info.record {
            Some(r) => format!(
                "another session holds the project ({} on {}, since {})",
                printable(&r.user),
                printable(&r.host),
                printable(&r.started_at)
            ),
            None => "another session holds the project".to_owned(),
        },
        R::NotWritable(message) => format!("the project folder cannot be written ({})", printable(message)),
        R::Integrity => "it has integrity findings; `ew check` lists them".to_owned(),
        R::Format(why) => format!("it was {why}"),
        R::LockLost(_) => "the lock was taken over".to_owned(),
    }
}

/// Why a writer other than `ew upgrade` and `ew recover` does not write a project of an older format, which opens
/// migrated in memory (WP-S1.17 D11, decision log ID-1312): said before the unsaved-changes check and the save's
/// consent gate, naming the writer that converts it. `ew fmt --check` says it too.
fn older_format(dir: &Path, from: &str) -> String {
    format!(
        "{}: the project is in the older format {from}; convert it with `ew upgrade <project>` first; nothing was \
         written",
        dir.display()
    )
}

/// The refusal of every writer (`apply`, `fmt`, `import parts`, the human-only subcommands) while a crashed session's
/// journal holds unsaved changes, which a new save would hide (doc 05 section 8.3). It names the command that replays
/// or discards them, `ew recover`, and no follow-up number (cycle 41 review, ruling 1 of its fix round).
fn journal_refusal(dir: &Path) -> String {
    format!(
        "error: {}: a crashed session left unsaved changes in its journal; replay or discard them first with `ew \
         recover`; nothing was written",
        dir.display()
    )
}

/// `text` with its control characters and the Unicode bidirectional controls escaped: text from a project or state
/// file (a journal's command name, a finding about a file's content, a lock record's user) never reaches the terminal
/// as a control sequence, nor reorders the line it prints in (U+061C, U+200E, U+200F, U+202A to U+202E and U+2066 to
/// U+2069 print as `\u{…}`).
fn printable(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() {
                c.escape_default().to_string()
            } else if is_bidi_control(c) {
                c.escape_unicode().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// Whether `c` is a Unicode bidirectional control (the Bidi_Control property).
fn is_bidi_control(c: char) -> bool {
    matches!(
        c,
        '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
    )
}

/// `ew project checks set`: `project.checks.set` from a confirmed CLI subcommand, the human origin the command's
/// exposure class `HumanOnly` accepts (ADR-0008 amendment 3, FT-336), after the confirmation typed on a controlling
/// terminal (doc 17 section 2.3, FR-EXT-030); without a terminal nothing is opened.
fn run_checks_set(a: &ChecksSetArgs) -> i32 {
    use std::io::IsTerminal;
    // 1. The read-only part: every named scheme ships (doc 15 section 4.9); else exit 1 without asking.
    for name in [&a.active, &a.release].into_iter().flatten() {
        if scheme(name).is_none() {
            let names: Vec<&str> = ew_api::checks::SCHEMES.iter().map(|s| s.name).collect();
            eprintln!(
                "error: `{name}` is not a shipped check scheme; the schemes are: {}; nothing was changed",
                names.join(", ")
            );
            return 1;
        }
    }
    // 2. Without a controlling terminal (CI, a pipe, a script) a HumanOnly command is never submitted (doc 17
    //    section 2.3, FR-EXT-030); the project is not opened, so nothing is locked or journalled.
    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        eprintln!(
            "error: HumanOnly: needs an interactive terminal (`project checks set` is confirmed by typing the \
             scheme name); nothing was changed"
        );
        return 2;
    }
    // 3. The change summary, then the typed confirmation: the scheme's name, exactly. There is no `--yes` flag.
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    if !confirm_typed(a, &mut input, &mut output) {
        eprintln!("error: not confirmed; nothing was changed");
        return 1;
    }
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let origin = ew_api::Origin::Cli {
        subcommand: "project checks set".to_owned(),
        confirmed: true,
    };
    let mut session = match open_for_writing(&dir, origin) {
        Ok(s) => s,
        Err(code) => return code,
    };
    if let Err(e) = session.execute(&checks_set_command(a)) {
        eprintln!("error: {e}");
        session.close();
        return 1;
    }
    let code = match session.save() {
        Ok(saved) => {
            warn_unreleased(saved.unstable, &mut std::io::stderr());
            println!("check schemes set; saved");
            0
        }
        Err(e) => {
            eprintln!("error: the save failed: {e:?}");
            2
        }
    };
    session.close();
    code
}

/// The subcommand name of a `ew revision` action, as the origin of its session names it.
fn revision_subcommand(action: &RevisionAction) -> &'static str {
    match action {
        RevisionAction::Approve { .. } => "revision approve",
        RevisionAction::Release { .. } => "revision release",
        RevisionAction::Withdraw { .. } => "revision withdraw",
    }
}

/// Today's date (UTC) from the system clock: the date of a sign-off. The CLI is the front-end that reads the clock;
/// the command takes the date as a parameter (WP-S1.16 §10 rule 2).
fn today() -> ew_api::catalog::Date {
    use ew_api::Clock as _;
    let seconds = ew_api::SystemClock
        .now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
    date_from_epoch(seconds)
        .and_then(|d| ew_api::catalog::Date::new(d.year, d.month, d.day).ok())
        .unwrap_or_else(|| ew_api::catalog::Date::new(1980, 1, 1).expect("a valid date"))
}

/// `ew revision approve|release|withdraw` (WP-S1.18 part 3, D10): the `HumanOnly` commands of the release workflow,
/// each submitted from the confirmed origin only after the person types the revision label on a controlling terminal
/// (doc 15 section 5.4, doc 17 section 2.3); without a terminal nothing is opened (exit 2).
fn run_revision(a: &RevisionArgs) -> i32 {
    use std::io::IsTerminal;
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    run_revision_with(
        a,
        terminal,
        &mut input,
        &mut output,
        ew_api::StoreOptions::new(env!("CARGO_PKG_VERSION")),
        today(),
        &ew_api::git::GitExe::default(),
    )
}

/// One finding as the report text prints it: `<severity> <rule> <code> <subject>: <message>`.
fn finding_line(f: &ew_api::checks::Finding) -> String {
    format!(
        "{} {} {} {}: {}\n",
        f.severity, f.rule, f.code, f.subject, f.message
    )
}

/// Prints `prompt` to `output` and reads one line from `input`: `true` when it is exactly `word`, surrounding white
/// space aside.
fn typed_word(
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
    prompt: &str,
    word: &str,
) -> bool {
    let mut answer = String::new();
    output
        .write_all(prompt.as_bytes())
        .and_then(|()| output.flush())
        .is_ok()
        && input.read_line(&mut answer).is_ok()
        && answer.trim() == word
}

/// [`run_revision`] with the terminal test, the reader, the writer, the store options, the date and the git port
/// injected: the confirmation hook of the tests (doc 18 section 3.15's `test-confirm`, realised as the injected
/// terminal; the production binary reads the real terminal and exits 2 without one, and runs the `git` executable).
fn run_revision_with(
    a: &RevisionArgs,
    terminal: bool,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
    store: ew_api::StoreOptions,
    today: ew_api::catalog::Date,
    git: &dyn ew_api::git::Git,
) -> i32 {
    let subcommand = revision_subcommand(&a.action);
    if !terminal {
        eprintln!(
            "error: HumanOnly: needs an interactive terminal (`{subcommand}` is confirmed by typing the revision \
             label); nothing was changed"
        );
        return 2;
    }
    let checker = match &a.action {
        RevisionAction::Approve { role } => {
            match serde_json::from_value::<ew_api::PartyRole>(serde_json::Value::String(role.clone())) {
                Ok(r) if !matches!(r, ew_api::PartyRole::Unknown(_)) => Some(r),
                _ => {
                    eprintln!(
                        "error: `{role}` is not a party role (customer, end_user, panel_builder, author, checker, \
                         engineer); nothing was changed"
                    );
                    return 2;
                }
            }
        }
        _ => None,
    };
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let origin = ew_api::Origin::Cli {
        subcommand: subcommand.to_owned(),
        confirmed: true,
    };
    let (mut session, items) = match open_for_writing_items(&dir, origin, store) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let code = match (&a.action, checker) {
        (RevisionAction::Approve { .. }, Some(checker)) => {
            approve_revision(&mut session, &items, &checker, input, output, today)
        }
        (RevisionAction::Release { label, archive_dir }, _) => {
            let release = Release {
                label,
                archive_dir: archive_dir.as_deref(),
                git,
                today,
            };
            release_revision(&mut session, &items, &release, input, output)
        }
        (RevisionAction::Withdraw { label, reason }, _) => {
            withdraw_revision(&mut session, label, reason, input, output)
        }
        (RevisionAction::Approve { .. }, None) => 2,
    };
    // The release's housekeeping (FT-814): a run record that could not be removed, as the project opened or a run
    // was settled.
    for trouble in session.take_release_troubles() {
        eprintln!("warning: {trouble}");
    }
    session.close();
    code
}

/// The warning after a save that wrote an unreleased format (FT-1154, WP-S6.08 D3).
pub(crate) const UNRELEASED_SAVE: &str = "warning: saved in a format that is not released yet; released versions of \
                                          ElektraWatt open this project read-only";

/// After a save: [`UNRELEASED_SAVE`] on `errors` when the manifest it wrote is marked `unstable` (FT-1154); the exit
/// code stays the save's.
pub(crate) fn warn_unreleased(unstable: bool, errors: &mut dyn std::io::Write) {
    if unstable {
        let _ = writeln!(errors, "{UNRELEASED_SAVE}");
    }
}

/// Saves after a confirmed revision command: exit 0 with `done` printed, 2 when the save fails.
fn save_revision(session: &mut ew_api::Session, output: &mut dyn std::io::Write, done: &str) -> i32 {
    match session.save() {
        Ok(report) => {
            warn_unreleased(report.unstable, &mut std::io::stderr());
            let _ = writeln!(output, "{done}; saved (generation {})", report.generation);
            0
        }
        Err(e) => {
            eprintln!("error: the save failed: {e:?}");
            2
        }
    }
}

/// `ew revision approve --as <role>`: the gate on the saved generation, the safety-kind waivers to acknowledge, the
/// typed label, then `revision.approve` through [`ew_api::Session::approve`] and a save.
fn approve_revision(
    session: &mut ew_api::Session,
    items: &[ew_api::checks::IntegrityItem],
    checker: &ew_api::PartyRole,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
    today: ew_api::catalog::Date,
) -> i32 {
    let Some(label) = session.open_revision_label() else {
        eprintln!("error: the project has no open revision; nothing was approved");
        return 1;
    };
    let gate = match session.release_gate(&label, items) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("error: {e}; nothing was approved");
            return 1;
        }
    };
    let blocked = gate.approval_blockers();
    if !blocked.is_empty() {
        let listing: String = blocked.iter().map(|f| finding_line(f)).collect();
        let _ = output.write_all(listing.as_bytes());
        eprintln!(
            "error: the release gate has {} unwaived error(s) besides {}; nothing was approved",
            blocked.len(),
            ew_api::FOUR_EYES_RULE
        );
        return 1;
    }
    let role = ew_api::role_name(checker);
    let mut summary = format!(
        "approve revision {label} as {role} (author: {}): generation {}, content digest {}, scheme {}\n",
        gate.author
            .as_ref()
            .map_or_else(|| "not set".to_owned(), ew_api::role_name),
        gate.generation,
        gate.content_digest,
        gate.report.scheme
    );
    for w in &gate.safety_waivers {
        let waived = gate.report.findings.iter().find(|f| {
            matches!(&f.state, ew_api::checks::FindingState::Waived { waiver } if *waiver == w.to_string())
        });
        let what = waived.map_or_else(String::new, |f| format!(": {} {}", f.rule, f.subject));
        summary.push_str(&format!("  safety waiver {w}{what}\n"));
    }
    let acknowledge = if gate.safety_waivers.is_empty() {
        String::new()
    } else {
        format!(
            " and acknowledge the {} safety waiver(s) above",
            gate.safety_waivers.len()
        )
    };
    let prompt = format!(
        "{summary}Type the revision label `{label}` to approve it{acknowledge}, anything else to abort: "
    );
    if !typed_word(input, output, &prompt, &label) {
        eprintln!("error: not confirmed; nothing was changed");
        return 1;
    }
    let approval = ew_api::Approval {
        label: label.clone(),
        checker: checker.clone(),
        date: today,
        waivers_ack: gate.safety_waivers.clone(),
    };
    if let Err(e) = session.approve(&approval, items) {
        if let ew_api::ApproveError::Blocked(list) = &e {
            let listing: String = list.iter().map(finding_line).collect();
            let _ = output.write_all(listing.as_bytes());
        }
        eprintln!("error: {e}; nothing was approved");
        return 1;
    }
    save_revision(session, output, &format!("approved revision {label} as {role}"))
}

/// What `ew revision release` was asked: the label, the archive root, the git port and the release date.
struct Release<'a> {
    label: &'a str,
    archive_dir: Option<&'a Path>,
    git: &'a dyn ew_api::git::Git,
    today: ew_api::catalog::Date,
}

impl Release<'_> {
    /// The release options; `git_consent` is the typed confirmation, whose prompt names the git steps.
    fn options(&self, git_consent: bool) -> ew_api::ReleaseOptions<'_> {
        ew_api::ReleaseOptions {
            archive_root: self.archive_dir,
            git: self.git,
            git_consent,
            parallelism: Parallelism::threads(
                std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
            ),
        }
    }
}

/// The exit code of a release error: 1 for a refusal before anything was written, 2 when writing failed.
fn release_exit(e: &ew_api::ReleaseError) -> i32 {
    use ew_api::ReleaseError as E;
    match e {
        E::Save(_)
        | E::Generation(_)
        | E::Output(_)
        | E::Archive(_)
        | E::Write(_)
        | E::Stamp(_)
        | E::Unsettled(_)
        | E::Undecided(_)
        | E::Io { .. } => 2,
        _ => 1,
    }
}

/// `ew revision release <label>` (doc 05 section 14.1; WP-S1.18 part 4): an incomplete release of the label is settled
/// first ([`settle_release`]); else the plan (the gate on the saved generation, with its blockers when it refuses; the
/// archive; the git worktree and tag), the typed label, then [`ew_api::Session::release`], which saves.
fn release_revision(
    session: &mut ew_api::Session,
    items: &[ew_api::checks::IntegrityItem],
    release: &Release<'_>,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
) -> i32 {
    let label = release.label;
    match session.incomplete_release(release.archive_dir) {
        Ok(Some(found)) if found.label == label => {
            return settle_release(session, items, release, &found, input, output);
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!("error: {e}; nothing was released");
            return release_exit(&e);
        }
    }
    let plan = match session.release_plan(label, items, &release.options(false)) {
        Ok(plan) => plan,
        Err(ew_api::ReleaseError::Blocked(blockers)) => {
            let listing: String = blockers.iter().map(finding_line).collect();
            let _ = output.write_all(listing.as_bytes());
            eprintln!(
                "error: release of {label} refused: the gate has {} blocker(s); nothing was released",
                blockers.len()
            );
            return 1;
        }
        Err(e) => {
            eprintln!("error: {e}; nothing was released");
            return release_exit(&e);
        }
    };
    let mut summary = format!(
        "release revision {label}: generation {}, content digest {}, scheme {}\n  archive: {}\n",
        plan.gate.generation,
        plan.gate.content_digest,
        plan.gate.report.scheme,
        plan.archive.display()
    );
    if let Some((worktree, tag)) = &plan.git {
        summary.push_str(&format!(
            "  git: commits the project directory in the worktree {} (\"Release {label}\") and creates the tag {tag}\n",
            worktree.display()
        ));
        if plan.archive_in_worktree {
            summary.push_str(&format!(
                "warning: the archive root lies inside the git worktree {}; archives are never added to git, so name a \
                 root outside it with --archive-dir <dir>\n",
                worktree.display()
            ));
        }
    }
    let prompt = format!(
        "{summary}A released revision is immutable. Type the revision label `{label}` to release it, anything else to \
         abort: "
    );
    if !typed_word(input, output, &prompt, label) {
        eprintln!("error: not confirmed; nothing was changed");
        return 1;
    }
    let ctx = ew_api::ReleaseContext {
        label: label.to_owned(),
        date: release.today,
    };
    match session.release(&ctx, items, &release.options(true)) {
        Ok(done) => {
            released_lines(output, &done);
            0
        }
        Err(e) => {
            if let ew_api::ReleaseError::Blocked(list) = &e {
                let listing: String = list.iter().map(finding_line).collect();
                let _ = output.write_all(listing.as_bytes());
            }
            eprintln!("error: {e}");
            release_exit(&e)
        }
    }
}

/// What a completed release prints.
fn released_lines(output: &mut dyn std::io::Write, done: &ew_api::Released) {
    let _ = writeln!(
        output,
        "released revision {}: archive {} ({}); saved (generation {})",
        done.label,
        done.archive.display(),
        done.sha256,
        done.stamped
    );
    if let Some((_, tag)) = &done.git {
        let _ = writeln!(
            output,
            "tagged {tag}; generation {} is left for your next commit",
            done.stamped
        );
    }
    if done.save_incomplete {
        let _ = writeln!(
            output,
            "note: the save of generation {} completes when the project is next opened",
            done.stamped
        );
    }
}

/// An archive of the open revision's label that a crashed release left (doc 05 section 14.1 "On failure"): the
/// person types `complete` (when it can be completed) to stamp it, or `discard` (when this project directory's own run
/// provably wrote it, FT-814) to remove it and its tag. An archive that is not this project's (another project's or
/// revision's, or one that names no project) is offered neither: it is kept, and the release is refused; so is one that
/// can neither be completed nor provably discarded.
fn settle_release(
    session: &mut ew_api::Session,
    items: &[ew_api::checks::IntegrityItem],
    release: &Release<'_>,
    found: &ew_api::IncompleteRelease,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
) -> i32 {
    let label = &found.label;
    if let Some(why) = &found.foreign {
        eprintln!(
            "error: release of {label} refused: {why}; it is kept. Name another archive folder with --archive-dir \
             <dir>; nothing was changed"
        );
        return 1;
    }
    if let (Some(refusal), Some(unproven)) = (&found.refusal, &found.unproven) {
        eprintln!(
            "error: release of {label} refused: {unproven}, and it cannot be completed ({refusal}); it is kept. Remove \
             the file by hand if you know it is a leftover, or name another archive folder with --archive-dir <dir>; \
             nothing was changed"
        );
        return 1;
    }
    let mut summary = format!(
        "incomplete release {label}: the archive {} ({}) is in place, but the revision is still open\n",
        found.archive.display(),
        found.sha256
    );
    let words = match (&found.refusal, &found.unproven) {
        (Some(why), _) => {
            summary.push_str(&format!("  it cannot be completed: {why}\n"));
            "`discard` to remove it (and the tag of the label, if this folder's crashed release created it)"
        }
        (None, Some(why)) => {
            summary.push_str(&format!(
                "  it is not offered for discarding: {why}; remove the file by hand if you know it is a leftover\n"
            ));
            "`complete` to stamp the revision released with it (committing and tagging when the project lies in a git \
             worktree)"
        }
        (None, None) => {
            "`complete` to stamp the revision released with it (committing and tagging when the project lies in a git \
             worktree), `discard` to remove it (and the tag of the label, if this folder's crashed release created it)"
        }
    };
    let prompt = format!("{summary}Type {words}, anything else to abort: ");
    let mut answer = String::new();
    let read = output
        .write_all(prompt.as_bytes())
        .and_then(|()| output.flush())
        .is_ok()
        && input.read_line(&mut answer).is_ok();
    match (read, answer.trim(), &found.refusal) {
        (true, "complete", None) => match session.complete_release(items, &release.options(true)) {
            Ok(done) => {
                released_lines(output, &done);
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                release_exit(&e)
            }
        },
        (true, "discard", _) => match session.discard_incomplete_release(&release.options(true)) {
            Ok(done) => {
                let tag = done
                    .tag_removed
                    .map_or_else(String::new, |t| format!(" and the tag {t}"));
                let _ = writeln!(
                    output,
                    "discarded the incomplete release {label}: removed {}{tag}; the revision is still open",
                    done.archive.display()
                );
                if let Some(t) = done.tag_kept {
                    let _ = writeln!(
                        output,
                        "note: the git tag {t} was not made by this folder's crashed release, so it was kept; a new \
                         release of {label} needs it gone"
                    );
                }
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                release_exit(&e)
            }
        },
        _ => {
            eprintln!("error: not confirmed; nothing was changed");
            1
        }
    }
}

/// `ew revision withdraw <label> --reason <text>`: the typed label, then `revision.withdraw` of the released revision
/// and a save.
fn withdraw_revision(
    session: &mut ew_api::Session,
    label: &str,
    reason: &str,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
) -> i32 {
    let Some(id) = session.released_revision(label) else {
        eprintln!("error: the project has no released revision {label}; nothing was withdrawn");
        return 1;
    };
    let prompt = format!(
        "withdraw revision {label}: {reason}\nA withdrawal stays in the revision history. Type the revision label \
         `{label}` to withdraw it, anything else to abort: "
    );
    if !typed_word(input, output, &prompt, label) {
        eprintln!("error: not confirmed; nothing was changed");
        return 1;
    }
    let cmd = ew_api::AnyCommand {
        name: "revision.withdraw".to_owned(),
        params: serde_json::json!({"revision": id.to_string(), "reason": reason}),
    };
    if let Err(e) = session.execute(&cmd) {
        eprintln!("error: {e}; nothing was withdrawn");
        return 1;
    }
    save_revision(session, output, &format!("withdrew revision {label}"))
}

/// The word a person types to confirm `ew project assets purge`.
const PURGE_WORD: &str = "purge";

/// `ew project assets purge`: the asset purge of doc 05 section 13.3 (`project.assets.purge`, FT-380), a `HumanOnly`
/// command (owner ruling, 2026-10-23): the preview's list, then, after the word typed on a controlling terminal, the
/// purge of exactly the listed files. `--preview` needs no terminal; without one the purge itself exits 2 before the
/// project is opened.
fn run_assets_purge(a: &AssetsPurgeArgs) -> i32 {
    use std::io::IsTerminal;
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    run_assets_purge_with(
        a,
        terminal,
        &mut input,
        &mut output,
        ew_api::StoreOptions::new(env!("CARGO_PKG_VERSION")),
    )
}

/// [`run_assets_purge`] with the terminal test, the reader, the writer and the store options injected (unit tests).
fn run_assets_purge_with(
    a: &AssetsPurgeArgs,
    terminal: bool,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
    store: ew_api::StoreOptions,
) -> i32 {
    if !a.preview && !terminal {
        eprintln!(
            "error: HumanOnly: needs an interactive terminal (`project assets purge` is confirmed by typing \
             `{PURGE_WORD}`; `--preview` lists the files without one); nothing was removed"
        );
        return 2;
    }
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    // The preview is a reader: no lock, no roll-forward, no journal, nothing written, so it runs beside an open
    // editor. The purge opens for writing with the confirmed origin, used only after the word is typed.
    let opened = if a.preview {
        ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "project assets purge")
            .map(|(s, _)| s)
            .map_err(|e| {
                eprintln!("error: {}: {e}", dir.display());
                2
            })
    } else {
        let origin = ew_api::Origin::Cli {
            subcommand: "project assets purge".to_owned(),
            confirmed: true,
        };
        open_for_writing_with(&dir, origin, store)
    };
    let mut session = match opened {
        Ok(s) => s,
        Err(code) => return code,
    };
    let refused = |e: &ew_api::PurgeError| match e {
        ew_api::PurgeError::Io { .. } | ew_api::PurgeError::Save { .. } | ew_api::PurgeError::HumanOnly => 2,
        _ => 1,
    };
    let plan = match session.purge_preview() {
        Ok(plan) => plan,
        Err(e) => {
            eprintln!("error: {e}; nothing was removed");
            session.close();
            return refused(&e);
        }
    };
    let listing: String = plan
        .remove
        .iter()
        .map(|f| format!("remove {} ({} bytes)\n", f.path.as_str(), f.len))
        .collect();
    let verb = if a.preview { "would remove" } else { "removes" };
    let _ = output.write_all(listing.as_bytes());
    let _ = output.flush();
    eprintln!(
        "purge: {verb} {} asset file(s), {} bytes; {} kept (named)",
        plan.remove.len(),
        plan.bytes(),
        plan.kept
    );
    if a.preview || plan.remove.is_empty() {
        session.close();
        return 0;
    }
    let prompt = format!(
        "Deleted asset files do not come back by undo. Type `{PURGE_WORD}` to delete the {} file(s) above, anything \
         else to abort: ",
        plan.remove.len()
    );
    let mut answer = String::new();
    let confirmed = output
        .write_all(prompt.as_bytes())
        .and_then(|()| output.flush())
        .is_ok()
        && input.read_line(&mut answer).is_ok()
        && answer.trim() == PURGE_WORD;
    if !confirmed {
        eprintln!("error: not confirmed; nothing was removed");
        session.close();
        return 1;
    }
    let code = match session.purge_assets(Some(&plan.hashes())) {
        Ok((done, report)) => {
            warn_unreleased(report.unstable, &mut std::io::stderr());
            let _ = writeln!(
                output,
                "removed {} asset file(s); generation {}",
                done.remove.len(),
                report.generation
            );
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            refused(&e)
        }
    };
    session.close();
    code
}

/// The `project.checks.set` command of `a`: a scheme not given is absent (its default applies).
fn checks_set_command(a: &ChecksSetArgs) -> ew_api::AnyCommand {
    let mut params = serde_json::Map::new();
    if let Some(s) = &a.active {
        params.insert("active_scheme".to_owned(), serde_json::Value::from(s.as_str()));
    }
    if let Some(s) = &a.release {
        params.insert("release_scheme".to_owned(), serde_json::Value::from(s.as_str()));
    }
    ew_api::AnyCommand {
        name: "project.checks.set".to_owned(),
        params: serde_json::Value::Object(params),
    }
}

/// The scheme name the person types to confirm `a`: the active scheme's, else the release scheme's.
fn confirmation_word(a: &ChecksSetArgs) -> &str {
    a.active.as_deref().or(a.release.as_deref()).unwrap_or_default()
}

/// Prints the change summary of `a` to `output` and reads one line from `input`: `true` when it is exactly the
/// confirmation word ([`confirmation_word`]), surrounding white space aside (doc 17 section 2.3).
fn confirm_typed(
    a: &ChecksSetArgs,
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
) -> bool {
    let word = confirmation_word(a);
    let line = |name: &str, scheme: &Option<String>, default: &str| match scheme {
        Some(s) => format!("  {name} scheme: {s}\n"),
        None => format!("  {name} scheme: back to the default ({default})\n"),
    };
    let summary = format!(
        "project checks set:\n{}{}The check scheme decides check outcomes: which rules the release gate runs and what \
         the checks report.\nType `{word}` to apply it, anything else to abort: ",
        line("active", &a.active, "drafting"),
        line("release", &a.release, "pre-delivery"),
    );
    if output
        .write_all(summary.as_bytes())
        .and_then(|()| output.flush())
        .is_err()
    {
        return false;
    }
    let mut answer = String::new();
    match input.read_line(&mut answer) {
        Ok(_) => answer.trim() == word,
        Err(_) => false,
    }
}

/// `ew apply --preview`: the replay on a read-only session (no lock, no journal, never saved), then the changes.
fn run_apply_preview(dir: &Path, log: &CmdLog, file: &str, json: bool) -> i32 {
    let (mut session, _) = match ew_api::Session::open_reader(dir, env!("CARGO_PKG_VERSION"), "apply") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let before = session.world();
    let result = session.run_script(log, file);
    let (code, committed) = match &result {
        Ok(receipts) => (0, receipts.len()),
        Err(e) => {
            eprintln!("error: {e}");
            (i32::from(e.exit_code()), e.committed())
        }
    };
    let diff = match session.diff_from(&before) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e:?}");
            return 2;
        }
    };
    if json {
        fn row<K: std::fmt::Display>(r: &ew_api::DerivedChange<K>) -> results::DerivedRowChange {
            results::DerivedRowChange {
                key: r.key.to_string(),
                before: r.before.clone(),
                after: r.after.clone(),
            }
        }
        let records = diff
            .model
            .changes
            .iter()
            .map(|c| results::RecordChange {
                change: format!("{:?}", c.change),
                kind: c.key.0.to_string(),
                id: c.key.1.to_string(),
                fields: c.fields.iter().map(ToString::to_string).collect(),
            })
            .collect();
        print!(
            "{}",
            results::to_text(&results::ApplyResult {
                schema: results::APPLY.to_owned(),
                lines: log.len(),
                committed,
                saved: false,
                preview: Some(results::ApplyChanges {
                    records,
                    tags: diff.derived.tags.iter().map(row).collect(),
                    xrefs: diff.derived.xrefs.iter().map(row).collect(),
                }),
            })
        );
        return code;
    }
    println!(
        "preview: {committed} of {} lines commit; {} records, {} tags and {} cross-references change; nothing is saved",
        log.len(),
        diff.model.changes.len(),
        diff.derived.tags.len(),
        diff.derived.xrefs.len()
    );
    for c in &diff.model.changes {
        let (kind, id) = c.key;
        let fields = if c.fields.is_empty() {
            String::new()
        } else {
            format!(": {}", c.fields.join(", "))
        };
        println!("{:?} {kind} {id}{fields}", c.change);
    }
    for t in &diff.derived.tags {
        println!("tag {}: {} -> {}", t.key, t.before, t.after);
    }
    for x in &diff.derived.xrefs {
        println!("xref {}: {} -> {}", x.key, x.before, x.after);
    }
    code
}

fn run_verify_derived(a: &VerifyArgs) -> i32 {
    let Some(dir) = reader_dir(a.project.as_ref()) else {
        return 2;
    };
    let (session, _) = match ew_api::Session::open_reader_with(
        &dir,
        env!("CARGO_PKG_VERSION"),
        "verify-derived",
        ew_api::MemoMode::Oracle,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let report = match ew_api::verify::verify_derived(&session) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    if a.json {
        print!(
            "{}",
            results::to_text(&results::VerifyDerivedResult {
                schema: results::VERIFY_DERIVED.to_owned(),
                families: report.keys.iter().map(|(f, n)| ((*f).to_owned(), *n)).collect(),
                keys: report.total_keys(),
                differences: report.differences.clone(),
                oracle: report
                    .oracle
                    .iter()
                    .map(|m| results::OracleMismatch {
                        family: m.family.to_owned(),
                        key: m.key.clone(),
                        memoised: m.memoised.clone(),
                        cold: m.cold.clone(),
                    })
                    .collect(),
            })
        );
        return i32::from(!report.is_clean());
    }
    println!(
        "verify-derived: {} families, {} keys, {} differences, {} oracle mismatches",
        report.keys.len(),
        report.total_keys(),
        report.differences.len(),
        report.oracle.len()
    );
    for d in &report.differences {
        println!("differs: {d}");
    }
    for m in &report.oracle {
        println!("oracle: {m:?}");
    }
    i32::from(!report.is_clean())
}

fn run_sql_dump(a: &SqlDumpArgs) -> i32 {
    let projection = match open_projection(a.project.as_ref()) {
        Ok(p) => p,
        Err(code) => return code,
    };
    match std::fs::write(&a.out, projection.image()) {
        Ok(()) if a.json => {
            print!(
                "{}",
                results::to_text(&results::SqlDumpResult {
                    schema: results::SQL_DUMP.to_owned(),
                    out: a.out.display().to_string(),
                })
            );
            0
        }
        Ok(()) => {
            println!("projection written to {}", a.out.display());
            0
        }
        Err(e) => {
            eprintln!("error: {}: {e}", a.out.display());
            2
        }
    }
}

fn run_check(a: &CheckArgs) -> i32 {
    let dir = match &a.project {
        Some(d) => d.clone(),
        None => {
            let found = std::env::current_dir().ok().and_then(|cwd| find_project(&cwd));
            match found {
                Some(d) => d,
                None => {
                    eprintln!("error: no project directory given and none found above the current directory");
                    return 2;
                }
            }
        }
    };
    let (mut session, opened) = match ew_api::Session::open_read_only(&dir, env!("CARGO_PKG_VERSION")) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    if a.integrity {
        if a.after_merge {
            session.assume_merged();
        }
        let result = report::localise(&session.check_integrity(&opened), &a.lang);
        return print_check(a, &result);
    }
    // `--scheme`, else the scheme the project names for the mode (FT-366), else the shipped default. A project naming a
    // scheme this build does not ship (a newer writer, a hand edit) gets the default and a notice; the exit status is
    // the check's (FT-405, decision log ID-602).
    let sch = match &a.scheme {
        Some(name) => scheme(name),
        None => {
            let named = session.project_check_scheme_name(a.mode);
            match named.as_deref().and_then(scheme) {
                Some(sch) => Some(sch),
                None => {
                    if let Some(name) = &named {
                        // The name comes from the project file: escaped so control characters cannot reach the
                        // terminal.
                        eprintln!(
                            "notice: project scheme '{}' is not shipped by this build; using pre-delivery",
                            name.escape_debug()
                        );
                    }
                    scheme("pre-delivery")
                }
            }
        }
    };
    let Some(sch) = sch else {
        eprintln!(
            "error: unknown scheme `{}`",
            a.scheme.as_deref().unwrap_or("pre-delivery")
        );
        return 2;
    };
    // The integrity findings of opening (EW-CORE-001 … 019) join the report: in release mode the "n r"
    // errors count and set the exit code (doc 15 §4.2).
    let result = report::localise(
        &session
            .check(sch, a.mode)
            .with_integrity(ew_api::integrity_items(&opened)),
        &a.lang,
    );
    print_check(a, &result)
}

/// Prints a check report as `ew check` does, to `--out` or stdout with the totals on stderr; its exit code.
fn print_check(a: &CheckArgs, result: &ew_api::checks::CheckReport) -> i32 {
    let result = result.clone();
    let text = match a.format {
        CheckFormat::Text => report::to_text(&result),
        CheckFormat::Markdown => report::to_markdown(&result),
        CheckFormat::Json => format!("{:#}\n", report::to_json(&result)),
    };
    match &a.out {
        Some(path) => match write_report(path, &text) {
            Ok(()) => println!("report written to {}", path.display()),
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
        },
        None => {
            print!("{text}");
            let c = result.counts;
            eprintln!(
                "{}: {} errors, {} warnings, {} infos, {} waived",
                result.scheme, c.errors, c.warnings, c.infos, c.waived
            );
        }
    }
    result.exit_code()
}

// The program boundary: `EW_CONTENT_DIR` only names the directory of the starter content, as `--content` does.
#[allow(clippy::disallowed_methods)]
fn content_dir() -> Option<PathBuf> {
    std::env::var_os("EW_CONTENT_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Opens the tiers of `ew lib` (exit code 2 on failure).
fn open_libraries(company: &[PathBuf]) -> Result<ew_api::library::Registry, i32> {
    let system: Vec<PathBuf> = content_dir().map(|d| d.join("libraries")).into_iter().collect();
    let Some(tiers) = ew_api::library::LibraryTiers::for_user(system, company.to_vec()) else {
        eprintln!("error: no data and cache directories (set EW_DATA_DIR and EW_CACHE_DIR)");
        return Err(2);
    };
    ew_api::library::Registry::open(&tiers).map_err(|e| {
        eprintln!("error: the libraries cannot be opened: {e}");
        2
    })
}

fn index_state(s: &ew_api::library::IndexState) -> String {
    match s {
        ew_api::library::IndexState::Fresh => "fresh".to_owned(),
        ew_api::library::IndexState::Rebuilt => "rebuilt".to_owned(),
        ew_api::library::IndexState::Failed(m) => format!("failed: {m}"),
    }
}

fn json_text(v: serde_json::Result<serde_json::Value>) -> String {
    match v {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

fn run_lib(a: &LibArgs) -> i32 {
    use ew_api::library::{ItemQuery, PartQuery};
    let mut reg = match open_libraries(&a.company) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let libs = reg.libraries();
    let keys: std::collections::BTreeMap<String, String> =
        libs.iter().map(|l| (l.id.to_string(), l.key.clone())).collect();
    let key_of = |id: &dyn std::fmt::Display| {
        let id = id.to_string();
        keys.get(&id).cloned().unwrap_or(id)
    };
    match &a.action {
        LibAction::List => {
            if a.json {
                let libraries = libs
                    .iter()
                    .map(|l| results::LibraryRow {
                        id: l.id.to_string(),
                        key: l.key.clone(),
                        name: l.name.clone(),
                        tier: l.tier.name().to_owned(),
                        version: l.version.clone(),
                        licence: l.licence.clone(),
                        writable: l.writable,
                        generation: l.generation,
                        index: index_state(&l.index),
                        path: l.path.as_ref().map(|p| p.display().to_string()),
                        // WP-S7.03 T2: an address profile that does not render is left out of the profiles at hand.
                        findings: l
                            .findings
                            .iter()
                            .cloned()
                            .chain(ew_api::unreadable_profiles(&reg, l.id))
                            .collect(),
                    })
                    .collect();
                print!(
                    "{}",
                    results::to_text(&results::LibListResult {
                        schema: results::LIB_LIST.to_owned(),
                        libraries,
                    })
                );
            } else {
                for l in &libs {
                    let path = l
                        .path
                        .as_ref()
                        .map_or_else(|| "(compiled in)".to_owned(), |p| p.display().to_string());
                    println!(
                        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{path}",
                        l.tier.name(),
                        l.key,
                        l.version,
                        l.name,
                        l.id,
                        if l.writable { "rw" } else { "ro" },
                        index_state(&l.index)
                    );
                    for f in l
                        .findings
                        .iter()
                        .cloned()
                        .chain(ew_api::unreadable_profiles(&reg, l.id))
                    {
                        println!("  {f}");
                    }
                }
            }
            for f in reg.findings() {
                eprintln!("warning: {f}");
            }
            0
        }
        LibAction::Search {
            text,
            kinds,
            all,
            limit,
        } => {
            let kinds: Vec<_> = kinds
                .iter()
                .filter_map(|k| ew_api::library::item_kind(k))
                .collect();
            let parts = kinds.is_empty() || kinds.iter().any(|k| k.name() == "part");
            let others: Vec<_> = kinds.into_iter().filter(|k| k.name() != "part").collect();
            let mut rows = Vec::new();
            if parts {
                let q = PartQuery {
                    text: text.clone(),
                    include_inactive: *all,
                    limit: *limit,
                    ..PartQuery::default()
                };
                let hits = match reg.try_search(&q) {
                    Ok(h) => h,
                    Err(e) => {
                        eprintln!("error: {e}");
                        return 2;
                    }
                };
                for h in hits {
                    rows.push((
                        results::LibHit::Part(results::PartHitRow {
                            kind: "part".to_owned(),
                            library: h.library.to_string(),
                            library_key: key_of(&h.library),
                            item: h.item.to_string(),
                            version: h.version.clone(),
                            tier: h.tier.name().to_owned(),
                            part_number: h.part_number.clone(),
                            manufacturer: h.manufacturer.clone(),
                            description: h.description.clone(),
                            part_kind: json_text(serde_json::to_value(&h.kind)),
                            status: serde_json::to_value(&h.status).unwrap_or_default(),
                            rank_tier: h.rank_tier,
                        }),
                        format!(
                            "part\t{}\t{}\t{}\t{}\t{}/{}@{}",
                            h.part_number,
                            h.manufacturer,
                            h.description,
                            json_text(serde_json::to_value(&h.status)),
                            key_of(&h.library),
                            h.item,
                            h.version
                        ),
                    ));
                }
            }
            if !others.is_empty() {
                let q = ItemQuery {
                    text: text.clone(),
                    kinds: others,
                    include_inactive: *all,
                    limit: *limit,
                    ..ItemQuery::default()
                };
                for h in reg.search_items(&q) {
                    rows.push((
                        results::LibHit::Item(results::ItemHitRow {
                            kind: h.kind.name().to_owned(),
                            library: h.library.to_string(),
                            library_key: key_of(&h.library),
                            item: h.item.to_string(),
                            version: h.version.clone(),
                            tier: h.tier.name().to_owned(),
                            key: h.key.clone(),
                            name: h.name.clone(),
                            status: serde_json::to_value(&h.status).unwrap_or_default(),
                            rank_tier: h.rank_tier,
                        }),
                        format!(
                            "{}\t{}\t{}\t{}\t{}/{}@{}",
                            h.kind.name(),
                            h.key,
                            h.name,
                            json_text(serde_json::to_value(&h.status)),
                            key_of(&h.library),
                            h.item,
                            h.version
                        ),
                    ));
                }
            }
            if a.json {
                print!(
                    "{}",
                    results::to_text(&results::LibSearchResult {
                        schema: results::LIB_SEARCH.to_owned(),
                        hits: rows.into_iter().map(|(j, _)| j).collect(),
                    })
                );
            } else {
                if rows.is_empty() {
                    eprintln!("nothing found");
                }
                for (_, line) in rows {
                    println!("{line}");
                }
            }
            0
        }
        LibAction::IndexRebuild => {
            if let Err(e) = reg.rebuild_index() {
                eprintln!("error: the index cannot be rebuilt: {e}");
                return 2;
            }
            match reg.index_counts() {
                Ok((parts, items)) => {
                    if a.json {
                        print!(
                            "{}",
                            results::to_text(&results::LibIndexResult {
                                schema: results::LIB_INDEX.to_owned(),
                                parts,
                                items,
                            })
                        );
                    } else {
                        println!("index rebuilt: {parts} parts, {items} other items");
                    }
                    0
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    2
                }
            }
        }
        LibAction::Create {
            dir,
            key,
            name,
            licence,
        } => match reg.create(dir, key, name, licence) {
            Ok(id) => {
                let listed = reg.libraries().iter().any(|l| l.id == id);
                if a.json {
                    print!(
                        "{}",
                        results::to_text(&results::LibCreateResult {
                            schema: results::LIB_CREATE.to_owned(),
                            key: key.clone(),
                            id: id.to_string(),
                            dir: dir.display().to_string(),
                            listed,
                        })
                    );
                } else {
                    println!("created {key} ({id}) in {}", dir.display());
                }
                if !listed {
                    eprintln!(
                        "note: {} lies in no library tier; pass it as `--company` to use it",
                        dir.display()
                    );
                }
                0
            }
            Err(e) => {
                eprintln!("error: {e}");
                2
            }
        },
        LibAction::ImportParts {
            library,
            file,
            map,
            as_of,
            bump,
            take_over,
        } => {
            let Some(target) = libs
                .iter()
                .find(|l| l.key == *library || l.id.to_string() == *library)
                .map(|l| (l.id, l.key.clone()))
            else {
                eprintln!("error: no library `{library}` (see `ew lib list`)");
                return 2;
            };
            let (map, input) = match read_map_and_table(map, file) {
                Ok(r) => r,
                Err(code) => return code,
            };
            match ew_api::catalog::import_into_library(
                &mut reg,
                target.0,
                &input,
                &map,
                Some(*as_of),
                *bump,
                *take_over,
            ) {
                Ok(done) => {
                    if a.json {
                        print!(
                            "{}",
                            results::to_text(&results::LibImportPartsResult {
                                schema: results::LIB_IMPORT_PARTS.to_owned(),
                                library: target.1.clone(),
                                source: input.name().to_string(),
                                rows: row_counts(&done.import),
                                report: report_rows(&done.import),
                                written: done.write.as_ref().map(|w| results::LibWrite {
                                    added: w.added.len(),
                                    updated: w.updated.len(),
                                    unchanged: w.unchanged.len(),
                                    generation: w.generation,
                                }),
                            })
                        );
                        if let Some(w) = done.write.as_ref().and_then(index_warning) {
                            eprintln!("{w}");
                        }
                        return 0;
                    }
                    print_catalog_rows(&done.import);
                    let written = done.write.as_ref().map_or_else(
                        || "nothing to write".to_owned(),
                        |w| {
                            format!(
                                "written: {} added, {} updated, {} unchanged (generation {})",
                                w.added.len(),
                                w.updated.len(),
                                w.unchanged.len(),
                                w.generation
                            )
                        },
                    );
                    println!(
                        "imported {} into {}: {}; {written}",
                        input.name(),
                        target.1,
                        counts_text(&done.import)
                    );
                    if let Some(w) = done.write.as_ref().and_then(index_warning) {
                        eprintln!("{w}");
                    }
                    0
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    if matches!(
                        e,
                        ew_api::catalog::CatalogError::Library(ew_api::library::LibraryError::Stale { .. })
                    ) {
                        eprintln!("note: pass `--take-over` to take the stale lock over");
                    }
                    2
                }
            }
        }
    }
}

/// The warning for a library write whose search index failed to take the committed write (FT-665): the write
/// stands, and the index is brought up to date on the next open or write.
fn index_warning(w: &ew_api::library::WriteReport) -> Option<String> {
    w.index_error.as_deref().map(|e| {
        format!("warning: the library's search index was not updated ({e}); it is rebuilt on the next open or write")
    })
}

/// Reads the column map and the table of a mapped parts import (exit code 2 on failure).
fn read_map_and_table(
    map: &Path,
    file: &Path,
) -> Result<(ew_api::catalog::ColumnMap, ew_api::catalog::ImportInput), i32> {
    let map = ew_api::catalog::read_column_map(map).map_err(|e| {
        eprintln!("error: the column map: {e}");
        2
    })?;
    let input = ew_api::catalog::read_input(file, &ew_api::catalog::Limits::default()).map_err(|e| {
        eprintln!("error: {e}");
        2
    })?;
    Ok((map, input))
}

/// The counts of a mapped parts import, as text.
fn counts_text(i: &ew_api::catalog::PartsImport) -> String {
    let c = i.counts();
    format!(
        "{} new, {} updated, {} unchanged, {} duplicate, {} rejected",
        c.new, c.updated, c.unchanged, c.duplicate, c.rejected
    )
}

/// The row counts of a mapped parts import, as its JSON result names them.
fn row_counts(i: &ew_api::catalog::PartsImport) -> results::RowCounts {
    let c = i.counts();
    results::RowCounts {
        new: c.new,
        updated: c.updated,
        unchanged: c.unchanged,
        duplicate: c.duplicate,
        rejected: c.rejected,
    }
}

/// The report of a mapped parts import, one JSON row per finding.
fn report_rows(i: &ew_api::catalog::PartsImport) -> Vec<results::ReportRow> {
    i.report
        .entries
        .iter()
        .map(|e| results::ReportRow {
            code: e.code.to_string(),
            severity: match e.severity {
                ew_api::catalog::Severity::Info => results::ReportSeverity::Info,
                ew_api::catalog::Severity::Warning => results::ReportSeverity::Warning,
                ew_api::catalog::Severity::Error => results::ReportSeverity::Error,
            },
            line: e.location.as_ref().and_then(|l| l.line).map(u64::from),
            message: e.message.clone(),
        })
        .collect()
}

/// Prints the report of a mapped parts import: one line per finding (unmapped columns, rejected rows, split part
/// numbers, aliases, new manufacturers).
fn print_catalog_rows(i: &ew_api::catalog::PartsImport) {
    for e in &i.report.entries {
        let severity = match e.severity {
            ew_api::catalog::Severity::Info => "info",
            ew_api::catalog::Severity::Warning => "warning",
            ew_api::catalog::Severity::Error => "error",
        };
        let at = e
            .location
            .as_ref()
            .and_then(|l| l.line)
            .map_or_else(String::new, |n| format!("line {n}: "));
        println!("{} {severity}: {at}{}", e.code, e.message);
    }
}

/// `ew import parts --map`: a supplier's table through a column map into the project.
fn run_import_parts_mapped(a: &ImportPartsArgs, map: &Path, dir: &Path) -> i32 {
    let (map, input) = match read_map_and_table(map, &a.file) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let master = match open_libraries(&a.company) {
        Ok(reg) => ew_api::catalog::manufacturer_master(&reg),
        Err(code) => return code,
    };
    let origin = ew_api::Origin::Cli {
        subcommand: "import parts".into(),
        confirmed: false,
    };
    let Ok(mut session) = open_for_writing(dir, origin.clone()) else {
        return 2;
    };
    let (import, cmd) = match session.catalog_parts(&input, &map, &master, a.as_of) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            session.close();
            return 2;
        }
    };
    if !a.json {
        print_catalog_rows(&import);
    }
    let (mut created, mut skipped) = (0, 0);
    if let Some(cmd) = cmd {
        let receipt = match session
            .preview_with(&cmd, ew_api::Submit::from(origin))
            .and_then(|preview| session.commit(preview))
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: {e}");
                session.close();
                return 2;
            }
        };
        (created, skipped) =
            ew_api::import::parts_import_counts(receipt.outcome.note.as_deref().unwrap_or_default())
                .unwrap_or((0, 0));
        if let Err(e) = session
            .save()
            .map(|r| warn_unreleased(r.unstable, &mut std::io::stderr()))
        {
            eprintln!("error: the save failed: {e:?}");
            session.close();
            return 2;
        }
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ImportPartsResult {
                schema: results::IMPORT_PARTS.to_owned(),
                source: input.name().to_string(),
                created,
                skipped,
                rows: Some(row_counts(&import)),
                report: report_rows(&import),
            })
        );
    } else {
        println!(
            "imported {}: {}; {created} created, {skipped} skipped; saved",
            input.name(),
            counts_text(&import)
        );
    }
    session.close();
    0
}

fn run_import_parts(a: &ImportPartsArgs) -> i32 {
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    if let Some(map) = &a.map {
        return run_import_parts_mapped(a, map, &dir);
    }
    let origin = ew_api::Origin::Cli {
        subcommand: "import parts".into(),
        confirmed: false,
    };
    let Ok(mut session) = open_for_writing(&dir, origin.clone()) else {
        return 2;
    };
    let id = session.query().project().id.as_uuid();
    let parts = match ew_api::import::read_parts_tables(&a.file, id) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            session.close();
            return 2;
        }
    };
    let cmd = ew_api::AnyCommand {
        name: ew_api::import::PARTS_IMPORT.into(),
        params: parts.params(),
    };
    let receipt = match session
        .preview_with(&cmd, ew_api::Submit::from(origin))
        .and_then(|preview| session.commit(preview))
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            session.close();
            return 2;
        }
    };
    let (created, skipped) =
        ew_api::import::parts_import_counts(receipt.outcome.note.as_deref().unwrap_or_default())
            .unwrap_or((0, 0));
    if let Err(e) = session
        .save()
        .map(|r| warn_unreleased(r.unstable, &mut std::io::stderr()))
    {
        eprintln!("error: the save failed: {e:?}");
        session.close();
        return 2;
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ImportPartsResult {
                schema: results::IMPORT_PARTS.to_owned(),
                source: parts.source.to_string(),
                created,
                skipped,
                rows: None,
                report: Vec::new(),
            })
        );
    } else {
        println!(
            "imported {}: {created} created, {skipped} skipped, 0 errors; saved",
            parts.source
        );
    }
    session.close();
    0
}

fn run_import_qet(a: &ImportQetArgs) -> i32 {
    let content = a.content.clone().or_else(content_dir);
    let library = match &content {
        Some(dir) => match StarterLibrary::from_dir(dir) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("error: starter content: {e}");
                return 2;
            }
        },
        None => {
            eprintln!(
                "warning: no starter content (--content or EW_CONTENT_DIR); folio reports are dropped and pages get the default frame"
            );
            StarterLibrary::empty()
        }
    };
    let bytes = match std::fs::read(&a.file) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {}: {e}", a.file.display());
            return 2;
        }
    };
    let name = a.file.file_name().map_or_else(
        || a.file.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    if let Some(project) = &a.into {
        return run_reimport_qet(a, project, &name, bytes, &library);
    }
    let imported = match import_qet(&name, bytes, &library) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let c = imported.counts();
    let report = imported.migration_report();
    // With the report on stdout, stdout is exactly the report: the summary goes to stderr.
    let to_stderr = a.json || (a.report.is_some() && a.report_out.is_none());
    let say = |line: String| {
        if to_stderr {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    };
    say(format!(
        "imported {name}: {} pages, {} devices ({} strips, {} terminals), {} functions, {} representations, {} connections, {} junctions",
        c.pages, c.devices, c.strips, c.terminals, c.functions, c.representations, c.connections, c.junctions
    ));
    say(format!(
        "report: {} findings, {} lost connections",
        report.total, report.lost_connections
    ));
    if !report.live.is_empty() {
        let n: u32 = report.live.iter().map(|g| g.count).sum();
        say(format!("checks: {n} lenient findings"));
    }
    let mut code = 0;
    let lenient: u32 = report.live.iter().map(|g| g.count).sum();
    let (derived_pages, model_check_error) = match &imported.session {
        Ok(s) => {
            say(format!("derived: {} pages in page order", s.page_labels().len()));
            (Some(s.page_labels().len()), None)
        }
        Err(msg) => {
            say(format!("model check failed: {msg}"));
            code = 1;
            (None, Some(msg.to_string()))
        }
    };
    // ID-1374 (C51-p2#7): as `import csv --strict`, a failed `--strict` writes no project.
    let mut strict_failed = false;
    if a.strict {
        let errors = imported
            .batch
            .report
            .entries
            .iter()
            .filter(|e| e.severity == Severity::Error)
            .count();
        if report.lost_connections > 0 {
            eprintln!("strict: {} lost connections", report.lost_connections);
            strict_failed = true;
        }
        if errors > 0 {
            eprintln!("strict: {errors} error findings");
            strict_failed = true;
        }
        if strict_failed {
            code = 1;
        }
    }
    if let Some(fmt) = a.report {
        let text = report.render(fmt);
        match &a.report_out {
            Some(path) => match write_report(path, &text) {
                Ok(()) => say(format!("report written to {}", path.display())),
                Err(e) => {
                    eprintln!("error: {}: {e}", path.display());
                    return 2;
                }
            },
            None => print!("{text}"),
        }
    }
    if a.out.is_some() && strict_failed {
        eprintln!("strict: nothing was written");
    } else if let Some(out) = &a.out {
        match imported.session {
            Ok(session) => {
                match ew_api::import::create_project_from(
                    session,
                    out,
                    ew_api::StoreOptions::new(env!("CARGO_PKG_VERSION")),
                ) {
                    Ok(()) => say(format!("project written to {}", out.display())),
                    Err(e) => {
                        eprintln!("error: {e}");
                        return 2;
                    }
                }
            }
            Err(_) => {
                eprintln!("error: the imported records do not make a project; nothing was written");
                return 2;
            }
        }
    }
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ImportQetResult {
                schema: results::IMPORT_QET.to_owned(),
                source: name.clone(),
                counts: results::QetCounts {
                    pages: c.pages,
                    devices: c.devices,
                    strips: c.strips,
                    terminals: c.terminals,
                    functions: c.functions,
                    representations: c.representations,
                    connections: c.connections,
                    junctions: c.junctions,
                },
                findings: report.total,
                lost_connections: report.lost_connections,
                lenient_findings: lenient,
                derived_pages,
                model_check_error,
                report_out: a
                    .report
                    .and(a.report_out.as_ref())
                    .map(|p| p.display().to_string()),
            })
        );
    }
    code
}

/// `ew import qet <file> --into <project> [--apply]` (WP-S3.09 D10 to D12): the file re-imported into its earlier
/// import in the project. Prints T6 when the import is matched by its items, T8 and the rows of the records added,
/// changed in the source, changed on both sides (each member with this project's and the file's value) and removed
/// in the source; without `--apply` nothing is written (T12), with it the re-import is one transaction, saved. Exit 2
/// when the file is refused or is not an earlier import of the project (T7).
fn run_reimport_qet(
    a: &ImportQetArgs,
    project: &Path,
    name: &str,
    bytes: Vec<u8>,
    library: &StarterLibrary,
) -> i32 {
    use ew_api::import::{ReimportClass as C, member_text};
    let opened = if a.apply {
        let origin = ew_api::Origin::Cli {
            subcommand: "import qet".into(),
            confirmed: false,
        };
        open_for_writing(project, origin)
    } else {
        ew_api::Session::open_read_only(project, env!("CARGO_PKG_VERSION"))
            .map(|(s, _)| s)
            .map_err(|e| {
                eprintln!("error: {}: {e}", project.display());
                2
            })
    };
    let Ok(mut session) = opened else {
        return 2;
    };
    let detected = match session.detect_qet_lineage(name, bytes.clone(), library) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            session.close();
            return 2;
        }
    };
    if let Some(question) = detected.question(name) {
        println!("{question}");
    }
    let reimport = match session.reimport_qet(name, bytes, library, detected.lineage()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            session.close();
            return 2;
        }
    };
    println!("{}", reimport.summary());
    for row in &reimport.plan.rows {
        let class = match row.class {
            C::Added => "added",
            C::SourceChanged => "changed in the source",
            C::Conflict => "changed on both sides",
            C::Removed => "removed in the source",
            C::EwChanged | C::Unchanged => continue,
        };
        println!("{class}: {}", row.record);
        for m in &row.members {
            println!(
                "  {}: here {}, in {name} {}",
                m.path,
                member_text(m.ours.as_ref()),
                member_text(m.theirs.as_ref())
            );
        }
    }
    let writes = reimport.plan.writes();
    if !a.apply {
        if writes {
            println!("nothing was written: add --apply to commit the re-import");
        }
        session.close();
        return 0;
    }
    if let Err(e) = session.apply_reimport(reimport) {
        eprintln!("error: {e}");
        session.close();
        return 2;
    }
    if writes {
        if let Err(e) = session
            .save()
            .map(|r| warn_unreleased(r.unstable, &mut std::io::stderr()))
        {
            eprintln!("error: the save failed: {e:?}");
            session.close();
            return 2;
        }
        println!("re-import applied; saved");
    } else {
        println!("nothing to apply");
    }
    session.close();
    0
}

/// `ew import csv` (WP-S1.22 part 3, FT-27): the golden-dialect tables of a directory with the starter content as a new
/// project in `--out`. Every finding is printed; `--strict` (the default) writes nothing when the import made error
/// findings (exit 1), `--lenient` writes the project with them.
fn run_import_csv(a: &ImportCsvArgs) -> i32 {
    use ew_api::import::{CreateError, ImportMode, create_project, import_tables, read_tables};
    let Some(content) = a.content.clone().or_else(content_dir) else {
        eprintln!(
            "error: `import csv` needs the starter content (`--content <dir>` or EW_CONTENT_DIR): the tables name its \
             symbols, function definitions and forms; nothing was written"
        );
        return 2;
    };
    let library = match StarterLibrary::from_dir(&content) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: starter content: {}: {e}", content.display());
            return 2;
        }
    };
    let inputs = match read_tables(&a.tables) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let mode = if a.lenient {
        ImportMode::Lenient
    } else {
        ImportMode::Strict
    };
    let batch = match import_tables(&inputs, &library, mode) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let rows = table_report_rows(&batch.report.entries);
    // With `--json` stdout is the result: the human lines go to stderr.
    let say = |line: String| {
        if a.json {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    };
    for r in &rows {
        let severity = match r.severity {
            results::ReportSeverity::Info => "info",
            results::ReportSeverity::Warning => "warning",
            results::ReportSeverity::Error => "error",
        };
        let at = match (&r.file, r.line) {
            (Some(f), Some(l)) => format!("{}:{l}: ", printable(f)),
            (Some(f), None) => format!("{}: ", printable(f)),
            _ => String::new(),
        };
        say(format!("{} {severity}: {at}{}", r.code, printable(&r.message)));
    }
    let errors = rows
        .iter()
        .filter(|r| r.severity == results::ReportSeverity::Error)
        .count();
    let tables = inputs.len();
    let (records, documents) = (batch.records.len(), batch.lib.len());
    let (code, written) = if !a.lenient && errors > 0 {
        eprintln!(
            "error: the import made {} under `--strict` (the default); nothing was written",
            plural(errors, "error finding", "error findings")
        );
        (1, false)
    } else {
        match create_project(
            &batch,
            &a.out,
            ew_api::StoreOptions::new(env!("CARGO_PKG_VERSION")),
        ) {
            Ok(()) => {
                say(format!(
                    "imported {} into {}: {}, {}",
                    plural(tables, "table", "tables"),
                    a.out.display(),
                    plural(records, "record", "records"),
                    plural(documents, "library document", "library documents")
                ));
                (0, true)
            }
            Err(e @ CreateError::Model(_)) => {
                eprintln!("error: {e}; nothing was written");
                (1, false)
            }
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        }
    };
    if a.json {
        print!(
            "{}",
            results::to_text(&results::ImportCsvResult {
                schema: results::IMPORT_CSV.to_owned(),
                tables: inputs.iter().map(|i| i.name().to_owned()).collect(),
                mode: if a.lenient {
                    results::ImportCsvMode::Lenient
                } else {
                    results::ImportCsvMode::Strict
                },
                out: a.out.display().to_string(),
                written,
                records,
                documents,
                report: rows,
            })
        );
    }
    code
}

/// The findings of a table import as result rows, in the report's order.
fn table_report_rows(entries: &[ew_api::import::ReportEntry]) -> Vec<results::TableReportRow> {
    entries
        .iter()
        .map(|e| results::TableReportRow {
            code: e.code.to_string(),
            severity: match e.severity {
                Severity::Info => results::ReportSeverity::Info,
                Severity::Warning => results::ReportSeverity::Warning,
                Severity::Error => results::ReportSeverity::Error,
            },
            file: e.location.as_ref().map(|l| l.file.clone()),
            line: e.location.as_ref().and_then(|l| l.line).map(u64::from),
            message: e.message.clone(),
        })
        .collect()
}

/// `n` with the singular or plural noun: `1 file`, `2 files`.
fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// `ew commands [--json]` (WP-S1.22 D6): the command registry, one line per command (name, parameters version,
/// exposure, cost, confirmation, undoable, stability, aliases), or its JSON result.
fn run_commands(json: bool) -> i32 {
    let list = results::commands();
    if json {
        print!("{}", results::to_text(&list));
        return 0;
    }
    for c in &list.commands {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            c.name,
            c.params_version,
            c.exposure,
            c.cost,
            c.confirmation,
            if c.undoable { "undoable" } else { "not undoable" },
            c.stability,
            c.aliases.join(",")
        );
    }
    0
}

/// `ew schema <name>` (WP-S1.22 D6): the parameter schema of a command (or an alias), byte for byte the file of
/// `crates/ew-commands/schemas/`, or a result schema (`ew.<command>-result/<n>`), the file of `bin/ew/schemas/`. An
/// unknown name exits 2 naming it.
fn run_schema(name: &str) -> i32 {
    let text = if name.starts_with("ew.") && name.contains("-result/") {
        results::schema_text(name)
    } else {
        results::command_schema_text(name)
    };
    match text {
        Some(t) => {
            print!("{t}");
            0
        }
        None => {
            eprintln!(
                "error: unknown schema `{}` (a command name of `ew commands`, or a result schema `ew.<command>-result/<n>`)",
                name.escape_debug()
            );
            2
        }
    }
}

fn write_report(path: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    /// FT-877, ruling 1 of the cycle-42 FT batch: "error: <dir>: the project opens read-only: <reason>; nothing was
    /// written", each reason in the window's words; text from the system is printed escaped. The held lock is
    /// `tests/cli.rs`'s `recover_and_apply_refuse_a_held_lock_with_the_same_text`.
    #[test]
    fn a_read_only_refusal_words_each_reason() {
        use ew_api::ReadOnlyReason as R;
        let dir = std::path::Path::new("p");
        for (reason, text) in [
            (
                R::Requested,
                "error: p: the project opens read-only: it was opened read-only on request; nothing was written",
            ),
            (
                R::NotWritable("Access is denied.\u{1b}[2J".into()),
                "error: p: the project opens read-only: the project folder cannot be written (Access is \
                 denied.\\u{1b}[2J); nothing was written",
            ),
            (
                R::Integrity,
                "error: p: the project opens read-only: it has integrity findings; `ew check` lists them; nothing \
                 was written",
            ),
            (
                R::Format("written by a newer major format"),
                "error: p: the project opens read-only: it was written by a newer major format; nothing was written",
            ),
            (
                R::Format("written by a development build with an unreleased format"),
                "error: p: the project opens read-only: it was written by a development build with an unreleased \
                 format; nothing was written",
            ),
            (
                R::LockLost(None),
                "error: p: the project opens read-only: the lock was taken over; nothing was written",
            ),
        ] {
            assert_eq!(super::read_only_refusal(dir, &reason), text);
        }
    }

    /// FT-943: the Unicode bidirectional controls print escaped, like the control characters; other text is kept.
    #[test]
    fn printable_escapes_the_bidirectional_controls() {
        assert_eq!(super::printable("a\u{202E}b"), "a\\u{202e}b");
        assert_eq!(
            super::printable("\u{061C}\u{200E}\u{200F}\u{202A}\u{2066}\u{2069}x"),
            "\\u{61c}\\u{200e}\\u{200f}\\u{202a}\\u{2066}\\u{2069}x"
        );
        assert_eq!(super::printable("Jürgen \u{1b}"), "Jürgen \\u{1b}");
    }

    #[test]
    fn index_warning_prints_the_index_error_only_when_there_is_one() {
        let mut w = ew_api::library::WriteReport::default();
        assert_eq!(super::index_warning(&w), None);
        w.index_error = Some("disk full".into());
        let text = super::index_warning(&w).expect("a warning");
        assert!(text.starts_with("warning: "), "{text}");
        assert!(text.contains("disk full"), "{text}");
    }

    /// FIX40a (finding 8): the release run's record is kept per user, so the help names it as the person's own.
    #[test]
    fn the_help_names_the_release_record_as_the_users() {
        let mut cmd = super::command();
        cmd.build();
        let release = cmd
            .find_subcommand_mut("revision")
            .and_then(|r| r.find_subcommand_mut("release"))
            .expect("`revision release`");
        let help = release.render_long_help().to_string();
        let text = help.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            text.contains("`discard` is offered only when your record on this computer shows"),
            "the help text"
        );
        assert!(!text.contains("this computer's record"), "the help text");
    }

    use super::*;

    /// The command of a line, or clap's rendering of its error.
    fn p(args: &[&str]) -> Result<Command, String> {
        parse(args.iter().map(|s| (*s).to_owned())).map_err(|e| e.render().to_string())
    }

    /// clap's whole rendering of a usage error of the subcommand whose usage line is `usage`.
    fn usage_error(message: &str, usage: &str) -> Result<Command, String> {
        Err(format!(
            "error: {message}\n\nUsage: {usage}\n\nFor more information, try '--help'.\n"
        ))
    }

    /// clap's whole rendering of a value that the flag's value parser refuses with `message`.
    fn invalid(value: &str, flag: &str, message: &str) -> Result<Command, String> {
        Err(format!(
            "error: invalid value '{value}' for '{flag}': {message}\n\nFor more information, try '--help'.\n"
        ))
    }

    /// WP-S3.07 D9: `ew print` lowers its flags, the platform's default adapter resolved (`lp` on Linux, `windows` on
    /// Windows, else the viewer; ruling 4 of ID-1218); `--out` excludes `--via` and `--printer`; `--sections` reaches
    /// `export png` too.
    #[test]
    fn print_lowers_its_flags_and_resolves_the_adapter() {
        use ew_api::output::print::Paper;
        #[cfg(windows)]
        let default = PrintVia::Windows;
        #[cfg(not(windows))]
        let default = if cfg!(target_os = "linux") {
            PrintVia::Lp
        } else {
            PrintVia::Viewer
        };
        assert_eq!(
            p(&[
                "print",
                "proj",
                "--sections",
                "1,2",
                "--paper",
                "A5",
                "--fit",
                "--mono",
                "--out",
                "p.pdf"
            ]),
            Ok(Command::Print(PrintArgs {
                project: Some(PathBuf::from("proj")),
                pages: PageSelection::Sections(vec!["1".to_owned(), "2".to_owned()]),
                paper: Paper::A5,
                fit: true,
                mono: true,
                printer: None,
                via: default,
                out: Some(PathBuf::from("p.pdf")),
                json: false,
            }))
        );
        let Ok(Command::Print(a)) = p(&["print", "--via", "lp", "--printer", "office"]) else {
            panic!("`--via lp --printer`");
        };
        assert_eq!(
            (a.via, a.printer.as_deref(), a.paper, a.pages),
            (PrintVia::Lp, Some("office"), Paper::Sheet, PageSelection::All)
        );
        let printer = p(&["print", "--printer", "office"]);
        if cfg!(any(target_os = "linux", windows)) {
            assert!(matches!(printer, Ok(Command::Print(_))));
        } else {
            assert_eq!(
                printer,
                usage_error(
                    "`--printer` needs `--via lp` or `--via windows`; with the viewer, choose the printer in its print \
                     dialog",
                    "ew print [OPTIONS] [project-dir]"
                )
            );
        }
        assert_eq!(
            p(&["print", "--via", "viewer", "--printer", "office"]),
            usage_error(
                "`--printer` needs `--via lp` or `--via windows`; with the viewer, choose the printer in its print \
                 dialog",
                "ew print [OPTIONS] [project-dir]"
            )
        );
        // Ruling 4 of ID-1218: `--via windows` with or without a printer on Windows, refused in words elsewhere.
        let windows = p(&["print", "--via", "windows", "--printer", "office"]);
        #[cfg(windows)]
        {
            let Ok(Command::Print(a)) = windows else {
                panic!("`--via windows --printer`: {windows:?}");
            };
            assert_eq!((a.via, a.printer.as_deref()), (PrintVia::Windows, Some("office")));
        }
        #[cfg(not(windows))]
        assert_eq!(
            windows,
            usage_error(
                "`--via windows` prints on Windows only",
                "ew print [OPTIONS] [project-dir]"
            )
        );
        assert_eq!(
            p(&["print", "--out", "p.pdf", "--via", "lp"]),
            usage_error(
                "the argument '--out <file>' cannot be used with '--via <adapter>'",
                "ew print --out <file> [project-dir]"
            )
        );
        let Ok(Command::ExportPages(png)) = p(&["export", "png", "--out", "d", "--sections", "9"]) else {
            panic!("`export png --sections`");
        };
        assert_eq!(png.pages, PageSelection::Sections(vec!["9".to_owned()]));
        assert_eq!(
            p(&["export", "svg", "--out", "d", "--sections", "1,,2"]),
            invalid(
                "1,,2",
                "--sections <numbers>",
                "page selection: `1,,2` has an empty item (section numbers separated by `,`)"
            )
        );
    }

    /// What `ew print` says when a spooler took the set: through `lp`, or the Windows print spooler (ruling 4 of
    /// ID-1218), to the printer named or the default one.
    #[test]
    fn a_spooled_print_names_its_printer_and_its_way() {
        assert_eq!(
            super::spooled_line(4, None, "lp"),
            "4 pages sent to the default printer through `lp`"
        );
        assert_eq!(
            super::spooled_line(5, Some("Microsoft Print to PDF"), "windows"),
            "5 pages sent to printer Microsoft Print to PDF through the Windows print spooler"
        );
    }

    /// Cycle 43, ruling 4: a line without its subcommand prints the help on stderr and exits 2; `--help` and
    /// `--version` print on stdout and exit 0, the version as `ew <version>`.
    #[test]
    fn help_and_version_are_clap_s() {
        let e = |args: &[&str]| parse(args.iter().map(|s| (*s).to_owned())).unwrap_err();
        for args in [&[][..], &["lib"], &["import"], &["project", "checks"]] {
            let err = e(args);
            assert_eq!(
                err.kind(),
                clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
                "{args:?}"
            );
            assert_eq!(err.exit_code(), 2, "{args:?}");
        }
        for args in [
            &["--help"][..],
            &["-h"],
            &["help"],
            &["export", "pdf", "--help"],
            &["help", "lib", "search"],
        ] {
            let err = e(args);
            assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp, "{args:?}");
            assert_eq!(err.exit_code(), 0, "{args:?}");
        }
        for args in [&["--version"][..], &["-V"]] {
            let err = e(args);
            assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion, "{args:?}");
            assert_eq!(err.exit_code(), 0, "{args:?}");
            assert_eq!(
                err.render().to_string(),
                format!("ew {}\n", env!("CARGO_PKG_VERSION"))
            );
        }
        assert_eq!(
            p(&["frobnicate"]),
            usage_error("unrecognized subcommand 'frobnicate'", "ew <COMMAND>")
        );
    }

    /// C43-p4#2: a positional that takes hyphen values (`query`, `lib search`, `lib create`, `lib import parts`) takes
    /// a word starting with one `-`, but refuses one starting with `--` as a mistyped option (exit 2), as the old
    /// parser did.
    #[test]
    fn a_double_dash_word_in_a_hyphen_positional_is_a_mistyped_option() {
        let e = |args: &[&str]| parse(args.iter().map(|s| (*s).to_owned())).unwrap_err();
        for (args, value, positional) in [
            (&["lib", "search", "--alll"][..], "--alll", "<text>"),
            (
                &["query", "device_tag", "tag:-101F1", "--explian"],
                "--explian",
                "[key]",
            ),
            (&["query", "n", "k", "--bogus"], "--bogus", "[key]"),
            (
                &[
                    "lib",
                    "create",
                    "--frob",
                    "--key",
                    "k",
                    "--name",
                    "n",
                    "--licence",
                    "MIT",
                ],
                "--frob",
                "<dir>",
            ),
            (
                &[
                    "lib",
                    "import",
                    "parts",
                    "--lib",
                    "t.csv",
                    "--map",
                    "m.toml",
                    "--as-of",
                    "2026-01-01",
                ],
                "--lib",
                "<library>",
            ),
        ] {
            assert_eq!(
                p(args),
                invalid(value, positional, &format!("unknown option `{value}`")),
                "{args:?}"
            );
            assert_eq!(e(args).exit_code(), 2, "{args:?}");
        }
        let Ok(Command::Lib(l)) = p(&["lib", "search", "-X1"]) else {
            panic!("a search text starting with one `-`");
        };
        let LibAction::Search { text, .. } = l.action else {
            panic!("`lib search`");
        };
        assert_eq!(text, "-X1");
    }

    /// Decision log ID-1273: `merge-driver`'s help quotes doc 05 §11.1's three `.gitattributes` lines in full, as
    /// `ew git setup` will write them, and says its exits.
    #[test]
    fn merge_driver_s_help_quotes_the_attribute_lines_and_its_exits() {
        let cmd = command();
        let help = cmd
            .find_subcommand("merge-driver")
            .and_then(clap::Command::get_long_about)
            .map(ToString::to_string);
        assert_eq!(
            help.as_deref(),
            Some(
                "Merge one project file three-way: git's merge driver (plumbing).\n\nGit runs it for a file changed on \
                 both sides, with `%O` (the ancestor), `%A` (ours, where the result goes), `%B` (theirs), `%L` (the \
                 marker size, unused: conflicts are written as records, not markers) and `%P` (the file's path in the \
                 repository); the older form `%O %A %B %P` gives the same result. `%P` is taken relative to the \
                 deepest folder above it that holds a `manifest.json` in the current directory, the working tree, and \
                 selects the merge: a record file per record, a library document, `manifest.json`; any other file \
                 keeps ours. The three files are read under the reader's limits, and `%A` is replaced whole through a \
                 temporary file beside it. Git takes a driver only from its own configuration, so it is registered by \
                 two lines, `git config merge.elektrawatt.name \"ElektraWatt record merge\"` and `git config \
                 merge.elektrawatt.driver \"ew merge-driver %O %A %B %L %P\"`, and selected by three lines of the \
                 project's `.gitattributes`, `*.jsonl text eol=lf merge=elektrawatt diff=elektrawatt`, \
                 `lib/**/*.json text eol=lf merge=elektrawatt diff=elektrawatt` and `manifest.json text eol=lf \
                 merge=elektrawatt` (`eol=lf` keeps the manifest's hashes true under `core.autocrlf`). Exit 0 when \
                 merged cleanly, 1 when conflict records were written or a side or the merged file is larger than \
                 the reader's limits (`%A` then as it was), 129 on an internal error (a file that cannot be read or \
                 written, a failure in the merge) or a usage error, with `%A` as it was: git reads every code up to \
                 128 as conflicts written."
            )
        );
    }

    /// Decision log ID-1273 (ADR-0012 amendment 2): a usage error of `merge-driver` exits 129, which git reads as a
    /// failure rather than as conflicts written; a usage error of every other command keeps 2, the help 0.
    #[test]
    fn a_usage_error_of_merge_driver_exits_129_and_of_another_command_2() {
        let code = |args: &[&str]| {
            let args: Vec<std::ffi::OsString> = args.iter().map(Into::into).collect();
            let e = parse(args.iter().cloned()).expect_err("a usage error or the help");
            error_exit_code(&args, &e)
        };
        assert_eq!(code(&["merge-driver", "o", "a", "b"]), 129);
        assert_eq!(code(&["merge-driver"]), 129);
        assert_eq!(code(&["merge-driver", "--help"]), 0);
        assert_eq!(code(&["schema"]), 2);
        assert_eq!(code(&["check", "--frob"]), 2);
        assert_eq!(code(&["frobnicate", "merge-driver"]), 2);
    }

    /// Decision log ID-1273: `merge-driver`'s positionals take a leading `-` (a `%P` under a folder such as `-old.ew`),
    /// in both forms; three arguments are refused naming `<P>`, the argument the usage line shows as required.
    #[test]
    fn merge_driver_takes_a_leading_hyphen_and_names_a_missing_p() {
        let driver = |path: &str| {
            Ok(Command::MergeDriver(merge_driver::MergeDriverArgs {
                base: PathBuf::from("-o"),
                ours: PathBuf::from("-a"),
                theirs: PathBuf::from("-b"),
                path: path.to_owned(),
            }))
        };
        assert_eq!(
            p(&["merge-driver", "-o", "-a", "-b", "7", "-old.ew/pages/x.jsonl"]),
            driver("-old.ew/pages/x.jsonl")
        );
        assert_eq!(
            p(&["merge-driver", "-o", "-a", "-b", "-old.ew/manifest.json"]),
            driver("-old.ew/manifest.json")
        );
        assert_eq!(
            p(&["merge-driver", "o", "a", "b"]),
            usage_error(
                "the following required arguments were not provided:\n  <P>",
                "ew merge-driver <O> <A> <B> [L] <P>"
            )
        );
    }

    /// Cycle 43, ruling 3: a valued flag takes the next word even when it starts with `-`; a second `--project` is an
    /// error, `--company` accumulates, another flag is last-wins; a bare `--` ends the options.
    #[test]
    fn the_grammar_of_values_repeats_and_the_option_end() {
        let Ok(Command::Export(t)) = p(&["export", "terminals", "--strip", "--json"]) else {
            panic!("a value starting with `-`");
        };
        assert_eq!(
            (t.kind, t.json),
            (ExportKind::Terminals(Some("--json".to_owned())), false)
        );
        assert_eq!(
            p(&["fmt", "--project", "a", "--project", "b"]),
            usage_error(
                "the argument '--project <dir>' cannot be used multiple times",
                "ew fmt [OPTIONS] [project-dir]"
            )
        );
        let Ok(Command::Check(c)) = p(&["check", "--out", "a", "--out", "b", "--json", "--json"]) else {
            panic!("last-wins");
        };
        assert_eq!((c.out, c.format), (Some(PathBuf::from("b")), CheckFormat::Json));
        let Ok(Command::Check(c)) = p(&["check", "--lang", "NL-nl"]) else {
            panic!("--lang in any case");
        };
        assert_eq!(c.lang, "nl-NL");
        let Ok(Command::Fmt(f)) = p(&["fmt", "--", "--check"]) else {
            panic!("`--` ends the options");
        };
        assert_eq!((f.project, f.check), (Some(PathBuf::from("--check")), false));
        let Ok(Command::Query(q)) = p(&["query", "device_tag", "-X1031:5", "--json"]) else {
            panic!("a key starting with `-`");
        };
        assert_eq!((q.key.as_str(), q.json), ("-X1031:5", true));
        let Ok(Command::Lib(l)) = p(&["lib", "search", "-X1", "--company", "a", "--company", "b"]) else {
            panic!("a search text starting with `-`");
        };
        assert_eq!(l.company, [PathBuf::from("a"), PathBuf::from("b")]);
        let Ok(Command::Query(q)) = p(&["query", "proj", "device_tag", "-"]) else {
            panic!("`-` for a query without a key");
        };
        assert_eq!((q.project, q.key.as_str()), (Some(PathBuf::from("proj")), "-"));
        // `import csv --strict` names the default: the run reads only `lenient`.
        let Ok(Command::ImportCsv(strict)) = p(&["import", "csv", "t", "--out", "o", "--strict"]) else {
            panic!("`--strict` is the explicit default");
        };
        assert_eq!((strict.strict, strict.lenient), (true, false));
        assert_eq!(
            Ok(Command::ImportCsv(ImportCsvArgs {
                strict: false,
                ..strict
            })),
            p(&["import", "csv", "t", "--out", "o"])
        );
        assert_eq!(
            p(&["import", "csv", "t", "--out", "o", "--strict", "--lenient"]),
            usage_error(
                "the argument '--strict' cannot be used with '--lenient'",
                "ew import csv --out <dir> --strict <tables-dir>"
            )
        );
        // The refused `--lang` takes no flag as its value: `--lang --out o` is still the `--lang` refusal (C43-p4#5).
        for args in [
            &["export", "pdf", "--out", "o", "--lang", "de"][..],
            &["export", "pdf", "--lang", "--out", "o"],
        ] {
            assert_eq!(
                p(args),
                usage_error(
                    "`--lang` is not built yet",
                    "ew export pdf [OPTIONS] --out <path> [project-dir]"
                ),
                "{args:?}"
            );
        }
        assert_eq!(
            p(&["export", "png", "--out", "o", "--omit-personal-data"]),
            usage_error(
                "`--omit-personal-data` is not built yet",
                "ew export png [OPTIONS] --out <dir> [project-dir]"
            )
        );
        assert_eq!(
            p(&["import", "qet", "a.qet", "--json", "--report", "md"]),
            usage_error(
                "`--json` prints the result on stdout: give the report a file with `--report-out <file>`",
                "ew import qet [OPTIONS] <file.qet>"
            )
        );
        assert_eq!(
            p(&["check", "--json", "--format", "md"]),
            usage_error(
                "`--json` cannot be combined with `--format md` (`--json` is `--format json`)",
                "ew check [OPTIONS] [project-dir]"
            )
        );
        assert_eq!(
            p(&["sql", "--project", "p", "--file", "q", "proj"]),
            usage_error(
                "the argument '[project-dir]' cannot be used with '--project <dir>'",
                "ew sql [OPTIONS] [project-dir] <select>\n       ew sql [OPTIONS] [project-dir] --file <query.sql>\n       \
                 ew sql <COMMAND>"
            )
        );
        assert_eq!(
            p(&["sql", "SELECT 1", "--bogus"]),
            invalid("--bogus", "[select]", "unknown option `--bogus`")
        );
    }

    #[test]
    fn lib_commands() {
        let lib = |action, company: &[&str], json| {
            Ok(Command::Lib(LibArgs {
                action,
                company: company.iter().map(PathBuf::from).collect(),
                json,
            }))
        };
        assert_eq!(p(&["lib", "list"]), lib(LibAction::List, &[], false));
        assert_eq!(
            p(&["lib", "list", "--company", "a", "--json", "--company", "b"]),
            lib(LibAction::List, &["a", "b"], true)
        );
        assert_eq!(
            p(&["lib", "search", "A9P52610"]),
            lib(
                LibAction::Search {
                    text: "A9P52610".into(),
                    kinds: Vec::new(),
                    all: false,
                    limit: 50
                },
                &[],
                false
            )
        );
        assert_eq!(
            p(&[
                "lib", "search", "--kind", "symbol", "mcb", "--kind", "fndef", "--all", "--limit", "0",
                "--json"
            ]),
            lib(
                LibAction::Search {
                    text: "mcb".into(),
                    kinds: vec!["symbol".into(), "fndef".into()],
                    all: true,
                    limit: 0
                },
                &[],
                true
            )
        );
        assert_eq!(
            p(&["lib", "index", "--rebuild"]),
            lib(LibAction::IndexRebuild, &[], false)
        );
        assert_eq!(
            p(&[
                "lib",
                "create",
                "d",
                "--key",
                "k",
                "--name",
                "N",
                "--licence",
                "CC0-1.0",
                "--company",
                "c"
            ]),
            lib(
                LibAction::Create {
                    dir: PathBuf::from("d"),
                    key: "k".into(),
                    name: "N".into(),
                    licence: "CC0-1.0".into()
                },
                &["c"],
                false
            )
        );
        let create = "ew lib create --key <key> --name <name> --licence <spdx> <dir>";
        for (args, error) in [
            (
                &["lib", "frob"][..],
                usage_error("unrecognized subcommand 'frob'", "ew lib [OPTIONS] <COMMAND>"),
            ),
            (
                &["lib", "list", "x"],
                usage_error("unexpected argument 'x' found", "ew lib list [OPTIONS]"),
            ),
            (
                &["lib", "search"],
                usage_error(
                    "the following required arguments were not provided:\n  <text>",
                    "ew lib search <text>",
                ),
            ),
            (
                &["lib", "search", "a", "b"],
                usage_error("unexpected argument 'b' found", "ew lib search [OPTIONS] <text>"),
            ),
            (
                &["lib", "search", "a", "--kind", "device"],
                invalid(
                    "device",
                    "--kind <kind>",
                    "unknown kind `device` (part, symbol, fndef, form, macro, manufacturer)",
                ),
            ),
            (
                &["lib", "search", "a", "--limit", "x"],
                invalid("x", "--limit <n>", "`--limit` needs a number, not `x`"),
            ),
            (
                &["lib", "index"],
                usage_error(
                    "the following required arguments were not provided:\n  --rebuild",
                    "ew lib index --rebuild",
                ),
            ),
            (
                &["lib", "create", "d", "--key", "k", "--name", "N"],
                usage_error(
                    "the following required arguments were not provided:\n  --licence <spdx>",
                    create,
                ),
            ),
            (
                &["lib", "create", "--key", "k"],
                usage_error(
                    "the following required arguments were not provided:\n  --name <name>\n  --licence <spdx>\n  <dir>",
                    create,
                ),
            ),
            (
                &["lib", "list", "--company"],
                Err("error: a value is required for '--company <dir>' but none was supplied\n\nFor more information, \
                     try '--help'.\n"
                    .to_owned()),
            ),
        ] {
            assert_eq!(p(args), error, "{args:?}");
        }
    }

    #[test]
    fn import_qet_options() {
        let Ok(Command::ImportQet(a)) = p(&[
            "import",
            "qet",
            "a.qet",
            "--content",
            "c",
            "--report",
            "md",
            "--report-out",
            "r.md",
        ]) else {
            panic!("parse failed");
        };
        assert_eq!(a.file, PathBuf::from("a.qet"));
        assert_eq!(a.content, Some(PathBuf::from("c")));
        assert_eq!(a.report, Some(ReportFormat::Markdown));
        assert_eq!(a.report_out, Some(PathBuf::from("r.md")));
        let Ok(Command::ImportQet(b)) = p(&["import", "qet", "--report", "json", "x.qet"]) else {
            panic!("parse failed");
        };
        assert_eq!(b.report, Some(ReportFormat::Json));
        assert_eq!(b.content, None);
        assert!(!b.strict);
        let Ok(Command::ImportQet(c)) = p(&["import", "qet", "--strict", "x.qet"]) else {
            panic!("parse failed");
        };
        assert!(c.strict);
    }

    #[test]
    fn check_defaults_and_options() {
        let Ok(Command::Check(a)) = p(&["check"]) else {
            panic!("parse failed");
        };
        assert_eq!(a.project, None);
        assert_eq!(a.scheme, None);
        assert_eq!(a.mode, CheckMode::Release);
        assert_eq!(a.format, CheckFormat::Text);
        assert_eq!(a.out, None);
        let Ok(Command::Check(b)) = p(&[
            "check", "proj", "--scheme", "drafting", "--mode", "online", "--format", "json", "--out",
            "r.json",
        ]) else {
            panic!("parse failed");
        };
        assert_eq!(b.project, Some(PathBuf::from("proj")));
        assert_eq!(b.scheme.as_deref(), Some("drafting"));
        assert_eq!(b.mode, CheckMode::Online);
        assert_eq!(b.format, CheckFormat::Json);
        assert_eq!(b.out, Some(PathBuf::from("r.json")));
        let Ok(Command::Check(c)) = p(&["check", "--format", "md", "proj"]) else {
            panic!("parse failed");
        };
        assert_eq!(c.format, CheckFormat::Markdown);
    }

    #[test]
    fn check_errors() {
        assert!(p(&["check", "--scheme", "bogus"]).is_err());
        assert!(p(&["check", "--mode", "x"]).is_err());
        assert!(p(&["check", "--format", "xml"]).is_err());
        assert!(p(&["check", "a", "b"]).is_err());
        assert!(p(&["check", "--bogus"]).is_err());
        assert!(p(&["check", "--scheme"]).is_err());
    }

    fn sql(args: &[&str]) -> SqlArgs {
        let mut v = vec!["sql"];
        v.extend_from_slice(args);
        match p(&v) {
            Ok(Command::Sql(a)) => a,
            other => panic!("parse failed: {other:?}"),
        }
    }

    #[test]
    fn sql_defaults_and_positionals() {
        let a = sql(&["SELECT 1"]);
        assert_eq!(a.project, None);
        assert_eq!(a.query, SqlSource::Text("SELECT 1".to_owned()));
        assert_eq!(a.format, SqlFormat::Csv);
        assert_eq!(a.timeout, None);
        assert_eq!(a.out, None);
        let b = sql(&[
            "proj",
            "SELECT 1",
            "--format",
            "json",
            "--timeout",
            "5",
            "--out",
            "r.json",
        ]);
        assert_eq!(b.project, Some(PathBuf::from("proj")));
        assert_eq!(b.query, SqlSource::Text("SELECT 1".to_owned()));
        assert_eq!(b.format, SqlFormat::Json);
        assert_eq!(b.timeout, Some(5));
        assert_eq!(b.out, Some(PathBuf::from("r.json")));
        let c = sql(&["--file", "q.sql"]);
        assert_eq!(c.project, None);
        assert_eq!(c.query, SqlSource::File(PathBuf::from("q.sql")));
        let d = sql(&["proj", "--file", "q.sql"]);
        assert_eq!(d.project, Some(PathBuf::from("proj")));
        let e = sql(&["--project", "proj", "SELECT 1"]);
        assert_eq!(e.project, Some(PathBuf::from("proj")));
        assert_eq!(e.query, SqlSource::Text("SELECT 1".to_owned()));
        let f = sql(&["--file", "q.sql", "--project", "proj"]);
        assert_eq!(f.project, Some(PathBuf::from("proj")));
    }

    #[test]
    fn sql_dump_and_schema_forms() {
        assert_eq!(
            p(&["sql", "dump", "o.sqlite"]),
            Ok(Command::SqlDump(SqlDumpArgs {
                project: None,
                out: PathBuf::from("o.sqlite"),
                json: false,
            }))
        );
        assert_eq!(
            p(&["sql", "dump", "proj", "o.sqlite"]),
            Ok(Command::SqlDump(SqlDumpArgs {
                project: Some(PathBuf::from("proj")),
                out: PathBuf::from("o.sqlite"),
                json: false,
            }))
        );
        assert_eq!(
            p(&["sql", "dump", "--project", "proj", "o.sqlite"]),
            Ok(Command::SqlDump(SqlDumpArgs {
                project: Some(PathBuf::from("proj")),
                out: PathBuf::from("o.sqlite"),
                json: false,
            }))
        );
        assert_eq!(p(&["sql", "schema"]), Ok(Command::SqlSchema));
    }

    #[test]
    fn sql_errors() {
        // WP-S3.08 D7 (FT-218): `--format xlsx` writes a file, and the CSV options apply to CSV only.
        assert_eq!(
            p(&["sql", "SELECT 1", "--format", "xlsx"]),
            usage_error(
                "`--format xlsx` writes a file: give `--out <file.xlsx>`",
                "ew sql [OPTIONS] [project-dir] <select>\n       ew sql [OPTIONS] [project-dir] --file <query.sql>\n       \
                 ew sql <COMMAND>"
            )
        );
        assert_eq!(
            p(&["sql", "SELECT 1", "--json", "--preset", "excel-nl"]),
            usage_error(
                "`--preset` applies to CSV only; drop it, or give `--format csv`",
                "ew sql [OPTIONS] [project-dir] <select>\n       ew sql [OPTIONS] [project-dir] --file <query.sql>\n       \
                 ew sql <COMMAND>"
            )
        );
        assert_eq!(
            p(&[
                "sql",
                "SELECT 1",
                "--format",
                "xlsx",
                "--out",
                "q.xlsx",
                "--line-end",
                "lf"
            ]),
            usage_error(
                "`--line-end` applies to CSV only; drop it, or give `--format csv`",
                "ew sql [OPTIONS] [project-dir] <select>\n       ew sql [OPTIONS] [project-dir] --file <query.sql>\n       \
                 ew sql <COMMAND>"
            )
        );
        assert_eq!(
            p(&["sql", "SELECT 1", "--format", "x"]),
            invalid("x", "--format <format>", "unknown format `x` (csv, json, xlsx)")
        );
        assert!(p(&["sql", "SELECT 1", "--timeout", "0"]).is_err());
        assert!(p(&["sql", "SELECT 1", "--timeout", "abc"]).is_err());
        assert!(p(&["sql", "SELECT 1", "--timeout", "-1"]).is_err());
        assert!(p(&["sql", "SELECT 1", "--timeout"]).is_err());
        // The timeout is capped at one day: no overflow of the clock.
        assert!(p(&["sql", "SELECT 1", "--timeout", "18446744073709551615"]).is_err());
        assert!(p(&["sql", "SELECT 1", "--timeout", "86401"]).is_err());
        assert!(matches!(
            p(&["sql", "SELECT 1", "--timeout", "86400"]),
            Ok(Command::Sql(SqlArgs {
                timeout: Some(86_400),
                ..
            }))
        ));
        // An inline query may start with an SQL comment.
        let Ok(Command::Sql(c)) = p(&[
            "sql",
            "-- count
SELECT 1",
        ]) else {
            panic!("a query starting with a comment is not an option");
        };
        assert_eq!(
            c.query,
            SqlSource::Text(
                "-- count
SELECT 1"
                    .to_owned()
            )
        );
        assert!(
            p(&[
                "sql",
                "proj",
                "-- c
SELECT 1"
            ])
            .is_ok()
        );
        assert!(p(&["sql", "proj", "SELECT 1", "--file", "q.sql"]).is_err());
        assert!(p(&["sql", "a", "b", "c"]).is_err());
        assert!(p(&["sql"]).is_err());
        assert!(p(&["sql", "schema", "extra"]).is_err());
        assert!(p(&["sql", "dump"]).is_err());
        assert!(p(&["sql", "dump", "a", "b", "c"]).is_err());
        assert!(p(&["sql", "SELECT 1", "--bogus"]).is_err());
        assert!(p(&["sql", "dump", "a", "--bogus"]).is_err());
        assert!(p(&["sql", "--project", "p", "proj", "SELECT 1"]).is_err());
        assert!(p(&["sql", "--project", "p", "proj", "--file", "q.sql"]).is_err());
        assert!(p(&["sql", "dump", "--project", "p", "a", "b"]).is_err());
    }

    #[test]
    fn import_parts_options() {
        let Ok(Command::ImportParts(a)) = p(&["import", "parts", "a.csv"]) else {
            panic!("import parts");
        };
        assert_eq!(a.file, PathBuf::from("a.csv"));
        assert_eq!(a.project, None);
        let Ok(Command::ImportParts(b)) = p(&["import", "parts", "--project", "d", "a.csv"]) else {
            panic!("import parts --project");
        };
        assert_eq!(b.project, Some(PathBuf::from("d")));
    }

    #[test]
    fn import_parts_errors() {
        assert!(p(&["import", "parts"]).is_err());
        assert!(p(&["import", "parts", "a", "b"]).is_err());
        assert!(p(&["import", "parts", "a", "--bogus"]).is_err());
        assert!(p(&["import", "parts", "a", "--project"]).is_err());
    }

    /// WP-S1.19 part 4: `import parts --map` and `lib import parts`.
    #[test]
    fn mapped_parts_imports() {
        let Ok(Command::ImportParts(a)) = p(&[
            "import",
            "parts",
            "list.xlsx",
            "--map",
            "m.toml",
            "--as-of",
            "2026-10-29",
            "--company",
            "c",
        ]) else {
            panic!("import parts --map");
        };
        assert_eq!(a.map, Some(PathBuf::from("m.toml")));
        assert_eq!(a.as_of, Some("2026-10-29".parse().unwrap()));
        assert_eq!(a.company, [PathBuf::from("c")]);
        let Ok(Command::ImportParts(b)) = p(&["import", "parts", "parts.csv"]) else {
            panic!("import parts");
        };
        assert_eq!((b.map, b.as_of, b.company), (None, None, Vec::new()));
        assert_eq!(
            p(&[
                "lib",
                "import",
                "parts",
                "std",
                "list.csv",
                "--map",
                "m.toml",
                "--as-of",
                "2026-10-29",
                "--bump",
                "--take-over",
                "--company",
                "c"
            ]),
            Ok(Command::Lib(LibArgs {
                action: LibAction::ImportParts {
                    library: "std".into(),
                    file: PathBuf::from("list.csv"),
                    map: PathBuf::from("m.toml"),
                    as_of: "2026-10-29".parse().unwrap(),
                    bump: true,
                    take_over: true,
                },
                company: vec![PathBuf::from("c")],
                json: false,
            }))
        );
        let lib_import = "ew lib import parts --map <map.toml> --as-of <date> <library> <table>";
        let missing = "the following required arguments were not provided:";
        for (args, error) in [
            (
                &["import", "parts", "a.csv", "--as-of", "2026-10-29"][..],
                usage_error(
                    &format!("{missing}\n  --map <map.toml>"),
                    "ew import parts --map <map.toml> --as-of <date> <table>",
                ),
            ),
            (
                &["import", "parts", "a.csv", "--map", "m", "--as-of", "29.10.2026"],
                invalid(
                    "29.10.2026",
                    "--as-of <date>",
                    "`--as-of` needs a date YYYY-MM-DD, not `29.10.2026`",
                ),
            ),
            (
                &["lib", "import", "macros"],
                Err("error: unrecognized subcommand 'macros'\n\n  tip: a similar subcommand exists: 'parts'\n\nUsage: \
                     ew lib import [OPTIONS] <COMMAND>\n\nFor more information, try '--help'.\n"
                    .to_owned()),
            ),
            (
                &["lib", "import", "parts", "std"],
                usage_error(
                    &format!("{missing}\n  --map <map.toml>\n  --as-of <date>\n  <table>"),
                    lib_import,
                ),
            ),
            (
                &["lib", "import", "parts", "std", "a.csv", "--as-of", "2026-10-29"],
                usage_error(&format!("{missing}\n  --map <map.toml>"), lib_import),
            ),
            (
                &["lib", "import", "parts", "std", "a.csv", "--map", "m"],
                usage_error(&format!("{missing}\n  --as-of <date>"), lib_import),
            ),
            (
                &["lib", "import", "parts", "std", "a.csv", "b.csv"],
                usage_error(
                    "unexpected argument 'b.csv' found",
                    "ew lib import parts [OPTIONS] --map <map.toml> --as-of <date> <library> <table>",
                ),
            ),
            (
                &["lib", "list", "--bump"],
                usage_error("unexpected argument '--bump' found", "ew lib list [OPTIONS]"),
            ),
        ] {
            assert_eq!(p(args), error, "{args:?}");
        }
    }

    #[test]
    fn import_qet_errors() {
        assert!(p(&["import"]).is_err());
        assert!(p(&["import", "dxf", "a"]).is_err());
        assert!(p(&["import", "qet"]).is_err());
        assert!(p(&["import", "qet", "a", "b"]).is_err());
        assert!(p(&["import", "qet", "a", "--bogus"]).is_err());
        assert!(p(&["import", "qet", "a", "--report"]).is_err());
        assert!(p(&["import", "qet", "a", "--report", "xml"]).is_err());
    }
    #[test]
    fn export_forms() {
        let Ok(Command::Export(a)) = p(&["export", "cables"]) else {
            panic!("parse failed");
        };
        assert_eq!(a.kind, ExportKind::Cables);
        assert_eq!((a.project, a.out), (None, None));
        let Ok(Command::Export(b)) = p(&["export", "cables", "proj", "--format", "csv", "--out", "c.csv"])
        else {
            panic!("parse failed");
        };
        assert_eq!(b.project, Some(PathBuf::from("proj")));
        assert_eq!(b.out, Some(PathBuf::from("c.csv")));
        // The parts list is the default of `export bom`; `component` is the doc 17 name of `parts`.
        for (args, mode) in [
            (&["export", "bom"][..], BomMode::Component),
            (&["export", "bom", "--mode", "parts"][..], BomMode::Component),
            (&["export", "bom", "--mode", "component"][..], BomMode::Component),
            (
                &["export", "bom", "proj", "--mode", "assembly"][..],
                BomMode::Assembly,
            ),
        ] {
            let Ok(Command::Export(c)) = p(args) else {
                panic!("{args:?}: parse failed");
            };
            assert_eq!(c.kind, ExportKind::Bom(mode), "{args:?}");
        }
    }

    #[test]
    fn export_errors() {
        assert!(p(&["export"]).is_err());
        assert!(p(&["export", "nets"]).is_err());
        assert!(p(&["export", "cables", "a", "b"]).is_err());
        assert!(p(&["export", "cables", "--bogus"]).is_err());
        assert!(p(&["export", "cables", "--out"]).is_err());
        // `--mode` belongs to `bom`; the other formats are not built.
        assert!(p(&["export", "cables", "--mode", "parts"]).is_err());
        assert!(p(&["export", "bom", "--mode", "everything"]).is_err());
        assert!(p(&["export", "bom", "--mode"]).is_err());
        assert!(p(&["export", "bom", "--format", "xlsx"]).is_err());
        assert!(p(&["export", "bom", "--format", "pdf"]).is_err());
        assert!(p(&["export", "bom", "--format", "tsv"]).is_err());
        // `--strip` belongs to `terminals`, `--block` needs a value, `report` needs a block and PDF a file.
        assert!(p(&["export", "wires", "--strip", "-X1"]).is_err());
        assert!(p(&["export", "wires", "--block"]).is_err());
        assert!(p(&["export", "report"]).is_err());
        assert!(p(&["export", "report", "--block", "908", "--format", "pdf"]).is_err());
        assert!(p(&["export", "report", "--block", "908", "--format", "svg"]).is_err());
        // WP-S3.08 D6, D7: `--format xlsx` needs `--out` and takes no CSV option; the value refusals name the choices.
        for what in ["bom", "cables", "wires", "terminals"] {
            let usage = format!("ew export {what} [OPTIONS] [project-dir]");
            assert_eq!(
                p(&["export", what, "--format", "xlsx"]),
                usage_error("`--format xlsx` writes a file: give `--out <file.xlsx>`", &usage)
            );
            assert_eq!(
                p(&[
                    "export", what, "--format", "xlsx", "--out", "x.xlsx", "--preset", "excel-nl"
                ]),
                usage_error(
                    "`--preset` applies to CSV only; drop it, or give `--format csv`",
                    &usage
                )
            );
            assert_eq!(
                p(&[
                    "export", what, "--format", "xlsx", "--out", "x.xlsx", "--bom", "on"
                ]),
                usage_error(
                    "`--bom` applies to CSV only; drop it, or give `--format csv`",
                    &usage
                )
            );
            assert_eq!(
                p(&["export", what, "--preset", "excel"]),
                invalid(
                    "excel",
                    "--preset <preset>",
                    "unknown preset `excel` (ew, excel-nl, excel-en, rfc4180)"
                )
            );
            assert_eq!(
                p(&["export", what, "--bom", "yes"]),
                invalid(
                    "yes",
                    "--bom <on|off>",
                    "unknown byte-order mark setting `yes` (on, off)"
                )
            );
            assert_eq!(
                p(&["export", what, "--line-end", "cr"]),
                invalid("cr", "--line-end <crlf|lf>", "unknown line end `cr` (crlf, lf)")
            );
            assert_eq!(
                p(&["export", what, "--format", "tsv"]),
                invalid("tsv", "--format <format>", "unknown format `tsv` (csv, xlsx)")
            );
        }
        let report = "ew export report [OPTIONS] <--block <block>|--kind <kind>> [project-dir]";
        assert_eq!(
            p(&["export", "report", "--block", "908", "--format", "xlsx"]),
            usage_error("`--format xlsx` writes a file: give `--out <file.xlsx>`", report)
        );
        assert_eq!(
            p(&[
                "export", "report", "--block", "908", "--format", "pdf", "--out", "r.pdf", "--preset",
                "rfc4180",
            ]),
            usage_error(
                "`--preset` applies to CSV only; drop it, or give `--format csv`",
                report
            )
        );
        assert_eq!(
            p(&["export", "report", "--block", "908", "--format", "svg"]),
            invalid(
                "svg",
                "--format <format>",
                "unknown format `svg` (csv, xlsx, pdf)"
            )
        );
        // Cycle-40 review, part 3, finding 5: the row exports write rows, and the refusal points to the report PDF.
        for what in ["bom", "cables", "wires", "terminals"] {
            assert_eq!(
                p(&["export", what, "--format", "pdf"]),
                usage_error(
                    &format!(
                        "`export {what}` writes rows as CSV or XLSX; a report block's pages go to PDF with `ew export report --block <block> --format pdf`"
                    ),
                    &format!("ew export {what} [OPTIONS] [project-dir]")
                )
            );
        }
    }

    /// WP-S3.08 D6, D7: `--format xlsx` with `--out`, `--preset`, `--bom` and `--line-end` lower to the row format;
    /// the default is the golden dialect.
    #[test]
    fn row_export_formats() {
        use ew_api::output::{CsvOptions, LineEnd, Preset};
        let Ok(Command::Export(a)) = p(&["export", "wires", "--format", "xlsx", "--out", "w.xlsx"]) else {
            panic!("parse failed");
        };
        assert_eq!(
            (a.format, a.out),
            (RowFormat::Xlsx, Some(PathBuf::from("w.xlsx")))
        );
        let Ok(Command::Export(a)) = p(&[
            "export",
            "bom",
            "--preset",
            "excel-nl",
            "--bom",
            "off",
            "--line-end",
            "lf",
        ]) else {
            panic!("parse failed");
        };
        assert_eq!(
            a.format,
            RowFormat::Csv(CsvOptions {
                preset: Preset::ExcelNl,
                bom: Some(false),
                line_end: Some(LineEnd::Lf),
            })
        );
        let Ok(Command::Export(a)) = p(&[
            "export", "report", "--kind", "io_list", "--format", "xlsx", "--out", "k.xlsx",
        ]) else {
            panic!("parse failed");
        };
        assert_eq!(
            (a.kind, a.format),
            (ExportKind::ReportKind("io_list".to_owned()), RowFormat::Xlsx)
        );
        let Ok(Command::Export(a)) = p(&["export", "terminals"]) else {
            panic!("parse failed");
        };
        assert_eq!(a.format, RowFormat::default());
        let Ok(Command::Sql(q)) = p(&["sql", "SELECT 1", "--format", "xlsx", "--out", "q.xlsx"]) else {
            panic!("parse failed");
        };
        assert_eq!((q.format, q.csv), (SqlFormat::Xlsx, CsvOptions::default()));
        let Ok(Command::Sql(q)) = p(&["sql", "SELECT 1", "--preset", "rfc4180"]) else {
            panic!("parse failed");
        };
        assert_eq!(q.csv, CsvOptions::preset(Preset::Rfc4180));
    }

    /// D8: `wires`, `terminals [--strip]`, `report --block … --format csv|pdf` and `--block` on the row exports.
    #[test]
    fn row_export_forms() {
        let Ok(Command::Export(a)) = p(&["export", "wires", "proj", "--block", "907"]) else {
            panic!("parse failed");
        };
        assert_eq!(
            (a.kind, a.block, a.project),
            (
                ExportKind::Wires,
                Some("907".to_owned()),
                Some(PathBuf::from("proj"))
            )
        );
        let Ok(Command::Export(t)) = p(&["export", "terminals", "--strip", "-X1031"]) else {
            panic!("parse failed");
        };
        assert_eq!(t.kind, ExportKind::Terminals(Some("-X1031".to_owned())));
        let Ok(Command::Export(t)) = p(&["export", "terminals"]) else {
            panic!("parse failed");
        };
        assert_eq!((t.kind, t.block), (ExportKind::Terminals(None), None));
        let Ok(Command::Export(b)) = p(&["export", "bom", "--block", "x", "--mode", "assembly"]) else {
            panic!("parse failed");
        };
        assert_eq!(
            (b.kind, b.block),
            (ExportKind::Bom(BomMode::Assembly), Some("x".to_owned()))
        );
        let Ok(Command::Export(r)) = p(&["export", "report", "--block", "908"]) else {
            panic!("parse failed");
        };
        assert_eq!(r.kind, ExportKind::Report { pdf: false });
        let Ok(Command::Export(r)) = p(&[
            "export", "report", "--block", "908", "--format", "pdf", "--out", "r.pdf",
        ]) else {
            panic!("parse failed");
        };
        assert_eq!(
            (r.kind, r.out),
            (ExportKind::Report { pdf: true }, Some(PathBuf::from("r.pdf")))
        );
    }

    /// WP-S3.06 D4 (the orchestrator's ruling 4 of cycle 42): `export report --kind <kind>` is CSV only and excludes
    /// `--block`; each refusal is the report layer's text, whole, given before the project opens.
    #[test]
    fn report_kind_forms_and_refusals() {
        let Ok(Command::Export(r)) = p(&[
            "export",
            "report",
            "proj",
            "--kind",
            "device_list",
            "--format",
            "csv",
        ]) else {
            panic!("parse failed");
        };
        assert_eq!(
            (r.kind, r.block, r.project),
            (
                ExportKind::ReportKind("device_list".to_owned()),
                None,
                Some(PathBuf::from("proj"))
            )
        );
        let report = "ew export report <--block <block>|--kind <kind>> [project-dir]";
        // WP-S3.06 D4's refusal, before the pages refusal (C43-p4#4).
        for args in [
            &["export", "report", "--kind", "device_list", "--block", "910"][..],
            &[
                "export",
                "report",
                "--block",
                "910",
                "--kind",
                "device_list",
                "--format",
                "pdf",
                "--out",
                "r.pdf",
            ],
        ] {
            assert_eq!(
                p(args),
                usage_error(
                    "give `--block` or `--kind`, not both",
                    "ew export report [OPTIONS] <--block <block>|--kind <kind>> [project-dir]"
                ),
                "{args:?}"
            );
        }
        assert_eq!(
            p(&[
                "export",
                "report",
                "--kind",
                "device_list",
                "--format",
                "pdf",
                "--out",
                "r.pdf"
            ]),
            usage_error(
                "`--kind` exports rows: a kind has pages only through a block, use `--block <block> --format pdf`",
                "ew export report [OPTIONS] <--block <block>|--kind <kind>> [project-dir]"
            )
        );
        assert_eq!(
            p(&["export", "report"]),
            usage_error(
                "the following required arguments were not provided:\n  <--block <block>|--kind <kind>>",
                report
            )
        );
        assert_eq!(
            p(&["export", "wires", "--kind", "device_list"]),
            Err("error: unexpected argument '--kind' found\n\n  tip: to pass '--kind' as a value, use '-- --kind'\n\n\
                 Usage: ew export wires [OPTIONS] [project-dir]\n\nFor more information, try '--help'.\n"
                .to_owned())
        );
    }

    /// D7: `render --page <folio> --png [--dpi] [--out]` and `export png --dpi`.
    #[test]
    fn render_and_png_forms() {
        assert_eq!(
            p(&["render", "proj", "--page", "103", "--png"]),
            Ok(Command::Render(RenderArgs {
                project: Some(PathBuf::from("proj")),
                page: "103".to_owned(),
                dpi: 150,
                out: None,
                json: false,
            }))
        );
        let Ok(Command::Render(r)) = p(&[
            "render", "--page", "2.1", "--png", "--dpi", "300", "--out", "x.png", "--json",
        ]) else {
            panic!("parse failed");
        };
        assert_eq!((r.dpi, r.out, r.json), (300, Some(PathBuf::from("x.png")), true));
        assert!(p(&["render", "--page", "103"]).is_err(), "--png is required");
        assert!(p(&["render", "--png"]).is_err(), "--page is required");
        for bad in ["71", "601", "x", "0"] {
            assert!(
                p(&["render", "--page", "1", "--png", "--dpi", bad]).is_err(),
                "{bad}"
            );
        }
        assert!(p(&["render", "--page", "1", "--png", "--dpi", "72"]).is_ok());
        assert!(p(&["render", "--page", "1", "--png", "--dpi", "600"]).is_ok());
        let Ok(Command::ExportPages(e)) =
            p(&["export", "png", "--out", "o", "--dpi", "300", "--threads", "4"])
        else {
            panic!("parse failed");
        };
        assert_eq!((e.format, e.dpi, e.threads), (PageFormat::Png, 300, Some(4)));
        assert!(p(&["export", "png"]).is_err(), "--out is required");
        // `--dpi` is the raster's: PDF and SVG refuse it.
        assert!(p(&["export", "svg", "--out", "o", "--dpi", "300"]).is_err());
        assert!(p(&["export", "pdf", "--out", "o", "--dpi", "300"]).is_err());
    }

    #[test]
    fn export_pages_forms() {
        let Ok(Command::ExportPages(a)) = p(&["export", "pdf", "proj", "--out", "o.pdf"]) else {
            panic!("parse failed");
        };
        assert_eq!(a.format, PageFormat::Pdf);
        assert_eq!(a.project, Some(PathBuf::from("proj")));
        assert_eq!(a.out, PathBuf::from("o.pdf"));
        assert_eq!((a.pages, a.threads), (PageSelection::All, None));
        let Ok(Command::ExportPages(b)) = p(&[
            "export",
            "svg",
            "--out",
            "svg",
            "--pages",
            "101-106,1",
            "--threads",
            "4",
        ]) else {
            panic!("parse failed");
        };
        assert_eq!(b.format, PageFormat::Svg);
        assert_eq!(b.project, None);
        assert_eq!(
            b.pages,
            PageSelection::Folios(vec!["101-106".to_owned(), "1".to_owned()])
        );
        assert_eq!(b.threads, Some(4));
    }

    #[test]
    fn export_pages_errors() {
        assert!(p(&["export", "pdf"]).is_err(), "--out is required");
        assert!(p(&["export", "svg", "proj"]).is_err());
        assert!(p(&["export", "pdf", "--out"]).is_err());
        assert!(p(&["export", "pdf", "--out", "o", "--pages", ""]).is_err());
        assert!(p(&["export", "pdf", "--out", "o", "--threads", "0"]).is_err());
        assert!(p(&["export", "pdf", "--out", "o", "--threads", "x"]).is_err());
        assert!(p(&["export", "pdf", "--out", "o", "--lang", "de"]).is_err());
        assert!(p(&["export", "pdf", "--out", "o", "a", "b"]).is_err());
        assert!(p(&["export", "png", "--out", "o", "--dpi", "1000"]).is_err());
    }

    #[test]
    fn apply_forms() {
        assert_eq!(
            p(&["apply", "proj", "x.ewcmd", "--preview"]),
            Ok(Command::Apply(ApplyArgs {
                project: Some(PathBuf::from("proj")),
                log: PathBuf::from("x.ewcmd"),
                preview: true,
                one_step: false,
                json: false,
            }))
        );
        assert_eq!(
            p(&["apply", "--one-step", "x.ewcmd"]),
            Ok(Command::Apply(ApplyArgs {
                project: None,
                log: PathBuf::from("x.ewcmd"),
                preview: false,
                one_step: true,
                json: false,
            }))
        );
        assert!(p(&["apply"]).is_err());
        assert!(p(&["apply", "a", "b", "c"]).is_err());
        assert!(p(&["apply", "x.ewcmd", "--bogus"]).is_err());
    }

    #[test]
    fn verify_derived_forms() {
        assert_eq!(
            p(&["verify-derived"]),
            Ok(Command::VerifyDerived(VerifyArgs {
                project: None,
                json: false
            }))
        );
        assert_eq!(
            p(&["verify-derived", "proj"]),
            Ok(Command::VerifyDerived(VerifyArgs {
                project: Some(PathBuf::from("proj")),
                json: false,
            }))
        );
        assert!(p(&["verify-derived", "a", "b"]).is_err());
        assert!(p(&["verify-derived", "--bogus"]).is_err());
    }
}

#[cfg(test)]
mod checks_set_tests {
    use super::*;

    fn args(active: Option<&str>, release: Option<&str>) -> ChecksSetArgs {
        ChecksSetArgs {
            project: None,
            active: active.map(str::to_owned),
            release: release.map(str::to_owned),
        }
    }

    fn typed(a: &ChecksSetArgs, answer: &str) -> (bool, String) {
        let mut input = std::io::Cursor::new(answer.as_bytes().to_vec());
        let mut output = Vec::new();
        let ok = confirm_typed(a, &mut input, &mut output);
        (ok, String::from_utf8(output).unwrap())
    }

    #[test]
    fn the_typed_confirmation_is_the_scheme_name_exactly() {
        let a = args(Some("drafting"), Some("full"));
        let (ok, shown) = typed(&a, "drafting\n");
        assert!(ok);
        assert!(shown.contains("active scheme: drafting"), "{shown}");
        assert!(shown.contains("release scheme: full"), "{shown}");
        assert!(shown.contains("decides check outcomes"), "{shown}");
        assert!(shown.contains("Type `drafting`"), "{shown}");
        for wrong in ["", "\n", "y\n", "yes\n", "Drafting\n", "full\n", "drafting-x\n"] {
            assert!(!typed(&a, wrong).0, "{wrong:?} must abort");
        }
        // Only a release scheme: its name is the word, and the active one is said to return to its default.
        let b = args(None, Some("pre-delivery"));
        let (ok, shown) = typed(&b, "  pre-delivery \r\n");
        assert!(ok);
        assert!(
            shown.contains("active scheme: back to the default (drafting)"),
            "{shown}"
        );
    }

    #[test]
    fn the_command_carries_the_given_schemes_only() {
        let c = checks_set_command(&args(None, Some("full")));
        assert_eq!(c.name, "project.checks.set");
        assert_eq!(c.params, serde_json::json!({"release_scheme": "full"}));
    }

    #[test]
    fn the_subcommand_takes_no_yes_flag() {
        let parse = |a: &[&str]| parse(a.iter().map(|s| (*s).to_owned())).map_err(|e| e.render().to_string());
        assert!(parse(&["project", "checks", "set", "--release", "full", "--confirm"]).is_err());
        assert!(parse(&["project", "checks", "set", "--yes"]).is_err());
        assert!(
            parse(&["project", "checks", "set"]).is_err(),
            "a scheme is needed"
        );
        assert_eq!(
            parse(&["project", "checks", "set", "p.ew", "--active", "full"]),
            Ok(Command::ChecksSet(ChecksSetArgs {
                project: Some(PathBuf::from("p.ew")),
                active: Some("full".to_owned()),
                release: None,
            }))
        );
    }
}

#[cfg(test)]
mod assets_purge_tests {
    use super::*;

    #[test]
    fn assets_purge_parses_its_directory_and_preview_flag() {
        let parse = |a: &[&str]| parse(a.iter().map(|s| (*s).to_owned())).map_err(|e| e.render().to_string());
        assert_eq!(
            parse(&["project", "assets", "purge"]),
            Ok(Command::AssetsPurge(AssetsPurgeArgs {
                project: None,
                preview: false,
            }))
        );
        assert_eq!(
            parse(&["project", "assets", "purge", "p.ew", "--preview"]),
            Ok(Command::AssetsPurge(AssetsPurgeArgs {
                project: Some(PathBuf::from("p.ew")),
                preview: true,
            }))
        );
        assert!(parse(&["project", "assets", "purge", "a", "b"]).is_err());
        assert!(parse(&["project", "assets", "purge", "--yes"]).is_err());
        assert!(parse(&["project", "assets"]).is_err());
    }

    /// A saved project in a fresh directory whose first logo's asset file nothing names any more (the logo was
    /// replaced and saved, and the session that did it is closed).
    fn project_with_an_unnamed_asset(tag: &str) -> PathBuf {
        let dir = ew_testdir::fresh_dir(&format!("ew-cli-purge-{tag}"));
        let project = serde_json::from_str(
            r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
        )
        .unwrap();
        let mut session =
            ew_api::Session::from_records(project, [], [], ew_api::SessionOptions::default()).unwrap();
        session.create(&dir, store()).unwrap();
        for logo in [
            &br#"<svg viewBox="0 0 8 2"><rect width="8" height="2"/></svg>"#[..],
            &br#"<svg viewBox="0 0 3 3"><circle r="1"/></svg>"#[..],
        ] {
            let cmd = ew_api::AnyCommand {
                name: "project.company.set".into(),
                params: serde_json::json!({ "logo": ew_api::base64_encode(logo) }),
            };
            let p = session.preview(&cmd).unwrap();
            session.commit(p).unwrap();
            session.save().unwrap();
        }
        session.close();
        dir
    }

    fn store() -> ew_api::StoreOptions {
        let mut o = ew_api::StoreOptions::new("test");
        o.state_dir = None;
        o
    }

    fn assets(dir: &Path) -> usize {
        std::fs::read_dir(dir.join("assets")).unwrap().count()
    }

    /// Runs the subcommand on `dir` with `terminal` and the typed `answer`; the exit code and what it printed.
    fn purge(dir: &Path, preview: bool, terminal: bool, answer: &str) -> (i32, String) {
        let args = AssetsPurgeArgs {
            project: Some(dir.to_path_buf()),
            preview,
        };
        let mut input = std::io::Cursor::new(answer.as_bytes().to_vec());
        let mut output = Vec::new();
        let code = run_assets_purge_with(&args, terminal, &mut input, &mut output, store());
        (code, String::from_utf8(output).unwrap())
    }

    /// Owner ruling (2026-10-23): the purge is human-only. Each path: the preview (no terminal needed, nothing
    /// changes), off a terminal (exit 2, nothing deleted), a wrong word (exit 1, nothing deleted), the typed word
    /// (exit 0, the listed file deleted after the list was shown).
    #[test]
    fn the_purge_needs_the_typed_word_on_a_terminal() {
        let dir = project_with_an_unnamed_asset("typed");
        assert_eq!(assets(&dir), 2);

        let (code, shown) = purge(&dir, true, false, "");
        assert_eq!(code, 0);
        assert!(
            shown.starts_with("remove assets/") && shown.lines().count() == 1,
            "{shown}"
        );
        assert_eq!(assets(&dir), 2, "the preview changes nothing");
        // The preview is a reader: it runs while an editor holds the project's lock, and leaves the lock alone.
        let (editor, _) = ew_api::Session::open(
            &dir,
            store(),
            ew_api::SessionOptions::default(),
            ew_api::Recovery::Keep,
        )
        .unwrap();
        let lock = std::fs::read(dir.join(".~ew.lock")).unwrap();
        let (code, shown) = purge(&dir, true, false, "");
        assert_eq!(code, 0, "{shown}");
        assert!(shown.starts_with("remove assets/"), "{shown}");
        assert_eq!(
            std::fs::read(dir.join(".~ew.lock")).unwrap(),
            lock,
            "the editor's lock is untouched"
        );
        editor.close();

        let (code, shown) = purge(&dir, false, false, "purge\n");
        assert_eq!(code, 2, "off a terminal");
        assert!(shown.is_empty(), "{shown}");
        assert_eq!(assets(&dir), 2);

        for wrong in ["", "\n", "y\n", "yes\n", "Purge\n", "purge it\n"] {
            let (code, shown) = purge(&dir, false, true, wrong);
            assert_eq!(code, 1, "{wrong:?} must abort");
            assert!(
                shown.contains("remove assets/"),
                "the list comes before the prompt: {shown}"
            );
            assert!(
                shown.contains("Type `purge` to delete the 1 file(s) above"),
                "{shown}"
            );
            assert_eq!(assets(&dir), 2, "{wrong:?}: nothing is deleted");
        }

        let (code, shown) = purge(&dir, false, true, "  purge \r\n");
        assert_eq!(code, 0, "{shown}");
        assert!(shown.contains("removed 1 asset file(s)"), "{shown}");
        assert_eq!(assets(&dir), 1);
        // Nothing left: no prompt.
        let (code, shown) = purge(&dir, false, true, "");
        assert_eq!(code, 0);
        assert!(!shown.contains("Type `purge`"), "{shown}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod revision_tests {
    //! `ew revision approve|release|withdraw` (WP-S1.18 part 3, D10) with the injected terminal: the runs work on a
    //! copy of Golden A in a fresh directory; nothing under `golden/` is opened for writing.
    use super::*;

    fn parse(a: &[&str]) -> Result<Command, String> {
        super::parse(a.iter().map(|s| (*s).to_owned())).map_err(|e| e.render().to_string())
    }

    #[test]
    fn revision_subcommands_parse_their_label_role_reason_and_directory() {
        assert_eq!(
            parse(&["revision", "approve", "--as", "checker"]),
            Ok(Command::Revision(RevisionArgs {
                action: RevisionAction::Approve {
                    role: "checker".into()
                },
                project: None,
            }))
        );
        assert_eq!(
            parse(&["revision", "release", "V2.0", "p.ew"]),
            Ok(Command::Revision(RevisionArgs {
                action: RevisionAction::Release {
                    label: "V2.0".into(),
                    archive_dir: None
                },
                project: Some(PathBuf::from("p.ew")),
            }))
        );
        assert_eq!(
            parse(&["revision", "release", "V2.0", "--archive-dir", "out/archives"]),
            Ok(Command::Revision(RevisionArgs {
                action: RevisionAction::Release {
                    label: "V2.0".into(),
                    archive_dir: Some(PathBuf::from("out/archives"))
                },
                project: None,
            }))
        );
        assert_eq!(
            parse(&["revision", "withdraw", "V1.0", "--reason", "wrong terminals"]),
            Ok(Command::Revision(RevisionArgs {
                action: RevisionAction::Withdraw {
                    label: "V1.0".into(),
                    reason: "wrong terminals".into()
                },
                project: None,
            }))
        );
        for bad in [
            &["revision"][..],
            &["revision", "sign"],
            &["revision", "approve"],
            &["revision", "approve", "--as"],
            &["revision", "approve", "--as", "checker", "--yes"],
            &["revision", "release"],
            &["revision", "release", "V2.0", "a", "b"],
            &["revision", "release", "V2.0", "--as", "checker"],
            &["revision", "release", "V2.0", "--archive-dir"],
            &["revision", "approve", "--as", "checker", "--archive-dir", "a"],
            &["revision", "withdraw", "V1.0"],
            &["revision", "withdraw", "--reason", "r"],
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }

    fn golden() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew")
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            let target = to.join(e.file_name());
            if e.path().is_dir() {
                copy_dir(&e.path(), &target);
            } else {
                std::fs::copy(e.path(), &target).unwrap();
            }
        }
    }

    /// A copy of Golden A in a fresh directory.
    fn golden_copy(tag: &str) -> PathBuf {
        let dir = ew_testdir::fresh_dir(&format!("ew-cli-revision-{tag}"));
        copy_dir(&golden(), &dir);
        dir
    }

    fn store() -> ew_api::StoreOptions {
        let mut o = ew_api::StoreOptions::new("test");
        o.state_dir = None;
        o
    }

    fn date() -> ew_api::catalog::Date {
        ew_api::catalog::Date::new(2025, 6, 2).unwrap()
    }

    /// The bytes of every file under `dir`, by relative path: "nothing was written" compares two of these.
    fn files(dir: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
        fn walk(root: &Path, dir: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(root, &p, out);
                } else {
                    out.insert(
                        p.strip_prefix(root).unwrap().to_path_buf(),
                        std::fs::read(&p).unwrap(),
                    );
                }
            }
        }
        let mut out = std::collections::BTreeMap::new();
        walk(dir, dir, &mut out);
        out
    }

    /// The git port of the tests: the copies in the temporary directory lie in no worktree, and no test looks.
    struct NoGit;

    impl ew_api::git::Git for NoGit {
        fn worktree(&self, _: &dyn ew_api::Vfs, _: &Path) -> std::io::Result<Option<PathBuf>> {
            Ok(None)
        }
        fn available(&self) -> Result<(), ew_api::git::GitError> {
            Err(ew_api::git::GitError::Missing)
        }
        fn commit_dir(&self, _: &Path, _: &str) -> Result<ew_api::git::Committed, ew_api::git::GitError> {
            Err(ew_api::git::GitError::Missing)
        }
        fn tag_message(&self, _: &Path, _: &str) -> Result<Option<String>, ew_api::git::GitError> {
            Err(ew_api::git::GitError::Missing)
        }
        fn tag_exists(&self, _: &Path, _: &str) -> Result<bool, ew_api::git::GitError> {
            Err(ew_api::git::GitError::Missing)
        }
        fn tag(&self, _: &Path, _: &str, _: &str) -> Result<(), ew_api::git::GitError> {
            Err(ew_api::git::GitError::Missing)
        }
        fn delete_tag(&self, _: &Path, _: &str) -> Result<(), ew_api::git::GitError> {
            Err(ew_api::git::GitError::Missing)
        }
    }

    /// Runs `ew revision <action> <dir>` with `terminal` and the typed `answer`; the exit code and what it printed.
    fn run(dir: &Path, action: RevisionAction, terminal: bool, answer: &str) -> (i32, String) {
        run_in(dir, action, terminal, answer, None)
    }

    /// [`run`] with the state directory `state` (the release's run records, FT-814).
    fn run_in(
        dir: &Path,
        action: RevisionAction,
        terminal: bool,
        answer: &str,
        state: Option<&Path>,
    ) -> (i32, String) {
        let args = RevisionArgs {
            action,
            project: Some(dir.to_path_buf()),
        };
        let mut input = std::io::Cursor::new(answer.as_bytes().to_vec());
        let mut output = Vec::new();
        let mut store = store();
        store.state_dir = state.map(Path::to_path_buf);
        let code = run_revision_with(&args, terminal, &mut input, &mut output, store, date(), &NoGit);
        (code, String::from_utf8(output).unwrap())
    }

    fn approve(role: &str) -> RevisionAction {
        RevisionAction::Approve { role: role.into() }
    }

    fn release(label: &str) -> RevisionAction {
        RevisionAction::Release {
            label: label.into(),
            archive_dir: None,
        }
    }

    fn release_into(label: &str, root: &Path) -> RevisionAction {
        RevisionAction::Release {
            label: label.into(),
            archive_dir: Some(root.to_path_buf()),
        }
    }

    fn withdraw(label: &str) -> RevisionAction {
        RevisionAction::Withdraw {
            label: label.into(),
            reason: "the terminal plan was wrong".into(),
        }
    }

    /// Waives every error of the release gate besides EW-REL-001 from the GUI and saves.
    fn waive_the_gate(dir: &Path) {
        let (mut s, report) = ew_api::Session::open(
            dir,
            store(),
            ew_api::SessionOptions::default(),
            ew_api::Recovery::Keep,
        )
        .unwrap();
        let items = ew_api::integrity_items(&report);
        let gate = s.release_gate("V2.0", &items).unwrap();
        let waive: Vec<(String, String)> = gate
            .approval_blockers()
            .iter()
            .map(|f| (f.rule.to_owned(), f.fingerprint.to_string()))
            .collect();
        for (rule, fingerprint) in waive {
            let cmd = ew_api::AnyCommand {
                name: "check.waive".into(),
                params: serde_json::json!({"rule": rule, "fingerprint": fingerprint,
                    "reason": "accepted for the test release", "author": "engineer", "date": "2025-06-01"}),
            };
            s.execute_with(&cmd, ew_api::Submit::from(ew_api::Origin::Gui))
                .unwrap();
        }
        s.save().unwrap();
        s.close();
    }

    /// Without a terminal each subcommand exits 2 and writes nothing; a role that is no party role exits 2.
    #[test]
    fn revision_commands_need_a_terminal_and_a_party_role() {
        let dir = golden_copy("terminal");
        let before = files(&dir);
        for action in [approve("checker"), release("V2.0"), withdraw("V1.0")] {
            let (code, shown) = run(&dir, action.clone(), false, "V2.0\nV1.0\n");
            assert_eq!(code, 2, "{action:?}");
            assert_eq!(shown, "", "{action:?}: no prompt");
        }
        let (code, shown) = run(&dir, approve("boss"), true, "V2.0\n");
        assert_eq!((code, shown.as_str()), (2, ""));
        assert_eq!(files(&dir), before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// RG-4/D15 through the CLI: the release of Golden A is refused with the gate's blockers listed (the release
    /// scheme's errors and EW-REL-001's checker), and the approval is refused while those errors are open.
    #[test]
    fn the_gate_refuses_release_and_approval_of_golden_a_listing_the_blockers() {
        let dir = golden_copy("refused");
        let before = files(&dir);
        let (code, shown) = run(&dir, release("V2.0"), true, "");
        assert_eq!(code, 1, "{shown}");
        assert!(
            shown
                .lines()
                .any(|l| l.starts_with("error EW-REL-001 ") && l.contains("checker not set")),
            "{shown}"
        );
        assert!(shown.lines().any(|l| l.starts_with("error EW-POT-")), "{shown}");
        assert!(shown.lines().all(|l| l.starts_with("error ")), "{shown}");
        let (code, listed) = run(&dir, approve("checker"), true, "V2.0\n");
        assert_eq!(code, 1, "{listed}");
        assert!(!listed.contains("EW-REL-001"), "{listed}");
        assert!(!listed.contains("Type the revision label"), "no prompt: {listed}");
        assert_eq!(listed.lines().count() + 1, shown.lines().count());
        // A label that is not the open revision's.
        let (code, _) = run(&dir, release("V1.0"), true, "");
        assert_eq!(code, 1);
        assert_eq!(files(&dir), before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// RG-1/RG-3 and RA-1 through the CLI: the typed label approves (the safety waivers listed first), the author's
    /// role and a wrong label do not; the release shows its plan and needs the typed label, then writes the archive
    /// (with Golden A's report blocks as CSV) and stamps the revision; a released revision is withdrawn on its typed
    /// label.
    #[test]
    fn approve_release_and_withdraw_on_the_typed_label() {
        let dir = golden_copy("approve");
        let root = dir.with_extension("archives");
        let _ = std::fs::remove_dir_all(&root);
        waive_the_gate(&dir);
        let before = files(&dir);
        for wrong in ["", "\n", "v2.0\n", "V2\n", "yes\n", "V1.0\n"] {
            let (code, shown) = run(&dir, approve("checker"), true, wrong);
            assert_eq!(code, 1, "{wrong:?}");
            assert!(
                shown.contains("Type the revision label `V2.0` to approve it and acknowledge the"),
                "{shown}"
            );
            assert!(shown.contains("  safety waiver "), "{shown}");
        }
        // The author's party may not check (Golden A's V2.0 is authored by the engineer).
        let (code, _) = run(&dir, approve("engineer"), true, "V2.0\n");
        assert_eq!(code, 1);
        assert_eq!(files(&dir), before);

        let (code, shown) = run(&dir, approve("checker"), true, " V2.0 \r\n");
        assert_eq!(code, 0, "{shown}");
        assert!(
            shown.contains("approved revision V2.0 as checker; saved"),
            "{shown}"
        );
        let revisions = std::fs::read_to_string(dir.join("revisions.jsonl")).unwrap();
        assert!(revisions.contains("\"sign_off\""), "{revisions}");
        assert!(revisions.contains("\"checker\":\"checker\""), "{revisions}");

        // The gate passes now: the plan and the prompt; a wrong label writes nothing.
        let approved = files(&dir);
        let (code, shown) = run(&dir, release_into("V2.0", &root), true, "V1.0\n");
        assert_eq!(code, 1, "{shown}");
        assert!(shown.starts_with("release revision V2.0: generation "), "{shown}");
        assert!(shown.contains("  archive: "), "{shown}");
        assert!(!shown.contains("  git: "), "outside a worktree: {shown}");
        assert!(
            shown.contains("Type the revision label `V2.0` to release it"),
            "{shown}"
        );
        assert_eq!(files(&dir), approved);
        assert!(!root.exists());
        // An archive root inside the project is refused.
        let (code, _) = run(&dir, release_into("V2.0", &dir.join("archives")), true, "V2.0\n");
        assert_eq!(code, 1);
        assert_eq!(files(&dir), approved);

        let (code, shown) = run(&dir, release_into("V2.0", &root), true, "V2.0\n");
        assert_eq!(code, 0, "{shown}");
        assert!(shown.contains("released revision V2.0: archive "), "{shown}");
        let archive = std::fs::read(root.join("V2.0.ewarchive")).expect("the archive");
        let limits = ew_api::zip::ZipLimits {
            max_ratio: u64::MAX,
            ..ew_api::zip::ZipLimits::default()
        };
        let entries = ew_api::archive::read(&archive, &limits).expect("the archive verifies");
        assert!(
            entries
                .keys()
                .any(|k| k.starts_with("reports/") && k.ends_with(".csv")),
            "{:?}",
            entries.keys()
        );
        assert!(
            entries.keys().any(|k| k.starts_with("pdf/")),
            "{:?}",
            entries.keys()
        );
        let revisions = std::fs::read_to_string(dir.join("revisions.jsonl")).unwrap();
        assert!(revisions.contains("\"V2.0.ewarchive\""), "{revisions}");
        // Released once: a second release is refused.
        let (code, _) = run(&dir, release_into("V2.0", &root), true, "V2.0\n");
        assert_eq!(code, 1);
        let released = files(&dir);

        // Withdraw: a wrong label, then the typed label.
        let (code, shown) = run(&dir, withdraw("V1.0"), true, "V2.0\n");
        assert_eq!(code, 1);
        assert!(
            shown.contains("Type the revision label `V1.0` to withdraw it"),
            "{shown}"
        );
        assert_eq!(files(&dir), released);
        let (code, shown) = run(&dir, withdraw("V1.0"), true, "V1.0\n");
        assert_eq!(code, 0, "{shown}");
        let revisions = std::fs::read_to_string(dir.join("revisions.jsonl")).unwrap();
        assert!(revisions.contains("the terminal plan was wrong"), "{revisions}");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// RA-3 through the CLI: an archive of the open revision's label that no record names (what a crashed release
    /// leaves: here the archive of an identical copy) is offered before a new release; `complete` stamps the revision
    /// with it, and a damaged one can only be discarded. FT-814: a copy of the project finds the other copy's archive
    /// in the shared archive folder, but `discard` is not offered without this folder's run record, and typed it
    /// removes nothing; with the record it is offered and removes the archive and the record.
    #[test]
    fn an_incomplete_release_is_completed_or_discarded() {
        let a = golden_copy("incomplete-a");
        waive_the_gate(&a);
        let (code, shown) = run(&a, approve("checker"), true, "V2.0\n");
        assert_eq!(code, 0, "{shown}");
        let b = ew_testdir::fresh_dir("ew-cli-revision-incomplete-b");
        copy_dir(&a, &b);
        let root_a = a.with_extension("archives");
        let root_b = b.with_extension("archives");
        let state = a.with_extension("state");
        for r in [&root_a, &root_b, &state] {
            let _ = std::fs::remove_dir_all(r);
        }
        let (code, shown) = run_in(&a, release_into("V2.0", &root_a), true, "V2.0\n", Some(&state));
        assert_eq!(code, 0, "{shown}");

        // B, a copy of A, on A's archive folder with the same state directory: `discard` is not offered, and typed it
        // removes nothing (FT-814).
        let archived = std::fs::read(root_a.join("V2.0.ewarchive")).unwrap();
        let before_b = files(&b);
        let (code, shown) = run_in(&b, release_into("V2.0", &root_a), true, "discard\n", Some(&state));
        assert_eq!(code, 1, "{shown}");
        assert!(
            shown.contains(
                "  it is not offered for discarding: the archive V2.0.ewarchive in that folder is not provably a \
                 leftover of this project folder's own release (no release from this folder recorded writing it, and a \
                 copy of the folder names the same project); remove the file by hand if you know it is a leftover\n"
            ),
            "{shown}"
        );
        assert!(shown.contains("Type `complete` to stamp"), "{shown}");
        assert!(!shown.contains("`discard`"), "{shown}");
        assert_eq!(std::fs::read(root_a.join("V2.0.ewarchive")).unwrap(), archived);
        assert_eq!(files(&b), before_b);

        std::fs::create_dir_all(&root_b).unwrap();
        std::fs::copy(root_a.join("V2.0.ewarchive"), root_b.join("V2.0.ewarchive")).unwrap();

        // Aborted: nothing changes.
        let before = files(&b);
        let (code, shown) = run(&b, release_into("V2.0", &root_b), true, "V2.0\n");
        assert_eq!(code, 1, "{shown}");
        assert!(
            shown.starts_with("incomplete release V2.0: the archive "),
            "{shown}"
        );
        assert!(shown.contains("Type `complete` to stamp"), "{shown}");
        assert_eq!(files(&b), before);
        // With a run record of B's folder for the archive (what B's own crashed run leaves), `discard` is offered and
        // removes the archive and the record.
        let record = run_record_of(&b, &root_b, &state);
        let (code, shown) = run_in(&b, release_into("V2.0", &root_b), true, "discard\n", Some(&state));
        assert_eq!(code, 0, "{shown}");
        assert!(shown.contains(", `discard` to remove it"), "{shown}");
        assert!(
            shown.contains("discarded the incomplete release V2.0: removed "),
            "{shown}"
        );
        assert!(!root_b.join("V2.0.ewarchive").exists());
        assert!(!record.exists(), "the run's record is removed");
        assert_eq!(files(&b), before_b);
        std::fs::copy(root_a.join("V2.0.ewarchive"), root_b.join("V2.0.ewarchive")).unwrap();

        // Completed with the archive's own facts.
        let (code, shown) = run(&b, release_into("V2.0", &root_b), true, "complete\n");
        assert_eq!(code, 0, "{shown}");
        assert!(shown.contains("released revision V2.0: archive "), "{shown}");
        assert_eq!(
            std::fs::read(b.join("revisions.jsonl")).unwrap(),
            std::fs::read(a.join("revisions.jsonl")).unwrap(),
            "the same release record as the uninterrupted release"
        );

        // A damaged archive names no project: neither completed nor discarded, but kept, and nothing is offered.
        let c = ew_testdir::fresh_dir("ew-cli-revision-incomplete-c");
        copy_dir(&golden(), &c);
        let root_c = c.with_extension("archives");
        let _ = std::fs::remove_dir_all(&root_c);
        std::fs::create_dir_all(&root_c).unwrap();
        std::fs::write(root_c.join("V2.0.ewarchive"), b"not an archive").unwrap();
        for answer in ["complete\n", "discard\n"] {
            let (code, shown) = run(&c, release_into("V2.0", &root_c), true, answer);
            assert_eq!(code, 1, "{shown}");
            assert!(!shown.contains("Type `discard`"), "{shown}");
            assert_eq!(
                std::fs::read(root_c.join("V2.0.ewarchive")).unwrap(),
                b"not an archive",
                "{answer}: the damaged archive is kept"
            );
        }
        for d in [&a, &b, &c, &root_a, &root_b, &root_c, &state] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    /// Writes the run record a release of the project in `dir` leaves in `state` once it has written the archive of the
    /// open revision in `root` (FT-814), as if that run had crashed right after; returns the record's file.
    fn run_record_of(dir: &Path, root: &Path, state: &Path) -> PathBuf {
        let mut options = store();
        options.state_dir = Some(state.to_path_buf());
        let (session, _) = ew_api::Session::open(
            dir,
            options,
            ew_api::SessionOptions::default(),
            ew_api::Recovery::Keep,
        )
        .unwrap();
        let found = session.incomplete_release(Some(root)).unwrap().unwrap();
        let store = session.store().unwrap();
        let vfs = &**store.vfs();
        let modified = std::fs::metadata(&found.archive).unwrap().modified().unwrap();
        let record = ew_api::archive::RunRecord {
            project_dir: ew_api::archive::path_text(vfs, store.root()),
            archive: ew_api::archive::path_text(vfs, &found.archive),
            project: session.query().project().id.to_string(),
            revision: found.revision.to_string(),
            label: found.label.clone(),
            generation: found.facts.as_ref().unwrap().1,
            sha256: found.sha256.to_string(),
            modified: ew_api::archive::record_time(modified),
            run: "the-crashed-run".to_owned(),
        };
        let path = ew_api::archive::write_run_record(vfs, state, &record).unwrap();
        session.close();
        path
    }
}

#[cfg(test)]
mod guard_tests {
    //! WP-S6.04 D15, SE-9: `ew sql`'s statement under its panic guard, given a panicking closure, on a copy of
    //! Golden A in a fresh directory; nothing under `golden/` is opened for writing.
    use super::*;

    /// Every file below `dir` with its bytes, by path relative to `base`.
    fn tree(dir: &Path, base: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                tree(&p, base, out);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }

    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap() {
            let p = e.unwrap().path();
            let target = to.join(p.file_name().unwrap());
            if p.is_dir() {
                copy(&p, &target);
            } else {
                std::fs::copy(&p, &target).unwrap();
            }
        }
    }

    /// C51-p1#2: `ew import table`'s refusal lines, each whole: `error: ` and the refusal, its first letter in lower
    /// case (S2 for a read that panicked).
    #[test]
    fn an_import_table_refusal_is_one_lower_case_error_line() {
        use ew_api::output::TableImportError as E;
        assert_eq!(
            import_table_error_line(&E::Panicked("pages.csv".into())),
            "error: the table import stopped on an internal error while reading pages.csv; nothing was changed."
        );
        assert_eq!(
            import_table_error_line(&E::CsvNoTable),
            "error: a CSV names no table: give `--table <key>`"
        );
    }

    /// S1 whole, when the statement panics after it ran on the projection of a copy of Golden A; the copy's files
    /// and the session's model are unchanged; an SQL error keeps its own line.
    #[test]
    fn a_panicking_statement_gives_s1_and_changes_nothing() {
        let root = ew_testdir::fresh_dir("ew-cli-guard");
        let dir = root.join("J-E-2416.ew");
        copy(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew"),
            &dir,
        );
        let files = |d: &Path| {
            let mut out = std::collections::BTreeMap::new();
            tree(d, d, &mut out);
            out
        };
        let before = files(&dir);
        let (projection, session) = open_projection_and_session(Some(&dir)).unwrap();
        let model = session.world().model.clone();
        let r: Result<(), String> = guarded_query(|| {
            let set = projection
                .reader(ew_api::sql::Profile::EW_SQL)
                .and_then(|r| r.query("SELECT count(*) FROM api_v1_terminal"))?;
            assert_eq!(set.rows.len(), 1);
            panic!("a statement that panics")
        });
        assert_eq!(
            r,
            Err("error: the query stopped on an internal error; nothing was changed".to_owned())
        );
        let r = guarded_query(|| {
            projection
                .reader(ew_api::sql::Profile::EW_SQL)
                .and_then(|r| r.query("DELETE FROM api_v1_terminal"))
        });
        assert_eq!(
            r.map(|_| ()),
            Err("error: statement denied by the projection sandbox (DELETE): only SELECT, WITH or VALUES may run"
                .to_owned())
        );
        assert!(session.world().model.content_eq(&model), "the model changed");
        drop((projection, session));
        assert!(files(&dir) == before, "the copy's files changed");
        let _ = std::fs::remove_dir_all(&root);
    }
}
