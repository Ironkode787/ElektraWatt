// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property grid headless (acceptance checks PG9 and PG10): the edits the pilot makes on folios
//! 101 to 103 (§6.12) as an `ew.uiscript/1` script through the built binary, byte-identical transcripts on two runs,
//! the `--record` log replayed onto a fresh copy giving equal records, and D-06's property-grid entry (the description
//! override of folio 101 with its finding, reset, undo) read back through the transcript and `ui.describe`. No window,
//! no GPU, no display; every run on a temporary copy of the sample. A module of `tests/it/headless.rs`, whose `bin_exe` it
//! uses.

use std::path::Path;
use std::process::{Output, Stdio};

use ew_viewmodel::ew_api::{CmdLog, Session};
use serde_json::{Value, json};

use crate::support::Tmp;

/// The ID of the first record of the sample's page `folio` (its file, `pages/<id>.jsonl`) whose line holds every one of
/// `needles`: the sample's IDs are made by its script, so the pilot finds its records by content.
fn record_id(folio: u32, needles: &[&str]) -> String {
    let number = format!("\"number\":{folio}}}");
    let dir = std::path::Path::new(crate::support::SAMPLE).join("pages");
    for e in std::fs::read_dir(dir).unwrap() {
        let text = std::fs::read_to_string(e.unwrap().path()).unwrap();
        if !text.lines().next().is_some_and(|l| l.contains(&number)) {
            continue;
        }
        if let Some(line) = text.lines().find(|l| needles.iter().all(|n| l.contains(n))) {
            let v: Value = serde_json::from_str(line).unwrap();
            return v["id"].as_str().unwrap().to_owned();
        }
    }
    panic!("no record {needles:?} on folio {folio}");
}

/// A plain wire on folio 103 (no cable core, no override): its cross-section and colour are overridden.
fn wire_103() -> String {
    let id = record_id(103, &["\"$k\":\"connection\""]);
    let text = page_text(103);
    let plain = text
        .lines()
        .find(|l| {
            l.contains("\"$k\":\"connection\"") && !l.contains("\"kind\"") && !l.contains("\"overrides\"")
        })
        .map(|l| {
            serde_json::from_str::<Value>(l).unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned()
        });
    plain.unwrap_or(id)
}

/// The text of the sample's page `folio`.
fn page_text(folio: u32) -> String {
    let number = format!("\"number\":{folio}}}");
    let dir = std::path::Path::new(crate::support::SAMPLE).join("pages");
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .find(|t| t.lines().next().is_some_and(|l| l.contains(&number)))
        .unwrap()
}

/// The shield wire of -103W1 on folio 103, the one wire with a cross-section override (2.5 mm²): it is reset.
fn shield_103() -> String {
    record_id(103, &["\"overrides\":{\"csa\":\"2.5\"}"])
}

/// The potential definition 24V at the power supply on folio 101 (a device file holds it, beside its device).
fn def_24v() -> String {
    let dir = std::path::Path::new(crate::support::SAMPLE).join("devices");
    std::fs::read_dir(dir)
        .unwrap()
        .find_map(|e| {
            let text = std::fs::read_to_string(e.unwrap().path()).unwrap();
            let line = text
                .lines()
                .find(|l| l.contains("\"$k\":\"potential_def\"") && l.contains("\"name\":\"24V\""))?;
            Some(
                serde_json::from_str::<Value>(line).unwrap()["id"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            )
        })
        .expect("the potential definition 24V")
}

/// The interruption point PE on folio 102, where the PE comes in from 101.
fn ip_pe_102() -> String {
    record_id(102, &["\"ip\":{\"name\":\"PE\""])
}

/// The pilot's command log as `--record` writes it after its header: the constant `PILOT_LOG` of
/// `tests/grid/pilot_log.rs`, which `bin/ew/tests/it/cli.rs` includes and replays through `ew apply`.
const PILOT_LOG_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/grid/pilot_log.rs");

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
    let mut cmd = crate::support::elektrawatt(&super::bin_exe());
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
    let terminals: Vec<String> = (1..=8).map(|i| format!("tag:-X1021:{i}")).collect();
    let (wire, shield, def, ip) = (wire_103(), shield_103(), def_24v(), ip_pe_102());
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
        // -UC1: unfreeze its tag (the enclosure's tag, which Enclosure… freezes).
        ("select.set", json!({"refs": ["tag:-UC1"]})),
        (
            "props.action",
            json!({"row": "device/derived.tag", "action": "unfreeze"}),
        ),
        // The terminals of -X1021: their part in one command (FR-DEV-031).
        ("page.goto", json!({"folio": "102"})),
        ("select.set", json!({"refs": terminals})),
        ("props.set", json!({"row": "device/parts[0].qty", "value": "2"})),
        ("ui.describe", json!({})),
        ("toast.dismiss", json!({})),
        // A wire on 103: cross-section and colour overridden (AC-28).
        ("page.goto", json!({"folio": "103"})),
        ("select.set", json!({"ids": [wire]})),
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
        // 24V: name and voltage.
        ("select.set", json!({"ids": [def]})),
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
        // the other frozen tag, -UC1-plate; a terminal's designation; the wire's colour and the shield's
        // cross-section reset; the name of 24V; a representation's arrangement and text slots; an interruption point
        // read-only.
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
        ("props.set", json!({"row": "device/tag.letter", "value": "K"})),
        ("props.set", json!({"row": "device/tag.counter", "value": "9"})),
        ("select.set", json!({"refs": ["tag:-UC1-plate"]})),
        (
            "props.action",
            json!({"row": "device/derived.tag", "action": "unfreeze"}),
        ),
        ("select.set", json!({"refs": ["tag:-X1021:8"]})),
        (
            "props.set",
            json!({"row": "device/terminal.designation", "value": "8A"}),
        ),
        ("select.set", json!({"ids": [wire]})),
        (
            "props.action",
            json!({"row": "connection/derived.colour", "action": "reset"}),
        ),
        ("select.set", json!({"ids": [shield]})),
        (
            "props.action",
            json!({"row": "connection/derived.csa", "action": "reset"}),
        ),
        ("select.set", json!({"ids": [def]})),
        ("props.set", json!({"row": "potential_def/name", "value": "24VA"})),
        ("page.goto", json!({"folio": "101"})),
        // -101F1's representation, clicked on its symbol (the script placed it at 101-E8).
        ("tool.select", json!({})),
        ("canvas.press", json!({"x_mm": 145.486, "y_mm": 114.0})),
        ("canvas.release", json!({"x_mm": 145.486, "y_mm": 114.0})),
        (
            "props.set",
            json!({"row": "representation/arrangement", "value": "B"}),
        ),
        (
            "props.set",
            json!({"row": "representation/text_slots", "value": "[{\"slot\":0,\"visible\":false}]"}),
        ),
        ("page.goto", json!({"folio": "102"})),
        ("select.set", json!({"ids": [ip]})),
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
fn the_pilot_script_is_deterministic_and_replays() {
    pg10_the_pilot_script_is_deterministic_and_replays_case(false);
}

pub(crate) fn pg10_the_pilot_script_is_deterministic_and_replays_case(twice: bool) {
    let tmp = Tmp::new();
    let a = tmp.sample("a.ew");
    let record = tmp.path("pilot.ewcmd");
    let out_a = run(&tmp, "a.jsonl", &script(&pilot_steps(&a)), Some(&record));
    let steps = all_done(&out_a);
    // No UUID in a transcript line. The second run names a record the transcript has printed by its alias: a
    // `select.set` `ids` step whose record an earlier line printed gets `@n` instead of the UUID; the transcripts stay
    // byte-identical and its `--record` log holds the same UUIDs.
    let text_a = String::from_utf8_lossy(&out_a.stdout).into_owned();
    crate::support::assert_no_uuid(&text_a);
    if twice {
        let b = tmp.sample("b.ew");
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
            assert!(ew_viewmodel::alias::is_alias(&alias), "{alias}");
            if lines_a[..i].iter().any(|l| l.contains(&format!("\"{alias}\""))) {
                *params = json!({ "ids": [alias] });
                aliased.push(id);
            }
        }
        assert!(aliased.contains(&wire_103()), "{aliased:?}");
        assert!(aliased.contains(&def_24v()), "{aliased:?}");
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
        d.contains("row \"device/state\" \"State\" \"Spare\" authored"),
        "{d}"
    );
    let d = describe(12);
    assert!(d.contains("toast \"Set quantity on 8 devices.\" undo"), "{d}");
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
        d.contains("row \"page/derived.description\" \"Description\" \"Control Cabinet Expansion\" overridden \"override · project description is 'Conveyor drive with a pneumatic clamp'\""),
        "{d}"
    );
    assert!(d.contains("properties finding info \"EW-CORE-023\""), "{d}");
    // No UUID in the new members.
    for s in &steps {
        for member in ["properties", "card", "toast"] {
            let text = s["state"][member].to_string();
            assert!(
                !text.contains(&wire_103()) && !text.contains(&def_24v()),
                "{text}"
            );
        }
    }
    // The record replayed onto a fresh copy: the same records as the saved project.
    let log = CmdLog::parse(&std::fs::read_to_string(&record).unwrap()).unwrap();
    assert!(
        log.commands().all(|c| c.name == "fields.set"),
        "the grid commits fields.set only here"
    );
    let fresh = tmp.sample("fresh.ew");
    let (mut replayed, _) = Session::open_read_only(&fresh, "test").unwrap();
    replayed.run_script(&log, "pilot.ewcmd").unwrap();
    let (saved, _) = Session::open_read_only(&a, "test").unwrap();
    let diff = replayed.diff_from(&saved.world()).unwrap();
    assert!(diff.model.is_empty(), "{:?}", diff.model);
    // The record is the committed pilot log (after its header), which `ew apply` replays in `bin/ew/tests/it/cli.rs`:
    // the spec's path (§9 item 2) without a second binary in this package's tests. A missing file is written.
    let recorded = std::fs::read_to_string(&record).unwrap();
    let body: String = recorded.lines().skip(1).map(|l| format!("{l}\n")).collect();
    if !Path::new(PILOT_LOG_FILE).exists() {
        let source = format!(
            "// SPDX-{}: 2026 The ElektraWatt contributors\n// SPDX-{}: GPL-3.0-or-later\n\n\
             //! The pilot's grid edits on folios 101 to 103 as `--record` writes them after its\n\
             //! header: generated by `tests/it/headless/grid_properties.rs`, replayed through `ew apply` by `bin/ew/tests/it/cli.rs`.\n\n\
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
    // The interruption point's sheet shows its own rows (its name among them); the kind they hang under is a rare
    // row and waits in the closed More.
    let ip = steps
        .iter()
        .filter_map(|s| s["describe"].as_str())
        .find(|d| d.contains("  section function targets 1"))
        .expect("the interruption point's sheet");
    assert!(
        ip.contains("row \"function/kind.ip.name\"") && !ip.contains("row \"function/kind\" "),
        "{ip}"
    );
}

/// §6.12: a counter collision on -102S1 (with -102S2) is refused with the next free counter; the run stops at that step.
#[test]
fn a_counter_collision_stops_the_script_with_its_remedy() {
    let tmp = Tmp::new();
    let dir = tmp.sample("collision.ew");
    let steps = vec![
        ("file.open", json!({ "dir": dir.to_string_lossy() })),
        ("select.set", json!({"refs": ["tag:-102S1"]})),
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
        crate::support::first_uuid(message).is_none(),
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
    let dir = tmp.sample("d06.ew");
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
    let inherited = "row \"page/derived.description\" \"Description\" \"Conveyor drive with a pneumatic clamp\" inherited \"project description\" actions override";
    let overridden = "row \"page/derived.description\" \"Description\" \"Control Cabinet Expansion\" overridden \"override · project description is 'Conveyor drive with a pneumatic clamp'\" actions override reset";
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
        let dir2 = tmp.sample("d06b.ew");
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
