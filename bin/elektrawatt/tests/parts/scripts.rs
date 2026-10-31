// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.07 part 4a headless (PS9): the part dialog's and the part editor's flows of PS1, PS4, PS5 and PS7 as
//! `ew.uiscript/1` scripts through the built binary, each on its own copy of Golden A with an empty `EW_DATA_DIR`
//! (the compiled-in starter catalogue only) and its own `EW_CONFIG_DIR`. Per gate one run; the deep nightly runs each
//! twice and compares the transcripts byte for byte. The `--record` log of each editing script, replayed onto a fresh
//! copy by `Session::run_script` without any library (what `ew apply` runs), gives the saved project's records: the
//! chosen part travels inside the command. The model half of each flow runs in `ew-viewmodel`'s
//! `tests/part_selection.rs`. WP-S2.07 part 4b adds PS8's paste choice (two copies of Golden A over a user library
//! the test writes; the model half in `ew-viewmodel`'s `tests/paste_choice.rs`). A module of `tests/headless.rs`, whose
//! `bin_exe` it uses.

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
        let dir = std::env::temp_dir().join(format!("ew-parts-script-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
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

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn script(name: &str, steps: &[(&str, Value)]) -> String {
    let mut text = json!({"$schema": "ew.uiscript/1", "label": name}).to_string();
    text.push('\n');
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

/// Runs `steps` in `run` with its own configuration and an empty data directory, `--record` into `record.ewcmd`.
fn run(run: &Path, name: &str, steps: &[(&str, Value)]) -> Output {
    let file = run.join(format!("{name}.uiscript"));
    std::fs::write(&file, script(name, steps)).unwrap();
    std::fs::create_dir_all(run.join("data")).unwrap();
    Command::new(super::bin_exe())
        .args(["--headless", "--script"])
        .arg(&file)
        .arg("--record")
        .arg(run.join("record.ewcmd"))
        .env("EW_CONFIG_DIR", run.join("config"))
        .env("EW_DATA_DIR", run.join("data"))
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

/// The describe text of a `ui.describe` step.
fn describe(step: &Value) -> &str {
    step["describe"].as_str().expect("a describe step")
}

fn has_line(d: &str, line: &str) -> bool {
    d.lines().any(|l| l.trim_start() == line)
}

/// Runs `steps` once, or twice when `twice`, each run on its own copy of Golden A at `<side>/g.ew`; checks that every
/// step is done (a refusal the flow expects is `script.expect_refused`), that two runs give the same bytes, that no temporary
/// path reaches the transcript and that the `--record` log replayed onto a fresh copy without a library gives the
/// saved project's records. Returns the steps of the first run and the number of recorded commands.
fn run_flow(
    name: &str,
    twice: bool,
    steps: impl Fn(&Path) -> Vec<(&'static str, Value)>,
) -> (Vec<Value>, usize) {
    let tmp = Tmp::new();
    let mut transcripts = Vec::new();
    let mut first = None;
    let mut commands = 0;
    for side in super::support::runs(twice) {
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
        super::support::assert_no_uuid(&all);
        transcripts.push(all);
        let log = CmdLog::parse(&std::fs::read_to_string(dir.join("record.ewcmd")).unwrap()).unwrap();
        commands = log.commands().count();
        let fresh = dir.join("fresh.ew");
        copy_dir(Path::new(GOLDEN_A), &fresh);
        let (mut replayed, _) = Session::open_read_only(&fresh, "test").unwrap();
        replayed.run_script(&log, "record.ewcmd").unwrap();
        let (saved, _) = Session::open_read_only(&dir.join("g.ew"), "test").unwrap();
        let diff = replayed.diff_from(&saved.world()).unwrap();
        assert!(
            diff.model.changes.is_empty(),
            "the replay differs: {:?}",
            diff.model
        );
        first.get_or_insert(lines);
    }
    if twice {
        assert_eq!(transcripts[0], transcripts[1], "two runs give the same bytes");
    }
    (first.unwrap(), commands)
}

/// PS1: F55's part dialog lists the starter's 1P+N MCB (same FnDef), then its 1P MCB (same category), then the
/// others; Only parts that fit keeps the two; Escape commits nothing.
#[test]
fn ps1_the_part_dialog_script_is_deterministic() {
    ps1_the_part_dialog_script_is_deterministic_case(false);
}

pub(crate) fn ps1_the_part_dialog_script_is_deterministic_case(twice: bool) {
    let (lines, commands) = run_flow("ps1", twice, |project| {
        vec![
            ("file.open", json!({ "dir": path(project) })),
            ("page.goto", json!({"folio": "101"})),
            ("select.set", json!({"refs": ["tag:F55"]})),
            ("part.choose", json!({"use": 0})),
            ("ui.describe", json!({})),
            ("part.search", json!({"fits": true})),
            ("ui.describe", json!({})),
            ("ui.dismiss", json!({})),
        ]
    });
    assert_eq!(commands, 0, "the dialog commits nothing");
    let d = describe(&lines[4]);
    assert!(has_line(d, "dialog \"Choose a part for F55\""), "{d}");
    assert!(
        d.contains(
            "result 1 \"EW-GEN-MCB-2\" \"Generic\" \"Miniature circuit breaker, 1 pole and neutral\" library \"ElektraWatt starter parts\" compatible same-fndef cursor"
        ),
        "{d}"
    );
    assert!(d.contains("result 2 \"EW-GEN-MCB-1\" "), "{d}");
    let d = describe(&lines[6]);
    assert!(has_line(d, "results showing 1 to 2"), "{d}");
}

/// PS4: F55's first use → EW-GEN-MCB-2 (`part.assign` replace with `copies`), then Add part… with a held copy; the
/// record replays onto Golden A without a library.
#[test]
fn ps4_the_choose_script_is_deterministic_and_replays() {
    ps4_the_choose_script_is_deterministic_and_replays_case(false);
}

pub(crate) fn ps4_the_choose_script_is_deterministic_and_replays_case(twice: bool) {
    let (lines, commands) = run_flow("ps4", twice, |project| {
        vec![
            ("file.open", json!({ "dir": path(project) })),
            ("page.goto", json!({"folio": "101"})),
            ("select.set", json!({"refs": ["tag:F55"]})),
            ("part.choose", json!({"use": 0})),
            ("part.pick", json!({"index": 1})),
            ("part.choose", json!({"add": true})),
            ("part.pick", json!({"part": "A9P52610"})),
            ("ui.describe", json!({})),
            ("file.save", json!({})),
        ]
    });
    assert_eq!(commands, 2, "two choices, two commands");
    let d = describe(&lines[7]);
    assert!(
        d.lines().any(|l| l.contains("row \"device/parts\"")
            && l.contains("EW-GEN-MCB-2")
            && l.contains("A9P52610")),
        "{d}"
    );
}

/// PS5: the part editor's refusals: an empty manufacturer shows under its field and refuses Save; the history is
/// unchanged.
#[test]
fn ps5_the_editor_refusal_script_is_deterministic() {
    ps5_the_editor_refusal_script_is_deterministic_case(false);
}

pub(crate) fn ps5_the_editor_refusal_script_is_deterministic_case(twice: bool) {
    let (lines, commands) = run_flow("ps5", twice, |project| {
        vec![
            ("file.open", json!({ "dir": path(project) })),
            ("page.goto", json!({"folio": "101"})),
            ("select.set", json!({"refs": ["tag:F55"]})),
            ("part.edit", json!({})),
            ("part.field", json!({"field": "manufacturer", "value": ""})),
            ("script.expect_refused", json!({"action": "part.save"})),
            ("ui.describe", json!({})),
            ("ui.dismiss", json!({})),
            ("part.edit", json!({})),
            ("script.expect_refused", json!({"action": "part.save"})),
            ("ui.dismiss", json!({})),
        ]
    });
    assert_eq!(commands, 0, "nothing is committed");
    let d = describe(&lines[6]);
    assert!(
        has_line(d, "invalid \"Manufacturer\" \"A part needs a manufacturer\""),
        "{d}"
    );
    assert!(has_line(d, "error: A part needs a manufacturer"), "{d}");
}

/// PS7: a library copy forks: EW-GEN-MCB-2 on F55, Edit part…, a new description, Save: the card's fork note,
/// Apply; the record replays onto Golden A without a library.
#[test]
fn ps7_the_fork_script_is_deterministic_and_replays() {
    ps7_the_fork_script_is_deterministic_and_replays_case(false);
}

pub(crate) fn ps7_the_fork_script_is_deterministic_and_replays_case(twice: bool) {
    let (lines, commands) = run_flow("ps7", twice, |project| {
        vec![
            ("file.open", json!({ "dir": path(project) })),
            ("page.goto", json!({"folio": "101"})),
            ("select.set", json!({"refs": ["tag:F55"]})),
            ("part.choose", json!({"use": 0})),
            ("part.pick", json!({"part": "EW-GEN-MCB-2"})),
            ("part.edit", json!({})),
            (
                "part.field",
                json!({"field": "description", "value": "MCB 1P+N C4"}),
            ),
            ("part.save", json!({})),
            ("ui.describe", json!({})),
            ("preview.apply", json!({})),
            ("part.edit", json!({})),
            ("ui.describe", json!({})),
            ("ui.dismiss", json!({})),
            ("file.save", json!({})),
        ]
    });
    assert_eq!(commands, 2, "the choice and the fork");
    let d = describe(&lines[8]);
    assert!(d.lines().any(|l| l.trim_start().starts_with("note ")), "{d}");
    let d = describe(&lines[11]);
    assert!(
        has_line(d, "source project version \"0.1.0\" fork off"),
        "the second edit is in place: {d}"
    );
    assert!(has_line(d, "field \"Description\" \"MCB 1P+N C4\""), "{d}");
}

// WP-S2.07 part 4b

/// The user tiers of a run whose data directory is `data` (what `EW_DATA_DIR` gives the binary).
fn data_tiers(data: &Path) -> ew_ui::ew_viewmodel::ew_api::library::LibraryTiers {
    ew_ui::ew_viewmodel::ew_api::library::LibraryTiers {
        system: Vec::new(),
        community: data.join("packs"),
        company: Vec::new(),
        user: data.join("libraries"),
        cache: data.join("cache"),
    }
}

/// Writes FX-MCB-1PN by the manufacturer Fixture with `description` into the user library "Fixture parts" under `data`
/// (created on the first call) through the parts import into a library; `bump` writes a changed description as the
/// next patch version (1.0.0, then 1.0.1).
fn publish_fixture(data: &Path, description: &str, bump: bool) {
    use ew_ui::ew_viewmodel::ew_api::catalog::{self, ColumnMap, Limits};
    use ew_ui::ew_viewmodel::ew_api::library::Registry;
    let t = data_tiers(data);
    let mut reg = Registry::open(&t).unwrap();
    let lib = match reg.libraries().into_iter().find(|l| l.key == "fixture") {
        Some(l) => l.id,
        None => reg
            .create(
                &t.user.join("fixture.ewlib"),
                "fixture",
                "Fixture parts",
                "CC0-1.0",
            )
            .unwrap(),
    };
    let table = data.join("fixture-parts.csv");
    std::fs::write(
        &table,
        format!("Maker,Order code,Text,Category\nFixture,FX-MCB-1PN,{description},device\n"),
    )
    .unwrap();
    let map = ColumnMap::new(
        None,
        [
            ("manufacturer", "Maker"),
            ("part_number", "Order code"),
            ("description", "Text"),
            ("kind", "Category"),
        ],
    )
    .unwrap();
    let input = catalog::read_input(&table, &Limits::default()).unwrap();
    catalog::import_into_library(
        &mut reg,
        lib,
        &input,
        &map,
        "2026-10-31".parse().ok(),
        bump,
        false,
    )
    .unwrap();
}

/// Runs `steps` in `dir` with the data directory `data` and the configuration `<dir>/config`, `--record` into
/// `<dir>/record.ewcmd`; every step must be done.
fn run_with_data(dir: &Path, data: &Path, name: &str, steps: &[(&str, Value)]) -> Vec<Value> {
    std::fs::create_dir_all(dir).unwrap();
    let file = dir.join(format!("{name}.uiscript"));
    std::fs::write(&file, script(name, steps)).unwrap();
    let out = Command::new(super::bin_exe())
        .args(["--headless", "--script"])
        .arg(&file)
        .arg("--record")
        .arg(dir.join("record.ewcmd"))
        .env("EW_CONFIG_DIR", dir.join("config"))
        .env("EW_DATA_DIR", data)
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .unwrap();
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
    lines
}

/// The steps that give F55 of the project at `project` the fixture's FX-MCB-1PN as the library holds it now.
fn choose_fixture(project: &Path) -> Vec<(&'static str, Value)> {
    vec![
        ("file.open", json!({ "dir": path(project) })),
        ("page.goto", json!({"folio": "101"})),
        ("select.set", json!({"refs": ["tag:F55"]})),
        ("part.choose", json!({"use": 0})),
        ("part.pick", json!({"part": "FX-MCB-1PN (Fixture)"})),
        ("file.save", json!({})),
    ]
}

/// The id-free content of a project: its devices' tags, and the version of every part copy by part number.
fn tags_and_versions(s: &Session) -> (Vec<String>, Vec<(String, String)>) {
    let q = s.query();
    let mut tags: Vec<String> = q
        .devices()
        .iter()
        .map(|(id, _)| s.tag_list(&[id.as_uuid()]).trim_end().to_owned())
        .collect();
    tags.sort();
    let mut versions: Vec<(String, String)> = q
        .parts()
        .iter()
        .map(|(_, c)| (c.item.part_number.as_str().to_owned(), c.version.clone()))
        .collect();
    versions.sort();
    (tags, versions)
}

/// PS8 headless (D22, FT-396): P1 (a copy of Golden A) holds FX-MCB-1PN 1.0.0 on F55, P2 holds 1.0.1 (a description
/// change, a patch), both chosen through the part dialog by setup scripts over one user library written by the test.
/// The flow: F55 copied in P1, pasted in P2: `edit.paste_clip` is refused with `reason-paste-versions` and the choice
/// card lists the copy; Update the project's copy is refused (`reason-paste-update-older`); `paste.resolve` with
/// `adapt` arms the paste, which commits one `edit.paste` with `adapt`. The `--record` log replayed onto P2 as the
/// setup left it gives the same tags and part versions (the pasted IDs are minted).
#[test]
fn ps8_the_paste_choice_script_is_deterministic_and_replays() {
    ps8_the_paste_choice_script_is_deterministic_and_replays_case(false);
}

pub(crate) fn ps8_the_paste_choice_script_is_deterministic_and_replays_case(twice: bool) {
    let tmp = Tmp::new();
    let mut transcripts = Vec::new();
    for side in super::support::runs(twice) {
        let root = tmp.0.join(side);
        let data = root.join("data");
        let (p1, p2) = (root.join("p1.ew"), root.join("p2.ew"));
        copy_dir(Path::new(GOLDEN_A), &p1);
        copy_dir(Path::new(GOLDEN_A), &p2);
        publish_fixture(&data, "MCB 1P+N C10", false);
        run_with_data(&root.join("setup-1"), &data, "ps8-setup", &choose_fixture(&p1));
        publish_fixture(&data, "MCB 1P+N C10 new text", true);
        run_with_data(&root.join("setup-2"), &data, "ps8-setup", &choose_fixture(&p2));
        let start = root.join("p2-start.ew");
        copy_dir(&p2, &start);
        let flow = root.join("flow");
        let lines = run_with_data(
            &flow,
            &data,
            "ps8",
            &[
                ("file.open", json!({ "dir": path(&p1), "read_only": true })),
                ("page.goto", json!({"folio": "101"})),
                ("select.set", json!({"refs": ["tag:F55"]})),
                ("edit.copy", json!({})),
                ("file.open", json!({ "dir": path(&p2) })),
                ("page.goto", json!({"folio": "101"})),
                ("script.expect_refused", json!({"action": "edit.paste_clip"})),
                ("ui.describe", json!({})),
                (
                    "script.expect_refused",
                    json!({"action": "paste.resolve", "params": {"choice": "update"}}),
                ),
                ("paste.resolve", json!({"choice": "adapt"})),
                ("canvas.pointer", json!({"x_mm": 120.0, "y_mm": 90.0})),
                ("canvas.press", json!({"x_mm": 120.0, "y_mm": 90.0})),
                ("file.save", json!({})),
            ],
        );
        let d = describe(&lines[7]);
        assert!(
            has_line(
                d,
                "conflict \"FX-MCB-1PN (Fixture)\" project \"1.0.1\" fragment \"1.0.0\" patch"
            ),
            "{d}"
        );
        assert!(has_line(d, "choice adapt enabled"), "{d}");
        assert!(
            has_line(
                d,
                "choice update disabled: \"The pasted version is not newer than the project's\""
            ),
            "{d}"
        );
        let log = CmdLog::parse(&std::fs::read_to_string(flow.join("record.ewcmd")).unwrap()).unwrap();
        let commands: Vec<_> = log.commands().collect();
        assert_eq!(commands.len(), 1, "one edit.paste");
        let all = String::from_utf8(std::fs::read(flow.join("record.ewcmd")).unwrap()).unwrap();
        assert!(all.contains("\"adapt\""), "the paste carries adapt");
        let (mut replayed, _) = Session::open_read_only(&start, "test").unwrap();
        replayed.run_script(&log, "record.ewcmd").unwrap();
        let (saved, _) = Session::open_read_only(&p2, "test").unwrap();
        let (tags, versions) = tags_and_versions(&saved);
        assert_eq!(tags_and_versions(&replayed), (tags.clone(), versions.clone()));
        assert!(
            versions.contains(&("FX-MCB-1PN".to_owned(), "1.0.1".to_owned())),
            "{versions:?}"
        );
        let transcript: String = lines.iter().map(|l| format!("{l}\n")).collect();
        assert!(!transcript.contains(&*path(&tmp.0)), "a path in the transcript");
        super::support::assert_no_uuid(&transcript);
        transcripts.push(transcript);
    }
    if twice {
        assert_eq!(transcripts[0], transcripts[1], "two runs give the same bytes");
    }
}
