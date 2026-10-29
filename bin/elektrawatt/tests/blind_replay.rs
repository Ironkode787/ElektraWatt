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
//!
//! A visual recording (`run.json` `senses = "visual"`, WP-S6.13 part 4) is replayed with `--visual --no-frames`: its
//! input and frame lines are settled as in the run and no frame is painted, so it runs on every OS. R1 then compares
//! each input's and each frame line's `dispatched` (and `closed`) too; the frames themselves are a report (§5.4).
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
    /// A visual run's recording.
    visual: bool,
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
        visual: run["senses"] == "visual",
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
    recordings_in(Path::new(RECORDINGS))
}

/// The recordings under `dir`, by name.
fn recordings_in(dir: &Path) -> Vec<(String, Recording)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
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
        .args(if rec.visual {
            &["--visual", "--no-frames"][..]
        } else {
            &[]
        })
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
    // ID-630 (d): a raw UUID in any replayed line is a product defect, named by its line.
    if let Some(at) = lines.iter().position(|l| has_uuid(l)) {
        failures.push(format!(
            "a raw UUID in replayed transcript line {}: {}",
            at + 1,
            lines[at].chars().take(160).collect::<String>()
        ));
    }
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
        // A text step's action, outcome and reason; a visual input's or frame line's dispatch (absent members of
        // the other kind compare as equal nulls).
        for key in [
            "action",
            "outcome",
            "reason",
            "input",
            "dispatched",
            "frame",
            "closed",
        ] {
            if got[key] != want[key] {
                let what = want["action"]
                    .as_str()
                    .or_else(|| want["input"]["input"].as_str())
                    .or_else(|| want["frame"].as_str())
                    .unwrap_or("?");
                failures.push(format!(
                    "R1 step {} ({what}): {key} was {} and is now {}",
                    i + 1,
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

/// What replaying a folder of recordings found.
struct Outcome {
    /// Every failure, with the recording's name.
    failures: Vec<String>,
    /// The recordings left out because they are stale: `name: reason`.
    skipped: Vec<String>,
}

/// Replays the recordings of `dir` whose `ci` is `which` twice and reports every failure at once; a recording with
/// `stale` set is skipped with its reason (D15).
fn run_recordings(dir: &Path, which: &str) -> Outcome {
    let mut out = Outcome {
        failures: Vec::new(),
        skipped: Vec::new(),
    };
    for (name, rec) in recordings_in(dir).iter().filter(|(_, r)| r.ci == which) {
        if let Some(reason) = &rec.stale {
            eprintln!("recording {name} is stale and skipped: {reason}");
            out.skipped.push(format!("{name}: {reason}"));
            continue;
        }
        let first = replay(rec);
        let second = replay(rec);
        out.failures
            .extend(check(rec, &first).into_iter().map(|f| format!("{name}: {f}")));
        if first != second {
            let at = first
                .iter()
                .zip(&second)
                .position(|(a, b)| a != b)
                .unwrap_or(first.len().min(second.len()));
            out.failures.push(format!(
                "{name}: two replays in fresh folders differ at transcript line {}",
                at + 1
            ));
        }
    }
    out
}

#[test]
fn blind_replay() {
    let out = run_recordings(Path::new(RECORDINGS), "pr");
    assert!(out.failures.is_empty(), "{}", out.failures.join("\n"));
}

/// The long recordings (`ci = "nightly"`), replayed by the deep nightly (FT-414).
#[test]
#[ignore = "the nightly's"]
fn blind_replay_long() {
    let out = run_recordings(Path::new(RECORDINGS), "nightly");
    assert!(out.failures.is_empty(), "{}", out.failures.join("\n"));
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

/// BV10: the visual self-test recording runs in the PR jobs on every OS (replayed without painting), and a changed
/// dispatch of one of its inputs fails R1 naming the input.
#[test]
fn the_visual_self_test_recording_replays_and_compares_the_dispatches() {
    let all = recordings();
    let visual = &all
        .iter()
        .find(|(n, _)| n == "selftest-visual")
        .expect("tools/blind/recordings/selftest-visual is committed (`cargo xtask blind selftest-visual` writes it)")
        .1;
    assert!(visual.visual && visual.ci == "pr" && visual.stale.is_none());
    let lines = replay(visual);
    assert!(check(visual, &lines).is_empty(), "{:?}", check(visual, &lines));
    let mut bad = load(&visual.dir);
    let at = bad
        .outcomes
        .iter()
        .position(|o| o["dispatched"][0]["outcome"] == "refused")
        .expect("the visual self-test has an input whose action is refused");
    bad.outcomes[at]["dispatched"][0]["outcome"] = Value::from("done");
    let failures = check(&bad, &lines);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].starts_with(&format!("R1 step {} (key): dispatched was", at + 1)),
        "{failures:?}"
    );
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

/// A recording with `stale` set is skipped with its reason (BD12): a copy of the self-test recording with a corrupted
/// `outcomes.jsonl` fails while it is not stale, and is skipped, with its reason, once `stale` is set.
#[test]
fn a_stale_recording_is_skipped_with_its_reason() {
    let tmp = Tmp::new();
    let all = recordings();
    let selftest = &all.iter().find(|(n, _)| n == "selftest").unwrap().1;
    let copy = tmp.0.join("copy");
    std::fs::create_dir_all(&copy).unwrap();
    for f in [
        "prelude.uiscript",
        "steps.uiscript",
        "epilogue.txt",
        "turns.jsonl",
    ] {
        std::fs::copy(selftest.dir.join(f), copy.join(f)).unwrap();
    }
    // The first outcome is corrupted: `file.new` is said to have been refused.
    let outcomes = std::fs::read_to_string(selftest.dir.join("outcomes.jsonl")).unwrap();
    let corrupted = outcomes.replacen("\"outcome\":\"done\"", "\"outcome\":\"refused\"", 1);
    assert_ne!(corrupted, outcomes);
    std::fs::write(copy.join("outcomes.jsonl"), corrupted).unwrap();
    let mut run: Value =
        serde_json::from_str(&std::fs::read_to_string(selftest.dir.join("run.json")).unwrap()).unwrap();
    std::fs::write(copy.join("run.json"), run.to_string()).unwrap();

    // Not stale: the corrupted outcome fails the replay.
    let live = run_recordings(&tmp.0, "pr");
    assert!(live.skipped.is_empty(), "{:?}", live.skipped);
    assert!(
        live.failures.iter().any(|f| f.starts_with("copy: R1 step 1")),
        "{:?}",
        live.failures
    );

    // Stale: skipped, with its reason, and nothing fails.
    run["stale"] = Value::from("the wizard's step order changed (WP-X)");
    std::fs::write(copy.join("run.json"), run.to_string()).unwrap();
    let skipped = run_recordings(&tmp.0, "pr");
    assert!(skipped.failures.is_empty(), "{:?}", skipped.failures);
    assert_eq!(skipped.skipped, ["copy: the wizard's step order changed (WP-X)"]);
}

/// A raw UUID in any replayed line fails the check and names the line (ID-630 (d)).
#[test]
fn a_raw_uuid_in_a_replayed_line_fails() {
    let all = recordings();
    let selftest = &all.iter().find(|(n, _)| n == "selftest").unwrap().1;
    let mut lines = replay(selftest);
    assert!(check(selftest, &lines).is_empty());
    lines[2] = lines[2].replacen(
        "\"state\"",
        "\"id\":\"0197a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b\",\"state\"",
        1,
    );
    let failures = check(selftest, &lines);
    assert!(
        failures
            .iter()
            .any(|f| f.starts_with("a raw UUID in replayed transcript line 3")),
        "{failures:?}"
    );
}
