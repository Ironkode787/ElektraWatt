// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The CI replay of the blind drafter's recordings (WP-S6.13 part 1, D15, acceptance check BD12).
//!
//! `tools/blind/recordings/<name>/` holds what a run needs to be replayed: `prelude.uiscript`, `steps.uiscript` (the
//! agent's steps, the harness's looks and the epilogue), `outcomes.jsonl` (per step its action, outcome and reason) and
//! `epilogue.txt` (the `page.describe` texts of the epilogue). This test replays every recording whose `run.json` says
//! `ci = "pr"` and `stale = null` through the built binary, **twice, each time in fresh folders**, with the
//! configuration folder, the sample folder and the guide folder set as the harness's keeper sets them (an empty sample
//! folder, an empty guide, a fresh config), and checks **R1** (every step's action, outcome and reason equal the
//! recording's) and **R2** (the epilogue's page texts equal `epilogue.txt` byte for byte). Outcomes and page texts carry
//! no record ID (the product prints the alias `@n`), so the test needs no normaliser, and a raw UUID in a replayed line
//! fails it. A recording with `stale` set is skipped with its reason; the self-test recording is never stale.
//! `blind_replay_long` (ignored) does the same for `ci = "nightly"`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::Value;

const RECORDINGS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/blind/recordings");
const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary folder, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-blind-replay-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
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

/// A recording read from its folder; `outcomes` may be replaced by a test.
struct Recording {
    dir: PathBuf,
    ci: String,
    stale: Option<String>,
    script: Vec<String>,
    outcomes: Vec<Value>,
    epilogue_text: String,
    epilogue_range: Option<(usize, usize)>,
    prelude_len: usize,
}

fn script_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .skip(1)
        .map(str::to_owned)
        .collect()
}

fn load(dir: &Path) -> Recording {
    let run: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
    let prelude = script_lines(&dir.join("prelude.uiscript"));
    let mut script = prelude.clone();
    script.extend(script_lines(&dir.join("steps.uiscript")));
    let outcomes = std::fs::read_to_string(dir.join("outcomes.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("outcomes.jsonl: {l}: {e}")))
        .collect();
    let turns = std::fs::read_to_string(dir.join("turns.jsonl")).unwrap();
    let epilogue_range = turns
        .lines()
        .skip(1)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .find_map(|l| {
            let t: Value = serde_json::from_str(l).ok()?;
            (t["kind"] == "epilogue").then(|| {
                (
                    t["steps"][0].as_u64().unwrap() as usize,
                    t["steps"][1].as_u64().unwrap() as usize,
                )
            })
        });
    Recording {
        dir: dir.to_path_buf(),
        ci: run["ci"].as_str().unwrap_or("none").to_owned(),
        stale: run["stale"].as_str().map(str::to_owned),
        script,
        outcomes,
        epilogue_text: std::fs::read_to_string(dir.join("epilogue.txt"))
            .unwrap()
            .replace("\r\n", "\n"),
        epilogue_range,
        prelude_len: prelude.len(),
    }
}

/// The recordings under `tools/blind/recordings`, by name.
fn recordings() -> Vec<(String, Recording)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(RECORDINGS) else {
        return out;
    };
    for e in entries.flatten() {
        if e.path().join("run.json").is_file() {
            out.push((e.file_name().to_string_lossy().into_owned(), load(&e.path())));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Replays `rec` in fresh folders and returns the transcript lines after the header.
fn replay(rec: &Recording) -> Vec<String> {
    let tmp = Tmp::new();
    for d in ["work", "config", "empty-sample", "book"] {
        std::fs::create_dir_all(tmp.0.join(d)).unwrap();
    }
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"replay\"}\n");
    for l in &rec.script {
        text.push_str(l);
        text.push('\n');
    }
    std::fs::write(tmp.0.join("replay.uiscript"), text).unwrap();
    let out = Command::new(bin_exe())
        .args(["--headless", "--script"])
        .arg(tmp.0.join("replay.uiscript"))
        .args(["--keep-going", "--config-dir"])
        .arg(tmp.0.join("config"))
        .current_dir(tmp.0.join("work"))
        .env("EW_SAMPLE_DIR", tmp.0.join("empty-sample"))
        .env("EW_DOCS_DIR", tmp.0.join("book"))
        .env("EW_CONTENT_DIR", CONTENT)
        .env_remove("EW_CONFIG_DIR")
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .skip(1)
        .map(str::to_owned)
        .collect()
}

/// R1 and R2 over one replay: the failures, empty when both hold.
fn check(rec: &Recording, lines: &[String]) -> Vec<String> {
    let mut failures = Vec::new();
    if lines.len() < rec.outcomes.len() {
        failures.push(format!(
            "the replay wrote {} steps and the recording has {}",
            lines.len(),
            rec.outcomes.len()
        ));
    }
    for (i, want) in rec.outcomes.iter().enumerate() {
        let Some(line) = lines.get(i) else { break };
        let got: Value = serde_json::from_str(line).unwrap_or_else(|e| panic!("step {}: {line}: {e}", i + 1));
        for key in ["action", "outcome", "reason"] {
            if got[key] != want[key] {
                failures.push(format!(
                    "R1 step {} ({}): {key} was {} and is now {}",
                    i + 1,
                    want["action"].as_str().unwrap_or("?"),
                    want[key],
                    got[key]
                ));
                break;
            }
        }
    }
    if let Some((from, to)) = rec.epilogue_range {
        let mut text = String::new();
        for k in from..=to {
            let (Some(script_line), Some(line)) = (
                rec.script.get(rec.prelude_len + k - 1),
                lines.get(rec.prelude_len + k - 1),
            ) else {
                continue;
            };
            let step: Value = serde_json::from_str(script_line).unwrap();
            if step["action"] != "page.describe" {
                continue;
            }
            let got: Value = serde_json::from_str(line).unwrap();
            if let Some(describe) = got["describe"].as_str() {
                let folio = step["params"]["folio"].as_str().unwrap_or("");
                text.push_str(&format!("== page.describe {folio} ==\n{describe}\n"));
            }
        }
        if text != rec.epilogue_text {
            failures.push(format!(
                "R2 the epilogue's page texts differ from {}/epilogue.txt",
                rec.dir.display()
            ));
        }
        // No record ID in a page text: the product prints aliases.
        if has_uuid(&text) {
            failures.push("a raw UUID in the page texts".to_owned());
        }
    }
    failures
}

fn has_uuid(text: &str) -> bool {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let b = text.as_bytes();
    'start: for i in 0..b.len() {
        let mut j = i;
        for (g, n) in GROUPS.iter().enumerate() {
            if g > 0 {
                if b.get(j) != Some(&b'-') {
                    continue 'start;
                }
                j += 1;
            }
            for _ in 0..*n {
                if !b.get(j).is_some_and(u8::is_ascii_hexdigit) {
                    continue 'start;
                }
                j += 1;
            }
        }
        return true;
    }
    false
}

/// Replays the recordings whose `ci` is `which` twice and reports every failure at once.
fn run_recordings(which: &str) {
    let mut failures = Vec::new();
    for (name, rec) in recordings().iter().filter(|(_, r)| r.ci == which) {
        if let Some(reason) = &rec.stale {
            eprintln!("recording {name} is stale and skipped: {reason}");
            continue;
        }
        let first = replay(rec);
        let second = replay(rec);
        failures.extend(check(rec, &first).into_iter().map(|f| format!("{name}: {f}")));
        if first != second {
            let at = first
                .iter()
                .zip(&second)
                .position(|(a, b)| a != b)
                .unwrap_or(first.len().min(second.len()));
            failures.push(format!(
                "{name}: two replays in fresh folders differ at transcript line {}",
                at + 1
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn blind_replay() {
    run_recordings("pr");
}

/// The long recordings (`ci = "nightly"`), replayed by the deep nightly (FT-414).
#[test]
#[ignore = "the nightly's"]
fn blind_replay_long() {
    run_recordings("nightly");
}

/// The self-test recording exists, runs in the PR jobs and is never stale (D15).
#[test]
fn the_self_test_recording_is_a_pr_recording_and_never_stale() {
    let all = recordings();
    let selftest = &all
        .iter()
        .find(|(n, _)| n == "selftest")
        .expect("tools/blind/recordings/selftest is committed (`cargo xtask blind selftest` writes it)")
        .1;
    assert_eq!(selftest.ci, "pr");
    assert_eq!(
        selftest.stale, None,
        "the self-test recording is re-blessed, never marked stale"
    );
    assert!(
        selftest.script.len() <= 100,
        "a `pr` recording has at most 100 steps"
    );
    // The recording holds the replay inputs and no full transcript.
    assert!(!selftest.dir.join("transcript.jsonl").exists());
}

/// A recording whose outcomes were corrupted fails R1, naming the step.
#[test]
fn a_corrupted_outcome_fails_the_replay() {
    let all = recordings();
    let selftest = &all.iter().find(|(n, _)| n == "selftest").unwrap().1;
    let lines = replay(selftest);
    assert!(
        check(selftest, &lines).is_empty(),
        "{:?}",
        check(selftest, &lines)
    );
    let mut bad = load(&selftest.dir);
    let at = bad
        .outcomes
        .iter()
        .position(|o| o["outcome"] == "refused")
        .expect("the self-test has a refused step");
    bad.outcomes[at]["outcome"] = Value::from("done");
    let failures = check(&bad, &lines);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].starts_with(&format!("R1 step {}", at + 1)),
        "{failures:?}"
    );
    // A changed epilogue text fails R2.
    let mut bad = load(&selftest.dir);
    bad.epilogue_text.push_str("extra\n");
    assert!(check(&bad, &lines).iter().any(|f| f.starts_with("R2")));
}

/// A recording with `stale` set is skipped with its reason: here a copy of the self-test recording is marked stale and
/// its `run.json` says why; `run_recordings` leaves it out.
#[test]
fn a_stale_recording_is_skipped_with_its_reason() {
    let tmp = Tmp::new();
    let all = recordings();
    let selftest = &all.iter().find(|(n, _)| n == "selftest").unwrap().1;
    let mut run: Value =
        serde_json::from_str(&std::fs::read_to_string(selftest.dir.join("run.json")).unwrap()).unwrap();
    run["stale"] = Value::from("the wizard's step order changed (WP-X)");
    std::fs::write(tmp.0.join("run.json"), run.to_string()).unwrap();
    for f in [
        "prelude.uiscript",
        "steps.uiscript",
        "outcomes.jsonl",
        "epilogue.txt",
        "turns.jsonl",
    ] {
        std::fs::copy(selftest.dir.join(f), tmp.0.join(f)).unwrap();
    }
    let rec = load(&tmp.0);
    assert_eq!(
        rec.stale.as_deref(),
        Some("the wizard's step order changed (WP-X)")
    );
    assert_eq!(rec.ci, "pr");
}
