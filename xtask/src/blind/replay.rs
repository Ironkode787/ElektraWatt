// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Recording, bless and replay (WP-S6.13 §5.10, D14, D15).
//!
//! The UI script is the replay artefact: a run's `steps.uiscript` (prelude, the agent's steps, the epilogue) replayed
//! through the built binary reproduces the run's outcomes and its `page.describe` epilogue. **R1** every step's action,
//! outcome and reason equal `outcomes.jsonl`; **R2** the epilogue's page texts equal `epilogue.txt` byte for byte; **R3**
//! when the run's own `transcript.jsonl` is at hand the normalised transcripts are compared and the differing lines are
//! counted, as a report and not as a failure (a reviewed vocabulary change alters describe text without changing what the
//! agent could do). R1 and R2 decide. A raw UUID in a replayed line is a product defect and fails the replay, naming the
//! line (ID-630 (d)).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::normalise::{Normaliser, absolute_path_in, find_uuid_line, first_uuid};
use super::run::{Run, RunJson, Turn, read_script, script_header, step_line};

/// The folder of the committed recordings.
pub fn recordings_dir(root: &Path) -> PathBuf {
    root.join("tools/blind/recordings")
}

/// `outcomes.jsonl` from the lines of a transcript (after its header): `step`, `action`, `outcome`, `reason` per step.
pub fn outcomes_jsonl(transcript: &[String]) -> Result<String> {
    let mut out = String::new();
    for line in transcript {
        let v: Value =
            serde_json::from_str(line).with_context(|| format!("not a transcript line: {line}"))?;
        let o = json!({
            "step": v["step"],
            "action": v["action"],
            "outcome": v["outcome"],
            "reason": v["reason"],
        });
        out.push_str(&format!("{o}\n"));
    }
    Ok(out)
}

/// `epilogue.txt`: the `page.describe` texts of the epilogue, each as `== page.describe <folio> ==` and its text. `lines`
/// are the transcript lines of the whole script (prelude and steps), `prelude` the number of prelude steps and `range`
/// the epilogue's 1-based lines in `steps.uiscript`.
pub fn epilogue_text(steps: &[(String, Value)], lines: &[String], prelude: usize, range: [u32; 2]) -> String {
    let mut out = String::new();
    for k in range[0]..=range[1] {
        let k = k as usize;
        let (Some((action, params)), Some(line)) = (steps.get(k - 1), lines.get(prelude + k - 1)) else {
            continue;
        };
        if action != "page.describe" {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(text) = v["describe"].as_str() {
            let folio = params["folio"].as_str().unwrap_or("");
            out.push_str(&format!("== page.describe {folio} ==\n{text}\n"));
        }
    }
    out
}

/// What a replay needs, from a recording or a stopped run.
#[derive(Clone, Debug)]
pub struct Inputs {
    /// The prelude's steps.
    pub prelude: Vec<(String, Value)>,
    /// The steps after the prelude (the agent's, the looks, the epilogue).
    pub steps: Vec<(String, Value)>,
    /// The expected `outcomes.jsonl` lines, parsed.
    pub outcomes: Vec<Value>,
    /// The expected `epilogue.txt`.
    pub epilogue: String,
    /// The epilogue's range in `steps.uiscript`, if the run had one.
    pub epilogue_range: Option<[u32; 2]>,
    /// The run's own transcript lines, normalised with its folder, when at hand (for R3).
    pub transcript: Option<Vec<String>>,
    /// Why the recording is stale, if it is.
    pub stale: Option<String>,
}

fn epilogue_range(turns: &[Turn]) -> Option<[u32; 2]> {
    turns
        .iter()
        .rev()
        .find(|t| t.kind == "epilogue")
        .and_then(|t| t.steps)
}

/// Reads the inputs of the replay from `dir`: a recording (with `outcomes.jsonl`) or a stopped run folder.
///
/// # Errors
///
/// A file that is missing or not valid.
pub fn load_inputs(dir: &Path) -> Result<Inputs> {
    let prelude = read_script(&dir.join("prelude.uiscript"))?;
    let steps = read_script(&dir.join("steps.uiscript"))?;
    let turns_text = std::fs::read_to_string(dir.join("turns.jsonl")).context("reading turns.jsonl")?;
    let turns: Vec<Turn> = turns_text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()
        .context("parsing turns.jsonl")?;
    let range = epilogue_range(&turns);
    let rj: RunJson =
        serde_json::from_str(&std::fs::read_to_string(dir.join("run.json")).context("reading run.json")?)
            .context("parsing run.json")?;
    let (outcomes, epilogue, transcript) = if dir.join("outcomes.jsonl").is_file() {
        let outcomes = std::fs::read_to_string(dir.join("outcomes.jsonl"))?
            .lines()
            .map(serde_json::from_str)
            .collect::<std::result::Result<Vec<Value>, _>>()?;
        let epilogue = std::fs::read_to_string(dir.join("epilogue.txt"))
            .unwrap_or_default()
            .replace("\r\n", "\n");
        (outcomes, epilogue, None)
    } else {
        let text =
            std::fs::read_to_string(dir.join("transcript.jsonl")).context("reading transcript.jsonl")?;
        let lines: Vec<String> = text.lines().skip(1).map(str::to_owned).collect();
        let outcomes = outcomes_jsonl(&lines)?
            .lines()
            .map(serde_json::from_str)
            .collect::<std::result::Result<Vec<Value>, _>>()?;
        let epilogue = range.map_or_else(String::new, |r| epilogue_text(&steps, &lines, prelude.len(), r));
        // The run's own lines are normalised like the replay's: with this run folder's path.
        let norm = Normaliser::new(dir);
        (
            outcomes,
            epilogue,
            Some(lines.iter().map(|l| norm.text(l)).collect()),
        )
    };
    Ok(Inputs {
        prelude,
        steps,
        outcomes,
        epilogue,
        epilogue_range: range,
        transcript,
        stale: rj.stale,
    })
}

/// What one replay found.
#[derive(Clone, Debug, Default)]
pub struct Replayed {
    /// The transcript lines of the replay (after the header), the run folder and the version normalised.
    pub lines: Vec<String>,
    /// R1 failures: a step whose action, outcome or reason differs, or a missing step.
    pub r1: Vec<String>,
    /// R2 failures: the epilogue text differs.
    pub r2: Vec<String>,
    /// A raw UUID in a line: `(line, uuid)`.
    pub uuid: Option<(usize, String)>,
    /// R3: the number of lines that differ from the run's own transcript, when it is at hand.
    pub r3: Option<usize>,
}

impl Replayed {
    /// Whether R1 and R2 hold and no UUID reached the output.
    pub fn ok(&self) -> bool {
        self.r1.is_empty() && self.r2.is_empty() && self.uuid.is_none()
    }

    /// The failures as text.
    pub fn failures(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        out.extend(self.r1.iter().map(|m| format!("R1 {m}")));
        out.extend(self.r2.iter().map(|m| format!("R2 {m}")));
        if let Some((line, uuid)) = &self.uuid {
            out.push(format!(
                "a raw UUID {uuid} reached the output at transcript line {line}: a product defect"
            ));
        }
        out
    }
}

static REPLAYS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// A fresh scratch folder for one replay: `<temp>/ew-blind-replay/<token>-<n>`.
fn scratch() -> PathBuf {
    let n = REPLAYS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let token: String = super::keeper::new_token().chars().take(8).collect();
    std::env::temp_dir()
        .join("ew-blind-replay")
        .join(format!("{token}-{n}"))
}

/// Runs the combined script of `inputs` through `elektrawatt` in a fresh folder and compares.
///
/// # Errors
///
/// The product does not start.
pub fn replay_once(root: &Path, exe: &Path, inputs: &Inputs) -> Result<Replayed> {
    let dir = scratch();
    std::fs::create_dir_all(dir.join("work"))?;
    std::fs::create_dir_all(dir.join("config"))?;
    std::fs::create_dir_all(dir.join("empty-sample"))?;
    std::fs::create_dir_all(dir.join("book"))?;
    let mut script = format!("{}\n", script_header("replay"));
    for (a, p) in inputs.prelude.iter().chain(&inputs.steps) {
        script.push_str(&step_line(a, p));
        script.push('\n');
    }
    std::fs::write(dir.join("replay.uiscript"), script)?;
    let out = Command::new(exe)
        .args(["--headless", "--script"])
        .arg(dir.join("replay.uiscript"))
        .args(["--keep-going", "--config-dir"])
        .arg(dir.join("config"))
        .current_dir(dir.join("work"))
        .env("EW_SAMPLE_DIR", dir.join("empty-sample"))
        .env("EW_DOCS_DIR", dir.join("book"))
        .env("EW_CONTENT_DIR", root.join("content"))
        .env_remove("EW_CONFIG_DIR")
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("running {}", exe.display()))?;
    let normaliser = Normaliser::new(&dir);
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let raw: Vec<&str> = text.lines().collect();
    let mut result = Replayed::default();
    let body: Vec<&str> = raw.iter().skip(1).copied().collect();
    result.uuid = find_uuid_line(body.iter().copied());
    result.lines = body.iter().map(|l| normaliser.text(l)).collect();
    let expected = &inputs.outcomes;
    if body.len() < expected.len() {
        let err = String::from_utf8_lossy(&out.stderr);
        result.r1.push(format!(
            "the replay wrote {} steps and the recording has {} (stderr: {})",
            body.len(),
            expected.len(),
            err.lines().next().unwrap_or("")
        ));
    }
    for (i, want) in expected.iter().enumerate() {
        let Some(line) = body.get(i) else { break };
        let Ok(got) = serde_json::from_str::<Value>(line) else {
            result.r1.push(format!("step {}: not a transcript line", i + 1));
            continue;
        };
        for key in ["action", "outcome", "reason"] {
            if got[key] != want[key] {
                result.r1.push(format!(
                    "step {} ({}): {key} was {} and is now {}",
                    i + 1,
                    want["action"].as_str().unwrap_or("?"),
                    want[key],
                    got[key]
                ));
                break;
            }
        }
    }
    if let Some(range) = inputs.epilogue_range {
        let owned: Vec<String> = body.iter().map(|l| (*l).to_owned()).collect();
        let text = epilogue_text(&inputs.steps, &owned, inputs.prelude.len(), range);
        if text != inputs.epilogue {
            let at = text
                .lines()
                .zip(inputs.epilogue.lines())
                .position(|(a, b)| a != b)
                .map_or_else(
                    || text.lines().count().min(inputs.epilogue.lines().count()) + 1,
                    |i| i + 1,
                );
            result.r2.push(format!(
                "the epilogue's page texts differ from epilogue.txt at its line {at}"
            ));
        }
    } else if !inputs.epilogue.is_empty() {
        result
            .r2
            .push("the recording has an epilogue text and no epilogue".to_owned());
    }
    if let Some(own) = &inputs.transcript {
        let differing = own.iter().zip(&result.lines).filter(|(a, b)| a != b).count()
            + own.len().abs_diff(result.lines.len());
        result.r3 = Some(differing);
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(result)
}

/// The result of `replay [--twice]`.
#[derive(Clone, Debug)]
pub struct ReplayReport {
    /// The first replay.
    pub first: Replayed,
    /// The second (`--twice`), in another fresh folder.
    pub second: Option<Replayed>,
    /// Where the two replays differed: the first differing transcript line, 1-based.
    pub twice_differs_at: Option<usize>,
}

impl ReplayReport {
    /// Whether every check holds.
    pub fn ok(&self) -> bool {
        self.first.ok() && self.second.as_ref().is_none_or(Replayed::ok) && self.twice_differs_at.is_none()
    }
}

/// Replays `inputs` once, or twice in two fresh folders (which catches a derived name that depends on the random IDs of a
/// new project).
///
/// # Errors
///
/// The product does not start.
pub fn replay(root: &Path, exe: &Path, inputs: &Inputs, twice: bool) -> Result<ReplayReport> {
    let first = replay_once(root, exe, inputs)?;
    let (second, differs) = if twice {
        let second = replay_once(root, exe, inputs)?;
        let at = first
            .lines
            .iter()
            .zip(&second.lines)
            .position(|(a, b)| a != b)
            .or_else(|| {
                (first.lines.len() != second.lines.len()).then(|| first.lines.len().min(second.lines.len()))
            })
            .map(|i| i + 1);
        (Some(second), at)
    } else {
        (None, None)
    };
    Ok(ReplayReport {
        first,
        second,
        twice_differs_at: differs,
    })
}

/// The text `replay` prints.
pub fn replay_text(report: &ReplayReport) -> String {
    let mut out = String::new();
    for f in report.first.failures() {
        out.push_str(&format!("FAIL {f}\n"));
    }
    if let Some(s) = &report.second {
        for f in s.failures() {
            out.push_str(&format!("FAIL (second replay) {f}\n"));
        }
    }
    if let Some(at) = report.twice_differs_at {
        out.push_str(&format!("FAIL the two replays differ at transcript line {at}\n"));
    }
    if let Some(n) = report.first.r3 {
        out.push_str(&format!(
            "R3 {n} transcript line(s) differ from the run's own transcript (a report, not a failure)\n"
        ));
    }
    out.push_str(if report.ok() {
        "replay ok: R1 and R2 hold\n"
    } else {
        "replay FAILED\n"
    });
    out
}

/// `bless <run> --as <name> [--ci pr|nightly|none]`: copies the replay inputs and the evidence into
/// `tools/blind/recordings/<name>/`, normalised (the full transcript is not committed).
///
/// # Errors
///
/// A run that is not stopped, lost or diverged; an absolute path in what would be committed.
pub fn bless(root: &Path, run: &Run, name: &str, ci: &str) -> Result<PathBuf> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        bail!("`{name}` is not a recording name");
    }
    if !matches!(ci, "pr" | "nightly" | "none") {
        bail!("--ci is pr, nightly or none");
    }
    let rj = run.load()?;
    if rj.golden_changed {
        bail!(
            "run `{}`: golden/ changed during the run (D16); it is not blessed",
            rj.run
        );
    }
    if matches!(rj.end.as_deref(), Some("lost" | "diverged")) {
        bail!(
            "run `{}` ended `{}`: it is not blessed",
            rj.run,
            rj.end.as_deref().unwrap_or("")
        );
    }
    if !run.path("out/report.md").exists() {
        bail!("run `{}` is not stopped (`stop` writes out/report.md)", rj.run);
    }
    let inputs = load_inputs(&run.dir)?;
    let steps_n = inputs.steps.len();
    if ci == "pr" && steps_n + inputs.prelude.len() > 100 {
        bail!(
            "a recording with ci = \"pr\" has at most 100 steps; this one has {}",
            steps_n + inputs.prelude.len()
        );
    }
    let norm = Normaliser::new(&run.dir);
    let mut files: Vec<(String, String)> = Vec::new();
    let read = |rel: &str| std::fs::read_to_string(run.path(rel)).map(|t| t.replace("\r\n", "\n"));
    for rel in [
        "prompt.md",
        "prelude.uiscript",
        "steps.uiscript",
        "turns.jsonl",
        "drafter.md",
        "handback.md",
        "checker.md",
        "out/report.md",
        "out/check.txt",
        "out/bom.csv",
        "out/score.json",
    ] {
        if let Ok(text) = read(rel) {
            files.push((rel.to_owned(), norm.text(&text)));
        }
    }
    files.push(("outcomes.jsonl".to_owned(), outcomes_jsonl(&run.transcript()?)?));
    files.push(("epilogue.txt".to_owned(), norm.text(&inputs.epilogue)));
    // The PDF's digest ties a gate run's recording to the PDF attached to the release (FT-420). It is evidence only,
    // never compared, and not reproducible: a new project mints random IDs that reach the PDF. The self-test
    // recording therefore leaves it out, or every bless would change a committed file.
    if rj.task != "selftest"
        && let Ok(pdf) = std::fs::read_to_string(run.path("out/project-t1.pdf.sha256"))
    {
        files.push(("out/project-t1.pdf.sha256".to_owned(), pdf));
    }
    if let Ok(log) = read("record.ewcmd") {
        files.push(("record.ewcmd.txt".to_owned(), alias_uuids(&norm.text(&log))));
    }
    let mut recorded = rj.clone();
    recorded.ci = ci.to_owned();
    recorded.stale = None;
    files.push((
        "run.json".to_owned(),
        format!("{}\n", serde_json::to_string_pretty(&recorded)?),
    ));
    for (rel, text) in &files {
        if let Some(path) = absolute_path_in(text) {
            bail!("{rel} holds an absolute path ({path}): a recording holds none");
        }
        if first_uuid_in_steps(rel, text) {
            bail!("{rel} holds a raw UUID: the agent's steps carry none");
        }
    }
    let dest = recordings_dir(root).join(name);
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    for (rel, text) in files {
        let path = dest.join(&rel);
        std::fs::create_dir_all(path.parent().context("a path without a parent")?)?;
        std::fs::write(path, text)?;
    }
    Ok(dest)
}

/// Whether a file that must hold no ID does (`steps.uiscript`, `prelude.uiscript`).
fn first_uuid_in_steps(rel: &str, text: &str) -> bool {
    matches!(rel, "steps.uiscript" | "prelude.uiscript") && first_uuid(text).is_some()
}

/// Replaces every UUID of `text` by `@n`, numbered in order of first appearance (the command log, which `--record`
/// writes with the real IDs, is evidence only).
pub fn alias_uuids(text: &str) -> String {
    let mut seen: Vec<String> = Vec::new();
    let mut out = String::new();
    let mut rest = text;
    while let Some(uuid) = first_uuid(rest) {
        let at = rest.find(uuid).unwrap_or(0);
        let n = match seen.iter().position(|s| s == uuid) {
            Some(i) => i + 1,
            None => {
                seen.push(uuid.to_owned());
                seen.len()
            }
        };
        out.push_str(&rest[..at]);
        out.push_str(&format!("@{n}"));
        rest = &rest[at + uuid.len()..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BD9: the committed self-test recording holds no absolute path, no raw UUID in its steps and no full transcript.
    #[test]
    fn the_committed_self_test_recording_is_clean() {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                if e.path().is_dir() {
                    walk(&e.path(), out);
                } else {
                    out.push(e.path());
                }
            }
        }
        let dir = recordings_dir(&crate::workspace_root()).join("selftest");
        let mut files = Vec::new();
        walk(&dir, &mut files);
        assert!(files.len() >= 10, "{files:?}");
        for f in &files {
            let text = std::fs::read_to_string(f).unwrap_or_default();
            assert_eq!(absolute_path_in(&text), None, "{}", f.display());
            for word in ["\"host\"", "\"user\"", "\"time\"", "\"timestamp\""] {
                assert!(!text.contains(word), "{}: {word}", f.display());
            }
        }
        assert!(
            !dir.join("out/project-t1.pdf.sha256").exists(),
            "the PDF digest is not reproducible: the self-test recording has none"
        );
        assert!(
            !dir.join("transcript.jsonl").exists(),
            "the full transcript is not committed"
        );
        for name in ["steps.uiscript", "prelude.uiscript"] {
            assert_eq!(
                first_uuid(&std::fs::read_to_string(dir.join(name)).unwrap()),
                None,
                "{name}"
            );
        }
        let run: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
        assert_eq!(run["ci"], "pr");
        assert_eq!(run["stale"], Value::Null);
        assert_eq!(run["end"], "handback");
    }

    #[test]
    fn outcomes_keep_the_action_the_outcome_and_the_reason() {
        let lines = vec![
            "{\"step\":1,\"action\":\"file.new\",\"outcome\":\"done\",\"reason\":null,\"message\":null,\"state\":{}}".to_owned(),
            "{\"step\":2,\"action\":\"edit.redo\",\"outcome\":\"refused\",\"reason\":\"reason-nothing-to-redo\",\"message\":null,\"state\":{}}".to_owned(),
        ];
        let text = outcomes_jsonl(&lines).unwrap();
        assert_eq!(
            text,
            "{\"action\":\"file.new\",\"outcome\":\"done\",\"reason\":null,\"step\":1}\n\
             {\"action\":\"edit.redo\",\"outcome\":\"refused\",\"reason\":\"reason-nothing-to-redo\",\"step\":2}\n"
        );
    }

    #[test]
    fn the_epilogue_text_collects_the_page_texts_of_its_range() {
        let steps = vec![
            ("help.about".to_owned(), json!({})),
            ("page.describe".to_owned(), json!({"folio": "101"})),
            ("page.describe".to_owned(), json!({"folio": "102"})),
            ("app.quit".to_owned(), json!({"discard": true})),
        ];
        let lines: Vec<String> = [
            json!({"step": 1, "action": "file.new", "outcome": "done"}),
            json!({"step": 2, "action": "help.about", "outcome": "done"}),
            json!({"step": 3, "action": "page.describe", "outcome": "done", "describe": "page \"101\" \"T\" reps 0\nsecond line"}),
            json!({"step": 4, "action": "page.describe", "outcome": "done", "describe": "page \"102\" \"U\" reps 1"}),
            json!({"step": 5, "action": "app.quit", "outcome": "done"}),
        ]
        .iter()
        .map(Value::to_string)
        .collect();
        let text = epilogue_text(&steps, &lines, 1, [2, 4]);
        assert_eq!(
            text,
            "== page.describe 101 ==\npage \"101\" \"T\" reps 0\nsecond line\n== page.describe 102 ==\npage \"102\" \"U\" reps 1\n"
        );
        // A range outside the steps adds nothing.
        assert_eq!(epilogue_text(&steps, &lines, 1, [9, 9]), "");
    }

    #[test]
    fn uuids_of_a_command_log_become_aliases_in_order_of_first_appearance() {
        let a = "0197a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b";
        let b = "0197a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2c";
        let text = format!("{{\"id\":\"{a}\"}}\n{{\"id\":\"{b}\",\"ref\":\"{a}\"}}\n");
        assert_eq!(
            alias_uuids(&text),
            "{\"id\":\"@1\"}\n{\"id\":\"@2\",\"ref\":\"@1\"}\n"
        );
        assert_eq!(alias_uuids("no ids here"), "no ids here");
    }

    #[test]
    fn a_replay_report_names_every_failure() {
        let r = Replayed {
            r1: vec!["step 3 (x): outcome was \"done\" and is now \"refused\"".to_owned()],
            uuid: Some((7, "0197a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b".to_owned())),
            ..Replayed::default()
        };
        assert!(!r.ok());
        let report = ReplayReport {
            first: r,
            second: None,
            twice_differs_at: Some(4),
        };
        let text = replay_text(&report);
        assert!(text.contains("FAIL R1 step 3"), "{text}");
        assert!(text.contains("transcript line 7: a product defect"), "{text}");
        assert!(text.contains("differ at transcript line 4"), "{text}");
        assert!(text.ends_with("replay FAILED\n"));
        let good = ReplayReport {
            first: Replayed::default(),
            second: Some(Replayed::default()),
            twice_differs_at: None,
        };
        assert!(replay_text(&good).ends_with("replay ok: R1 and R2 hold\n"));
    }
}
