// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Argument parsing and the commands of the `ew` CLI.

use std::path::{Path, PathBuf};

use ew_api::checks::{CheckMode, report, scheme};
use ew_api::import::Severity;
use ew_api::import::migration::ReportFormat;
use ew_api::import::{StarterLibrary, import_qet};
use ew_api::output::{ExportOptions, PageSelection, Parallelism, date_from_epoch};
use ew_api::{BomMode, CmdLog, Export};

/// The usage text.
pub const USAGE: &str = "usage:
  ew --version | -V
  ew import qet <file.qet> [--content <dir>] [--report md|csv|json] [--report-out <file>] [--strict]
  ew import parts <parts.csv> [--project <dir>]
  ew import parts <table.csv|table.xlsx> --map <map.toml> [--as-of <date>] [--project <dir>] [--company <dir>]...
  ew check [<project-dir>] [--scheme <name>] [--mode release|online] [--format text|md|json] [--out <file>]
  ew sql [<project-dir>] \"<select>\" [--format csv|json] [--timeout <seconds>] [--out <file>]
  ew sql [<project-dir>] --file <query.sql> [--format csv|json] [--timeout <seconds>] [--out <file>]
  ew sql dump [<project-dir>] <out.sqlite>
  ew sql schema
  ew export cables [<project-dir>] [--format csv] [--out <file>]
  ew export bom [<project-dir>] [--mode parts|assembly] [--format csv] [--out <file>]
  ew export pdf [<project-dir>] --out <file.pdf> [--pages <folios>] [--threads <n>]
  ew export svg [<project-dir>] --out <dir> [--pages <folios>] [--threads <n>]
  ew apply [<project-dir>] <file.ewcmd> [--preview] [--one-step]
  ew project checks set [<project-dir>] [--active <scheme>] [--release <scheme>]
  ew project assets purge [<project-dir>] [--preview]
  ew verify-derived [<project-dir>]
  ew lib list [--company <dir>]... [--json]
  ew lib search <text> [--kind <kind>]... [--all] [--limit <n>] [--company <dir>]... [--json]
  ew lib index --rebuild [--company <dir>]...
  ew lib create <dir> --key <key> --name <name> --licence <spdx> [--company <dir>]...
  ew lib import parts <library> <table.csv|table.xlsx> --map <map.toml> --as-of <date> [--bump] [--take-over]
      [--company <dir>]...
  ew help | --help | -h

`import qet` exits 0 when the import completes, even with findings; `--strict` exits 1 on lost connections or
error findings. With `--report` and no `--report-out` stdout is exactly the report; the summary goes to stderr.

`import parts` copies the parts of a parts table (the golden dialect: `parts.csv`, with `part_cps.csv`,
`part_templates.csv`, `cable_cores.csv` and `part_plc.csv` when they lie beside it) into the project (`--project`,
default: the nearest ancestor of the current directory holding `manifest.json`) as project-local part copies
(`parts.import`) and saves; a part whose key the project has already is skipped. Exit 0 when imported (counts on
stdout), 2 on everything else (usage, a refused table with its line and column, open and save errors).
With `--map` it reads any supplier's parts table instead, CSV (UTF-8, comma or semicolon) or XLSX (the first sheet
unless the map names one), through a column map (a TOML file: `sheet` and a `[columns]` table from `manufacturer`,
`part_number`, `type_designation`, `order_number`, `gtin`, `description`, `description.<lang>`, `kind`, `unit`,
`tech_data.<key>` and `free_props.<key>` to the column headers). Manufacturer names resolve through the manufacturers
of the libraries (`--company` as for `lib`); `--as-of` (YYYY-MM-DD) stamps the parts. Every row is reported: new,
unchanged (the project holds it), duplicate, rejected with the reason; an `A / B` part number becomes two candidates
flagged for review. The new parts are imported with `parts.import`, with copies of the manufacturers they name, and
the project is saved.

`check` runs a check scheme (mode `release`, format `text`) over a project directory
(default: the nearest ancestor of the current directory holding `manifest.json`) and exits 0 with no unwaived
errors, 1 with errors, 2 on usage and open errors. Without `--scheme` the scheme is the one the project names
(`project checks set`: `--release` for mode `release`, `--active` for mode `online`), else `pre-delivery`.

`sql` runs one read-only SELECT against the SQL projection of a project (the `api_v1_*` views; `ew sql schema` prints
them) in the sandbox of doc 17 §6.2, with the query given inline or from `--file`. The result is CSV (default) or
JSON on stdout, the row count on stderr; with `--out` it goes to a file. `--timeout` sets a deadline in seconds.
`--project <dir>` may replace the positional project directory. `sql dump` writes the projection as a SQLite
database file. `sql` exits 0 on success and 2 on everything else (usage, open and SQL errors, denials, limits).

`export cables` writes the cable list and `export bom` the bill of materials as CSV (the rows of the project's cable
overview and parts summary blocks, the golden dialect), to stdout or to `--out`. `--mode parts` (alias `component`,
the default) lists every component part; `--mode assembly` replaces the parts a configured product covers by its
row. `export` exits 0 on success and 2 on everything else (usage, open errors, a project without the block).

`export pdf` writes the project's pages as one PDF/A-2b (outline, page labels, links) and `export svg` one
`<folio>.svg` per page into the directory `--out` (replaced as a whole; it may hold SVG files only), through
the session's export job with the bundled fonts only.
`--pages` selects folio labels and ranges in page order (`101-106,901`); `--threads` changes the speed, never the
bytes. The date is `SOURCE_DATE_EPOCH` when set, else the model's. Exit 0 on success, 2 on everything else.

`apply` replays a command log (`.ewcmd`, doc 17 section 3.5) on the project, each line one transaction, and saves.
`--preview` replays it on a copy in memory, prints the changed records, tags and cross-references, and saves
nothing; `--one-step` saves only when every line commits. A project with unsaved changes of a crashed session is
refused (exit 2) until they are recovered or discarded. Exit 0 when every line commits, 1 when a prevent or
domain rule refuses a line, 2 on everything else (usage, a malformed log, an unresolvable reference, a refused
command, an open, lock or save error). Without `--one-step` the lines before a failing one are saved.

`project checks set` names the project's check schemes (`project.checks.set`): `--active` the online scheme,
`--release` the scheme of the release gate and `ew check`; a scheme not given returns to its default (`drafting`,
`pre-delivery`). The check scheme decides check outcomes, so the command is human-only (ADR-0008 amendment 3,
doc 17 section 2.3): it prints the change and asks the person to type the scheme's name (the active one, else the
release one) on a controlling terminal; there is no `--yes` flag. Without a terminal it exits 2 (HumanOnly: needs
an interactive terminal) before the project is opened. Exit 0 when written and saved, 1 for a scheme that does
not ship or an answer that is not the name, 2 on usage, terminal, open and save errors.

`project assets purge` deletes the project's asset files (`assets/<sha256>.<ext>`) whose hash no project file
mentions (kept lines and unknown members included) through the save protocol (`project.assets.purge`, doc 05
section 13.3). A save never removes an asset file; this is the explicit clean-up, and deleted files do not come back,
so the command is human-only (doc 17 section 2.3): it prints the files that would go and asks the person to type
`purge` on a controlling terminal; there is no `--yes` flag. Without a terminal it exits 2 before the project is
opened. `--preview` lists the files and changes nothing (no terminal needed). Refused while the project has unsaved
changes of a crashed session, quarantined lines or an incomplete save, and in a read-only session. Exit 0 when
purged (or nothing to purge, or previewed), 1 when the purge is refused or not confirmed, 2 on usage, terminal, open
and save errors.

`verify-derived` opens a project with the memo oracle on, recomputes every derived value cold and compares it with
the memoised one: exit 0 with no difference, 1 with differences, 2 on usage and open errors.

`lib` works on the libraries of the four tiers: system (the compiled-in starter, and `<content>/libraries` when
`EW_CONTENT_DIR` names the starter content), community (`<data>/packs`), company (every `--company <dir>`: a
library directory or a directory of `*.ewlib` libraries) and user (`<data>/libraries`), with the search index in
the cache directory (`EW_DATA_DIR` and `EW_CACHE_DIR` override the OS directories). `lib list` prints each library
(tier, key, version, name, ID, `rw` or `ro`, index state, directory) and what could not be read; `lib search`
finds parts by part number, manufacturer and description (`--kind part`, the default), or symbols, FnDefs, forms,
macros and manufacturers by key and name (`--kind symbol|fndef|form|macro|manufacturer`); `--all` includes
deprecated and withdrawn items, `--limit` caps the hits (default 50, 0: all). `lib index --rebuild` rebuilds the
index from the libraries. `lib create` makes an empty library in `<dir>` (absent or empty) with a new ID; it is
listed when `<dir>` lies in the user tier or a `--company` directory. `lib import parts` imports a parts table
through a column map (as `import parts --map`) into the writable library `<library>` (its key or ID) under the
library's lock, with the parts' import date `--as-of` (YYYY-MM-DD); new manufacturers are written with the parts. A
second import of a changed list updates the parts in place (same item IDs); a part whose content changed under its
published version refuses the whole file unless `--bump` writes it as the next patch version. A library locked by a
live session is refused; a stale lock is taken over only with `--take-over`. `--json` prints JSON. Exit 0 on
success, 2 on everything else (usage, unreadable tiers, a refused write or import, an index error).";

/// A parsed command line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Command {
    /// Print the version.
    Version,
    /// Print the usage.
    Help,
    /// Import a QElectroTech project.
    ImportQet(ImportQetArgs),
    /// `ew import parts`.
    ImportParts(ImportPartsArgs),
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
    /// Compare the memoised derived data with a cold recomputation.
    VerifyDerived(VerifyArgs),
    /// Write the pages as PDF or SVG.
    ExportPages(ExportPagesArgs),
    /// Replay a command log.
    Apply(ApplyArgs),
    /// Name the project's check schemes (`project checks set`, a human-only command).
    ChecksSet(ChecksSetArgs),
    /// Delete the asset files nothing names (`project assets purge`).
    AssetsPurge(AssetsPurgeArgs),
    /// `ew lib`: list, search, reindex and create libraries.
    Lib(LibArgs),
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

/// The page format of `ew export pdf|svg`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageFormat {
    /// One PDF file (`export pdf`).
    Pdf,
    /// One SVG file per page in a directory (`export svg`).
    Svg,
}

/// The arguments of `ew export pdf|svg`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExportPagesArgs {
    /// PDF or SVG.
    pub format: PageFormat,
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The PDF file or the SVG directory (`--out`).
    pub out: PathBuf,
    /// The page selection (`--pages`).
    pub pages: PageSelection,
    /// The threads of the page phase (`--threads`); `None`: the machine's parallelism.
    pub threads: Option<usize>,
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
}

/// What `ew export` writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportKind {
    /// The cable list (`export cables`).
    Cables,
    /// The bill of materials in a mode (`export bom --mode`).
    Bom(BomMode),
}

/// The arguments of `ew export`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExportArgs {
    /// What to write.
    pub kind: ExportKind,
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The output file (`--out`); `None`: stdout.
    pub out: Option<PathBuf>,
}

/// The arguments of `ew verify-derived`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VerifyArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
}

/// The result format of `ew sql`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SqlFormat {
    /// RFC 4180 CSV.
    Csv,
    /// JSON (`ew.sql-result/1`).
    Json,
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
}

/// The arguments of `ew sql dump`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SqlDumpArgs {
    /// The project directory; `None`: found from the current directory.
    pub project: Option<PathBuf>,
    /// The SQLite file to write.
    pub out: PathBuf,
}

/// The output format of `ew check`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CheckFormat {
    /// One line per finding.
    Text,
    /// Markdown.
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
}

/// The arguments of `ew import qet`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportQetArgs {
    /// The project file.
    pub file: PathBuf,
    /// The starter content directory (`--content`).
    pub content: Option<PathBuf>,
    /// The report format (`--report`).
    pub report: Option<ReportFormat>,
    /// The report file (`--report-out`).
    pub report_out: Option<PathBuf>,
    /// Fail on lost connections or error findings (`--strict`).
    pub strict: bool,
}

/// The arguments of `ew import parts`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportPartsArgs {
    /// The parts table (`parts.csv`).
    pub file: PathBuf,
    /// The project directory (`--project`).
    pub project: Option<PathBuf>,
    /// The column map (`--map`): a supplier's table instead of the golden dialect.
    pub map: Option<PathBuf>,
    /// The import date (`--as-of`, with `--map`).
    pub as_of: Option<ew_api::catalog::Date>,
    /// The company library paths (`--company`, with `--map`): their manufacturers resolve the names.
    pub company: Vec<PathBuf>,
}

/// Parses the arguments after the program name.
///
/// # Errors
///
/// The text of the usage error.
pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Command, String> {
    let mut it = args.into_iter();
    let Some(first) = it.next() else {
        return Err("no command given".to_owned());
    };
    match first.as_str() {
        "--version" | "-V" => no_more(it, Command::Version),
        "help" | "--help" | "-h" => no_more(it, Command::Help),
        "import" => match it.next().as_deref() {
            Some("qet") => parse_import_qet(it),
            Some("parts") => parse_import_parts(it),
            Some(other) => Err(format!("unknown import format `{other}`")),
            None => Err("`import` needs a format (qet, parts)".to_owned()),
        },
        "check" => parse_check(it),
        "sql" => parse_sql(it),
        "export" => parse_export(it),
        "apply" => parse_apply(it),
        "project" => match (it.next().as_deref(), it.next().as_deref()) {
            (Some("checks"), Some("set")) => parse_checks_set(it),
            (Some("assets"), Some("purge")) => parse_assets_purge(it),
            _ => Err("`project` takes `checks set` or `assets purge`".to_owned()),
        },
        "verify-derived" => parse_verify(it),
        "lib" => parse_lib(it),
        other => Err(format!("unknown command `{other}`")),
    }
}

fn no_more(mut it: impl Iterator<Item = String>, c: Command) -> Result<Command, String> {
    match it.next() {
        None => Ok(c),
        Some(extra) => Err(format!("unexpected argument `{extra}`")),
    }
}

fn parse_import_qet(mut it: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut file = None;
    let mut content = None;
    let mut report = None;
    let mut report_out = None;
    let mut strict = false;
    while let Some(a) = it.next() {
        match a.as_str() {
            "--content" => content = Some(PathBuf::from(value(&mut it, "--content")?)),
            "--report" => {
                report = Some(
                    value(&mut it, "--report")?
                        .parse::<ReportFormat>()
                        .map_err(|e| e.to_string())?,
                );
            }
            "--report-out" => report_out = Some(PathBuf::from(value(&mut it, "--report-out")?)),
            "--strict" => strict = true,
            opt if opt.starts_with('-') => return Err(format!("unknown option `{opt}`")),
            _ => {
                if file.replace(PathBuf::from(&a)).is_some() {
                    return Err(format!("unexpected argument `{a}`"));
                }
            }
        }
    }
    let file = file.ok_or_else(|| "`import qet` needs a project file".to_owned())?;
    Ok(Command::ImportQet(ImportQetArgs {
        file,
        content,
        report,
        report_out,
        strict,
    }))
}

fn parse_import_parts(mut it: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut file = None;
    let mut project = None;
    let mut map = None;
    let mut as_of = None;
    let mut company = Vec::new();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--project" => project = Some(PathBuf::from(value(&mut it, "--project")?)),
            "--map" => map = Some(PathBuf::from(value(&mut it, "--map")?)),
            "--as-of" => as_of = Some(date(&value(&mut it, "--as-of")?)?),
            "--company" => company.push(PathBuf::from(value(&mut it, "--company")?)),
            opt if opt.starts_with('-') => return Err(format!("unknown option `{opt}`")),
            _ => {
                if file.replace(PathBuf::from(&a)).is_some() {
                    return Err(format!("unexpected argument `{a}`"));
                }
            }
        }
    }
    let file = file.ok_or_else(|| "`import parts` needs a parts table (<parts.csv>)".to_owned())?;
    if map.is_none() && (as_of.is_some() || !company.is_empty()) {
        return Err("`--as-of` and `--company` need `--map`".to_owned());
    }
    Ok(Command::ImportParts(ImportPartsArgs {
        file,
        project,
        map,
        as_of,
        company,
    }))
}

/// An import date `YYYY-MM-DD`.
fn date(v: &str) -> Result<ew_api::catalog::Date, String> {
    v.parse()
        .map_err(|_| format!("`--as-of` needs a date YYYY-MM-DD, not `{v}`"))
}

fn parse_check(mut it: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut project = None;
    let mut scheme_name = None;
    let mut mode = CheckMode::Release;
    let mut format = CheckFormat::Text;
    let mut out = None;
    while let Some(a) = it.next() {
        match a.as_str() {
            "--scheme" => {
                let name = value(&mut it, "--scheme")?;
                if scheme(&name).is_none() {
                    return Err(format!("unknown scheme `{name}`"));
                }
                scheme_name = Some(name);
            }
            "--mode" => {
                mode = match value(&mut it, "--mode")?.as_str() {
                    "release" => CheckMode::Release,
                    "online" => CheckMode::Online,
                    other => return Err(format!("unknown mode `{other}` (release, online)")),
                };
            }
            "--format" => {
                format = match value(&mut it, "--format")?.as_str() {
                    "text" => CheckFormat::Text,
                    "md" => CheckFormat::Markdown,
                    "json" => CheckFormat::Json,
                    other => return Err(format!("unknown format `{other}` (text, md, json)")),
                };
            }
            "--out" => out = Some(PathBuf::from(value(&mut it, "--out")?)),
            opt if opt.starts_with('-') => return Err(format!("unknown option `{opt}`")),
            _ => {
                if project.replace(PathBuf::from(&a)).is_some() {
                    return Err(format!("unexpected argument `{a}`"));
                }
            }
        }
    }
    Ok(Command::Check(CheckArgs {
        project,
        scheme: scheme_name,
        mode,
        format,
        out,
    }))
}

fn parse_export(mut it: impl Iterator<Item = String>) -> Result<Command, String> {
    let what = match it.next().as_deref() {
        Some(w @ ("cables" | "bom")) => w.to_owned(),
        Some("pdf") => return parse_export_pages(PageFormat::Pdf, it),
        Some("svg") => return parse_export_pages(PageFormat::Svg, it),
        Some(other) => return Err(format!("unknown export `{other}` (cables, bom, pdf, svg)")),
        None => return Err("`export` needs a subject (cables, bom, pdf, svg)".to_owned()),
    };
    let mut project = None;
    let mut out = None;
    let mut mode = BomMode::Component;
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => out = Some(PathBuf::from(value(&mut it, "--out")?)),
            "--format" => match value(&mut it, "--format")?.as_str() {
                "csv" => {}
                other @ ("xlsx" | "pdf") => return Err(format!("`--format {other}` is not built yet (csv)")),
                other => return Err(format!("unknown format `{other}` (csv)")),
            },
            "--mode" if what == "bom" => {
                mode = match value(&mut it, "--mode")?.as_str() {
                    "parts" | "component" => BomMode::Component,
                    "assembly" => BomMode::Assembly,
                    other => return Err(format!("unknown mode `{other}` (parts, assembly)")),
                };
            }
            opt if opt.starts_with('-') => return Err(format!("unknown option `{opt}`")),
            _ => {
                if project.replace(PathBuf::from(&a)).is_some() {
                    return Err(format!("unexpected argument `{a}`"));
                }
            }
        }
    }
    let kind = if what == "cables" {
        ExportKind::Cables
    } else {
        ExportKind::Bom(mode)
    };
    Ok(Command::Export(ExportArgs { kind, project, out }))
}

fn parse_export_pages(format: PageFormat, mut it: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut project = None;
    let mut out = None;
    let mut pages = PageSelection::All;
    let mut threads = None;
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => out = Some(PathBuf::from(value(&mut it, "--out")?)),
            "--pages" => {
                pages = PageSelection::parse(&value(&mut it, "--pages")?).map_err(|e| e.to_string())?;
            }
            "--threads" => {
                let v = value(&mut it, "--threads")?;
                match v.parse::<usize>() {
                    Ok(n) if (1..=256).contains(&n) => threads = Some(n),
                    _ => return Err(format!("`--threads` needs a number from 1 to 256, got `{v}`")),
                }
            }
            opt @ ("--lang" | "--omit-personal-data") => {
                return Err(format!("`{opt}` is not built yet"));
            }
            opt if opt.starts_with('-') => return Err(format!("unknown option `{opt}`")),
            _ => {
                if project.replace(PathBuf::from(&a)).is_some() {
                    return Err(format!("unexpected argument `{a}`"));
                }
            }
        }
    }
    let what = match format {
        PageFormat::Pdf => "export pdf` needs `--out <file.pdf>",
        PageFormat::Svg => "export svg` needs `--out <dir>",
    };
    let out = out.ok_or_else(|| format!("`{what}`"))?;
    Ok(Command::ExportPages(ExportPagesArgs {
        format,
        project,
        out,
        pages,
        threads,
    }))
}

fn parse_apply(it: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut positionals = Vec::new();
    let mut preview = false;
    let mut one_step = false;
    for a in it {
        match a.as_str() {
            "--preview" => preview = true,
            "--one-step" => one_step = true,
            opt if opt.starts_with('-') => return Err(format!("unknown option `{opt}`")),
            _ => positionals.push(PathBuf::from(a)),
        }
    }
    let mut pos = positionals.into_iter();
    let (project, log) = match (pos.next(), pos.next(), pos.next()) {
        (Some(log), None, _) => (None, log),
        (Some(dir), Some(log), None) => (Some(dir), log),
        (None, ..) => return Err("`apply` needs a command log (<file.ewcmd>)".to_owned()),
        (Some(_), Some(_), Some(extra)) => return Err(format!("unexpected argument `{}`", extra.display())),
    };
    Ok(Command::Apply(ApplyArgs {
        project,
        log,
        preview,
        one_step,
    }))
}

fn parse_checks_set(mut it: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut args = ChecksSetArgs {
        project: None,
        active: None,
        release: None,
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--active" => args.active = Some(value(&mut it, "--active")?),
            "--release" => args.release = Some(value(&mut it, "--release")?),
            opt if opt.starts_with('-') => return Err(format!("unknown option `{opt}`")),
            _ if args.project.is_none() => args.project = Some(PathBuf::from(a)),
            _ => return Err(format!("unexpected argument `{a}`")),
        }
    }
    if args.active.is_none() && args.release.is_none() {
        return Err("`project checks set` needs `--active <scheme>` or `--release <scheme>`".to_owned());
    }
    Ok(Command::ChecksSet(args))
}

fn parse_assets_purge(it: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut args = AssetsPurgeArgs {
        project: None,
        preview: false,
    };
    for a in it {
        match a.as_str() {
            "--preview" => args.preview = true,
            opt if opt.starts_with('-') => return Err(format!("unknown option `{opt}`")),
            _ if args.project.is_none() => args.project = Some(PathBuf::from(a)),
            _ => return Err(format!("unexpected argument `{a}`")),
        }
    }
    Ok(Command::AssetsPurge(args))
}

fn parse_verify(mut it: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut project = None;
    for a in it.by_ref() {
        match a.as_str() {
            opt if opt.starts_with('-') => return Err(format!("unknown option `{opt}`")),
            _ => {
                if project.replace(PathBuf::from(&a)).is_some() {
                    return Err(format!("unexpected argument `{a}`"));
                }
            }
        }
    }
    Ok(Command::VerifyDerived(VerifyArgs { project }))
}

fn parse_sql(it: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut rest: Vec<String> = it.collect();
    match rest.first().map(String::as_str) {
        Some("schema") => {
            return match rest.get(1) {
                None => Ok(Command::SqlSchema),
                Some(extra) => Err(format!("unexpected argument `{extra}`")),
            };
        }
        Some("dump") => {
            rest.remove(0);
            return parse_sql_dump(rest.into_iter());
        }
        _ => {}
    }
    let mut it = rest.into_iter();
    let mut positionals: Vec<String> = Vec::new();
    let mut project_opt = None;
    let mut file = None;
    let mut format = SqlFormat::Csv;
    let mut timeout = None;
    let mut out = None;
    while let Some(a) = it.next() {
        match a.as_str() {
            "--project" => {
                if project_opt
                    .replace(PathBuf::from(value(&mut it, "--project")?))
                    .is_some()
                {
                    return Err("`--project` given twice".to_owned());
                }
            }
            "--file" => file = Some(PathBuf::from(value(&mut it, "--file")?)),
            "--format" => {
                format = match value(&mut it, "--format")?.as_str() {
                    "csv" => SqlFormat::Csv,
                    "json" => SqlFormat::Json,
                    "xlsx" => return Err("`--format xlsx` is not built yet (csv, json)".to_owned()),
                    other => return Err(format!("unknown format `{other}` (csv, json)")),
                };
            }
            "--timeout" => {
                let v = value(&mut it, "--timeout")?;
                let max = ew_api::sql::Profile::MAX_DEADLINE.as_secs();
                match v.parse::<u64>() {
                    Ok(n) if (1..=max).contains(&n) => timeout = Some(n),
                    _ => {
                        return Err(format!(
                            "`--timeout` needs a number of seconds from 1 to {max}, got `{v}`"
                        ));
                    }
                }
            }
            "--out" => out = Some(PathBuf::from(value(&mut it, "--out")?)),
            // An argument is an option only when it is one of the above; an inline query may start with an
            // SQL comment (`-- …`). A single word starting with `-` is still a mistyped option.
            opt if opt.starts_with('-') && !opt.contains(char::is_whitespace) => {
                return Err(format!("unknown option `{opt}`"));
            }
            _ => positionals.push(a),
        }
    }
    let mut pos = positionals.into_iter();
    let (positional_project, query) = match file {
        Some(f) => {
            let project = pos.next().map(PathBuf::from);
            if let Some(extra) = pos.next() {
                return Err(format!(
                    "`--file` cannot be combined with a query text (`{extra}`)"
                ));
            }
            (project, SqlSource::File(f))
        }
        None => match (pos.next(), pos.next(), pos.next()) {
            (Some(q), None, _) => (None, SqlSource::Text(q)),
            (Some(d), Some(q), None) => (Some(PathBuf::from(d)), SqlSource::Text(q)),
            (None, ..) => return Err("`sql` needs a query (or `--file`)".to_owned()),
            (Some(_), Some(_), Some(extra)) => return Err(format!("unexpected argument `{extra}`")),
        },
    };
    let project = match (project_opt, positional_project) {
        (Some(_), Some(_)) => {
            return Err("`--project` cannot be combined with a positional project directory".to_owned());
        }
        (Some(d), None) | (None, Some(d)) => Some(d),
        (None, None) => None,
    };
    Ok(Command::Sql(SqlArgs {
        project,
        query,
        format,
        timeout,
        out,
    }))
}

fn parse_sql_dump(mut it: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut positionals: Vec<String> = Vec::new();
    let mut project_opt = None;
    while let Some(a) = it.next() {
        match a.as_str() {
            "--project" => {
                if project_opt
                    .replace(PathBuf::from(value(&mut it, "--project")?))
                    .is_some()
                {
                    return Err("`--project` given twice".to_owned());
                }
            }
            opt if opt.starts_with('-') => return Err(format!("unknown option `{opt}`")),
            _ => positionals.push(a),
        }
    }
    let mut pos = positionals.into_iter();
    let (positional_project, out) = match (pos.next(), pos.next(), pos.next()) {
        (Some(o), None, _) => (None, PathBuf::from(o)),
        (Some(d), Some(o), None) => (Some(PathBuf::from(d)), PathBuf::from(o)),
        (None, ..) => return Err("`sql dump` needs an output file".to_owned()),
        (Some(_), Some(_), Some(extra)) => return Err(format!("unexpected argument `{extra}`")),
    };
    let project = match (project_opt, positional_project) {
        (Some(_), Some(_)) => {
            return Err("`--project` cannot be combined with a positional project directory".to_owned());
        }
        (Some(d), None) | (None, Some(d)) => Some(d),
        (None, None) => None,
    };
    Ok(Command::SqlDump(SqlDumpArgs { project, out }))
}

fn parse_lib(mut it: impl Iterator<Item = String>) -> Result<Command, String> {
    let sub = it
        .next()
        .ok_or_else(|| "`lib` takes `list`, `search`, `index`, `create` or `import`".to_owned())?;
    let mut company = Vec::new();
    let mut json = false;
    let mut positional: Vec<String> = Vec::new();
    let mut kinds = Vec::new();
    let mut all = false;
    let mut limit = None;
    let mut rebuild = false;
    let (mut key, mut name, mut licence) = (None, None, None);
    let (mut map, mut as_of, mut bump, mut take_over) = (None, None, false, false);
    while let Some(a) = it.next() {
        match (sub.as_str(), a.as_str()) {
            ("import", "--map") => map = Some(PathBuf::from(value(&mut it, "--map")?)),
            ("import", "--as-of") => as_of = Some(date(&value(&mut it, "--as-of")?)?),
            ("import", "--bump") => bump = true,
            ("import", "--take-over") => take_over = true,
            (_, "--company") => company.push(PathBuf::from(value(&mut it, "--company")?)),
            ("list" | "search", "--json") => json = true,
            ("search", "--kind") => {
                let k = value(&mut it, "--kind")?;
                if ew_api::library::item_kind(&k).is_none() {
                    return Err(format!(
                        "unknown kind `{k}` (part, symbol, fndef, form, macro, manufacturer)"
                    ));
                }
                kinds.push(k);
            }
            ("search", "--all") => all = true,
            ("search", "--limit") => {
                let v = value(&mut it, "--limit")?;
                limit = Some(
                    v.parse::<u32>()
                        .map_err(|_| format!("`--limit` needs a number, not `{v}`"))?,
                );
            }
            ("index", "--rebuild") => rebuild = true,
            ("create", "--key") => key = Some(value(&mut it, "--key")?),
            ("create", "--name") => name = Some(value(&mut it, "--name")?),
            ("create", "--licence") => licence = Some(value(&mut it, "--licence")?),
            (_, opt) if opt.starts_with("--") => return Err(format!("unknown option `{opt}`")),
            _ => positional.push(a),
        }
    }
    let one = |what: &str, positional: Vec<String>| -> Result<String, String> {
        let mut p = positional.into_iter();
        let first = p.next().ok_or_else(|| format!("`lib {sub}` needs {what}"))?;
        match p.next() {
            None => Ok(first),
            Some(extra) => Err(format!("unexpected argument `{extra}`")),
        }
    };
    let action = match sub.as_str() {
        "list" => {
            if let Some(extra) = positional.first() {
                return Err(format!("unexpected argument `{extra}`"));
            }
            LibAction::List
        }
        "search" => LibAction::Search {
            text: one("the text to search for", positional)?,
            kinds,
            all,
            limit: limit.unwrap_or(50),
        },
        "index" => {
            if let Some(extra) = positional.first() {
                return Err(format!("unexpected argument `{extra}`"));
            }
            if !rebuild {
                return Err("`lib index` needs `--rebuild`".to_owned());
            }
            LibAction::IndexRebuild
        }
        "create" => {
            let dir = PathBuf::from(one("a directory", positional)?);
            let need = |v: Option<String>, opt: &str| v.ok_or_else(|| format!("`lib create` needs `{opt}`"));
            LibAction::Create {
                dir,
                key: need(key, "--key")?,
                name: need(name, "--name")?,
                licence: need(licence, "--licence")?,
            }
        }
        "import" => {
            let mut p = positional.into_iter();
            if p.next().as_deref() != Some("parts") {
                return Err("`lib import` takes `parts`".to_owned());
            }
            let (Some(library), Some(file)) = (p.next(), p.next()) else {
                return Err("`lib import parts` needs a library and a parts table".to_owned());
            };
            if let Some(extra) = p.next() {
                return Err(format!("unexpected argument `{extra}`"));
            }
            LibAction::ImportParts {
                library,
                file: PathBuf::from(file),
                map: map.ok_or_else(|| "`lib import parts` needs `--map`".to_owned())?,
                as_of: as_of.ok_or_else(|| "`lib import parts` needs `--as-of`".to_owned())?,
                bump,
                take_over,
            }
        }
        other => {
            return Err(format!(
                "unknown `lib` command `{other}` (list, search, index, create, import)"
            ));
        }
    };
    Ok(Command::Lib(LibArgs {
        action,
        company,
        json,
    }))
}

fn value(it: &mut impl Iterator<Item = String>, opt: &str) -> Result<String, String> {
    it.next().ok_or_else(|| format!("`{opt}` needs a value"))
}

/// Runs `cmd` and returns the exit code (0 success, 1 model check failed or `--strict` violated, 2 usage, I/O and
/// import errors).
pub fn run(cmd: Command) -> i32 {
    match cmd {
        Command::Version => {
            println!("ew {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Command::Help => {
            println!("{USAGE}");
            0
        }
        Command::ImportQet(a) => run_import_qet(&a),
        Command::ImportParts(a) => run_import_parts(&a),
        Command::Check(a) => run_check(&a),
        Command::Sql(a) => run_sql(&a),
        Command::SqlDump(a) => run_sql_dump(&a),
        Command::Export(a) => run_export(&a),
        Command::VerifyDerived(a) => run_verify_derived(&a),
        Command::ExportPages(a) => run_export_pages(&a),
        Command::Apply(a) => run_apply(&a),
        Command::ChecksSet(a) => run_checks_set(&a),
        Command::AssetsPurge(a) => run_assets_purge(&a),
        Command::Lib(a) => run_lib(&a),
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

/// The explicit project directory, else the one found from the current directory.
fn project_dir(explicit: Option<&PathBuf>) -> Option<PathBuf> {
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
    let dir = project_dir(project).ok_or(2)?;
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
    session.sql_projection(Some(&report)).map_err(|e| {
        eprintln!("error: {e}");
        2
    })
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
    let projection = match open_projection(a.project.as_ref()) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let mut profile = ew_api::sql::Profile::EW_SQL;
    if let Some(n) = a.timeout {
        profile = profile.with_deadline(std::time::Duration::from_secs(n));
    }
    let set = match projection.reader(profile).and_then(|r| r.query(&sql)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    let text = match a.format {
        SqlFormat::Csv => set.to_csv(),
        SqlFormat::Json => format!("{:#}\n", set.to_json()),
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
    let Some(dir) = project_dir(a.project.as_ref()) else {
        return 2;
    };
    let (session, _) = match ew_api::Session::open_reader(&dir, env!("CARGO_PKG_VERSION"), "export") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
    let what = match a.kind {
        ExportKind::Cables => Export::Cables,
        ExportKind::Bom(mode) => Export::Bom(mode),
    };
    let exported = match session.export(what) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {}", export_refusal(what, &e));
            return 2;
        }
    };
    match &a.out {
        Some(path) => match write_report(path, &exported.csv) {
            Ok(()) => println!("export written to {}", path.display()),
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
        },
        None => {
            print!("{}", exported.csv);
            eprintln!("{} rows", exported.rows);
        }
    }
    0
}

// The program boundary: the one environment input of the exports, and there is no wall-clock fallback (no
// `SOURCE_DATE_EPOCH` means no creation date), so the same input and variable give the same bytes (doc 18 §3.5).
#[allow(clippy::disallowed_methods)]
fn source_date_epoch() -> Option<std::ffi::OsString> {
    std::env::var_os("SOURCE_DATE_EPOCH").filter(|v| !v.is_empty())
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

/// Writes an SVG set into the directory `out` as a whole: the files go to a fresh sibling directory first, which
/// then replaces `out`, so `out` holds exactly this export (no stale page of an earlier export) and a failure
/// leaves the earlier set as it was. A directory that holds anything but `.svg` files is refused, so no other file
/// is ever removed.
fn write_svg_set(out: &Path, files: &[ew_api::output::SvgFile]) -> Result<(), String> {
    let at = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    if out.exists() {
        let entries = std::fs::read_dir(out).map_err(|e| at(out, e))?;
        for e in entries {
            let path = e.map_err(|e| at(out, e))?.path();
            if !path.is_file() || path.extension().is_none_or(|x| x != "svg") {
                return Err(format!(
                    "{}: the directory holds {}, which is not an SVG file; choose an empty or SVG-only directory",
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
            files.iter().try_for_each(|f| {
                let path = tmp.join(&f.name);
                write_synced(&path, f.text.as_bytes()).map_err(|e| at(&path, e))
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

fn run_export_pages(a: &ExportPagesArgs) -> i32 {
    let Some(dir) = project_dir(a.project.as_ref()) else {
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
            (
                format!("SVG written to {}: {} files", a.out.display(), svg.files.len()),
                svg.findings.len(),
            )
        }
    };
    println!("{written}");
    if findings > 0 {
        eprintln!("{findings} page findings (unresolved wire ends, unknown styles, overflowing table texts)");
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
        return run_apply_preview(&dir, &log, &file);
    }
    let origin = ew_api::Origin::Cli {
        subcommand: "apply".to_owned(),
        confirmed: false,
    };
    let mut session = match open_for_writing(&dir, origin) {
        Ok(s) => s,
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
        return code;
    }
    match session.save() {
        Ok(_) => {
            println!("applied {committed} of {} lines; saved", log.len());
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
    let options = ew_api::SessionOptions {
        origin,
        ..ew_api::SessionOptions::default()
    };
    let (session, opened) = match ew_api::Session::open(dir, store, options, ew_api::Recovery::Keep) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return Err(2);
        }
    };
    if let ew_api::Access::ReadOnly(reason) = &opened.access {
        eprintln!(
            "error: {}: the project opens read-only ({reason:?})",
            dir.display()
        );
        session.close();
        return Err(2);
    }
    if opened.recovery.is_some() {
        eprintln!(
            "error: {}: a crashed session left unsaved changes in its journal; recover or discard them first: \
             discard it in ElektraWatt (File > Discard unsaved changes of the earlier session) or wait for \
             `ew recover` (FT-252); nothing was applied",
            dir.display()
        );
        session.close();
        return Err(2);
    }
    Ok(session)
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
            "error: HumanOnly: needs an interactive terminal (`project checks set` is confirmed by typing the              scheme name); nothing was changed"
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
        Ok(_) => {
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
fn run_apply_preview(dir: &Path, log: &CmdLog, file: &str) -> i32 {
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
    let Some(dir) = project_dir(a.project.as_ref()) else {
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
    let (session, opened) = match ew_api::Session::open_read_only(&dir, env!("CARGO_PKG_VERSION")) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
    };
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
    let result = session
        .check(sch, a.mode)
        .with_integrity(ew_api::integrity_items(&opened));
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
                let rows: Vec<serde_json::Value> = libs
                    .iter()
                    .map(|l| {
                        serde_json::json!({
                            "id": l.id.to_string(), "key": l.key, "name": l.name, "tier": l.tier.name(),
                            "version": l.version, "licence": l.licence, "writable": l.writable,
                            "generation": l.generation, "index": index_state(&l.index),
                            "path": l.path.as_ref().map(|p| p.display().to_string()), "findings": l.findings,
                        })
                    })
                    .collect();
                println!("{}", serde_json::Value::Array(rows));
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
                    for f in &l.findings {
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
                        serde_json::json!({
                            "kind": "part", "library": h.library.to_string(), "library_key": key_of(&h.library),
                            "item": h.item.to_string(), "version": h.version, "tier": h.tier.name(),
                            "part_number": h.part_number, "manufacturer": h.manufacturer,
                            "description": h.description, "part_kind": json_text(serde_json::to_value(&h.kind)),
                            "status": serde_json::to_value(&h.status).unwrap_or_default(), "rank_tier": h.rank_tier,
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
                        serde_json::json!({
                            "kind": h.kind.name(), "library": h.library.to_string(),
                            "library_key": key_of(&h.library), "item": h.item.to_string(), "version": h.version,
                            "tier": h.tier.name(), "key": h.key, "name": h.name,
                            "status": serde_json::to_value(&h.status).unwrap_or_default(), "rank_tier": h.rank_tier,
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
                println!(
                    "{}",
                    serde_json::Value::Array(rows.into_iter().map(|(j, _)| j).collect())
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
                    println!("index rebuilt: {parts} parts, {items} other items");
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
                println!("created {key} ({id}) in {}", dir.display());
                if !reg.libraries().iter().any(|l| l.id == id) {
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
    print_catalog_rows(&import);
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
        if let Err(e) = session.save() {
            eprintln!("error: the save failed: {e:?}");
            session.close();
            return 2;
        }
    }
    println!(
        "imported {}: {}; {created} created, {skipped} skipped; saved",
        input.name(),
        counts_text(&import)
    );
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
    if let Err(e) = session.save() {
        eprintln!("error: the save failed: {e:?}");
        session.close();
        return 2;
    }
    println!(
        "imported {}: {created} created, {skipped} skipped, 0 errors; saved",
        parts.source
    );
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
    let to_stderr = a.report.is_some() && a.report_out.is_none();
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
    match &imported.session {
        Ok(s) => say(format!("derived: {} pages in page order", s.page_labels().len())),
        Err(msg) => {
            say(format!("model check failed: {msg}"));
            code = 1;
        }
    }
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
            code = 1;
        }
        if errors > 0 {
            eprintln!("strict: {errors} error findings");
            code = 1;
        }
    }
    if let Some(fmt) = a.report {
        let text = report.render(fmt);
        match &a.report_out {
            Some(path) => match write_report(path, &text) {
                Ok(()) => println!("report written to {}", path.display()),
                Err(e) => {
                    eprintln!("error: {}: {e}", path.display());
                    return 2;
                }
            },
            None => print!("{text}"),
        }
    }
    code
}

fn write_report(path: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Command, String> {
        parse(args.iter().map(|s| (*s).to_owned()))
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
        for (args, error) in [
            (
                &["lib"][..],
                "`lib` takes `list`, `search`, `index`, `create` or `import`",
            ),
            (
                &["lib", "frob"],
                "unknown `lib` command `frob` (list, search, index, create, import)",
            ),
            (&["lib", "list", "x"], "unexpected argument `x`"),
            (&["lib", "search"], "`lib search` needs the text to search for"),
            (&["lib", "search", "a", "b"], "unexpected argument `b`"),
            (
                &["lib", "search", "a", "--kind", "device"],
                "unknown kind `device` (part, symbol, fndef, form, macro, manufacturer)",
            ),
            (
                &["lib", "search", "a", "--limit", "x"],
                "`--limit` needs a number, not `x`",
            ),
            (&["lib", "index"], "`lib index` needs `--rebuild`"),
            (&["lib", "index", "--json"], "unknown option `--json`"),
            (
                &["lib", "create", "d", "--key", "k", "--name", "N"],
                "`lib create` needs `--licence`",
            ),
            (&["lib", "create", "--key", "k"], "`lib create` needs a directory"),
            (&["lib", "list", "--company"], "`--company` needs a value"),
        ] {
            assert_eq!(p(args), Err(error.to_owned()), "{args:?}");
        }
    }

    #[test]
    fn simple_commands() {
        assert_eq!(p(&["--version"]), Ok(Command::Version));
        assert_eq!(p(&["-V"]), Ok(Command::Version));
        assert_eq!(p(&["help"]), Ok(Command::Help));
        assert_eq!(p(&["-h"]), Ok(Command::Help));
        assert!(p(&[]).is_err());
        assert!(p(&["--version", "x"]).is_err());
        assert!(p(&["frobnicate"]).is_err());
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
                out: PathBuf::from("o.sqlite")
            }))
        );
        assert_eq!(
            p(&["sql", "dump", "proj", "o.sqlite"]),
            Ok(Command::SqlDump(SqlDumpArgs {
                project: Some(PathBuf::from("proj")),
                out: PathBuf::from("o.sqlite")
            }))
        );
        assert_eq!(
            p(&["sql", "dump", "--project", "proj", "o.sqlite"]),
            Ok(Command::SqlDump(SqlDumpArgs {
                project: Some(PathBuf::from("proj")),
                out: PathBuf::from("o.sqlite")
            }))
        );
        assert_eq!(p(&["sql", "schema"]), Ok(Command::SqlSchema));
    }

    #[test]
    fn sql_errors() {
        let xlsx = p(&["sql", "SELECT 1", "--format", "xlsx"]).unwrap_err();
        assert_eq!(xlsx, "`--format xlsx` is not built yet (csv, json)");
        let unknown = p(&["sql", "SELECT 1", "--format", "x"]).unwrap_err();
        assert_eq!(unknown, "unknown format `x` (csv, json)");
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
        for (args, error) in [
            (
                &["import", "parts", "a.csv", "--as-of", "2026-10-29"][..],
                "`--as-of` and `--company` need `--map`",
            ),
            (
                &["import", "parts", "a.csv", "--map", "m", "--as-of", "29.10.2026"],
                "`--as-of` needs a date YYYY-MM-DD, not `29.10.2026`",
            ),
            (&["lib", "import", "macros"], "`lib import` takes `parts`"),
            (
                &["lib", "import", "parts", "std"],
                "`lib import parts` needs a library and a parts table",
            ),
            (
                &["lib", "import", "parts", "std", "a.csv", "--as-of", "2026-10-29"],
                "`lib import parts` needs `--map`",
            ),
            (
                &["lib", "import", "parts", "std", "a.csv", "--map", "m"],
                "`lib import parts` needs `--as-of`",
            ),
            (
                &["lib", "import", "parts", "std", "a.csv", "b.csv"],
                "unexpected argument `b.csv`",
            ),
            (&["lib", "list", "--bump"], "unknown option `--bump`"),
        ] {
            assert_eq!(p(args), Err(error.to_owned()), "{args:?}");
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
        assert!(p(&["export", "wires"]).is_err());
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
        assert!(p(&["export", "png", "--out", "o"]).is_err());
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
            }))
        );
        assert_eq!(
            p(&["apply", "--one-step", "x.ewcmd"]),
            Ok(Command::Apply(ApplyArgs {
                project: None,
                log: PathBuf::from("x.ewcmd"),
                preview: false,
                one_step: true,
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
            Ok(Command::VerifyDerived(VerifyArgs { project: None }))
        );
        assert_eq!(
            p(&["verify-derived", "proj"]),
            Ok(Command::VerifyDerived(VerifyArgs {
                project: Some(PathBuf::from("proj"))
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
        let parse = |a: &[&str]| parse(a.iter().map(|s| (*s).to_owned()));
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
        let parse = |a: &[&str]| parse(a.iter().map(|s| (*s).to_owned()));
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
        let n = std::process::id();
        let dir = std::env::temp_dir().join(format!("ew-cli-purge-{tag}-{n}"));
        let _ = std::fs::remove_dir_all(&dir);
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
