// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-516, WP-S2.09 part 2 headless (§9 items 1 and 3): the message panel's flows MS1 to MS4 as `ew.uiscript/1`
//! scripts through the built binary, each run twice on its own copy of Golden A with byte-identical transcripts: View ▸
//! Messages (`messages.show`), F8 and Shift+F8 (`messages.next`, `messages.previous`) walking, wrapping and docking the
//! closed pane, Enter (`navigator.open`) opening EF-08, a pending finding that `messages.check` ends, and the quick fix
//! (`messages.fix`) applied from its card. The `--record` log of each editing script replayed onto a fresh copy gives
//! the saved project's records. The model half of each flow runs in `ew-viewmodel`'s `tests/messages.rs`. A module of
//! `tests/headless.rs`, whose `bin_exe` it uses.

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
        let dir = std::env::temp_dir().join(format!("ew-messages-script-{}-{n}", std::process::id()));
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
    Command::new(super::bin_exe())
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

/// Runs `steps` twice, each on its own copy of Golden A at `<side>/g.ew`, checks every step is done, that the
/// transcripts are byte-identical and that the `--record` log replayed onto a fresh copy gives the saved project's
/// records (up to the IDs of records it created, when `new_records`); returns the steps of the first run and the number of recorded commands.
fn twice(
    name: &str,
    new_records: bool,
    steps: impl Fn(&Path) -> Vec<(&'static str, Value)>,
) -> (Vec<Value>, usize) {
    let tmp = Tmp::new();
    let mut transcripts = Vec::new();
    let mut first = None;
    let mut commands = 0;
    for side in ["a", "b"] {
        let dir = tmp.0.join(side);
        copy_dir(Path::new(GOLDEN_A), &dir.join("g.ew"));
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
        assert!(!all.contains(&*path(&tmp.0)), "a path in the transcript");
        transcripts.push(all);
        // The record replayed onto a fresh copy: the records of the saved project.
        let log = CmdLog::parse(&std::fs::read_to_string(dir.join("record.ewcmd")).unwrap()).unwrap();
        commands = log.commands().count();
        let fresh = dir.join("fresh.ew");
        copy_dir(Path::new(GOLDEN_A), &fresh);
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
    assert_eq!(transcripts[0], transcripts[1], "two runs give the same bytes");
    (first.unwrap(), commands)
}

/// MS1 and MS4: View ▸ Messages, F8 and Shift+F8 with the pane closed, walking and wrapping, and Enter on EF-08.
#[test]
fn ms1_ms4_view_messages_f8_shift_f8_and_enter_on_ef08_are_byte_identical_twice() {
    let (lines, commands) = twice("ms1-ms4", false, |project| {
        vec![
            ("file.open", json!({ "dir": path(project) })),
            ("page.goto", json!({"folio": "102"})),
            // View ▸ Messages.
            ("messages.show", json!({})),
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
            // EF-08 under the cursor, the tab elsewhere, Enter.
            ("navigator.filter", json!({"kind": "messages", "text": "-3W1"})),
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
    assert_eq!(
        line(d, "  messages scheme "),
        "  messages scheme \"drafting\" errors 1 warnings 0 infos 10 waived 0 pending 0"
    );
    let rows = finding_rows(d);
    assert_eq!(rows.len(), 11, "{d}");
    assert!(
        rows[0].contains("-3W1") && rows[0].ends_with(" badge \"error\""),
        "{d}"
    );
    // The cursor of each walk step: the first, the second, the last, the first.
    let cursor = |i: usize| -> usize {
        let rows = finding_rows(describe(&lines[i]));
        assert_eq!(rows.len(), 11);
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
        lines[5]["state"]["page"], "3",
        "F8 opened the first finding's page"
    );
    assert_eq!(cursor(8), 1);
    assert_eq!(cursor(11), 10, "Shift+F8 from the first wraps to the last");
    assert_eq!(cursor(13), 0, "F8 from the last wraps to the first");
    // Enter on EF-08 opens -3W1's page with it selected.
    assert_eq!(lines[15]["state"]["page"], "1");
    assert_eq!(lines[16]["state"]["page"], "3");
    let d = describe(&lines[17]);
    let rows = finding_rows(d);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].contains("-3W1") && rows[0].ends_with(" cursor"), "{d}");
    assert_eq!(
        line(d, "  detail rule "),
        "  detail rule \"EW-CORE-033\" title \"Cable without length\" kind completeness severity error"
    );
    assert_eq!(
        line(d, "properties "),
        "properties \"-3W1\" items 1 kinds representation 1"
    );
}

/// MS2: a new unpaired interruption point on the editing page is pending; `messages.check` ends it.
#[test]
fn ms2_messages_check_ends_pending_byte_identical_twice_and_replays() {
    let (lines, commands) = twice("ms2", true, |project| {
        vec![
            ("file.open", json!({ "dir": path(project) })),
            ("page.goto", json!({"folio": "101"})),
            ("view.zoom_actual", json!({})),
            ("messages.show", json!({})),
            ("tool.ip", json!({})),
            ("cursor.set", json!({"x_mm": 300.0, "y_mm": 30.0})),
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
    assert_eq!(
        line(d, "  messages scheme "),
        "  messages scheme \"drafting\" errors 1 warnings 0 infos 10 waived 0 pending 1",
        "errors unchanged, pending 1"
    );
    let rows = finding_rows(d);
    assert_eq!(rows.len(), 1, "{d}");
    assert!(rows[0].contains("interruption point MSGTEST"), "{d}");
    assert!(rows[0].contains(" badge \"error\" badge \"pending\""), "{d}");
    let d = describe(&lines[12]);
    assert_eq!(
        line(d, "  messages scheme "),
        "  messages scheme \"drafting\" errors 2 warnings 0 infos 10 waived 0 pending 0",
        "Run checks makes it an error"
    );
    let rows = finding_rows(d);
    assert_eq!(rows.len(), 1, "{d}");
    assert!(!rows[0].contains("\"pending\""), "{d}");
}

/// MS3: the quick fix of EW-CORE-023 opened with `messages.fix` on the cursor's finding and applied from its card.
#[test]
fn ms3_messages_fix_with_apply_is_byte_identical_twice_and_replays() {
    let (lines, commands) = twice("ms3", false, |project| {
        vec![
            ("file.open", json!({ "dir": path(project) })),
            ("page.goto", json!({"folio": "102"})),
            (
                "command.run",
                json!({"cmd": "fields.set", "params": {"kind": "page", "targets": [{"ref": "folio:102"}],
                    "path": "description_override", "value": "Overridden"}}),
            ),
            ("messages.show", json!({})),
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
    assert_eq!(line(d, "  detail fix "), "  detail fix \"drop override\"");
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
    assert_eq!(
        line(d, "  messages scheme "),
        "  messages scheme \"drafting\" errors 1 warnings 0 infos 10 waived 0 pending 0"
    );
}
