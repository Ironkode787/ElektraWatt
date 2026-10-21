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
  ew help | --help | -h

`import qet` exits 0 when the import completes, even with findings; `--strict` exits 1 on lost connections or
error findings. With `--report` and no `--report-out` stdout is exactly the report; the summary goes to stderr.

`check` runs a check scheme (default `pre-delivery`, mode `release`, format `text`) over a project directory
(default: the nearest ancestor of the current directory holding `manifest.json`) and exits 0 with no unwaived
errors, 1 with errors, 2 on usage and open errors.";

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
    }
}

/// The nearest ancestor of `start` (itself included) that holds `manifest.json` (doc 17 §3.1).
fn find_project(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|d| d.join("manifest.json").is_file())
        .map(Path::to_path_buf)
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
    if !opened.findings.is_empty() {
        eprintln!("warning: {} integrity findings on opening", opened.findings.len());
    }
    let result = session.check(sch, a.mode);
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
