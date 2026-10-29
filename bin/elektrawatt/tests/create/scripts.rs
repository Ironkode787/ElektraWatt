// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The helpers and the script-test bodies of `create.rs`, shared with the deep nightly's
//! `headless::nightly_every_script_transcript_is_byte_identical_twice`: `create.rs` runs each body with
//! `twice` false (one run of the script per gate, then its checks and the `--record` replay), the nightly with
//! `twice` true (a second run on an equal start and the byte-identical comparison of the two transcripts).

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_ui::ew_viewmodel::ew_api::{CmdLog, Session};
use serde_json::{Value, json};

pub(crate) const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
pub(crate) struct Tmp(pub(crate) PathBuf);

impl Tmp {
    pub(crate) fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-create-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn copy_dir(from: &Path, to: &Path) {
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

pub(crate) fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

pub(crate) type Steps = Vec<(String, Value)>;

pub(crate) fn steps(list: &[(&str, Value)]) -> Steps {
    list.iter().map(|(a, p)| ((*a).to_owned(), p.clone())).collect()
}

pub(crate) fn script_text(steps: &Steps) -> String {
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"create script\"}\n");
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

/// The path of the `elektrawatt` binary under test (nextest sets it at run time; see `tests/headless.rs`).
// Allowed: the program boundary of a test — nextest's binary path is read once, never persisted or printed.
#[allow(clippy::disallowed_methods)]
pub(crate) fn bin_exe() -> PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_elektrawatt")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_elektrawatt"))) // bin-exe-fallback
}

/// Runs a script headless (with the content of the source tree, a configuration folder of its own), optionally recording
/// the committed commands.
pub(crate) fn run(tmp: &Tmp, name: &str, steps: &Steps, record: Option<&Path>) -> Output {
    let file = tmp.path(&format!("{name}.jsonl"));
    std::fs::write(&file, script_text(steps)).unwrap();
    let mut cmd = Command::new(bin_exe());
    cmd.args(["--headless", "--script"]).arg(&file);
    if let Some(r) = record {
        cmd.arg("--record").arg(r);
    }
    cmd.env("EW_CONFIG_DIR", tmp.path(&format!("config-{name}")))
        .env("EW_CONTENT_DIR", CONTENT)
        .env_remove("EW_SAMPLE_DIR")
        .env_remove("EW_DOCS_DIR")
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// Every step of a transcript was done; the transcript.
#[track_caller]
pub(crate) fn all_done(out: &Output) -> String {
    let text = String::from_utf8(out.stdout.clone()).unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}\n{text}",
        String::from_utf8_lossy(&out.stderr)
    );
    for line in text.lines().skip(1) {
        let step: Value = serde_json::from_str(line).unwrap();
        assert_eq!(step["outcome"], "done", "{step}");
    }
    text
}

/// The text of the `describe` member of the transcript's `ui.describe` step number `n` (1-based).
pub(crate) fn describe_of(transcript: &str, action: &str) -> String {
    transcript
        .lines()
        .skip(1)
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|s| s["action"] == action)
        .and_then(|s| s["describe"].as_str().map(str::to_owned))
        .unwrap_or_else(|| panic!("no {action} step"))
}

/// A new project at `name` of `tmp` (the builtin template), saved and closed.
pub(crate) fn new_project(tmp: &Tmp, name: &str, more: &[(&str, Value)]) -> PathBuf {
    let dir = tmp.path(name);
    let mut script = steps(&[("file.new", json!({"dir": path(&dir), "template": "builtin"}))]);
    script.extend(steps(more));
    script.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    all_done(&run(tmp, name, &script, None));
    dir
}

/// Panics at the first line two transcripts differ in.
#[track_caller]
pub(crate) fn same_transcript(a: &str, b: &str) {
    if let Some((x, y)) = a.lines().zip(b.lines()).find(|(x, y)| x != y) {
        let at = x.bytes().zip(y.bytes()).position(|(p, q)| p != q).unwrap_or(0);
        panic!(
            "two runs, two transcripts: first difference at {at}: {} | {}",
            &x[at.saturating_sub(120)..(at + 80).min(x.len())],
            &y[at.saturating_sub(120)..(at + 80).min(y.len())]
        );
    }
    assert_eq!(a, b, "two runs, one transcript");
}

/// FT-408's acceptance (a): folios 101 to 103 on a new project.
pub(crate) fn page_new_creates_folios_101_to_103_twice_identically_and_the_record_replays_case(twice: bool) {
    let tmp = Tmp::new();
    let start = new_project(&tmp, "start.ew", &[]);
    let (start_session, _) = Session::open_read_only(&start, "test").unwrap();
    let before = start_session.page_labels();
    drop(start_session);
    let script = |dir: &Path| {
        steps(&[
            ("file.open", json!({"dir": path(dir)})),
            (
                "page.new",
                json!({"folio": "101", "type": "multi_line", "title": "Power Supply 1 - Main Current"}),
            ),
            (
                "page.new",
                json!({"folio": "102", "title": "Power Supply 2 - Control Current"}),
            ),
            (
                "page.new",
                json!({"folio": "103", "type": "multi_line", "title": "Pneumatics"}),
            ),
            ("ui.describe", json!({})),
            ("page.describe", json!({"folio": "103"})),
            ("file.save", json!({})),
            ("file.close", json!({})),
        ])
    };
    let mut transcripts = Vec::new();
    let mut records = Vec::new();
    for n in super::support::runs(twice) {
        let dir = tmp.path(n).join("p.ew");
        copy_dir(&start, &dir);
        let record = tmp.path(&format!("{n}.ewcmd"));
        let text = all_done(&run(&tmp, n, &script(&dir), Some(&record)));
        assert!(
            !text.contains(&*tmp.0.to_string_lossy()),
            "a path in the transcript"
        );
        super::support::assert_no_uuid(&text);
        transcripts.push(text);
        records.push(record);
    }
    if twice {
        same_transcript(&transcripts[0], &transcripts[1]);
    }
    // The describe text lists the three pages.
    let describe = describe_of(&transcripts[0], "ui.describe");
    for (folio, title) in [
        ("101", "Power Supply 1 - Main Current"),
        ("102", "Power Supply 2 - Control Current"),
        ("103", "Pneumatics"),
    ] {
        assert!(
            describe.lines().any(|l| l.trim_start().starts_with("row ")
                && l.contains(&format!("page \"{folio}\" \"{title}\""))),
            "no row of {folio} in:\n{describe}"
        );
    }
    // The new page is on show after each step, and the last one after the three.
    assert!(describe.contains("page view: 103"), "{describe}");
    // `--record` holds the three `page.create` lines and replays on a third copy of the start.
    let log = CmdLog::parse(&std::fs::read_to_string(&records[0]).unwrap()).unwrap();
    let names: Vec<&str> = log.commands().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["page.create", "page.create", "page.create"]);
    let replay = tmp.path("c").join("p.ew");
    copy_dir(&start, &replay);
    let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
    store.state_dir = Some(tmp.path("state"));
    let (mut s, _) = Session::open(
        &replay,
        store,
        ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
        ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
    )
    .unwrap();
    s.run_script(&log, "a.ewcmd").unwrap();
    let (scripted, _) = Session::open_read_only(&tmp.path("a").join("p.ew"), "test").unwrap();
    assert_eq!(
        s.page_labels(),
        scripted.page_labels(),
        "the replay equals the scripted session"
    );
    let labels = s.page_labels();
    assert_eq!(labels.len(), before.len() + 3);
    for folio in ["101", "102", "103"] {
        assert!(labels.iter().any(|l| l == folio), "{folio} in {labels:?}");
    }
}

// WP-S2.06 part 1
/// Golden A in the source tree (copied before every run).
pub(crate) const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

/// The (folio, title) pairs of a project's page order.
pub(crate) fn page_titles(dir: &Path) -> Vec<(String, String)> {
    let (s, _) = Session::open_read_only(dir, "test").unwrap();
    s.page_rows().into_iter().map(|r| (r.folio, r.title)).collect()
}

/// WP-S2.06 part 1 through the built binary (acceptance checks NV5, NV7 and NV10; §9 items 1 and 2): a script on a
/// copy of Golden A opens the Devices navigator with folio 102 shown, marks -201U3 by type-ahead and Space (the chips
/// 201-E4 and 203-B4), opens it (folio 201), reorders 106 before 103 through the card, and saves. Two runs give
/// byte-identical transcripts without a UUID; the `--record` log holds one `page.reorder` and replays on a third copy
/// to the same pages.
pub(crate) fn a_navigator_script_runs_twice_identically_and_its_record_replays_case(twice: bool) {
    let tmp = Tmp::new();
    let script = |dir: &Path| {
        steps(&[
            ("file.open", json!({"dir": path(dir)})),
            ("page.goto", json!({"folio": "102"})),
            ("navigator.show", json!({"kind": "devices"})),
            ("navigator.jump", json!({"kind": "devices", "prefix": "-201U3"})),
            ("navigator.mark", json!({"kind": "devices"})),
            ("ui.describe", json!({})),
            ("navigator.open", json!({"kind": "devices"})),
            ("pages.reorder", json!({"pages": ["106"], "before": "103"})),
            ("ui.describe", json!({})),
            ("navigator.apply", json!({})),
            ("ui.describe", json!({})),
            ("file.save", json!({})),
            ("file.close", json!({})),
        ])
    };
    let mut transcripts = Vec::new();
    for n in super::support::runs(twice) {
        let dir = tmp.path(n).join("J-E-2416.ew");
        copy_dir(Path::new(GOLDEN_A), &dir);
        let record = tmp.path(&format!("{n}.ewcmd"));
        let text = all_done(&run(&tmp, n, &script(&dir), Some(&record)));
        assert!(
            !text.contains(&*tmp.0.to_string_lossy()),
            "a path in the transcript"
        );
        super::support::assert_no_uuid(&text);
        transcripts.push(text);
    }
    if twice {
        same_transcript(&transcripts[0], &transcripts[1]);
    }
    let steps: Vec<Value> = transcripts[0]
        .lines()
        .skip(1)
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    // NV5: 102 stays on show, the selection is -201U3's record, the chips name its two representations.
    let first = steps[5]["describe"].as_str().unwrap();
    assert!(first.contains("\npage view: 102\n"), "{first}");
    assert!(first.contains("\n  elsewhere \"201-E4\" \"203-B4\"\n"), "{first}");
    assert!(
        first.contains("device \"-201U3\" expanded selected cursor"),
        "{first}"
    );
    assert_eq!(steps[5]["state"]["focus"], "navigator.devices");
    assert_eq!(steps[6]["state"]["page"], "201", "Enter opens 201");
    // NV7: the card, then Apply.
    assert_eq!(steps[7]["state"]["card"]["origin"], "reorder");
    let card = steps[8]["describe"].as_str().unwrap();
    assert!(card.contains(" reorder \"106 → 103\" \"103 → 106\""), "{card}");
    assert!(steps[9]["state"]["card"].is_null());
    assert_eq!(steps[9]["state"]["can_undo"], true);
    // §9 item 2: the record holds one `page.reorder` with explicit page IDs and replays to the same pages.
    let log = CmdLog::parse(&std::fs::read_to_string(tmp.path("a.ewcmd")).unwrap()).unwrap();
    let names: Vec<&str> = log.commands().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["page.reorder"]);
    let replay = tmp.path("c").join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &replay);
    let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
    store.state_dir = Some(tmp.path("state"));
    let (mut s, _) = Session::open(
        &replay,
        store,
        ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
        ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
    )
    .unwrap();
    s.run_script(&log, "a.ewcmd").unwrap();
    let replayed: Vec<(String, String)> = s.page_rows().into_iter().map(|r| (r.folio, r.title)).collect();
    drop(s);
    assert_eq!(
        replayed,
        page_titles(&tmp.path("a").join("J-E-2416.ew")),
        "the replay equals the scripted project"
    );
    assert_ne!(
        replayed,
        page_titles(Path::new(GOLDEN_A)),
        "103 and 106 swapped their titles"
    );
}

// WP-S2.06 part 2
/// WP-S2.06 part 2 through the built binary (acceptance checks NP3 and NP6; §9 items 1 and 2): a script on a copy of
/// Golden A opens the further navigators, highlights 24VDC-1 from the Potentials navigator (type-ahead, then
/// `navigator.highlight` on the cursor row), shows folios 102 and 103 and clears the highlights. Two runs give
/// byte-identical transcripts without a UUID; the canvas section names the highlight on both folios; the `--record`
/// log is empty (a highlight is view state, never a command).
pub(crate) fn a_highlight_script_runs_twice_identically_and_records_nothing_case(twice: bool) {
    let tmp = Tmp::new();
    let script = |dir: &Path| {
        steps(&[
            ("file.open", json!({"dir": path(dir), "read_only": true})),
            ("page.goto", json!({"folio": "102"})),
            ("navigator.show", json!({"kind": "connections"})),
            ("navigator.show", json!({"kind": "strips"})),
            ("navigator.show", json!({"kind": "plc"})),
            ("navigator.show", json!({"kind": "potentials"})),
            (
                "navigator.jump",
                json!({"kind": "potentials", "prefix": "24VDC-1"}),
            ),
            ("navigator.highlight", json!({"kind": "potentials"})),
            ("ui.describe", json!({})),
            ("page.goto", json!({"folio": "103"})),
            ("ui.describe", json!({})),
            ("view.clear_highlights", json!({})),
            ("ui.describe", json!({})),
            ("file.close", json!({})),
        ])
    };
    let mut transcripts = Vec::new();
    for n in super::support::runs(twice) {
        let dir = tmp.path(n).join("J-E-2416.ew");
        copy_dir(Path::new(GOLDEN_A), &dir);
        let record = tmp.path(&format!("{n}.ewcmd"));
        let text = all_done(&run(&tmp, n, &script(&dir), Some(&record)));
        assert!(
            !text.contains(&*tmp.0.to_string_lossy()),
            "a path in the transcript"
        );
        super::support::assert_no_uuid(&text);
        transcripts.push(text);
    }
    if twice {
        same_transcript(&transcripts[0], &transcripts[1]);
    }
    let steps: Vec<Value> = transcripts[0]
        .lines()
        .skip(1)
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    for (at, folio) in [(8, "102"), (10, "103")] {
        let d = steps[at]["describe"].as_str().unwrap();
        assert!(
            d.contains(&format!(
                "
page view: {folio}
"
            )),
            "{d}"
        );
        assert!(
            d.contains(
                "
  highlight slot 0 potential \"24VDC-1\" connections "
            ),
            "{folio}: {d}"
        );
        assert!(
            d.contains("row level 1 potential \"24VDC-1\" expanded cursor highlight 0"),
            "{d}"
        );
        assert!(d.contains("navigator \"PLC\" kind plc view tree"), "{d}");
        assert!(d.contains("row level 1 station \"-201U1\""), "{d}");
    }
    let cleared = steps[12]["describe"].as_str().unwrap();
    assert!(!cleared.contains("  highlight slot"), "{cleared}");
    let log = std::fs::read_to_string(tmp.path("a.ewcmd")).unwrap_or_default();
    let log = CmdLog::parse(&log).unwrap();
    assert_eq!(log.commands().count(), 0, "a highlight records nothing");
}

// WP-S2.06 part 3
/// The Terminal strips table's cell texts of a project, row by row (the table's nodes in tree order).
pub(crate) fn strip_cells(dir: &Path) -> Vec<Vec<String>> {
    let (s, _) = Session::open_read_only(dir, "test").unwrap();
    let t = s.navigator_table(ew_ui::ew_viewmodel::ew_api::output::NavKind::Strips);
    t.nodes.into_iter().map(|n| n.cells).collect()
}

/// WP-S2.06 part 3 through the built binary (acceptance checks TB2 and TB8; §9 items 1 and 2): a script on a copy of
/// Golden A shows the Terminal strips navigator as a table filtered `x1031` and sorted by Terminal, sets terminal 13's
/// function text to "Spare" by a cell edit, marks terminals 13 to 17 and fills down, copies the marked rows and saves.
/// Two runs give byte-identical transcripts without a UUID; the `--record` log holds the two `fields.set` steps and
/// replays on a third copy to the same Terminal strips table.
pub(crate) fn a_table_script_runs_twice_identically_and_its_record_replays_case(twice: bool) {
    let tmp = Tmp::new();
    let mut list = vec![
        ("page.goto", json!({"folio": "102"})),
        ("navigator.show", json!({"kind": "strips"})),
        ("navigator.table", json!({"kind": "strips", "on": true})),
        ("navigator.filter", json!({"kind": "strips", "text": "x1031"})),
        ("navigator.sort", json!({"kind": "strips", "by": "terminal"})),
        ("ui.focus", json!({"area": "navigator", "kind": "strips"})),
        ("navigator.jump", json!({"kind": "strips", "prefix": "13"})),
        ("table.cell", json!({"column": "label"})),
        ("table.edit", json!({})),
        ("table.input", json!({"text": "Spare"})),
        ("ui.describe", json!({})),
        ("table.commit", json!({})),
        ("navigator.mark", json!({"kind": "strips"})),
    ];
    for _ in 0..4 {
        list.push(("table.cell", json!({"step": 1})));
        list.push(("navigator.mark", json!({"kind": "strips"})));
    }
    list.extend([
        ("table.fill_down", json!({})),
        ("table.copy", json!({})),
        ("ui.describe", json!({})),
        ("file.save", json!({})),
        ("file.close", json!({})),
    ]);
    let script = |dir: &Path| {
        let mut all = steps(&[("file.open", json!({"dir": path(dir)}))]);
        all.extend(steps(&list));
        all
    };
    let mut transcripts = Vec::new();
    for n in super::support::runs(twice) {
        let dir = tmp.path(n).join("J-E-2416.ew");
        copy_dir(Path::new(GOLDEN_A), &dir);
        let record = tmp.path(&format!("{n}.ewcmd"));
        let text = all_done(&run(&tmp, n, &script(&dir), Some(&record)));
        assert!(
            !text.contains(&*tmp.0.to_string_lossy()),
            "a path in the transcript"
        );
        super::support::assert_no_uuid(&text);
        transcripts.push(text);
    }
    if twice {
        same_transcript(&transcripts[0], &transcripts[1]);
    }
    let steps: Vec<Value> = transcripts[0]
        .lines()
        .skip(1)
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let editing = steps[11]["describe"].as_str().unwrap();
    assert!(
        editing.contains("\n  editing \"Function text\" buffer \"Spare\"\n"),
        "{editing}"
    );
    let filled = steps.iter().rev().find_map(|s| s["describe"].as_str()).unwrap();
    assert!(
        filled.contains(
            "navigator \"Terminal strips\" kind strips view tree table sort \"Terminal\" ascending"
        ),
        "{filled}"
    );
    assert!(
        filled.contains("toast \"Set Function text on 5 terminals\" undo"),
        "{filled}"
    );
    for t in ["13", "14", "15", "16", "17"] {
        // WP-S2.07 D2: the Pos cell (the terminal's seq) after the strip.
        assert!(
            filled.contains(&format!("\n  row \"-X1031\" \"{t}\" \"{t}\" \"Spare\" ")),
            "terminal {t}: {filled}"
        );
    }
    // §9 item 2: the record holds the cell edit and the fill, and replays to the same table.
    let log = CmdLog::parse(&std::fs::read_to_string(tmp.path("a.ewcmd")).unwrap()).unwrap();
    let names: Vec<&str> = log.commands().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["fields.set", "fields.set"]);
    let replay = tmp.path("c").join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &replay);
    let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
    store.state_dir = Some(tmp.path("state"));
    let (mut s, _) = Session::open(
        &replay,
        store,
        ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
        ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
    )
    .unwrap();
    s.run_script(&log, "a.ewcmd").unwrap();
    s.save().unwrap();
    drop(s);
    let scripted = strip_cells(&tmp.path("a").join("J-E-2416.ew"));
    assert_eq!(
        strip_cells(&replay),
        scripted,
        "the replay equals the scripted project"
    );
    assert_ne!(
        scripted,
        strip_cells(Path::new(GOLDEN_A)),
        "terminals 13 to 17 changed"
    );
}
