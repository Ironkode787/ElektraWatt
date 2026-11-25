// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The message panel's flows, headless, as `ew.uiscript/1`
//! scripts through the built binary, each run twice on its own copy of the sample with byte-identical transcripts: View ▸
//! Messages (`messages.show`), F8 and Shift+F8 (`messages.next`, `messages.previous`) walking, wrapping and docking the
//! closed pane, Enter (`navigator.open`) opening a finding's page, a pending finding that `messages.check` ends, and the quick fix
//! (`messages.fix`) applied from its card. The `--record` log of each editing script replayed onto a fresh copy gives
//! the saved project's records. The model half of each flow runs in `ew-viewmodel`'s `tests/messages.rs`. A module of
//! `tests/it/headless.rs`, whose `bin_exe` it uses.

use std::path::Path;
use std::process::{Output, Stdio};

use ew_viewmodel::ew_api::{CmdLog, Session};
use serde_json::{Value, json};

use crate::support::{SAMPLE, Tmp, copy_dir};

fn script(steps: &[(&str, Value)]) -> String {
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"messages\"}\n");
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

/// Runs `steps` in `run` (a folder with its own config directory) with `--record` into `record.ewcmd` there.
fn run(run: &Path, name: &str, steps: &[(&str, Value)]) -> Output {
    let file = run.join(format!("{name}.uiscript"));
    std::fs::write(&file, script(steps)).unwrap();
    crate::support::elektrawatt(&super::bin_exe())
        .args(["--headless", "--script"])
        .arg(&file)
        .arg("--record")
        .arg(run.join("record.ewcmd"))
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

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// The describe text of a `ui.describe` step.
fn describe(step: &Value) -> &str {
    step["describe"].as_str().expect("a describe step")
}

/// The line of `d` that starts with `prefix`.
fn line<'a>(d: &'a str, prefix: &str) -> &'a str {
    d.lines()
        .find(|l| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("no line {prefix}:\n{d}"))
}

/// The Messages pane's finding rows of `d`, in order.
fn finding_rows(d: &str) -> Vec<&str> {
    d.lines()
        .filter(|l| l.starts_with("  row level 3 finding \""))
        .collect()
}

/// The sample's copy in `dir` with two findings, made as a person makes them at the window: the shield wire of
/// -103W1 set to 1.5 mm², below its 2.5 mm² line conductors (EW-POT-006, an error on 103), and -101W1 without its
/// length (EW-CORE-033, a warning). The sample itself is clean.
fn with_findings(dir: &Path) {
    // -101W1, the one cable of 10 m.
    let cable = {
        let devices = std::fs::read_dir(dir.join("devices")).unwrap();
        devices
            .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
            .find_map(|t| {
                let line = t
                    .lines()
                    .find(|l| l.contains(r#""length":{"manual":10000000000}"#))?;
                Some(serde_json::from_str::<Value>(line).unwrap()["id"].clone())
            })
            .expect("the cable -101W1")
    };
    let mut store = ew_viewmodel::ew_api::StoreOptions::new("test");
    store.state_dir = None;
    let (mut s, _) = Session::open(
        dir,
        store,
        ew_viewmodel::ew_api::SessionOptions::default(),
        ew_viewmodel::ew_api::Recovery::Keep,
    )
    .unwrap();
    // The shield wire, found by its ends: the terminal -X1031:SH and the motor -103M1.
    let [terminal, motor] =
        ["-X1031:SH", "-103M1"].map(|t| s.resolve_reference(&format!("tag:{t}")).unwrap());
    let q = s.query();
    let shield = q
        .connections()
        .iter()
        .find(|(_, c)| {
            let ends = [&c.a, &c.b].map(|e| {
                e.function()
                    .and_then(|f| q.functions().get(f))
                    .and_then(|f| f.device)
                    .map(|d| d.as_uuid())
            });
            ends == [Some(terminal), Some(motor)] || ends == [Some(motor), Some(terminal)]
        })
        .map(|(id, _)| id.to_string())
        .expect("the shield wire");
    drop(q);
    for params in [
        json!({"kind": "connection", "targets": [shield], "path": "overrides.csa", "value": "1.5"}),
        json!({"kind": "device", "targets": [cable], "path": "cable.length", "value": null}),
    ] {
        let cmd = ew_viewmodel::ew_api::AnyCommand {
            name: "fields.set".into(),
            params,
        };
        s.execute(&cmd).unwrap();
    }
    s.save().unwrap();
    s.close();
}

/// Runs `steps` once, or twice when `twice` (the deep nightly), each run on its own copy of the sample (with the two
/// findings of [`with_findings`] when `findings`) at
/// `<side>/g.ew`; checks every step is done, that the transcripts of two runs are byte-identical and that the `--record` log replayed onto a fresh copy gives the saved project's
/// records (up to the IDs of records it created, when `new_records`); returns the steps of the first run and the number of recorded commands.
fn run_flow(
    name: &str,
    new_records: bool,
    findings: bool,
    twice: bool,
    steps: impl Fn(&Path) -> Vec<(&'static str, Value)>,
) -> (Vec<Value>, usize) {
    let tmp = Tmp::new();
    let mut transcripts = Vec::new();
    let mut first = None;
    let mut commands = 0;
    for side in crate::support::runs(twice) {
        let dir = tmp.dir().join(side);
        copy_dir(Path::new(SAMPLE), &dir.join("g.ew"));
        if findings {
            with_findings(&dir.join("g.ew"));
        }
        let out = run(&dir, name, &steps(&dir.join("g.ew")));
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let lines = steps_of(&out);
        for s in &lines {
            assert_eq!(s["outcome"], "done", "{s}");
        }
        let all = String::from_utf8(out.stdout.clone()).unwrap();
        assert!(!all.contains(&*path(tmp.dir())), "a path in the transcript");
        transcripts.push(all);
        // The record replayed onto a fresh copy: the records of the saved project.
        let log = CmdLog::parse(&std::fs::read_to_string(dir.join("record.ewcmd")).unwrap()).unwrap();
        commands = log.commands().count();
        let fresh = dir.join("fresh.ew");
        copy_dir(Path::new(SAMPLE), &fresh);
        if findings {
            with_findings(&fresh);
        }
        let (mut replayed, _) = Session::open_read_only(&fresh, "test").unwrap();
        replayed.run_script(&log, "record.ewcmd").unwrap();
        let (saved, _) = Session::open_read_only(&dir.join("g.ew"), "test").unwrap();
        let diff = replayed.diff_from(&saved.world()).unwrap();
        // A record the script created has a new ID on replay: such records must pair up (one removed and one added
        // of each kind, no changed fields); everything else is equal.
        let mut added: Vec<String> = Vec::new();
        let mut removed: Vec<String> = Vec::new();
        for c in &diff.model.changes {
            match format!("{:?}", c.change).as_str() {
                "Added" => added.push(format!("{:?}", c.key.0)),
                "Removed" => removed.push(format!("{:?}", c.key.0)),
                _ => panic!("a changed record on replay: {c:?}"),
            }
        }
        added.sort();
        removed.sort();
        assert_eq!(added, removed, "{:?}", diff.model);
        assert!(new_records || added.is_empty(), "{:?}", diff.model);
        first.get_or_insert(lines);
    }
    if twice {
        assert_eq!(transcripts[0], transcripts[1], "two runs give the same bytes");
    }
    (first.unwrap(), commands)
}

/// MS1 and MS4: View ▸ Messages, F8 and Shift+F8 with the pane closed, walking and wrapping, and Enter on a cable's
/// finding.
/// Per gate one run of the script (`twice` false); the deep nightly's
/// `nightly_every_script_transcript_is_byte_identical_twice` runs the body with `twice` true: a second run on an
/// equal start and the byte-identical comparison of the two transcripts (test clean-up, 2026-10-30).
#[test]
fn view_messages_f8_shift_f8_and_enter_on_a_finding_are_byte_identical_twice() {
    ms1_ms4_view_messages_f8_shift_f8_and_enter_on_a_finding_are_byte_identical_twice_case(false);
}

pub(crate) fn ms1_ms4_view_messages_f8_shift_f8_and_enter_on_a_finding_are_byte_identical_twice_case(
    twice: bool,
) {
    let (lines, commands) = run_flow("ms1-ms4", false, true, twice, |project| {
        vec![
            ("file.open", json!({ "dir": path(project) })),
            ("page.goto", json!({"folio": "102"})),
            // View ▸ Messages.
            ("messages.show", json!({"on": true})),
            ("ui.describe", json!({})),
            ("messages.show", json!({"on": false})),
            // F8 with the pane closed docks it and goes to the first finding; F8 again to the second.
            ("messages.next", json!({})),
            ("ui.describe", json!({})),
            ("messages.next", json!({})),
            ("ui.describe", json!({})),
            // Shift+F8 twice: back to the first, then wrapping to the last.
            ("messages.previous", json!({})),
            ("messages.previous", json!({})),
            ("ui.describe", json!({})),
            // F8 from the last wraps to the first.
            ("messages.next", json!({})),
            ("ui.describe", json!({})),
            // The shield's finding under the cursor, the tab elsewhere, Enter. The filter (its words "PE wire") hides
            // -101W1's finding, which has no page; F8 puts the cursor on the shield's.
            ("navigator.filter", json!({"kind": "messages", "text": "PE wire"})),
            ("messages.next", json!({})),
            ("page.goto", json!({"folio": "1"})),
            ("navigator.open", json!({"kind": "messages"})),
            ("ui.describe", json!({})),
            ("file.save", json!({})),
        ]
    });
    assert_eq!(commands, 0, "the flows commit nothing");
    // View ▸ Messages: the menu item checked, the pane with its header and findings.
    let d = describe(&lines[3]);
    assert!(
        d.contains("  item messages.show \"Messages\" (checked) enabled"),
        "{d}"
    );
    assert!(
        line(d, "navigator \"Messages\"").starts_with(
            "navigator \"Messages\" kind messages view tree sort \"Label\" ascending filter \"\" rows "
        ),
        "{d}"
    );
    // The set-up's error and warning (`with_findings`) beside the sample's infos.
    let scheme = line(d, "  messages scheme ");
    assert!(
        scheme.starts_with("  messages scheme \"Drafting\" errors 1 warnings 1 infos ")
            && scheme.ends_with(" waived 0 pending 0"),
        "{scheme}"
    );
    let rows = finding_rows(d);
    assert!(rows.len() >= 3, "{d}");
    // The error leads: the shield's EW-POT-006 on 103; -101W1's EW-CORE-033 is a warning while the drawing grows.
    assert!(
        rows[0].contains("-103W1/SHIELD") && rows[0].ends_with(" badge \"error\""),
        "{d}"
    );
    assert!(
        rows.iter()
            .any(|r| r.contains("-101W1") && r.ends_with(" badge \"warning\"")),
        "{d}"
    );
    // The cursor of each walk step: the first, the second, the last, the first. The row count is not pinned: it
    // follows the sample's findings, which grow with the rule packs.
    let cursor = |i: usize| -> usize {
        let rows = finding_rows(describe(&lines[i]));
        let at: Vec<usize> = (0..rows.len())
            .filter(|k| rows[*k].ends_with(" cursor"))
            .collect();
        assert_eq!(at.len(), 1, "one cursor row at step {}", i + 1);
        at[0]
    };
    assert!(describe(&lines[6]).contains("  item messages.show \"Messages\" (checked) enabled"));
    assert_eq!(
        cursor(6),
        0,
        "F8 docks the closed pane and goes to the first finding"
    );
    assert_eq!(
        lines[5]["state"]["page"], "103",
        "F8 opened the first finding's page"
    );
    assert_eq!(cursor(8), 1);
    let last = finding_rows(describe(&lines[11])).len() - 1;
    assert_eq!(cursor(11), last, "Shift+F8 from the first wraps to the last");
    assert_eq!(cursor(13), 0, "F8 from the last wraps to the first");
    // Enter on the shield's finding opens its page, 103, with the wire selected.
    assert_eq!(lines[16]["state"]["page"], "1");
    assert_eq!(lines[17]["state"]["page"], "103");
    let d = describe(&lines[18]);
    let rows = finding_rows(d);
    // The count of the rows the filter keeps is not pinned: the cursor is on the shield wire.
    let at: Vec<&&str> = rows.iter().filter(|r| r.ends_with(" cursor")).collect();
    assert!(at.len() == 1 && at[0].contains("-103W1/SHIELD"), "{d}");
    assert_eq!(
        line(d, "  detail rule "),
        "  detail rule \"EW-POT-006\" title \"PE undersized\" kind safety severity error"
    );
    assert!(line(d, "properties ").contains(" items 1 "), "{d}");
}

/// MS2: a new unpaired interruption point on the editing page is pending; `messages.check` ends it.
/// Per gate one run of the script (`twice` false); the deep nightly's
/// `nightly_every_script_transcript_is_byte_identical_twice` runs the body with `twice` true: a second run on an
/// equal start and the byte-identical comparison of the two transcripts (test clean-up, 2026-10-30).
#[test]
fn messages_check_ends_pending_byte_identical_twice_and_replays() {
    ms2_messages_check_ends_pending_byte_identical_twice_and_replays_case(false);
}

pub(crate) fn ms2_messages_check_ends_pending_byte_identical_twice_and_replays_case(twice: bool) {
    let (lines, commands) = run_flow("ms2", true, false, twice, |project| {
        vec![
            ("file.open", json!({ "dir": path(project) })),
            ("page.goto", json!({"folio": "101"})),
            ("view.zoom_actual", json!({})),
            ("messages.show", json!({"on": true})),
            ("tool.ip", json!({})),
            ("cursor.set", json!({"x_mm": 260.0, "y_mm": 30.0})),
            ("ip.name", json!({"text": "MSGTEST"})),
            ("canvas.activate", json!({})),
            ("ui.dismiss", json!({})),
            ("navigator.filter", json!({"kind": "messages", "text": "MSGTEST"})),
            ("ui.describe", json!({})),
            // Run checks.
            ("messages.check", json!({})),
            ("ui.describe", json!({})),
            ("file.save", json!({})),
        ]
    });
    assert_eq!(commands, 1, "the placed point");
    let d = describe(&lines[10]);
    // The sample has no error: the new point's is pending until the check runs.
    assert!(
        line(d, "  messages scheme ").starts_with("  messages scheme \"Drafting\" errors 0 warnings 0 "),
        "{d}"
    );
    let rows = finding_rows(d);
    assert_eq!(rows.len(), 1, "{d}");
    assert!(rows[0].contains("interruption point MSGTEST"), "{d}");
    assert!(rows[0].contains(" badge \"error\" badge \"pending\""), "{d}");
    let d = describe(&lines[12]);
    let scheme = line(d, "  messages scheme ");
    assert!(
        scheme.starts_with("  messages scheme \"Drafting\" errors 1 warnings 0 ")
            && scheme.ends_with(" pending 0"),
        "Run checks makes it an error: {scheme}"
    );
    let rows = finding_rows(d);
    assert_eq!(rows.len(), 1, "{d}");
    assert!(!rows[0].contains("\"pending\""), "{d}");
}

/// MS3: the quick fix of EW-CORE-023 opened with `messages.fix` on the cursor's finding and applied from its card.
/// Per gate one run of the script (`twice` false); the deep nightly's
/// `nightly_every_script_transcript_is_byte_identical_twice` runs the body with `twice` true: a second run on an
/// equal start and the byte-identical comparison of the two transcripts (test clean-up, 2026-10-30).
#[test]
fn messages_fix_with_apply_is_byte_identical_twice_and_replays() {
    ms3_messages_fix_with_apply_is_byte_identical_twice_and_replays_case(false);
}

pub(crate) fn ms3_messages_fix_with_apply_is_byte_identical_twice_and_replays_case(twice: bool) {
    let (lines, commands) = run_flow("ms3", false, false, twice, |project| {
        vec![
            ("file.open", json!({ "dir": path(project) })),
            ("page.goto", json!({"folio": "102"})),
            (
                "command.run",
                json!({"cmd": "fields.set", "params": {"kind": "page", "targets": [{"ref": "folio:102"}],
                    "path": "description_override", "value": "Overridden"}}),
            ),
            ("messages.show", json!({"on": true})),
            (
                "navigator.filter",
                json!({"kind": "messages", "text": "EW-CORE-023"}),
            ),
            ("messages.next", json!({})),
            ("messages.fix", json!({})),
            ("ui.describe", json!({})),
            ("preview.apply", json!({})),
            ("ui.describe", json!({})),
            ("file.save", json!({})),
        ]
    });
    assert_eq!(commands, 2, "the override and the quick fix");
    let d = describe(&lines[7]);
    let rows = finding_rows(d);
    assert_eq!(rows.len(), 1, "{d}");
    assert!(
        rows[0].contains(" badge \"info\" badge \"fix\"") && rows[0].ends_with(" cursor"),
        "{d}"
    );
    assert_eq!(line(d, "  detail fix "), "  detail fix \"Drop override\"");
    let card = line(d, "card ");
    assert!(
        card.starts_with("card ready \"fields.set\" targets 1 changes 1 "),
        "{card}"
    );
    assert!(card.ends_with(" fix \"EW-CORE-023\""), "{card}");
    // Nothing was committed by the fix itself; Apply commits once and removes the finding.
    assert_eq!(lines[6]["state"]["dirty"], true);
    let d = describe(&lines[9]);
    assert!(finding_rows(d).is_empty(), "{d}");
    assert!(!d.contains("\ncard "), "{d}");
    // The sample has no error or warning; folio 102's findings would be pending as the edited page's (§7.2).
    assert!(
        line(d, "  messages scheme ").starts_with("  messages scheme \"Drafting\" errors 0 warnings 0 "),
        "{d}"
    );
}
