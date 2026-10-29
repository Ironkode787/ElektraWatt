// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property grid headless (WP-S2.05 part 1, acceptance checks PG9 and PG10): the edits the pilot makes on folios
//! 101 to 103 (§6.12) as an `ew.uiscript/1` script through the built binary, byte-identical transcripts on two runs,
//! the `--record` log replayed onto a fresh copy giving equal records, and D-06's property-grid entry (the description
//! override of folio 101 with its finding, reset, undo) read back through the transcript and `ui.describe`. No window,
//! no GPU, no display; every run on a temporary copy of Golden A. A module of `tests/headless.rs`, whose `bin_exe` it
//! uses (FT-353).

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_ui::ew_viewmodel::ew_api::{CmdLog, Session};
use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

/// W29 on folio 103 and W20 on 102 (`connections|W29`, `connections|W20`), the potential definition 24VDC-1.
const W29: &str = "9eb2d865-fca4-59b1-87bf-c073d7190ceb";
const DEF_24VDC_1: &str = "51b0b676-8978-50af-91cc-cde58ea478d9";
/// W20 on folio 102 (`connections|W20`), -101F1's representation, the interruption point PE-1.2 on 102.
const W20: &str = "1c5963b7-6c34-53d6-8fa6-9d02d4629711";
const REP_101F1: &str = "7b76f7a3-8966-50ed-a194-36d38ac1c0a1";
const IP_PE_1_2: &str = "6198cbd3-a6a2-569c-8284-2d39ba05bfae";

/// The pilot's command log as `--record` writes it after its header: the constant `PILOT_LOG` of
/// `tests/grid/pilot_log.rs`, which `bin/ew/tests/cli.rs` includes and replays through `ew apply`.
const PILOT_LOG_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/grid/pilot_log.rs");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-props-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// A copy of Golden A in this directory.
    fn golden(&self, name: &str) -> PathBuf {
        let target = self.path(name);
        copy_dir(Path::new(GOLDEN_A), &target);
        target
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
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"property grid\"}\n");
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

/// Runs a script file with `--record` into `record` (when given) and a config directory of its own.
fn run(tmp: &Tmp, name: &str, text: &str, record: Option<&Path>) -> Output {
    let file = tmp.path(name);
    std::fs::write(&file, text).unwrap();
    let mut cmd = Command::new(super::bin_exe());
    cmd.args(["--headless", "--script"]).arg(&file);
    if let Some(r) = record {
        cmd.arg("--record").arg(r);
    }
    cmd.env("EW_CONFIG_DIR", tmp.path(&format!("{name}.config")))
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn transcript(out: &Output) -> Vec<Value> {
    let text = String::from_utf8(out.stdout.clone()).unwrap();
    let mut lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{l}: {e}")))
        .collect();
    lines.remove(0);
    lines
}

#[track_caller]
fn all_done(out: &Output) -> Vec<Value> {
    let steps = transcript(out);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    for s in &steps {
        assert_eq!(s["outcome"], "done", "{s}");
    }
    steps
}

/// The pilot's edits on folios 101 to 103 (§6.12), saved at the end.
fn pilot_steps(dir: &Path) -> Vec<(&'static str, Value)> {
    let terminals: Vec<String> = (1..=20).map(|i| format!("tag:-X1031:{i}")).collect();
    vec![
        ("file.open", json!({ "dir": dir.to_string_lossy() })),
        ("page.goto", json!({"folio": "101"})),
        ("ui.describe", json!({})),
        // -101F1: function text, state.
        ("select.set", json!({"refs": ["tag:-101F1"]})),
        (
            "props.set",
            json!({"row": "device/function_text", "value": "24 V distribution"}),
        ),
        ("props.set", json!({"row": "device/state", "value": "spare"})),
        ("ui.describe", json!({})),
        // S51: unfreeze its tag (EF-15).
        ("select.set", json!({"refs": ["tag:S51"]})),
        (
            "props.action",
            json!({"row": "device/derived.tag", "action": "unfreeze"}),
        ),
        // The terminals of -X1031: their part in one command (FR-DEV-031).
        ("page.goto", json!({"folio": "102"})),
        ("select.set", json!({"refs": terminals})),
        ("props.set", json!({"row": "device/parts[0].qty", "value": "2"})),
        ("ui.describe", json!({})),
        ("toast.dismiss", json!({})),
        // W29: cross-section and colour overridden (AC-28).
        ("page.goto", json!({"folio": "103"})),
        ("select.set", json!({"ids": [W29]})),
        (
            "props.action",
            json!({"row": "connection/derived.csa", "action": "override"}),
        ),
        ("props.input", json!({"text": "1.0"})),
        ("props.commit", json!({})),
        (
            "props.action",
            json!({"row": "connection/derived.colour", "action": "override"}),
        ),
        ("props.input", json!({"text": "BU"})),
        ("props.commit", json!({})),
        ("ui.describe", json!({})),
        // 24VDC-1: name and voltage.
        ("select.set", json!({"ids": [DEF_24VDC_1]})),
        (
            "props.set",
            json!({"row": "potential_def/voltage", "value": "26.4"}),
        ),
        ("ui.describe", json!({})),
        // Page 101: its title and the description override (D-06), then reset.
        ("select.set", json!({"ids": []})),
        ("page.goto", json!({"folio": "101"})),
        ("props.set", json!({"row": "page/title", "value": "Power supply"})),
        (
            "props.action",
            json!({"row": "page/derived.description", "action": "override"}),
        ),
        ("props.input", json!({"text": "Control Cabinet Expansion"})),
        ("props.commit", json!({})),
        ("ui.describe", json!({})),
        (
            "props.action",
            json!({"row": "page/derived.description", "action": "reset"}),
        ),
        // The rest of §6.12: -101F1's part quantity and technical data; -102F1's function text, letter and counter;
        // the other frozen tags of EF-15; a terminal's designation; W29's colour and W20's cross-section reset; the
        // name of 24VDC-1; a representation's arrangement and text slots; an interruption point read-only.
        ("select.set", json!({"refs": ["tag:-101F1"]})),
        ("props.set", json!({"row": "device/parts[0].qty", "value": "2"})),
        (
            "props.set",
            json!({"row": "device/tech_data", "value": "[{\"key\":\"curve\",\"value\":\"C\"}]"}),
        ),
        ("select.set", json!({"refs": ["tag:-102F1"]})),
        (
            "props.set",
            json!({"row": "device/function_text", "value": "Supply 24 V"}),
        ),
        ("props.set", json!({"row": "device/tag.letter", "value": "Q"})),
        ("props.set", json!({"row": "device/tag.counter", "value": "9"})),
        ("select.set", json!({"refs": ["tag:F55"]})),
        (
            "props.action",
            json!({"row": "device/derived.tag", "action": "unfreeze"}),
        ),
        ("select.set", json!({"refs": ["tag:T55"]})),
        (
            "props.action",
            json!({"row": "device/derived.tag", "action": "unfreeze"}),
        ),
        ("select.set", json!({"refs": ["tag:E55"]})),
        (
            "props.action",
            json!({"row": "device/derived.tag", "action": "unfreeze"}),
        ),
        ("select.set", json!({"refs": ["tag:S56"]})),
        (
            "props.action",
            json!({"row": "device/derived.tag", "action": "unfreeze"}),
        ),
        ("select.set", json!({"refs": ["tag:E56"]})),
        (
            "props.action",
            json!({"row": "device/derived.tag", "action": "unfreeze"}),
        ),
        ("select.set", json!({"refs": ["tag:1X1"]})),
        (
            "props.action",
            json!({"row": "device/derived.tag", "action": "unfreeze"}),
        ),
        ("select.set", json!({"refs": ["tag:-X1031:20"]})),
        (
            "props.set",
            json!({"row": "device/terminal.designation", "value": "20A"}),
        ),
        ("select.set", json!({"ids": [W29]})),
        (
            "props.action",
            json!({"row": "connection/derived.colour", "action": "reset"}),
        ),
        ("select.set", json!({"ids": [W20]})),
        (
            "props.action",
            json!({"row": "connection/derived.csa", "action": "reset"}),
        ),
        ("select.set", json!({"ids": [DEF_24VDC_1]})),
        (
            "props.set",
            json!({"row": "potential_def/name", "value": "24VDC-1A"}),
        ),
        ("select.set", json!({"ids": [REP_101F1]})),
        (
            "props.set",
            json!({"row": "representation/arrangement", "value": "B"}),
        ),
        (
            "props.set",
            json!({"row": "representation/text_slots", "value": "[{\"slot\":0,\"visible\":false}]"}),
        ),
        ("select.set", json!({"ids": [IP_PE_1_2]})),
        ("ui.describe", json!({})),
        ("select.set", json!({"ids": []})),
        ("file.save", json!({})),
        ("ui.describe", json!({})),
    ]
}

/// PG10: the pilot's script twice gives byte-identical transcripts; its `--record` log replayed onto a fresh copy
/// gives the records the script saved.
/// Per gate one run of the script (`twice` false); the deep nightly's
/// `nightly_every_script_transcript_is_byte_identical_twice` runs the body with `twice` true: a second run on an
/// equal start and the byte-identical comparison of the two transcripts (test clean-up, 2026-10-30).
#[test]
fn pg10_the_pilot_script_is_deterministic_and_replays() {
    pg10_the_pilot_script_is_deterministic_and_replays_case(false);
}

pub(crate) fn pg10_the_pilot_script_is_deterministic_and_replays_case(twice: bool) {
    let tmp = Tmp::new();
    let a = tmp.golden("a.ew");
    let record = tmp.path("pilot.ewcmd");
    let out_a = run(&tmp, "a.jsonl", &script(&pilot_steps(&a)), Some(&record));
    let steps = all_done(&out_a);
    // FT-422: no UUID in a transcript line. The second run names a record the transcript has printed by its alias: a
    // `select.set` `ids` step whose record an earlier line printed gets `@n` instead of the UUID; the transcripts stay
    // byte-identical and its `--record` log holds the same UUIDs (ID-630 (c)).
    let text_a = String::from_utf8_lossy(&out_a.stdout).into_owned();
    super::support::assert_no_uuid(&text_a);
    if twice {
        let b = tmp.golden("b.ew");
        let lines_a: Vec<&str> = text_a.lines().skip(1).collect();
        let mut steps_b = pilot_steps(&b);
        let mut aliased = Vec::new();
        for (i, (action, params)) in steps_b.iter_mut().enumerate() {
            let Some(id) = params["ids"].get(0).and_then(Value::as_str).map(str::to_owned) else {
                continue;
            };
            assert_eq!(*action, "select.set");
            let alias = steps[i]["state"]["canvas"]["selection"][0]
                .as_str()
                .unwrap()
                .to_owned();
            assert!(ew_ui::ew_viewmodel::alias::is_alias(&alias), "{alias}");
            if lines_a[..i].iter().any(|l| l.contains(&format!("\"{alias}\""))) {
                *params = json!({ "ids": [alias] });
                aliased.push(id);
            }
        }
        assert!(aliased.contains(&W29.to_owned()), "{aliased:?}");
        assert!(aliased.contains(&DEF_24VDC_1.to_owned()), "{aliased:?}");
        let record_b = tmp.path("pilot-b.ewcmd");
        let out_b = run(&tmp, "b.jsonl", &script(&steps_b), Some(&record_b));
        all_done(&out_b);
        assert_eq!(
            std::fs::read_to_string(&record).unwrap(),
            std::fs::read_to_string(&record_b).unwrap(),
            "an alias in the input records the UUID it names"
        );
        // The transcripts name the project directories, which are the same name in both.
        assert_eq!(
            String::from_utf8_lossy(&out_a.stdout).replace("a.ew", "x.ew"),
            String::from_utf8_lossy(&out_b.stdout).replace("b.ew", "x.ew"),
            "two runs give the same bytes"
        );
    }
    // What the steps read back.
    let describe = |i: usize| steps[i]["describe"].as_str().unwrap().to_owned();
    let d = describe(6);
    assert!(
        d.contains("row \"device/function_text\" \"Function text\" \"24 V distribution\" authored"),
        "{d}"
    );
    assert!(
        d.contains("row \"device/state\" \"State\" \"spare\" authored"),
        "{d}"
    );
    let d = describe(12);
    assert!(d.contains("toast \"Set Quantity on 20 devices\" undo"), "{d}");
    let d = describe(22);
    assert!(
        d.contains(
            "row \"connection/derived.csa\" \"Cross-section\" \"1 mm²\" overridden \"override (CDP)\""
        ),
        "{d}"
    );
    assert!(
        d.contains("row \"connection/derived.colour\" \"Colour\" \"BU\" overridden \"override (CDP)\""),
        "{d}"
    );
    let d = describe(25);
    assert!(
        d.contains("row \"potential_def/voltage\" \"Voltage\" \"26.4\" authored"),
        "{d}"
    );
    let d = describe(32);
    assert!(
        d.contains("row \"page/derived.description\" \"Description\" \"Control Cabinet Expansion\" overridden \"override · project description is 'Expansion Control Cabinets'\""),
        "{d}"
    );
    assert!(d.contains("properties finding info \"EW-CORE-023\""), "{d}");
    // No UUID in the new members.
    for s in &steps {
        for member in ["properties", "card", "toast"] {
            let text = s["state"][member].to_string();
            assert!(!text.contains(W29) && !text.contains(DEF_24VDC_1), "{text}");
        }
    }
    // The record replayed onto a fresh copy: the same records as the saved project.
    let log = CmdLog::parse(&std::fs::read_to_string(&record).unwrap()).unwrap();
    assert!(
        log.commands().all(|c| c.name == "fields.set"),
        "the grid commits fields.set only here"
    );
    let fresh = tmp.golden("fresh.ew");
    let (mut replayed, _) = Session::open_read_only(&fresh, "test").unwrap();
    replayed.run_script(&log, "pilot.ewcmd").unwrap();
    let (saved, _) = Session::open_read_only(&a, "test").unwrap();
    let diff = replayed.diff_from(&saved.world()).unwrap();
    assert!(diff.model.is_empty(), "{:?}", diff.model);
    // The record is the committed pilot log (after its header), which `ew apply` replays in `bin/ew/tests/cli.rs`:
    // the spec's path (§9 item 2) without a second binary in this package's tests. A missing file is written.
    let recorded = std::fs::read_to_string(&record).unwrap();
    let body: String = recorded.lines().skip(1).map(|l| format!("{l}\n")).collect();
    if !Path::new(PILOT_LOG_FILE).exists() {
        let source = format!(
            "// SPDX-{}: 2026 The ElektraWatt contributors\n// SPDX-{}: GPL-3.0-or-later\n\n\
             //! The pilot's grid edits on folios 101 to 103 (WP-S2.05 part 1, §6.12) as `--record` writes them after its\n\
             //! header: generated by `tests/grid/properties.rs`, replayed through `ew apply` by `bin/ew/tests/cli.rs`.\n\n\
             /// The command log's lines after the header.\n#[allow(dead_code)]\npub const PILOT_LOG: &str = r##\"{body}\"##;\n",
            "FileCopyrightText", "License-Identifier"
        );
        std::fs::write(PILOT_LOG_FILE, source).unwrap();
    }
    let committed = std::fs::read_to_string(PILOT_LOG_FILE)
        .unwrap()
        .replace("\r\n", "\n");
    let committed_body = committed
        .split_once("r##\"")
        .and_then(|(_, rest)| rest.split_once("\"##"))
        .map(|(b, _)| b.to_owned())
        .unwrap_or_default();
    assert_eq!(body, committed_body, "the record is tests/grid/pilot_log.rs");
    // Every row of §6.12 was edited: the commands touch these paths.
    let paths: Vec<String> = log
        .commands()
        .map(|c| {
            format!(
                "{} {}",
                c.params["kind"].as_str().unwrap_or(""),
                c.params["path"].as_str().unwrap_or("")
            )
        })
        .collect();
    for want in [
        "device function_text",
        "device parts",
        "device tech_data",
        "device state",
        "device tag.frozen",
        "device tag.letter",
        "device tag.counter",
        "device terminal.designation",
        "connection overrides.csa",
        "connection overrides.colour",
        "page title",
        "page description_override",
        "potential_def name",
        "potential_def voltage",
        "representation text_slots",
        "representation arrangement",
    ] {
        assert!(paths.iter().any(|p| p == want), "{want}: {paths:?}");
    }
    // The interruption point's members are read-only (no command edits them yet; §6.12, a follow-up).
    let ip = steps
        .iter()
        .filter_map(|s| s["describe"].as_str())
        .find(|d| d.contains("  section function targets 1"))
        .expect("the interruption point's sheet");
    assert!(
        ip.contains("row \"function/kind\"") && ip.contains("read-only \"no command edits this member yet\""),
        "{ip}"
    );
}

/// §6.12: a counter collision on -102F1 is refused with the next free counter; the run stops at that step.
#[test]
fn pg8_a_counter_collision_stops_the_script_with_its_remedy() {
    let tmp = Tmp::new();
    let dir = tmp.golden("collision.ew");
    let steps = vec![
        ("file.open", json!({ "dir": dir.to_string_lossy() })),
        ("select.set", json!({"refs": ["tag:-102F1"]})),
        ("props.set", json!({"row": "device/tag.counter", "value": "2"})),
    ];
    let out = run(&tmp, "collision.jsonl", &script(&steps), None);
    assert_eq!(out.status.code(), Some(1));
    let t = transcript(&out);
    let last = t.last().unwrap();
    assert_eq!(last["outcome"], "failed");
    let message = last["message"].as_str().unwrap();
    assert!(message.contains("next free counter"), "{message}");
    assert!(
        !message.contains('-') || !message.contains("4aa43308"),
        "no page ID: {message}"
    );
    assert_eq!(last["state"]["properties"]["error"]["row"], "device/tag.counter");
}

/// PG9: D-06's property-grid entry headless — the override of folio 101's description shows as Overridden with the
/// project's description, the finding EW-CORE-023 at info in the header; Reset removes both; each one undo step.
/// Per gate one run of the script (`twice` false); the deep nightly's
/// `nightly_every_script_transcript_is_byte_identical_twice` runs the body with `twice` true: a second run on an
/// equal start and the byte-identical comparison of the two transcripts (test clean-up, 2026-10-30).
#[test]
fn pg9_d06_headless() {
    pg9_d06_headless_case(false);
}

pub(crate) fn pg9_d06_headless_case(twice: bool) {
    let tmp = Tmp::new();
    let dir = tmp.golden("d06.ew");
    let steps = vec![
        ("file.open", json!({ "dir": dir.to_string_lossy() })),
        ("page.goto", json!({"folio": "101"})),
        ("ui.describe", json!({})),
        ("props.focus", json!({})),
        ("props.row", json!({"to": "page/derived.description"})),
        ("props.edit", json!({})),
        ("props.input", json!({"text": "Control Cabinet Expansion"})),
        ("props.commit", json!({})),
        ("ui.describe", json!({})),
        ("props.action", json!({"action": "reset"})),
        ("ui.describe", json!({})),
        ("edit.undo", json!({})),
        ("ui.describe", json!({})),
        ("edit.undo", json!({})),
        ("ui.describe", json!({})),
    ];
    let out = run(&tmp, "d06.jsonl", &script(&steps), None);
    let t = all_done(&out);
    let d = |i: usize| t[i]["describe"].as_str().unwrap().to_owned();
    let inherited = "row \"page/derived.description\" \"Description\" \"Expansion Control Cabinets\" inherited \"project description\" actions override";
    let overridden = "row \"page/derived.description\" \"Description\" \"Control Cabinet Expansion\" overridden \"override · project description is 'Expansion Control Cabinets'\" actions override reset";
    assert!(d(2).contains(inherited), "{}", d(2));
    assert!(!d(2).contains("EW-CORE-023"));
    assert!(d(8).contains(overridden), "{}", d(8));
    assert!(
        d(8).contains("properties finding info \"EW-CORE-023\""),
        "{}",
        d(8)
    );
    assert!(d(10).contains(inherited), "{}", d(10));
    assert!(!d(10).contains("EW-CORE-023"), "{}", d(10));
    assert!(
        d(12).contains(overridden),
        "undo brings the override back: {}",
        d(12)
    );
    assert!(d(14).contains(inherited), "{}", d(14));
    assert_eq!(t[7]["state"]["properties"]["row"], "page/derived.description");
    assert_eq!(t[7]["state"]["focus"], "properties");
    if twice {
        // Twice: the same bytes.
        let dir2 = tmp.golden("d06b.ew");
        let steps2: Vec<(&str, Value)> = steps
            .iter()
            .map(|(a, p)| {
                if *a == "file.open" {
                    (*a, json!({ "dir": dir2.to_string_lossy() }))
                } else {
                    (*a, p.clone())
                }
            })
            .collect();
        let out2 = run(&tmp, "d06b.jsonl", &script(&steps2), None);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).replace("d06.ew", "x"),
            String::from_utf8_lossy(&out2.stdout).replace("d06b.ew", "x")
        );
    }
}
