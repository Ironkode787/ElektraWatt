// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The clipboard headless (WP-S2.05 part 3, acceptance checks CB5, CB6 and the script half of CB8): T55, E55 and E56
//! of Golden A's folio 101 copied into folio 1 of a new project by an `ew.uiscript/1` script through the built binary
//! (three devices with new IDs, no frozen tag, tags of that project's scheme, the part copies embedded once),
//! byte-identical transcripts on two runs without a raw UUID, and the `--record` log (one `edit.paste` carrying the
//! fragment) replayed onto a copy of the same new project giving the same records, IDs aside; two shell models as two instances
//! (-102G1 and -102F1 with the wires between them pasted into a new project in the other); the tag list. A module of
//! `tests/headless.rs`, whose `bin_exe` it uses.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_ui::ew_viewmodel::ew_api::{CmdLog, Session};
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-clip-test-{}-{n}", std::process::id()));
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
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"clipboard\"}\n");
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
    Command::new(super::bin_exe())
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

/// The point the scripts paste at, sheet mm: inside the new project's first page.
const AT: (f64, f64) = (120.0, 90.0);

fn steps(golden: &Path, new: &Path) -> Vec<(&'static str, Value)> {
    vec![
        ("file.open", json!({ "dir": golden.to_string_lossy() })),
        ("page.goto", json!({"folio": "101"})),
        ("select.set", json!({"refs": ["tag:T55", "tag:E55", "tag:E56"]})),
        ("edit.copy", json!({})),
        ("ui.describe", json!({})),
        ("file.open", json!({ "dir": new.to_string_lossy() })),
        ("page.goto", json!({"folio": "1"})),
        ("edit.paste_clip", json!({})),
        ("canvas.pointer", json!({"x_mm": AT.0, "y_mm": AT.1})),
        ("canvas.dwell", json!({"ms": 100})),
        ("ui.describe", json!({})),
        ("canvas.press", json!({"x_mm": AT.0, "y_mm": AT.1})),
        ("ui.describe", json!({})),
        ("file.save", json!({})),
    ]
}

/// The id-free content of a project: its devices' tags and frozen state, its representations' places, its part copies.
fn content(s: &Session) -> (Vec<String>, Vec<(i64, i64)>, usize) {
    let q = s.query();
    let mut tags: Vec<String> = q
        .devices()
        .iter()
        .map(|(id, d)| {
            let frozen = if d.tag.frozen.is_some() { " frozen" } else { "" };
            format!("{}{frozen}", s.tag_list(&[id.as_uuid()]).trim_end())
        })
        .collect();
    tags.sort();
    let tables = s.page_tables("1").unwrap();
    let mut places: Vec<(i64, i64)> = tables.reps.iter().map(|r| r.pos).collect();
    places.sort_unstable();
    (tags, places, q.parts().len())
}

/// CB5 (FR-UI-036) and CB8: T55, E55 and E56 copied into a new project's folio 1 by a script, twice byte-identical,
/// no raw UUID, and the `--record` log replayed onto a fresh new project.
/// Per gate one run of the script (`twice` false); the deep nightly's
/// `nightly_every_script_transcript_is_byte_identical_twice` runs the body with `twice` true: a second run on an
/// equal start and the byte-identical comparison of the two transcripts (test clean-up, 2026-10-30).
#[test]
fn cb5_cb8_copy_into_a_new_project_headless_twice_and_replayed() {
    cb5_cb8_copy_into_a_new_project_headless_twice_and_replayed_case(false);
}

pub(crate) fn cb5_cb8_copy_into_a_new_project_headless_twice_and_replayed_case(twice: bool) {
    let tmp = Tmp::new();
    // The new project, made once by `file.new`: the start of both runs and of the replay.
    let start = tmp.path("start.ew");
    let new = script(&[("file.new", json!({ "dir": start.to_string_lossy() }))]);
    all_done(&run(&tmp, "new.jsonl", &new, &tmp.path("new.ewcmd")));
    let (new_a, new_b, fresh_dir) = (tmp.path("new_a.ew"), tmp.path("new_b.ew"), tmp.path("fresh.ew"));
    for d in [&new_a, &new_b, &fresh_dir] {
        copy_dir(&start, d);
    }
    let a = tmp.golden("a.ew");
    let record = tmp.path("clip.ewcmd");
    let out = run(&tmp, "clip.jsonl", &script(&steps(&a, &new_a)), &record);
    let t = all_done(&out);
    let d = |i: usize| t[i]["describe"].as_str().unwrap().to_owned();
    assert_eq!(t[3]["state"]["clipboard"]["kind"], "fragment");
    assert!(d(4).contains("clipboard fragment records "), "{}", d(4));
    assert_eq!(t[7]["state"]["canvas"]["tool"]["name"], "paste");
    assert!(d(10).contains("canvas ghost paste records "), "{}", d(10));
    assert_eq!(t[9]["state"]["canvas"]["preview"]["state"], "ready");
    assert_eq!(t[11]["state"]["canvas"]["tool"]["name"], "select");
    assert_eq!(t[11]["state"]["canvas"]["selection"].as_array().unwrap().len(), 3);
    let transcript = String::from_utf8_lossy(&out.stdout).into_owned();
    super::support::assert_no_uuid(&transcript);
    if twice {
        // Twice: the same bytes (the directories aside).
        let b = tmp.golden("b.ew");
        let out_b = run(
            &tmp,
            "clip_b.jsonl",
            &script(&steps(&b, &new_b)),
            &tmp.path("clip_b.ewcmd"),
        );
        all_done(&out_b);
        assert_eq!(
            transcript.replace("new_a.ew", "x").replace("a.ew", "y"),
            String::from_utf8_lossy(&out_b.stdout)
                .replace("new_b.ew", "x")
                .replace("b.ew", "y")
        );
    }
    // CB5 on the saved project: three new devices, no frozen tag, tags of the new project's scheme, the copies once.
    let (saved, _) = Session::open_read_only(&new_a, "test").unwrap();
    let (tags, places, parts) = content(&saved);
    assert_eq!(tags.len(), 3, "{tags:?}");
    assert!(tags.iter().all(|t| !t.ends_with("frozen")), "{tags:?}");
    assert!(
        tags.iter().all(|t| t.starts_with("-1")),
        "page-based tags of folio 1: {tags:?}"
    );
    assert_eq!(places.len(), 3);
    assert!(parts >= 1, "the golden part copies are embedded");
    // The record: one `edit.paste` carrying the fragment, replayed onto a fresh new project.
    let log = CmdLog::parse(&std::fs::read_to_string(&record).unwrap()).unwrap();
    let names: Vec<&str> = log.commands().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["edit.paste"]);
    let (mut fresh, _) = Session::open_read_only(&fresh_dir, "test").unwrap();
    fresh.run_script(&log, "clip.ewcmd").unwrap();
    assert_eq!(
        content(&fresh),
        content(&saved),
        "the replay's records equal the script's, IDs aside"
    );
}

fn shell(tmp: &Tmp, name: &str) -> ShellModel {
    ShellModel::new(Mode::Headless, ShellConfig::in_dir(&tmp.path(name), "0.0.0-test"))
}

#[track_caller]
fn done(shell: &mut ShellModel, action: &str, params: Value) {
    assert_eq!(
        shell.dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// CB6 (FR-UI-040): two shell models as two instances: -102G1 and -102F1 copied in one, pasted into a new project in
/// the other as the system clipboard's text, with the junction and the three wires between them; "Copy tag list" of
/// -101F1, S51 and F55 is `-101F1\nF55\nS51\n`.
#[test]
fn cb6_two_instances_and_the_tag_list() {
    let tmp = Tmp::new();
    let golden = tmp.golden("g.ew");
    let mut one = shell(&tmp, "one");
    done(&mut one, "file.open", json!({ "dir": golden.to_string_lossy() }));
    done(&mut one, "page.goto", json!({"folio": "102"}));
    done(
        &mut one,
        "select.set",
        json!({"refs": ["tag:-102G1", "tag:-102F1"]}),
    );
    done(&mut one, "edit.copy", json!({}));
    let text = one.clipboard().text().unwrap().to_owned();
    assert!(text.starts_with("#ew-fragment v1\n"));
    // The other instance gets the text as a paste event's.
    let mut two = shell(&tmp, "two");
    done(
        &mut two,
        "file.new",
        json!({ "dir": tmp.path("new.ew").to_string_lossy() }),
    );
    done(&mut two, "page.goto", json!({"folio": "1"}));
    done(&mut two, "edit.paste_clip", json!({ "text": text }));
    done(&mut two, "canvas.pointer", json!({"x_mm": 60.0, "y_mm": 40.0}));
    done(&mut two, "canvas.press", json!({"x_mm": 60.0, "y_mm": 40.0}));
    let tables = two.canvas().unwrap().page().tables.clone();
    let devices: std::collections::BTreeSet<_> = tables.reps.iter().filter_map(|r| r.device).collect();
    assert_eq!(devices.len(), 2, "both devices");
    assert_eq!(tables.connections.len(), 3, "the wires between them");
    assert_eq!(tables.junctions.len(), 1, "the junction where they meet");
    // Copy tag list in the first instance.
    done(&mut one, "page.goto", json!({"folio": "101"}));
    done(
        &mut one,
        "select.set",
        json!({"refs": ["tag:S51", "tag:-101F1", "tag:F55"]}),
    );
    done(&mut one, "edit.copy_tags", json!({}));
    assert_eq!(one.clipboard().text(), Some("-101F1\nF55\nS51\n"));
    one.shutdown();
    two.shutdown();
}
