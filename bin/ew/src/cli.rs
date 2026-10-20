// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Argument parsing and the commands of the `ew` CLI.

use std::path::{Path, PathBuf};

use ew_api::import::migration::ReportFormat;
use ew_api::import::{StarterLibrary, import_qet};

/// The usage text.
pub const USAGE: &str = "usage:
  ew --version | -V
  ew import qet <file.qet> [--content <dir>] [--report md|csv|json] [--report-out <file>]
  ew help | --help | -h";

/// A parsed command line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Command {
    /// Print the version.
    Version,
    /// Print the usage.
    Help,
    /// Import a QElectroTech project.
    ImportQet(ImportQetArgs),
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
    }))
}

fn value(it: &mut impl Iterator<Item = String>, opt: &str) -> Result<String, String> {
    it.next().ok_or_else(|| format!("`{opt}` needs a value"))
}

/// Runs `cmd` and returns the exit code (0 success, 1 model check failed, 2 usage, I/O and import errors).
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
    }
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
    println!(
        "imported {name}: {} pages, {} devices ({} strips, {} terminals), {} functions, {} representations, {} connections, {} junctions",
        c.pages, c.devices, c.strips, c.terminals, c.functions, c.representations, c.connections, c.junctions
    );
    println!(
        "report: {} findings, {} lost connections",
        report.total, report.lost_connections
    );
    let mut code = 0;
    match &imported.session {
        Ok(s) => println!("derived: {} pages in page order", s.page_labels().len()),
        Err(msg) => {
            println!("model check failed: {msg}");
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
