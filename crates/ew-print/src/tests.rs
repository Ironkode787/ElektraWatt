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

/// The viewer on `dir` with the stand-in as its opener.
fn stand_in_viewer(dir: &Dir) -> Viewer {
    Viewer {
        opener: stand_in_program(),
        dir: dir.0.clone(),
        project: "J-E-2416".to_owned(),
    }
}

/// The file the viewer's set goes to with the counter at `n`.
fn viewer_file(dir: &Dir, n: u32) -> PathBuf {
    dir.0
        .join(format!("J-E-2416-print-{}-{n}.pdf", std::process::id()))
}

/// The counter in an opened file's name.
fn counter_of(outcome: &PrintOutcome) -> u32 {
    let PrintOutcome::Opened { file } = outcome else {
        panic!("opened: {outcome:?}")
    };
    let name = file.file_name().unwrap().to_string_lossy();
    let prefix = format!("J-E-2416-print-{}-", std::process::id());
    name.strip_prefix(&prefix)
        .and_then(|n| n.strip_suffix(".pdf"))
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("a fresh viewer file: {name}"))
}

/// A set's PDF with its own bytes.
fn job_with(pdf: &[u8]) -> PrintJob {
    PrintJob {
        pdf: pdf.to_vec(),
        ..job("t", None)
    }
}

/// PR-4 and D9 (ID-1144): the viewer writes each set to a fresh file `<dir>/<project>-print-<process>-<n>.pdf` and runs
/// the opener on it, the file left for the viewer; a second print goes to a second file and leaves the first as it
/// was; a printer is refused before anything is written.
#[test]
fn the_viewer_writes_each_set_to_a_fresh_file_and_opens_it() {
    let dir = Dir::new();
    let viewer = stand_in_viewer(&dir);
    assert_eq!(viewer.name(), "viewer");
    let e = viewer.print(&job("t", Some("office"))).unwrap_err();
    assert_eq!(
        e.to_string(),
        "`--printer` needs `--via lp`; with the viewer, choose the printer in its print dialog"
    );
    assert!(dir.names().is_empty());
    let first = viewer.print(&job_with(b"%PDF-1.7 the first set")).unwrap();
    let n = counter_of(&first);
    let one = viewer_file(&dir, n);
    assert_eq!(first, PrintOutcome::Opened { file: one.clone() });
    let (_, args) = recorded(&dir);
    assert_eq!(args, [one.display().to_string(), "exists true".to_owned()]);
    let second = viewer.print(&job_with(b"%PDF-1.7 the second set")).unwrap();
    let two = viewer_file(&dir, counter_of(&second));
    assert_ne!(one, two);
    assert_eq!(second, PrintOutcome::Opened { file: two.clone() });
    assert_eq!(std::fs::read(&one).unwrap(), b"%PDF-1.7 the first set");
    assert_eq!(std::fs::read(&two).unwrap(), b"%PDF-1.7 the second set");
    let name = |p: &Path| p.file_name().unwrap().to_string_lossy().into_owned();
    let mut want = vec![
        name(&one),
        format!("{}.args", name(&one)),
        name(&two),
        format!("{}.args", name(&two)),
    ];
    want.sort();
    assert_eq!(dir.names(), want, "both files stay for the viewer");
}

/// D9 (ID-1144): a file already at the next name is left as it is and the set goes to the one after; when every
/// attempt finds its name taken, the set is refused with the last name and the system's reason.
#[test]
fn the_viewer_never_writes_over_a_file_at_its_next_name() {
    let dir = Dir::new();
    let viewer = stand_in_viewer(&dir);
    let n = counter_of(&viewer.print(&job("t", None)).unwrap());
    let planted = viewer_file(&dir, n + 1);
    std::fs::write(&planted, b"another file").unwrap();
    let after = viewer.print(&job("t", None)).unwrap();
    assert_eq!(
        after,
        PrintOutcome::Opened {
            file: viewer_file(&dir, n + 2)
        }
    );
    assert_eq!(std::fs::read(&planted).unwrap(), b"another file");
    assert_eq!(
        std::fs::read(viewer_file(&dir, n + 2)).unwrap(),
        job("t", None).pdf
    );
    // Every name of the next attempts taken: the set is refused, and nothing is written over.
    let from = n + 3;
    for k in from..from + NAME_ATTEMPTS {
        std::fs::write(viewer_file(&dir, k), b"another file").unwrap();
    }
    let before = dir.names();
    let last = viewer_file(&dir, from + NAME_ATTEMPTS - 1);
    let reason = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&last)
        .unwrap_err()
        .to_string();
    assert_eq!(
        viewer.print(&job("t", None)).unwrap_err(),
        PrintError::Io { path: last, reason }
    );
    assert_eq!(dir.names(), before);
    for k in from..from + NAME_ATTEMPTS {
        assert_eq!(std::fs::read(viewer_file(&dir, k)).unwrap(), b"another file");
    }
}

/// D9 (ID-1144), Unix: a symbolic link at the viewer's next name is not followed (its target keeps its bytes, the
/// link stays a link) and the set goes to the name after; the viewer's file and the shared fresh file `lp` prints
/// are readable by their owner only (mode `0o600`).
#[cfg(unix)]
#[test]
fn the_print_files_follow_no_link_and_are_the_owners_alone() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = Dir::new();
    let viewer = stand_in_viewer(&dir);
    let n = counter_of(&viewer.print(&job("t", None)).unwrap());
    let target = dir.0.join("target.pdf");
    std::fs::write(&target, b"the target").unwrap();
    let link = viewer_file(&dir, n + 1);
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let file = viewer_file(&dir, n + 2);
    assert_eq!(
        viewer.print(&job("t", None)).unwrap(),
        PrintOutcome::Opened { file: file.clone() }
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"the target");
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&file), 0o600);
    let lp_file = fresh_file(&dir.0, "ew-print", b"%PDF").unwrap();
    assert_eq!(mode(&lp_file), 0o600);
}

/// The viewer's file names keep the project's name inside the directory.
#[test]
fn the_viewer_file_name_never_leaves_its_directory() {
    let viewer = |project: &str| Viewer::new(PathBuf::from("d"), project).stem();
    assert_eq!(viewer("J-E-2416"), "J-E-2416-print");
    assert_eq!(viewer("../x y"), "_x_y-print");
    assert_eq!(viewer(""), "project-print");
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
    assert_eq!(PrintError::Cancelled.to_string(), "printing was cancelled");
    assert_eq!(
        portal_down().to_string(),
        "the system print dialog could not be used: org.freedesktop.portal.Desktop was not provided by any .service files"
    );
}

// WP-S3.07 part 3 (D11, PW-3; ruling 3 of ID-1180): the fallback chain.

type Calls = std::sync::Arc<std::sync::Mutex<Vec<PrintJob>>>;

/// A test double of the adapter trait: answers with `answer` and records every job it was called with; it panics on
/// the title `panic`.
struct Double {
    name: &'static str,
    answer: Result<PrintOutcome, PrintError>,
    calls: Calls,
}

impl Double {
    fn new(name: &'static str, answer: Result<PrintOutcome, PrintError>) -> Double {
        Double {
            name,
            answer,
            calls: Calls::default(),
        }
    }

    fn calls(&self) -> Calls {
        std::sync::Arc::clone(&self.calls)
    }
}

impl PrintAdapter for Double {
    fn name(&self) -> &'static str {
        self.name
    }

    fn print(&self, job: &PrintJob) -> Result<PrintOutcome, PrintError> {
        self.calls.lock().unwrap().push(job.clone());
        assert!(job.title != "panic", "asked to panic");
        self.answer.clone()
    }
}

fn portal_down() -> PrintError {
    PrintError::Portal {
        reason: "org.freedesktop.portal.Desktop was not provided by any .service files".to_owned(),
    }
}

fn spooled() -> PrintOutcome {
    PrintOutcome::Spooled {
        printer: None,
        message: String::new(),
    }
}

/// PW-3: the portal adapter is called with the set; when it fails, `lp` prints it (the stand-in records its
/// arguments) and the result carries D11's one notice, naming the failed portal and `lp`; the viewer is not run.
#[test]
fn a_failing_portal_falls_back_to_lp_with_one_notice() {
    let dir = Dir::new();
    let portal = Double::new("portal", Err(portal_down()));
    let portal_calls = portal.calls();
    let chain = Chain::new(vec![
        Box::new(portal),
        Box::new(Lp {
            program: stand_in_program(),
            temp_dir: dir.0.clone(),
        }),
        Box::new(stand_in_viewer(&dir)),
    ]);
    assert_eq!(chain.names(), ["portal", "lp", "viewer"]);
    let set = job("J-E-2416", None);
    let printed = chain.print(&set).unwrap();
    assert_eq!(*portal_calls.lock().unwrap(), std::slice::from_ref(&set), "the portal got the set");
    assert_eq!(printed.adapter, "lp");
    assert_eq!(
        printed.fallback,
        Some(Fallback {
            failed: vec![("portal", portal_down())],
            used: "lp",
        })
    );
    assert!(
        matches!(printed.outcome, PrintOutcome::Spooled { printer: None, .. }),
        "{:?}",
        printed.outcome
    );
    let (name, args) = recorded(&dir);
    assert_eq!(args[..2], ["-t".to_owned(), "J-E-2416".to_owned()]);
    assert_eq!(dir.names(), [name], "lp's print file is removed; the viewer wrote none");
}

/// A portal that prints needs no notice; a cancel in the print dialog ends the chain with nothing printed; a
/// panicking adapter fails like any other; when all fail, every failure is listed.
#[test]
fn the_chain_stops_at_the_first_print_and_at_a_cancel() {
    let after = Double::new("lp", Ok(spooled()));
    let after_calls = after.calls();
    let chain = Chain::new(vec![Box::new(Double::new("portal", Ok(spooled()))), Box::new(after)]);
    let printed = chain.print(&job("t", None)).unwrap();
    assert_eq!((printed.adapter, printed.fallback), ("portal", None));
    assert!(after_calls.lock().unwrap().is_empty());

    let after = Double::new("lp", Ok(spooled()));
    let after_calls = after.calls();
    let chain = Chain::new(vec![
        Box::new(Double::new("portal", Err(PrintError::Cancelled))),
        Box::new(after),
    ]);
    assert_eq!(
        chain.print(&job("t", None)),
        Err(vec![("portal", PrintError::Cancelled)])
    );
    assert!(after_calls.lock().unwrap().is_empty(), "a cancel prints nothing");

    let chain = Chain::new(vec![
        Box::new(Double::new("portal", Ok(spooled()))),
        Box::new(Double::new("viewer", Ok(spooled()))),
    ]);
    let failed = chain.print(&job("panic", None)).unwrap_err();
    let stopped = |program: &str| PrintError::Spawn {
        program: program.to_owned(),
        reason: "it stopped unexpectedly".to_owned(),
    };
    assert_eq!(failed, [("portal", stopped("portal")), ("viewer", stopped("viewer"))]);
    assert_eq!(
        failed[0].1.to_string(),
        "`portal` could not be started: it stopped unexpectedly"
    );
    assert_eq!(Chain::new(Vec::new()).print(&job("t", None)), Err(Vec::new()));
}

/// The window's chains: on Linux the portal, `lp`, then the viewer; elsewhere the viewer; Open in PDF viewer is the
/// viewer alone.
#[test]
fn the_system_chain_is_portal_lp_viewer_on_linux_and_the_viewer_elsewhere() {
    let dir = Dir::new();
    let system = Chain::system(dir.0.clone(), "J-E-2416");
    if cfg!(target_os = "linux") {
        assert_eq!(system.names(), ["portal", "lp", "viewer"]);
    } else {
        assert_eq!(system.names(), ["viewer"]);
    }
    assert_eq!(Chain::viewer(dir.0.clone(), "J-E-2416").names(), ["viewer"]);
}
