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
use super::score::{Score, score_json, score_markdown, score_run};

/// The folder of the committed recordings.
pub fn recordings_dir(root: &Path) -> PathBuf {
    root.join("tools/blind/recordings")
}

/// `outcomes.jsonl` from the lines of a transcript (after its header): `step`, `action`, `outcome`, `reason` per step;
/// in a visual run `step`, `input` and `dispatched` per input and `frame` and `dispatched` per frame (visual spec
/// §5.3), with `closed` once the application has closed.
pub fn outcomes_jsonl(transcript: &[String]) -> Result<String> {
    let mut out = String::new();
    for line in transcript {
        let v: Value =
            serde_json::from_str(line).with_context(|| format!("not a transcript line: {line}"))?;
        let mut o = if v.get("input").is_some() {
            json!({"step": v["step"], "input": v["input"], "dispatched": v["dispatched"]})
        } else if v.get("frame").is_some() {
            json!({"frame": v["frame"], "dispatched": v["dispatched"]})
        } else {
            json!({
                "step": v["step"],
                "action": v["action"],
                "outcome": v["outcome"],
                "reason": v["reason"],
            })
        };
        if let Some(closed) = v.get("closed") {
            o["closed"] = closed.clone();
        }
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
    /// Whether the run is a visual run (its script has input and frame lines).
    pub visual: bool,
    /// The recorded `frames.sha256`: `(file, sha256)` of every painted frame, for R4.
    pub frames: Vec<(String, String)>,
    /// `replay --frames <dir>`: where a visual replay paints its frames (none without).
    pub paint_into: Option<PathBuf>,
}

/// `frames.sha256` from the lines of a transcript: `<sha256>  <file>` per painted frame line, in order.
pub fn frames_sha256(transcript: &[String]) -> String {
    let mut out = String::new();
    for line in transcript {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let (Some(file), Some(sha)) = (v["frame"].as_str(), v["sha256"].as_str()) {
            out.push_str(&format!("{sha}  {file}\n"));
        }
    }
    out
}

/// The `(file, sha256)` pairs of a `frames.sha256` text.
fn parse_frames_sha256(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| l.split_once("  "))
        .map(|(sha, file)| (file.trim().to_owned(), sha.trim().to_owned()))
        .collect()
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
    let visual = rj.visual();
    // A recording's frames are its `frames.sha256`; a stopped run's the hashes its transcript recorded.
    let mut frames = std::fs::read_to_string(dir.join("frames.sha256"))
        .map(|t| parse_frames_sha256(&t))
        .unwrap_or_default();
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
        if frames.is_empty() {
            frames = parse_frames_sha256(&frames_sha256(&lines));
        }
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
        visual,
        frames,
        paint_into: None,
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
    /// R4 (visual spec §5.3, reported): the number of painted frames whose SHA-256 differs from the recording's, when the
    /// replay painted frames.
    pub r4: Option<usize>,
    /// The product's standard output as written, its header line first (for `rescore`).
    pub raw: Vec<String>,
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
    let result = replay_in(root, exe, inputs, &dir);
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Runs the combined script of `inputs` through `elektrawatt` in `dir` (its `work` folder the product's) and compares;
/// the folder is kept.
///
/// # Errors
///
/// The product does not start.
pub fn replay_in(root: &Path, exe: &Path, inputs: &Inputs, dir: &Path) -> Result<Replayed> {
    let dir = dir.to_path_buf();
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
    let mut command = Command::new(exe);
    command
        .args(["--headless", "--script"])
        .arg(dir.join("replay.uiscript"))
        .args(["--keep-going", "--config-dir"])
        .arg(dir.join("config"));
    // A visual recording replays through the visual driver: without frames (laid out, never painted) unless asked.
    if inputs.visual {
        command.arg("--visual");
        match &inputs.paint_into {
            Some(frames) => command.arg("--frames").arg(frames),
            None => command.arg("--no-frames"),
        };
    }
    let out = command
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
    let mut result = Replayed {
        raw: raw.iter().map(|l| (*l).to_owned()).collect(),
        ..Replayed::default()
    };
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
        // R1, and R1v for a visual input or frame line: the inputs' dispatched actions and the frame names.
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
    if inputs.paint_into.is_some() {
        let painted = frames_sha256(&result.lines);
        let recorded: std::collections::BTreeMap<&str, &str> = inputs
            .frames
            .iter()
            .map(|(f, s)| (f.as_str(), s.as_str()))
            .collect();
        let now = parse_frames_sha256(&painted);
        let differing = now
            .iter()
            .filter(|(f, s)| recorded.get(f.as_str()) != Some(&s.as_str()))
            .count()
            + recorded.len().saturating_sub(now.len());
        result.r4 = Some(differing);
    }
    if let Some(own) = &inputs.transcript {
        let differing = own.iter().zip(&result.lines).filter(|(a, b)| a != b).count()
            + own.len().abs_diff(result.lines.len());
        result.r3 = Some(differing);
    }
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
    if let Some(n) = report.first.r4 {
        out.push_str(&format!(
            "R4 {n} frame(s) differ from the recording's frames.sha256 (a report, not a failure)\n"
        ));
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
    // The task's committed prelude, normalised like the run's own prelude, so that the two compare like for like.
    let task_prelude =
        std::fs::read_to_string(root.join(format!("tools/blind/tasks/{}/prelude.uiscript", rj.task)))
            .ok()
            .map(|t| norm.text(&t.replace("\r\n", "\n")));
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
        "friction.jsonl",
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
    // A visual run keeps its frames' hashes, never the PNGs (visual spec §5.3).
    if rj.visual() {
        files.push(("frames.sha256".to_owned(), frames_sha256(&run.transcript()?)));
    }
    files.push(("epilogue.txt".to_owned(), norm.text(&inputs.epilogue)));
    // The PDF's digest ties a gate run's recording to the PDF attached to the release (FT-420). It is evidence only,
    // never compared, and not reproducible: a new project mints random IDs that reach the PDF. The self-test
    // recording therefore leaves it out, or every bless would change a committed file.
    if !rj.task.starts_with("selftest")
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
        if let Some(msg) = raw_uuid_refusal(rel, text, task_prelude.as_deref()) {
            bail!("{msg}");
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

/// `rescore <recording> [--bin <dir>] [--write]` (FT-555): scores a committed recording again against its task's
/// current oracle through the harness's own scoring path. The recording is replayed into a scratch run folder (R1 and
/// R2 must hold), the run's exports are made with the built `ew` as `stop` makes them, and [`score_run`] scores the
/// folder. With `write` the recording's `out/score.json` and the score section of its `out/report.md` are replaced.
/// Returns the new score and the recorded `score.json` it replaces.
///
/// # Errors
///
/// A recording that does not load, a replay that fails R1 or R2, a binary that is not built.
pub fn rescore(root: &Path, bin: &Path, name: &str, write: bool) -> Result<(Score, String)> {
    let rec = recordings_dir(root).join(name);
    let inputs = load_inputs(&rec)?;
    if let Some(stale) = &inputs.stale {
        bail!("recording `{name}` is stale ({stale}): it is not re-scored");
    }
    let exe = |n: &str| bin.join(format!("{n}{}", std::env::consts::EXE_SUFFIX));
    for n in ["elektrawatt", "ew"] {
        if !exe(n).is_file() {
            bail!(
                "{} is not built (cargo build -p elektrawatt -p ew)",
                exe(n).display()
            );
        }
    }
    let dir = std::env::temp_dir().join("ew-blind-rescore").join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    let result = (|| -> Result<(Score, String)> {
        std::fs::create_dir_all(dir.join(".harness/bin"))?;
        std::fs::create_dir_all(dir.join(".harness/empty-sample"))?;
        for rel in ["run.json", "prelude.uiscript", "steps.uiscript", "turns.jsonl"] {
            std::fs::copy(rec.join(rel), dir.join(rel)).with_context(|| format!("copying {rel}"))?;
        }
        std::fs::copy(
            exe("ew"),
            dir.join(format!(".harness/bin/ew{}", std::env::consts::EXE_SUFFIX)),
        )?;
        super::run::copy_dir(&root.join("content"), &dir.join(".harness/content"))?;
        let replayed = replay_in(root, &exe("elektrawatt"), &inputs, &dir)?;
        if !replayed.ok() {
            bail!("the replay of `{name}` fails: {}", replayed.failures().join("; "));
        }
        let mut transcript = replayed.raw.join("\n");
        transcript.push('\n');
        std::fs::write(dir.join("transcript.jsonl"), transcript)?;
        let run = Run { dir: dir.clone() };
        std::fs::create_dir_all(run.path("out"))?;
        let facts = super::stop::exports_of(&run)?;
        let score = score_run(root, &run, facts)?;
        let old = std::fs::read_to_string(rec.join("out/score.json")).unwrap_or_default();
        if write {
            let norm = Normaliser::new(&dir);
            let json = norm.text(&score_json(&score)?);
            if let Some(path) = absolute_path_in(&json) {
                bail!("the new score holds an absolute path ({path})");
            }
            std::fs::write(rec.join("out/score.json"), json)?;
            let report_path = rec.join("out/report.md");
            let report = std::fs::read_to_string(&report_path)
                .context("reading out/report.md")?
                .replace("\r\n", "\n");
            std::fs::write(&report_path, with_score_section(&report, &score_markdown(&score)))?;
        }
        Ok((score, old))
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// `report` with its `## Score` section (up to the next `## ` heading) replaced by `section`; a report without one gets
/// it after its first paragraph.
pub fn with_score_section(report: &str, section: &str) -> String {
    let Some(start) = report.find("## Score\n") else {
        return match report.find("\n\n") {
            Some(i) => format!("{}{section}{}", &report[..i + 2], &report[i + 2..]),
            None => format!("{report}\n{section}"),
        };
    };
    let end = report[start + 1..]
        .find("\n## ")
        .map_or(report.len(), |i| start + 1 + i + 1);
    format!("{}{section}{}", &report[..start], &report[end..])
}

/// The refusal for a file that holds a raw UUID where none may be, or `None`. The agent's `steps.uiscript` holds none. The
/// `prelude.uiscript` may hold the fixed IDs of the golden part copies (its `library.embed` line), but only when it is the
/// task's committed prelude: `start` copies that file verbatim, so the run's prelude equals it.
fn raw_uuid_refusal(rel: &str, text: &str, task_prelude: Option<&str>) -> Option<String> {
    first_uuid(text)?;
    match rel {
        "steps.uiscript" => Some(format!("{rel} holds a raw UUID: the agent's steps carry none")),
        "prelude.uiscript" if task_prelude == Some(text) => None,
        "prelude.uiscript" => Some(format!(
            "{rel} holds a raw UUID and is not the task's committed prelude"
        )),
        _ => None,
    }
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
    fn a_uuid_in_the_agents_steps_is_refused() {
        let text = "{\"id\":\"0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b\"}\n";
        let msg = raw_uuid_refusal("steps.uiscript", text, None);
        assert!(
            msg.as_deref()
                .is_some_and(|m| m.contains("the agent's steps carry none")),
            "{msg:?}"
        );
    }

    #[test]
    fn the_tasks_own_prelude_may_hold_ids() {
        let prelude = "library.embed \"0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b\"\n";
        assert_eq!(raw_uuid_refusal("prelude.uiscript", prelude, Some(prelude)), None);
    }

    #[test]
    fn a_changed_prelude_with_ids_is_refused() {
        let prelude = "library.embed \"0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b\"\n";
        let other = "library.embed \"0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5c\"\n";
        for task in [Some(other), None] {
            let msg = raw_uuid_refusal("prelude.uiscript", prelude, task);
            assert!(
                msg.as_deref()
                    .is_some_and(|m| m.contains("not the task's committed prelude")),
                "{msg:?}"
            );
        }
    }

    #[test]
    fn files_without_ids_pass() {
        let plain = "step \"open\"\n";
        assert_eq!(raw_uuid_refusal("steps.uiscript", plain, None), None);
        assert_eq!(raw_uuid_refusal("prelude.uiscript", plain, None), None);
        let id = "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b";
        assert_eq!(
            raw_uuid_refusal("out/report.md", &format!("id {id}\n"), None),
            None
        );
    }

    /// FT-555: `rescore --write` replaces only the report's score section.
    #[test]
    fn the_score_section_of_a_report_is_replaced_in_place() {
        let report = "# Run r\n\nkind gate\n\n## Score\n\nold\n\n## Protocol\n\nturns 1\n";
        assert_eq!(
            with_score_section(report, "## Score\n\nnew\n\n"),
            "# Run r\n\nkind gate\n\n## Score\n\nnew\n\n## Protocol\n\nturns 1\n"
        );
        // The last section, and a report without one.
        assert_eq!(
            with_score_section("# Run r\n\n## Score\n\nold\n", "## Score\n\nnew\n"),
            "# Run r\n\n## Score\n\nnew\n"
        );
        assert_eq!(
            with_score_section("# Run r\n\n## Protocol\n", "## Score\n\nnew\n\n"),
            "# Run r\n\n## Score\n\nnew\n\n## Protocol\n"
        );
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

    /// BV10: the committed visual self-test recording holds the frames' hashes and never a PNG, no absolute path, no
    /// PDF digest and no full transcript; it is a `pr` recording of a visual run with a friction log.
    #[test]
    fn the_committed_visual_self_test_recording_is_clean() {
        let dir = recordings_dir(&crate::workspace_root()).join("selftest-visual");
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            assert!(!name.ends_with(".png") && name != "frames", "{name}");
            if e.path().is_file() {
                let text = std::fs::read_to_string(e.path()).unwrap();
                assert_eq!(absolute_path_in(&text), None, "{name}");
            }
        }
        assert!(!dir.join("out/project-t1.pdf.sha256").exists());
        assert!(!dir.join("transcript.jsonl").exists());
        let sums = std::fs::read_to_string(dir.join("frames.sha256")).unwrap();
        assert!(sums.lines().any(|l| l.ends_with("  t0000.png")), "{sums}");
        assert!(dir.join("friction.jsonl").is_file());
        let run: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("run.json")).unwrap()).unwrap();
        assert_eq!(
            (run["ci"].as_str(), run["senses"].as_str()),
            (Some("pr"), Some("visual"))
        );
        assert_eq!(run["end"], "handback");
    }

    /// BV5: a visual transcript keeps the input, its dispatch and the frame name in `outcomes.jsonl`, and the frames'
    /// hashes in `frames.sha256`; a frame that was not painted has no hash line.
    #[test]
    fn a_visual_transcript_keeps_inputs_dispatches_and_frame_hashes() {
        let a = "ab".repeat(32);
        let lines = vec![
            format!(r#"{{"frame":"t0000.png","sha256":"{a}","dispatched":[]}}"#),
            r#"{"step":1,"input":{"input":"key","chord":"ctrl+z"},"dispatched":[{"action":"edit.undo","outcome":"done","reason":null}],"state":{"dirty":true}}"#.to_owned(),
            r#"{"frame":"t0001.png","sha256":null,"dispatched":[]}"#.to_owned(),
            r#"{"step":2,"input":{"input":"key","chord":"alt+f4"},"dispatched":[],"closed":true}"#.to_owned(),
        ];
        assert_eq!(
            outcomes_jsonl(&lines).unwrap(),
            "{\"dispatched\":[],\"frame\":\"t0000.png\"}\n\
             {\"dispatched\":[{\"action\":\"edit.undo\",\"outcome\":\"done\",\"reason\":null}],\"input\":{\"chord\":\"ctrl+z\",\"input\":\"key\"},\"step\":1}\n\
             {\"dispatched\":[],\"frame\":\"t0001.png\"}\n\
             {\"closed\":true,\"dispatched\":[],\"input\":{\"chord\":\"alt+f4\",\"input\":\"key\"},\"step\":2}\n"
        );
        let sums = frames_sha256(&lines);
        assert_eq!(sums, format!("{a}  t0000.png\n"));
        assert_eq!(parse_frames_sha256(&sums), vec![("t0000.png".to_owned(), a)]);
    }
}
