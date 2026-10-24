// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-408 and FT-409 through the built binary (the pilot's prerequisites `page.new`, `structure.new` and
//! `external_ref.new`, decision log ID-619 and ID-623): no window, no GPU, no display.
//!
//! - **Pages.** On a new project (`file.new`) a script runs `page.new` for folios 101 to 103 and the describe text
//!   lists the three; two runs give byte-identical transcripts and the `--record` log replays on a copy of the start
//!   (the session's command-log replay, which `ew apply` runs).
//! - **Folio 101's external points.** The location `+J-E-206` and the external reference `JE-N1000-03` are made by the
//!   two actions on the new project; folio 101's generated placement script then places its interruption points with
//!   the external reference named by the three external arrows, and the arrows print "+J-E-206 / JE-N1000-03"
//!   (doc 20 §3.4 item 9, FR-XRF-012).

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_ui::ew_viewmodel::ew_api::output::RowValue;
use ew_ui::ew_viewmodel::ew_api::{CmdLog, Session};
use serde_json::{Value, json};
use support::{SheetMap, golden_reps, placement_steps};

const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-create-test-{}-{n}", std::process::id()));
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

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

type Steps = Vec<(String, Value)>;

fn steps(list: &[(&str, Value)]) -> Steps {
    list.iter().map(|(a, p)| ((*a).to_owned(), p.clone())).collect()
}

fn script_text(steps: &Steps) -> String {
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
fn bin_exe() -> PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_elektrawatt")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_elektrawatt"))) // bin-exe-fallback
}

/// Runs a script headless (with the content of the source tree, a configuration folder of its own), optionally recording
/// the committed commands.
fn run(tmp: &Tmp, name: &str, steps: &Steps, record: Option<&Path>) -> Output {
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
fn all_done(out: &Output) -> String {
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
fn describe_of(transcript: &str, action: &str) -> String {
    transcript
        .lines()
        .skip(1)
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|s| s["action"] == action)
        .and_then(|s| s["describe"].as_str().map(str::to_owned))
        .unwrap_or_else(|| panic!("no {action} step"))
}

/// A new project at `name` of `tmp` (the builtin template), saved and closed.
fn new_project(tmp: &Tmp, name: &str, more: &[(&str, Value)]) -> PathBuf {
    let dir = tmp.path(name);
    let mut script = steps(&[("file.new", json!({"dir": path(&dir), "template": "builtin"}))]);
    script.extend(steps(more));
    script.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    all_done(&run(tmp, name, &script, None));
    dir
}

/// Panics at the first line two transcripts differ in.
#[track_caller]
fn same_transcript(a: &str, b: &str) {
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
#[test]
fn page_new_creates_folios_101_to_103_twice_identically_and_the_record_replays() {
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
    for n in ["a", "b"] {
        let dir = tmp.path(n).join("p.ew");
        copy_dir(&start, &dir);
        let record = tmp.path(&format!("{n}.ewcmd"));
        let text = all_done(&run(&tmp, n, &script(&dir), Some(&record)));
        assert!(
            !text.contains(&*tmp.0.to_string_lossy()),
            "a path in the transcript"
        );
        support::assert_no_uuid(&text);
        transcripts.push(text);
        records.push(record);
    }
    same_transcript(&transcripts[0], &transcripts[1]);
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

/// The folio-101 start of §13.6 on a project whose structure the two actions made: the page, the strip and the IP
/// symbols as in `canvas.rs`'s start, the location `+J-E-206` and its external reference `JE-N1000-03`.
fn folio_start(tmp: &Tmp) -> PathBuf {
    let embed = |files: &[&str]| -> Value {
        let copies: Vec<Value> = files
            .iter()
            .map(|f| {
                serde_json::from_str(&std::fs::read_to_string(Path::new(CONTENT).join(f)).unwrap()).unwrap()
            })
            .collect();
        json!({ "copies": copies })
    };
    new_project(
        tmp,
        "folio.ew",
        &[
            (
                "page.new",
                json!({"folio": "101", "type": "multi_line", "title": "Power Supply 1 - Main Current"}),
            ),
            (
                "structure.new",
                json!({"kind": "location", "designation": "J-E-206"}),
            ),
            (
                "external_ref.new",
                json!({"location": "J-E-206", "device": "JE-N1000-03"}),
            ),
            (
                "command.run",
                json!({"cmd": "device.create", "params": {"letter": "X", "counter": 1, "strip": {}}}),
            ),
            (
                "command.run",
                json!({"cmd": "library.embed", "params": embed(&[
                    "symbols/qet10/ew.gen.ip@qet10.json",
                    "fndefs/ew.fn.ip.json",
                    "symbols/qet10/ew.gen.external-arrow@qet10.json",
                    "fndefs/ew.fn.external-arrow.json",
                ])}),
            ),
        ],
    )
}

/// FT-409's acceptance (b): folio 101's external points print the texts of the node and the reference the two actions
/// made.
#[test]
fn folio_101s_external_points_print_the_structure_node_and_the_external_reference() {
    let tmp = Tmp::new();
    let job = folio_start(&tmp);
    let (session, _) = Session::open_read_only(&job, "test").unwrap();
    let grid = session.page_grid("101").expect("page 101");
    let map = SheetMap {
        scale_ppb: i128::from(grid.sheet_scale_ppb),
        offset: (
            i128::from(grid.sheet_offset_nm.0),
            i128::from(grid.sheet_offset_nm.1),
        ),
    };
    let q = session.query();
    let strip = q
        .devices()
        .iter()
        .find(|(_, d)| d.aspects.strip.is_some())
        .map(|(id, _)| id.to_string())
        .expect("the start project's strip");
    // What the two actions made: one node `J-E-206` of the location aspect, one reference at it.
    let nodes: Vec<(String, String)> = q
        .structure_nodes()
        .iter()
        .map(|(_, n)| {
            (
                serde_json::to_value(&n.aspect)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned(),
                n.code.as_str().to_owned(),
            )
        })
        .collect();
    assert_eq!(nodes, [("location".to_owned(), "J-E-206".to_owned())]);
    let refs: Vec<_> = q
        .external_refs()
        .iter()
        .map(|(id, e)| (id.to_string(), e.device.as_str().to_owned()))
        .collect();
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].1, "JE-N1000-03");
    let external = refs[0].0.clone();
    drop(q);
    drop(session);
    // The placement script of §13.6 with the three external arrows naming the reference.
    let (mut placements, runs) = placement_steps(101, map, &strip);
    let mut external_points = 0;
    for (action, params) in &mut placements {
        if action == "command.run"
            && params["cmd"] == "ip.place"
            && params["params"]["name"]
                .as_str()
                .is_some_and(|n| n.starts_with("J-E-206-"))
        {
            params["params"]["external"] = json!({"ref": format!("id:{external}")});
            external_points += 1;
        }
    }
    assert_eq!(external_points, 3, "the three external arrows of folio 101");
    assert_eq!(runs, 6);
    let mut transcripts = Vec::new();
    let mut records = Vec::new();
    for n in ["a", "b"] {
        let dir = tmp.path(n).join("folio.ew");
        copy_dir(&job, &dir);
        let mut script = steps(&[
            ("file.open", json!({"dir": path(&dir)})),
            ("page.goto", json!({"folio": "101"})),
        ]);
        script.extend(placements.iter().cloned());
        script.extend(steps(&[
            ("ui.describe", json!({})),
            ("file.save", json!({})),
            ("file.close", json!({})),
        ]));
        let record = tmp.path(&format!("{n}.ewcmd"));
        let text = all_done(&run(&tmp, n, &script, Some(&record)));
        support::assert_no_uuid(&text);
        transcripts.push(text);
        records.push(record);
    }
    same_transcript(&transcripts[0], &transcripts[1]);
    // The oracle of §13.6 still holds: 17 representations, each the golden symbol at the golden point.
    let mut golden: Vec<(String, (i64, i64), u8)> = golden_reps(101)
        .into_iter()
        .map(|r| (r.symbol, r.pos, r.orient))
        .collect();
    golden.sort();
    let (placed_session, _) = Session::open_read_only(&tmp.path("a").join("folio.ew"), "test").unwrap();
    let tables = placed_session.page_tables("101").unwrap();
    let mut placed: Vec<(String, (i64, i64), u8)> = tables
        .reps
        .iter()
        .map(|r| (r.symbol_key.clone(), r.pos, r.orient))
        .collect();
    placed.sort();
    assert_eq!(placed, golden);
    // The three external arrows print "+J-E-206 / JE-N1000-03" in their cross-reference row.
    let arrows: Vec<_> = tables
        .reps
        .iter()
        .filter(|r| r.ip.as_ref().is_some_and(|ip| ip.name.starts_with("J-E-206-")))
        .collect();
    assert_eq!(arrows.len(), 3);
    for rep in arrows {
        let sheet = placed_session.property_sheet(&[rep.id]);
        let row = sheet
            .sections
            .iter()
            .flat_map(|s| &s.rows)
            .find(|r| r.key == "representation/derived.xref")
            .expect("the representation's derived cross-reference row");
        let RowValue::One { text, .. } = &row.value else {
            panic!("{:?}", row.value);
        };
        assert_eq!(text, "+J-E-206 / JE-N1000-03", "{:?}", rep.ip);
    }
    // The record replays on a copy of the start, where the node and the reference already are.
    let log = CmdLog::parse(&std::fs::read_to_string(&records[0]).unwrap()).unwrap();
    let replay = tmp.path("c").join("folio.ew");
    copy_dir(&job, &replay);
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
    assert_eq!(s.page_tables("101").unwrap().reps.len(), 17);
}
