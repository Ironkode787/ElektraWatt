// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Headless: two renumberings of the sample's section 1 through the renumber action and the Preview tab, with the
//! history panel open, as an `ew.uiscript/1` script through the built binary, run twice on its own copy of the sample
//! with byte-identical transcripts; the `--record` log replayed onto a fresh copy gives the records of the saved
//! project. A module of `tests/it/headless.rs`, whose `bin_exe` it uses.

use std::path::Path;
use std::process::{Output, Stdio};

use ew_viewmodel::ew_api::{CmdLog, Session};
use serde_json::{Value, json};

use crate::support::{SAMPLE, Tmp, copy_dir};

fn script(steps: &[(&str, Value)]) -> String {
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"renumber\"}\n");
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
    crate::support::elektrawatt(&super::bin_exe())
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

/// The folios of section 1 renumbered from 111 and back from 101 through `pages.renumber`, each applied from the
/// Preview tab; then an undo of the second, an Undo to here back to it, and a save.
fn renumber_steps(project: &Path) -> Vec<(&'static str, Value)> {
    vec![
        ("file.open", json!({ "dir": project.to_string_lossy() })),
        ("history.show", json!({"on": true})),
        ("pages.renumber", json!({"section": "1", "start": 111})),
        ("ui.describe", json!({})),
        ("preview.apply", json!({})),
        ("pages.renumber", json!({"section": "1", "start": 101})),
        ("ui.describe", json!({})),
        ("preview.apply", json!({})),
        ("edit.undo", json!({})),
        ("history.undo_to", json!({"entry": 2})),
        ("file.save", json!({})),
        ("ui.describe", json!({})),
    ]
}

/// Per gate one run of the script (`twice` false); the deep nightly's
/// `nightly_every_script_transcript_is_byte_identical_twice` runs the body with `twice` true: a second run on an
/// equal start and the byte-identical comparison of the two transcripts (test clean-up, 2026-10-30).
#[test]
fn renumber_script_is_byte_identical_twice_and_replays() {
    pv2_renumber_script_is_byte_identical_twice_and_replays_case(false);
}

pub(crate) fn pv2_renumber_script_is_byte_identical_twice_and_replays_case(twice: bool) {
    let tmp = Tmp::new();
    let mut transcripts = Vec::new();
    for side in crate::support::runs(twice) {
        let dir = tmp.dir().join(side);
        let project = dir.join("g.ew");
        copy_dir(Path::new(SAMPLE), &project);
        let record = dir.join("record.ewcmd");
        let out = run(&dir, &renumber_steps(&project), &record);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        transcripts.push(String::from_utf8(out.stdout.clone()).unwrap());
        let steps = steps_of(&out);
        assert_eq!(steps.len(), 12);
        for s in &steps {
            assert_eq!(s["outcome"], "done", "{s}");
        }
        // The page-based tags follow their folios: -101F1 on 101, -103W1 on 103.
        let d = steps[3]["describe"].as_str().unwrap();
        assert!(d.contains("  summary records "), "{d}");
        assert!(d.contains("  tag \"-101F1\" \"-111F1\""), "{d}");
        assert!(d.contains("  tag \"-103W1\" \"-113W1\""), "{d}");
        let d = steps[6]["describe"].as_str().unwrap();
        assert!(d.contains("  tag \"-111F1\" \"-101F1\""), "{d}");
        let d = steps[11]["describe"].as_str().unwrap();
        assert!(
            d.contains("history entries 2 cursor 2\n  marker \"Saved\"\n  entry 2 now "),
            "{d}"
        );
        assert!(d.contains("preview none"), "{d}");
        let all = String::from_utf8_lossy(&out.stdout);
        assert!(
            !crate::support::names_folder(&all, tmp.dir()),
            "a path in the transcript"
        );
        // The record replayed onto a fresh copy: the records of the saved project.
        let log = CmdLog::parse(&std::fs::read_to_string(&record).unwrap()).unwrap();
        assert_eq!(log.commands().count(), 2, "the two renumberings");
        let fresh = dir.join("fresh.ew");
        copy_dir(Path::new(SAMPLE), &fresh);
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
