// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.09 part 3 headless (§9 items 1 and 3, §10.3 PV2, PV5, PV7): Golden B through the renumber actions and the
//! Preview tab, with the history panel open, as an `ew.uiscript/1` script through the built binary, run twice on its own
//! copy of Golden A with byte-identical transcripts; the `--record` log replayed onto a fresh copy gives the records of
//! the saved project. A module of `tests/headless.rs`, whose `bin_exe` it uses.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_ui::ew_viewmodel::ew_api::{CmdLog, Session};
use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-preview-script-{}-{n}", std::process::id()));
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
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"golden b\"}\n");
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

/// Runs `steps` in `run` (a folder with its own config directory) with `--record` into `record`.
fn run(run: &Path, steps: &[(&str, Value)], record: &Path) -> Output {
    let file = run.join("b.uiscript");
    std::fs::write(&file, script(steps)).unwrap();
    Command::new(super::bin_exe())
        .args(["--headless", "--script"])
        .arg(&file)
        .arg("--record")
        .arg(record)
        .env("EW_CONFIG_DIR", run.join("config"))
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

/// Golden B: B1's settings step through `command.run`, its renumber through `devices.renumber`, B2 through
/// `pages.renumber`, each applied from the Preview tab; then an undo of B2, an Undo to here back to B2, and a save.
fn golden_b_steps(project: &Path) -> Vec<(&'static str, Value)> {
    vec![
        ("file.open", json!({ "dir": project.to_string_lossy() })),
        ("history.show", json!({"on": true})),
        (
            "command.run",
            json!({"cmd": "project.settings.set", "params": {"member": "numbering", "value": {"default": "page-based",
                "schemes": [{"key": "page-based", "format": "{page}{letter}{counter}", "rules": [], "child_separator": "-"}]}}}),
        ),
        (
            "devices.renumber",
            json!({"scope": "project", "scheme": "page_based", "include_frozen": true, "only_nonconforming": true}),
        ),
        ("ui.describe", json!({})),
        ("preview.apply", json!({})),
        ("pages.renumber", json!({"section": "1", "start": 101})),
        ("ui.describe", json!({})),
        ("preview.apply", json!({})),
        ("edit.undo", json!({})),
        ("history.undo_to", json!({"entry": 3})),
        ("file.save", json!({})),
        ("ui.describe", json!({})),
    ]
}

/// Per gate one run of the script (`twice` false); the deep nightly's
/// `nightly_every_script_transcript_is_byte_identical_twice` runs the body with `twice` true: a second run on an
/// equal start and the byte-identical comparison of the two transcripts (test clean-up, 2026-10-30).
#[test]
fn pv2_golden_b_script_is_byte_identical_twice_and_replays() {
    pv2_golden_b_script_is_byte_identical_twice_and_replays_case(false);
}

pub(crate) fn pv2_golden_b_script_is_byte_identical_twice_and_replays_case(twice: bool) {
    let tmp = Tmp::new();
    let mut transcripts = Vec::new();
    for side in super::support::runs(twice) {
        let dir = tmp.0.join(side);
        let project = dir.join("g.ew");
        copy_dir(Path::new(GOLDEN_A), &project);
        let record = dir.join("record.ewcmd");
        let out = run(&dir, &golden_b_steps(&project), &record);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        transcripts.push(String::from_utf8(out.stdout.clone()).unwrap());
        let steps = steps_of(&out);
        assert_eq!(steps.len(), 13);
        for s in &steps {
            assert_eq!(s["outcome"], "done", "{s}");
        }
        let d = steps[4]["describe"].as_str().unwrap();
        assert!(
            d.contains(
                "preview \"numbering.renumber\" ready rev 1\n  summary records 8 tags 11 xrefs 0 pages 1\n"
            ),
            "{d}"
        );
        assert!(d.contains("  tag \"F55\" \"-101F2\""), "{d}");
        assert!(
            d.contains("card ready \"numbering.renumber\" targets 8 changes 8 tags 11 xrefs 0"),
            "{d}"
        );
        let d = steps[7]["describe"].as_str().unwrap();
        assert!(d.contains("  summary records 1 tags 2 xrefs 14 pages "), "{d}");
        assert!(
            d.contains("  tag \"-106W1\" \"-104W1\"\n  tag \"-106W2\" \"-104W2\"\n"),
            "{d}"
        );
        let d = steps[12]["describe"].as_str().unwrap();
        assert!(
            d.contains("history entries 3 cursor 3\n  marker \"Saved\"\n  entry 3 now "),
            "{d}"
        );
        assert!(d.contains("preview none"), "{d}");
        let all = String::from_utf8_lossy(&out.stdout);
        assert!(
            !all.contains(&*tmp.0.to_string_lossy()),
            "a path in the transcript"
        );
        // The record replayed onto a fresh copy: the records of the saved project.
        let log = CmdLog::parse(&std::fs::read_to_string(&record).unwrap()).unwrap();
        assert_eq!(log.commands().count(), 3, "settings, renumber, page renumber");
        let fresh = dir.join("fresh.ew");
        copy_dir(Path::new(GOLDEN_A), &fresh);
        let (mut replayed, _) = Session::open_read_only(&fresh, "test").unwrap();
        replayed.run_script(&log, "b.ewcmd").unwrap();
        let (saved, _) = Session::open_read_only(&project, "test").unwrap();
        let diff = replayed.diff_from(&saved.world()).unwrap();
        assert!(diff.model.is_empty(), "{:?}", diff.model);
    }
    if twice {
        assert_eq!(transcripts[0], transcripts[1], "two runs give the same bytes");
    }
}
