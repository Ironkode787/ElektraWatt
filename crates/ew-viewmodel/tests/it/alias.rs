// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The ID alias `@n` (FT-422, decision log ID-630) through the headless runner on temporary copies of Golden A: no UUID
//! in a transcript line, aliases numbered by first appearance, `@n` accepted where an action parameter takes a UUID
//! or a reference (`select.set`, a navigator key, a command's `{"ref": "@n"}`), an unknown alias refused naming it,
//! `--record` holding the resolved UUID, and two runs of one script giving the same bytes.

use crate::common;

use common::Tmp;
use ew_viewmodel::alias;
use ew_viewmodel::script::{self, NoExtension};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::{Value, json};

/// The script text of `steps`.
fn script_text(steps: &[(&str, Value)]) -> String {
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"alias\"}\n");
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

/// A headless shell whose settings live in `tmp`, and a copy of Golden A there.
fn shell(tmp: &Tmp) -> (ShellModel, String) {
    let dir = tmp.0.join("J-E-2416.ew");
    if !dir.exists() {
        common::copy_dir(std::path::Path::new(common::GOLDEN_A), &dir);
    }
    let shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    (shell, dir.to_string_lossy().into_owned())
}

/// Runs `steps` on `shell` and returns the transcript lines (the header first).
fn run(shell: &mut ShellModel, steps: &[(&str, Value)]) -> Vec<String> {
    let parsed = script::parse(&script_text(steps)).unwrap();
    let mut lines = Vec::new();
    script::run(shell, &mut NoExtension, &parsed, false, &mut |l| {
        lines.push(l.to_owned());
        Ok(())
    })
    .unwrap();
    lines
}

fn step(line: &str) -> Value {
    serde_json::from_str(line).unwrap()
}

/// Asserts that no hyphenated UUID is in `text`.
#[track_caller]
fn no_uuid(text: &str) {
    assert_eq!(alias::find_uuid(text), None, "a UUID in: {text}");
}

/// Acceptance 4: `select.set` with `ids: ["@2"]` selects the second-aliased record; `@9` that the table does not hold
/// is refused with a text naming it; the transcript names records only by alias.
#[test]
fn select_set_takes_an_alias_and_refuses_an_unknown_one_naming_it() {
    let tmp = Tmp::new("alias-select");
    let (mut shell, dir) = shell(&tmp);
    let lines = run(
        &mut shell,
        &[
            ("file.open", json!({"dir": dir, "read_only": true})),
            ("page.goto", json!({"folio": "101"})),
            ("select.set", json!({"refs": ["tag:-101F1", "tag:-102F1"]})),
            ("ui.describe", json!({})),
        ],
    );
    for l in &lines {
        no_uuid(l);
    }
    let selected = step(&lines[3])["state"]["canvas"]["selection"].clone();
    let selected: Vec<String> = selected
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(selected.len(), 2);
    assert!(selected.iter().all(|s| alias::is_alias(s)), "{selected:?}");
    // The describe text names them the same way, unquoted.
    let describe = step(&lines[4])["describe"].as_str().unwrap().to_owned();
    assert!(
        describe.contains(&format!("canvas selection 2 {} {}", selected[0], selected[1])),
        "{describe}"
    );
    // The second-aliased record: what `@2` names in the table.
    let second = shell.alias_uuid(2).unwrap();
    assert!(shell.alias_count() < 9, "the table holds {}", shell.alias_count());
    // A new run starts a new table (the table lives for one script run): `@2` before anything printed names nothing.
    let more = run(&mut shell, &[("select.set", json!({"ids": ["@2"]}))]);
    assert_eq!(step(&more[1])["outcome"], "failed", "{}", more[1]);
    assert_eq!(
        step(&more[1])["message"],
        "No record has the alias `@2` in this session."
    );
    // Within one run: the alias printed by an earlier step selects that record.
    let lines = run(
        &mut shell,
        &[
            ("file.open", json!({"dir": dir, "read_only": true})),
            ("page.goto", json!({"folio": "101"})),
            ("select.set", json!({"refs": ["tag:-101F1", "tag:-102F1"]})),
            ("select.set", json!({"ids": ["@2"]})),
            ("select.set", json!({"ids": ["@9"]})),
        ],
    );
    for l in &lines {
        no_uuid(l);
    }
    assert_eq!(
        shell.alias_uuid(2).as_deref(),
        Some(second.as_str()),
        "the same numbering"
    );
    let picked = step(&lines[4]);
    assert_eq!(picked["outcome"], "done", "{picked}");
    assert_eq!(picked["state"]["canvas"]["selection"], json!(["@2"]));
    assert_eq!(
        shell
            .selection()
            .items
            .iter()
            .map(|e| e.0.to_string())
            .collect::<Vec<_>>(),
        vec![second]
    );
    let unknown = step(&lines[5]);
    assert_eq!(unknown["outcome"], "failed");
    let message = unknown["message"].as_str().unwrap();
    assert!(message.contains("`@9`"), "{message}");
    assert_eq!(message, "No record has the alias `@9` in this session.");
    // The refused step changed nothing.
    assert_eq!(unknown["state"]["canvas"]["selection"], json!(["@2"]));
    shell.shutdown();
}

/// A navigator key prints its page's ID as an alias (`p:@n`) and is accepted back; `refs` takes an alias too.
#[test]
fn a_printed_navigator_key_and_a_reference_resolve_back() {
    let tmp = Tmp::new("alias-keys");
    let (mut shell, dir) = shell(&tmp);
    let lines = run(
        &mut shell,
        &[
            ("file.open", json!({"dir": dir, "read_only": true})),
            ("pages.mode", json!({"mode": "flat"})),
            ("pages.row", json!({"to": "last"})),
        ],
    );
    let key = step(&lines[3])["state"]["navigator"]["row"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(key.contains(":@"), "{key}");
    no_uuid(&key);
    let real = shell.navigator_rows().last().unwrap().key.clone();
    assert_ne!(key, real, "the model keeps the UUID");
    let mut steps = vec![
        ("file.open", json!({"dir": dir, "read_only": true})),
        ("pages.mode", json!({"mode": "flat"})),
        ("pages.row", json!({"to": "last"})),
        ("pages.open", json!({ "key": key })),
        ("select.set", json!({"refs": ["tag:-101F1"]})),
        ("select.set", json!({"ids": []})),
    ];
    let lines = run(&mut shell, &steps);
    let opened = step(&lines[4]);
    assert_eq!(opened["outcome"], "done", "{opened}");
    assert_eq!(
        opened["state"]["page_index"],
        json!(shell.page_count() - 1),
        "the last page opened"
    );
    // The device's alias from the selection, back through `refs` in the same script run again (same numbering).
    let device = step(&lines[5])["state"]["canvas"]["selection"][0]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(alias::is_alias(&device), "{device}");
    steps.push(("select.set", json!({"refs": [device.clone()]})));
    let lines = run(&mut shell, &steps);
    let again = step(&lines[7]);
    assert_eq!(again["outcome"], "done", "{again}");
    assert_eq!(again["state"]["canvas"]["selection"], json!([device]));
    shell.shutdown();
}

/// A command's reference `{"ref": "@n"}` resolves before the command runs, `--record` writes `id:<uuid>`, and an
/// unknown one is refused naming it.
#[test]
fn a_command_reference_alias_is_recorded_as_its_uuid() {
    let tmp = Tmp::new("alias-command");
    let (mut shell, dir) = shell(&tmp);
    let fields_set = |target: &str| {
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": target}],
            "path": "function_text", "value": "Aliased"}})
    };
    let lines = run(
        &mut shell,
        &[
            ("file.open", json!({ "dir": dir })),
            ("select.set", json!({"refs": ["tag:-101F1"]})),
            ("command.run", fields_set("@1")),
            ("command.run", fields_set("@7")),
        ],
    );
    for l in &lines {
        no_uuid(l);
    }
    assert_eq!(step(&lines[2])["state"]["canvas"]["selection"], json!(["@1"]));
    let device = shell.alias_uuid(1).unwrap();
    assert_eq!(step(&lines[3])["outcome"], "done", "{}", lines[3]);
    let refused = step(&lines[4]);
    assert_eq!(refused["outcome"], "failed");
    assert_eq!(
        refused["message"],
        "No record has the alias `@7` in this session."
    );
    // The record holds the real UUID as an `id:` reference, never the alias.
    let log = script::record_log(&shell);
    assert!(log.contains(&format!("{{\"ref\":\"id:{device}\"}}")), "{log}");
    assert!(!log.contains("\"@"), "{log}");
    assert_eq!(shell.recorded_commands().len(), 1);
    shell.shutdown();
}

/// The table lives for the whole script run (ID-630 (b)): a project opened or created mid-run keeps the numbering, so
/// an earlier alias still names its record (or names a record the new project lacks, refused); `run_stream` and `run`
/// each start a fresh table.
#[test]
fn the_table_lives_for_the_run_across_projects() {
    let tmp = Tmp::new("alias-run");
    let (mut shell, dir) = shell(&tmp);
    let new_dir = tmp.0.join("new.ew").to_string_lossy().into_owned();
    let lines = run(
        &mut shell,
        &[
            ("file.open", json!({"dir": dir, "read_only": true})),
            ("select.set", json!({"refs": ["tag:-101F1"]})),
            ("file.open", json!({"dir": dir, "read_only": true})),
            ("select.set", json!({"refs": ["tag:-102F1"]})),
            ("select.set", json!({"ids": ["@1"]})),
            ("file.new", json!({ "dir": new_dir })),
            ("select.set", json!({"ids": ["@1"]})),
        ],
    );
    for l in &lines {
        no_uuid(l);
    }
    assert_eq!(step(&lines[2])["state"]["canvas"]["selection"], json!(["@1"]));
    let device = shell.alias_uuid(1).unwrap();
    // After the reopen the next record gets the next number, and `@1` is still -101F1.
    assert_eq!(step(&lines[4])["state"]["canvas"]["selection"], json!(["@2"]));
    let again = step(&lines[5]);
    assert_eq!(again["outcome"], "done", "{again}");
    assert_eq!(again["state"]["canvas"]["selection"], json!(["@1"]));
    // In a new project `@1` still means -101F1, which that project lacks: refused by the resolver, naming `id:@1`.
    let gone = step(&lines[7]);
    assert_eq!(gone["outcome"], "failed", "{gone}");
    let message = gone["message"].as_str().unwrap();
    assert!(message.contains("id:@1"), "{message}");
    assert_eq!(shell.alias_uuid(1).as_deref(), Some(device.as_str()));
    // A streamed run starts a fresh table.
    let input = [
        "{\"$schema\":\"ew.uiscript/1\",\"label\":\"stream\"}".to_owned(),
        json!({"action": "select.set", "params": {"ids": ["@1"]}}).to_string(),
    ];
    let mut out = Vec::new();
    script::run_stream(
        &mut shell,
        &mut NoExtension,
        &mut input.into_iter().map(Ok),
        false,
        &mut |l| {
            out.push(l.to_owned());
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(shell.alias_count(), 0);
    assert_eq!(
        step(&out[1])["message"],
        "No record has the alias `@1` in this session."
    );
    shell.shutdown();
}

/// A `Failed` message of `command.run` that names a UUID prints its alias.
#[test]
fn a_failed_command_message_prints_the_alias() {
    let tmp = Tmp::new("alias-failed");
    let (mut shell, dir) = shell(&tmp);
    let missing = "00000000-0000-0000-0000-0000000000aa";
    let lines = run(
        &mut shell,
        &[
            ("file.open", json!({ "dir": dir })),
            (
                "command.run",
                json!({"cmd": "fields.set", "params": {"kind": "device",
                    "targets": [{"ref": format!("id:{missing}")}], "path": "function_text", "value": "x"}}),
            ),
        ],
    );
    let failed = step(&lines[2]);
    assert_eq!(failed["outcome"], "failed", "{failed}");
    let message = failed["message"].as_str().unwrap();
    no_uuid(message);
    let n = shell.alias_number(missing).expect("the UUID was printed");
    assert!(message.contains(&format!("id:@{n}")), "{message}");
    shell.shutdown();
}

/// Ruling 9 (fix round): a link field's value typed back as `id:@n` resolves to `id:<uuid>`; free text with `@n` in
/// it stays text; `select.set` `refs` and a command reference take `id:@n` too.
#[test]
fn a_typed_link_value_and_references_take_the_id_form() {
    let tmp = Tmp::new("alias-typed");
    let (mut shell, dir) = shell(&tmp);
    // A part copy of the project, read from the copy itself.
    let copy = {
        let (s, _) =
            ew_viewmodel::ew_api::Session::open_read_only(std::path::Path::new(&dir), "test").unwrap();
        let q = s.query();
        let id = q.parts().iter().last().map(|(id, _)| id.to_string()).unwrap();
        drop(q);
        drop(s);
        id
    };
    let lines = run(
        &mut shell,
        &[
            ("file.open", json!({ "dir": dir })),
            ("select.set", json!({ "ids": [copy] })),
            ("select.set", json!({"refs": ["tag:-101F1"]})),
            (
                "props.set",
                json!({"row": "device/parts[0].part", "value": "id:@1"}),
            ),
            (
                "props.set",
                json!({"row": "device/function_text", "value": "see id:@1 here"}),
            ),
            ("select.set", json!({"refs": ["id:@1"]})),
            ("select.set", json!({"refs": ["tag:-101F1"]})),
            (
                "command.run",
                json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "id:@2"}],
                    "path": "function_text", "value": "by id form"}}),
            ),
            (
                "props.set",
                json!({"row": "device/parts[0].part", "value": "id:@9"}),
            ),
        ],
    );
    for l in &lines {
        no_uuid(l);
    }
    assert_eq!(step(&lines[2])["state"]["canvas"]["selection"], json!(["@1"]));
    assert_eq!(step(&lines[3])["state"]["canvas"]["selection"], json!(["@2"]));
    for i in [4, 5, 6, 8] {
        assert_eq!(step(&lines[i])["outcome"], "done", "{}", lines[i]);
    }
    assert_eq!(step(&lines[6])["state"]["canvas"]["selection"], json!(["@1"]));
    let unknown = step(&lines[9]);
    assert_eq!(unknown["outcome"], "failed");
    assert_eq!(
        unknown["message"],
        "No record has the alias `@9` in this session."
    );
    // The record: the part as the copy's UUID, the free text as typed, the command's reference as `id:<uuid>`.
    let device = shell.alias_uuid(2).unwrap();
    let log = script::record_log(&shell);
    assert!(log.contains(&format!("\"copy\":\"{copy}\"")), "{log}");
    assert!(log.contains("\"see id:@1 here\""), "{log}");
    assert!(log.contains(&format!("{{\"ref\":\"id:{device}\"}}")), "{log}");
    shell.shutdown();
}
