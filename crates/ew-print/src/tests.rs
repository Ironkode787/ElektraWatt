// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The adapters with a stand-in program (WP-S3.07 PR-4): this test binary itself, run with `--exact
//! tests::stand_in --nocapture --` before the adapter's arguments, records them in `<last argument>.args`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

/// The stand-in: run by an adapter (its arguments follow a `--`), it writes them to `<last argument>.args`, one per
/// line, then `exists <bool>` for the last argument as a file; with the title `fail` it fails. Run by the harness
/// (no `--`), it does nothing.
#[test]
fn stand_in() {
    let args: Vec<String> = std::env::args().collect();
    let Some(at) = args.iter().position(|a| a == "--") else {
        return;
    };
    let own = &args[at + 1..];
    let last = own.last().expect("an argument");
    let mut text = own.join("\n");
    text += &format!("\nexists {}\n", Path::new(last).is_file());
    std::fs::write(format!("{last}.args"), text).unwrap();
    assert!(!own.iter().any(|a| a == "fail"), "asked to fail");
}

/// This binary as the stand-in program.
fn stand_in_program() -> Program {
    Program {
        path: std::env::current_exe().unwrap(),
        leading: ["--exact", "tests::stand_in", "--nocapture", "--"]
            .map(OsString::from)
            .to_vec(),
        check_status: true,
    }
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh empty directory, removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new() -> Dir {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-print-test-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Dir(dir)
    }

    /// The names of the files in the directory, sorted.
    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn job(title: &str, printer: Option<&str>) -> PrintJob {
    PrintJob {
        title: title.to_owned(),
        pdf: b"%PDF-1.7 a print set".to_vec(),
        pages: Vec::new(),
        printer: printer.map(str::to_owned),
        monochrome: false,
    }
}

/// The recorded arguments of the one `.args` file in `dir`, and that file's name.
fn recorded(dir: &Dir) -> (String, Vec<String>) {
    let names = dir.names();
    let args: Vec<&String> = names.iter().filter(|n| n.ends_with(".args")).collect();
    assert_eq!(args.len(), 1, "{names:?}");
    let text = std::fs::read_to_string(dir.0.join(args[0])).unwrap();
    (args[0].clone(), text.lines().map(str::to_owned).collect())
}

/// PR-4: `lp -d <printer> -t <title> <file>`, the file present while `lp` runs and gone afterwards; without a printer
/// `lp -t <title> <file>`.
#[test]
fn lp_is_run_with_the_printer_the_title_and_the_file_which_is_removed_afterwards() {
    for printer in [Some("office"), None] {
        let dir = Dir::new();
        let lp = Lp {
            program: stand_in_program(),
            temp_dir: dir.0.clone(),
        };
        assert_eq!(lp.name(), "lp");
        let outcome = lp.print(&job("J-E-2416 section 1", printer)).unwrap();
        let PrintOutcome::Spooled { printer: named, .. } = outcome else {
            panic!("spooled: {outcome:?}")
        };
        assert_eq!(named.as_deref(), printer);
        let (name, args) = recorded(&dir);
        let file = dir.0.join(name.trim_end_matches(".args"));
        let file = file.display().to_string();
        let mut want: Vec<String> = Vec::new();
        if let Some(p) = printer {
            want.extend(["-d".to_owned(), p.to_owned()]);
        }
        want.extend([
            "-t".to_owned(),
            "J-E-2416 section 1".to_owned(),
            file,
            "exists true".to_owned(),
        ]);
        assert_eq!(args, want);
        assert_eq!(dir.names(), [name], "the print file is removed");
    }
}

/// A missing `lp` and a failing one are errors in words, and the print file is removed either way.
#[test]
fn a_missing_or_failing_lp_is_refused_and_leaves_no_file() {
    let dir = Dir::new();
    let missing = Lp {
        program: Program::named("ew-print-test-no-such-program"),
        temp_dir: dir.0.clone(),
    };
    let e = missing.print(&job("t", None)).unwrap_err();
    assert_eq!(
        e.to_string(),
        "printing needs the CUPS command `lp`; install cups-client or print from the window"
    );
    assert!(dir.names().is_empty());
    let failing = Lp {
        program: stand_in_program(),
        temp_dir: dir.0.clone(),
    };
    let e = failing.print(&job("fail", None)).unwrap_err();
    let PrintError::Failed { status, .. } = &e else {
        panic!("failed: {e:?}")
    };
    assert_eq!(status, "exit code 101");
    let names = dir.names();
    assert!(
        names.len() == 1 && names[0].ends_with(".pdf.args"),
        "only the stand-in's record: {names:?}"
    );
}

/// PR-4: the viewer writes `<dir>/<project>-print.pdf` and runs the opener on it; a printer is refused before
/// anything is written.
#[test]
fn the_viewer_writes_the_file_and_opens_it() {
    let dir = Dir::new();
    let viewer = Viewer {
        opener: stand_in_program(),
        dir: dir.0.clone(),
        project: "J-E-2416".to_owned(),
    };
    assert_eq!(viewer.name(), "viewer");
    let e = viewer.print(&job("t", Some("office"))).unwrap_err();
    assert_eq!(
        e.to_string(),
        "`--printer` needs `--via lp` or `--via windows`; the viewer asks"
    );
    assert!(dir.names().is_empty());
    let file = dir.0.join("J-E-2416-print.pdf");
    assert_eq!(
        viewer.print(&job("t", None)).unwrap(),
        PrintOutcome::Opened { file: file.clone() }
    );
    assert_eq!(std::fs::read(&file).unwrap(), job("t", None).pdf);
    let (_, args) = recorded(&dir);
    assert_eq!(args, [file.display().to_string(), "exists true".to_owned()]);
    assert_eq!(dir.names(), ["J-E-2416-print.pdf", "J-E-2416-print.pdf.args"]);
}

/// The viewer's file name keeps the project's name inside the directory.
#[test]
fn the_viewer_file_name_never_leaves_its_directory() {
    let viewer = |project: &str| Viewer::new(PathBuf::from("d"), project).file();
    assert_eq!(viewer("J-E-2416"), Path::new("d").join("J-E-2416-print.pdf"));
    assert_eq!(viewer("../x y"), Path::new("d").join("_x_y-print.pdf"));
    assert_eq!(viewer(""), Path::new("d").join("project-print.pdf"));
}

/// Every refusal of the adapters, whole.
#[test]
fn the_errors_read_as_words() {
    let failed = |message: &str| PrintError::Failed {
        program: "lp".to_owned(),
        status: "exit code 1".to_owned(),
        message: message.to_owned(),
    };
    assert_eq!(
        failed("lp: The printer or class does not exist.").to_string(),
        "`lp` failed with exit code 1: lp: The printer or class does not exist."
    );
    assert_eq!(failed("").to_string(), "`lp` failed with exit code 1");
    assert_eq!(
        PrintError::Spawn {
            program: "xdg-open".to_owned(),
            reason: "permission denied".to_owned()
        }
        .to_string(),
        "`xdg-open` could not be started: permission denied"
    );
    assert_eq!(bounded(&[b'x'; 5000]).len(), MAX_MESSAGE);
}
