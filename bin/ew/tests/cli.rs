// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end tests of the `ew` binary.

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
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
        let p = self.0.join("t.qet");
        std::fs::write(&p, PROJECT).unwrap();
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
    assert!(out.contains("derived: "), "{out}");
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
    let json = &out[out.find('{').expect("json object")..];
    let v: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(v["source"], "t.qet");
    assert!(v["groups"].is_array());
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
