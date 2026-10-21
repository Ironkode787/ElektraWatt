// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end tests of the `ew` binary.

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
const LOST: &str = r#"<project version="0.100" title="T"><diagram order="1" folio="1" title="P"><elements/><conductors><conductor terminal1="999" terminal2="998"/></conductors></diagram></project>"#;
const PROJECT: &str = r#"<project version="0.100" title="T"><diagram order="1" folio="1" title="P"><elements/><conductors/></diagram></project>"#;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-cli-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    fn project(&self) -> PathBuf {
        self.project_with(PROJECT)
    }

    fn project_with(&self, xml: &str) -> PathBuf {
        let p = self.0.join("t.qet");
        std::fs::write(&p, xml).unwrap();
        p
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn ew(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ew"))
        .args(args)
        .env_remove("EW_CONTENT_DIR")
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn version() {
    let o = ew(&["--version".as_ref()]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(stdout(&o), format!("ew {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn import_summary_with_content() {
    let t = Tmp::new();
    let f = t.project();
    let o = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        f.as_os_str(),
        "--content".as_ref(),
        CONTENT.as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let out = stdout(&o);
    assert!(out.starts_with("imported t.qet:"), "{out}");
    assert!(out.contains("report:"), "{out}");
    assert!(out.contains("derived: 1 pages"), "{out}");
    assert!(
        !out.contains(&t.0.to_string_lossy().into_owned()),
        "the name is never a path"
    );
}

#[test]
fn import_without_content_warns() {
    let t = Tmp::new();
    let f = t.project();
    let o = ew(&["import".as_ref(), "qet".as_ref(), f.as_os_str()]);
    assert_eq!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stderr).contains("warning: no starter content"));
}

#[test]
fn report_json_to_stdout() {
    let t = Tmp::new();
    let f = t.project();
    let o = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        f.as_os_str(),
        "--content".as_ref(),
        CONTENT.as_ref(),
        "--report".as_ref(),
        "json".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(0));
    let out = stdout(&o);
    let v: serde_json::Value = serde_json::from_str(&out).expect("stdout is exactly the report");
    assert_eq!(v["source"], "t.qet");
    assert!(v["groups"].is_array());
    assert!(v["live"].is_array());
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("imported t.qet:"), "{err}");
    assert!(err.contains("derived: 1 pages"), "{err}");
}

#[test]
fn strict_passes_clean_project() {
    let t = Tmp::new();
    let f = t.project();
    let o = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        f.as_os_str(),
        "--strict".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn strict_fails_on_error_finding_lenient_does_not() {
    let t = Tmp::new();
    let f = t.project_with(LOST);
    let lenient = ew(&["import".as_ref(), "qet".as_ref(), f.as_os_str()]);
    assert_eq!(
        lenient.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&lenient.stderr)
    );
    let strict = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        f.as_os_str(),
        "--strict".as_ref(),
    ]);
    assert_eq!(strict.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&strict.stderr).contains("strict:"));
}

#[test]
fn report_csv_to_file() {
    let t = Tmp::new();
    let f = t.project();
    let out_file = t.0.join("report.csv");
    let o = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        f.as_os_str(),
        "--report".as_ref(),
        "csv".as_ref(),
        "--report-out".as_ref(),
        out_file.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0));
    assert!(stdout(&o).contains(&format!("report written to {}", out_file.display())));
    let text = std::fs::read_to_string(&out_file).unwrap();
    assert!(text.starts_with("code,severity"), "{text}");
}

#[test]
fn missing_file_is_exit_2() {
    let t = Tmp::new();
    let missing = t.0.join("nope.qet");
    let o = ew(&["import".as_ref(), "qet".as_ref(), missing.as_os_str()]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("error:"));
}

#[test]
fn usage_errors_are_exit_2() {
    assert_eq!(ew(&["import".as_ref(), "qet".as_ref()]).status.code(), Some(2));
    assert_eq!(ew(&["bogus".as_ref()]).status.code(), Some(2));
    assert_eq!(ew(&[]).status.code(), Some(2));
}

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

#[test]
fn check_golden_a_json() {
    let o = ew(&[
        "check".as_ref(),
        GOLDEN_A.as_ref(),
        "--format".as_ref(),
        "json".as_ref(),
    ]);
    let code = o.status.code();
    assert!(
        code == Some(0) || code == Some(1),
        "{code:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["$schema"], "ew.check-result/1");
    assert_eq!(v["scheme"], "pre-delivery");
    assert_eq!(v["exit_code"].as_i64(), code.map(i64::from));
    assert!(!std::path::Path::new(GOLDEN_A).join(".~ew.lock").exists());
}

#[test]
fn check_markdown_title() {
    let o = ew(&[
        "check".as_ref(),
        GOLDEN_A.as_ref(),
        "--format".as_ref(),
        "md".as_ref(),
    ]);
    assert!(matches!(o.status.code(), Some(0 | 1)));
    assert!(
        stdout(&o).starts_with("# Check report: pre-delivery"),
        "{}",
        stdout(&o)
    );
}

#[test]
fn check_unknown_scheme_is_exit_2() {
    let o = ew(&[
        "check".as_ref(),
        GOLDEN_A.as_ref(),
        "--scheme".as_ref(),
        "bogus".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(2));
}

#[test]
fn check_missing_project_is_exit_2() {
    let t = Tmp::new();
    let missing = t.0.join("nope.ew");
    let o = ew(&["check".as_ref(), missing.as_os_str()]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("error:"));
}
