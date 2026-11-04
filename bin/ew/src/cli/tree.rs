// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `ew` command line as one clap derive tree: every subcommand and flag is declared here once, with its help in
//! its doc comment; [`Cli::into_command`] lowers a parsed line to the [`Command`] that [`super::run`] runs.
//!
//! The grammar (doc 17 §3.1, WP-S1.22 D3): a valued flag takes the next word as its value even when it starts with
//! `-`; a second `--project` is an error, `--company` and `lib search --kind` accumulate, every other flag is
//! last-wins; the positional project directory and `--project` exclude each other; a positional that takes hyphen
//! values refuses a word starting with `--` as a mistyped option. A rule the relations cannot say is one check in the
//! lowering that returns a usage error.

use std::path::PathBuf;

use clap::error::ErrorKind;
use clap::{ArgGroup, Args, CommandFactory, Parser, Subcommand, ValueEnum};
use ew_api::BomMode;
use ew_api::checks::CheckMode;
use ew_api::import::migration::ReportFormat;
use ew_api::output::print::Paper;
use ew_api::output::{CsvOptions, LineEnd, PageSelection, Preset};

use super::collab::GitSetupArgs;
use super::diff::DiffArgs;
use super::merge_driver::MergeDriverArgs;
use super::merge_resolve::MergeResolveArgs;
use super::query::QueryArgs;
use super::{
    ApplyArgs, AssetsPurgeArgs, CheckArgs, CheckFormat, ChecksSetArgs, Command, ExportArgs, ExportEwzArgs,
    ExportKind, ExportPagesArgs, ExportTableArgs, ExtractArgs, FmtArgs, ImportCsvArgs, ImportPartsArgs,
    ImportQetArgs, ImportTableArgs, LibAction, LibArgs, PageFormat, PrintArgs, PrintVia, RecoverAction,
    RecoverArgs, RenderArgs, RevisionAction, RevisionArgs, RowFormat, SqlArgs, SqlDumpArgs, SqlFormat,
    SqlSource, UpgradeArgs, VerifyArgs, XLSX_NEEDS_OUT,
};

/// The ElektraWatt command-line interface.
///
/// Every subcommand that works on one project takes it as the positional directory or as `--project <dir>`, never
/// both, else the nearest ancestor of the current directory holding `manifest.json`. Every subcommand that prints a
/// result takes `--json`: stdout is then one JSON object whose first member `$schema` names its published schema
/// (`ew.<command>-result/1`; `ew schema <name>` prints it), diagnostics stay on stderr and the exit codes are
/// unchanged. For `check` and `sql`, `--json` is `--format json`. The human-only subcommands (`project`,
/// `revision`, `recover --discard`) print a dialogue, not a result, and take no `--json`. A usage error exits 2 (129
/// for `merge-driver`).
#[derive(Parser, Debug)]
#[command(name = "ew", bin_name = "ew", version)]
pub(super) struct Cli {
    /// The subcommand.
    #[command(subcommand)]
    command: Top,
}

// map: cli: a new subcommand or flag goes in this derive tree with its help; its exits go in catalogue::TABLE.
/// The subcommands of `ew`.
#[derive(Subcommand, Debug)]
enum Top {
    /// Import a project, a parts table or golden-dialect tables.
    #[command(subcommand)]
    Import(Import),
    /// Run a check scheme over a project.
    ///
    /// Runs a check scheme (mode `release`, format `text`) over a project directory and exits 0 with no unwaived
    /// errors, 1 with errors, 2 on usage and open errors. Without `--scheme` the scheme is the one the project names
    /// (`project checks set`: `--release` for mode `release`, `--active` for mode `online`), else `pre-delivery`.
    /// `--lang` renders the messages in a shipped language (en-GB, the default, or nl-NL); a message without a text
    /// in that language prints in en-GB.
    Check(CheckCli),
    /// Run one read-only SELECT against the SQL projection of a project.
    ///
    /// Runs one read-only SELECT against the SQL projection of a project (the `api_v1_*` views; `ew sql schema`
    /// prints them) in the sandbox of doc 17 §6.2, with the query given inline or from `--file`. With one positional
    /// it is the query; with two, the project directory and the query; with `--file`, the one positional is the
    /// project directory. An inline query may start with an SQL comment (`-- ...`). The result is CSV (default) or
    /// JSON on stdout, the row count on stderr; with `--out` it goes to a file; `--format xlsx` writes a workbook to
    /// `--out`. `--preset` writes the CSV for Excel (`excel-nl`, `excel-en`) or as RFC 4180 (`rfc4180`); `--bom` and
    /// `--line-end` override the preset. `--timeout` sets a deadline in seconds. `sql` exits 0 on success and 2 on
    /// everything else (usage, open and SQL errors, denials, limits).
    Sql(SqlCli),
    /// Write a report's rows as CSV, the pages as PDF, SVG or PNG, or the project as an .ewz.
    #[command(subcommand)]
    Export(ExportCli),
    /// Unpack an .ewz into a new project folder.
    ///
    /// Reads the `.ewz` whole and checks it before anything is written (its layout, the archive bounds, every file
    /// against the manifest's hash; a hostile name, an entry the manifest does not list or a compression bomb refuses
    /// it), then writes it into `--into <dir>` (default: `<name>.ew` beside the archive), which must not exist: the files
    /// go into the sibling folder `<dir>.tmp-ewz` first, are synced and read back, and that folder is then renamed to
    /// `<dir>`, so `<dir>` never holds a part of a project and a failure leaves neither folder. Exit 0 when extracted, 2
    /// on everything else (usage, an unreadable or refused archive, an existing folder, write errors).
    Extract(ExtractCli),
    /// Print the pages through the OS, or write the print set as a PDF.
    ///
    /// Builds the print set of the pages (`--pages` folio labels and ranges, or `--sections` section numbers, a
    /// section with its divider; not both; default every page) and hands it to the OS: `--via lp` runs the CUPS
    /// command `lp` (the default on Linux; `--printer` names the printer, else the default one), `--via windows`
    /// shows the Windows print dialog, or prints to the printer `--printer` names without it (the default on
    /// Windows), `--via viewer` opens it in the default PDF application, which asks for the printer (the default
    /// elsewhere). `--paper` prints on a paper size (sheet, the default, or A5, A4, A3, A2, A1, A0, Letter, Legal,
    /// Tabloid, turned like each page); `--fit` scales each page to the whole paper, never enlarged, and stamps a
    /// scaled page `scaled <n> %` in its bottom margin; without it a larger page is clipped and named in a notice.
    /// `--mono` prints in black and grey. `--out <file.pdf>` writes the print set instead and prints nothing. The
    /// date is `SOURCE_DATE_EPOCH` when set, else the model's. Exit 0 when printed or written, 2 on everything else (usage, open errors, an
    /// unknown folio or section, a refusal of the adapter, write errors).
    Print(PrintCli),
    /// Render one page as PNG.
    ///
    /// `render --page <folio> --png` renders one page to `--out` (default: `<folio>.png` in the current directory,
    /// never stdout) on the CPU with the bundled fonts at `--dpi` (72 to 600, default 150, the golden PNGs'
    /// resolution). Exit 0 on success, 2 on everything else (usage, open errors, an unknown folio, write errors).
    Render(RenderCli),
    /// Replay a command log on the project and save.
    ///
    /// Replays a command log (`.ewcmd`, doc 17 section 3.5) on the project, each line one transaction, and saves.
    /// With one positional it is the log; with two, the project directory and the log. `--preview` replays it on a
    /// copy in memory, prints the changed records, tags and cross-references, and saves nothing; `--one-step` saves
    /// only when every line commits. A project with unsaved changes of a crashed session is refused (exit 2) until
    /// they are recovered or discarded (`ew recover`). Exit 0 when every line commits, 1 when a prevent or domain
    /// rule refuses a line, 2 on everything else (usage, a malformed log, an unresolvable reference, a refused
    /// command, an open, lock or save error). Without `--one-step` the lines before a failing one are saved.
    #[command(override_usage = "ew apply [OPTIONS] [project-dir] <file.ewcmd>")]
    Apply(ApplyCli),
    /// Rewrite the project's files in their canonical form.
    ///
    /// Rewrites the project's files in their canonical encoding and line order (doc 05 section 3), only the files
    /// that are not, through the save protocol (a crash leaves the old or the new generation, never a torn file). It
    /// changes form only, never content: a file whose canonical form would change a record's content refuses the
    /// whole format (exit 2). Quarantined lines, malformed values and unknown kinds, members and files stay verbatim;
    /// they are listed on stderr. `fmt --check` is a reader (no lock): it lists the files a format would rewrite and
    /// writes nothing. A project with unsaved changes of a crashed session, or one that opens read-only, is refused
    /// (exit 2). Exit 0 when every file is canonical or was rewritten, 1 when `--check` finds files to rewrite, 2 on
    /// usage, open, lock, content and save errors.
    ///
    /// `fmt --repair` discards the lines that are not JSON or not UTF-8 (EW-CORE-005), which make the project open
    /// read-only, and nothing else: it first packs the project's files as found, those lines and any edit outside EW
    /// since the last save included, as an `.ewz` backup beside the project folder
    /// (`<project folder>.before-repair-g<generation>.ewz`; a file already there is reused only when it holds that
    /// backup), then rewrites the files that held the lines in one save, other kept lines verbatim, and opens the
    /// project again. It prints each discarded line (`file:line` and its text, cut at 80 characters) and the backup's
    /// path, with the number of files edited outside EW; with `--json` the result is `ew.fmt-repair-result/1`. Refused
    /// before anything is written (exit 2): an `.ewz`, a crashed session's journal, any other finding that forces
    /// read-only, a project whose files cannot be packed (no manifest, a file that cannot be read or that changed
    /// since the repair opened it), a backup name holding another file, a folder that cannot be written. Exit 0 when
    /// the project opens editable afterwards or there was nothing to repair, 2 otherwise. `--repair` and `--check`
    /// exclude each other.
    Fmt(FmtCli),
    /// Summarise, replay or discard the journal a crashed session left.
    ///
    /// Works on the journals that crashed sessions left (doc 05 section 8.3), one at a time, the latest change
    /// first. Without a flag it prints the journal's summary (its unsaved transactions with their commands, whether
    /// the project was saved since, how many more journals remain) and changes nothing. `--replay` replays its
    /// transactions on the saved project and saves; a replay that meets a change saved since (the same field) is
    /// refused and the journal kept. `--discard` prints the summary and deletes the journal, losing its work, so it
    /// is human-only: type `discard` on a controlling terminal; there is no `--yes` flag, and without a terminal it
    /// exits 2 before the project is opened. `recover` needs the project's lock: a project another session holds, or
    /// one that opens read-only, is refused (exit 2) as `apply` refuses it. Exit 0 when summarised, replayed and
    /// saved, discarded, or there is no journal; 1 when the replay is refused or the discard not confirmed; 2 on
    /// usage, terminal, lock, open and save errors.
    Recover(RecoverCli),
    /// Convert a project of an older format to this version's, after a backup.
    ///
    /// A project written in an older format opens converted in memory; saving it writes this version's format, which
    /// older versions of ElektraWatt cannot open. `upgrade` asks first: type `upgrade` on a controlling terminal, or
    /// pass `--yes`; without either it exits 2 and changes nothing. Once confirmed it packs the project's files as
    /// found, as an `.ewz` backup beside the project folder (`<project folder>.pre-<format>.ewz`, or `-2`, `-3` and so
    /// on when a file there holds something else; one that holds this backup is reused), reads it back, and only
    /// then saves the project in this version's format; it prints the backup's path. `--check` writes nothing and
    /// takes no lock: it says whether an upgrade is needed. `upgrade` needs the project's lock: a project another
    /// session holds, one that opens read-only and a crashed session's journal are refused (exit 2). Exit 0 when
    /// upgraded and saved, or nothing is to be upgraded; 1 when `--check` finds an upgrade needed, or it is not
    /// confirmed; 2 on usage, terminal, lock, open, backup and save errors.
    Upgrade(UpgradeCli),
    /// The project's check schemes and asset files (human-only).
    #[command(subcommand)]
    Project(ProjectCli),
    /// The release workflow's human-only commands.
    ///
    /// Each asks the person to type the revision's label on a controlling terminal (doc 15 section 5.4, doc 17
    /// section 2.3); there is no `--yes` flag, and without a terminal it exits 2 before the project is opened. Exit 0
    /// when approved, released, settled or withdrawn and saved, 1 when refused (the gate's blockers, a wrong label, a
    /// refused command or git step) or not confirmed, 2 on usage, terminal and open errors and when writing the
    /// release fails.
    #[command(subcommand)]
    Revision(RevisionCli),
    /// Compare every memoised derived value with a cold recomputation.
    ///
    /// Opens a project with the memo oracle on, recomputes every derived value cold and compares it with the
    /// memoised one: exit 0 with no difference, 1 with differences, 2 on usage and open errors.
    VerifyDerived(ProjectJsonCli),
    /// Answer one public query of the project.
    ///
    /// `query <name> <key>` answers one public query of the project (`ew queries` lists them with their keys): the
    /// key is a reference as in a command log (`tag:-101F1`, `id:<uuid>`, `folio:102`, a bare tag such as
    /// `-X1031:5`), or `-` for a query without one. With three positionals the first is the project directory.
    /// `--explain` adds the memo's policy (verified and changed at which revision, and whether the value was memoised
    /// before the query ran), what the value depends on in words, and, for a value the property grid shows, the
    /// provenance of its row. Exit 0 when answered, 2 on usage, an unknown query, open errors and a key that names no
    /// record or a record of another kind.
    #[command(override_usage = "ew query [OPTIONS] [project-dir] <name> <key>")]
    Query(QueryCli),
    /// List the public queries with their keys.
    ///
    /// Lists the public queries with their keys (`ew query` answers one); exits 0.
    Queries(JsonCli),
    /// List, search, reindex, create and import into libraries.
    ///
    /// Works on the libraries of the four tiers: system (the compiled-in starter, and `<content>/libraries` when
    /// `EW_CONTENT_DIR` names the starter content), community (`<data>/packs`), company (every `--company <dir>`: a
    /// library directory or a directory of `*.ewlib` libraries) and user (`<data>/libraries`), with the search index
    /// in the cache directory (`EW_DATA_DIR` and `EW_CACHE_DIR` override the OS directories). `--json` prints JSON.
    /// Exit 0 on success, 2 on everything else (usage, unreadable tiers, a refused write or import, an index error).
    Lib(LibCli),
    /// List the command registry.
    ///
    /// Lists the command registry (name, parameters version, exposure, cost, confirmation, undoable, stability,
    /// aliases); exits 0.
    Commands(JsonCli),
    /// Print a command's parameter schema or a result schema.
    ///
    /// Prints the JSON Schema of a command's parameters (a name or alias of `ew commands`) or of a result
    /// (`ew.<command>-result/<n>`). Exit 0, 2 for an unknown name.
    Schema(SchemaCli),
    /// The conflict records a merge left.
    #[command(subcommand)]
    Merge(MergeCli),
    /// Merge one project file three-way: git's merge driver (plumbing).
    ///
    /// Git runs it for a file changed on both sides, with `%O` (the ancestor), `%A` (ours, where the result goes),
    /// `%B` (theirs), `%L` (the marker size, unused: conflicts are written as records, not markers) and `%P` (the
    /// file's path in the repository); the older form `%O %A %B %P` gives the same result. `%P` is taken relative to
    /// the deepest folder above it that holds a `manifest.json` in the current directory, the working tree, and
    /// selects the merge: a record file per record, a library document, `manifest.json`; any other file keeps ours.
    /// The three files are read under the reader's limits, and `%A` is replaced whole through a temporary file beside
    /// it. Git takes a driver only from its own configuration, so it is registered by two lines,
    /// `git config merge.elektrawatt.name "ElektraWatt record merge"` and
    /// `git config merge.elektrawatt.driver "ew merge-driver %O %A %B %L %P"`, and selected by three lines of the
    /// project's `.gitattributes`, `*.jsonl text eol=lf merge=elektrawatt diff=elektrawatt`,
    /// `lib/**/*.json text eol=lf merge=elektrawatt diff=elektrawatt` and `manifest.json text eol=lf merge=elektrawatt`
    /// (`eol=lf` keeps the manifest's hashes true under `core.autocrlf`). Exit 0 when merged cleanly, 1 when conflict
    /// records were written or a side or the merged file is larger than the reader's limits (`%A` then as it was),
    /// 129 on an internal error (a file that cannot be read or written, a failure in the merge) or a usage error, with
    /// `%A` as it was: git reads every code up to 128 as conflicts written.
    #[command(override_usage = "ew merge-driver <O> <A> <B> [L] <P>")]
    MergeDriver(MergeDriverCli),
    /// Print the records that differ between two versions of a project.
    ///
    /// Opens both sides read-only and prints one line per record that differs, labelled with the tags of its own side
    /// (`-101F1: parts … → …`). A side is a project directory, an `.ewz`, an `.ewarchive`, `git:<rev>` (the project's
    /// files at that git revision) or `rev:<label>` (the release tag `ew/rev/<label>`); the project of a git side is
    /// the other side when that is a directory, else the project found from the current directory. `--json` prints
    /// `ew.diff-result/1`. `--textconv <file>` prints one project file as one line per record (`<kind> <label>
    /// <path>=<value> …`), labelled from the project in the current directory: git's textconv, which `ew git setup`
    /// configures. Exit 0 when the sides are equal, or differ without `--exit-code`; 1 when they differ with
    /// `--exit-code`; 2 on usage and open errors.
    #[command(override_usage = "ew diff [OPTIONS] <from> <to>\n       ew diff --textconv <file>")]
    Diff(DiffCli),
    /// Git integration.
    #[command(subcommand)]
    Git(GitCli),
}

/// `ew diff`.
#[derive(Args, Debug)]
struct DiffCli {
    /// The first side: a directory, an `.ewz`, an `.ewarchive`, `git:<rev>` or `rev:<label>`.
    #[arg(
        value_name = "from",
        required_unless_present = "textconv",
        conflicts_with = "textconv"
    )]
    from: Option<String>,
    /// The second side.
    #[arg(
        value_name = "to",
        required_unless_present = "textconv",
        conflicts_with = "textconv"
    )]
    to: Option<String>,
    /// Print one project file as text lines (git's textconv) instead.
    #[arg(long, value_name = "file", allow_hyphen_values = true)]
    textconv: Option<PathBuf>,
    /// Exit 1 when the sides differ.
    #[arg(long, overrides_with = "exit_code", conflicts_with = "textconv")]
    exit_code: bool,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json", conflicts_with = "textconv")]
    json: bool,
}

/// `ew git`.
#[derive(Subcommand, Debug)]
enum GitCli {
    /// Configure git's merge driver and textconv for ElektraWatt projects.
    ///
    /// Writes four keys into the repository's git configuration (`--global`: the user's) after your consent, a y/N
    /// question on a terminal or `--yes`: `merge.elektrawatt.name`, `merge.elektrawatt.driver` (this `ew`'s
    /// `merge-driver %O %A %B %L %P`), `diff.elektrawatt.textconv` (this `ew`'s `diff --textconv`) and
    /// `diff.elektrawatt.cachetextconv = true`. Git never takes a driver from a repository, so the configuration is
    /// yours to give. It then writes the managed blocks of `.gitattributes` and `.gitignore` into each project folder of
    /// the worktree, keeping the lines outside them. `--hooks` installs a `post-merge` hook that runs
    /// `ew check --integrity --after-merge` on each project folder, and refuses to replace a hook it did not write. Run
    /// it again after adding a project; it changes nothing that is up to date. Exit 0 when configured, 2 on everything
    /// else (no terminal and no `--yes`, a declined question, not in a git worktree, git missing or refusing, a foreign
    /// hook, write errors); nothing is written before the consent.
    Setup(GitSetupCli),
}

/// `ew git setup`.
#[derive(Args, Debug)]
struct GitSetupCli {
    /// Write the user's global git configuration instead of the repository's.
    #[arg(long, overrides_with = "global")]
    global: bool,
    /// Install the `post-merge` hook that runs `ew check --integrity`.
    #[arg(long, overrides_with = "hooks")]
    hooks: bool,
    /// Consent without the question.
    #[arg(long, overrides_with = "yes")]
    yes: bool,
}

/// `ew import`.
#[derive(Subcommand, Debug)]
enum Import {
    /// Import a QElectroTech project.
    ///
    /// Exits 0 when the import completes, even with findings; `--strict` exits 1 on lost connections or error
    /// findings. With `--report` and no `--report-out` stdout is exactly the report; the summary goes to stderr.
    Qet(ImportQetArgs),
    /// Copy the parts of a parts table into the project.
    ///
    /// Copies the parts of a parts table (the golden dialect: `parts.csv`, with `part_cps.csv`,
    /// `part_templates.csv`, `cable_cores.csv` and `part_plc.csv` when they lie beside it) into the project
    /// (`--project`, default: the nearest ancestor of the current directory holding `manifest.json`) as project-local
    /// part copies (`parts.import`) and saves; a part whose key the project has already is skipped. Exit 0 when
    /// imported (counts on stdout), 2 on everything else (usage, a refused table with its line and column, open and
    /// save errors).
    ///
    /// With `--map` it reads any supplier's parts table instead, CSV (UTF-8, comma or semicolon) or XLSX (the first
    /// sheet unless the map names one), through a column map (a TOML file: `sheet` and a `[columns]` table from
    /// `manufacturer`, `part_number`, `type_designation`, `order_number`, `gtin`, `description`,
    /// `description.<lang>`, `kind`, `unit`, `tech_data.<key>` and `free_props.<key>` to the column headers).
    /// Manufacturer names resolve through the manufacturers of the libraries (`--company` as for `lib`); `--as-of`
    /// (YYYY-MM-DD) stamps the parts. Every row is reported: new, unchanged (the project holds it), duplicate,
    /// rejected with the reason; an `A / B` part number becomes two candidates flagged for review. The new parts are
    /// imported with `parts.import`, with copies of the manufacturers they name, and the project is saved.
    Parts(ImportPartsArgs),
    /// Import golden-dialect tables as a new project.
    ///
    /// Imports golden-dialect tables (the `*.csv` files of `<tables-dir>`: `project.csv`, `devices.csv`,
    /// `conductors.csv` and the others of `golden/J-E-2416/tables/`) with the starter content (`--content` or
    /// `EW_CONTENT_DIR`; required, the tables name its symbols, function definitions and forms) as a new project in
    /// `--out`, which must not exist or be an empty directory: the project is written beside it and renamed into
    /// place, so `--out` never holds a part of one. Every finding of the import is printed. Strict is the default: a
    /// row the dialect refuses, such as a foreign colour code (EW-IMP-002), is dropped and then nothing is written;
    /// `--lenient` keeps a foreign colour code with a warning (EW-IMP-008) and writes the project with its findings.
    /// Exit 0 when written, 1 when nothing is written because strict refused error findings or the records do not
    /// make a project, 2 on usage, starter content, an unreadable table (with its line, and its column where the
    /// reader names one), an `--out` that holds something, and write errors.
    Csv(ImportCsvArgs),
    /// Read a round-trip table back and apply its changed cells.
    ///
    /// Reads a table that `ew export table` or `ew export report` wrote (XLSX, or CSV with `--table <key>`), matches its
    /// rows to the project's by the `id` column (an I/O list's also by station, slot and channel) and previews every
    /// changed cell: changed only in the file, applied; changed only in the project since the export, kept; changed in
    /// both, the project's value kept unless `--take-file`; a read-only cell, a row whose ID the project no longer has
    /// and a number a spreadsheet changed are reported. `--apply` commits the applied cells as one `fields.set_many`
    /// (one undo step) and saves; without it nothing changes. `--preset` names a CSV's preset (`rfc4180` keeps a
    /// leading apostrophe). Exit 0 with the preview (and with `--apply` once saved), 2 on usage, open, read and save
    /// errors and a file of another project.
    Table(ImportTableCli),
}

/// `ew import table` (WP-S3.08 D7, D11).
#[derive(Args, Debug)]
struct ImportTableCli {
    /// The round-trip file: an XLSX workbook or a CSV.
    #[arg(value_name = "file", value_parser = path_word)]
    file: PathBuf,
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// The table's key (`nav:strips/<uuid>`, `kind:io_list`): needed for a CSV.
    #[arg(
        long,
        value_name = "key",
        allow_hyphen_values = true,
        overrides_with = "table"
    )]
    table: Option<String>,
    /// The CSV's preset: excel-nl, excel-en or rfc4180 (default: from its separator).
    #[arg(long, value_name = "preset", allow_hyphen_values = true, overrides_with = "preset", value_parser = import_preset)]
    preset: Option<Preset>,
    /// Take the file's value of a cell changed in both.
    #[arg(long, overrides_with = "take_file")]
    take_file: bool,
    /// Commit the applied cells and save.
    #[arg(long, overrides_with = "apply")]
    apply: bool,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// `ew check`.
#[derive(Args, Debug)]
struct CheckCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// The check scheme (default: the project's scheme for the mode, else `pre-delivery`).
    #[arg(long, value_name = "name", allow_hyphen_values = true, overrides_with = "scheme", value_parser = scheme_name)]
    scheme: Option<String>,
    /// The check mode.
    #[arg(
        long,
        value_name = "mode",
        allow_hyphen_values = true,
        overrides_with = "mode",
        default_value = "release"
    )]
    mode: ModeArg,
    /// The output format (default: text).
    #[arg(
        long,
        value_name = "format",
        allow_hyphen_values = true,
        overrides_with = "format"
    )]
    format: Option<CheckFormat>,
    /// Print the result as JSON (`--format json`).
    #[arg(long, overrides_with = "json")]
    json: bool,
    /// The language of the messages (a shipped language).
    #[arg(long, value_name = "tag", allow_hyphen_values = true, overrides_with = "lang", value_parser = language, default_value = ew_api::checks::messages::DEFAULT_LANGUAGE)]
    lang: String,
    /// The output file (default: stdout).
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    out: Option<PathBuf>,
    /// Run only the project-integrity check: the integrity findings of opening the project and the rules EW-CORE-001
    /// to EW-CORE-019, conflict records (EW-CORE-011) and collisions after a merge (EW-CORE-013) among them, in release
    /// mode; exits 1 on any error.
    #[arg(long, overrides_with = "integrity", conflicts_with_all = ["scheme", "mode"])]
    integrity: bool,
    /// With `--integrity`: count the project as just merged, so that collisions of numbers report as EW-CORE-013 (the
    /// post-merge hook of `ew git setup --hooks` passes it).
    #[arg(long, overrides_with = "after_merge", requires = "integrity")]
    after_merge: bool,
}

/// The mode of `ew check --mode`.
#[derive(ValueEnum, Clone, Copy, PartialEq, Eq, Debug)]
enum ModeArg {
    /// The release gate's mode.
    Release,
    /// The online checks' mode.
    Online,
}

/// `ew sql`: the query, or `dump` and `schema`.
#[derive(Args, Debug)]
#[command(
    args_conflicts_with_subcommands = true,
    override_usage = "ew sql [OPTIONS] [project-dir] <select>\n       ew sql [OPTIONS] [project-dir] --file <query.sql>\n       ew sql <COMMAND>"
)]
struct SqlCli {
    /// `dump` or `schema`.
    #[command(subcommand)]
    sub: Option<SqlSub>,
    /// The query, or with a second positional or `--file` the project directory.
    #[arg(value_name = "project-dir|select", allow_hyphen_values = true, value_parser = sql_word, required_unless_present = "file")]
    first: Option<String>,
    /// The query, after the project directory.
    #[arg(value_name = "select", allow_hyphen_values = true, value_parser = sql_word, conflicts_with = "file")]
    second: Option<String>,
    /// The project directory (instead of the positional one).
    #[arg(long, value_name = "dir", allow_hyphen_values = true)]
    project: Option<PathBuf>,
    /// A file holding the query.
    #[arg(
        long,
        value_name = "query.sql",
        allow_hyphen_values = true,
        overrides_with = "file"
    )]
    file: Option<PathBuf>,
    /// The result format: csv (default), json, or xlsx (with `--out`).
    #[arg(long, value_name = "format", allow_hyphen_values = true, overrides_with = "format", value_parser = sql_format)]
    format: Option<SqlFormat>,
    /// The CSV options.
    #[command(flatten)]
    csv: CsvCli,
    /// Print the result as JSON (`--format json`).
    #[arg(long, overrides_with = "json")]
    json: bool,
    /// The deadline in seconds.
    #[arg(long, value_name = "seconds", allow_hyphen_values = true, overrides_with = "timeout", value_parser = timeout)]
    timeout: Option<u64>,
    /// The output file (default: stdout).
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    out: Option<PathBuf>,
}

/// `ew sql dump|schema`.
#[derive(Subcommand, Debug)]
enum SqlSub {
    /// Write the SQL projection as a SQLite database file.
    ///
    /// With one positional it is the output file; with two, the project directory and the output file.
    #[command(override_usage = "ew sql dump [OPTIONS] [project-dir] <out.sqlite>")]
    Dump(SqlDumpCli),
    /// Print the SQL schema (the `api_v1_*` views).
    Schema,
}

/// `ew sql dump`.
#[derive(Args, Debug)]
struct SqlDumpCli {
    /// The output file, or with a second positional the project directory.
    #[arg(value_name = "project-dir|out.sqlite")]
    first: String,
    /// The output file, after the project directory.
    #[arg(value_name = "out.sqlite")]
    second: Option<String>,
    /// The project directory (instead of the positional one).
    #[arg(long, value_name = "dir", allow_hyphen_values = true)]
    project: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// `ew export`.
#[derive(Subcommand, Debug)]
enum ExportCli {
    /// Write the cable list as CSV or XLSX.
    ///
    /// Writes the rows of the project's cable overview (the golden dialect) to stdout or to `--out`. `--block`
    /// names the report block by its ID or its first folio (`908`); without it the block is the project's block of
    /// the kind with the lowest first folio; a block of another kind is refused. `--format xlsx` writes a workbook to
    /// `--out`; `--preset` writes the CSV for Excel (`excel-nl`, `excel-en`) or as RFC 4180 (`rfc4180`), and `--bom`
    /// and `--line-end` override the preset. Exit 0 on success, 2 on everything else (usage, open errors, a project
    /// without the block, an unknown or wrong block).
    Cables(RowsCli),
    /// Write the bill of materials as CSV or XLSX.
    ///
    /// Writes the rows of the project's parts summary (the golden dialect) to stdout or to `--out`. `--mode parts`
    /// (alias `component`, the default) lists every component part; `--mode assembly` replaces the parts a
    /// configured product covers by its row. `--block` names the report block by its ID or its first folio;
    /// without it the block is the project's block of the kind with the lowest first folio. `--format`, `--preset`,
    /// `--bom` and `--line-end` as for `export cables`. Exit 0 on success, 2 on everything else.
    Bom(BomCli),
    /// Write the wire list as CSV or XLSX.
    ///
    /// Writes the rows of the project's wire list (the golden dialect) to stdout or to `--out`. `--block` names the
    /// report block by its ID or its first folio; without it the block is the project's block of the kind with the
    /// lowest first folio. `--format`, `--preset`, `--bom` and `--line-end` as for `export cables`. Exit 0 on success,
    /// 2 on everything else.
    Wires(RowsCli),
    /// Write the terminal diagrams as CSV or XLSX.
    ///
    /// Writes the rows of the project's terminal diagram blocks (the golden dialect) to stdout or to `--out`.
    /// Without `--block`, every terminal diagram block. `--strip <tag>` writes the rows of one strip (`-X1031`);
    /// without it every strip, each strip's rows after a heading row `heading,<tag>` (in XLSX, one sheet per strip
    /// named by its tag). `--format`, `--preset`, `--bom` and `--line-end` as for `export cables`. Exit 0 on success,
    /// 2 on everything else (also an unknown strip).
    Terminals(TerminalsCli),
    /// Write the rows of a report block or a report kind.
    ///
    /// `--block <block>` writes the rows of any block as CSV or XLSX, or with `--format pdf` the block's pages as a
    /// PDF to `--out` (as `export pdf --pages`). `--kind <kind>` writes the whole-project rows of a report kind
    /// (`device_list`, `terminal_overview`, ...) in the kind's default order, as CSV or XLSX, with no block.
    /// `--preset`, `--bom` and `--line-end` as for `export cables`. Exit 0 on success, 2 on everything else.
    Report(ReportCli),
    /// Write a navigator's table as CSV or XLSX for a round trip.
    ///
    /// Writes the table of a navigator (pages, devices, connections, strips, cables, plc, potentials, ips,
    /// structure, parts, blocks, unplaced) unfiltered: its table rows in node order with every column, each row with
    /// its ID. `--strip <tag>` writes one strip's rows of `export table strips`. `--format xlsx` writes a workbook to
    /// `--out` with the hidden round-trip sheets (`_ew`, and `_ew_base` with the editable cells as exported), which
    /// `ew import table` reads back; `--preset`, `--bom` and `--line-end` as for `export cables`. Exit 0 on success, 2
    /// on everything else (usage, open errors, an unknown navigator or strip).
    Table(TableCli),
    /// Write the pages as one PDF/A-2b.
    ///
    /// Writes the project's pages as one PDF/A-2b (outline, page labels, links) through the session's export job
    /// with the bundled fonts only. `--pages` selects folio labels and ranges in page order (`101-106,901`), or
    /// `--sections` section numbers, each section's pages with its divider (`1,2`), not both; `--threads` changes
    /// the speed, never the bytes. The date is `SOURCE_DATE_EPOCH` when set, else the model's. Exit 0 on success, 2
    /// on everything else.
    Pdf(PagesCli),
    /// Write one SVG file per page.
    ///
    /// Writes one `<folio>.svg` per page into the directory `--out` (replaced as a whole; it may hold SVG files
    /// only) through the session's export job with the bundled fonts only. `--pages`, `--sections` and `--threads`
    /// as for `export pdf`. Exit 0 on success, 2 on everything else.
    Svg(PagesCli),
    /// Pack the project's saved generation into an .ewz.
    ///
    /// Packs the project's saved generation into `--out <file.ewz>` (required), a deterministic ZIP (doc 05 §12):
    /// `mimetype` first, `manifest.json`, then every file the manifest lists and `.gitattributes` and `.gitignore` when
    /// present, read back from the disk and checked against the manifest; strays, foreign files, `.ew/`, the lock files,
    /// temps and `.git/` never enter, and the same project gives the same bytes. Every reader (`check`, `sql`, the row
    /// and page exports, `export table`, `print`, `render`, `verify-derived`, `query`, `fmt --check`) takes an `.ewz` as
    /// its project and opens it read-only; every other subcommand refuses one; `ew extract` unpacks one. Exit 0 when
    /// written, 2 on everything else (usage, open errors, a project changed on disk or one that cannot be packed, write
    /// errors).
    Ewz(EwzCli),
    /// Write one PNG file per page.
    ///
    /// Writes one `<folio>.png` per page into the directory `--out` (replaced as a whole; it may hold PNG files
    /// only), rendered on the CPU with the bundled fonts at `--dpi` (72 to 600, default 150, the golden PNGs'
    /// resolution). `--pages` and `--sections` as for `export pdf`. Exit 0 on success, 2 on everything else (usage,
    /// open errors, an unknown folio or section, write errors).
    Png(PngCli),
}

/// `ew export ewz`.
#[derive(Args, Debug)]
struct EwzCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// The .ewz file to write (required).
    #[arg(
        long,
        value_name = "file.ewz",
        allow_hyphen_values = true,
        overrides_with = "out"
    )]
    out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// `ew extract`.
#[derive(Args, Debug)]
struct ExtractCli {
    /// The .ewz file.
    #[arg(value_name = "file.ewz")]
    file: PathBuf,
    /// The new project folder (default: `<name>.ew` beside the archive); it must not exist.
    #[arg(long, value_name = "dir", allow_hyphen_values = true, overrides_with = "into")]
    into: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// `ew export cables|wires`.
#[derive(Args, Debug)]
struct RowsCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// The report block, by its ID or its first folio.
    #[arg(
        long,
        value_name = "block",
        allow_hyphen_values = true,
        overrides_with = "block"
    )]
    block: Option<String>,
    /// The format: csv (default) or xlsx (with `--out`).
    #[arg(long, value_name = "format", allow_hyphen_values = true, overrides_with = "format", value_parser = row_format)]
    format: Option<ReportPages>,
    /// The CSV options.
    #[command(flatten)]
    csv: CsvCli,
    /// The output file (default: stdout).
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// `ew export bom`.
#[derive(Args, Debug)]
struct BomCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// The report block, by its ID or its first folio.
    #[arg(
        long,
        value_name = "block",
        allow_hyphen_values = true,
        overrides_with = "block"
    )]
    block: Option<String>,
    /// Every component part, or the configured products' rows in place of the parts they cover.
    #[arg(
        long,
        value_name = "mode",
        allow_hyphen_values = true,
        overrides_with = "mode",
        default_value = "parts"
    )]
    mode: BomArg,
    /// The format: csv (default) or xlsx (with `--out`).
    #[arg(long, value_name = "format", allow_hyphen_values = true, overrides_with = "format", value_parser = row_format)]
    format: Option<ReportPages>,
    /// The CSV options.
    #[command(flatten)]
    csv: CsvCli,
    /// The output file (default: stdout).
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// The mode of `ew export bom --mode`.
#[derive(ValueEnum, Clone, Copy, PartialEq, Eq, Debug)]
enum BomArg {
    /// Every component part.
    #[value(alias = "component")]
    Parts,
    /// The configured products' rows in place of the parts they cover.
    Assembly,
}

/// `ew export terminals`.
#[derive(Args, Debug)]
struct TerminalsCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// The report block, by its ID or its first folio (default: every terminal diagram block).
    #[arg(
        long,
        value_name = "block",
        allow_hyphen_values = true,
        overrides_with = "block"
    )]
    block: Option<String>,
    /// One strip, by its tag (`-X1031`).
    #[arg(
        long,
        value_name = "tag",
        allow_hyphen_values = true,
        overrides_with = "strip"
    )]
    strip: Option<String>,
    /// The format: csv (default) or xlsx (with `--out`).
    #[arg(long, value_name = "format", allow_hyphen_values = true, overrides_with = "format", value_parser = row_format)]
    format: Option<ReportPages>,
    /// The CSV options.
    #[command(flatten)]
    csv: CsvCli,
    /// The output file (default: stdout).
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// `ew export report`.
#[derive(Args, Debug)]
// The group allows both so that the lowering refuses the pair in WP-S3.06 D4's words.
#[command(group(ArgGroup::new("rows").args(["block", "kind"]).required(true).multiple(true)))]
struct ReportCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// The report block, by its ID or its first folio.
    #[arg(
        long,
        value_name = "block",
        allow_hyphen_values = true,
        overrides_with = "block"
    )]
    block: Option<String>,
    /// A report kind: its whole-project rows (CSV or XLSX, no block).
    #[arg(
        long,
        value_name = "kind",
        allow_hyphen_values = true,
        overrides_with = "kind"
    )]
    kind: Option<String>,
    /// The format: csv (default), xlsx (with `--out`) or pdf (a block's pages, with `--out`).
    #[arg(long, value_name = "format", allow_hyphen_values = true, overrides_with = "format", value_parser = report_format)]
    format: Option<ReportPages>,
    /// The CSV options.
    #[command(flatten)]
    csv: CsvCli,
    /// The output file (default: stdout).
    #[arg(
        long,
        value_name = "file",
        allow_hyphen_values = true,
        overrides_with = "out",
        required_if_eq("format", "pdf")
    )]
    out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// `ew export table` (WP-S3.08 D7, D8).
#[derive(Args, Debug)]
struct TableCli {
    /// The navigator.
    #[arg(value_name = "navigator", value_parser = word)]
    navigator: String,
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// One strip of `export table strips`, by its tag (`-X1031`).
    #[arg(
        long,
        value_name = "tag",
        allow_hyphen_values = true,
        overrides_with = "strip"
    )]
    strip: Option<String>,
    /// The format: csv (default) or xlsx (with `--out`).
    #[arg(long, value_name = "format", allow_hyphen_values = true, overrides_with = "format", value_parser = table_format)]
    format: Option<ReportPages>,
    /// The CSV options.
    #[command(flatten)]
    csv: CsvCli,
    /// The output file (default: stdout).
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// What `ew export report --format` writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ReportPages {
    /// Rows as CSV.
    Csv,
    /// Rows as an XLSX workbook.
    Xlsx,
    /// A block's pages as PDF.
    Pdf,
}

/// The CSV options of a row export and of `sql` (WP-S3.08 D6).
#[derive(Args, Debug)]
struct CsvCli {
    /// The CSV preset: ew (default, the golden dialect), excel-nl, excel-en or rfc4180.
    #[arg(long, value_name = "preset", allow_hyphen_values = true, overrides_with = "preset", value_parser = preset)]
    preset: Option<Preset>,
    /// A UTF-8 byte-order mark: on or off (default: the preset's).
    #[arg(long, value_name = "on|off", allow_hyphen_values = true, overrides_with = "bom", value_parser = bom)]
    bom: Option<bool>,
    /// The line end: crlf or lf (default: the preset's).
    #[arg(long, value_name = "crlf|lf", allow_hyphen_values = true, overrides_with = "line_end", value_parser = line_end)]
    line_end: Option<LineEnd>,
}

impl CsvCli {
    /// The options, or the refusal of a CSV option given with another format (`format`: its name).
    fn options(self, path: &[&str], format: &str) -> Result<CsvOptions, clap::Error> {
        if format != "csv" {
            let given = [
                ("preset", self.preset.is_some()),
                ("bom", self.bom.is_some()),
                ("line-end", self.line_end.is_some()),
            ];
            if let Some((flag, _)) = given.into_iter().find(|(_, g)| *g) {
                return Err(usage_error(
                    path,
                    format!("`--{flag}` applies to CSV only; drop it, or give `--format csv`"),
                ));
            }
        }
        Ok(CsvOptions {
            preset: self.preset.unwrap_or(Preset::Ew),
            bom: self.bom,
            line_end: self.line_end,
        })
    }
}

/// The format of a row export (WP-S3.08 D7): `xlsx` takes no CSV option and needs `--out`; `pdf` takes no CSV option.
fn rows_format(
    path: &[&str],
    format: Option<ReportPages>,
    csv: CsvCli,
    out: Option<&PathBuf>,
) -> Result<RowFormat, clap::Error> {
    match format {
        Some(ReportPages::Xlsx) => {
            csv.options(path, "xlsx")?;
            if out.is_none() {
                return Err(usage_error(path, XLSX_NEEDS_OUT));
            }
            Ok(RowFormat::Xlsx)
        }
        Some(ReportPages::Pdf) => {
            csv.options(path, "pdf")?;
            Ok(RowFormat::default())
        }
        Some(ReportPages::Csv) | None => Ok(RowFormat::Csv(csv.options(path, "csv")?)),
    }
}

/// `ew export pdf|svg`.
#[derive(Args, Debug)]
struct PagesCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// The PDF file, or the directory of the SVG files.
    #[arg(long, value_name = "path", allow_hyphen_values = true, overrides_with = "out")]
    out: PathBuf,
    /// The folio labels and ranges, in page order (`101-106,901`; default: every page).
    #[arg(long, value_name = "folios", allow_hyphen_values = true, overrides_with = "pages", value_parser = pages)]
    pages: Option<PageSelection>,
    /// The section numbers, each section's pages with its divider (`1,2`; instead of `--pages`).
    #[arg(long, value_name = "numbers", allow_hyphen_values = true, overrides_with = "sections", value_parser = sections)]
    sections: Option<PageSelection>,
    /// The threads of the page phase (1 to 256; default: the machine's parallelism).
    #[arg(long, value_name = "n", allow_hyphen_values = true, overrides_with = "threads", value_parser = threads)]
    threads: Option<usize>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
    /// Not built yet: refused.
    #[arg(long, hide = true, num_args = 0..=1, overrides_with = "lang")]
    lang: Option<Option<String>>,
    /// Not built yet: refused.
    #[arg(long, hide = true, overrides_with = "omit_personal_data")]
    omit_personal_data: bool,
}

/// `ew print`.
#[derive(Args, Debug)]
struct PrintCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// The folio labels and ranges, in page order (`101-106,901`; default: every page).
    #[arg(long, value_name = "folios", allow_hyphen_values = true, overrides_with = "pages", value_parser = pages)]
    pages: Option<PageSelection>,
    /// The section numbers, each section's pages with its divider (`1,2`; instead of `--pages`).
    #[arg(long, value_name = "numbers", allow_hyphen_values = true, overrides_with = "sections", value_parser = sections)]
    sections: Option<PageSelection>,
    /// The paper (sheet, A5, A4, A3, A2, A1, A0, Letter, Legal, Tabloid; default: each page's sheet).
    #[arg(long, value_name = "size", allow_hyphen_values = true, overrides_with = "paper", value_parser = paper)]
    paper: Option<Paper>,
    /// Scale each page to the whole paper (never enlarged) and stamp a scaled page.
    #[arg(long, overrides_with = "fit")]
    fit: bool,
    /// Print in black and grey.
    #[arg(long, overrides_with = "mono")]
    mono: bool,
    /// The printer (with `--via lp` or `--via windows`; default: `lp`'s default printer, or the one chosen in the
    /// Windows print dialog).
    #[arg(
        long,
        value_name = "name",
        allow_hyphen_values = true,
        overrides_with = "printer",
        conflicts_with = "out"
    )]
    printer: Option<String>,
    /// How the set reaches the printer (default: `lp` on Linux, `windows` on Windows, else `viewer`).
    #[arg(
        long,
        value_name = "adapter",
        allow_hyphen_values = true,
        overrides_with = "via",
        conflicts_with = "out"
    )]
    via: Option<ViaArg>,
    /// Write the print set to this PDF file instead of printing it.
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// The adapter of `ew print --via`.
#[derive(ValueEnum, Clone, Copy, PartialEq, Eq, Debug)]
enum ViaArg {
    /// The CUPS command `lp`.
    Lp,
    /// The default PDF application.
    Viewer,
    /// The Windows print dialog, or the printer `--printer` names (Windows only).
    Windows,
}

/// `ew export png`.
#[derive(Args, Debug)]
struct PngCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// The directory of the PNG files.
    #[arg(long, value_name = "dir", allow_hyphen_values = true, overrides_with = "out")]
    out: PathBuf,
    /// The folio labels and ranges, in page order (`101-106,901`; default: every page).
    #[arg(long, value_name = "folios", allow_hyphen_values = true, overrides_with = "pages", value_parser = pages)]
    pages: Option<PageSelection>,
    /// The section numbers, each section's pages with its divider (`1,2`; instead of `--pages`).
    #[arg(long, value_name = "numbers", allow_hyphen_values = true, overrides_with = "sections", value_parser = sections)]
    sections: Option<PageSelection>,
    /// The resolution (72 to 600).
    #[arg(long, value_name = "n", allow_hyphen_values = true, overrides_with = "dpi", value_parser = dpi, default_value_t = ew_api::output::DEFAULT_DPI)]
    dpi: u32,
    /// The threads of the page phase (1 to 256; default: the machine's parallelism).
    #[arg(long, value_name = "n", allow_hyphen_values = true, overrides_with = "threads", value_parser = threads)]
    threads: Option<usize>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
    /// Not built yet: refused.
    #[arg(long, hide = true, num_args = 0..=1, overrides_with = "lang")]
    lang: Option<Option<String>>,
    /// Not built yet: refused.
    #[arg(long, hide = true, overrides_with = "omit_personal_data")]
    omit_personal_data: bool,
}

/// `ew render`.
#[derive(Args, Debug)]
struct RenderCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// The page, by its folio label.
    #[arg(
        long,
        value_name = "folio",
        allow_hyphen_values = true,
        overrides_with = "page"
    )]
    page: String,
    /// The page format (PNG is the only one).
    #[arg(long, overrides_with = "png", required = true)]
    png: bool,
    /// The resolution (72 to 600).
    #[arg(long, value_name = "n", allow_hyphen_values = true, overrides_with = "dpi", value_parser = dpi, default_value_t = ew_api::output::DEFAULT_DPI)]
    dpi: u32,
    /// The PNG file (default: `<folio>.png` in the current directory).
    #[arg(
        long,
        value_name = "file.png",
        allow_hyphen_values = true,
        overrides_with = "out"
    )]
    out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// `ew apply`.
#[derive(Args, Debug)]
struct ApplyCli {
    /// The command log, or with a second positional the project directory.
    #[arg(value_name = "project-dir|file.ewcmd")]
    first: PathBuf,
    /// The command log, after the project directory.
    #[arg(value_name = "file.ewcmd")]
    second: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(long, value_name = "dir", allow_hyphen_values = true)]
    project: Option<PathBuf>,
    /// Replay on a copy in memory, print the changes and save nothing.
    #[arg(long, overrides_with = "preview")]
    preview: bool,
    /// Save only when every line commits.
    #[arg(long, overrides_with = "one_step")]
    one_step: bool,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// `ew fmt`.
#[derive(Args, Debug)]
struct FmtCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// List the files a format would rewrite and write nothing.
    #[arg(long, overrides_with = "check")]
    check: bool,
    /// Discard the lines that are not JSON or not UTF-8, after an `.ewz` backup beside the project folder.
    #[arg(long, overrides_with = "repair", conflicts_with = "check")]
    repair: bool,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// `ew upgrade`.
#[derive(Args, Debug)]
struct UpgradeCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// Say whether the project needs an upgrade and write nothing.
    #[arg(long, overrides_with = "check")]
    check: bool,
    /// Confirm the upgrade without typing `upgrade` (the backup keeps the project as it was).
    #[arg(long, overrides_with = "yes", conflicts_with = "check")]
    yes: bool,
}

/// `ew recover`.
#[derive(Args, Debug)]
struct RecoverCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// Replay the journal's transactions on the saved project and save.
    #[arg(long, overrides_with = "replay", conflicts_with = "discard")]
    replay: bool,
    /// Delete the journal after the word `discard` typed on a controlling terminal.
    #[arg(long, overrides_with = "discard")]
    discard: bool,
    /// Print the result as JSON (not with `--discard`, which prints a dialogue).
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// `ew project`.
#[derive(Subcommand, Debug)]
enum ProjectCli {
    /// The project's check schemes.
    #[command(subcommand)]
    Checks(ChecksCli),
    /// The project's asset files.
    #[command(subcommand)]
    Assets(AssetsCli),
}

/// `ew project checks`.
#[derive(Subcommand, Debug)]
enum ChecksCli {
    /// Name the project's check schemes (human-only).
    ///
    /// Names the project's check schemes (`project.checks.set`): `--active` the online scheme, `--release` the
    /// scheme of the release gate and `ew check`; a scheme not given returns to its default (`drafting`,
    /// `pre-delivery`). The check scheme decides check outcomes, so the command is human-only (ADR-0008 amendment 3,
    /// doc 17 section 2.3): it prints the change and asks the person to type the scheme's name (the active one, else
    /// the release one) on a controlling terminal; there is no `--yes` flag. Without a terminal it exits 2
    /// (HumanOnly: needs an interactive terminal) before the project is opened. Exit 0 when written and saved, 1 for
    /// a scheme that does not ship or an answer that is not the name, 2 on usage, terminal, open and save errors.
    Set(ChecksSetCli),
}

/// `ew project checks set`.
#[derive(Args, Debug)]
#[command(group(ArgGroup::new("schemes").args(["active", "release"]).required(true).multiple(true)))]
struct ChecksSetCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// The online scheme.
    #[arg(
        long,
        value_name = "scheme",
        allow_hyphen_values = true,
        overrides_with = "active"
    )]
    active: Option<String>,
    /// The scheme of the release gate and `ew check`.
    #[arg(
        long,
        value_name = "scheme",
        allow_hyphen_values = true,
        overrides_with = "release"
    )]
    release: Option<String>,
}

/// `ew project assets`.
#[derive(Subcommand, Debug)]
enum AssetsCli {
    /// Delete the asset files no project file names (human-only).
    ///
    /// Deletes the project's asset files (`assets/<sha256>.<ext>`) whose hash no project file mentions (kept lines
    /// and unknown members included) through the save protocol (`project.assets.purge`, doc 05 section 13.3). A save
    /// never removes an asset file; this is the explicit clean-up, and deleted files do not come back, so the command
    /// is human-only (doc 17 section 2.3): it prints the files that would go and asks the person to type `purge` on a
    /// controlling terminal; there is no `--yes` flag. Without a terminal it exits 2 before the project is opened.
    /// `--preview` lists the files and changes nothing (no terminal needed). Refused while the project has unsaved
    /// changes of a crashed session, quarantined lines or an incomplete save, and in a read-only session. Exit 0 when
    /// purged (or nothing to purge, or previewed), 1 when the purge is refused or not confirmed, 2 on usage,
    /// terminal, open and save errors.
    Purge(AssetsPurgeCli),
}

/// `ew project assets purge`.
#[derive(Args, Debug)]
struct AssetsPurgeCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// List the files that would go and change nothing.
    #[arg(long, overrides_with = "preview")]
    preview: bool,
}

/// `ew revision`.
#[derive(Subcommand, Debug)]
enum RevisionCli {
    /// Record the checker's sign-off on the open revision.
    ///
    /// `revision approve --as <role>` records the checker's sign-off on the open revision (`revision.approve`): the
    /// checker is a party role (`checker`, `author`, `engineer`, ...), never the author's; the project must be
    /// saved; it runs the release gate on the saved generation (the release scheme in release mode with the EW-REL
    /// rules) and is refused while any error other than EW-REL-001 is open, prints the safety-kind waivers in force
    /// (the typed label acknowledges them), then approves and saves.
    Approve(ApproveCli),
    /// Release the open revision with its as-built archive.
    ///
    /// `revision release <label>` releases the open revision (doc 05 section 14.1): it runs the release gate on the
    /// saved generation and prints its blockers when it refuses; else it shows the archive it writes
    /// (`<label>.ewarchive` in `--archive-dir`, by default `<name>.ew-archive` beside the project directory, never
    /// inside it) and, when the project lies in a git worktree, the commit of the project directory and the tag
    /// `ew/rev/<label>` it makes there (a warning when the archive root lies in the worktree: archives are never
    /// added to git). After the typed label it writes and verifies the archive, commits and tags, then stamps the
    /// revision released and saves; a failure leaves the revision open and removes what the run wrote. An archive of
    /// the label left by a crashed release is offered first: type `complete` to stamp it or `discard` to remove it
    /// (with its tag). `discard` is offered only when your record on this computer shows that a release from this
    /// project directory wrote the archive: a copy of the directory names the same project, so without that record
    /// the archive is kept (remove it by hand if you know it is a leftover).
    Release(ReleaseCli),
    /// Withdraw a released revision.
    ///
    /// `revision withdraw <label> --reason <text>` withdraws a released revision (`revision.withdraw`) and saves.
    Withdraw(WithdrawCli),
}

/// `ew revision approve`.
#[derive(Args, Debug)]
struct ApproveCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// The checker's party role.
    #[arg(
        long = "as",
        value_name = "role",
        allow_hyphen_values = true,
        overrides_with = "role"
    )]
    role: String,
}

/// `ew revision release`.
#[derive(Args, Debug)]
struct ReleaseCli {
    /// The open revision's label.
    #[arg(value_name = "label")]
    label: String,
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// The archive root (default: `<name>.ew-archive` beside the project directory).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        overrides_with = "archive_dir"
    )]
    archive_dir: Option<PathBuf>,
}

/// `ew revision withdraw`.
#[derive(Args, Debug)]
struct WithdrawCli {
    /// The released revision's label.
    #[arg(value_name = "label")]
    label: String,
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// Why the revision is withdrawn.
    #[arg(
        long,
        value_name = "text",
        allow_hyphen_values = true,
        overrides_with = "reason"
    )]
    reason: String,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
}

/// A subcommand that takes a project and `--json` only.
#[derive(Args, Debug)]
struct ProjectJsonCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// `ew query`.
#[derive(Args, Debug)]
struct QueryCli {
    /// The query's name, or with three positionals the project directory.
    #[arg(value_name = "project-dir|name", allow_hyphen_values = true, value_parser = word)]
    first: String,
    /// The key, or with three positionals the query's name.
    #[arg(value_name = "name|key", allow_hyphen_values = true, value_parser = word)]
    second: String,
    /// The key, after the project directory and the name.
    #[arg(value_name = "key", allow_hyphen_values = true, value_parser = word)]
    third: Option<String>,
    /// The project directory (instead of the positional one).
    #[arg(long, value_name = "dir", allow_hyphen_values = true)]
    project: Option<PathBuf>,
    /// Add the memo's policy, the value's dependencies and the row's provenance.
    #[arg(long, overrides_with = "explain")]
    explain: bool,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// A subcommand that takes `--json` only.
#[derive(Args, Debug)]
struct JsonCli {
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// `ew schema`.
#[derive(Args, Debug)]
struct SchemaCli {
    /// A command's name or alias (see `ew commands`), or a result schema's name.
    #[arg(value_name = "name")]
    name: String,
}

/// `ew merge`.
#[derive(Subcommand, Debug)]
enum MergeCli {
    /// Resolve the conflict records a merge left, taking ours or theirs, and save.
    ///
    /// Resolves every conflict record of the project that `--kind` and `--field` select (both optional) in one
    /// transaction of `merge.resolve`, and saves. `--kind` is the kind of the record a conflict is on (`connection`),
    /// or `file` for a file a merge could not read; `--field` selects the record-level conflicts on that member
    /// (`overrides.colour`), or on a member below or above it. `--take ours` keeps the record as the merge left it;
    /// `--take theirs` writes theirs' conflicting members into it (deletes it when theirs deleted it, makes it again
    /// when ours did), and for a file takes the lines theirs holds. A project with unsaved changes of a crashed
    /// session, or one that opens read-only, is refused (exit 2). Exit 0 when resolved and saved, 1 when
    /// `merge.resolve` refuses (theirs does not make a valid record, a file's lines do not read), 2 on usage (no
    /// conflict record selected), open, lock and save errors.
    Resolve(MergeResolveCli),
}

/// `ew merge resolve`.
#[derive(Args, Debug)]
struct MergeResolveCli {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    project: Option<PathBuf>,
    /// The side to take.
    #[arg(long, value_enum, value_name = "side")]
    take: TakeArg,
    /// Only the conflicts on records of this kind (`connection`), or `file`.
    #[arg(long, value_name = "kind")]
    kind: Option<String>,
    /// Only the conflicts on this member (`overrides.colour`).
    #[arg(long, value_name = "path")]
    field: Option<String>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    json: bool,
}

/// `ew merge resolve --take`.
#[derive(ValueEnum, Clone, Copy, PartialEq, Eq, Debug)]
enum TakeArg {
    /// Ours: the record as the merge left it.
    Ours,
    /// Theirs: the other side.
    Theirs,
}

/// `ew merge-driver`. Every positional takes a leading `-`: `%P` may name a folder such as `-old.ew`. The fourth is
/// named `P`, as the usage line shows the required one, so that three arguments are refused naming `<P>`.
#[derive(Args, Debug)]
struct MergeDriverCli {
    /// `%O`: the ancestor's version of the file.
    #[arg(value_name = "O", allow_hyphen_values = true)]
    base: PathBuf,
    /// `%A`: ours; the merged file is written here.
    #[arg(value_name = "A", allow_hyphen_values = true)]
    ours: PathBuf,
    /// `%B`: theirs.
    #[arg(value_name = "B", allow_hyphen_values = true)]
    theirs: PathBuf,
    /// `%P` alone, or `%L` (unused) when `%P` follows it.
    #[arg(value_name = "P", allow_hyphen_values = true)]
    fourth: PathBuf,
    /// `%P` after `%L`: the file's path in the repository.
    #[arg(value_name = "P", allow_hyphen_values = true)]
    path: Option<PathBuf>,
}

/// `ew lib`.
#[derive(Args, Debug)]
struct LibCli {
    /// The subcommand.
    #[command(subcommand)]
    action: LibSub,
    /// A company library: a library directory or a directory of `*.ewlib` libraries (repeatable).
    #[arg(long, value_name = "dir", allow_hyphen_values = true, global = true)]
    company: Vec<PathBuf>,
    /// Print JSON.
    #[arg(long, overrides_with = "json", global = true)]
    json: bool,
}

/// `ew lib list|search|index|create|import`.
#[derive(Subcommand, Debug)]
enum LibSub {
    /// List the libraries.
    ///
    /// Prints each library (tier, key, version, name, ID, `rw` or `ro`, index state, directory) and what could not be
    /// read.
    List,
    /// Find parts or other library items.
    ///
    /// Finds parts by part number, manufacturer and description (`--kind part`, the default), or symbols, FnDefs,
    /// forms, macros and manufacturers by key and name (`--kind symbol|fndef|form|macro|manufacturer`); `--all`
    /// includes deprecated and withdrawn items, `--limit` caps the hits (default 50, 0: all).
    Search(SearchCli),
    /// Rebuild the search index from the libraries.
    Index(IndexCli),
    /// Make an empty library with a new ID.
    ///
    /// Makes an empty library in `<dir>` (absent or empty) with a new ID; it is listed when `<dir>` lies in the user
    /// tier or a `--company` directory.
    Create(CreateCli),
    /// Import into a library.
    #[command(subcommand)]
    Import(LibImportCli),
}

/// `ew lib search`.
#[derive(Args, Debug)]
struct SearchCli {
    /// The text to search for.
    #[arg(value_name = "text", allow_hyphen_values = true, value_parser = word)]
    text: String,
    /// The kind of item (repeatable; default: part).
    #[arg(long, value_name = "kind", allow_hyphen_values = true, value_parser = item_kind)]
    kind: Vec<String>,
    /// Include deprecated and withdrawn items.
    #[arg(long, overrides_with = "all")]
    all: bool,
    /// The most hits (0: all).
    #[arg(long, value_name = "n", allow_hyphen_values = true, overrides_with = "limit", value_parser = limit, default_value_t = 50)]
    limit: u32,
}

/// `ew lib index`.
#[derive(Args, Debug)]
struct IndexCli {
    /// Rebuild the index.
    #[arg(long, overrides_with = "rebuild", required = true)]
    rebuild: bool,
}

/// `ew lib create`.
#[derive(Args, Debug)]
struct CreateCli {
    /// The new library's directory (absent or empty).
    #[arg(value_name = "dir", allow_hyphen_values = true, value_parser = path_word)]
    dir: PathBuf,
    /// The library's key.
    #[arg(long, value_name = "key", allow_hyphen_values = true, overrides_with = "key")]
    key: String,
    /// The library's name.
    #[arg(
        long,
        value_name = "name",
        allow_hyphen_values = true,
        overrides_with = "name"
    )]
    name: String,
    /// The library's licence (an SPDX expression).
    #[arg(
        long,
        value_name = "spdx",
        allow_hyphen_values = true,
        overrides_with = "licence"
    )]
    licence: String,
}

/// `ew lib import`.
#[derive(Subcommand, Debug)]
enum LibImportCli {
    /// Import a parts table into a writable library through a column map.
    ///
    /// Imports a parts table through a column map (as `import parts --map`) into the writable library `<library>`
    /// (its key or ID) under the library's lock, with the parts' import date `--as-of` (YYYY-MM-DD); new
    /// manufacturers are written with the parts. A second import of a changed list updates the parts in place (same
    /// item IDs); a part whose content changed under its published version refuses the whole file unless `--bump`
    /// writes it as the next patch version. A library locked by a live session is refused; a stale lock is taken
    /// over only with `--take-over`.
    Parts(LibImportPartsCli),
}

/// `ew lib import parts`.
#[derive(Args, Debug)]
struct LibImportPartsCli {
    /// The target library, by its key or ID.
    #[arg(value_name = "library", allow_hyphen_values = true, value_parser = word)]
    library: String,
    /// The parts table (CSV or XLSX).
    #[arg(value_name = "table", allow_hyphen_values = true, value_parser = path_word)]
    file: PathBuf,
    /// The column map (a TOML file).
    #[arg(
        long,
        value_name = "map.toml",
        allow_hyphen_values = true,
        overrides_with = "map"
    )]
    map: PathBuf,
    /// The parts' import date (YYYY-MM-DD).
    #[arg(long, value_name = "date", allow_hyphen_values = true, overrides_with = "as_of", value_parser = date)]
    as_of: ew_api::catalog::Date,
    /// Write changed parts as the next patch version.
    #[arg(long, overrides_with = "bump")]
    bump: bool,
    /// Take over a stale library lock.
    #[arg(long, overrides_with = "take_over")]
    take_over: bool,
}

// ---------------------------------------------------------------------------------------------------------------
// The value parsers: each keeps the refusal's words.

/// A `--scheme` that ships.
fn scheme_name(v: &str) -> Result<String, String> {
    match ew_api::checks::scheme(v) {
        Some(_) => Ok(v.to_owned()),
        None => Err(format!("unknown scheme `{v}`")),
    }
}

/// A `--lang` tag of a shipped language, any case, stored canonical.
fn language(v: &str) -> Result<String, String> {
    let languages = ew_api::checks::messages::LANGUAGES;
    match languages.iter().find(|l| l.eq_ignore_ascii_case(v)) {
        Some(l) => Ok((*l).to_owned()),
        None => Err(format!(
            "unknown message language `{v}` ({})",
            languages.join(", ")
        )),
    }
}

/// An `--as-of` date `YYYY-MM-DD`.
pub(super) fn date(v: &str) -> Result<ew_api::catalog::Date, String> {
    v.parse()
        .map_err(|_| format!("`--as-of` needs a date YYYY-MM-DD, not `{v}`"))
}

/// An `import qet --report` format.
pub(super) fn report(v: &str) -> Result<ReportFormat, String> {
    v.parse::<ReportFormat>().map_err(|e| e.to_string())
}

/// A `--dpi` value: 72 to 600 (WP-S1.22 D7).
fn dpi(v: &str) -> Result<u32, String> {
    match v.parse::<u32>() {
        Ok(n) if ew_api::output::DPI_RANGE.contains(&n) => Ok(n),
        _ => Err(format!(
            "`--dpi` needs a number from {} to {}, got `{v}`",
            ew_api::output::DPI_RANGE.start(),
            ew_api::output::DPI_RANGE.end()
        )),
    }
}

/// A `--threads` value: 1 to 256.
fn threads(v: &str) -> Result<usize, String> {
    match v.parse::<usize>() {
        Ok(n) if (1..=256).contains(&n) => Ok(n),
        _ => Err(format!("`--threads` needs a number from 1 to 256, got `{v}`")),
    }
}

/// A `--pages` selection.
fn pages(v: &str) -> Result<PageSelection, String> {
    PageSelection::parse(v).map_err(|e| e.to_string())
}

/// A `--sections` selection.
fn sections(v: &str) -> Result<PageSelection, String> {
    PageSelection::parse_sections(v).map_err(|e| e.to_string())
}

/// A `--paper` name.
fn paper(v: &str) -> Result<Paper, String> {
    Paper::parse(v).map_err(|e| e.to_string())
}

/// The selection of `--pages` or `--sections` (WP-S3.07 D6): both together is a usage error, neither is every page.
fn selection(
    path: &[&str],
    pages: Option<PageSelection>,
    sections: Option<PageSelection>,
) -> Result<PageSelection, clap::Error> {
    match (pages, sections) {
        (Some(_), Some(_)) => Err(usage_error(path, "give `--pages` or `--sections`, not both")),
        (Some(s), None) | (None, Some(s)) => Ok(s),
        (None, None) => Ok(PageSelection::All),
    }
}

/// A `--timeout` in seconds: 1 to the sandbox's longest deadline (one day).
fn timeout(v: &str) -> Result<u64, String> {
    let max = ew_api::sql::Profile::MAX_DEADLINE.as_secs();
    match v.parse::<u64>() {
        Ok(n) if (1..=max).contains(&n) => Ok(n),
        _ => Err(format!(
            "`--timeout` needs a number of seconds from 1 to {max}, got `{v}`"
        )),
    }
}

/// A positional of `sql`: an inline query may start with an SQL comment (`-- …`), but a single word starting with
/// `-` is a mistyped option.
fn sql_word(v: &str) -> Result<String, String> {
    if v.starts_with('-') && !v.contains(char::is_whitespace) {
        Err(format!("unknown option `{v}`"))
    } else {
        Ok(v.to_owned())
    }
}

/// A positional that takes hyphen values (`query`, `lib search`, `lib create`, `lib import parts`): a word starting
/// with one `-` is a value (a bare tag `-X1031:5`, or `-` for a query without a key), but one starting with `--` is a
/// mistyped option, as the old parser refused it.
fn word(v: &str) -> Result<String, String> {
    if v.starts_with("--") {
        Err(format!("unknown option `{v}`"))
    } else {
        Ok(v.to_owned())
    }
}

/// [`word`] for a path positional.
fn path_word(v: &str) -> Result<PathBuf, String> {
    word(v).map(PathBuf::from)
}

/// An `sql --format`.
fn sql_format(v: &str) -> Result<SqlFormat, String> {
    match v {
        "csv" => Ok(SqlFormat::Csv),
        "json" => Ok(SqlFormat::Json),
        "xlsx" => Ok(SqlFormat::Xlsx),
        other => Err(format!("unknown format `{other}` (csv, json, xlsx)")),
    }
}

/// The `--format` of a row export (`export cables|bom|wires|terminals`): CSV or XLSX; `pdf` is refused by the
/// lowering with the subject's words ([`csv_only`]).
fn row_format(v: &str) -> Result<ReportPages, String> {
    match v {
        "csv" => Ok(ReportPages::Csv),
        "xlsx" => Ok(ReportPages::Xlsx),
        "pdf" => Ok(ReportPages::Pdf),
        other => Err(format!("unknown format `{other}` (csv, xlsx)")),
    }
}

/// The `--format` of `export table`: CSV or XLSX.
fn table_format(v: &str) -> Result<ReportPages, String> {
    match v {
        "csv" => Ok(ReportPages::Csv),
        "xlsx" => Ok(ReportPages::Xlsx),
        other => Err(format!("unknown format `{other}` (csv, xlsx)")),
    }
}

/// The `--preset` of `import table` (WP-S3.08 D11): a preset of the round trip, not the golden dialect.
fn import_preset(v: &str) -> Result<Preset, String> {
    Preset::parse(v).filter(|p| *p != Preset::Ew).ok_or_else(|| {
        let names: Vec<&str> = Preset::ALL
            .iter()
            .filter(|p| **p != Preset::Ew)
            .map(|p| p.name())
            .collect();
        format!("unknown preset `{v}` ({})", names.join(", "))
    })
}

/// A `--preset` (WP-S3.08 D6).
fn preset(v: &str) -> Result<Preset, String> {
    Preset::parse(v).ok_or_else(|| {
        let names: Vec<&str> = Preset::ALL.iter().map(|p| p.name()).collect();
        format!("unknown preset `{v}` ({})", names.join(", "))
    })
}

/// A `--bom`.
fn bom(v: &str) -> Result<bool, String> {
    match v {
        "on" => Ok(true),
        "off" => Ok(false),
        other => Err(format!("unknown byte-order mark setting `{other}` (on, off)")),
    }
}

/// A `--line-end`.
fn line_end(v: &str) -> Result<LineEnd, String> {
    LineEnd::parse(v).ok_or_else(|| format!("unknown line end `{v}` (crlf, lf)"))
}

/// A row export's `--format pdf`: the row exports write rows, and the refusal points to the report PDF.
fn csv_only(what: &str, format: Option<ReportPages>) -> Result<(), clap::Error> {
    match format {
        Some(ReportPages::Pdf) => Err(usage_error(
            &["export", what],
            format!(
                "`export {what}` writes rows as CSV or XLSX; a report block's pages go to PDF with `ew export report --block <block> --format pdf`"
            ),
        )),
        _ => Ok(()),
    }
}

/// The `--format` of `export report`.
fn report_format(v: &str) -> Result<ReportPages, String> {
    match v {
        "csv" => Ok(ReportPages::Csv),
        "xlsx" => Ok(ReportPages::Xlsx),
        "pdf" => Ok(ReportPages::Pdf),
        other => Err(format!("unknown format `{other}` (csv, xlsx, pdf)")),
    }
}

/// A `lib search --kind`.
fn item_kind(v: &str) -> Result<String, String> {
    match ew_api::library::item_kind(v) {
        Some(_) => Ok(v.to_owned()),
        None => Err(format!(
            "unknown kind `{v}` (part, symbol, fndef, form, macro, manufacturer)"
        )),
    }
}

/// A `lib search --limit`.
fn limit(v: &str) -> Result<u32, String> {
    v.parse::<u32>()
        .map_err(|_| format!("`--limit` needs a number, not `{v}`"))
}

// ---------------------------------------------------------------------------------------------------------------
// The lowering.

/// A usage error of the subcommand at `path`, with clap's usage line for it.
fn usage_error(path: &[&str], message: impl std::fmt::Display) -> clap::Error {
    /// The error of the command at `path` below `cmd`; the paths are the tree's own, a missing word stops there.
    fn at(cmd: &mut clap::Command, path: &[&str], message: String) -> clap::Error {
        if let Some((word, rest)) = path.split_first()
            && let Some(sub) = cmd.find_subcommand_mut(word)
        {
            return at(sub, rest, message);
        }
        cmd.error(ErrorKind::ArgumentConflict, message)
    }
    let mut root = Cli::command();
    root.build();
    at(&mut root, path, message.to_string())
}

/// `--json` beside an explicit `--format`: `--json` is the alias of `--format json`, so another format is a usage
/// error (D3 (c)).
fn json_alias(path: &[&str], json: bool, format: Option<&str>) -> Result<(), clap::Error> {
    match format {
        Some(f) if json && f != "json" => Err(usage_error(
            path,
            format!("`--json` cannot be combined with `--format {f}` (`--json` is `--format json`)"),
        )),
        _ => Ok(()),
    }
}

/// The refused flags of `export pdf|svg|png`: accepted by the grammar, refused as not built.
fn not_built(path: &[&str], lang: bool, omit_personal_data: bool) -> Result<(), clap::Error> {
    match (lang, omit_personal_data) {
        (true, _) => Err(usage_error(path, "`--lang` is not built yet")),
        (_, true) => Err(usage_error(path, "`--omit-personal-data` is not built yet")),
        _ => Ok(()),
    }
}

/// The project of a line: `--project` or the positional directory (the relations exclude both).
fn project(flag: Option<PathBuf>, positional: Option<PathBuf>) -> Option<PathBuf> {
    flag.or(positional)
}

/// The project of a line whose positionals are resolved by their count (`apply`, `query`, `sql`, `sql dump`): the
/// relations would name the positional by its other meaning, so both together is refused here, in the words clap
/// uses for the other subcommands.
fn counted_project(
    path: &[&str],
    flag: Option<PathBuf>,
    positional: Option<PathBuf>,
) -> Result<Option<PathBuf>, clap::Error> {
    match (flag, positional) {
        (Some(_), Some(_)) => Err(usage_error(
            path,
            "the argument '[project-dir]' cannot be used with '--project <dir>'",
        )),
        (flag, positional) => Ok(flag.or(positional)),
    }
}

// Every `*Cli` is destructured whole, with no `..`: a field the lowering forgets is E0027, and a binding it does not
// use is `unused_variables` (C43-p4#3).
impl Cli {
    /// The command a parsed line names.
    ///
    /// # Errors
    ///
    /// A usage error for a rule the relations of the tree cannot say.
    pub(super) fn into_command(self) -> Result<Command, clap::Error> {
        Ok(match self.command {
            Top::Import(Import::Qet(a)) => {
                if a.json && a.report.is_some() && a.report_out.is_none() {
                    return Err(usage_error(
                        &["import", "qet"],
                        "`--json` prints the result on stdout: give the report a file with `--report-out <file>`",
                    ));
                }
                Command::ImportQet(a)
            }
            Top::Import(Import::Parts(a)) => Command::ImportParts(a),
            Top::Import(Import::Csv(a)) => Command::ImportCsv(a),
            Top::Import(Import::Table(ImportTableCli {
                file,
                project_dir,
                project: flag,
                table,
                preset,
                take_file,
                apply,
                json,
            })) => Command::ImportTable(ImportTableArgs {
                file,
                project: project(flag, project_dir),
                table,
                preset,
                take_file,
                apply,
                json,
            }),
            Top::Check(CheckCli {
                project_dir,
                project: flag,
                scheme,
                mode,
                format,
                json,
                lang,
                out,
                integrity,
                after_merge,
            }) => {
                let format_name = format.map(|f| match f {
                    CheckFormat::Text => "text",
                    CheckFormat::Markdown => "md",
                    CheckFormat::Json => "json",
                });
                json_alias(&["check"], json, format_name)?;
                Command::Check(CheckArgs {
                    project: project(flag, project_dir),
                    scheme,
                    mode: match mode {
                        ModeArg::Release => CheckMode::Release,
                        ModeArg::Online => CheckMode::Online,
                    },
                    format: if json {
                        CheckFormat::Json
                    } else {
                        format.unwrap_or(CheckFormat::Text)
                    },
                    out,
                    lang,
                    integrity,
                    after_merge,
                })
            }
            Top::Sql(a) => sql(a)?,
            Top::Export(e) => export(e)?,
            Top::Extract(ExtractCli { file, into, json }) => {
                Command::Extract(ExtractArgs { file, into, json })
            }
            Top::Print(PrintCli {
                project_dir,
                project: flag,
                pages,
                sections,
                paper,
                fit,
                mono,
                printer,
                via,
                out,
                json,
            }) => {
                let pages = selection(&["print"], pages, sections)?;
                let via = match via {
                    Some(ViaArg::Lp) => PrintVia::Lp,
                    Some(ViaArg::Viewer) => PrintVia::Viewer,
                    #[cfg(windows)]
                    Some(ViaArg::Windows) | None => PrintVia::Windows,
                    #[cfg(not(windows))]
                    Some(ViaArg::Windows) => {
                        return Err(usage_error(&["print"], "`--via windows` prints on Windows only"));
                    }
                    #[cfg(not(windows))]
                    None if cfg!(target_os = "linux") => PrintVia::Lp,
                    #[cfg(not(windows))]
                    None => PrintVia::Viewer,
                };
                if via == PrintVia::Viewer && printer.is_some() {
                    return Err(usage_error(&["print"], ew_print::PrintError::PrinterWithViewer));
                }
                Command::Print(PrintArgs {
                    project: project(flag, project_dir),
                    pages,
                    paper: paper.unwrap_or_default(),
                    fit,
                    mono,
                    printer,
                    via,
                    out,
                    json,
                })
            }
            Top::Render(RenderCli {
                project_dir,
                project: flag,
                page,
                // `--png` is required: PNG is the only page format.
                png: _required,
                dpi,
                out,
                json,
            }) => Command::Render(RenderArgs {
                project: project(flag, project_dir),
                page,
                dpi,
                out,
                json,
            }),
            Top::Apply(ApplyCli {
                first,
                second,
                project: flag,
                preview,
                one_step,
                json,
            }) => {
                let (dir, log) = match second {
                    Some(log) => (Some(first), log),
                    None => (None, first),
                };
                Command::Apply(ApplyArgs {
                    project: counted_project(&["apply"], flag, dir)?,
                    log,
                    preview,
                    one_step,
                    json,
                })
            }
            Top::Fmt(FmtCli {
                project_dir,
                project: flag,
                check,
                repair,
                json,
            }) => Command::Fmt(FmtArgs {
                project: project(flag, project_dir),
                check,
                repair,
                json,
            }),
            Top::Recover(RecoverCli {
                project_dir,
                project: flag,
                replay,
                discard,
                json,
            }) => {
                let action = match (replay, discard) {
                    (true, _) => RecoverAction::Replay,
                    (false, true) => RecoverAction::Discard,
                    (false, false) => RecoverAction::Summary,
                };
                if action == RecoverAction::Discard && json {
                    return Err(usage_error(
                        &["recover"],
                        "`--discard` asks for a typed word on a terminal and prints no result: it takes no `--json`",
                    ));
                }
                Command::Recover(RecoverArgs {
                    project: project(flag, project_dir),
                    action,
                    json,
                })
            }
            Top::Upgrade(UpgradeCli {
                project_dir,
                project: flag,
                check,
                yes,
            }) => Command::Upgrade(UpgradeArgs {
                project: project(flag, project_dir),
                check,
                yes,
            }),
            Top::Project(ProjectCli::Checks(ChecksCli::Set(ChecksSetCli {
                project_dir,
                project: flag,
                active,
                release,
            }))) => Command::ChecksSet(ChecksSetArgs {
                project: project(flag, project_dir),
                active,
                release,
            }),
            Top::Project(ProjectCli::Assets(AssetsCli::Purge(AssetsPurgeCli {
                project_dir,
                project: flag,
                preview,
            }))) => Command::AssetsPurge(AssetsPurgeArgs {
                project: project(flag, project_dir),
                preview,
            }),
            Top::Revision(r) => {
                let (action, dir, flag) = match r {
                    RevisionCli::Approve(ApproveCli {
                        project_dir,
                        project: flag,
                        role,
                    }) => (RevisionAction::Approve { role }, project_dir, flag),
                    RevisionCli::Release(ReleaseCli {
                        label,
                        project_dir,
                        project: flag,
                        archive_dir,
                    }) => (RevisionAction::Release { label, archive_dir }, project_dir, flag),
                    RevisionCli::Withdraw(WithdrawCli {
                        label,
                        project_dir,
                        reason,
                        project: flag,
                    }) => (RevisionAction::Withdraw { label, reason }, project_dir, flag),
                };
                Command::Revision(RevisionArgs {
                    action,
                    project: project(flag, dir),
                })
            }
            Top::VerifyDerived(ProjectJsonCli {
                project_dir,
                project: flag,
                json,
            }) => Command::VerifyDerived(VerifyArgs {
                project: project(flag, project_dir),
                json,
            }),
            Top::Query(QueryCli {
                first,
                second,
                third,
                project: flag,
                explain,
                json,
            }) => {
                let (dir, name, key) = match third {
                    Some(key) => (Some(PathBuf::from(first)), second, key),
                    None => (None, first, second),
                };
                Command::Query(QueryArgs {
                    project: counted_project(&["query"], flag, dir)?,
                    name,
                    key,
                    explain,
                    json,
                })
            }
            Top::Queries(JsonCli { json }) => Command::Queries { json },
            Top::Lib(LibCli {
                action,
                company,
                json,
            }) => Command::Lib(LibArgs {
                action: match action {
                    LibSub::List => LibAction::List,
                    LibSub::Search(SearchCli {
                        text,
                        kind,
                        all,
                        limit,
                    }) => LibAction::Search {
                        text,
                        kinds: kind,
                        all,
                        limit,
                    },
                    // `--rebuild` is required, so it is always set: the old parser refused `lib index` without it,
                    // and `LibAction::IndexRebuild` carries nothing.
                    LibSub::Index(IndexCli { rebuild: _required }) => LibAction::IndexRebuild,
                    LibSub::Create(CreateCli {
                        dir,
                        key,
                        name,
                        licence,
                    }) => LibAction::Create {
                        dir,
                        key,
                        name,
                        licence,
                    },
                    LibSub::Import(LibImportCli::Parts(LibImportPartsCli {
                        library,
                        file,
                        map,
                        as_of,
                        bump,
                        take_over,
                    })) => LibAction::ImportParts {
                        library,
                        file,
                        map,
                        as_of,
                        bump,
                        take_over,
                    },
                },
                company,
                json,
            }),
            Top::Commands(JsonCli { json }) => Command::Commands { json },
            Top::Schema(SchemaCli { name }) => Command::Schema(name),
            Top::Merge(MergeCli::Resolve(MergeResolveCli {
                project_dir,
                project: flag,
                take,
                kind,
                field,
                json,
            })) => Command::MergeResolve(MergeResolveArgs {
                project: project(flag, project_dir),
                theirs: take == TakeArg::Theirs,
                kind,
                field,
                json,
            }),
            Top::MergeDriver(MergeDriverCli {
                base,
                ours,
                theirs,
                fourth,
                path,
            }) => Command::MergeDriver(MergeDriverArgs {
                base,
                ours,
                theirs,
                // `%O %A %B %L %P`, or the older `%O %A %B %P`: `%L` is unused.
                path: path.unwrap_or(fourth).to_string_lossy().into_owned(),
            }),
            Top::Diff(DiffCli {
                from,
                to,
                textconv,
                exit_code,
                json,
            }) => Command::Diff(DiffArgs {
                from,
                to,
                textconv,
                json,
                exit_code,
            }),
            Top::Git(GitCli::Setup(GitSetupCli { global, hooks, yes })) => {
                Command::GitSetup(GitSetupArgs { global, hooks, yes })
            }
        })
    }
}

/// `ew sql` and its subcommands.
fn sql(a: SqlCli) -> Result<Command, clap::Error> {
    let SqlCli {
        sub,
        first,
        second,
        project: flag,
        file,
        format,
        csv,
        json,
        timeout,
        out,
    } = a;
    match sub {
        Some(SqlSub::Schema) => return Ok(Command::SqlSchema),
        Some(SqlSub::Dump(SqlDumpCli {
            first,
            second,
            project: flag,
            json,
        })) => {
            let (dir, out) = match second {
                Some(out) => (Some(PathBuf::from(first)), out),
                None => (None, first),
            };
            return Ok(Command::SqlDump(SqlDumpArgs {
                project: counted_project(&["sql", "dump"], flag, dir)?,
                out: PathBuf::from(out),
                json,
            }));
        }
        None => {}
    }
    let (dir, query) = match (file, first, second) {
        (Some(file), dir, _) => (dir.map(PathBuf::from), SqlSource::File(file)),
        (None, Some(dir), Some(q)) => (Some(PathBuf::from(dir)), SqlSource::Text(q)),
        (None, Some(q), None) => (None, SqlSource::Text(q)),
        // `required_unless_present = "file"` gives the query.
        (None, None, _) => return Err(usage_error(&["sql"], "`sql` needs a query (or `--file`)")),
    };
    let format_name = format.map(|f| match f {
        SqlFormat::Csv => "csv",
        SqlFormat::Json => "json",
        SqlFormat::Xlsx => "xlsx",
    });
    let project = counted_project(&["sql"], flag, dir)?;
    json_alias(&["sql"], json, format_name)?;
    let format = if json {
        SqlFormat::Json
    } else {
        format.unwrap_or(SqlFormat::Csv)
    };
    let csv = csv.options(
        &["sql"],
        match format {
            SqlFormat::Csv => "csv",
            SqlFormat::Json => "json",
            SqlFormat::Xlsx => "xlsx",
        },
    )?;
    if format == SqlFormat::Xlsx && out.is_none() {
        return Err(usage_error(&["sql"], XLSX_NEEDS_OUT));
    }
    Ok(Command::Sql(SqlArgs {
        project,
        query,
        format,
        timeout,
        out,
        csv,
    }))
}

/// The refusal of `ew export ewz` without `--out` (ID-1175 ruling 5).
pub(super) const EWZ_NEEDS_OUT: &str = "`ew export ewz` writes a file: give `--out <file.ewz>`";

/// `ew export` and its subjects.
fn export(e: ExportCli) -> Result<Command, clap::Error> {
    let rows = |kind, block, flag, dir, out, json, format| {
        Command::Export(ExportArgs {
            kind,
            block,
            project: project(flag, dir),
            out,
            json,
            format,
        })
    };
    Ok(match e {
        ExportCli::Cables(RowsCli {
            project_dir,
            project: flag,
            block,
            format,
            csv,
            out,
            json,
        }) => {
            csv_only("cables", format)?;
            let format = rows_format(&["export", "cables"], format, csv, out.as_ref())?;
            rows(ExportKind::Cables, block, flag, project_dir, out, json, format)
        }
        ExportCli::Wires(RowsCli {
            project_dir,
            project: flag,
            block,
            format,
            csv,
            out,
            json,
        }) => {
            csv_only("wires", format)?;
            let format = rows_format(&["export", "wires"], format, csv, out.as_ref())?;
            rows(ExportKind::Wires, block, flag, project_dir, out, json, format)
        }
        ExportCli::Bom(BomCli {
            project_dir,
            project: flag,
            block,
            mode,
            format,
            csv,
            out,
            json,
        }) => {
            csv_only("bom", format)?;
            let format = rows_format(&["export", "bom"], format, csv, out.as_ref())?;
            let mode = match mode {
                BomArg::Parts => BomMode::Component,
                BomArg::Assembly => BomMode::Assembly,
            };
            rows(ExportKind::Bom(mode), block, flag, project_dir, out, json, format)
        }
        ExportCli::Terminals(TerminalsCli {
            project_dir,
            project: flag,
            block,
            strip,
            format,
            csv,
            out,
            json,
        }) => {
            csv_only("terminals", format)?;
            let format = rows_format(&["export", "terminals"], format, csv, out.as_ref())?;
            rows(
                ExportKind::Terminals(strip),
                block,
                flag,
                project_dir,
                out,
                json,
                format,
            )
        }
        ExportCli::Report(ReportCli {
            project_dir,
            project: flag,
            block,
            kind,
            format,
            csv,
            out,
            json,
        }) => {
            let pdf = format == Some(ReportPages::Pdf);
            let kind = match kind {
                Some(name) => {
                    // WP-S3.06 D4: the refusals are the report layer's texts, the pair first.
                    if block.is_some() {
                        return Err(usage_error(
                            &["export", "report"],
                            ew_api::output::RowsError::KindAndBlock,
                        ));
                    }
                    if pdf {
                        return Err(usage_error(
                            &["export", "report"],
                            ew_api::output::RowsError::KindPages,
                        ));
                    }
                    ExportKind::ReportKind(name)
                }
                None => ExportKind::Report { pdf },
            };
            let format = rows_format(&["export", "report"], format, csv, out.as_ref())?;
            rows(kind, block, flag, project_dir, out, json, format)
        }
        ExportCli::Ewz(EwzCli {
            project_dir,
            project: flag,
            out,
            json,
        }) => {
            let Some(out) = out else {
                return Err(usage_error(&["export", "ewz"], EWZ_NEEDS_OUT));
            };
            Command::ExportEwz(ExportEwzArgs {
                project: project(flag, project_dir),
                out,
                json,
            })
        }
        ExportCli::Table(TableCli {
            navigator,
            project_dir,
            project: flag,
            strip,
            format,
            csv,
            out,
            json,
        }) => {
            let path = &["export", "table"];
            // D8: the refusals are the round trip's texts; the navigator's first.
            if ew_api::output::NavKind::from_word(&navigator)
                .is_none_or(|k| !ew_api::output::TABLE_NAVIGATORS.contains(&k))
            {
                return Err(usage_error(
                    path,
                    ew_api::output::TableError::UnknownNavigator(navigator),
                ));
            }
            if strip.is_some() && navigator != "strips" {
                return Err(usage_error(path, ew_api::output::TableError::StripElsewhere));
            }
            Command::ExportTable(ExportTableArgs {
                format: rows_format(path, format, csv, out.as_ref())?,
                navigator,
                strip,
                project: project(flag, project_dir),
                out,
                json,
            })
        }
        ExportCli::Pdf(a) => pages_command(PageFormat::Pdf, a)?,
        ExportCli::Svg(a) => pages_command(PageFormat::Svg, a)?,
        ExportCli::Png(PngCli {
            project_dir,
            project: flag,
            out,
            pages,
            sections,
            dpi,
            threads,
            json,
            lang,
            omit_personal_data,
        }) => {
            not_built(&["export", "png"], lang.is_some(), omit_personal_data)?;
            Command::ExportPages(ExportPagesArgs {
                format: PageFormat::Png,
                project: project(flag, project_dir),
                out,
                pages: selection(&["export", "png"], pages, sections)?,
                threads,
                dpi,
                json,
            })
        }
    })
}

/// `ew export pdf|svg`.
fn pages_command(format: PageFormat, a: PagesCli) -> Result<Command, clap::Error> {
    let PagesCli {
        project_dir,
        project: flag,
        out,
        pages,
        sections,
        threads,
        json,
        lang,
        omit_personal_data,
    } = a;
    let path: &[&str] = match format {
        PageFormat::Pdf => &["export", "pdf"],
        PageFormat::Svg | PageFormat::Png => &["export", "svg"],
    };
    not_built(path, lang.is_some(), omit_personal_data)?;
    Ok(Command::ExportPages(ExportPagesArgs {
        format,
        project: project(flag, project_dir),
        out,
        pages: selection(path, pages, sections)?,
        threads,
        dpi: ew_api::output::DEFAULT_DPI,
        json,
    }))
}
