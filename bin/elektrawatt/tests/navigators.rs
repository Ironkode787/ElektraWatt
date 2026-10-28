// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.06 part 1 through the built binary (acceptance checks NV5, NV7 and NV10, §9 items 1 and 2): no window, no GPU,
//! no display. A script on a copy of Golden A opens the Devices navigator with folio 102 shown, marks -201U3 by
//! type-ahead and Space (the chips 201-E4 and 203-B4), opens it (folio 201), reorders 106 before 103 through the card,
//! and saves. Two runs give byte-identical transcripts without a UUID; the `--record` log holds one `page.reorder`
//! with explicit IDs and replays on a third copy to the same pages.

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_ui::ew_viewmodel::ew_api::{CmdLog, Session};
use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-navigators-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
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

/// The path of the `elektrawatt` binary under test (nextest sets it at run time; see `tests/headless.rs`).
// Allowed: the program boundary of a test — nextest's binary path is read once, never persisted or printed.
#[allow(clippy::disallowed_methods)]
fn bin_exe() -> PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_elektrawatt")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_elektrawatt"))) // bin-exe-fallback
}

fn script(dir: &Path) -> String {
    let steps: Vec<(&str, Value)> = vec![
        ("file.open", json!({"dir": dir.to_string_lossy()})),
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
    ];
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"navigators\"}\n");
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

/// Runs the script on a fresh copy of Golden A in `tmp/<name>`, recording the committed commands.
fn run(tmp: &Tmp, name: &str) -> (Output, PathBuf, PathBuf) {
    let dir = tmp.path(name).join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
    let file = tmp.path(&format!("{name}.jsonl"));
    std::fs::write(&file, script(&dir)).unwrap();
    let record = tmp.path(&format!("{name}.ewcmd"));
    let out = Command::new(bin_exe())
        .args(["--headless", "--script"])
        .arg(&file)
        .arg("--record")
        .arg(&record)
        .env("EW_CONFIG_DIR", tmp.path(&format!("config-{name}")))
        .env("EW_CONTENT_DIR", CONTENT)
        .env_remove("EW_SAMPLE_DIR")
        .env_remove("EW_DOCS_DIR")
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (out, dir, record)
}

/// The transcript's steps, every one done.
#[track_caller]
fn steps_of(out: &Output) -> Vec<Value> {
    let text = String::from_utf8(out.stdout.clone()).unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}\n{text}",
        String::from_utf8_lossy(&out.stderr)
    );
    let steps: Vec<Value> = text
        .lines()
        .skip(1)
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    for s in &steps {
        assert_eq!(s["outcome"], "done", "{s}");
    }
    steps
}

/// The (folio, title) pairs of a project's page order.
fn pages(dir: &Path) -> Vec<(String, String)> {
    let (s, _) = Session::open_read_only(dir, "test").unwrap();
    s.page_rows().into_iter().map(|r| (r.folio, r.title)).collect()
}

#[test]
fn a_navigator_script_runs_twice_identically_and_its_record_replays() {
    let tmp = Tmp::new();
    let (a, dir_a, record_a) = run(&tmp, "a");
    let (b, _, _) = run(&tmp, "b");
    assert_eq!(a.stdout, b.stdout, "two runs, one transcript");
    let text = String::from_utf8(a.stdout.clone()).unwrap();
    assert!(
        !text.contains(&*tmp.0.to_string_lossy()),
        "a path in the transcript"
    );
    support::assert_no_uuid(&text);
    let steps = steps_of(&a);
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
    let log = CmdLog::parse(&std::fs::read_to_string(&record_a).unwrap()).unwrap();
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
    assert_eq!(replayed, pages(&dir_a), "the replay equals the scripted project");
    let golden = pages(Path::new(GOLDEN_A));
    assert_ne!(replayed, golden, "103 and 106 swapped their titles");
}
