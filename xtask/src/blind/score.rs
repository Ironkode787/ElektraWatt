// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Scoring: what `stop` writes into a run's `score.json`, and the task files the harness reads.
//!
//! `score.json` (`ew.blindscore/1`) holds the protocol's counts and observations of every run but a check run. A
//! brief run's score adds `brief` ([`BriefScore`]): each acceptance item decided on the saved project with
//! `check-brief`'s code (in `brief.rs`), the attempt (unassisted, with help, not completed) and the counts of errors,
//! undo and assists. The score never calls a model, and no map iteration order reaches the output (NFR-REL-07).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::run::{Counts, Run, RunJson, Turn, parse_script};

/// The `$schema` of `score.json`.
pub const SCORE_SCHEMA: &str = "ew.blindscore/1";

/// One step of the prelude: its action and its class (`prelude.toml`).
#[derive(Deserialize, Serialize, Clone, PartialEq, Eq, Debug)]
pub struct PreludeEntry {
    /// The action of the prelude step this entry describes (a check that it matches).
    pub action: String,
    /// `setup` or `drawing`.
    pub class: String,
    /// For `drawing`: the follow-up that removes the line.
    #[serde(default)]
    pub gap: Option<String>,
}

/// `prelude.toml`.
#[derive(Deserialize, Serialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct PreludeFile {
    /// One entry per prelude step, in order.
    #[serde(default)]
    pub step: Vec<PreludeEntry>,
}

/// The folder of a task.
pub fn task_dir(root: &Path, task: &str) -> PathBuf {
    root.join("tools/blind/tasks").join(task)
}

/// Reads and parses a TOML file of a task.
///
/// # Errors
///
/// A missing or malformed file.
pub fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text.replace("\r\n", "\n")).with_context(|| format!("parsing {}", path.display()))
}

// ---------------------------------------------------------------------------------------------------------------
// The score.

/// How many `do` turns a folio took.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct FolioUse {
    /// `do` turns that ended on the folio.
    pub turns: u32,
}

/// What the harness observed and never judges.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct Observations {
    /// Whether the project was clean after the agent's last step.
    pub clean_before_epilogue: Option<bool>,
    /// The product's exit code.
    pub product_exit: Option<i32>,
    /// Whether the PDFs of 1 and 4 threads are byte-identical.
    pub pdfs_identical: Option<bool>,
    /// The exit code of `ew check`.
    pub ew_check_exit: Option<i32>,
}

/// The protocol part of the score.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct Protocol {
    /// What the run used.
    pub counts: Counts,
    /// How the run ended.
    pub end: Option<String>,
    /// Whether the hand-back was filed.
    pub handed_back: bool,
    /// The folios of the run.
    pub folios: Vec<String>,
    /// The turn use per folio.
    pub turns_by_folio: BTreeMap<String, FolioUse>,
    /// The observations.
    pub observations: Observations,
}

/// `score.json`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Score {
    /// `ew.blindscore/1`.
    #[serde(rename = "$schema")]
    pub schema: String,
    /// The run.
    pub run: String,
    /// The task.
    pub task: String,
    /// The protocol.
    pub protocol: Protocol,
    /// A brief run's acceptance list decided at `stop` and its attempt; absent for other runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brief: Option<BriefScore>,
}

/// The decision of one acceptance item of a brief run, by the code of `check-brief` ([`super::brief::decide_items`]).
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct ItemDecision {
    /// `A1`, `A2`, …
    pub id: String,
    /// `hard` or `soft`.
    pub kind: String,
    /// Whether the run's project meets it.
    pub met: bool,
    /// `met`, `not met: <why>`, or for a soft item the build cannot meet `infeasible (<reason>); <why>`.
    pub decision: String,
}

/// The counts of an attempt: errors, undo and assists as counts, never times.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct AttemptCounts {
    /// `do` calls.
    pub turns: u32,
    /// Steps (a visual run: inputs) the agent sent.
    pub steps: u32,
    /// The agent's steps (a visual run: dispatched actions) the product refused.
    pub refused: u32,
    /// The agent's steps (a visual run: dispatched actions) that failed.
    pub failed: u32,
    /// `edit.undo` done.
    pub undo: u32,
    /// Hints given.
    pub hints: u32,
    /// Friction entries by severity (every severity, zero included).
    pub friction: BTreeMap<String, u32>,
}

/// A brief run's score.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct BriefScore {
    /// The task's ID in its plan (`T1`), when `task.toml` names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Every acceptance item in order.
    pub items: Vec<ItemDecision>,
    /// `unassisted` (every hard item met, no hint), `with help` (met after a hint) or `not completed` (a hard item
    /// unmet, or the run ended early: a cap, a quit rule, the orchestrator, a lost session, a divergence).
    pub attempt: String,
    /// The counts.
    pub counts: AttemptCounts,
}

/// The attempt of a brief run: `not completed` when it ended early or a hard item is unmet, else `with help` after a
/// hint and `unassisted` without one.
pub fn attempt_of(end: Option<&str>, items: &[ItemDecision], hints: u32) -> &'static str {
    let early = matches!(end, Some("cap" | "quit" | "orchestrator" | "lost" | "diverged"));
    if early || items.iter().any(|i| i.kind == "hard" && !i.met) {
        "not completed"
    } else if hints > 0 {
        "with help"
    } else {
        "unassisted"
    }
}

/// The brief part of a stopped brief run's score: its acceptance list decided on `work/project` with `ew`, the
/// attempt and the counts. `ew` runs the product's `ew` in `work`.
///
/// # Errors
///
/// A task file or a file of the run that cannot be read.
pub fn brief_score(root: &Path, run: &Run, ev: &Evidence, ew: &mut super::brief::EwFn) -> Result<BriefScore> {
    let rj = &ev.rj;
    let dir = task_dir(root, &rj.task);
    let acceptance: super::brief::Acceptance = read_toml(&dir.join("acceptance.toml"))?;
    let task_id = super::brief::task_toml(root, &rj.task)?.and_then(|t| t.task_id);
    // The agent's part of the transcript: after the prelude, before the epilogue.
    let prelude = super::run::read_script(&run.path("prelude.uiscript"))?.len();
    let transcript = run.transcript()?;
    let epilogue = ev
        .turns
        .iter()
        .rev()
        .find(|t| t.kind == "epilogue")
        .and_then(|t| t.steps)
        .map(|[a, _]| usize::try_from(a).unwrap_or(usize::MAX).saturating_sub(1));
    let end = epilogue.map_or(transcript.len(), |n| (prelude + n).min(transcript.len()));
    let lines: Vec<&str> = transcript[..end].iter().map(String::as_str).collect();
    let outcomes = super::brief::agent_outcomes(&lines, prelude);
    let done = super::brief::done_actions(&outcomes);
    let decisions = super::brief::decide_items(&acceptance, &ev.work, &done, ew)?;
    let items: Vec<ItemDecision> = acceptance
        .item
        .iter()
        .zip(decisions)
        .map(|(it, d)| ItemDecision {
            id: it.id.clone(),
            kind: it.kind.clone(),
            met: d.is_ok(),
            decision: match (&d, &it.infeasible) {
                (Ok(()), _) => "met".to_owned(),
                (Err(why), Some(inf)) if it.kind == "soft" => format!("infeasible ({inf}); {why}"),
                (Err(why), _) => format!("not met: {why}"),
            },
        })
        .collect();
    let count = |outcome: &str| {
        u32::try_from(outcomes.iter().filter(|(_, o)| o == outcome).count()).unwrap_or(u32::MAX)
    };
    let hints = u32::try_from(ev.turns.iter().filter(|t| t.kind == "hint").count()).unwrap_or(u32::MAX);
    let mut friction: BTreeMap<String, u32> = super::friction::SEVERITIES
        .iter()
        .map(|s| ((*s).to_owned(), 0))
        .collect();
    let log = std::fs::read_to_string(run.path("friction.jsonl")).unwrap_or_default();
    for line in log.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        if let Ok(e) = serde_json::from_str::<super::friction::Entry>(line) {
            *friction.entry(e.severity).or_default() += 1;
        }
    }
    let counts = AttemptCounts {
        turns: rj.counts.turns,
        steps: rj.counts.steps,
        refused: count("refused"),
        failed: count("failed"),
        undo: u32::try_from(done.iter().filter(|a| *a == "edit.undo").count()).unwrap_or(u32::MAX),
        hints,
        friction,
    };
    Ok(BriefScore {
        task_id,
        attempt: attempt_of(rj.end.as_deref(), &items, hints).to_owned(),
        items,
        counts,
    })
}

/// What a run holds that the score reads.
#[derive(Clone, Debug)]
pub struct Evidence {
    /// The run's `run.json`.
    pub rj: RunJson,
    /// The turns of `turns.jsonl`.
    pub turns: Vec<Turn>,
    /// The run's `work/` folder (the agent's exports are files in it).
    pub work: PathBuf,
    /// The PDFs of 1 and 4 threads were identical.
    pub pdfs_identical: Option<bool>,
    /// The exit code of `ew check`.
    pub ew_check_exit: Option<i32>,
}

/// The protocol part of the score.
pub fn protocol_of(ev: &Evidence) -> Protocol {
    let rj = &ev.rj;
    let mut p = Protocol {
        counts: rj.counts,
        end: rj.end.clone(),
        handed_back: rj.handed_back,
        folios: rj.folios.clone(),
        observations: Observations {
            clean_before_epilogue: rj.clean_before_epilogue,
            product_exit: rj.child_exit,
            pdfs_identical: ev.pdfs_identical,
            ew_check_exit: ev.ew_check_exit,
        },
        ..Protocol::default()
    };
    for t in ev.turns.iter().filter(|t| t.kind == "do") {
        if let Some(f) = &t.folio {
            p.turns_by_folio.entry(f.clone()).or_default().turns += 1;
        }
    }
    p
}

/// Scores a run: its protocol (a brief run's acceptance list is added by [`score_run`]).
pub fn score(ev: &Evidence) -> Score {
    Score {
        schema: SCORE_SCHEMA.to_owned(),
        run: ev.rj.run.clone(),
        task: ev.rj.task.clone(),
        protocol: protocol_of(ev),
        brief: None,
    }
}

/// `score.json` as written: pretty, a final newline.
///
/// # Errors
///
/// Never in practice (the score is plain data).
pub fn score_json(score: &Score) -> Result<String> {
    let mut text = serde_json::to_string_pretty(score)?;
    text.push('\n');
    Ok(text)
}

/// The score as a Markdown section of `out/report.md`.
pub fn score_markdown(score: &Score) -> String {
    let mut md = String::new();
    let _ = writeln!(md, "## Score\n");
    if let Some(b) = &score.brief {
        let _ = writeln!(md, "Attempt: **{}**\n", b.attempt);
        let _ = writeln!(md, "| ID | Kind | Decision |");
        let _ = writeln!(md, "|---|---|---|");
        for i in &b.items {
            let _ = writeln!(md, "| {} | {} | {} |", i.id, i.kind, i.decision.replace('|', "/"));
        }
        let c = &b.counts;
        let friction: Vec<String> = c.friction.iter().map(|(s, n)| format!("{s} {n}")).collect();
        let _ = writeln!(
            md,
            "\nturns {}, steps {}, refused {}, failed {}, undo {}, hints {}; friction {}\n",
            c.turns,
            c.steps,
            c.refused,
            c.failed,
            c.undo,
            c.hints,
            friction.join(", ")
        );
        return md;
    }
    let _ = writeln!(
        md,
        "A free run holds no oracle: the protocol counts below are its score.\n"
    );
    md
}

// ---------------------------------------------------------------------------------------------------------------
// Reading a run.

/// What `stop` knows about the exports, for the observations.
#[derive(Clone, Copy, Default, Debug)]
pub struct ExportFacts {
    /// The PDFs of 1 and 4 threads are byte-identical.
    pub pdfs_identical: Option<bool>,
    /// The exit code of `ew check`.
    pub ew_check_exit: Option<i32>,
}

/// The evidence of a stopped run: its files.
///
/// # Errors
///
/// A file of the run that cannot be read.
pub fn evidence_of(run: &Run, facts: ExportFacts) -> Result<Evidence> {
    Ok(Evidence {
        rj: run.load()?,
        turns: run.turns()?,
        work: run.path("work"),
        pdfs_identical: facts.pdfs_identical,
        ew_check_exit: facts.ew_check_exit,
    })
}

/// Scores the stopped run `run` and writes `out/score.json`; returns the score.
///
/// A brief run holds its protocol and its acceptance list ([`brief_score`]); every other run holds only its protocol.
///
/// # Errors
///
/// A missing or malformed task file, or a file of the run that cannot be read or written.
pub fn score_run(root: &Path, run: &Run, facts: ExportFacts) -> Result<Score> {
    let ev = evidence_of(run, facts)?;
    let mut score = score(&ev);
    // A brief run's acceptance list, decided with `check-brief`'s code on the saved project. An `ew` that does
    // not start decides nothing: its queries fail.
    if ev.rj.kind == "brief" {
        let ew = run.product_exe("ew");
        let work = run.path("work");
        score.brief = Some(brief_score(root, run, &ev, &mut |args: &[&str]| {
            Ok(super::session::run_tool(run, &ew, args, &work)
                .map_or((None, String::new()), |(code, out, _)| (code, out)))
        })?);
    }
    std::fs::create_dir_all(run.path("out"))?;
    super::run::write_atomic(&run.path("out/score.json"), &score_json(&score)?)?;
    Ok(score)
}

#[cfg(test)]
mod tests;

/// The steps of a prelude text, for the task folder's checks of `check-brief`.
///
/// # Errors
///
/// A line that is not a step.
pub fn prelude_actions(text: &str) -> Result<Vec<String>> {
    Ok(parse_script(text)?.into_iter().map(|(a, _)| a).collect())
}
