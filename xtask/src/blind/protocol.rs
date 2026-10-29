// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The protocol (WP-S6.13 §5.2, D5): a turn is one `do` call followed by exactly one note; the harness forwards the
//! steps in order and stops at the first step that is not done; `look` calls are free within a cap; a step that fails
//! twice in a row is marked broken; the quit rules end a run on low scores; caps are counts, never wall time.
//!
//! Every function here works on a run folder through a [`Backend`] and returns the text the command prints. The state of
//! the protocol is derived from `turns.jsonl` at each call, so the harness process itself keeps none.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use anyhow::Result;
use serde_json::Value;

use super::filter::{Reference, StepIn, check_step};
use super::run::{Kind, NotDone, Run, RunJson, Turn, append, canonical, step_line, write_atomic};
use super::session::{Backend, Diverged, Lost, Sent, Step};
use super::stop::CHECKER_HEADINGS;

/// The headings of the fixed hand-back, in order (§5.6).
pub const HANDBACK_HEADINGS: [&str; 7] = [
    "1. Keep or drop",
    "2. Where it lost me",
    "3. What the tool is",
    "4. Broken",
    "5. The drawing set against the design language",
    "6. Flow chart",
    "7. Good",
];

/// The longest a note's free text may be (characters, after the two scores).
pub const NOTE_MAX: usize = 1200;

/// The most `look` calls between two `do` calls.
pub const LOOKS_PER_TURN: u32 = 3;

/// What the protocol knows after the turns so far.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct State {
    /// `do` calls so far.
    pub turns: u32,
    /// The turn that still needs its note.
    pub pending_note: Option<u32>,
    /// `look` calls since the last `do`.
    pub looks_since_do: u32,
    /// The first step that was not done in the last `do`.
    pub prev_not_done: Option<NotDone>,
    /// The keys of the steps marked broken.
    pub broken: BTreeSet<String>,
    /// How many of the last notes in a row had a patience of 2 or less.
    pub low_patience: u32,
    /// How many of the last notes in a row had a flow of 2 or less.
    pub low_flow: u32,
}

/// Derives the state from the turn lines.
pub fn derive_state(turns: &[Turn]) -> State {
    let mut st = State::default();
    for t in turns {
        match t.kind.as_str() {
            "do" => {
                st.turns = t.turn.unwrap_or(st.turns + 1);
                st.pending_note = Some(st.turns);
                st.looks_since_do = 0;
                st.prev_not_done.clone_from(&t.first_not_done);
                if let Some(b) = &t.broken {
                    st.broken.insert(b.step_key());
                }
            }
            "look" => st.looks_since_do += 1,
            "note" => {
                st.pending_note = None;
                st.low_patience = if t.patience.is_some_and(|p| p <= 2) {
                    st.low_patience + 1
                } else {
                    0
                };
                st.low_flow = if t.flow.is_some_and(|f| f <= 2) {
                    st.low_flow + 1
                } else {
                    0
                };
            }
            _ => {}
        }
    }
    st
}

fn state_path(run: &Run) -> std::path::PathBuf {
    run.path(".harness/last_state.json")
}

fn last_state(run: &Run) -> Value {
    std::fs::read_to_string(state_path(run))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null)
}

/// One `do` step's line (§5.2.3): `<n> <action> <outcome>[ <reason>[: <sentence>]][ "<message>"]`, then (G-PP triage
/// row 10) `  preview: <card summary>; Apply or Cancel` when the step leaves a preview card open.
fn step_text(n: usize, sent: &Sent) -> String {
    let action = sent.value["action"].as_str().unwrap_or("?");
    let mut line = format!("{n} {action} {}", sent.outcome());
    if let Some(r) = sent.value["reason"].as_str() {
        let _ = write!(line, " {r}");
        if let Some(t) = sent.value["reason_text"].as_str() {
            let _ = write!(line, ": {t}");
        }
    }
    if let Some(m) = sent.value["message"].as_str() {
        let _ = write!(line, " {}", Value::from(m));
    }
    if let Some(p) = sent.value["pending"].as_str() {
        let _ = write!(line, "\n  preview: {p}; Apply or Cancel");
    }
    line
}

/// The lines of the members of `state` (and of `state.canvas`) that differ from `before`, compact JSON.
fn state_diff(before: &Value, after: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let Some(a) = after.as_object() else { return out };
    for (k, v) in a {
        let was = before.get(k);
        if was == Some(v) {
            continue;
        }
        if k == "canvas"
            && let (Some(Value::Object(old)), Value::Object(new)) = (was, v)
        {
            for (ck, cv) in new {
                if old.get(ck) != Some(cv) {
                    out.push(format!("  canvas.{ck}: {cv}"));
                }
            }
            continue;
        }
        out.push(format!("  {k}: {v}"));
    }
    out
}

/// What the end of a run is called in the message of a refused call.
fn ended(end: &str) -> String {
    format!("harness: the run has ended ({end})\n")
}

fn lost_message(run: &Run) -> String {
    let id = run.load().map_or_else(|_| String::new(), |j| j.run);
    format!("harness: session lost, run {} resume {id}\n", run.blind_command())
}

/// Marks the run lost in `run.json` (a keeper that does not answer).
fn mark_lost(run: &Run) {
    let _ = run.update(|j| {
        if j.end.is_none() {
            j.end = Some("lost".to_owned());
        }
    });
}

/// Applies the caps to `run.json`: a counter that reached its cap ends the run.
fn apply_caps(j: &mut RunJson, force: bool) {
    let (c, k) = (j.counts, j.caps);
    if j.end.is_none()
        && (force
            || c.turns >= k.turns
            || c.steps >= k.steps
            || c.looks >= k.looks
            || c.refusals >= k.refusals)
    {
        j.end = Some("cap".to_owned());
    }
}

/// `do`: sends 1 to 20 steps in order, stopping at the first that is refused by the harness or not done.
///
/// # Errors
///
/// A failure of the run folder or of the product that is not a lost session.
pub fn do_steps(
    run: &Run,
    reference: &Reference,
    backend: &mut dyn Backend,
    steps: &[StepIn],
) -> Result<String> {
    let rj = run.load()?;
    if let Some(end) = &rj.end {
        return Ok(ended(end));
    }
    let st = derive_state(&run.turns()?);
    if let Some(n) = st.pending_note {
        return Ok(format!("harness: note required for turn {n}\n"));
    }
    let kind = Kind::parse(&rj.kind)?;
    let turn_no = st.turns + 1;
    let before = run.steps()?.len();
    let mut out = String::new();
    let mut sent_count = 0u32;
    let mut first_not_done: Option<NotDone> = None;
    let mut refusal: Option<String> = None;
    let mut cap_hit = false;
    let mut lost = false;
    let mut diverged: Option<Diverged> = None;
    let mut shown = last_state(run);
    let mut folio: Option<String> = None;
    for (i, s) in steps.iter().enumerate() {
        let number = before + i + 1;
        let stopped = first_not_done.is_some() || refusal.is_some() || cap_hit || lost || diverged.is_some();
        if stopped {
            let _ = writeln!(out, "{number} {} skipped", s.action);
            continue;
        }
        if let Err(text) = check_step(reference, kind, s, &st.broken) {
            let _ = writeln!(out, "{text}");
            refusal = Some(text);
            continue;
        }
        if rj.counts.steps + sent_count >= rj.caps.steps {
            cap_hit = true;
            let _ = writeln!(out, "{number} {} skipped", s.action);
            continue;
        }
        let step = Step {
            action: s.action.clone(),
            params: s.params_value(),
        };
        let sent = match backend.execute(&step) {
            Ok(sent) => sent,
            Err(e) => {
                if e.downcast_ref::<Lost>().is_some() {
                    lost = true;
                    let _ = writeln!(out, "{number} {} skipped", s.action);
                    continue;
                }
                if let Some(d) = e.downcast_ref::<Diverged>() {
                    diverged = Some(Diverged {
                        step: d.step,
                        what: d.what.clone(),
                    });
                    continue;
                }
                return Err(e);
            }
        };
        append(
            &run.path("steps.uiscript"),
            &format!("{}\n", step_line(&step.action, &step.params)),
        )?;
        append(&run.path("transcript.jsonl"), &format!("{}\n", sent.raw))?;
        sent_count += 1;
        let _ = writeln!(out, "{}", step_text(number, &sent));
        let state = sent.value.get("state").cloned().unwrap_or(Value::Null);
        for l in state_diff(&shown, &state) {
            let _ = writeln!(out, "{l}");
        }
        folio = state["page"].as_str().map(str::to_owned);
        shown = state;
        if sent.outcome() != "done" {
            first_not_done = Some(NotDone {
                action: step.action.clone(),
                params: canonical(&step.params),
                outcome: sent.outcome().to_owned(),
                reason: sent.reason(),
            });
        }
    }
    let ended_call = backend.end_call();
    std::fs::write(state_path(run), shown.to_string())?;
    if let Some(d) = diverged {
        let _ = run.update(|j| {
            j.end = Some("diverged".to_owned());
            j.diverged_at = Some(d.step);
        });
        let _ = writeln!(
            out,
            "harness: the replay differs at step {} ({}); the run is over",
            d.step, d.what
        );
        return Ok(out);
    }
    if lost && sent_count == 0 {
        // Nothing reached the product: no turn.
        mark_lost(run);
        out.push_str(&lost_message(run));
        return Ok(out);
    }
    ended_call?;
    // The broken rule: the same step failed the same way in this turn and in the one before.
    let broken = first_not_done
        .clone()
        .filter(|nd| st.prev_not_done.as_ref() == Some(nd));
    if let Some(b) = &broken {
        append(
            &run.path("drafter.md"),
            &format!(
                "broken: {} {} {} {}\n\n",
                b.action,
                b.params,
                b.outcome,
                b.reason.as_deref().unwrap_or("")
            ),
        )?;
    }
    let range = (sent_count > 0).then(|| {
        let first = u32::try_from(before + 1).unwrap_or(u32::MAX);
        [first, first + sent_count - 1]
    });
    run.push_turn(Turn {
        kind: "do".to_owned(),
        turn: Some(turn_no),
        steps: range,
        folio,
        refusal: refusal.clone(),
        first_not_done: first_not_done.clone(),
        broken: broken.clone(),
        ..Turn::default()
    })?;
    run.update(|j| {
        j.counts.turns += 1;
        j.counts.steps += sent_count;
        if refusal.is_some() {
            j.counts.refusals += 1;
        }
        match first_not_done.as_ref().map(|n| n.outcome.as_str()) {
            Some("refused") => j.counts.refused += 1,
            Some("failed") => j.counts.failed += 1,
            _ => {}
        }
        if broken.is_some() {
            j.counts.broken += 1;
        }
        apply_caps(j, cap_hit);
    })?;
    if lost {
        mark_lost(run);
        out.push_str(&lost_message(run));
    }
    Ok(out)
}

/// What `look` shows.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LookOpts {
    /// The sections to show (none: all but the menus).
    pub sections: Vec<String>,
    /// Only the lines added or removed since the previous look of the same sections.
    pub diff: bool,
    /// Include the menu lines.
    pub menus: bool,
    /// `--page [<folio>]`: the page description instead of the interface (`Some(None)`: the page on show).
    pub page: Option<Option<String>>,
}

/// The sections of a `ui.describe` text: a top-level line (no indent) with the indented lines under it, named by the
/// line's first token.
pub fn sections(text: &str) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        if line.starts_with(' ')
            && let Some(last) = out.last_mut()
        {
            last.1.push(line.to_owned());
        } else {
            let name = line.split(' ').next().unwrap_or("").to_owned();
            out.push((name, vec![line.to_owned()]));
        }
    }
    out
}

/// Whether the section `name` is asked for by `want` (`title:` and `title` name the same section).
fn wanted(name: &str, want: &str) -> bool {
    name == want || name.trim_end_matches(':') == want.trim_end_matches(':')
}

/// The lines added (`+`) and removed (`-`) between `old` and `new`, by a longest common subsequence.
pub fn line_diff(old: &[String], new: &[String]) -> Vec<String> {
    let (n, m) = (old.len(), new.len());
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if old[i] == new[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < n || j < m {
        if i < n && j < m && old[i] == new[j] {
            i += 1;
            j += 1;
        } else if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            out.push(format!("+ {}", new[j]));
            j += 1;
        } else {
            out.push(format!("- {}", old[i]));
            i += 1;
        }
    }
    out
}

/// `look`: sends `ui.describe` (or `page.describe` with `--page`) and prints the sections asked for.
///
/// # Errors
///
/// A failure of the run folder or of the product that is not a lost session.
pub fn look(run: &Run, backend: &mut dyn Backend, opts: &LookOpts) -> Result<String> {
    let rj = run.load()?;
    if let Some(end) = &rj.end {
        return Ok(ended(end));
    }
    let st = derive_state(&run.turns()?);
    let refuse = |why: &str| -> Result<String> {
        run.push_turn(Turn {
            kind: "look".to_owned(),
            refusal: Some(why.to_owned()),
            ..Turn::default()
        })?;
        run.update(|j| {
            j.counts.refusals += 1;
            apply_caps(j, false);
        })?;
        Ok(format!("{why}\n"))
    };
    if st.looks_since_do >= LOOKS_PER_TURN {
        return refuse("harness: look cap per turn");
    }
    let (action, params) = match &opts.page {
        Some(folio) => (
            "page.describe",
            folio
                .as_ref()
                .map_or_else(|| serde_json::json!({}), |f| serde_json::json!({ "folio": f })),
        ),
        None => ("ui.describe", serde_json::json!({})),
    };
    let step = Step {
        action: action.to_owned(),
        params,
    };
    let sent = match backend.execute(&step) {
        Ok(s) => s,
        Err(e) if e.downcast_ref::<Lost>().is_some() => {
            mark_lost(run);
            return Ok(lost_message(run));
        }
        Err(e) => return Err(e),
    };
    let before = run.steps()?.len();
    append(
        &run.path("steps.uiscript"),
        &format!("{}\n", step_line(&step.action, &step.params)),
    )?;
    append(&run.path("transcript.jsonl"), &format!("{}\n", sent.raw))?;
    backend.end_call()?;
    let state = sent.value.get("state").cloned().unwrap_or(Value::Null);
    std::fs::write(state_path(run), state.to_string())?;
    let n = u32::try_from(before + 1).unwrap_or(u32::MAX);
    run.push_turn(Turn {
        kind: "look".to_owned(),
        steps: Some([n, n]),
        folio: state["page"].as_str().map(str::to_owned),
        ..Turn::default()
    })?;
    run.update(|j| {
        j.counts.looks += 1;
        apply_caps(j, false);
    })?;
    let Some(text) = sent.describe().map(str::to_owned) else {
        return Ok(format!(
            "harness: {action} was {} {}\n",
            sent.outcome(),
            sent.reason().unwrap_or_default()
        ));
    };
    // The lines shown: a page text whole, the interface's sections as asked.
    let (key, shown): (String, Vec<String>) = if opts.page.is_some() {
        (
            format!("page-{}", opts.page.clone().flatten().unwrap_or_default()),
            text.lines().map(str::to_owned).collect(),
        )
    } else {
        let mut lines = Vec::new();
        for (name, body) in sections(&text) {
            let asked = if opts.sections.is_empty() {
                true
            } else {
                opts.sections.iter().any(|w| wanted(&name, w))
            };
            let is_menu = name == "menu";
            if asked && (opts.menus || !is_menu || opts.sections.iter().any(|w| wanted("menu", w))) {
                lines.extend(body);
            }
        }
        let key = if opts.sections.is_empty() {
            "all".to_owned()
        } else {
            format!(
                "sections-{}",
                opts.sections
                    .join("-")
                    .replace(|c: char| !c.is_ascii_alphanumeric() && c != '-', "_")
            )
        };
        (key, lines)
    };
    let store = run.path(&format!(
        ".harness/looks/{}{}.txt",
        key,
        if opts.menus { "-menus" } else { "" }
    ));
    let previous: Option<Vec<String>> = std::fs::read_to_string(&store)
        .ok()
        .map(|t| t.lines().map(str::to_owned).collect());
    std::fs::create_dir_all(run.path(".harness/looks"))?;
    write_atomic(&store, &format!("{}\n", shown.join("\n")))?;
    let mut out = String::new();
    match (opts.diff, previous) {
        (true, Some(old)) => {
            let d = line_diff(&old, &shown);
            if d.is_empty() {
                out.push_str("(no change since the last look)\n");
            }
            for l in d {
                let _ = writeln!(out, "{l}");
            }
        }
        _ => {
            for l in &shown {
                let _ = writeln!(out, "{l}");
            }
        }
    }
    Ok(out)
}

/// `note`: the live note of the last turn: `patience <0-10> flow <0-10>` and at most 1,200 characters of free text.
///
/// # Errors
///
/// A failure of the run folder.
pub fn note(run: &Run, text: &str) -> Result<String> {
    let st = derive_state(&run.turns()?);
    let Some(turn) = st.pending_note else {
        return Ok("harness: no turn waits for a note\n".to_owned());
    };
    let bad = "harness: a note starts with `patience <0-10> flow <0-10>`\n";
    let mut words = text.trim_start().splitn(5, char::is_whitespace);
    let (Some("patience"), Some(p), Some("flow"), Some(f)) =
        (words.next(), words.next(), words.next(), words.next())
    else {
        return Ok(bad.to_owned());
    };
    let score = |s: &str| s.parse::<u8>().ok().filter(|v| *v <= 10);
    let (Some(patience), Some(flow)) = (score(p), score(f)) else {
        return Ok(bad.to_owned());
    };
    let rest = words.next().unwrap_or("").trim();
    if rest.chars().count() > NOTE_MAX {
        return Ok(format!(
            "harness: a note has at most {NOTE_MAX} characters after the scores\n"
        ));
    }
    append(
        &run.path("drafter.md"),
        &format!("## Turn {turn}\npatience {patience} flow {flow}\n{rest}\n\n"),
    )?;
    run.push_turn(Turn {
        kind: "note".to_owned(),
        turn: Some(turn),
        patience: Some(patience),
        flow: Some(flow),
        ..Turn::default()
    })?;
    let st = derive_state(&run.turns()?);
    let mut out = format!("noted turn {turn}\n");
    let quit = if st.low_patience >= 3 {
        Some("patience")
    } else if st.low_flow >= 5 {
        Some("flow")
    } else {
        None
    };
    if let Some(which) = quit {
        let ended = run.update(|j| {
            if j.end.is_none() {
                j.end = Some("quit".to_owned());
            }
        })?;
        if ended.end.as_deref() == Some("quit") {
            let _ = writeln!(
                out,
                "harness: your {which} ran out; the run ends here: file your hand-back"
            );
        }
    }
    Ok(out)
}

/// `handback`: the fixed hand-back with its seven headings, accepted once; it ends a run that goes on.
///
/// # Errors
///
/// A failure of the run folder.
pub fn handback(run: &Run, text: &str) -> Result<String> {
    let rj = run.load()?;
    if rj.handed_back {
        return Ok("harness: the hand-back is filed\n".to_owned());
    }
    if let Some(end) = &rj.end
        && !matches!(end.as_str(), "quit" | "cap")
    {
        return Ok(ended(end));
    }
    // A check run files the checker's audit (five headings, `checker.md`); the others the drafter's hand-back.
    let check = rj.kind == "check";
    let headings: &[&str] = if check {
        &CHECKER_HEADINGS
    } else {
        &HANDBACK_HEADINGS
    };
    let file = if check { "checker.md" } else { "handback.md" };
    let mut from = 0usize;
    let lines: Vec<&str> = text.lines().collect();
    for h in headings {
        let at = lines
            .iter()
            .skip(from)
            .position(|l| l.trim_start_matches(['#', ' ', '*']).starts_with(h));
        match at {
            Some(i) => from += i + 1,
            None => {
                return Ok(format!(
                    "harness: the hand-back needs these headings in order; `{h}` is missing or out of order: {}\n",
                    headings.join(" | ")
                ));
            }
        }
    }
    let mut body = text.trim_end().to_owned();
    body.push('\n');
    write_atomic(&run.path(file), &body)?;
    run.push_turn(Turn {
        kind: "handback".to_owned(),
        ..Turn::default()
    })?;
    run.update(|j| {
        j.handed_back = true;
        if j.end.is_none() {
            j.end = Some("handback".to_owned());
        }
    })?;
    Ok("hand-back filed; the run is over\n".to_owned())
}

/// `report <run> check|bom|score`: a file the stop wrote.
///
/// # Errors
///
/// A failure of the run folder.
pub fn report(run: &Run, which: &str) -> Result<String> {
    // A check run reads the reports of the drafter run it audits, copied into its `reports/` by `start`; any other run
    // reads what its own `stop` wrote into `out/`.
    let dir = if run.load()?.kind == "check" {
        "reports"
    } else {
        "out"
    };
    let files: &[&str] = match which {
        "check" => &["check.txt"],
        "bom" => &["bom.csv"],
        "score" => &["score.json", "report.md"],
        other => return Ok(format!("harness: report check, bom or score, not `{other}`\n")),
    };
    let texts: Vec<String> = files
        .iter()
        .filter_map(|f| std::fs::read_to_string(run.path(&format!("{dir}/{f}"))).ok())
        .collect();
    if texts.is_empty() {
        return Ok(format!("harness: {dir}/{} does not exist yet\n", files[0]));
    }
    Ok(texts.join("\n"))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::blind::filter::test_reference;
    use crate::blind::run::turns_header;
    use crate::blind::session::{Product, Spawner};
    use crate::blind::testkit::{Tmp, new_run_json};

    /// G-PP triage row 10: a refusal prints its sentence beside its key; a pending card prints its summary on a line
    /// of its own; a line without the new members prints as before.
    #[test]
    fn a_step_prints_the_refusal_sentence_and_the_pending_card() {
        let refused = Sent::parse(
            r#"{"step":2,"action":"bridge.place","outcome":"refused","reason":"reason-bridge-selection","reason_text":"Select two terminals of one strip.","message":null,"pending":null,"state":{}}"#.to_owned(),
        )
        .unwrap();
        assert_eq!(
            step_text(2, &refused),
            "2 bridge.place refused reason-bridge-selection: Select two terminals of one strip."
        );
        let pending = Sent::parse(
            r#"{"step":3,"action":"props.set","outcome":"done","reason":null,"reason_text":null,"message":null,"pending":"4 devices · 4 changes · 4 tags, 0 cross-reference texts","state":{}}"#.to_owned(),
        )
        .unwrap();
        assert_eq!(
            step_text(3, &pending),
            "3 props.set done\n  preview: 4 devices · 4 changes · 4 tags, 0 cross-reference texts; Apply or Cancel"
        );
        let old = Sent::parse(
            r#"{"step":1,"action":"edit.redo","outcome":"refused","reason":"reason-nothing-to-redo","message":null,"state":{}}"#.to_owned(),
        )
        .unwrap();
        assert_eq!(step_text(1, &old), "1 edit.redo refused reason-nothing-to-redo");
    }

    /// A backend straight on a fake product.
    pub struct Direct {
        product: Box<dyn Product>,
        pub sent: Vec<String>,
    }

    impl Direct {
        pub fn new(spawner: &dyn Spawner, run: &Run) -> Direct {
            let (product, _) = spawner.spawn(run).unwrap();
            Direct {
                product,
                sent: Vec::new(),
            }
        }
    }

    impl Backend for Direct {
        fn execute(&mut self, step: &Step) -> Result<Sent> {
            self.sent.push(step.action.clone());
            Sent::parse(self.product.send(&step.line())?)
        }
        fn end_call(&mut self) -> Result<Option<i32>> {
            Ok(None)
        }
        fn finish(&mut self) -> Result<Option<i32>> {
            self.product.finish()
        }
    }

    pub struct Fixture {
        pub tmp: Tmp,
        pub run: Run,
        pub backend: Direct,
        pub reference: Reference,
    }

    pub fn fixture(kind: Kind) -> Fixture {
        let tmp = Tmp::new("protocol");
        let run = Run {
            dir: tmp.0.join("run"),
        };
        std::fs::create_dir_all(run.path(".harness")).unwrap();
        std::fs::write(run.path("turns.jsonl"), format!("{}\n", turns_header())).unwrap();
        std::fs::write(run.path("steps.uiscript"), "").unwrap();
        std::fs::write(
            run.path("prelude.uiscript"),
            format!("{}\n", crate::blind::run::script_header("p")),
        )
        .unwrap();
        std::fs::write(
            run.path("transcript.jsonl"),
            "{\"$schema\":\"ew.uitranscript/1\",\"version\":\"0\"}\n",
        )
        .unwrap();
        run.save(&new_run_json("r1", kind)).unwrap();
        let spawner = crate::blind::session::fake::FakeSpawner::default();
        let backend = Direct::new(&spawner, &run);
        Fixture {
            tmp,
            run,
            backend,
            reference: test_reference(),
        }
    }

    pub fn s(action: &str, params: Value) -> StepIn {
        StepIn {
            action: action.to_owned(),
            params: params.as_object().cloned().unwrap_or_default(),
        }
    }

    impl Fixture {
        pub fn run_do(&mut self, steps: &[StepIn]) -> String {
            do_steps(&self.run, &self.reference, &mut self.backend, steps).unwrap()
        }
        pub fn note(&self, text: &str) -> String {
            note(&self.run, text).unwrap()
        }
    }

    #[test]
    fn a_turn_needs_its_note_before_the_next_do() {
        let mut f = fixture(Kind::Free);
        let out = f.run_do(&[s("help.about", serde_json::json!({}))]);
        assert!(out.starts_with("1 help.about done"), "{out}");
        assert_eq!(
            f.run_do(&[s("help.about", serde_json::json!({}))]),
            "harness: note required for turn 1\n"
        );
        // Nothing was sent by the refused call.
        assert_eq!(f.backend.sent, ["help.about"]);
        assert_eq!(f.note("patience 9 flow 8 it opened the dialog"), "noted turn 1\n");
        let out = f.run_do(&[s("ui.dismiss", serde_json::json!({}))]);
        assert!(out.starts_with("2 ui.dismiss done"), "{out}");
        let drafter = std::fs::read_to_string(f.run.path("drafter.md")).unwrap();
        assert_eq!(drafter, "## Turn 1\npatience 9 flow 8\nit opened the dialog\n\n");
        let rj = f.run.load().unwrap();
        assert_eq!((rj.counts.turns, rj.counts.steps), (2, 2));
        // The steps and their transcript lines are in the run folder.
        assert_eq!(f.run.steps().unwrap().len(), 2);
        assert_eq!(f.run.transcript().unwrap().len(), 2);
    }

    #[test]
    fn a_note_has_a_fixed_form() {
        let mut f = fixture(Kind::Free);
        assert_eq!(
            f.note("patience 9 flow 8 x"),
            "harness: no turn waits for a note\n"
        );
        f.run_do(&[s("help.about", serde_json::json!({}))]);
        for bad in [
            "flow 3 patience 4 x",
            "patience 11 flow 3 x",
            "patience nine flow 3",
            "patience 5",
            "hello",
            "patience -1 flow 3",
        ] {
            assert!(f.note(bad).starts_with("harness: a note starts with"), "{bad}");
        }
        let long = format!("patience 5 flow 5 {}", "x".repeat(NOTE_MAX + 1));
        assert!(f.note(&long).contains("at most 1200 characters"));
        let ok = format!("patience 0 flow 10 {}", "y".repeat(NOTE_MAX));
        assert_eq!(f.note(&ok), "noted turn 1\n");
        assert_eq!(
            f.note("patience 5 flow 5 again"),
            "harness: no turn waits for a note\n"
        );
    }

    /// BD7: the harness stops at the first step that is not done and reports the rest `skipped`.
    #[test]
    fn a_batch_stops_at_the_first_step_that_is_not_done() {
        let mut f = fixture(Kind::Free);
        let out = f.run_do(&[
            s("help.about", serde_json::json!({})),
            s("edit.redo", serde_json::json!({})),
            s("ui.dismiss", serde_json::json!({})),
            s("edit.undo", serde_json::json!({})),
        ]);
        let lines: Vec<&str> = out.lines().filter(|l| !l.starts_with("  ")).collect();
        assert_eq!(
            lines,
            [
                "1 help.about done",
                "2 edit.redo refused reason-nothing-to-redo",
                "3 ui.dismiss skipped",
                "4 edit.undo skipped"
            ],
            "{out}"
        );
        assert_eq!(f.backend.sent, ["help.about", "edit.redo"]);
        assert_eq!(f.run.steps().unwrap().len(), 2);
        let rj = f.run.load().unwrap();
        assert_eq!((rj.counts.steps, rj.counts.refused), (2, 1));
        let t = f.run.turns().unwrap();
        assert_eq!(t[0].steps, Some([1, 2]));
        assert_eq!(t[0].first_not_done.as_ref().unwrap().action, "edit.redo");
    }

    /// BD6: a step the filter refuses is not sent; the turn ends there.
    #[test]
    fn a_harness_refusal_is_not_sent_and_counts() {
        let mut f = fixture(Kind::Free);
        let out = f.run_do(&[
            s("help.about", serde_json::json!({})),
            s("command.run", serde_json::json!({"cmd": "x"})),
            s("ui.dismiss", serde_json::json!({})),
        ]);
        assert!(
            out.contains("harness: command.run is not available to the drafter"),
            "{out}"
        );
        assert!(out.contains("3 ui.dismiss skipped"), "{out}");
        assert_eq!(f.backend.sent, ["help.about"]);
        let rj = f.run.load().unwrap();
        assert_eq!((rj.counts.refusals, rj.counts.turns, rj.counts.steps), (1, 1, 1));
        assert_eq!(
            f.run.turns().unwrap()[0].refusal.as_deref(),
            Some("harness: command.run is not available to the drafter")
        );
    }

    /// BD7: a step that fails twice in a row, the same way, is broken; the exact step is refused afterwards.
    #[test]
    fn the_same_failure_twice_in_a_row_marks_the_step_broken() {
        let mut f = fixture(Kind::Free);
        let fail = || s("file.open", serde_json::json!({"dir": "missing"}));
        let out = f.run_do(&[fail()]);
        assert!(out.contains("1 file.open failed \"It failed.\""), "{out}");
        f.note("patience 6 flow 6 hm");
        // A different step in between does not count: the failure must repeat in consecutive turns.
        f.run_do(&[fail()]);
        let drafter = std::fs::read_to_string(f.run.path("drafter.md")).unwrap();
        assert!(
            drafter.ends_with("broken: file.open {\"dir\":\"missing\"} failed It failed.\n\n"),
            "{drafter}"
        );
        assert_eq!(f.run.load().unwrap().counts.broken, 1);
        f.note("patience 5 flow 5 again");
        assert_eq!(f.run_do(&[fail()]), "harness: marked broken, move on\n");
        f.note("patience 5 flow 5 ok");
        // The same action with other parameters stays allowed; so does anything else.
        let out = f.run_do(&[s("file.open", serde_json::json!({"dir": "other"}))]);
        assert!(out.contains("file.open failed"), "{out}");
        f.note("patience 5 flow 5 other");
        // A failure that is not repeated in the next turn is not broken.
        let mut g = fixture(Kind::Free);
        g.run_do(&[fail()]);
        g.note("patience 5 flow 5 a");
        g.run_do(&[s("help.about", serde_json::json!({}))]);
        g.note("patience 5 flow 5 b");
        g.run_do(&[fail()]);
        assert_eq!(g.run.load().unwrap().counts.broken, 0);
    }

    /// BD7: three notes in a row with patience 2 or less, or five with flow 2 or less, end the run.
    #[test]
    fn the_quit_rules_end_the_run_at_the_third_and_the_fifth_low_note() {
        let mut f = fixture(Kind::Free);
        for turn in 1..=3 {
            f.run_do(&[s("help.about", serde_json::json!({}))]);
            let out = f.note("patience 2 flow 9 slow");
            assert_eq!(out.contains("the run ends here"), turn == 3, "{turn}: {out}");
        }
        assert_eq!(f.run.load().unwrap().end.as_deref(), Some("quit"));
        assert_eq!(
            f.run_do(&[s("help.about", serde_json::json!({}))]),
            "harness: the run has ended (quit)\n"
        );
        // The hand-back is still accepted.
        let hb = HANDBACK_HEADINGS
            .iter()
            .map(|h| format!("## {h}\nx"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            handback(&f.run, &hb).unwrap(),
            "hand-back filed; the run is over\n"
        );
        let rj = f.run.load().unwrap();
        assert_eq!(rj.end.as_deref(), Some("quit"), "the first cause stays");
        assert!(rj.handed_back);

        // Two low notes, a good one, two more: no quit (the rule is about consecutive notes).
        let mut g = fixture(Kind::Free);
        for p in [1, 2, 8, 1, 2] {
            g.run_do(&[s("help.about", serde_json::json!({}))]);
            g.note(&format!("patience {p} flow 9 x"));
        }
        assert_eq!(g.run.load().unwrap().end, None);
        // Five low flows end the run at the fifth.
        let mut h = fixture(Kind::Free);
        for turn in 1..=5 {
            h.run_do(&[s("help.about", serde_json::json!({}))]);
            let out = h.note("patience 9 flow 1 x");
            assert_eq!(out.contains("the run ends here"), turn == 5, "{turn}");
        }
        assert_eq!(h.run.load().unwrap().end.as_deref(), Some("quit"));
    }

    /// BD7: each cap ends the run, counted, never timed.
    #[test]
    fn each_cap_ends_the_run() {
        // Turns: the third turn ends the run; its note is still taken.
        let mut f = fixture(Kind::Free);
        f.run.update(|j| j.caps.turns = 3).unwrap();
        for _ in 0..3 {
            f.run_do(&[s("help.about", serde_json::json!({}))]);
            f.note("patience 9 flow 9 x");
        }
        assert_eq!(f.run.load().unwrap().end.as_deref(), Some("cap"));
        // Steps: a batch is cut at the cap and the run ends.
        let mut g = fixture(Kind::Free);
        g.run.update(|j| j.caps.steps = 2).unwrap();
        let out = g.run_do(&[
            s("help.about", serde_json::json!({})),
            s("ui.dismiss", serde_json::json!({})),
            s("help.about", serde_json::json!({})),
        ]);
        assert!(out.contains("3 help.about skipped"), "{out}");
        assert_eq!(g.run.load().unwrap().end.as_deref(), Some("cap"));
        assert_eq!(g.run.load().unwrap().counts.steps, 2);
        // Harness refusals: the run ends when they reach the cap.
        let mut h = fixture(Kind::Free);
        h.run.update(|j| j.caps.refusals = 2).unwrap();
        for _ in 0..2 {
            h.run_do(&[s("no.such.action", serde_json::json!({}))]);
            h.note("patience 9 flow 9 x");
        }
        assert_eq!(h.run.load().unwrap().end.as_deref(), Some("cap"));
        // The caps of the kinds are the specification's.
        assert_eq!(Kind::Gate.caps().turns, 900);
        assert_eq!(Kind::Free.caps().steps, 800);
        assert_eq!(Kind::Check.caps().looks, 600);
    }

    /// BD7: at most three looks between two `do` calls; `look --page` is not the `page` section.
    #[test]
    fn looks_are_free_within_a_cap_and_page_is_a_flag() {
        let mut f = fixture(Kind::Free);
        for _ in 0..3 {
            let out = look(&f.run, &mut f.backend, &LookOpts::default()).unwrap();
            assert!(out.contains("title: Fake"), "{out}");
            assert!(!out.contains("menu File"), "the menus are left out: {out}");
        }
        assert_eq!(
            look(&f.run, &mut f.backend, &LookOpts::default()).unwrap(),
            "harness: look cap per turn\n"
        );
        assert_eq!(f.run.load().unwrap().counts.looks, 3);
        // A `do` opens a new turn and the looks count again; looks need no note.
        f.run_do(&[s("help.about", serde_json::json!({}))]);
        f.note("patience 9 flow 9 x");
        let page = look(
            &f.run,
            &mut f.backend,
            &LookOpts {
                page: Some(Some("101".to_owned())),
                ..LookOpts::default()
            },
        )
        .unwrap();
        assert!(page.starts_with("page \"101\" \"Title\" reps 1"), "{page}");
        // The `page` section of the interface is a different thing.
        let section = look(
            &f.run,
            &mut f.backend,
            &LookOpts {
                sections: vec!["page".to_owned()],
                ..LookOpts::default()
            },
        )
        .unwrap();
        assert_eq!(section, "page \"101\"\n  label \"x\"\n");
        // The harness's own steps are in the script and counted as looks, not as the agent's steps.
        let rj = f.run.load().unwrap();
        assert_eq!((rj.counts.looks, rj.counts.steps), (5, 1));
        let menus = look(
            &f.run,
            &mut f.backend,
            &LookOpts {
                menus: true,
                ..LookOpts::default()
            },
        )
        .unwrap();
        assert!(
            menus.contains("menu File") && menus.contains("item \"New\""),
            "{menus}"
        );
    }

    #[test]
    fn the_look_cap_of_the_run_ends_it() {
        let mut f = fixture(Kind::Free);
        f.run.update(|j| j.caps.looks = 2).unwrap();
        look(&f.run, &mut f.backend, &LookOpts::default()).unwrap();
        look(&f.run, &mut f.backend, &LookOpts::default()).unwrap();
        assert_eq!(f.run.load().unwrap().end.as_deref(), Some("cap"));
        assert_eq!(
            look(&f.run, &mut f.backend, &LookOpts::default()).unwrap(),
            "harness: the run has ended (cap)\n"
        );
    }

    #[test]
    fn look_diff_shows_only_what_changed_since_the_last_look() {
        let a: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| (*s).to_owned()).collect();
        let b: Vec<String> = ["a", "c", "d", "e"].iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(line_diff(&a, &b), ["- b", "+ e"]);
        assert!(line_diff(&a, &a).is_empty());
        assert_eq!(line_diff(&[], &b).len(), 4);
        let mut f = fixture(Kind::Free);
        let diff = LookOpts {
            diff: true,
            sections: vec!["title:".to_owned(), "status".to_owned()],
            ..LookOpts::default()
        };
        // The first look has nothing to compare with and shows the sections whole.
        let first = look(&f.run, &mut f.backend, &diff).unwrap();
        assert_eq!(first, "title: Fake\nstatus bar: ready\n");
        // Nothing changed.
        assert_eq!(
            look(&f.run, &mut f.backend, &diff).unwrap(),
            "(no change since the last look)\n"
        );
        // Sections are matched with or without their colon.
        assert!(wanted("title:", "title") && wanted("title:", "title:") && !wanted("page", "title"));
    }

    #[test]
    fn the_handback_has_seven_headings_in_order_and_is_accepted_once() {
        let mut f = fixture(Kind::Free);
        let all: Vec<String> = HANDBACK_HEADINGS
            .iter()
            .map(|h| format!("### {h}\ntext"))
            .collect();
        let missing = all[..6].join("\n");
        assert!(
            handback(&f.run, &missing)
                .unwrap()
                .contains("`7. Good` is missing")
        );
        let mut swapped = all.clone();
        swapped.swap(2, 3);
        assert!(
            handback(&f.run, &swapped.join("\n"))
                .unwrap()
                .contains("out of order")
        );
        assert_eq!(f.run.load().unwrap().end, None);
        assert_eq!(
            handback(&f.run, &all.join("\n")).unwrap(),
            "hand-back filed; the run is over\n"
        );
        assert_eq!(f.run.load().unwrap().end.as_deref(), Some("handback"));
        assert!(
            std::fs::read_to_string(f.run.path("handback.md"))
                .unwrap()
                .contains("### 7. Good")
        );
        assert_eq!(
            handback(&f.run, &all.join("\n")).unwrap(),
            "harness: the hand-back is filed\n"
        );
        assert_eq!(
            f.run_do(&[s("help.about", serde_json::json!({}))]),
            "harness: the run has ended (handback)\n"
        );
    }

    #[test]
    fn a_check_run_files_the_checkers_audit_with_its_five_headings() {
        let f = fixture(Kind::Check);
        let seven: Vec<String> = HANDBACK_HEADINGS
            .iter()
            .map(|h| {
                format!(
                    "## {h}
x"
                )
            })
            .collect();
        assert!(
            handback(
                &f.run,
                &seven.join(
                    "
"
                )
            )
            .unwrap()
            .contains("`1. Verdict per folio` is missing")
        );
        let five: Vec<String> = CHECKER_HEADINGS
            .iter()
            .map(|h| {
                format!(
                    "## {h}
x"
                )
            })
            .collect();
        assert_eq!(
            handback(
                &f.run,
                &five.join(
                    "
"
                )
            )
            .unwrap(),
            "hand-back filed; the run is over
"
        );
        assert!(f.run.path("checker.md").is_file() && !f.run.path("handback.md").exists());
    }

    #[test]
    fn a_lost_session_is_said_and_marked() {
        struct Gone;
        impl Backend for Gone {
            fn execute(&mut self, _: &Step) -> Result<Sent> {
                Err(Lost("nobody home".to_owned()).into())
            }
            fn end_call(&mut self) -> Result<Option<i32>> {
                Ok(None)
            }
            fn finish(&mut self) -> Result<Option<i32>> {
                Ok(None)
            }
        }
        let f = fixture(Kind::Free);
        let out = do_steps(
            &f.run,
            &f.reference,
            &mut Gone,
            &[s("help.about", serde_json::json!({}))],
        )
        .unwrap();
        assert!(
            out.contains("harness: session lost, run ") && out.contains(" resume r1"),
            "{out}"
        );
        assert_eq!(f.run.load().unwrap().end.as_deref(), Some("lost"));
        // Nothing reached the product, so there is no turn that wants a note.
        assert!(f.run.turns().unwrap().is_empty());
    }

    #[test]
    fn the_state_diff_prints_the_changed_members() {
        let before: Value =
            serde_json::from_str(r#"{"title":"A","dirty":false,"canvas":{"zoom":100,"grid":1}}"#).unwrap();
        let after: Value =
            serde_json::from_str(r#"{"title":"A","dirty":true,"canvas":{"zoom":120,"grid":1}}"#).unwrap();
        let mut d = state_diff(&before, &after);
        d.sort();
        assert_eq!(d, ["  canvas.zoom: 120", "  dirty: true"]);
        assert!(state_diff(&after, &after).is_empty());
    }
}
