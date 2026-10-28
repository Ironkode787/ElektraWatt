// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.09 part 1 headless (§9 items 1 to 3, §10.1): the PDF export as a job (JB4, JB5's result line) and the
//! journal recovery (JB7) as `ew.uiscript/1` scripts through the built binary, each run twice on its own copy of
//! Golden A with byte-identical transcripts; the export's PDF is the golden digest; the `--record` log of the export
//! script replayed onto a fresh copy gives equal records. The flows with a held gate run in `ew-viewmodel`'s
//! `tests/jobs.rs` (feature `testkit`). A module of `tests/headless.rs`, whose `bin_exe` it uses.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_ui::ew_viewmodel::ew_api::{CmdLog, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/expected");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-jobs-script-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn script(steps: &[(&str, Value)]) -> String {
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"jobs\"}\n");
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

/// Runs `steps` in `run` (a folder with its own config directory), with `--record` into `record` when given.
fn run(run: &Path, name: &str, steps: &[(&str, Value)], record: Option<&Path>) -> Output {
    let file = run.join(format!("{name}.uiscript"));
    std::fs::write(&file, script(steps)).unwrap();
    let mut cmd = Command::new(super::bin_exe());
    cmd.args(["--headless", "--script"]).arg(&file);
    if let Some(r) = record {
        cmd.arg("--record").arg(r);
    }
    cmd.env("EW_CONFIG_DIR", run.join("config"))
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn steps_of(out: &Output) -> Vec<Value> {
    let text = String::from_utf8(out.stdout.clone()).unwrap();
    let mut lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{l}: {e}")))
        .collect();
    lines.remove(0);
    lines
}

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// The export flow in `dir`: a new file, the Jobs pane and the log, a replace, an edit and its save, and last the
/// refused export over the existing file without `replace`.
fn export_steps(dir: &Path) -> Vec<(&'static str, Value)> {
    let project = dir.join("g.ew");
    let pdf = dir.join("out.pdf");
    vec![
        ("file.open", json!({ "dir": path(&project) })),
        // `on`: the workspace of the first run keeps the pane (layouts persist).
        ("jobs.show", json!({"on": true})),
        ("file.export_pdf", json!({ "path": path(&pdf) })),
        ("notifications.show", json!({"on": true})),
        ("ui.describe", json!({})),
        // An edit with stable IDs (a new record's ID would differ on replay).
        (
            "command.run",
            json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-101F1"}],
                "path": "function_text", "value": "Exported twice"}}),
        ),
        ("file.export_pdf", json!({ "path": path(&pdf), "replace": true })),
        ("file.save", json!({})),
        ("ui.describe", json!({})),
        ("file.export_pdf", json!({ "path": path(&pdf) })),
    ]
}

#[test]
fn jb4_the_export_script_is_byte_identical_twice_writes_the_golden_pdf_and_replays() {
    let tmp = Tmp::new();
    let mut transcripts = Vec::new();
    for side in ["a", "b"] {
        let dir = tmp.0.join(side);
        copy_dir(Path::new(GOLDEN_A), &dir.join("g.ew"));
        // The first export, before the edit, is the golden PDF: kept aside before the replace overwrites it.
        let first = run(&dir, "first", &export_steps(&dir)[..3], None);
        assert_eq!(
            first.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&first.stdout)
        );
        let pdf = std::fs::read(dir.join("out.pdf")).unwrap();
        let digest: String = Sha256::digest(&pdf).iter().map(|b| format!("{b:02x}")).collect();
        let expected = std::fs::read_to_string(Path::new(EXPECTED).join("pdf/sha256.txt")).unwrap();
        assert_eq!(digest, expected.trim(), "the job's PDF is the golden PDF");
        std::fs::remove_file(dir.join("out.pdf")).unwrap();
        let record = dir.join("record.ewcmd");
        let out = run(&dir, "export", &export_steps(&dir), Some(&record));
        assert_eq!(out.status.code(), Some(1), "the last step is refused");
        transcripts.push(String::from_utf8(out.stdout.clone()).unwrap());
        let steps = steps_of(&out);
        assert_eq!(steps.len(), 10);
        for s in &steps[..9] {
            assert_eq!(s["outcome"], "done", "{s}");
        }
        assert_eq!(steps[9]["outcome"], "refused");
        assert_eq!(steps[9]["reason"], "reason-file-exists");
        let d = steps[4]["describe"].as_str().unwrap();
        assert!(
            d.contains("jobs running 0\n  job 1 pdf \"out.pdf\" done pages 36 kib "),
            "{d}"
        );
        assert!(
            d.contains("notifications 1\n  notification 1 \"Exported out.pdf: 36 pages, "),
            "{d}"
        );
        assert!(d.contains("\" open"), "{d}");
        assert!(d.contains("| Notifications 1"), "{d}");
        let d = steps[8]["describe"].as_str().unwrap();
        assert!(d.contains("  job 2 pdf \"out.pdf\" done pages 36 kib "), "{d}");
        assert!(d.contains("notification 2 \"Exported out.pdf: 36 pages, "), "{d}");
        // The second export's toast replaced the first (one key).
        assert_eq!(d.matches("\ntoast \"Exported out.pdf").count(), 1, "{d}");
        // No path, no time.
        let all = String::from_utf8_lossy(&out.stdout);
        assert!(!all.contains(&*path(&tmp.0)), "a path in the transcript");
        assert!(!all.contains(".ew-part"), "the sibling is never named");
        assert!(!dir.join(".out.pdf.ew-part").exists());
        // The record replayed onto a fresh copy: the records of the saved project (the export changed none).
        let log = CmdLog::parse(&std::fs::read_to_string(&record).unwrap()).unwrap();
        assert_eq!(log.commands().count(), 1);
        let fresh = dir.join("fresh.ew");
        copy_dir(Path::new(GOLDEN_A), &fresh);
        let (mut replayed, _) = Session::open_read_only(&fresh, "test").unwrap();
        replayed.run_script(&log, "export.ewcmd").unwrap();
        let (saved, _) = Session::open_read_only(&dir.join("g.ew"), "test").unwrap();
        let diff = replayed.diff_from(&saved.world()).unwrap();
        assert!(diff.model.is_empty(), "{:?}", diff.model);
    }
    assert_eq!(transcripts[0], transcripts[1], "two runs give the same bytes");
}

#[test]
fn jb7_the_recovery_script_is_byte_identical_twice() {
    let tmp = Tmp::new();
    let mut transcripts = Vec::new();
    for side in ["a", "b"] {
        let dir = tmp.0.join(side);
        let project = dir.join("g.ew");
        copy_dir(Path::new(GOLDEN_A), &project);
        // A run that ends with unsaved changes exits 1 and keeps its journal (ID-475).
        let crash = run(
            &dir,
            "crash",
            &[
                ("file.open", json!({ "dir": path(&project) })),
                (
                    "command.run",
                    json!({"cmd": "page.create", "params": {"number": 996}}),
                ),
            ],
            None,
        );
        assert_eq!(crash.status.code(), Some(1));
        let steps = [
            ("file.open", json!({ "dir": path(&project) })),
            ("ui.describe", json!({})),
            ("page.next", json!({})),
            ("journal.recover", json!({})),
            ("ui.describe", json!({})),
            ("file.save", json!({})),
        ];
        let out = run(&dir, "recover", &steps, None);
        let lines = steps_of(&out);
        assert_eq!(out.status.code(), Some(1), "the refused step");
        transcripts.push(String::from_utf8(out.stdout.clone()).unwrap());
        assert_eq!(lines[0]["state"]["access"], "read-only");
        let d = lines[1]["describe"].as_str().unwrap();
        assert!(
            d.contains(
                "dialog \"Recover unsaved changes\"\n  label \"g.ew has unsaved changes from a session that ended \
                 unexpectedly.\"\n  recovery transactions 1 base matches\n  button \"Recover\""
            ),
            "{d}"
        );
        assert_eq!(lines[2]["outcome"], "refused");
        assert_eq!(lines[2]["reason"], "reason-dialog-open");
        // Without --keep-going the run ends at the refused step: the recovery runs in a second script.
        assert_eq!(lines.len(), 3);
        let out = run(
            &dir,
            "recover2",
            &[
                ("file.open", json!({ "dir": path(&project) })),
                ("journal.recover", json!({})),
                ("ui.describe", json!({})),
                ("file.save", json!({})),
            ],
            None,
        );
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        transcripts.push(String::from_utf8(out.stdout.clone()).unwrap());
        let lines = steps_of(&out);
        assert_eq!(lines[1]["state"]["access"], "edit");
        assert_eq!(lines[1]["state"]["dirty"], true);
        assert_eq!(
            lines[1]["state"]["toast"]["text"],
            "Recovered 1 change; save to keep it"
        );
        let d = lines[2]["describe"].as_str().unwrap();
        assert!(d.contains("page \"996\""), "the recovered page: {d}");
        assert_eq!(lines[3]["state"]["dirty"], false);
    }
    assert_eq!(transcripts[0], transcripts[2], "two runs give the same bytes");
    assert_eq!(transcripts[1], transcripts[3], "two runs give the same bytes");
}

/// Review finding 3 of cycle 30: a script whose last step is an export that does not wait (`wait: false`) and has no
/// `jobs.wait` still writes its file, because the end of the run waits for its jobs; two runs give the same
/// transcript and the same PDF (the golden digest), and no sibling is left.
#[test]
fn jb4_an_export_that_did_not_wait_is_written_at_the_end_of_the_run() {
    let tmp = Tmp::new();
    let mut transcripts = Vec::new();
    let mut pdfs = Vec::new();
    for side in ["a", "b"] {
        let dir = tmp.0.join(side);
        copy_dir(Path::new(GOLDEN_A), &dir.join("g.ew"));
        let pdf = dir.join("out.pdf");
        let steps = [
            ("file.open", json!({ "dir": path(&dir.join("g.ew")) })),
            ("file.export_pdf", json!({ "path": path(&pdf), "wait": false })),
        ];
        let out = run(&dir, "nowait", &steps, None);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.stderr.is_empty(), "{}", String::from_utf8_lossy(&out.stderr));
        let lines = steps_of(&out);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1]["outcome"], "done");
        let bytes = std::fs::read(&pdf).expect("the PDF is written");
        let digest: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let expected = std::fs::read_to_string(Path::new(EXPECTED).join("pdf/sha256.txt")).unwrap();
        assert_eq!(digest, expected.trim(), "the job's PDF is the golden PDF");
        assert!(!dir.join(".out.pdf.ew-part").exists(), "no sibling");
        transcripts.push(String::from_utf8(out.stdout.clone()).unwrap());
        pdfs.push(bytes);
    }
    assert_eq!(
        transcripts[0], transcripts[1],
        "two runs give the same transcript"
    );
    assert_eq!(pdfs[0], pdfs[1], "two runs give the same PDF");
}
