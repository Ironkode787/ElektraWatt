// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `ew` command line as one clap derive tree: every subcommand and flag is declared here once, with its help in
//! its doc comment. A subcommand is one structure: its arguments as clap parses them (`*Args`, `*Cli`), which its run
//! function takes, and beside the run function its [`Leaf`]: its exit codes, the schema name of its `--json` result
//! and the rules of its line that the tree's relations cannot say. Some structures live beside their run function
//! (`dxf.rs`, `eplan.rs`, `symbol.rs`, the imports of `cli.rs`). A group of subcommands is an enum declared through
//! [`group!`], which walks its variants for the check of a line and for the CLI catalogue (`catalogue.rs`).
//!
//! The grammar (doc 17 §3.1): a valued flag takes the next word as its value even when it starts with
//! `-`; a second `--project` is an error, `--company` and `lib search --kind` accumulate, every other flag is
//! last-wins; the positional project directory and `--project` exclude each other; a positional that takes hyphen
//! values refuses a word starting with `--` as a mistyped option. A rule the relations cannot say is the
//! subcommand's [`Leaf::check`], which returns a usage error when the line is parsed; the methods below each structure
//! read a line that needs converting (a positional read by the count, `--json` as `--format json`, a platform
//! default).

use std::path::PathBuf;

use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::error::ErrorKind;
use clap::{ArgGroup, Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use ew_api::BomMode;
use ew_api::checks::CheckMode;
use ew_api::import::migration::ReportFormat;
use ew_api::output::print::Paper;
use ew_api::output::{CsvOptions, LineEnd, PageSelection, Preset};

use super::export::{DrillingCli, LabelsCli, PackageCli, ReworkCli};
use super::io::Io;
use super::{
    CheckFormat, ExportArgs, ExportKind, ExportPagesArgs, ImportCsvArgs, ImportPartsArgs, ImportQetArgs,
    PageFormat, PrintVia, RecoverAction, RevisionAction, RevisionArgs, RowFormat, SqlFormat, SqlSource,
    XLSX_NEEDS_OUT,
};
use crate::catalogue::{Entry, Exit};

/// A subcommand path of the tree, stated beside its run function: its exit codes, the schema name of its `--json`
/// result, and the rules of its line that the relations of the tree cannot say.
pub(crate) trait Leaf {
    /// Its exit codes, each with what it means for this subcommand.
    const EXITS: &'static [Exit];
    /// The schema name of its `--json` result (`ew.<command>-result/1`), or `None` when it prints none.
    const RESULT: Option<&'static str> = None;

    /// A usage error for a rule of the line that the relations cannot say, found when the line is parsed, before
    /// anything runs.
    ///
    /// # Errors
    ///
    /// The usage error, with clap's usage line of the subcommand.
    fn check(&self) -> Result<(), clap::Error> {
        Ok(())
    }

    /// The catalogue entries of the paths below this one, `cmd` its clap command at `path` (`sql dump` and `sql
    /// schema` below `sql`); none for most.
    fn below(_cmd: &clap::Command, _path: &mut Vec<String>, _out: &mut Vec<Entry>) {}
}

/// A node of the tree: a subcommand path ([`Leaf`]) or a group of subcommands ([`group!`]).
pub(crate) trait Node {
    /// [`Leaf::check`] of the subcommand this node holds.
    ///
    /// # Errors
    ///
    /// Its usage error.
    fn check(&self) -> Result<(), clap::Error>;

    /// The catalogue entries of this node, `cmd` its clap command at `path`, in declaration order.
    fn entries(cmd: &clap::Command, path: &mut Vec<String>, out: &mut Vec<Entry>);
}

impl<T: Leaf> Node for T {
    fn check(&self) -> Result<(), clap::Error> {
        Leaf::check(self)
    }

    fn entries(cmd: &clap::Command, path: &mut Vec<String>, out: &mut Vec<Entry>) {
        out.push(crate::catalogue::entry(cmd, path, T::EXITS, T::RESULT));
        T::below(cmd, path, out);
    }
}

/// A group of subcommands: the enum as written, each variant holding the node of one subcommand, and its [`Node`],
/// which visits the variants in declaration order, the order of clap's subcommands of the group.
macro_rules! group {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident($node:ty), )*
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $( $(#[$variant_meta])* $variant($node), )*
        }

        impl $crate::cli::tree::Node for $name {
            fn check(&self) -> Result<(), clap::Error> {
                match self {
                    $( Self::$variant(node) => $crate::cli::tree::Node::check(node), )*
                }
            }

            fn entries(
                cmd: &clap::Command,
                path: &mut Vec<String>,
                out: &mut Vec<$crate::catalogue::Entry>,
            ) {
                let mut subcommands = $crate::catalogue::subcommands(cmd);
                $( $crate::catalogue::below::<$node>(subcommands.next(), path, out); )*
                assert!(subcommands.next().is_none(), "a subcommand of `{}` without its variant", cmd.get_name());
            }
        }
    };
}
pub(super) use group;

/// Prints the usage error `e` as clap renders it, on the standard error (the help and the version on the standard
/// output), and returns its exit code: a [`Leaf::check`] found when the line was parsed, which a run function meets
/// again only when it is called on an unchecked line.
pub(crate) fn usage(e: &clap::Error, io: &mut Io<'_>) -> i32 {
    let to = if e.use_stderr() {
        &mut *io.err
    } else {
        &mut *io.out
    };
    // Printing fails only when the stream is gone; the exit code still says what happened.
    let _ = write!(to, "{}", e.render());
    e.exit_code()
}

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
pub(crate) struct Cli {
    /// The subcommand.
    #[command(subcommand)]
    pub(super) command: Top,
}

group! {
/// The subcommands of `ew`.
#[derive(Subcommand, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Top {
    /// Create, inspect, import and publish portable symbol drafts.
    #[command(subcommand)]
    Symbol(super::symbol::SymbolArgs),
    /// Import a project, a parts table or tables of the `ew` dialect.
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
    /// The exchange with PLC software: its status.
    #[command(subcommand)]
    Plc(PlcCli),
    /// Unpack an .ewz into a new project folder.
    ///
    /// Reads the `.ewz` whole and checks it before anything is written (its layout, the archive bounds, every file
    /// against the manifest's hash; a hostile name, an entry the manifest does not list or a compression bomb refuses
    /// it), then writes it into `--into <dir>` (default: `<name>.ew` beside the archive), which must not exist: the files
    /// go into the sibling folder `<dir>.tmp-ewz` first, are synced and read back, and that folder is then renamed to
    /// `<dir>`, so `<dir>` never holds a part of a project and a failure leaves neither folder. Exit 0 when extracted, 2
    /// on everything else (usage, an unreadable or refused archive, an existing folder, write errors).
    Extract(ExtractArgs),
    /// Print the pages through the OS, or write the print set as a PDF.
    ///
    /// Builds the print set of the pages (`--pages` folio labels and ranges, or `--sections` section numbers, a
    /// section with its divider; not both; default every page) and hands it to the OS: `--via lp` runs the CUPS
    /// command `lp` (the default on Linux; `--printer` names the printer, else the default one), `--via windows`
    /// shows the Windows print dialog, or prints to the printer `--printer` names without it (the default on
    /// Windows), `--via viewer` opens it in the default PDF application, which asks for the printer (the default
    /// elsewhere). `--paper` prints on a paper size (sheet, the default, or A5, A4, A3, A2, A1, A0, Letter, Legal,
    /// Tabloid, turned like each page); `--fit` scales each page to the whole paper, up or down, and stamps a
    /// scaled page `scaled <n> %` in its bottom margin; without it a larger page is clipped and named in a notice.
    /// `--mono` prints in black and grey. `--out <file.pdf>` writes the print set instead and prints nothing. The
    /// date is `SOURCE_DATE_EPOCH` when set, else the model's. Exit 0 when printed or written, 2 on everything else (usage, open errors, an
    /// unknown folio or section, a refusal of the adapter, write errors).
    Print(PrintCli),
    /// Render one page as PNG.
    ///
    /// `render --page <folio> --png` renders one page to `--out` (default: `<folio>.png` in the current directory,
    /// never stdout) on the CPU with the bundled fonts at `--dpi` (72 to 600, default 150).
    /// Exit 0 on success, 2 on everything else (usage, open errors, an unknown folio, write errors).
    Render(RenderArgs),
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
    Fmt(FmtArgs),
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
    Upgrade(UpgradeArgs),
    /// Update the project's damaged library copies from their libraries.
    ///
    /// A project whose library copy (`lib/<kind>/<id>.json`) is damaged opens read-only. `update-copies` reads each
    /// damaged copy's item from the libraries of `ew lib` (the compiled-in starter, `<content>/libraries` when
    /// `EW_CONTENT_DIR` names the starter content, every `--company <dir>`, the user's) and checks it before anything is
    /// written, then writes every copy a library holds over its damaged file in one save, under the project's lock, and
    /// opens the project again. A copy no library holds stays damaged and is named. Refused before anything is
    /// written (exit 1): no damaged copy that a library holds, libraries' versions that would leave a project that does
    /// not open, a project another session holds, a project that opens read-only for another reason or has other
    /// integrity findings. Exit 0 when every damaged copy was updated and the
    /// project opens for editing, or it has no damaged copy; 1 when refused or a copy stays damaged; 2 on usage, open
    /// and save errors.
    UpdateCopies(UpdateCopiesArgs),
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
    VerifyDerived(VerifyArgs),
    /// Compare an imported project with the PDF of the drawing it came from.
    ///
    /// Draws every page of the project at 150 dpi, reads the matching page of `--reference` and lists where they
    /// differ: texts the reference has and the project lacks (or the other way round, or more than 0.5 mm away), and
    /// frame cells whose drawing differs with the texts masked, so that another font does not count. Pages pair by
    /// folio label when the PDF's page labels name every page, else by order. `--expected <file.json>` declares the
    /// deltas the project is expected to have, an array of `{"kind", "page", "text", "cell"}` (the last three
    /// optional); the check expects exactly those. Exit 0 when only the expected deltas are found, 1 with other
    /// deviations or with an expected delta not found, 2 on usage, open and read errors.
    VerifyMigration(VerifyMigrationArgs),
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
    Queries(QueriesCli),
    /// Measure how the pages are drawn.
    ///
    /// Opens the project as a reader (no lock) and prints the layout measure of the page `--page <folio>` names, else
    /// of every drawn page but the panel-layout pages, in page order: the faults (a wire through a connection point or
    /// a junction it does not end at, two wires on one line, a wire through a symbol's body other than straight out of
    /// its own pin, a junction with four or more arms, two wires whose corners meet without a junction, a symbol on a
    /// symbol or outside the drawing area), the wires without a route, the cost in grid steps, and what a reader
    /// compares (crossings, bends, wires drawn straight, turn-backs, the detour, texts placed off their first place,
    /// texts crossed by a wire or lying on a text or a symbol), each item named by its wires, symbols and texts. A wire
    /// without a route counts as unrouted only: its dashed line is no wire. `--json` prints `ew.layout-result/1`. Exit
    /// 0 when measured, 2 on usage, open errors and a folio that names no drawn page.
    Layout(LayoutArgs),
    /// Route the wires of the pages.
    ///
    /// Runs the window's Route on the page `--page <folio>` names, else on every drawn page but the panel-layout
    /// pages: the wires that are unrouted, broken or routed by the program are routed as nets, with trunks where the
    /// pins allow, and the plan is saved in one step per page. With `--selection <ids>` (record IDs joined by `,`) it
    /// routes the connections named and every connection with an end on a representation named, whoever shaped
    /// them. A wire without a way keeps its route, or stays unrouted, and is listed with what is in its way.
    /// `--arrange` runs Arrange and route instead: the symbols are put in line with the pins they are wired to, on the
    /// side those pins face, within the rules of the page's kind, and then routed; symbols kept in place stay.
    /// `--dry-run` prints the plan and saves nothing. `--json` prints `ew.route-result/1`. Exit 0 when no wire is left
    /// without a way, 1 when one is, 2 on usage, open, an unknown folio, a refused command and save errors.
    Route(RouteArgs),
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
    /// Lists the command registry (name, exposure, cost, confirmation, undoable, aliases); exits 0.
    Commands(CommandsCli),
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
    Diff(DiffArgs),
    /// Git integration.
    #[command(subcommand)]
    Git(GitCli),
}
}

/// `ew diff`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct DiffArgs {
    /// The first side: a directory, an `.ewz`, an `.ewarchive`, `git:<rev>` or `rev:<label>`.
    #[arg(
        value_name = "from",
        required_unless_present = "textconv",
        conflicts_with = "textconv"
    )]
    pub from: Option<String>,
    /// The second side.
    #[arg(
        value_name = "to",
        required_unless_present = "textconv",
        conflicts_with = "textconv"
    )]
    pub to: Option<String>,
    /// Print one project file as text lines (git's textconv) instead.
    #[arg(long, value_name = "file", allow_hyphen_values = true)]
    pub textconv: Option<PathBuf>,
    /// Exit 1 when the sides differ.
    #[arg(long, overrides_with = "exit_code", conflicts_with = "textconv")]
    pub exit_code: bool,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json", conflicts_with = "textconv")]
    pub json: bool,
}

group! {
/// `ew git`.
#[derive(Subcommand, Clone, PartialEq, Eq, Debug)]
pub(crate) enum GitCli {
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
    Setup(GitSetupArgs),
}
}

/// `ew git setup`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct GitSetupArgs {
    /// Write the user's global git configuration instead of the repository's.
    #[arg(long, overrides_with = "global")]
    pub global: bool,
    /// Install the `post-merge` hook that runs `ew check --integrity`.
    #[arg(long, overrides_with = "hooks")]
    pub hooks: bool,
    /// Consent without the question.
    #[arg(long, overrides_with = "yes")]
    pub yes: bool,
}

group! {
/// `ew import`.
#[derive(Subcommand, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Import {
    /// Import a QElectroTech project.
    ///
    /// Exits 0 when the import completes, even with findings; `--strict` exits 1 on lost connections or error
    /// findings. With `--report` and no `--report-out` stdout is exactly the report; the summary goes to stderr.
    /// With `--out <dir>` (absent or empty) the import is written there as a new project, its import run under
    /// `imports/`, after the report; without it nothing is written. Exit 2 when the project cannot be written.
    /// With `--into <project>` the file is re-imported into its earlier import in that project: the changes are
    /// printed and nothing is written; `--apply` commits them as one step and saves. Exit 2 when the file is not an
    /// earlier import of the project. With `--batch <dir> --out <dir>` every `.qet` of the first directory is
    /// imported as a new project `<out>/<name>/` with its migration report `<out>/<name>.report.md`, and the summary
    /// `<out>/summary.md`; a file that cannot be imported is a row of the summary, and `--strict` exits 1 when any
    /// project lost a connection or has an error finding. A `symbol-map.csv` beside the file (columns `element`,
    /// `function`, `cps`, `tag`, `number`, `xref`) binds its element definitions to starter coils, contacts, terminals
    /// and interruption points, keeping their drawing, place and texts; a wrong row exits 2 and imports nothing.
    Qet(ImportQetArgs),
    /// Copy the parts of a parts table into the project.
    ///
    /// Copies the parts of a parts table (the `ew` dialect: `parts.csv`, with `part_cps.csv`,
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
    /// Import tables of the `ew` dialect as a new project.
    ///
    /// Imports tables of the `ew` dialect (the `*.csv` files of `<tables-dir>`: `project.csv`, `devices.csv`,
    /// `conductors.csv` and the dialect's other tables) with the starter content (`--content` or
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
    Table(ImportTableArgs),
    /// Import a DXF drawing as a graphic page, an underlay, symbol drafts or a part outline.
    ///
    /// Reads an ASCII or binary DXF (R12 to 2018). `--as page` (the default) adds the drawing as a new graphic page
    /// of the project (`--project`, default: the nearest ancestor of the current directory holding `manifest.json`)
    /// after its last page, one undo step with its import run, and saves; the findings are printed. `--layout` reads
    /// a paper-space layout instead of the model space; `--show-layer` imports a layer that is off or frozen. A
    /// drawing whose header names no unit needs `--units`. `--as underlay --on <folio>` places it as a locked
    /// underlay on that page, kept outside the project unless `--embed`. `--as symbol` prints one symbol draft per
    /// named block as JSON (`ew symbol publish` saves one into a user library). `--as outline --library <dir>
    /// --part <part-number>` writes its lines as the part's front outline, the part's next patch version. Exit 0 when
    /// imported, 2 on usage, a refused file, a missing unit and open, write and save errors.
    Dxf(super::dxf::ImportDxfArgs),
    /// Place a page of a PDF as a locked underlay.
    ///
    /// Places page `--page` (default 1) of the PDF as a locked underlay over the whole sheet of the page `--on` of
    /// the project, for redrawing: no check, report or export reads it unless an export prints underlays
    /// (`--underlays`). The PDF is kept outside the project unless `--embed` (a customer drawing may be
    /// confidential). Exit 0 when placed and saved, 2 on usage, a file that is no readable PDF or lacks the page, a
    /// page the project does not have, open and save errors.
    Pdf(super::dxf::ImportPdfArgs),
    /// Import a DWG drawing through the ODA File Converter.
    ///
    /// Runs the user's ODA File Converter (`--converter <program>`, else the program `EW_DWG_CONVERTER` names; it is
    /// never bundled or downloaded) on a copy of the DWG in a temporary folder, at most 120 seconds, and imports the
    /// DXF it writes as `ew import dxf` does, with the same options; the import's findings say the drawing was
    /// converted (EW-IMP-257). Without a converter the refusal says how to give one. Exit 0 when imported, 2 on usage,
    /// no converter, a failed or timed-out conversion, and as for `import dxf`.
    Dwg(super::dxf::ImportDwgArgs),
    /// Preview or apply an AutomationML file of PLC software.
    ///
    /// Reads an AML AR APC file (CAEX 2.15 or 3.0, as TIA Portal's CAx export writes it), matches its stations,
    /// modules, channels and tags to the configuration project's (`--config`, default the only one) and previews
    /// every differing field against the last exchange: `apply` (changed in the file, and the PLC or both own it),
    /// `rejected` (changed in the file, the schematic owns it), `kept` and `pending` (changed in the project only),
    /// `conflict` (changed in both); an object without a counterpart is parked, one the file no longer has is
    /// listed. Only the stations in the file are compared; `--complete` compares every station. Nothing is written
    /// without `--accept <choices.json>`, a JSON object that maps a change's key or a parked object's ID to `ours`,
    /// `theirs` or `create` (`{}` takes every default): the import then commits as one undo step with its snapshot,
    /// and the project is saved. Exit 0 with the preview (and once applied and saved), 2 on usage, open, read and
    /// save errors and a refused choice.
    PlcAml(ImportPlcAmlCli),
    /// Preview or apply a TIA Portal "PLC Tags" workbook.
    ///
    /// Reads the sheet `PLC Tags` of a workbook and matches its rows to the configuration project's tags by the
    /// hidden ID column, then by name, then by logical address; previews and applies as `import plc-aml`, for the
    /// names, data types and comments (`--lang` names the language of the comment column). A row without a channel
    /// is listed and never created.
    #[command(alias = "plc-tags")]
    PlcXlsx(ImportPlcXlsxCli),
    /// Take an instrument list into a PLC configuration project.
    ///
    /// Reads a CSV or the first sheet of a workbook with the columns `tag` and `signal type` (DI, DO, AI, AO) and,
    /// when present, `description`, `range`, `field location` and `preferred station`. Each row gets a placeholder
    /// device with the tag as its process tag and the first spare channel of its kind, of the preferred station
    /// first; a row without one is an open item (EW-PRC-002). Prints every row's channel; `--apply` commits them as
    /// one undo step and saves. Exit 0 with the rows, 2 on usage, open, read and save errors.
    InstrumentList(InstrumentListArgs),
    /// Import the parts of an EPLAN EDZ.
    ///
    /// Reads an EDZ (7-Zip or ZIP, told by its content): the `*.part.xml` members into parts, with their
    /// multilingual descriptions, pins, function templates (through the function-definition table, `--fndef-map`
    /// adds your own rows), sizes, free properties and provenance (the file, its SHA-256, the vendor terms). The
    /// EPLAN Data Portal terms notice is printed first. `--library <key|id>` writes the parts into a writable
    /// library (`--bump` writes a changed part as its next patch version, `--take-over` takes a stale library lock
    /// over, `--company` names company libraries, `--as-of` stamps the parts). `--project <dir>` writes nothing into
    /// a library: it previews the update of the project's part copies of the same part numbers, field by field,
    /// and with `--apply` commits it as one undo step and saves; a part the project has no copy of is listed, never
    /// created. Exit 0 when imported or previewed, 2 on usage, a refused file, a refused library write and open and
    /// save errors.
    Edz(super::eplan::ImportEplanPartsArgs),
    /// Import the parts of an EPLAN parts XML.
    ///
    /// As `import edz`, for a `partsmanagement` file (parts, variants, function templates, free properties,
    /// accessory lists; its `length-unit` scales the sizes) or one part XML, without the terms notice. Parts that the
    /// library holds already are updated in place.
    #[command(name = "parts-xml")]
    PartsXml(super::eplan::ImportEplanPartsArgs),
    /// Import an EPLAN report export as a new project without drawings.
    ///
    /// Reads an EPLAN device, terminal-diagram or connection list (XLSX or CSV; the delimiter and the header row are
    /// found) through a column mapping (`--map <file.toml>` of your own, or `--preset <name>` for the shipped German
    /// and English headers) as a new project in `--out` (absent or empty): devices unplaced, strips with their
    /// terminals (rising levels of one designation folded into one multi-level terminal), connections between
    /// terminals; no drawing. A column the mapping does not name is reported. The records of a newer export under the
    /// same `--key` keep their IDs; an existing project is not updated. Exit 0 when written, 2 on usage, a table the mapping does not fit, an `--out` that holds
    /// something and write errors.
    #[command(name = "eplan-report")]
    EplanReport(super::eplan::ImportEplanReportArgs),
}
}

/// `ew import table`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct ImportTableArgs {
    /// The round-trip file: an XLSX workbook or a CSV.
    #[arg(value_name = "file", value_parser = path_word)]
    pub file: PathBuf,
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The table's key (`nav:strips/<uuid>`, `kind:io_list`): needed for a CSV.
    #[arg(
        long,
        value_name = "key",
        allow_hyphen_values = true,
        overrides_with = "table"
    )]
    pub table: Option<String>,
    /// The CSV's preset: excel-nl, excel-en or rfc4180 (default: from its separator).
    #[arg(long, value_name = "preset", allow_hyphen_values = true, overrides_with = "preset", value_parser = import_preset())]
    pub preset: Option<Preset>,
    /// Take the file's value of a cell changed in both.
    #[arg(long, overrides_with = "take_file")]
    pub take_file: bool,
    /// Commit the applied cells and save.
    #[arg(long, overrides_with = "apply")]
    pub apply: bool,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub json: bool,
}

/// `ew check`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct CheckCli {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The check scheme (default: the project's scheme for the mode, else `pre-delivery`).
    #[arg(long, value_name = "name", allow_hyphen_values = true, overrides_with = "scheme", value_parser = scheme_name())]
    pub(super) scheme: Option<String>,
    /// The check mode.
    #[arg(
        long,
        value_name = "mode",
        allow_hyphen_values = true,
        overrides_with = "mode",
        default_value = "release"
    )]
    pub(super) mode: ModeArg,
    /// The output format (default: text).
    #[arg(
        long,
        value_name = "format",
        allow_hyphen_values = true,
        overrides_with = "format"
    )]
    pub(super) format: Option<CheckFormat>,
    /// Print the result as JSON (`--format json`).
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
    /// The language of the messages (a shipped language).
    #[arg(long, value_name = "tag", allow_hyphen_values = true, overrides_with = "lang", value_parser = language(), ignore_case = true, default_value = ew_api::checks::messages::DEFAULT_LANGUAGE)]
    pub(super) lang: String,
    /// The output file (default: stdout).
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    pub(super) out: Option<PathBuf>,
    /// Run only the project-integrity check: the integrity findings of opening the project and the rules EW-CORE-001
    /// to EW-CORE-019, conflict records (EW-CORE-011) and collisions after a merge (EW-CORE-013) among them, in release
    /// mode; exits 1 on any error.
    #[arg(long, overrides_with = "integrity", conflicts_with_all = ["scheme", "mode"])]
    pub(super) integrity: bool,
    /// With `--integrity`: count the project as just merged, so that collisions of numbers report as EW-CORE-013 (the
    /// post-merge hook of `ew git setup --hooks` passes it).
    #[arg(long, overrides_with = "after_merge", requires = "integrity")]
    pub(super) after_merge: bool,
}

/// The mode of `ew check --mode`.
#[derive(ValueEnum, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ModeArg {
    /// The release gate's mode.
    Release,
    /// The online checks' mode.
    Online,
}

impl CheckCli {
    /// The check mode.
    pub(super) fn check_mode(&self) -> CheckMode {
        match self.mode {
            ModeArg::Release => CheckMode::Release,
            ModeArg::Online => CheckMode::Online,
        }
    }

    /// The output format: `--json` is `--format json`, so another format beside it is refused.
    pub(super) fn output_format(&self) -> Result<CheckFormat, clap::Error> {
        let format_name = self.format.map(|f| match f {
            CheckFormat::Text => "text",
            CheckFormat::Markdown => "md",
            CheckFormat::Json => "json",
        });
        json_alias(&["check"], self.json, format_name)?;
        Ok(if self.json {
            CheckFormat::Json
        } else {
            self.format.unwrap_or(CheckFormat::Text)
        })
    }
}

/// `ew sql`: the query, or `dump` and `schema`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
#[command(
    args_conflicts_with_subcommands = true,
    override_usage = "ew sql [OPTIONS] [project-dir] <select>\n       ew sql [OPTIONS] [project-dir] --file <query.sql>\n       ew sql <COMMAND>"
)]
pub(crate) struct SqlCli {
    /// `dump` or `schema`.
    #[command(subcommand)]
    pub(super) sub: Option<SqlSub>,
    /// The query, or with a second positional or `--file` the project directory.
    #[arg(value_name = "project-dir|select", allow_hyphen_values = true, value_parser = sql_word, required_unless_present = "file")]
    pub(super) first: Option<String>,
    /// The query, after the project directory.
    #[arg(value_name = "select", allow_hyphen_values = true, value_parser = sql_word, conflicts_with = "file")]
    pub(super) second: Option<String>,
    /// The project directory (instead of the positional one).
    #[arg(long, value_name = "dir", allow_hyphen_values = true)]
    pub(super) project: Option<PathBuf>,
    /// A file holding the query.
    #[arg(
        long,
        value_name = "query.sql",
        allow_hyphen_values = true,
        overrides_with = "file"
    )]
    pub(super) file: Option<PathBuf>,
    /// The result format: csv (default), json, or xlsx (with `--out`).
    #[arg(long, value_name = "format", allow_hyphen_values = true, overrides_with = "format", value_parser = sql_format())]
    pub(super) format: Option<SqlFormat>,
    /// The CSV options.
    #[command(flatten)]
    pub(super) csv: CsvCli,
    /// Print the result as JSON (`--format json`).
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
    /// The deadline in seconds.
    #[arg(long, value_name = "seconds", allow_hyphen_values = true, overrides_with = "timeout", value_parser = timeout())]
    pub(super) timeout: Option<u64>,
    /// The output file (default: stdout).
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    pub(super) out: Option<PathBuf>,
}

impl SqlCli {
    /// The project directory and the query: one positional is the query; two, the project directory and the query;
    /// with `--file`, the one positional is the project directory.
    pub(super) fn project_and_query(&self) -> Result<(Option<PathBuf>, SqlSource), clap::Error> {
        let (dir, query) = match (&self.file, &self.first, &self.second) {
            (Some(file), dir, _) => (dir.as_ref().map(PathBuf::from), SqlSource::File(file.clone())),
            (None, Some(dir), Some(q)) => (Some(PathBuf::from(dir)), SqlSource::Text(q.clone())),
            (None, Some(q), None) => (None, SqlSource::Text(q.clone())),
            // `required_unless_present = "file"` gives the query.
            (None, None, _) => return Err(usage_error(&["sql"], "`sql` needs a query (or `--file`)")),
        };
        Ok((counted_project(&["sql"], self.project.clone(), dir)?, query))
    }

    /// The result format: `--json` is `--format json`, so another format beside it is refused.
    pub(super) fn result_format(&self) -> Result<SqlFormat, clap::Error> {
        let format_name = self.format.map(|f| match f {
            SqlFormat::Csv => "csv",
            SqlFormat::Json => "json",
            SqlFormat::Xlsx => "xlsx",
        });
        json_alias(&["sql"], self.json, format_name)?;
        Ok(if self.json {
            SqlFormat::Json
        } else {
            self.format.unwrap_or(SqlFormat::Csv)
        })
    }

    /// The CSV options of the result in `format`: a CSV option with another format is refused, and `xlsx` needs
    /// `--out`.
    pub(super) fn csv_options(&self, format: SqlFormat) -> Result<CsvOptions, clap::Error> {
        let csv = self.csv.options(
            &["sql"],
            match format {
                SqlFormat::Csv => "csv",
                SqlFormat::Json => "json",
                SqlFormat::Xlsx => "xlsx",
            },
        )?;
        if format == SqlFormat::Xlsx && self.out.is_none() {
            return Err(usage_error(&["sql"], XLSX_NEEDS_OUT));
        }
        Ok(csv)
    }
}

group! {
/// `ew sql dump|schema`.
#[derive(Subcommand, Clone, PartialEq, Eq, Debug)]
pub(crate) enum SqlSub {
    /// Write the SQL projection as a SQLite database file.
    ///
    /// With one positional it is the output file; with two, the project directory and the output file.
    #[command(override_usage = "ew sql dump [OPTIONS] [project-dir] <out.sqlite>")]
    Dump(SqlDumpCli),
    /// Print the SQL schema (the `api_v1_*` views).
    Schema(SqlSchemaCli),
}
}

/// `ew sql dump`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct SqlDumpCli {
    /// The output file, or with a second positional the project directory.
    #[arg(value_name = "project-dir|out.sqlite")]
    pub(super) first: String,
    /// The output file, after the project directory.
    #[arg(value_name = "out.sqlite")]
    pub(super) second: Option<String>,
    /// The project directory (instead of the positional one).
    #[arg(long, value_name = "dir", allow_hyphen_values = true)]
    pub(super) project: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
}

impl SqlDumpCli {
    /// The project directory and the SQLite file: one positional is the file; two, the project directory and the file.
    pub(super) fn project_and_out(&self) -> Result<(Option<PathBuf>, PathBuf), clap::Error> {
        let (dir, out) = match &self.second {
            Some(out) => (Some(PathBuf::from(&self.first)), out),
            None => (None, &self.first),
        };
        Ok((
            counted_project(&["sql", "dump"], self.project.clone(), dir)?,
            PathBuf::from(out),
        ))
    }
}

/// `ew sql schema`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct SqlSchemaCli {}

group! {
/// `ew export`.
#[derive(Subcommand, Clone, PartialEq, Eq, Debug)]
pub(crate) enum ExportCli {
    /// Write the cable list as CSV or XLSX.
    ///
    /// Writes the rows of the project's cable overview (the `ew` dialect) to stdout or to `--out`. `--block`
    /// names the report block by its ID or its first folio (`908`); without it the block is the project's block of
    /// the kind with the lowest first folio; a block of another kind is refused. `--format xlsx` writes a workbook to
    /// `--out`; `--preset` writes the CSV for Excel (`excel-nl`, `excel-en`) or as RFC 4180 (`rfc4180`), and `--bom`
    /// and `--line-end` override the preset. Exit 0 on success, 2 on everything else (usage, open errors, a project
    /// without the block, an unknown or wrong block).
    Cables(CablesCli),
    /// Write the bill of materials as CSV or XLSX.
    ///
    /// Writes the rows of the project's parts summary (the `ew` dialect) to stdout or to `--out`. `--mode parts`
    /// (alias `component`, the default) lists every component part; `--mode assembly` replaces the parts a
    /// configured product covers by its row. `--block` names the report block by its ID or its first folio;
    /// without it the block is the project's block of the kind with the lowest first folio. `--format`, `--preset`,
    /// `--bom` and `--line-end` as for `export cables`. Exit 0 on success, 2 on everything else.
    Bom(BomCli),
    /// Write the wire list as CSV or XLSX.
    ///
    /// Writes the rows of the project's wire list (the `ew` dialect) to stdout or to `--out`. `--block` names the
    /// report block by its ID or its first folio; without it the block is the project's block of the kind with the
    /// lowest first folio. `--format`, `--preset`, `--bom` and `--line-end` as for `export cables`. Exit 0 on success,
    /// 2 on everything else.
    Wires(WiresCli),
    /// Write the terminal diagrams as CSV or XLSX.
    ///
    /// Writes the rows of the project's terminal diagram blocks (the `ew` dialect) to stdout or to `--out`.
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
    Pdf(PdfCli),
    /// Write one SVG file per page.
    ///
    /// Writes one `<folio>.svg` per page into the directory `--out` (empty, or one an earlier SVG export wrote, which
    /// is replaced as a whole without a question; any other is refused) through the session's export job with the bundled fonts only. `--pages`, `--sections` and `--threads`
    /// as for `export pdf`. Exit 0 on success, 2 on everything else.
    Svg(SvgCli),
    /// Write one DXF file per page.
    ///
    /// Writes one `<folio>.dxf` per page (AutoCAD 2010, AC1024, in millimetres) and the title block's PNG or JPEG
    /// picture files they name into the directory `--out` (empty, or one an earlier DXF export wrote, which is
    /// replaced as a whole without a question; any other is refused): the frame, symbols as blocks named in words and inserts with the device's texts as attributes (`TAG`,
    /// `FUNCTION_TEXT`, `CROSS_REFERENCE` …), wires, texts, graphics and tables on their layers (`--layer` renames
    /// one), a cross-reference with a hyperlink to its target's page file (`--dwg-links`: its `.dwg`), curves
    /// flattened to 0.01 mm. `--pages` and `--sections` as for `export pdf`. Exit 0 on success, 2 on everything
    /// else.
    Dxf(DxfCli),
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
    /// Write the rework list since a release as CSV or XLSX.
    ///
    /// Compares the project as it is with the as-built archive of a release (doc 13 §3.5) and writes what the fitter
    /// changes in the built cabinet, one row per thing he does (Pull, Replace, Move end, Check length, Add), to `--out`
    /// (required). `--since <revision>` names the release by its label; without it the last release whose archive is
    /// found beside the project (`<name>.ew-archive`). `--archive <file>` compares with an archive file instead.
    /// `--format xlsx` writes a workbook of one sheet; the CSV is the `ew` dialect. Exit 0 when written, 2 on
    /// everything else (usage, open errors, no release or no archive found, an archive that does not open, write
    /// errors).
    Rework(ReworkCli),
    /// Write the label files, one per label set and marker part.
    ///
    /// Writes the project's labels (doc 13 §5.4) into the folder `--dir` (required, made when missing): one file per
    /// label set and marker part, named by the drawing's number, the set and the marker part's number, in the columns
    /// of the project's export profile (else the generic one: Text 1, Text 2, Text 3, Copies, For), each written whole.
    /// `--since <revision>` writes only the labels the rework list against that release names. `--format xlsx` writes
    /// workbooks of one sheet; the CSV has a byte-order mark, CR LF and the profile's delimiter, its values as they
    /// are. Exit 0 when written, 2 on everything else (usage, open errors, no such release or its archive not found,
    /// write errors).
    Labels(LabelsCli),
    /// Write the drawings for drilling: per surface with holes a template and a DXF.
    ///
    /// Writes into the folder `--dir` (required, made when missing), per surface with holes (doc 13 §9.1), its
    /// template to print at full size (`<location> <surface> template.pdf`, A4 unless `--template a3`) and its DXF for a
    /// machine (`<location> <surface>.dxf`, origin at the surface's datum corner, a layer per way of making a hole),
    /// each written whole, in the project's language. `--surface` (repeatable) takes surfaces by their words ("Door")
    /// or their ID; without it every surface with holes. `--no-template` and `--no-dxf` leave one kind out. Exit 0 when
    /// written, 2 on everything else (usage, open errors, no surface with holes, an unknown surface, write errors).
    Drilling(DrillingCli),
    /// Write the package for the workshop: one ZIP with the drawings, the lists, the label files and the drawings for
    /// drilling.
    ///
    /// Writes the package of the project as it is saved (doc 13-package §8) to `--out` (required), whole or not at all:
    /// the drawings as one PDF (a draft's pages marked "Draft V1.0: not released"), each workshop list as its Excel
    /// export, the label files in the label file's form and the drawings for drilling, in folders by kind, with
    /// `Contents.txt` and `manifest.json`. The revision decides draft or release: a draft while a revision is open.
    /// `--leave-out <member>` (repeatable) leaves out a member by its kind (`pages`, `wire_list`, `labels`, `drilling`,
    /// …); `--omit-personal-data` leaves out the name and the address of each party marked as personal data. Prints each
    /// finding of the release check, then what it wrote. Exit 0 when written, 2 on everything else (usage, open errors,
    /// a release package with an open error of the release check, which it names, write errors).
    Package(PackageCli),
    /// Write one PNG file per page.
    ///
    /// Writes one `<folio>.png` per page into the directory `--out` (empty, or one an earlier PNG export wrote, which
    /// is replaced as a whole without a question; any other is refused), rendered on the CPU with the bundled fonts at `--dpi` (72 to 600, default 150).
    /// `--pages` and `--sections` as for `export pdf`. Exit 0 on success, 2 on everything else (usage,
    /// open errors, an unknown folio or section, write errors).
    Png(PngCli),
    /// Write a PLC configuration project as AutomationML for PLC software.
    ///
    /// Writes the configuration project (`--config`, default the project's only one) as AutomationML AR APC on
    /// CAEX 2.15 to `--out` (required): its stations with their modules, I/O ranges and channels, the tags with
    /// their links to the channels, and the bus interfaces. `--apc 1.2.0` writes the older version; `--station`
    /// (repeatable) exports those stations only. The file carries the project's own IDs and IEC designations, so a
    /// file that comes back is matched again; its dates are `SOURCE_DATE_EPOCH`, else the project's edited date.
    /// The export is recorded in the project as the base of the next import's merge, and the project is saved. A
    /// symbol that is no IEC identifier blocks the export (EW-PLC-005). Exit 0 when written and recorded, 2 on
    /// everything else.
    PlcAml(PlcAmlCli),
    /// Write the tags of a PLC configuration project as a TIA Portal "PLC Tags" workbook.
    ///
    /// Writes the sheet `PLC Tags` with the columns Name, Path, Data Type, Logical Address, Comment, Hmi Visible,
    /// Hmi Accessible, Hmi Writeable, Typeobject ID and Version ID to `--out` (required): one row per channel with
    /// a symbol and an address (`%Q0.1`), the comment in `--lang`. Recorded and saved as `export plc-aml`, for the
    /// tags' fields it carries only; warns while no PLC import has confirmed the addresses.
    #[command(alias = "plc-tags")]
    PlcXlsx(PlcXlsxCli),
    /// Write the tags of a PLC configuration project as a PLCopen XML global variable list.
    ///
    /// Writes PLCopen XML (TC6 v2.01) with the global variable list `IO_ElektraWatt` to `--out` (required): one
    /// variable per channel with a symbol and an address, the address in IEC notation (`%QX0.1`), the function
    /// text and the signal pin as its documentation. Not recorded: the list never comes back, so it is no base of
    /// an import. A symbol that is no IEC identifier blocks the export (EW-PLC-005).
    Plcopen(PlcopenCli),
}
}

/// `ew export plc-aml`, `plc-xlsx` and `plcopen`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct PlcExportCli {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The configuration project by name (default: the project's only one).
    #[arg(long, value_name = "name", allow_hyphen_values = true, overrides_with = "config", value_parser = word)]
    pub(super) config: Option<String>,
    /// The AR APC version of `plc-aml`: 1.4.0 (default) or 1.2.0.
    #[arg(long, value_name = "version", overrides_with = "apc", value_parser = apc_version())]
    pub(super) apc: Option<ew_api::plc_exchange::ApcVersion>,
    /// A station to export, by its tag (repeatable; default: every station of the configuration project).
    #[arg(long, value_name = "tag", allow_hyphen_values = true, value_parser = word)]
    pub(super) station: Vec<String>,
    /// The language of comments (`plc-xlsx`) and of the documentation (`plcopen`), as a language tag.
    #[arg(long, value_name = "tag", overrides_with = "lang", value_parser = word)]
    pub(super) lang: Option<String>,
    /// Leave person fields out; a PLC exchange file holds none either way.
    #[arg(long, overrides_with = "omit_personal_data")]
    pub(super) omit_personal_data: bool,
    /// The file to write (required).
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    pub(super) out: Option<PathBuf>,
}

/// `ew import plc-aml` and `plc-xlsx`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct PlcImportCli {
    /// The exchange file: an AML file or a tag workbook.
    #[arg(value_name = "file", value_parser = path_word)]
    pub(super) file: PathBuf,
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The configuration project by name (default: the project's only one).
    #[arg(long, value_name = "name", allow_hyphen_values = true, overrides_with = "config", value_parser = word)]
    pub(super) config: Option<String>,
    /// The language of a tag workbook's comment column, as a language tag.
    #[arg(long, value_name = "tag", overrides_with = "lang", value_parser = word)]
    pub(super) lang: Option<String>,
    /// The file is complete: a station it does not hold was removed in the PLC software.
    #[arg(long, overrides_with = "complete")]
    pub(super) complete: bool,
    /// Print the preview and write nothing (the default without `--accept`).
    #[arg(long, overrides_with = "preview", conflicts_with = "accept")]
    pub(super) preview: bool,
    /// Apply the import with these choices: a JSON object from a change's key or a parked object's ID to `ours`,
    /// `theirs` or `create` (`{}` takes every default).
    #[arg(
        long,
        value_name = "choices.json",
        allow_hyphen_values = true,
        overrides_with = "accept"
    )]
    pub(super) accept: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
}

/// `ew import instrument-list`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct InstrumentListArgs {
    /// The instrument list: a CSV or a workbook.
    #[arg(value_name = "file", value_parser = path_word)]
    pub file: PathBuf,
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The configuration project by name (default: the project's only one).
    #[arg(long, value_name = "name", allow_hyphen_values = true, overrides_with = "config", value_parser = word)]
    pub config: Option<String>,
    /// Commit the rows and save.
    #[arg(long, overrides_with = "apply")]
    pub apply: bool,
}

group! {
/// `ew plc`.
#[derive(Subcommand, Clone, PartialEq, Eq, Debug)]
pub(crate) enum PlcCli {
    /// Report the exchange status of a PLC configuration project.
    ///
    /// Prints the configuration project's last exchange (direction, format, date), every object of the project that
    /// differs from it (the drift EW-PLC-016 reports) and, with `<file>`, whether that file is the one last exchanged.
    /// The project opens as a reader. Exit 0 with the status, 2 on usage and open errors.
    Status(PlcStatusArgs),
}
}

/// `ew plc status`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct PlcStatusArgs {
    /// An exchange file to compare with the last exchange.
    #[arg(value_name = "file", value_parser = path_word)]
    pub file: Option<PathBuf>,
    /// The project directory.
    #[arg(long, value_name = "dir", allow_hyphen_values = true)]
    pub project: Option<PathBuf>,
    /// The configuration project by name (default: the project's only one).
    #[arg(long, value_name = "name", allow_hyphen_values = true, overrides_with = "config", value_parser = word)]
    pub config: Option<String>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub json: bool,
}

/// An `export plc-aml --apc`.
fn apc_version() -> impl TypedValueParser<Value = ew_api::plc_exchange::ApcVersion> {
    one_of(
        ew_api::plc_exchange::ApcVersion::all()
            .iter()
            .map(|(v, text)| (PossibleValue::new(*text), *v))
            .collect(),
    )
}

/// The refusal of a PLC export without `--out`.
pub(super) const PLC_NEEDS_OUT: &str = "a PLC export writes a file: give `--out <file>`";

/// `ew export plc-aml`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct PlcAmlCli {
    /// The export's arguments.
    #[command(flatten)]
    pub(super) plc: PlcExportCli,
}

/// `ew export plc-xlsx`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct PlcXlsxCli {
    /// The export's arguments.
    #[command(flatten)]
    pub(super) plc: PlcExportCli,
}

/// `ew export plcopen`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct PlcopenCli {
    /// The export's arguments.
    #[command(flatten)]
    pub(super) plc: PlcExportCli,
}

impl PlcExportCli {
    /// The file format of `export <word>` (`plc-aml`, `plc-xlsx`, `plcopen`) and the file it writes: `--apc` is an
    /// option of `plc-aml` alone, and `--out` is needed.
    pub(super) fn format_and_out(
        &self,
        word: &str,
    ) -> Result<(ew_api::plc_exchange::PlcFormat, &PathBuf), clap::Error> {
        use ew_api::plc_exchange::PlcFormat;
        let format = match word {
            "plc-aml" => PlcFormat::Aml(self.apc.unwrap_or_default()),
            _ if self.apc.is_some() => {
                return Err(usage_error(
                    &["export", word],
                    "`--apc` is an option of `export plc-aml`",
                ));
            }
            "plc-xlsx" => PlcFormat::TiaXlsx,
            _ => PlcFormat::Plcopen,
        };
        let Some(out) = &self.out else {
            return Err(usage_error(&["export", word], PLC_NEEDS_OUT));
        };
        Ok((format, out))
    }
}

/// `ew import plc-aml`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct ImportPlcAmlCli {
    /// The import's arguments.
    #[command(flatten)]
    pub(super) plc: PlcImportCli,
}

/// `ew import plc-xlsx`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct ImportPlcXlsxCli {
    /// The import's arguments.
    #[command(flatten)]
    pub(super) plc: PlcImportCli,
}

/// `ew export ewz`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct EwzCli {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The .ewz file to write (required).
    #[arg(
        long,
        value_name = "file.ewz",
        allow_hyphen_values = true,
        overrides_with = "out"
    )]
    pub(super) out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
}

impl EwzCli {
    /// The `.ewz` file to write: `--out` is needed.
    pub(super) fn file(&self) -> Result<&PathBuf, clap::Error> {
        self.out
            .as_ref()
            .ok_or_else(|| usage_error(&["export", "ewz"], EWZ_NEEDS_OUT))
    }
}

/// `ew extract`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct ExtractArgs {
    /// The .ewz file.
    #[arg(value_name = "file.ewz")]
    pub file: PathBuf,
    /// The new project folder (default: `<name>.ew` beside the archive); it must not exist.
    #[arg(long, value_name = "dir", allow_hyphen_values = true, overrides_with = "into")]
    pub into: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub json: bool,
}

/// `ew export cables`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct CablesCli {
    /// The export's arguments.
    #[command(flatten)]
    pub(super) rows: RowsCli,
}

/// `ew export wires`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct WiresCli {
    /// The export's arguments.
    #[command(flatten)]
    pub(super) rows: RowsCli,
}

impl RowsCli {
    /// The row export `export <what>` of `kind` (`cables`, `wires`).
    pub(super) fn export(&self, what: &str, kind: ExportKind) -> Result<ExportArgs, clap::Error> {
        csv_only(what, self.format)?;
        let format = rows_format(&["export", what], self.format, &self.csv, self.out.as_ref())?;
        Ok(ExportArgs {
            kind,
            block: self.block.clone(),
            project: self.project.0.clone(),
            out: self.out.clone(),
            json: self.json,
            format,
        })
    }
}

/// `ew export cables|wires`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct RowsCli {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The report block, by its ID or its first folio.
    #[arg(
        long,
        value_name = "block",
        allow_hyphen_values = true,
        overrides_with = "block"
    )]
    pub(super) block: Option<String>,
    /// The format: csv (default) or xlsx (with `--out`).
    #[arg(long, value_name = "format", allow_hyphen_values = true, overrides_with = "format", value_parser = row_format())]
    pub(super) format: Option<ReportPages>,
    /// The CSV options.
    #[command(flatten)]
    pub(super) csv: CsvCli,
    /// The output file (default: stdout).
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    pub(super) out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
}

/// `ew export bom`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct BomCli {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The report block, by its ID or its first folio.
    #[arg(
        long,
        value_name = "block",
        allow_hyphen_values = true,
        overrides_with = "block"
    )]
    pub(super) block: Option<String>,
    /// Every component part, or the configured products' rows in place of the parts they cover.
    #[arg(
        long,
        value_name = "mode",
        allow_hyphen_values = true,
        overrides_with = "mode",
        default_value = "parts"
    )]
    pub(super) mode: BomArg,
    /// The format: csv (default) or xlsx (with `--out`).
    #[arg(long, value_name = "format", allow_hyphen_values = true, overrides_with = "format", value_parser = row_format())]
    pub(super) format: Option<ReportPages>,
    /// The CSV options.
    #[command(flatten)]
    pub(super) csv: CsvCli,
    /// The output file (default: stdout).
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    pub(super) out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
}

impl BomCli {
    /// The row export `export bom` in its mode.
    pub(super) fn export(&self) -> Result<ExportArgs, clap::Error> {
        csv_only("bom", self.format)?;
        let format = rows_format(&["export", "bom"], self.format, &self.csv, self.out.as_ref())?;
        let mode = match self.mode {
            BomArg::Parts => BomMode::Component,
            BomArg::Assembly => BomMode::Assembly,
        };
        Ok(ExportArgs {
            kind: ExportKind::Bom(mode),
            block: self.block.clone(),
            project: self.project.0.clone(),
            out: self.out.clone(),
            json: self.json,
            format,
        })
    }
}

/// The mode of `ew export bom --mode`.
#[derive(ValueEnum, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BomArg {
    /// Every component part.
    #[value(alias = "component")]
    Parts,
    /// The configured products' rows in place of the parts they cover.
    Assembly,
}

/// `ew export terminals`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct TerminalsCli {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The report block, by its ID or its first folio (default: every terminal diagram block).
    #[arg(
        long,
        value_name = "block",
        allow_hyphen_values = true,
        overrides_with = "block"
    )]
    pub(super) block: Option<String>,
    /// One strip, by its tag (`-X1031`).
    #[arg(
        long,
        value_name = "tag",
        allow_hyphen_values = true,
        overrides_with = "strip"
    )]
    pub(super) strip: Option<String>,
    /// The format: csv (default) or xlsx (with `--out`).
    #[arg(long, value_name = "format", allow_hyphen_values = true, overrides_with = "format", value_parser = row_format())]
    pub(super) format: Option<ReportPages>,
    /// The CSV options.
    #[command(flatten)]
    pub(super) csv: CsvCli,
    /// The output file (default: stdout).
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    pub(super) out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
}

impl TerminalsCli {
    /// The row export `export terminals` of one strip or every strip.
    pub(super) fn export(&self) -> Result<ExportArgs, clap::Error> {
        csv_only("terminals", self.format)?;
        let format = rows_format(
            &["export", "terminals"],
            self.format,
            &self.csv,
            self.out.as_ref(),
        )?;
        Ok(ExportArgs {
            kind: ExportKind::Terminals(self.strip.clone()),
            block: self.block.clone(),
            project: self.project.0.clone(),
            out: self.out.clone(),
            json: self.json,
            format,
        })
    }
}

/// `ew export report`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
// The group allows both so that `ReportCli::export` refuses the pair in its own words.
#[command(group(ArgGroup::new("rows").args(["block", "kind"]).required(true).multiple(true)))]
pub(crate) struct ReportCli {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The report block, by its ID or its first folio.
    #[arg(
        long,
        value_name = "block",
        allow_hyphen_values = true,
        overrides_with = "block"
    )]
    pub(super) block: Option<String>,
    /// A report kind: its whole-project rows (CSV or XLSX, no block).
    #[arg(
        long,
        value_name = "kind",
        allow_hyphen_values = true,
        overrides_with = "kind"
    )]
    pub(super) kind: Option<String>,
    /// The format: csv (default), xlsx (with `--out`) or pdf (a block's pages, with `--out`).
    #[arg(long, value_name = "format", allow_hyphen_values = true, overrides_with = "format", value_parser = report_format())]
    pub(super) format: Option<ReportPages>,
    /// The CSV options.
    #[command(flatten)]
    pub(super) csv: CsvCli,
    /// The output file (default: stdout).
    #[arg(
        long,
        value_name = "file",
        allow_hyphen_values = true,
        overrides_with = "out",
        required_if_eq("format", "pdf")
    )]
    pub(super) out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
}

impl ReportCli {
    /// The rows of a block, its pages (`--format pdf`), or the whole-project rows of a report kind: a kind takes
    /// neither a block nor pages, refused in the report layer's words, the pair first.
    pub(super) fn export(&self) -> Result<ExportArgs, clap::Error> {
        let pdf = self.format == Some(ReportPages::Pdf);
        let kind = match &self.kind {
            Some(name) => {
                if self.block.is_some() {
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
                ExportKind::ReportKind(name.clone())
            }
            None => ExportKind::Report { pdf },
        };
        let format = rows_format(&["export", "report"], self.format, &self.csv, self.out.as_ref())?;
        Ok(ExportArgs {
            kind,
            block: self.block.clone(),
            project: self.project.0.clone(),
            out: self.out.clone(),
            json: self.json,
            format,
        })
    }
}

/// `ew export table`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct TableCli {
    /// The navigator.
    #[arg(value_name = "navigator", value_parser = word)]
    pub(super) navigator: String,
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// One strip of `export table strips`, by its tag (`-X1031`).
    #[arg(
        long,
        value_name = "tag",
        allow_hyphen_values = true,
        overrides_with = "strip"
    )]
    pub(super) strip: Option<String>,
    /// The format: csv (default) or xlsx (with `--out`).
    #[arg(long, value_name = "format", allow_hyphen_values = true, overrides_with = "format", value_parser = table_format())]
    pub(super) format: Option<ReportPages>,
    /// The CSV options.
    #[command(flatten)]
    pub(super) csv: CsvCli,
    /// The output file (default: stdout).
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    pub(super) out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
}

impl TableCli {
    /// How the table is written: the navigator and the strip are refused first, in the round trip's words.
    pub(super) fn row_format(&self) -> Result<RowFormat, clap::Error> {
        let path = &["export", "table"];
        if ew_api::output::NavKind::from_word(&self.navigator)
            .is_none_or(|k| !ew_api::output::TABLE_NAVIGATORS.contains(&k))
        {
            return Err(usage_error(
                path,
                ew_api::output::TableError::UnknownNavigator(self.navigator.clone()),
            ));
        }
        if self.strip.is_some() && self.navigator != "strips" {
            return Err(usage_error(path, ew_api::output::TableError::StripElsewhere));
        }
        rows_format(path, self.format, &self.csv, self.out.as_ref())
    }
}

/// What `ew export report --format` writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ReportPages {
    /// Rows as CSV.
    Csv,
    /// Rows as an XLSX workbook.
    Xlsx,
    /// A block's pages as PDF.
    Pdf,
}

/// The CSV options of a row export and of `sql`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct CsvCli {
    /// The CSV preset: ew (the default), excel-nl, excel-en or rfc4180.
    #[arg(long, value_name = "preset", allow_hyphen_values = true, overrides_with = "preset", value_parser = preset())]
    pub(super) preset: Option<Preset>,
    /// A UTF-8 byte-order mark: on or off (default: the preset's).
    #[arg(long, value_name = "on|off", allow_hyphen_values = true, overrides_with = "bom", value_parser = bom())]
    pub(super) bom: Option<bool>,
    /// The line end: crlf or lf (default: the preset's).
    #[arg(long, value_name = "crlf|lf", allow_hyphen_values = true, overrides_with = "line_end", value_parser = line_end())]
    pub(super) line_end: Option<LineEnd>,
}

impl CsvCli {
    /// The options, or the refusal of a CSV option given with another format (`format`: its name).
    pub(super) fn options(&self, path: &[&str], format: &str) -> Result<CsvOptions, clap::Error> {
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

/// The format of a row export: `xlsx` takes no CSV option and needs `--out`; `pdf` takes no CSV option.
fn rows_format(
    path: &[&str],
    format: Option<ReportPages>,
    csv: &CsvCli,
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

/// `ew export dxf`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct DxfCli {
    /// The pages, as for `export svg`.
    #[command(flatten)]
    pub(super) pages: PagesCli,
    /// Name the DXF layer of a class (frame, symbols, wires, texts, graphics, tables, underlay): `wires=WIRING`
    /// (repeatable; default `EW-FRAME`, `EW-SYMBOLS`, `EW-WIRES`, `EW-TEXTS`, `EW-GRAPHICS`, `EW-TABLES`,
    /// `EW-UNDERLAY`).
    #[arg(long, value_name = "class=name", allow_hyphen_values = true, value_parser = dxf_layer)]
    pub(super) layer: Vec<(String, String)>,
    /// Let the hyperlinks of the cross-references name the pages' `.dwg` files, for a set a converter turns into DWG
    /// (default: their `.dxf` files).
    #[arg(long)]
    pub(super) dwg_links: bool,
}

impl DxfCli {
    /// The page export `export dxf` with its layer names and links.
    pub(super) fn export(&self) -> Result<ExportPagesArgs, clap::Error> {
        let mut a = self.pages.export(PageFormat::Dxf)?;
        for (class, name) in &self.layer {
            a.dxf_layers
                .set(class, name)
                .map_err(|e| usage_error(&["export", "dxf"], e))?;
        }
        a.dxf_dwg_links = self.dwg_links;
        Ok(a)
    }
}

/// `ew export pdf`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct PdfCli {
    /// The export's arguments.
    #[command(flatten)]
    pub(super) pages: PagesCli,
}

/// `ew export svg`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct SvgCli {
    /// The export's arguments.
    #[command(flatten)]
    pub(super) pages: PagesCli,
}

impl PagesCli {
    /// The page export in `format` (`export pdf|svg|dxf`): the flags not built yet are refused, and the selection is
    /// `--pages` or `--sections`.
    pub(super) fn export(&self, format: PageFormat) -> Result<ExportPagesArgs, clap::Error> {
        let path: &[&str] = match format {
            PageFormat::Pdf => &["export", "pdf"],
            PageFormat::Svg | PageFormat::Png => &["export", "svg"],
            PageFormat::Dxf => &["export", "dxf"],
        };
        not_built(path, self.lang.is_some(), self.omit_personal_data)?;
        Ok(ExportPagesArgs {
            format,
            project: self.project.0.clone(),
            out: self.out.clone(),
            pages: selection(path, self.pages.clone(), self.sections.clone())?,
            threads: self.threads,
            dpi: ew_api::output::DEFAULT_DPI,
            dxf_layers: ew_api::output::DxfLayers::default(),
            dxf_dwg_links: false,
            underlays: self.underlays,
            json: self.json,
        })
    }
}

/// `ew export pdf|svg`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct PagesCli {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The PDF file, or the directory of the SVG files.
    #[arg(long, value_name = "path", allow_hyphen_values = true, overrides_with = "out")]
    pub(super) out: PathBuf,
    /// The folio labels and ranges, in page order (`101-106,901`; default: every page).
    #[arg(long, value_name = "folios", allow_hyphen_values = true, overrides_with = "pages", value_parser = pages)]
    pub(super) pages: Option<PageSelection>,
    /// The section numbers, each section's pages with its divider (`1,2`; instead of `--pages`).
    #[arg(long, value_name = "numbers", allow_hyphen_values = true, overrides_with = "sections", value_parser = sections)]
    pub(super) sections: Option<PageSelection>,
    /// The threads of the page phase (1 to 256; default: the machine's parallelism).
    #[arg(long, value_name = "n", allow_hyphen_values = true, overrides_with = "threads", value_parser = threads())]
    pub(super) threads: Option<usize>,
    /// Print the pages' underlays under them (a PDF page or a DXF drawing placed for redrawing; not printed by
    /// default).
    #[arg(long, overrides_with = "underlays")]
    pub(super) underlays: bool,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
    /// Not built yet: refused.
    #[arg(long, hide = true, num_args = 0..=1, overrides_with = "lang")]
    pub(super) lang: Option<Option<String>>,
    /// Not built yet: refused.
    #[arg(long, hide = true, overrides_with = "omit_personal_data")]
    pub(super) omit_personal_data: bool,
}

/// `ew print`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct PrintCli {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The folio labels and ranges, in page order (`101-106,901`; default: every page).
    #[arg(long, value_name = "folios", allow_hyphen_values = true, overrides_with = "pages", value_parser = pages)]
    pub(super) pages: Option<PageSelection>,
    /// The section numbers, each section's pages with its divider (`1,2`; instead of `--pages`).
    #[arg(long, value_name = "numbers", allow_hyphen_values = true, overrides_with = "sections", value_parser = sections)]
    pub(super) sections: Option<PageSelection>,
    /// The paper (sheet, A5, A4, A3, A2, A1, A0, Letter, Legal, Tabloid; default: each page's sheet).
    #[arg(long, value_name = "size", allow_hyphen_values = true, overrides_with = "paper", value_parser = paper)]
    pub(super) paper: Option<Paper>,
    /// Scale each page to the whole paper, up or down, and stamp a scaled page.
    #[arg(long, overrides_with = "fit")]
    pub(super) fit: bool,
    /// Print in black and grey.
    #[arg(long, overrides_with = "mono")]
    pub(super) mono: bool,
    /// The printer (with `--via lp` or `--via windows`; default: `lp`'s default printer, or the one chosen in the
    /// Windows print dialog).
    #[arg(
        long,
        value_name = "name",
        allow_hyphen_values = true,
        overrides_with = "printer",
        conflicts_with = "out"
    )]
    pub(super) printer: Option<String>,
    /// How the set reaches the printer (default: `lp` on Linux, `windows` on Windows, else `viewer`).
    #[arg(
        long,
        value_name = "adapter",
        allow_hyphen_values = true,
        overrides_with = "via",
        conflicts_with = "out"
    )]
    pub(super) via: Option<ViaArg>,
    /// Write the print set to this PDF file instead of printing it.
    #[arg(long, value_name = "file", allow_hyphen_values = true, overrides_with = "out")]
    pub(super) out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
}

impl PrintCli {
    /// The page selection: `--pages` or `--sections`, not both; neither is every page.
    pub(super) fn selection(&self) -> Result<PageSelection, clap::Error> {
        selection(&["print"], self.pages.clone(), self.sections.clone())
    }

    /// The adapter, its platform default resolved: `windows` prints on Windows only, and the viewer takes no
    /// `--printer`.
    pub(super) fn adapter(&self) -> Result<PrintVia, clap::Error> {
        let via = match self.via {
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
        if via == PrintVia::Viewer && self.printer.is_some() {
            return Err(usage_error(&["print"], ew_print::PrintError::PrinterWithViewer));
        }
        Ok(via)
    }
}

/// The adapter of `ew print --via`.
#[derive(ValueEnum, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ViaArg {
    /// The CUPS command `lp`.
    Lp,
    /// The default PDF application.
    Viewer,
    /// The Windows print dialog, or the printer `--printer` names (Windows only).
    Windows,
}

/// `ew export png`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct PngCli {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The directory of the PNG files.
    #[arg(long, value_name = "dir", allow_hyphen_values = true, overrides_with = "out")]
    pub(super) out: PathBuf,
    /// The folio labels and ranges, in page order (`101-106,901`; default: every page).
    #[arg(long, value_name = "folios", allow_hyphen_values = true, overrides_with = "pages", value_parser = pages)]
    pub(super) pages: Option<PageSelection>,
    /// The section numbers, each section's pages with its divider (`1,2`; instead of `--pages`).
    #[arg(long, value_name = "numbers", allow_hyphen_values = true, overrides_with = "sections", value_parser = sections)]
    pub(super) sections: Option<PageSelection>,
    /// The resolution (72 to 600).
    #[arg(long, value_name = "n", allow_hyphen_values = true, overrides_with = "dpi", value_parser = dpi(), default_value_t = ew_api::output::DEFAULT_DPI)]
    pub(super) dpi: u32,
    /// The threads of the page phase (1 to 256; default: the machine's parallelism).
    #[arg(long, value_name = "n", allow_hyphen_values = true, overrides_with = "threads", value_parser = threads())]
    pub(super) threads: Option<usize>,
    /// Print the pages' underlays under them (a PDF page or a DXF drawing placed for redrawing; not printed by
    /// default).
    #[arg(long, overrides_with = "underlays")]
    pub(super) underlays: bool,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
    /// Not built yet: refused.
    #[arg(long, hide = true, num_args = 0..=1, overrides_with = "lang")]
    pub(super) lang: Option<Option<String>>,
    /// Not built yet: refused.
    #[arg(long, hide = true, overrides_with = "omit_personal_data")]
    pub(super) omit_personal_data: bool,
}

impl PngCli {
    /// The page export `export png` at its resolution.
    pub(super) fn export(&self) -> Result<ExportPagesArgs, clap::Error> {
        not_built(&["export", "png"], self.lang.is_some(), self.omit_personal_data)?;
        Ok(ExportPagesArgs {
            format: PageFormat::Png,
            project: self.project.0.clone(),
            out: self.out.clone(),
            pages: selection(&["export", "png"], self.pages.clone(), self.sections.clone())?,
            threads: self.threads,
            dpi: self.dpi,
            dxf_layers: ew_api::output::DxfLayers::default(),
            dxf_dwg_links: false,
            underlays: self.underlays,
            json: self.json,
        })
    }
}

/// `ew render`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct RenderArgs {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The page, by its folio label.
    #[arg(
        long,
        value_name = "folio",
        allow_hyphen_values = true,
        overrides_with = "page"
    )]
    pub page: String,
    /// The page format (PNG is the only one).
    #[arg(long, overrides_with = "png", required = true)]
    pub png: bool,
    /// The resolution (72 to 600).
    #[arg(long, value_name = "n", allow_hyphen_values = true, overrides_with = "dpi", value_parser = dpi(), default_value_t = ew_api::output::DEFAULT_DPI)]
    pub dpi: u32,
    /// The PNG file (default: `<folio>.png` in the current directory).
    #[arg(
        long,
        value_name = "file.png",
        allow_hyphen_values = true,
        overrides_with = "out"
    )]
    pub out: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub json: bool,
}

/// `ew apply`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct ApplyCli {
    /// The command log, or with a second positional the project directory.
    #[arg(value_name = "project-dir|file.ewcmd")]
    pub(super) first: PathBuf,
    /// The command log, after the project directory.
    #[arg(value_name = "file.ewcmd")]
    pub(super) second: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(long, value_name = "dir", allow_hyphen_values = true)]
    pub(super) project: Option<PathBuf>,
    /// Replay on a copy in memory, print the changes and save nothing.
    #[arg(long, overrides_with = "preview")]
    pub(super) preview: bool,
    /// Save only when every line commits.
    #[arg(long, overrides_with = "one_step")]
    pub(super) one_step: bool,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
}

impl ApplyCli {
    /// The project directory and the command log: one positional is the log; two, the project directory and the
    /// log.
    pub(super) fn project_and_log(&self) -> Result<(Option<PathBuf>, &PathBuf), clap::Error> {
        let (dir, log) = match &self.second {
            Some(log) => (Some(self.first.clone()), log),
            None => (None, &self.first),
        };
        Ok((counted_project(&["apply"], self.project.clone(), dir)?, log))
    }
}

/// `ew fmt`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct FmtArgs {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// List the files a format would rewrite and write nothing.
    #[arg(long, overrides_with = "check")]
    pub check: bool,
    /// Discard the lines that are not JSON or not UTF-8, after an `.ewz` backup beside the project folder.
    #[arg(long, overrides_with = "repair", conflicts_with = "check")]
    pub repair: bool,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub json: bool,
}

/// `ew upgrade`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct UpgradeArgs {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// Say whether the project needs an upgrade and write nothing.
    #[arg(long, overrides_with = "check")]
    pub check: bool,
    /// Confirm the upgrade without typing `upgrade` (the backup keeps the project as it was).
    #[arg(long, overrides_with = "yes", conflicts_with = "check")]
    pub yes: bool,
}

/// `ew update-copies`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct UpdateCopiesArgs {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// A company library: a library directory or a directory of `*.ewlib` libraries (repeatable).
    #[arg(long, value_name = "dir", allow_hyphen_values = true)]
    pub company: Vec<PathBuf>,
}

/// `ew recover`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct RecoverCli {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// Replay the journal's transactions on the saved project and save.
    #[arg(long, overrides_with = "replay", conflicts_with = "discard")]
    pub(super) replay: bool,
    /// Delete the journal after the word `discard` typed on a controlling terminal.
    #[arg(long, overrides_with = "discard")]
    pub(super) discard: bool,
    /// Print the result as JSON (not with `--discard`, which prints a dialogue).
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
    /// With `--replay` on a project in an older format: confirm its upgrade without typing `upgrade` (the backup keeps
    /// the project as it was).
    #[arg(long, overrides_with = "yes", requires = "replay")]
    pub(super) yes: bool,
}

impl RecoverCli {
    /// What to do with the journal.
    pub(super) fn action(&self) -> RecoverAction {
        match (self.replay, self.discard) {
            (true, _) => RecoverAction::Replay,
            (false, true) => RecoverAction::Discard,
            (false, false) => RecoverAction::Summary,
        }
    }
}

group! {
/// `ew project`.
#[derive(Subcommand, Clone, PartialEq, Eq, Debug)]
pub(crate) enum ProjectCli {
    /// The project's check schemes.
    #[command(subcommand)]
    Checks(ChecksCli),
    /// The project's asset files.
    #[command(subcommand)]
    Assets(AssetsCli),
}
}

group! {
/// `ew project checks`.
#[derive(Subcommand, Clone, PartialEq, Eq, Debug)]
pub(crate) enum ChecksCli {
    /// Name the project's check schemes (human-only).
    ///
    /// Names the project's check schemes (`project.checks.set`): `--active` the online scheme, `--release` the
    /// scheme of the release gate and `ew check`; a scheme not given returns to its default (`drafting`,
    /// `pre-delivery`). The check scheme decides check outcomes, so the command is human-only (ADR-0008 amendment 3,
    /// doc 17 section 2.3): it prints the change and asks the person to type the scheme's name (the active one, else
    /// the release one) on a controlling terminal; there is no `--yes` flag. Without a terminal it exits 2
    /// (HumanOnly: needs an interactive terminal) before the project is opened. Exit 0 when written and saved, 1 for
    /// a scheme that does not ship or an answer that is not the name, 2 on usage, terminal, open and save errors.
    Set(ChecksSetArgs),
}
}

/// `ew project checks set`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
#[command(group(ArgGroup::new("schemes").args(["active", "release"]).required(true).multiple(true)))]
pub struct ChecksSetArgs {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The online scheme.
    #[arg(
        long,
        value_name = "scheme",
        allow_hyphen_values = true,
        overrides_with = "active"
    )]
    pub active: Option<String>,
    /// The scheme of the release gate and `ew check`.
    #[arg(
        long,
        value_name = "scheme",
        allow_hyphen_values = true,
        overrides_with = "release"
    )]
    pub release: Option<String>,
}

group! {
/// `ew project assets`.
#[derive(Subcommand, Clone, PartialEq, Eq, Debug)]
pub(crate) enum AssetsCli {
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
    Purge(AssetsPurgeArgs),
}
}

/// `ew project assets purge`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct AssetsPurgeArgs {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// List the files that would go and change nothing.
    #[arg(long, overrides_with = "preview")]
    pub preview: bool,
}

group! {
/// `ew revision`.
#[derive(Subcommand, Clone, PartialEq, Eq, Debug)]
pub(crate) enum RevisionCli {
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
}

/// `ew revision approve`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct ApproveCli {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The checker's party role.
    #[arg(
        long = "as",
        value_name = "role",
        allow_hyphen_values = true,
        overrides_with = "role",
        value_parser = PossibleValuesParser::new(ew_api::PartyRole::names())
            .map(|name| ew_api::PartyRole::from_name(&name).expect("a possible value is a role"))
    )]
    pub(super) role: ew_api::PartyRole,
}

/// `ew revision release`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct ReleaseCli {
    /// The open revision's label.
    #[arg(value_name = "label")]
    pub(super) label: String,
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The archive root (default: `<name>.ew-archive` beside the project directory).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        overrides_with = "archive_dir"
    )]
    pub(super) archive_dir: Option<PathBuf>,
}

/// `ew revision withdraw`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct WithdrawCli {
    /// The released revision's label.
    #[arg(value_name = "label")]
    pub(super) label: String,
    /// The project directory.
    #[arg(value_name = "project-dir")]
    pub(super) project_dir: Option<PathBuf>,
    /// Why the revision is withdrawn.
    #[arg(
        long,
        value_name = "text",
        allow_hyphen_values = true,
        overrides_with = "reason"
    )]
    pub(super) reason: String,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    pub(super) project: Option<PathBuf>,
}

impl ApproveCli {
    /// What `revision approve` does, and the project.
    pub(super) fn revision(&self) -> RevisionArgs {
        RevisionArgs {
            action: RevisionAction::Approve {
                role: self.role.clone(),
            },
            project: self.project.0.clone(),
        }
    }
}

impl ReleaseCli {
    /// What `revision release` does, and the project.
    pub(super) fn revision(&self) -> RevisionArgs {
        RevisionArgs {
            action: RevisionAction::Release {
                label: self.label.clone(),
                archive_dir: self.archive_dir.clone(),
            },
            project: self.project.0.clone(),
        }
    }
}

impl WithdrawCli {
    /// What `revision withdraw` does, and the project.
    pub(super) fn revision(&self) -> RevisionArgs {
        RevisionArgs {
            action: RevisionAction::Withdraw {
                label: self.label.clone(),
                reason: self.reason.clone(),
            },
            project: self.project.clone().or_else(|| self.project_dir.clone()),
        }
    }
}

/// `ew verify-migration`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct VerifyMigrationArgs {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The PDF of the drawing the project was imported from.
    #[arg(
        long,
        value_name = "file.pdf",
        allow_hyphen_values = true,
        overrides_with = "reference"
    )]
    pub reference: PathBuf,
    /// The JSON file of the deltas the project is expected to have.
    #[arg(
        long,
        value_name = "file.json",
        allow_hyphen_values = true,
        overrides_with = "expected"
    )]
    pub expected: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub json: bool,
}

/// `ew verify-derived`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct VerifyArgs {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub json: bool,
}

/// `ew query`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct QueryCli {
    /// The query's name, or with three positionals the project directory.
    #[arg(value_name = "project-dir|name", allow_hyphen_values = true, value_parser = word)]
    pub(super) first: String,
    /// The key, or with three positionals the query's name.
    #[arg(value_name = "name|key", allow_hyphen_values = true, value_parser = word)]
    pub(super) second: String,
    /// The key, after the project directory and the name.
    #[arg(value_name = "key", allow_hyphen_values = true, value_parser = word)]
    pub(super) third: Option<String>,
    /// The project directory (instead of the positional one).
    #[arg(long, value_name = "dir", allow_hyphen_values = true)]
    pub(super) project: Option<PathBuf>,
    /// Add the memo's policy, the value's dependencies and the row's provenance.
    #[arg(long, overrides_with = "explain")]
    pub(super) explain: bool,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
}

impl QueryCli {
    /// The project directory, the query's name and the key: with three positionals the first is the project
    /// directory.
    pub(super) fn project_name_key(&self) -> Result<(Option<PathBuf>, &str, &str), clap::Error> {
        let (dir, name, key) = match &self.third {
            Some(key) => (Some(PathBuf::from(&self.first)), &self.second, key),
            None => (None, &self.first, &self.second),
        };
        Ok((counted_project(&["query"], self.project.clone(), dir)?, name, key))
    }
}

/// `ew layout`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct LayoutArgs {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The page, by its folio label (default: every drawn page but the panel-layout pages).
    #[arg(
        long,
        value_name = "folio",
        allow_hyphen_values = true,
        overrides_with = "page"
    )]
    pub page: Option<String>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub json: bool,
}

/// `ew route`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct RouteArgs {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The page, by its folio label (default: every drawn page but the panel-layout pages).
    #[arg(
        long,
        value_name = "folio",
        allow_hyphen_values = true,
        overrides_with = "page"
    )]
    pub page: Option<String>,
    /// The connections and representations to route, by their IDs joined by `,`.
    #[arg(
        long,
        value_name = "ids",
        allow_hyphen_values = true,
        overrides_with = "selection",
        value_delimiter = ','
    )]
    pub selection: Vec<String>,
    /// Arrange and route: line the symbols up, then route their wires.
    #[arg(long, overrides_with = "arrange")]
    pub arrange: bool,
    /// Print the plan and save nothing.
    #[arg(long, overrides_with = "dry_run")]
    pub dry_run: bool,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub json: bool,
}

/// `ew commands`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct CommandsCli {
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
}

/// `ew queries`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct QueriesCli {
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub(super) json: bool,
}

/// `ew schema`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct SchemaCli {
    /// A command's name or alias (see `ew commands`), or a result schema's name.
    #[arg(value_name = "name")]
    pub(super) name: String,
}

group! {
/// `ew merge`.
#[derive(Subcommand, Clone, PartialEq, Eq, Debug)]
pub(crate) enum MergeCli {
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
    Resolve(MergeResolveArgs),
}
}

/// `ew merge resolve`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct MergeResolveArgs {
    /// The project directory: the positional one or `--project`.
    #[command(flatten)]
    pub project: ProjectArg,
    /// The side to take.
    #[arg(long, value_enum, value_name = "side")]
    pub take: TakeArg,
    /// Only the conflicts on records of this kind (`connection`), or `file`.
    #[arg(long, value_name = "kind")]
    pub kind: Option<String>,
    /// Only the conflicts on this member (`overrides.colour`).
    #[arg(long, value_name = "path")]
    pub field: Option<String>,
    /// Print the result as JSON.
    #[arg(long, overrides_with = "json")]
    pub json: bool,
}

/// `ew merge resolve --take`.
#[derive(ValueEnum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TakeArg {
    /// Ours: the record as the merge left it.
    Ours,
    /// Theirs: the other side.
    Theirs,
}

/// `ew merge-driver`. Every positional takes a leading `-`: `%P` may name a folder such as `-old.ew`. The fourth is
/// named `P`, as the usage line shows the required one, so that three arguments are refused naming `<P>`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct MergeDriverCli {
    /// `%O`: the ancestor's version of the file.
    #[arg(value_name = "O", allow_hyphen_values = true)]
    pub(super) base: PathBuf,
    /// `%A`: ours; the merged file is written here.
    #[arg(value_name = "A", allow_hyphen_values = true)]
    pub(super) ours: PathBuf,
    /// `%B`: theirs.
    #[arg(value_name = "B", allow_hyphen_values = true)]
    pub(super) theirs: PathBuf,
    /// `%P` alone, or `%L` (unused) when `%P` follows it.
    #[arg(value_name = "P", allow_hyphen_values = true)]
    pub(super) fourth: PathBuf,
    /// `%P` after `%L`: the file's path in the repository.
    #[arg(value_name = "P", allow_hyphen_values = true)]
    pub(super) path: Option<PathBuf>,
}

impl MergeDriverCli {
    /// `%P`, the file's path in the repository: the last positional of `%O %A %B %L %P`, or of the older
    /// `%O %A %B %P` (`%L` is unused).
    pub(super) fn repo_path(&self) -> String {
        self.path
            .as_ref()
            .unwrap_or(&self.fourth)
            .to_string_lossy()
            .into_owned()
    }
}

/// `ew lib`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct LibCli {
    /// The subcommand.
    #[command(subcommand)]
    pub(super) action: LibSub,
    /// A company library: a library directory or a directory of `*.ewlib` libraries (repeatable).
    #[arg(long, value_name = "dir", allow_hyphen_values = true, global = true)]
    pub(super) company: Vec<PathBuf>,
    /// Print JSON.
    #[arg(long, overrides_with = "json", global = true)]
    pub(super) json: bool,
}

/// `lib` is no path of its own: its subcommands are, with the parent's `--company` and `--json`.
impl Node for LibCli {
    fn check(&self) -> Result<(), clap::Error> {
        self.action.check()
    }

    fn entries(cmd: &clap::Command, path: &mut Vec<String>, out: &mut Vec<Entry>) {
        LibSub::entries(cmd, path, out);
    }
}

/// `ew lib list`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct LibListCli {}

group! {
/// `ew lib list|search|index|create|import`.
#[derive(Subcommand, Clone, PartialEq, Eq, Debug)]
pub(crate) enum LibSub {
    /// List the libraries.
    ///
    /// Prints each library (tier, key, version, name, ID, `rw` or `ro`, index state, directory) and what could not be
    /// read.
    List(LibListCli),
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
}

/// `ew lib search`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct SearchCli {
    /// The text to search for.
    #[arg(value_name = "text", allow_hyphen_values = true, value_parser = word)]
    pub(super) text: String,
    /// The kind of item (repeatable; default: part).
    #[arg(long, value_name = "kind", allow_hyphen_values = true, value_parser = item_kind())]
    pub(super) kind: Vec<String>,
    /// Include deprecated and withdrawn items.
    #[arg(long, overrides_with = "all")]
    pub(super) all: bool,
    /// The most hits (0: all).
    #[arg(long, value_name = "n", allow_hyphen_values = true, overrides_with = "limit", value_parser = limit(), default_value_t = 50)]
    pub(super) limit: u32,
}

/// `ew lib index`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct IndexCli {
    /// Rebuild the index.
    #[arg(long, overrides_with = "rebuild", required = true)]
    pub(super) rebuild: bool,
}

/// `ew lib create`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct CreateCli {
    /// The new library's directory (absent or empty).
    #[arg(value_name = "dir", allow_hyphen_values = true, value_parser = path_word)]
    pub(super) dir: PathBuf,
    /// The library's key.
    #[arg(long, value_name = "key", allow_hyphen_values = true, overrides_with = "key")]
    pub(super) key: String,
    /// The library's name.
    #[arg(
        long,
        value_name = "name",
        allow_hyphen_values = true,
        overrides_with = "name"
    )]
    pub(super) name: String,
    /// The library's licence (an SPDX expression).
    #[arg(
        long,
        value_name = "spdx",
        allow_hyphen_values = true,
        overrides_with = "licence"
    )]
    pub(super) licence: String,
}

group! {
/// `ew lib import`.
#[derive(Subcommand, Clone, PartialEq, Eq, Debug)]
pub(crate) enum LibImportCli {
    /// Import a parts table into a writable library through a column map.
    ///
    /// Imports a parts table through a column map (as `import parts --map`) into the writable library `<library>`
    /// (its key or ID) under the library's lock, with the parts' import date `--as-of` (YYYY-MM-DD); new
    /// manufacturers are written with the parts. A second import of a changed list updates the parts in place (same
    /// item IDs); a part whose content changed under its published version refuses the whole file unless `--bump`
    /// writes it as the next patch version. A library locked by a live session is refused; a stale lock is taken
    /// over only with `--take-over`.
    Parts(LibImportPartsCli),
    /// Convert a QElectroTech element collection into a writable library.
    ///
    /// Converts the `.elmt` files below `<dir>` (your QElectroTech installation's collection; links are not
    /// followed) into symbols and function definitions of the writable library `<library>` (its key or ID), on
    /// QET's 10 px grid with their source paths, under the licence `LicenseRef-QET-elements` and the flag
    /// `foreign_terms`: QET's terms allow the elements in diagrams but not in a pack, so an item of this library never
    /// goes into one. The `ELEMENTS.LICENSE` of the directory, when it has one, is named in every item's sources with
    /// its SHA-256. A file that cannot be converted is listed and the others go on; a second run writes nothing new.
    /// A stale library lock is taken over only with `--take-over`.
    #[command(name = "qet-elements")]
    QetElements(LibImportQetCli),
}
}

/// `ew lib import qet-elements`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct LibImportQetCli {
    /// The target library, by its key or ID.
    #[arg(value_name = "library", allow_hyphen_values = true, value_parser = word)]
    pub(super) library: String,
    /// The collection's directory.
    #[arg(value_name = "dir", allow_hyphen_values = true, value_parser = path_word)]
    pub(super) dir: PathBuf,
    /// Take over a stale library lock.
    #[arg(long, overrides_with = "take_over")]
    pub(super) take_over: bool,
}

/// `ew lib import parts`.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct LibImportPartsCli {
    /// The target library, by its key or ID.
    #[arg(value_name = "library", allow_hyphen_values = true, value_parser = word)]
    pub(super) library: String,
    /// The parts table (CSV or XLSX).
    #[arg(value_name = "table", allow_hyphen_values = true, value_parser = path_word)]
    pub(super) file: PathBuf,
    /// The column map (a TOML file).
    #[arg(
        long,
        value_name = "map.toml",
        allow_hyphen_values = true,
        overrides_with = "map"
    )]
    pub(super) map: PathBuf,
    /// The parts' import date (YYYY-MM-DD).
    #[arg(long, value_name = "date", allow_hyphen_values = true, overrides_with = "as_of", value_parser = date)]
    pub(super) as_of: ew_api::catalog::Date,
    /// Write changed parts as the next patch version.
    #[arg(long, overrides_with = "bump")]
    pub(super) bump: bool,
    /// Take over a stale library lock.
    #[arg(long, overrides_with = "take_over")]
    pub(super) take_over: bool,
}

/// The project of a subcommand: the positional directory or `--project <dir>` (the relations exclude both); `None`:
/// the command finds it from the current directory. It reads as the `Option<PathBuf>` it holds.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ProjectArg(pub Option<PathBuf>);

impl std::ops::Deref for ProjectArg {
    type Target = Option<PathBuf>;

    fn deref(&self) -> &Option<PathBuf> {
        &self.0
    }
}

/// The two ways to give a [`ProjectArg`], as the line declares them.
#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub(crate) struct ProjectFlags {
    /// The project directory.
    #[arg(value_name = "project-dir")]
    pub(super) project_dir: Option<PathBuf>,
    /// The project directory (instead of the positional one).
    #[arg(
        long,
        value_name = "dir",
        allow_hyphen_values = true,
        conflicts_with = "project_dir"
    )]
    pub(super) project: Option<PathBuf>,
}

impl FromArgMatches for ProjectArg {
    fn from_arg_matches(matches: &clap::ArgMatches) -> Result<ProjectArg, clap::Error> {
        let flags = ProjectFlags::from_arg_matches(matches)?;
        Ok(ProjectArg(flags.project.or(flags.project_dir)))
    }

    fn update_from_arg_matches(&mut self, matches: &clap::ArgMatches) -> Result<(), clap::Error> {
        *self = ProjectArg::from_arg_matches(matches)?;
        Ok(())
    }
}

impl Args for ProjectArg {
    fn augment_args(cmd: clap::Command) -> clap::Command {
        ProjectFlags::augment_args(cmd)
    }

    fn augment_args_for_update(cmd: clap::Command) -> clap::Command {
        ProjectFlags::augment_args_for_update(cmd)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The value parsers: each keeps the refusal's words.

/// One of `values` by its name: a value of another name is refused in clap's own words, with the names it takes
/// (`invalid value 'x' for '--format <format>'` and `[possible values: csv, xlsx]`).
pub(super) fn one_of<T: Clone + Send + Sync + 'static>(
    values: Vec<(PossibleValue, T)>,
) -> impl TypedValueParser<Value = T> {
    let names: Vec<PossibleValue> = values.iter().map(|(name, _)| name.clone()).collect();
    PossibleValuesParser::new(names).map(move |typed: String| {
        values
            .iter()
            .find(|(name, _)| name.matches(&typed, true))
            .map_or_else(|| values[0].1.clone(), |(_, v)| v.clone())
    })
}

/// A `--scheme` that ships.
fn scheme_name() -> PossibleValuesParser {
    PossibleValuesParser::new(ew_api::checks::SCHEMES.iter().map(|s| s.name))
}

/// A `--lang` tag of a shipped language, any case, stored canonical.
fn language() -> impl TypedValueParser<Value = String> {
    one_of(
        ew_api::checks::messages::LANGUAGES
            .iter()
            .map(|l| (PossibleValue::new(*l), (*l).to_owned()))
            .collect(),
    )
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

/// A `--dpi` value: 72 to 600.
fn dpi() -> impl TypedValueParser<Value = u32> {
    let (lo, hi) = (
        *ew_api::output::DPI_RANGE.start(),
        *ew_api::output::DPI_RANGE.end(),
    );
    clap::value_parser!(u32).range(i64::from(lo)..=i64::from(hi))
}

/// A `--threads` value: 1 to 256.
fn threads() -> impl TypedValueParser<Value = usize> {
    clap::builder::RangedU64ValueParser::<usize>::new().range(1..=256)
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

/// The selection of `--pages` or `--sections`: both together is a usage error, neither is every page.
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
fn timeout() -> impl TypedValueParser<Value = u64> {
    clap::value_parser!(u64).range(1..=ew_api::sql::Profile::MAX_DEADLINE.as_secs())
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
fn sql_format() -> impl TypedValueParser<Value = SqlFormat> {
    one_of(vec![
        ("csv".into(), SqlFormat::Csv),
        ("json".into(), SqlFormat::Json),
        ("xlsx".into(), SqlFormat::Xlsx),
    ])
}

/// The `--format` of a row export (`export cables|bom|wires|terminals`): CSV or XLSX; `pdf`, which the list of names
/// leaves out, is refused by the row export with the subject's words ([`csv_only`]).
fn row_format() -> impl TypedValueParser<Value = ReportPages> {
    one_of(vec![
        ("csv".into(), ReportPages::Csv),
        ("xlsx".into(), ReportPages::Xlsx),
        (PossibleValue::new("pdf").hide(true), ReportPages::Pdf),
    ])
}

/// The `--format` of `export table`: CSV or XLSX.
pub(super) fn table_format() -> impl TypedValueParser<Value = ReportPages> {
    one_of(vec![
        ("csv".into(), ReportPages::Csv),
        ("xlsx".into(), ReportPages::Xlsx),
    ])
}

/// The `--preset` of `import table`: a preset of the round trip, not the `ew` dialect.
fn import_preset() -> impl TypedValueParser<Value = Preset> {
    one_of(
        Preset::ALL
            .iter()
            .filter(|p| **p != Preset::Ew)
            .map(|p| (p.name().into(), *p))
            .collect(),
    )
}

/// A `--preset`.
fn preset() -> impl TypedValueParser<Value = Preset> {
    one_of(Preset::ALL.iter().map(|p| (p.name().into(), *p)).collect())
}

/// A `--bom`.
fn bom() -> impl TypedValueParser<Value = bool> {
    one_of(vec![("on".into(), true), ("off".into(), false)])
}

/// A `--line-end`.
fn line_end() -> impl TypedValueParser<Value = LineEnd> {
    one_of(
        [LineEnd::Crlf, LineEnd::Lf]
            .map(|l| (l.name().into(), l))
            .to_vec(),
    )
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
fn report_format() -> impl TypedValueParser<Value = ReportPages> {
    one_of(vec![
        ("csv".into(), ReportPages::Csv),
        ("xlsx".into(), ReportPages::Xlsx),
        ("pdf".into(), ReportPages::Pdf),
    ])
}

/// A `lib search --kind`: the name of a library item kind.
fn item_kind() -> PossibleValuesParser {
    PossibleValuesParser::new(ew_api::library::item_kind_names())
}

/// A `lib search --limit`.
fn limit() -> impl TypedValueParser<Value = u32> {
    clap::value_parser!(u32)
}

// ---------------------------------------------------------------------------------------------------------------
// The usage errors of the rules the relations cannot say (each `Leaf::check` and the reading of its line).

/// A usage error of the subcommand at `path`, with clap's usage line for it.
pub(super) fn usage_error(path: &[&str], message: impl std::fmt::Display) -> clap::Error {
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
/// error.
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

/// The refusal of `ew export ewz` without `--out`.
pub(super) const EWZ_NEEDS_OUT: &str = "`ew export ewz` writes a file: give `--out <file.ewz>`";

/// A `--layer <class>=<name>` of `export dxf`, checked against the layer classes.
fn dxf_layer(s: &str) -> Result<(String, String), String> {
    let (class, name) = s
        .split_once('=')
        .ok_or_else(|| format!("`{s}` is no `<class>=<name>`"))?;
    ew_api::output::DxfLayers::default().set(class.trim(), name)?;
    Ok((class.trim().to_owned(), name.trim().to_owned()))
}
