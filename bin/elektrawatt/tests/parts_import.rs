// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-410 through the built binary: `--headless --script` runs `file.new`, `parts.import path=<golden parts.csv>` and
//! `preview.apply`; the `--record` log has one `parts.import` command; two runs on equal inputs give byte-identical
//! stdout transcripts.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::{Value, json};

const GOLDEN_PARTS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../golden/J-E-2416/tables/parts.csv"
);

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-parts-import-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The binary of this package: nextest's path when it is set, else the compile-time one.
// Allowed: the program boundary of a test — nextest's binary path is read once, never persisted or printed.
#[allow(clippy::disallowed_methods)]
fn bin_exe() -> PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_elektrawatt")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_elektrawatt"))) // bin-exe-fallback
}

fn script(steps: &[(&str, Value)]) -> String {
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"test\"}\n");
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

/// One run in a fresh folder: the output and the record file's text.
fn run(project_dir: &Path, tmp: &Tmp) -> (Output, String) {
    let file = tmp.path("parts.jsonl");
    std::fs::write(
        &file,
        script(&[
            (
                "file.new",
                json!({"dir": project_dir.to_string_lossy(), "template": "builtin"}),
            ),
            ("parts.import", json!({"path": GOLDEN_PARTS})),
            ("preview.apply", json!({})),
            ("ui.describe", json!({})),
            ("file.save", json!({})),
            ("file.close", json!({})),
        ]),
    )
    .unwrap();
    let record = tmp.path("r.ewcmd");
    let out = Command::new(bin_exe())
        .args(["--headless", "--script"])
        .arg(&file)
        .arg("--record")
        .arg(&record)
        .env("EW_CONFIG_DIR", tmp.path("config"))
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let text = std::fs::read_to_string(&record).unwrap_or_default();
    (out, text)
}

#[test]
fn a_script_imports_the_golden_parts_and_records_one_command() {
    let tmp = Tmp::new();
    let (out, record) = run(&tmp.path("proj.ew"), &tmp);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let steps: Vec<Value> = stdout
        .lines()
        .skip(1)
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        steps[1]["state"]["card"]["origin"], "import",
        "the card waits for Apply"
    );
    assert_eq!(
        steps[2]["state"]["banner"],
        "Imported 29 parts from parts.csv (0 skipped)."
    );
    assert!(steps.iter().all(|s| s["outcome"] == "done"), "{stdout}");
    assert!(
        steps[3]["describe"]
            .as_str()
            .unwrap()
            .contains("item parts.import \"Parts table…\" enabled")
    );
    let lines: Vec<&str> = record
        .lines()
        .filter(|l| l.contains("\"cmd\":\"parts.import\""))
        .collect();
    assert_eq!(lines.len(), 1, "{record}");
}

#[test]
fn two_runs_give_byte_identical_transcripts() {
    let a = Tmp::new();
    let b = Tmp::new();
    // The same folder name in two temporary directories.
    let (first, _) = run(&a.path("proj.ew"), &a);
    let (second, _) = run(&b.path("proj.ew"), &b);
    assert_eq!(first.status.code(), Some(0));
    assert_eq!(second.status.code(), Some(0));
    assert!(!first.stdout.is_empty());
    assert_eq!(first.stdout, second.stdout);
}
