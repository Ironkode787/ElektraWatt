// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Argument parsing and the commands of the `ew` CLI.

use std::path::{Path, PathBuf};

use ew_api::checks::{CheckMode, report, scheme};
use ew_api::import::Severity;
use ew_api::import::migration::ReportFormat;
use ew_api::import::{StarterLibrary, import_qet};

/// The usage text.
pub const USAGE: &str = "usage:
  ew --version | -V
  ew import qet <file.qet> [--content <dir>] [--report md|csv|json] [--report-out <file>] [--strict]
  ew check [<project-dir>] [--scheme <name>] [--mode release|online] [--format text|md|json] [--out <file>]
  ew sql [<project-dir>] \"<select>\" [--format csv|json] [--timeout <seconds>] [--out <file>]
  ew sql [<project-dir>] --file <query.sql> [--format csv|json] [--timeout <seconds>] [--out <file>]
  ew sql dump [<project-dir>] <out.sqlite>
  ew sql schema
  ew help | --help | -h

`import qet` exits 0 when the import completes, even with findings; `--strict` exits 1 on lost connections or
error findings. With `--report` and no `--report-out` stdout is exactly the report; the summary goes to stderr.

`check` runs a check scheme (default `pre-delivery`, mode `release`, format `text`) over a project directory
(default: the nearest ancestor of the current directory holding `manifest.json`) and exits 0 with no unwaived
errors, 1 with errors, 2 on usage and open errors.

`sql` runs one read-only SELECT against the SQL projection of a project (the `api_v1_*` views; `ew sql schema` prints
them) in the sandbox of doc 17 §6.2, with the query given inline or from `--file`. The result is CSV (default) or
JSON on stdout, the row count on stderr; with `--out` it goes to a file. `--timeout` sets a deadline in seconds.
`--project <dir>` may replace the positional project directory. `sql dump` writes the projection as a SQLite
database file. `sql` exits 0 on success and 2 on everything else (usage, open and SQL errors, denials, limits).";

/// A parsed command line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Command {
    /// Print the version.
    Version,
    /// Print the usage.
    Help,
    /// Import a QElectroTech project.
    ImportQet(ImportQetArgs),
    /// Check a project.
    Check(CheckArgs),
    /// Run a read-only query on the SQL projection.
    Sql(SqlArgs),
    /// Write the SQL projection as a SQLite file.
    SqlDump(SqlDumpArgs),
    /// Print the SQL schema.
    SqlSchema,
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
    /// The scheme name (`--scheme`).
    pub scheme: String,
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
            Some(other) => Err(format!("unknown import format `{other}`")),
            None => Err("`import` needs a format (qet)".to_owned()),
        },
        "check" => parse_check(it),
        "sql" => parse_sql(it),
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

fn parse_check(mut it: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut project = None;
    let mut scheme_name = "pre-delivery".to_owned();
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
                scheme_name = name;
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
        Command::Check(a) => run_check(&a),
        Command::Sql(a) => run_sql(&a),
        Command::SqlDump(a) => run_sql_dump(&a),
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
    let Some(sch) = scheme(&a.scheme) else {
        eprintln!("error: unknown scheme `{}`", a.scheme);
        return 2;
    };
    let (session, opened) = match ew_api::Session::open_read_only(&dir, env!("CARGO_PKG_VERSION")) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return 2;
        }
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

fn run_import_qet(a: &ImportQetArgs) -> i32 {
    let content = a.content.clone().or_else(|| {
        std::env::var_os("EW_CONTENT_DIR")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    });
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
        assert_eq!(a.scheme, "pre-delivery");
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
        assert_eq!(b.scheme, "drafting");
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
    fn import_qet_errors() {
        assert!(p(&["import"]).is_err());
        assert!(p(&["import", "dxf", "a"]).is_err());
        assert!(p(&["import", "qet"]).is_err());
        assert!(p(&["import", "qet", "a", "b"]).is_err());
        assert!(p(&["import", "qet", "a", "--bogus"]).is_err());
        assert!(p(&["import", "qet", "a", "--report"]).is_err());
        assert!(p(&["import", "qet", "a", "--report", "xml"]).is_err());
    }
}
