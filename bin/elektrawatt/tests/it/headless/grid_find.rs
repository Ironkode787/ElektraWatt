// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Find and replace headless (acceptance checks FR3 to FR6 through the built binary): "Spare" on 201
//! (the sample's two spare valves) replaced by "Reserve" through the card, the regular expression `(Reserve)` → `$1 (unused)`, and the freeze
//! of three tags on 102, as an `ew.uiscript/1` script; byte-identical transcripts on two runs; the `--record` log
//! (three `fields.set_many` lines) replayed onto a fresh copy giving the records the script saved (`ew apply` replays
//! the same way, `bin/ew/tests/it/cli.rs`). No UUID in the `find` member or the find lines. A module of
//! `tests/it/headless.rs`, whose `bin_exe` it uses.

use std::path::Path;
use std::process::{Output, Stdio};

use ew_viewmodel::ew_api::{CmdLog, Session};
use serde_json::{Value, json};

use crate::support::Tmp;

fn script(steps: &[(&str, Value)]) -> String {
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"find and replace\"}\n");
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

/// Runs a script file with `--record` into `record` and a config directory of its own.
fn run(tmp: &Tmp, name: &str, text: &str, record: &Path) -> Output {
    let file = tmp.path(name);
    std::fs::write(&file, text).unwrap();
    crate::support::elektrawatt(&super::bin_exe())
        .args(["--headless", "--script"])
        .arg(&file)
        .arg("--record")
        .arg(record)
        .env("EW_CONFIG_DIR", tmp.path(&format!("{name}.config")))
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[track_caller]
fn all_done(out: &Output) -> Vec<Value> {
    let text = String::from_utf8(out.stdout.clone()).unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}\n{text}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut steps: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{l}: {e}")))
        .collect();
    steps.remove(0);
    for s in &steps {
        assert_eq!(s["outcome"], "done", "{s}");
    }
    steps
}

/// Whether `text` holds a hyphenated UUID.
fn has_uuid(text: &str) -> bool {
    let b = text.as_bytes();
    (0..b.len().saturating_sub(35)).any(|i| {
        b[i..i + 36].iter().enumerate().all(|(k, c)| match k {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
    })
}

fn steps(dir: &Path) -> Vec<(&'static str, Value)> {
    vec![
        ("file.open", json!({ "dir": dir.to_string_lossy() })),
        ("page.goto", json!({"folio": "201"})),
        // FR3: "Spare" on 201, the 2 spare valves the sample's script makes, replaced by "Reserve" through the card.
        ("find.open_replace", json!({})),
        ("find.text", json!({"text": "Spare"})),
        ("find.replacement", json!({"text": "Reserve"})),
        ("find.fields", json!({"fields": ["function_text"]})),
        ("find.folios", json!({"folios": ["201"]})),
        ("find.run", json!({})),
        ("ui.describe", json!({})),
        ("find.replace_all", json!({})),
        ("ui.describe", json!({})),
        ("preview.apply", json!({})),
        ("ui.describe", json!({})),
        // FR4: the regular expression, `$1` expanded.
        ("find.option", json!({"option": "regex", "on": true})),
        ("find.text", json!({"text": "(Reserve)"})),
        ("find.replacement", json!({"text": "$1 (unused)"})),
        ("find.replace_all", json!({})),
        ("preview.apply", json!({})),
        ("find.run", json!({})),
        ("ui.describe", json!({})),
        // FR5: freeze three tags of folio 102 through the card.
        ("page.goto", json!({"folio": "102"})),
        (
            "select.set",
            json!({"refs": ["tag:-102F1", "tag:-102S1", "tag:-102S2"]}),
        ),
        (
            "props.action",
            json!({"row": "device/derived.tag", "action": "freeze"}),
        ),
        ("ui.describe", json!({})),
        ("preview.apply", json!({})),
        ("file.save", json!({})),
    ]
}

/// Per gate one run of the script (`twice` false); the deep nightly's
/// `nightly_every_script_transcript_is_byte_identical_twice` runs the body with `twice` true: a second run on an
/// equal start and the byte-identical comparison of the two transcripts (test clean-up, 2026-10-30).
#[test]
fn find_and_replace_headless_twice_and_replayed() {
    fr3_to_fr6_find_and_replace_headless_twice_and_replayed_case(false);
}

pub(crate) fn fr3_to_fr6_find_and_replace_headless_twice_and_replayed_case(twice: bool) {
    let tmp = Tmp::new();
    let a = tmp.sample("a.ew");
    let record = tmp.path("find.ewcmd");
    let out = run(&tmp, "find.jsonl", &script(&steps(&a)), &record);
    let t = all_done(&out);
    let d = |i: usize| t[i]["describe"].as_str().unwrap().to_owned();
    assert!(
        d(8).contains(
            "find \"Spare\" replace \"Reserve\" regex off case off word off scope function_text folios \"201\" results 2"
        ),
        "{}",
        d(8)
    );
    assert_eq!(t[7]["state"]["find"]["results"], 2);
    assert!(
        d(10).contains("card ready \"fields.set_many\" targets 2 changes 2"),
        "{}",
        d(10)
    );
    assert_eq!(t[9]["state"]["card"]["origin"], "find");
    assert!(
        d(12).contains("find status \"Replaced 2; run again to refresh\""),
        "{}",
        d(12)
    );
    assert!(d(19).contains("results 2"), "{}", d(19));
    assert!(d(19).contains("\"Reserve (unused)\""), "{}", d(19));
    assert!(
        d(23).contains("card ready \"fields.set_many\" targets 3"),
        "{}",
        d(23)
    );
    // No UUID in the find member or the find lines.
    for s in &t {
        assert!(
            !has_uuid(&s["state"]["find"].to_string()),
            "{}",
            s["state"]["find"]
        );
        assert!(
            !has_uuid(&s["state"]["card"].to_string()),
            "{}",
            s["state"]["card"]
        );
        if let Some(text) = s["describe"].as_str() {
            for line in text
                .lines()
                .filter(|l| l.starts_with("find") || l.starts_with("  result"))
            {
                assert!(!has_uuid(line), "{line}");
            }
        }
    }
    if twice {
        // Twice: the same bytes.
        let b = tmp.sample("b.ew");
        let record_b = tmp.path("find_b.ewcmd");
        let out_b = run(&tmp, "find_b.jsonl", &script(&steps(&b)), &record_b);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("a.ew", "x"),
            String::from_utf8_lossy(&out_b.stdout).replace("b.ew", "x")
        );
    }
    // The record: three `fields.set_many` lines, replayed onto a fresh copy, give the saved records.
    let log = CmdLog::parse(&std::fs::read_to_string(&record).unwrap()).unwrap();
    let names: Vec<&str> = log.commands().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["fields.set_many"; 3]);
    let fresh = tmp.sample("fresh.ew");
    let (mut replayed, _) = Session::open_read_only(&fresh, "test").unwrap();
    replayed.run_script(&log, "find.ewcmd").unwrap();
    let (saved, _) = Session::open_read_only(&a, "test").unwrap();
    let diff = replayed.diff_from(&saved.world()).unwrap();
    assert!(diff.model.is_empty(), "{:?}", diff.model);
}
