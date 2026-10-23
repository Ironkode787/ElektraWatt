// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The ID alias `@n` (FT-422, decision log ID-630) through the headless runner on temporary copies of Golden A: no UUID
//! in a transcript line, aliases numbered by first appearance, `@n` accepted where an action parameter takes a UUID
//! or a reference (`select.set`, a navigator key, a command's `{"ref": "@n"}`), an unknown alias refused naming it,
//! `--record` holding the resolved UUID, and two runs of one script giving the same bytes.

mod common;

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
    script::run(shell, &mut NoExtension, &parsed, &mut |l| {
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
    let second = shell.aliases().uuid(2).unwrap().to_owned();
    assert!(
        shell.aliases().len() < 9,
        "the table holds {}",
        shell.aliases().len()
    );
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
        shell.aliases().uuid(2),
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
    let device = shell.aliases().uuid(1).unwrap().to_owned();
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

/// Acceptance 2: one script twice on equal inputs gives byte-identical transcripts, and the aliases are in them.
#[test]
fn two_runs_print_the_same_aliases() {
    let mut transcripts = Vec::new();
    for n in ["a", "b"] {
        let tmp = Tmp::new(&format!("alias-twice-{n}"));
        let (mut shell, dir) = shell(&tmp);
        let lines = run(
            &mut shell,
            &[
                ("file.open", json!({"dir": dir, "read_only": true})),
                ("page.goto", json!({"folio": "103"})),
                ("select.set", json!({"refs": ["tag:S51", "tag:-101F1"]})),
                ("props.show", json!({})),
                ("ui.describe", json!({})),
            ],
        );
        shell.shutdown();
        transcripts.push(lines.join("\n").replace(&dir, "DIR"));
    }
    assert_eq!(transcripts[0], transcripts[1]);
    no_uuid(&transcripts[0]);
    assert!(
        transcripts[0].contains("\"selection\":[\"@1\",\"@2\"]"),
        "{}",
        transcripts[0]
    );
}
