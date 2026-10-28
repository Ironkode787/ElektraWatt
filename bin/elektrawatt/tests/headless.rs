// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end tests of the headless mode of the `elektrawatt` binary (WP-S2.01 part 1, acceptance checks A6, A7
//! and A8): no window, no GPU, no display. Every test works on a temporary copy of Golden A with `EW_CONFIG_DIR`
//! in a temporary directory; nothing is written into the repository.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The property grid's headless checks (WP-S2.05 part 1, PG9 and PG10).
#[path = "grid/properties.rs"]
mod grid_properties;

/// Find and replace headless (WP-S2.05 part 2, FR3 to FR6).
#[path = "grid/find.rs"]
mod grid_find;

/// The clipboard headless (WP-S2.05 part 3, CB5, CB6 and CB8).
#[path = "grid/clipboard.rs"]
mod grid_clipboard;

/// The export job and the journal recovery headless (WP-S2.09 part 1, JB4 and JB7).
#[path = "jobs/scripts.rs"]
mod jobs_scripts;

/// The message panel's flows headless (WP-S2.09 part 2, MS1 to MS4; FT-516).
#[path = "messages/scripts.rs"]
mod messages_scripts;

/// Golden B through the renumber actions, the Preview tab and the history panel headless (WP-S2.09 part 3, PV2).
#[path = "preview/scripts.rs"]
mod preview_scripts;

/// The folio-script support; here for its UUID check (FT-422).
mod support;

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/expected");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-headless-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// A copy of Golden A named `J-E-2416.ew` in a subdirectory of its own.
    fn golden(&self, sub: &str) -> PathBuf {
        let target = self.0.join(sub).join("J-E-2416.ew");
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

fn elektrawatt(tmp: &Tmp, args: &[&std::ffi::OsStr]) -> Output {
    Command::new(bin_exe())
        .args(args)
        .env("EW_CONFIG_DIR", tmp.path("config"))
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn script(steps: &[(&str, Value)]) -> String {
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"test\"}\n");
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

fn run_script_file(tmp: &Tmp, name: &str, text: &str, extra: &[&std::ffi::OsStr]) -> Output {
    let file = tmp.path(name);
    std::fs::write(&file, text).unwrap();
    let mut args: Vec<&std::ffi::OsStr> = vec!["--headless".as_ref(), "--script".as_ref(), file.as_os_str()];
    args.extend_from_slice(extra);
    elektrawatt(tmp, &args)
}

/// The transcript lines of an output, parsed.
fn transcript(out: &Output) -> Vec<Value> {
    String::from_utf8(out.stdout.clone())
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{l}: {e}")))
        .collect()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn decode_rgba(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("valid png");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("png frame");
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        other => panic!("unexpected PNG colour type {other:?}"),
    };
    (info.width, info.height, rgba)
}

/// The share of pixels (in per cent) where any channel differs by more than `threshold`.
fn differing_percent(a: &[u8], b: &[u8], threshold: u8) -> f64 {
    assert_eq!(a.len(), b.len());
    let bad = a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > threshold))
        .count();
    100.0 * bad as f64 / (a.len() / 4) as f64
}

/// The relative names of all files under `dir`, with their sizes, sorted.
fn listing(dir: &Path) -> Vec<(String, u64)> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, u64)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                walk(base, &path, out);
            } else {
                let name = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((name, entry.metadata().unwrap().len()));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn smoke_script(dir: &Path, png: &Path, pdf: &Path) -> String {
    let dir = dir.to_string_lossy().into_owned();
    script(&[
        ("file.open", json!({ "dir": dir })),
        ("page.goto", json!({"folio": "103"})),
        (
            "view.snapshot",
            json!({"path": png.to_string_lossy(), "dpi": 150}),
        ),
        // Before any edit: the bytes of `ew export pdf`.
        ("file.export_pdf", json!({"path": pdf.to_string_lossy()})),
        (
            "command.run",
            json!({"cmd": "page.create", "params": {"number": 999, "title": "Smoke"}}),
        ),
        ("file.save", json!({})),
        ("file.close", json!({})),
        ("file.open", json!({ "dir": dir })),
        ("ui.describe", json!({})),
        ("file.close", json!({})),
    ])
}

/// A6: the smoke script of the spec through the built binary.
#[test]
fn the_smoke_script_runs_on_golden_a() {
    let tmp = Tmp::new();
    let project = tmp.golden("run");
    let png_path = tmp.path("103.png");
    let pdf_path = tmp.path("export.pdf");
    let record = tmp.path("recorded.ewcmd");
    let out = run_script_file(
        &tmp,
        "smoke.jsonl",
        &smoke_script(&project, &png_path, &pdf_path),
        &["--record".as_ref(), record.as_os_str()],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}\n{}",
        stderr(&out),
        String::from_utf8_lossy(&out.stdout)
    );
    let lines = transcript(&out);
    assert_eq!(lines[0]["$schema"], "ew.uitranscript/1");
    assert_eq!(lines[0]["version"], env!("CARGO_PKG_VERSION"));
    let steps = &lines[1..];
    assert_eq!(steps.len(), 10);
    for (i, step) in steps.iter().enumerate() {
        assert_eq!(step["step"], i + 1);
        assert_eq!(step["outcome"], "done", "{step}");
        assert_eq!(step["reason"], Value::Null);
        assert_eq!(step["message"], Value::Null);
    }
    // The states: open, goto, snapshot, export, edit, save, close, reopen.
    let state = |i: usize| &steps[i]["state"];
    assert_eq!(state(0)["project"], "J-E-2416.ew");
    assert_eq!(state(0)["title"], "J-E-2416.ew - ElektraWatt");
    assert_eq!(state(0)["access"], "edit");
    assert_eq!(state(0)["dirty"], false);
    let count = state(0)["page_count"].as_u64().unwrap();
    assert!(count > 30, "{count}");
    assert_eq!(state(1)["page"], "103");
    assert_eq!(state(4)["dirty"], true);
    assert_eq!(state(4)["can_undo"], true);
    assert_eq!(state(4)["page_count"].as_u64().unwrap(), count + 1);
    assert!(state(4)["title"].as_str().unwrap().starts_with('*'));
    assert_eq!(state(5)["dirty"], false);
    assert_eq!(state(6)["project"], Value::Null);
    assert_eq!(state(6)["title"], "ElektraWatt");
    // The canvas member (WP-S2.02): the page on show, null without a project.
    assert_eq!(state(1)["canvas"]["folio"], "103");
    assert_eq!(state(1)["canvas"]["backend"], "CPU");
    assert_eq!(state(6)["canvas"], Value::Null);
    assert_eq!(state(7)["page_count"].as_u64().unwrap(), count + 1);
    let describe = steps[8]["describe"].as_str().unwrap();
    assert!(
        describe.contains("page \"999\""),
        "the new page is in the page list"
    );
    assert!(describe.contains("menu File"));
    // WP-S2.17 part 3 (D12): the state at the left; the tool's hint is the HUD's, the page field left the bar.
    assert!(describe.contains("status bar: Ready | "), "{describe}");
    // Transcripts carry no path.
    let all = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        !all.contains(&*tmp.0.to_string_lossy()),
        "an absolute path in the transcript"
    );

    // The 150 dpi snapshot of folio 103 matches the committed golden PNG within the doc 20 tolerance.
    let committed = std::fs::read(Path::new(EXPECTED).join("png/103.png")).unwrap();
    let produced = std::fs::read(&png_path).unwrap();
    let (w0, h0, a) = decode_rgba(&committed);
    let (w1, h1, b) = decode_rgba(&produced);
    assert_eq!((w0, h0), (w1, h1), "size of the snapshot");
    let pct = differing_percent(&a, &b, 16);
    assert!(pct <= 0.5, "{pct:.3} % of the pixels differ");

    // The PDF exported before any edit is the golden export: the bytes of `ew export pdf`.
    let pdf = std::fs::read(&pdf_path).unwrap();
    let digest: String = Sha256::digest(&pdf).iter().map(|b| format!("{b:02x}")).collect();
    let expected = std::fs::read_to_string(Path::new(EXPECTED).join("pdf/sha256.txt")).unwrap();
    assert_eq!(digest, expected.trim());

    // The saved page survives the reopen: the project on disk has it.
    let (session, _) = ew_ui::ew_viewmodel::ew_api::Session::open_read_only(&project, "test").unwrap();
    let labels = session.page_labels();
    assert!(labels.iter().any(|l| l == "999"));

    // --record: a command log that CmdLog reads back to the committed command, and that replays to the same pages.
    let log_text = std::fs::read_to_string(&record).unwrap();
    assert!(log_text.starts_with("{\"$schema\":\"ew.cmdlog/1\",\"label\":\"recorded by elektrawatt "));
    let log = ew_ui::ew_viewmodel::ew_api::CmdLog::parse(&log_text).unwrap();
    let commands: Vec<_> = log.commands().collect();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].name, "page.create");
    assert_eq!(commands[0].params, json!({"number": 999, "title": "Smoke"}));
    let fresh = tmp.golden("fresh");
    let (mut again, _) = ew_ui::ew_viewmodel::ew_api::Session::open_read_only(&fresh, "test").unwrap();
    again.run_script(&log, "recorded.ewcmd").unwrap();
    assert_eq!(again.page_labels(), labels);
}

/// A7: the script run twice on equal inputs gives byte-identical transcripts, from a file and from stdin.
#[test]
fn the_transcript_is_deterministic() {
    let steps = |dir: &Path| {
        script(&[
            ("file.open", json!({ "dir": dir.to_string_lossy() })),
            ("page.last", json!({})),
            ("page.previous", json!({})),
            ("ui.describe", json!({})),
            ("tools.palette", json!({"query": "page"})),
            ("ui.describe", json!({})),
            ("ui.dismiss", json!({})),
            ("help.about", json!({})),
            ("ui.describe", json!({})),
            ("file.close", json!({})),
        ])
    };
    // Each run has settings of its own: the recent list is part of what the start page shows.
    let (tmp_a, tmp_b, tmp_c) = (Tmp::new(), Tmp::new(), Tmp::new());
    let a = run_script_file(&tmp_a, "a.jsonl", &steps(&tmp_a.golden("one")), &[]);
    let b = run_script_file(&tmp_b, "b.jsonl", &steps(&tmp_b.golden("two")), &[]);
    assert_eq!(a.status.code(), Some(0), "{}", stderr(&a));
    assert_eq!(b.status.code(), Some(0), "{}", stderr(&b));
    assert!(a.stdout.len() > 2000);
    assert_eq!(a.stdout, b.stdout);

    // The same script on stdin, line by line, gives the same transcript.
    let dir = tmp_c.golden("three");
    let mut child = Command::new(bin_exe())
        .args(["--headless", "--script", "-"])
        .env("EW_CONFIG_DIR", tmp_c.path("config"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(steps(&dir).as_bytes())
        .unwrap();
    let c = child.wait_with_output().unwrap();
    assert_eq!(c.status.code(), Some(0), "{}", stderr(&c));
    assert_eq!(c.stdout, a.stdout);
}

#[test]
fn exit_codes_follow_the_spec() {
    let tmp = Tmp::new();
    // 1: a refused step ends the run there.
    let out = run_script_file(
        &tmp,
        "refused.jsonl",
        &script(&[("file.save", json!({})), ("help.about", json!({}))]),
        &[],
    );
    assert_eq!(out.status.code(), Some(1));
    let lines = transcript(&out);
    assert_eq!(lines.len(), 2, "the header and the refused step");
    assert_eq!(lines[1]["outcome"], "refused");
    assert_eq!(lines[1]["reason"], "reason-no-project");

    // 1: a failed step (a directory that is no project) ends it too, with no path in the message.
    let missing = tmp.path("nothing-here");
    let out = run_script_file(
        &tmp,
        "failed.jsonl",
        &script(&[("file.open", json!({"dir": missing.to_string_lossy()}))]),
        &[],
    );
    assert_eq!(out.status.code(), Some(1));
    let lines = transcript(&out);
    assert_eq!(lines[1]["outcome"], "failed");
    let message = lines[1]["message"].as_str().unwrap();
    assert!(message.contains("nothing-here"), "{message}");
    assert!(!message.contains(&*tmp.0.to_string_lossy()), "{message}");

    // 2: an unknown action ID is found before the first step runs: no transcript at all.
    let out = run_script_file(
        &tmp,
        "unknown.jsonl",
        &script(&[("help.about", json!({})), ("no.such.action", json!({}))]),
        &[],
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(stderr(&out).contains("no.such.action"), "{}", stderr(&out));

    // 2: a script without its header, an unreadable file, and bad usage.
    let out = run_script_file(&tmp, "nohead.jsonl", "{\"action\":\"help.about\"}\n", &[]);
    assert_eq!(out.status.code(), Some(2));
    let out = elektrawatt(
        &tmp,
        &[
            "--headless".as_ref(),
            "--script".as_ref(),
            tmp.path("missing.jsonl").as_os_str(),
        ],
    );
    assert_eq!(out.status.code(), Some(2));
    let out = elektrawatt(&tmp, &["--headless".as_ref()]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("usage"));
}

/// WP-S6.13 BD1: with `--keep-going` a refused step in the middle of a script does not end the run; every step has
/// its line and the exit code is 1. Without the flag the run stops at the refusal, as it always did, and the lines it
/// wrote are the same bytes. The flag belongs to the headless mode, and the usage text lists it.
#[test]
fn keep_going_runs_a_script_past_a_refused_step() {
    let tmp = Tmp::new();
    let text = script(&[
        ("help.about", json!({})),
        ("file.save", json!({})),
        ("ui.dismiss", json!({})),
    ]);
    let stopped = run_script_file(&tmp, "stop.jsonl", &text, &[]);
    assert_eq!(stopped.status.code(), Some(1));
    let stopped_lines = transcript(&stopped);
    assert_eq!(stopped_lines.len(), 3, "the header and two steps");
    assert_eq!(stopped_lines[2]["outcome"], "refused");

    let kept = run_script_file(&tmp, "keep.jsonl", &text, &["--keep-going".as_ref()]);
    assert_eq!(kept.status.code(), Some(1), "{}", stderr(&kept));
    let kept_lines = transcript(&kept);
    assert_eq!(kept_lines.len(), 4, "the header and all three steps");
    assert_eq!(kept_lines[2]["outcome"], "refused");
    assert_eq!(kept_lines[3]["action"], "ui.dismiss");
    assert_eq!(kept_lines[3]["outcome"], "done");
    // The lines the two runs share are equal byte for byte.
    let (a, b) = (
        String::from_utf8(stopped.stdout).unwrap(),
        String::from_utf8(kept.stdout).unwrap(),
    );
    assert!(b.starts_with(&a), "{a}\n---\n{b}");

    // A script whose steps are all done exits 0 with the flag too.
    let clean = run_script_file(
        &tmp,
        "clean.jsonl",
        &script(&[("help.about", json!({})), ("ui.dismiss", json!({}))]),
        &["--keep-going".as_ref()],
    );
    assert_eq!(clean.status.code(), Some(0), "{}", stderr(&clean));

    // The flag without `--headless` is a usage error, and the usage text lists it.
    let out = elektrawatt(&tmp, &["--keep-going".as_ref()]);
    assert_eq!(out.status.code(), Some(2));
    let out = elektrawatt(&tmp, &["--help".as_ref()]);
    assert!(String::from_utf8_lossy(&out.stdout).contains("--keep-going"));
}

#[test]
fn unsaved_changes_need_discard_in_a_script() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g");
    let edit = json!({"cmd": "page.create", "params": {"number": 998}});
    let out = run_script_file(
        &tmp,
        "dirty.jsonl",
        &script(&[
            ("file.open", json!({"dir": dir.to_string_lossy()})),
            ("command.run", edit.clone()),
            ("file.close", json!({})),
        ]),
        &[],
    );
    assert_eq!(out.status.code(), Some(1));
    let lines = transcript(&out);
    assert_eq!(lines[3]["outcome"], "failed");
    assert!(lines[3]["message"].as_str().unwrap().contains("discard"));
    // The first run kept its journal for `dir`; a fresh copy shows the discard path.
    let fresh = tmp.golden("g2");
    let out = run_script_file(
        &tmp,
        "discard.jsonl",
        &script(&[
            ("file.open", json!({"dir": fresh.to_string_lossy()})),
            ("command.run", edit),
            ("edit.undo", json!({})),
            ("file.close", json!({"discard": true})),
        ]),
        &[],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

/// A8: `--smoke` on a copy of Golden A exits 0, on a missing directory 1.
#[test]
fn the_smoke_flag_opens_read_only_and_draws_a_page() {
    let tmp = Tmp::new();
    let dir = tmp.golden("s");
    let before = listing(&dir);
    let out = elektrawatt(&tmp, &["--smoke".as_ref(), dir.as_os_str()]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(text.starts_with("smoke ok: "), "{text}");
    assert!(text.contains("at 36 dpi"), "{text}");
    // Read-only: the project directory is as it was (no lock file, no new generation).
    assert_eq!(listing(&dir), before);
    // And no settings were written.
    assert!(!tmp.path("config").exists());

    let out = elektrawatt(&tmp, &["--smoke".as_ref(), tmp.path("missing").as_os_str()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).starts_with("error: "));
}

#[test]
fn version_and_help_work() {
    let tmp = Tmp::new();
    let out = elektrawatt(&tmp, &["--version".as_ref()]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        format!("elektrawatt {}\n", env!("CARGO_PKG_VERSION"))
    );
    let out = elektrawatt(&tmp, &["--help".as_ref()]);
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("--headless"));
}

/// Review ruling 6 (data loss): a script that ends with unsaved changes exits 1 and keeps their journal; the next
/// open offers it. `app.quit` with `discard` ends a script that edits cleanly.
#[test]
fn a_script_that_ends_dirty_exits_one_and_keeps_the_journal() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g");
    let edit = json!({"cmd": "page.create", "params": {"number": 994}});
    let out = run_script_file(
        &tmp,
        "dirty-end.jsonl",
        &script(&[
            ("file.open", json!({"dir": dir.to_string_lossy()})),
            ("command.run", edit.clone()),
        ]),
        &[],
    );
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("unsaved changes"), "{}", stderr(&out));
    // Every step itself was done: the exit code is about the end of the run.
    assert!(transcript(&out)[1..].iter().all(|s| s["outcome"] == "done"));

    // The journal is there: the next run opens the project read-only with the recovery banner.
    let out = run_script_file(
        &tmp,
        "after.jsonl",
        &script(&[("file.open", json!({"dir": dir.to_string_lossy()}))]),
        &[],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let lines = transcript(&out);
    assert_eq!(lines[1]["state"]["access"], "read-only");
    assert!(
        lines[1]["state"]["banner"]
            .as_str()
            .unwrap()
            .contains("unsaved changes")
    );

    // A fresh copy: the explicit discard ends cleanly and leaves no journal.
    let dir2 = tmp.golden("h");
    let out = run_script_file(
        &tmp,
        "quit.jsonl",
        &script(&[
            ("file.open", json!({"dir": dir2.to_string_lossy()})),
            ("command.run", edit),
            ("app.quit", json!({"discard": true})),
            ("help.about", json!({})),
        ]),
        &[],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        transcript(&out).len(),
        4,
        "the header and three steps: quit ends the script"
    );
    let out = run_script_file(
        &tmp,
        "after2.jsonl",
        &script(&[("file.open", json!({"dir": dir2.to_string_lossy()}))]),
        &[],
    );
    assert_eq!(transcript(&out)[1]["state"]["access"], "edit");
}

/// Review ruling 7: without `EW_CONFIG_DIR` a headless run has no settings directory of its own choosing: it
/// neither reads nor writes the user's, and `--config-dir` names one.
#[test]
fn a_headless_run_leaves_the_users_settings_alone() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g");
    let home = tmp.path("home");
    std::fs::create_dir_all(&home).unwrap();
    let text = script(&[
        ("file.open", json!({"dir": dir.to_string_lossy()})),
        ("file.close", json!({})),
    ]);
    let file = tmp.path("s.jsonl");
    std::fs::write(&file, &text).unwrap();
    let run = |extra: &[&std::ffi::OsStr]| {
        Command::new(bin_exe())
            .args(["--headless", "--script"])
            .arg(&file)
            .args(extra)
            .env_remove("EW_CONFIG_DIR")
            .env("HOME", &home)
            .env("APPDATA", &home)
            .env("LOCALAPPDATA", &home)
            .env("XDG_CONFIG_HOME", &home)
            .env("XDG_STATE_HOME", &home)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        listing(&home).is_empty(),
        "nothing was written to the user's directories"
    );

    // `--config-dir` names a directory for the run.
    let config = tmp.path("chosen");
    let out = run(&["--config-dir".as_ref(), config.as_os_str()]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let settings = std::fs::read_to_string(config.join("settings.toml")).unwrap();
    assert!(settings.contains("recent_projects"), "{settings}");
    assert!(listing(&home).is_empty());
}

/// The path of the `elektrawatt` binary under test. nextest sets `NEXTEST_BIN_EXE_elektrawatt` at run time (the binaries of a
/// `--archive-file` run are unpacked elsewhere than the compile-time path, FT-353); a plain `cargo test` falls back
/// to the compile-time path.
// Allowed: the program boundary of a test — nextest's binary path is read once, never persisted or printed.
#[allow(clippy::disallowed_methods)]
fn bin_exe() -> std::path::PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_elektrawatt")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_BIN_EXE_elektrawatt"))) // bin-exe-fallback
}
