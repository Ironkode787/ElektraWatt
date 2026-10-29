// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Scoring (WP-S6.13 §5.9, D12): the hard and soft checks of a gate run against the oracle of its task.
//!
//! `score.json` (`ew.blindscore/1`) lists the hard checks H1 to H7 and the soft checks S1 to S8, each as `ok`, `miss` or
//! `gap`, and the protocol's counts and observations. The score never calls a model and never opens the golden
//! fixture: the oracle is `tools/blind/tasks/<task>/oracle/` (made by `bless-oracle` from a temporary copy, D16), and
//! what the run produced is read from its folder: the agent's own steps and their outcomes, the epilogue's
//! `page.describe` texts, the exports of `stop` and the answers of the copied `ew` to the task's read-only queries.
//! A check is `gap` when it is not `ok` and the task lists an open follow-up or an undelivered package part for it, or
//! when `--folios` left its folio out; a gap listed for a check that is `ok` is `ok`.
//!
//! Every ordering is named (rows sort by their text, checks by their ID) and no map iteration order reaches the
//! output (NFR-REL-07).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::run::{Counts, Run, RunJson, Turn, parse_script};
use super::session::Sent;

/// The `$schema` of `score.json`.
pub const SCORE_SCHEMA: &str = "ew.blindscore/1";

/// The most differing rows a soft check lists; the rest is a count.
pub const MAX_MISSES: usize = 50;

/// The soft checks and the row set each reads: `(check ID, row set, what)`.
pub const SOFT: [(&str, &str, &str); 8] = [
    (
        "S1",
        "cells",
        "Anchor cells of every representation, by device tag and symbol",
    ),
    (
        "S2",
        "ends",
        "Endpoint labels of the drawn connections, up to orientation",
    ),
    ("S3", "junctions", "Junction sequences by far-end labels"),
    ("S4", "xrefs", "Cross-reference texts among the folios of the run"),
    ("S5", "tags", "Tags, frozen tags included"),
    ("S6", "pages", "Page titles"),
    ("S7", "markers", "Marker texts"),
    (
        "S8",
        "findings",
        "Findings on the drawn devices (rule, severity, tag)",
    ),
];

/// The names of the read-only queries of `oracle/rows.sql` the score needs.
pub const QUERIES: [&str; 5] = ["ends", "xrefs", "tags", "pages", "terminals"];

// ---------------------------------------------------------------------------------------------------------------
// The task: counts, gaps, prelude classes, oracle rows.

/// The exact counts of one folio (`oracle/counts.toml`).
#[derive(Deserialize, Serialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct FolioCount {
    /// The folio.
    pub folio: String,
    /// Representations (`reps` of the `page.describe` header).
    pub reps: u32,
    /// Drawn connections.
    pub connections: u32,
    /// Junctions.
    pub junctions: u32,
    /// Terminal devices on the page (folio 103 of the demo variant).
    #[serde(default)]
    pub terminals: Option<u32>,
    /// Strips holding them.
    #[serde(default)]
    pub strips: Option<u32>,
    /// The turn budget of the folio (reported, never enforced).
    #[serde(default)]
    pub budget_turns: Option<u32>,
    /// Where the numbers come from.
    pub source: String,
}

/// An expected marker text (`oracle/counts.toml`, written to `rows/markers.csv` by `bless-oracle`).
#[derive(Deserialize, Serialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct MarkerRow {
    /// The folio.
    pub folio: String,
    /// The text of the marker.
    pub text: String,
    /// Where it comes from.
    pub source: String,
}

/// `oracle/counts.toml`.
#[derive(Deserialize, Serialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct CountsFile {
    /// One entry per folio, in the order of H1, H2, H3.
    pub folio: Vec<FolioCount>,
    /// The expected marker texts.
    #[serde(default)]
    pub marker: Vec<MarkerRow>,
}

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

/// One gap of `gaps.toml`: a check an open follow-up or an undelivered package part blocks.
#[derive(Deserialize, Serialize, Clone, PartialEq, Eq, Debug)]
pub struct GapEntry {
    /// The check ID (`H4`, `S7`, …).
    pub check: String,
    /// The follow-up (`FT-…`) or the package part (`WP-S2.07`).
    pub gap: String,
    /// One line: why the check is blocked.
    pub reason: String,
}

/// `gaps.toml`.
#[derive(Deserialize, Serialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct GapsFile {
    /// The gaps.
    #[serde(default)]
    pub gap: Vec<GapEntry>,
}

/// One expected row of the parts list: part number, description and quantity.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct BomRow {
    /// The part number.
    pub part: String,
    /// The description.
    pub description: String,
    /// The summed quantity as text.
    pub qty: String,
}

/// A task of the gate kind with its oracle.
#[derive(Clone, Default, Debug)]
pub struct TaskDef {
    /// The exact counts per folio.
    pub counts: Vec<FolioCount>,
    /// The expected parts list.
    pub bom: Vec<BomRow>,
    /// The expected rows of each row set (`cells`, `ends`, …), the header dropped.
    pub rows: BTreeMap<String, Vec<Vec<String>>>,
    /// The prelude's classes, one per step.
    pub prelude: Vec<PreludeEntry>,
    /// The gaps by check ID.
    pub gaps: BTreeMap<String, GapEntry>,
    /// The read-only queries of `oracle/rows.sql` by name.
    pub queries: BTreeMap<String, String>,
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

/// Splits `oracle/rows.sql` into its queries: each begins at a line `-- query: <name>`.
///
/// # Errors
///
/// A text before the first marker, an empty or repeated name, or a query without a body.
pub fn parse_queries(text: &str) -> Result<BTreeMap<String, String>> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    let mut current: Option<(String, String)> = None;
    let mut finish = |cur: Option<(String, String)>| -> Result<()> {
        if let Some((name, body)) = cur {
            // One SELECT: the comment lines are dropped and so is the closing semicolon.
            let body = body.trim().trim_end_matches(';').trim().to_owned();
            if body.is_empty() {
                bail!("rows.sql: query `{name}` has no body");
            }
            if out.insert(name.clone(), body).is_some() {
                bail!("rows.sql: query `{name}` is repeated");
            }
        }
        Ok(())
    };
    for line in text.replace("\r\n", "\n").lines() {
        if let Some(name) = line.strip_prefix("-- query:") {
            finish(current.take())?;
            let name = name.trim();
            if name.is_empty() {
                bail!("rows.sql: a `-- query:` line without a name");
            }
            current = Some((name.to_owned(), String::new()));
        } else if let Some((_, body)) = current.as_mut() {
            if !line.trim_start().starts_with("--") {
                body.push_str(line);
                body.push('\n');
            }
        } else if !line.trim().is_empty() && !line.trim_start().starts_with("--") {
            bail!("rows.sql: text before the first `-- query:` line");
        }
    }
    finish(current.take())?;
    Ok(out)
}

impl TaskDef {
    /// Loads the task `task` of the repository at `root`.
    ///
    /// # Errors
    ///
    /// A missing or malformed file of the task.
    pub fn load(root: &Path, task: &str) -> Result<TaskDef> {
        let dir = task_dir(root, task);
        let counts: CountsFile = read_toml(&dir.join("oracle/counts.toml"))?;
        let prelude: PreludeFile = read_toml(&dir.join("prelude.toml"))?;
        let gaps: GapsFile = read_toml(&dir.join("gaps.toml"))?;
        let bom_text =
            std::fs::read_to_string(dir.join("oracle/bom.csv")).context("reading oracle/bom.csv")?;
        let bom = bom_rows(&parse_csv(&bom_text))?;
        let mut rows = BTreeMap::new();
        for (_, set, _) in SOFT {
            let path = dir.join(format!("oracle/rows/{set}.csv"));
            let text =
                std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
            let mut parsed = parse_csv(&text);
            if !parsed.is_empty() {
                parsed.remove(0);
            }
            rows.insert(set.to_owned(), parsed);
        }
        let sql = std::fs::read_to_string(dir.join("oracle/rows.sql")).context("reading oracle/rows.sql")?;
        Ok(TaskDef {
            counts: counts.folio,
            bom,
            rows,
            prelude: prelude.step,
            gaps: gaps.gap.into_iter().map(|g| (g.check.clone(), g)).collect(),
            queries: parse_queries(&sql)?,
        })
    }

    /// The folios of the oracle, in order.
    pub fn folios(&self) -> Vec<String> {
        self.counts.iter().map(|c| c.folio.clone()).collect()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// CSV and text helpers.

/// Parses CSV (RFC 4180: quoted fields, doubled quotes, line breaks inside quotes).
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let text = text.replace("\r\n", "\n");
    let mut chars = text.chars().peekable();
    let mut any = false;
    while let Some(c) = chars.next() {
        any = true;
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
        } else {
            match c {
                '"' => quoted = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                    any = false;
                }
                _ => field.push(c),
            }
        }
    }
    if any {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// CSV text of `rows`: a field is quoted when it holds a comma, a quote or a line break; lines end with `\n`.
pub fn write_csv(rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    for row in rows {
        let line: Vec<String> = row
            .iter()
            .map(|f| {
                if f.contains([',', '"', '\n', '\r']) {
                    format!("\"{}\"", f.replace('"', "\"\""))
                } else {
                    f.clone()
                }
            })
            .collect();
        out.push_str(&line.join(","));
        out.push('\n');
    }
    out
}

/// The parts list of a CSV with a header: the export's (`row,Pos,Qty,…`, only the `data` rows) or the oracle's
/// (`Pos,Qty,…`); the columns are found by name.
///
/// # Errors
///
/// A CSV without the columns `Qty`, `Part number` and `Description`.
pub fn bom_rows(rows: &[Vec<String>]) -> Result<Vec<BomRow>> {
    let Some(header) = rows.first() else {
        bail!("the parts list is empty");
    };
    let col = |name: &str| {
        header
            .iter()
            .position(|h| h == name)
            .with_context(|| format!("the parts list has no `{name}` column"))
    };
    let (qty, part, desc) = (col("Qty")?, col("Part number")?, col("Description")?);
    let kind = header.iter().position(|h| h == "row");
    let mut out: Vec<BomRow> = rows[1..]
        .iter()
        .filter(|r| kind.is_none_or(|k| r.get(k).is_some_and(|v| v == "data")))
        .map(|r| BomRow {
            part: r.get(part).cloned().unwrap_or_default(),
            description: r.get(desc).cloned().unwrap_or_default(),
            qty: r.get(qty).cloned().unwrap_or_default(),
        })
        .collect();
    out.sort();
    Ok(out)
}

/// The quoted strings of a describe line, in order (the describe text never holds a quotation mark inside a string).
pub fn quoted(line: &str) -> Vec<String> {
    line.split('"')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s.to_owned())
        .collect()
}

/// The word after `key` in `line`, outside the quoted strings.
fn word_after<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let at = line.find(&format!(" {key} "))?;
    line[at + key.len() + 2..].split(' ').next()
}

/// The `(reps, connections, junctions)` of a `page.describe` header line.
pub fn header_counts(text: &str) -> Option<(u32, u32, u32)> {
    let header = text.lines().next()?;
    if !header.starts_with("page ") {
        return None;
    }
    // After the title: `… reps N connections N junctions N`. Search from the end, the title is user text.
    let at = header.rfind(" reps ")?;
    let tail = &header[at..];
    let n = |key: &str| word_after(tail, key)?.parse::<u32>().ok();
    Some((n("reps")?, n("connections")?, n("junctions")?))
}

/// The rows of the page text for S1: `folio, label, symbol, cell` per `rep` line.
pub fn cell_rows(folio: &str, text: &str) -> Vec<Vec<String>> {
    text.lines()
        .filter(|l| l.starts_with("  rep "))
        .map(|l| {
            let q = quoted(l);
            let ip = l.starts_with("  rep ip ");
            let (label, cell, symbol) = match (ip, q.as_slice()) {
                (true, [name, cell, symbol, ..]) => (name.clone(), cell.clone(), symbol.clone()),
                (false, _) if l.starts_with("  rep none ") => (
                    "none".to_owned(),
                    q.first().cloned().unwrap_or_default(),
                    q.get(1).cloned().unwrap_or_default(),
                ),
                (false, [label, cell, symbol, ..]) => (label.clone(), cell.clone(), symbol.clone()),
                _ => (String::new(), String::new(), String::new()),
            };
            vec![folio.to_owned(), label, symbol, cell]
        })
        .collect()
}

/// The rows of the page text for S3: `folio, style, wiring, sequence` per `junction` line.
pub fn junction_rows(folio: &str, text: &str) -> Vec<Vec<String>> {
    text.lines()
        .filter(|l| l.starts_with("  junction "))
        .map(|l| {
            let at = l.find(" sequence ").unwrap_or(l.len());
            let (head, seq) = l.split_at(at);
            vec![
                folio.to_owned(),
                word_after(head, "style").unwrap_or("").to_owned(),
                word_after(head, "wiring").unwrap_or("").to_owned(),
                quoted(seq).join(" | "),
            ]
        })
        .collect()
}

/// The rows of the page text for S7: `folio, text` per `item marker` line.
pub fn marker_rows(folio: &str, text: &str) -> Vec<Vec<String>> {
    text.lines()
        .filter(|l| l.starts_with("  item marker "))
        .map(|l| vec![folio.to_owned(), quoted(l).last().cloned().unwrap_or_default()])
        .collect()
}

/// The folios a cross-reference text names: `102-A1` names 102, `∞ 3-B2, 106-G3` names 3 and 106.
pub fn xref_folios(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '.'))
        .filter_map(|w| {
            let (folio, cell) = w.split_once('-')?;
            let mut chars = cell.chars();
            let row = chars.next()?;
            let folio_ok = !folio.is_empty()
                && folio.chars().all(|c| c.is_ascii_digit() || c == '.')
                && folio.chars().next().is_some_and(|c| c.is_ascii_digit());
            let cell_ok =
                ('A'..='H').contains(&row) && !chars.as_str().is_empty() && chars.all(|c| c.is_ascii_digit());
            (folio_ok && cell_ok).then(|| folio.to_owned())
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------------
// The result of an `ew sql` call.

/// The rows of `ew sql … --format json`, every cell as text (`null` is empty).
///
/// # Errors
///
/// Text that is not an `ew.sql-result/1` document.
pub fn parse_sql_json(text: &str) -> Result<Vec<Vec<String>>> {
    let v: Value = serde_json::from_str(text).context("ew sql printed no JSON")?;
    let rows = v["rows"].as_array().context("ew sql printed no rows member")?;
    Ok(rows
        .iter()
        .map(|r| {
            r.as_array()
                .map(|cells| {
                    cells
                        .iter()
                        .map(|c| match c {
                            Value::Null => String::new(),
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect())
}

// ---------------------------------------------------------------------------------------------------------------
// The score.

/// `ok`, `miss` or `gap`.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// The run holds what the oracle wants.
    Ok,
    /// It does not, and nothing blocks it.
    Miss,
    /// It does not, and a listed follow-up, an undelivered part or the run's folios explain it.
    Gap,
}

/// `pass`, `pass-with-gaps`, `fail` or `incomplete`.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Every hard check is `ok`.
    Pass,
    /// Every hard check is `ok` or `gap`, at least one `gap`.
    PassWithGaps,
    /// A hard check is `miss`.
    Fail,
    /// The run ended before its epilogue by a cap, a quit rule, the orchestrator, a lost session or a divergence, or a
    /// folio was not started.
    Incomplete,
}

impl Outcome {
    /// The outcome's word.
    pub fn word(self) -> &'static str {
        match self {
            Outcome::Pass => "pass",
            Outcome::PassWithGaps => "pass-with-gaps",
            Outcome::Fail => "fail",
            Outcome::Incomplete => "incomplete",
        }
    }
}

/// One check of the score.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Check {
    /// `H1` … `H7`, `S1` … `S8`.
    pub id: String,
    /// What the check asks.
    pub what: String,
    /// What the oracle wants, in words.
    pub want: String,
    /// What the run holds, in words.
    pub got: String,
    /// `ok`, `miss` or `gap`.
    pub status: Status,
    /// For a `gap`: the follow-up, the package part or `not in this run`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<String>,
    /// The differing rows or the reasons, at most [`MAX_MISSES`].
    pub misses: Vec<String>,
    /// How many more differing rows there were.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub more: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// How many `do` turns a folio took against its budget.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct FolioUse {
    /// `do` turns that ended on the folio.
    pub turns: u32,
    /// The folio's budget (reported, never enforced).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<u32>,
}

/// The prelude's lines by class.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct PreludeUse {
    /// Lines of class `setup`.
    pub setup: u32,
    /// Lines of class `drawing`.
    pub drawing: u32,
}

/// What the harness observed and never judges (§5.9).
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
    /// The prelude's lines by class.
    pub prelude: PreludeUse,
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
    /// The outcome; `None` for a run that has no oracle (a free run holds only `protocol`).
    pub outcome: Option<Outcome>,
    /// The hard checks.
    pub hard: Vec<Check>,
    /// The soft checks.
    pub soft: Vec<Check>,
    /// The protocol.
    pub protocol: Protocol,
}

/// A step of the agent with its outcome.
#[derive(Clone, PartialEq, Debug)]
pub struct AgentStep {
    /// The action.
    pub action: String,
    /// The parameters.
    pub params: Value,
    /// `done`, `refused` or `failed`.
    pub outcome: String,
}

/// What a run holds that the score reads.
#[derive(Clone, Debug)]
pub struct Evidence {
    /// The run's `run.json`.
    pub rj: RunJson,
    /// The agent's own steps (not the harness's looks and not the epilogue), in order.
    pub agent_steps: Vec<AgentStep>,
    /// The turns of `turns.jsonl`.
    pub turns: Vec<Turn>,
    /// The epilogue's `page.describe` texts by folio.
    pub epilogue: BTreeMap<String, String>,
    /// The run's `work/` folder (the agent's exports are files in it).
    pub work: PathBuf,
    /// `out/bom.csv`, if `ew export bom` wrote it.
    pub bom_csv: Option<String>,
    /// `out/check.json`, if `ew check` wrote it.
    pub check_json: Option<String>,
    /// The answers of the read-only queries by name: the `ew sql --format json` text, or the error.
    pub sql: BTreeMap<String, Result<String, String>>,
    /// The actions of the run's prelude steps, in order (`prelude.toml` has one entry for each).
    pub prelude_actions: Vec<String>,
    /// The PDFs of 1 and 4 threads were identical.
    pub pdfs_identical: Option<bool>,
    /// The exit code of `ew check`.
    pub ew_check_exit: Option<i32>,
}

#[cfg(test)]
impl Evidence {
    /// Evidence of a run with nothing read yet.
    pub fn new(rj: RunJson) -> Evidence {
        Evidence {
            rj,
            agent_steps: Vec::new(),
            turns: Vec::new(),
            epilogue: BTreeMap::new(),
            work: PathBuf::new(),
            bom_csv: None,
            check_json: None,
            sql: BTreeMap::new(),
            prelude_actions: Vec::new(),
            pdfs_identical: None,
            ew_check_exit: None,
        }
    }
}

fn ok_check(id: &str, what: &str, want: String, got: String) -> Check {
    Check {
        id: id.to_owned(),
        what: what.to_owned(),
        want,
        got,
        status: Status::Ok,
        gap: None,
        misses: Vec::new(),
        more: 0,
    }
}

/// How a check ended before the gaps are applied.
struct Raw {
    id: String,
    what: String,
    want: String,
    got: String,
    ok: bool,
    misses: Vec<String>,
    /// The check's folios are not (all) in the run: a failed check is a gap with this reason.
    out_of_run: bool,
}

impl Raw {
    fn into_check(self, task: &TaskDef) -> Check {
        let mut c = ok_check(&self.id, &self.what, self.want, self.got);
        if self.ok {
            return c;
        }
        let total = self.misses.len();
        c.misses = self.misses.into_iter().take(MAX_MISSES).collect();
        c.more = total.saturating_sub(MAX_MISSES);
        if self.out_of_run {
            c.status = Status::Gap;
            c.gap = Some("not in this run".to_owned());
        } else if let Some(g) = task.gaps.get(&self.id) {
            c.status = Status::Gap;
            c.gap = Some(g.gap.clone());
        } else {
            c.status = Status::Miss;
        }
        c
    }
}

/// The rows of `want` that `got` lacks and the rows of `got` that `want` lacks, as lines (`missing: …`, `extra: …`),
/// sorted by row text; equal rows cancel one for one (a multiset difference).
pub fn row_diff(want: &[Vec<String>], got: &[Vec<String>]) -> Vec<String> {
    let text = |r: &Vec<String>| r.join(" | ");
    let mut count: BTreeMap<String, i64> = BTreeMap::new();
    for r in want {
        *count.entry(text(r)).or_default() += 1;
    }
    for r in got {
        *count.entry(text(r)).or_default() -= 1;
    }
    let mut out = Vec::new();
    for (row, n) in count {
        for _ in 0..n.max(0) {
            out.push(format!("missing: {row}"));
        }
        for _ in 0..(-n).max(0) {
            out.push(format!("extra: {row}"));
        }
    }
    out
}

/// The agent's own steps with their outcomes: the steps of every `do` turn, read from the transcript lines (the
/// transcript starts with the prelude's lines).
///
/// # Errors
///
/// A transcript line that is not JSON.
pub fn agent_steps(
    steps: &[(String, Value)],
    transcript: &[String],
    prelude_len: usize,
    turns: &[Turn],
) -> Result<Vec<AgentStep>> {
    let mut out = Vec::new();
    for t in turns.iter().filter(|t| t.kind == "do") {
        let Some([a, b]) = t.steps else { continue };
        for n in a..=b {
            let i = usize::try_from(n).unwrap_or(usize::MAX);
            let (Some((action, params)), Some(line)) = (
                steps.get(i.wrapping_sub(1)),
                transcript.get(prelude_len + i.wrapping_sub(1)),
            ) else {
                continue;
            };
            let sent = Sent::parse(line.clone())?;
            out.push(AgentStep {
                action: action.clone(),
                params: params.clone(),
                outcome: sent.outcome().to_owned(),
            });
        }
    }
    Ok(out)
}

/// The epilogue's `page.describe` texts by folio, from the transcript.
///
/// # Errors
///
/// A transcript line that is not JSON.
pub fn epilogue_texts(
    steps: &[(String, Value)],
    transcript: &[String],
    prelude_len: usize,
    turns: &[Turn],
) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let Some(Turn {
        steps: Some([a, b]), ..
    }) = turns.iter().rev().find(|t| t.kind == "epilogue")
    else {
        return Ok(out);
    };
    for n in *a..=*b {
        let i = usize::try_from(n).unwrap_or(usize::MAX).wrapping_sub(1);
        let (Some((action, params)), Some(line)) = (steps.get(i), transcript.get(prelude_len + i)) else {
            continue;
        };
        if action != "page.describe" {
            continue;
        }
        let sent = Sent::parse(line.clone())?;
        if let (Some(folio), Some(text)) = (params["folio"].as_str(), sent.describe()) {
            out.insert(folio.to_owned(), text.to_owned());
        }
    }
    Ok(out)
}

/// Whether the file `rel` (a bare name) exists in `work`.
fn work_has(work: &Path, rel: &str) -> bool {
    !rel.is_empty() && work.join(rel).is_file()
}

/// Whether `work` holds a PDF anywhere (a `file.export_pdf` without a path).
fn work_has_pdf(work: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(work) else {
        return false;
    };
    entries.flatten().any(|e| {
        let p = e.path();
        if p.is_dir() {
            work_has_pdf(&p)
        } else {
            p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pdf"))
        }
    })
}

fn hard_counts(task: &TaskDef, ev: &Evidence, run_folios: &[String]) -> Vec<Raw> {
    let mut out = Vec::new();
    for (i, c) in task.counts.iter().enumerate() {
        let id = format!("H{}", i + 1);
        let in_run = run_folios.contains(&c.folio);
        let mut want = format!(
            "reps {} connections {} junctions {}",
            c.reps, c.connections, c.junctions
        );
        if let (Some(t), Some(s)) = (c.terminals, c.strips) {
            let _ = write!(want, " terminals {t} strips {s}");
        }
        let mut misses = Vec::new();
        let got = match ev.epilogue.get(&c.folio).and_then(|t| header_counts(t)) {
            None => {
                misses.push("the epilogue holds no page text for the folio".to_owned());
                "no page text".to_owned()
            }
            Some((reps, connections, junctions)) => {
                let mut got = format!("reps {reps} connections {connections} junctions {junctions}");
                for (name, want_n, got_n) in [
                    ("reps", c.reps, reps),
                    ("connections", c.connections, connections),
                    ("junctions", c.junctions, junctions),
                ] {
                    if want_n != got_n {
                        misses.push(format!("{name}: want {want_n}, got {got_n}"));
                    }
                }
                if let (Some(wt), Some(ws)) = (c.terminals, c.strips) {
                    match ev.sql.get("terminals") {
                        Some(Ok(text)) => {
                            match parse_sql_json(text).ok().and_then(|r| r.into_iter().next()) {
                                Some(row) => {
                                    let n = |i: usize| row.get(i).and_then(|v| v.parse::<u32>().ok());
                                    let (gt, gs) = (n(0).unwrap_or(0), n(1).unwrap_or(0));
                                    let _ = write!(got, " terminals {gt} strips {gs}");
                                    if gt != wt {
                                        misses.push(format!("terminals: want {wt}, got {gt}"));
                                    }
                                    if gs != ws {
                                        misses.push(format!("strips: want {ws}, got {gs}"));
                                    }
                                }
                                None => misses.push("the terminal query returned no row".to_owned()),
                            }
                        }
                        Some(Err(e)) => misses.push(format!("the terminal query failed: {e}")),
                        None => misses.push("the terminal query was not run".to_owned()),
                    }
                }
                got
            }
        };
        out.push(Raw {
            id,
            what: format!("Exact counts on folio {}", c.folio),
            want,
            got,
            ok: misses.is_empty(),
            misses,
            out_of_run: !in_run,
        });
    }
    out
}

fn hard_bom(task: &TaskDef, ev: &Evidence, run_folios: &[String]) -> Raw {
    let all = task.folios().iter().all(|f| run_folios.contains(f));
    let want: Vec<Vec<String>> = task
        .bom
        .iter()
        .map(|b| vec![b.part.clone(), b.description.clone(), b.qty.clone()])
        .collect();
    let (got, misses) = match ev.bom_csv.as_deref().map(|t| bom_rows(&parse_csv(t))) {
        None => (
            Vec::new(),
            vec!["the run has no parts list (`ew export bom` wrote none)".to_owned()],
        ),
        Some(Err(e)) => (Vec::new(), vec![format!("the parts list does not parse: {e}")]),
        Some(Ok(rows)) => {
            let got: Vec<Vec<String>> = rows
                .iter()
                .map(|b| vec![b.part.clone(), b.description.clone(), b.qty.clone()])
                .collect();
            let d = row_diff(&want, &got);
            (got, d)
        }
    };
    Raw {
        id: "H4".to_owned(),
        what: "The parts list equals the oracle's (part number, description, quantity)".to_owned(),
        want: format!("{} rows", want.len()),
        got: format!("{} rows", got.len()),
        ok: misses.is_empty(),
        misses,
        out_of_run: !all,
    }
}

fn hard_agent(task: &TaskDef, ev: &Evidence) -> Vec<Raw> {
    let mut out = Vec::new();
    // H5: the agent exported the PDF itself.
    let exports: Vec<&AgentStep> = ev
        .agent_steps
        .iter()
        .filter(|s| s.action == "file.export_pdf" && s.outcome == "done")
        .collect();
    let file_ok = exports.iter().any(|s| match s.params["path"].as_str() {
        Some(p) => work_has(&ev.work, p),
        None => work_has_pdf(&ev.work),
    });
    let mut misses = Vec::new();
    if exports.is_empty() {
        misses.push("no `file.export_pdf` step of the agent was done".to_owned());
    } else if !file_ok {
        misses.push("the PDF the agent exported is not in the work folder".to_owned());
    }
    out.push(Raw {
        id: "H5".to_owned(),
        what: "The agent exported the PDF itself".to_owned(),
        want: "a `file.export_pdf` step of the agent done, its file in work/".to_owned(),
        got: format!(
            "{} done, file {}",
            exports.len(),
            if file_ok { "present" } else { "absent" }
        ),
        ok: misses.is_empty(),
        misses,
        out_of_run: false,
    });
    // H6: the agent saved its work itself, and the project was clean after its last step.
    let saves = ev
        .agent_steps
        .iter()
        .filter(|s| s.action == "file.save" && s.outcome == "done")
        .count();
    let clean = ev.rj.clean_before_epilogue;
    let mut misses = Vec::new();
    if saves == 0 {
        misses.push("no `file.save` step of the agent was done".to_owned());
    }
    match clean {
        Some(true) => {}
        Some(false) => misses
            .push("the project was dirty after the agent's last step (the epilogue saved it)".to_owned()),
        None => misses
            .push("it is not known whether the project was clean after the agent's last step".to_owned()),
    }
    out.push(Raw {
        id: "H6".to_owned(),
        what: "The agent saved its work itself".to_owned(),
        want: "a `file.save` step of the agent done and the project clean after its last step".to_owned(),
        got: format!(
            "{saves} saves done, project {}",
            match clean {
                Some(true) => "clean",
                Some(false) => "dirty",
                None => "unknown",
            }
        ),
        ok: misses.is_empty(),
        misses,
        out_of_run: false,
    });
    // H7: no prelude line of class `drawing`.
    let drawing: Vec<&PreludeEntry> = task.prelude.iter().filter(|p| p.class == "drawing").collect();
    let mut misses: Vec<String> = drawing
        .iter()
        .map(|p| {
            format!(
                "prelude line `{}` is drawing ({})",
                p.action,
                p.gap.as_deref().unwrap_or("no follow-up")
            )
        })
        .collect();
    if task.prelude.len() != ev.prelude_actions.len() {
        misses.push(format!(
            "prelude.toml has {} entries for {} prelude steps",
            task.prelude.len(),
            ev.prelude_actions.len()
        ));
    }
    out.push(Raw {
        id: "H7".to_owned(),
        what: "No prelude line of class drawing".to_owned(),
        want: "0 drawing lines".to_owned(),
        got: format!("{} drawing lines", drawing.len()),
        ok: misses.is_empty(),
        misses,
        out_of_run: false,
    });
    out
}

/// S1 leaves out the terminals of folio 103 (the demo variant places them where the golden strip segments were; H3
/// counts them) on both sides.
fn s1_left_out(row: &[String]) -> bool {
    row.first().is_some_and(|f| f == "103")
        && row
            .get(2)
            .is_some_and(|s| s.contains("terminal") || s.contains("strip-segment"))
}

/// A frame cell `<folio>-<row letter><column>` as `(folio, row, column)`.
fn split_cell(cell: &str) -> Option<(&str, u8, u32)> {
    let (folio, rest) = cell.rsplit_once('-')?;
    let mut chars = rest.chars();
    let row = chars.next().filter(char::is_ascii_uppercase)?;
    let column = chars.as_str().parse::<u32>().ok()?;
    Some((folio, u8::try_from(row).ok()?, column))
}

/// The strip of an S1 row that is a terminal of a strip (`<strip>:<designation>` drawn with a terminal symbol).
fn strip_of(row: &[String]) -> Option<&str> {
    let label = row.get(1)?;
    let symbol = row.get(2)?;
    if !symbol.contains("terminal") {
        return None;
    }
    label.split_once(':').map(|(strip, _)| strip)
}

/// FT-555: S1 accepts a strip's terminal at any cell of the strip's column run. The oracle anchors the `n` terminals of
/// a strip at one cell (1X1's three at 101-D2); a terminal drawn in the same row of the frame, in the anchor's column or
/// one of the `n - 1` columns after it (101-D2 to 101-D4), counts as drawn at the anchor. Every other row of `got` is
/// returned as it is.
pub fn fold_column_runs(want: &[Vec<String>], got: Vec<Vec<String>>) -> Vec<Vec<String>> {
    // The number of terminals the oracle anchors per (folio, strip, cell).
    let mut runs: BTreeMap<(String, String, String), u8> = BTreeMap::new();
    for w in want {
        if let (Some(strip), Some(folio), Some(cell)) = (strip_of(w), w.first(), w.get(3)) {
            let n = runs
                .entry((folio.clone(), strip.to_owned(), cell.clone()))
                .or_default();
            *n = n.saturating_add(1);
        }
    }
    got.into_iter()
        .map(|mut g| {
            if want.contains(&g) {
                return g;
            }
            let Some(strip) = strip_of(&g).map(str::to_owned) else {
                return g;
            };
            let Some((gf, gr, gc)) = g.get(3).and_then(|c| split_cell(c)) else {
                return g;
            };
            let at = want.iter().find_map(|w| {
                if w.first() != g.first() || w.get(1) != g.get(1) || w.get(2) != g.get(2) {
                    return None;
                }
                let cell = w.get(3)?;
                let (wf, wr, wc) = split_cell(cell)?;
                let n = runs.get(&(w[0].clone(), strip.clone(), cell.clone()))?;
                (wf == gf && wr == gr && gc >= wc && gc - wc < u32::from(*n)).then(|| cell.clone())
            });
            if let Some(cell) = at {
                g[3] = cell;
            }
            g
        })
        .collect()
}

/// The tag of a finding's subject among `tags`: the longest tag the subject ends with (after its sign).
fn finding_tag(subject: &str, tags: &BTreeSet<String>) -> Option<String> {
    tags.iter()
        .filter(|t| subject.ends_with(&format!("-{}", t.trim_start_matches('-'))))
        .max_by_key(|t| t.len())
        .cloned()
}

/// The rows of `findings` from `ew check --format json`: `rule, severity, tag`, restricted to the drawn tags.
pub fn finding_rows(check_json: &str, tags: &BTreeSet<String>) -> Result<Vec<Vec<String>>> {
    let v: Value = serde_json::from_str(check_json).context("out/check.json is not JSON")?;
    let list = v["findings"]
        .as_array()
        .context("out/check.json has no findings member")?;
    let mut rows: Vec<Vec<String>> = list
        .iter()
        .filter_map(|f| {
            let subject = f["subject"].as_str()?;
            let tag = finding_tag(subject, tags)?;
            Some(vec![
                f["rule"].as_str()?.to_owned(),
                f["severity"].as_str()?.to_owned(),
                tag,
            ])
        })
        .collect();
    rows.sort();
    Ok(rows)
}

/// The device tags (not interruption points) of rows of the `tags` set `folio, kind, tag`.
fn device_tags(rows: &[Vec<String>], folios: &[String]) -> BTreeSet<String> {
    rows.iter()
        .filter(|r| folios.contains(&r[0]) && r.get(1).is_some_and(|k| k != "ip"))
        .filter_map(|r| r.get(2).cloned())
        .collect()
}

fn in_folios(row: &[String], folios: &[String]) -> bool {
    row.first().is_some_and(|f| folios.contains(f))
}

/// The rows of a row set: the expected ones, or the run's own or the reason there are none.
type Rows = Vec<Vec<String>>;
type RowsOrReason = std::result::Result<Rows, String>;

fn soft_checks(task: &TaskDef, ev: &Evidence, run_folios: &[String]) -> Vec<Raw> {
    let sql_rows = |name: &str| -> RowsOrReason {
        match ev.sql.get(name) {
            Some(Ok(text)) => parse_sql_json(text).map_err(|e| format!("the `{name}` query: {e}")),
            Some(Err(e)) => Err(format!("the `{name}` query failed: {e}")),
            None => Err(format!("the `{name}` query was not run")),
        }
    };
    // What the epilogue's page texts hold, for S1, S3 and S7.
    let mut cells = Vec::new();
    let mut junctions = Vec::new();
    let mut markers = Vec::new();
    for (folio, text) in &ev.epilogue {
        cells.extend(cell_rows(folio, text));
        junctions.extend(junction_rows(folio, text));
        markers.extend(marker_rows(folio, text));
    }
    let tags_got = sql_rows("tags");
    let want_set = |set: &str| task.rows.get(set).cloned().unwrap_or_default();
    let mut out = Vec::new();
    for (id, set, what) in SOFT {
        let (want, got): (Rows, RowsOrReason) = match set {
            "cells" => {
                let want: Rows = want_set(set).into_iter().filter(|r| !s1_left_out(r)).collect();
                let got =
                    fold_column_runs(&want, cells.iter().filter(|r| !s1_left_out(r)).cloned().collect());
                (want, Ok(got))
            }
            "junctions" => (want_set(set), Ok(junctions.clone())),
            "markers" => (want_set(set), Ok(markers.clone())),
            "ends" => (
                // A wire whose pages are not all in the run ends somewhere the run does not draw.
                want_set(set)
                    .into_iter()
                    .filter(|r| {
                        r.get(3)
                            .is_some_and(|span| span.split(',').all(|f| run_folios.contains(&f.to_owned())))
                    })
                    .collect(),
                sql_rows("ends").map(|rows| {
                    rows.into_iter()
                        .filter(|r| {
                            r.get(3).is_some_and(|span| {
                                span.split(',').all(|f| run_folios.contains(&f.to_owned()))
                            })
                        })
                        .collect()
                }),
            ),
            "xrefs" => (
                // A text whose partner lies on a folio the run does not draw is left out (AC-05's rule).
                want_set(set)
                    .into_iter()
                    .filter(|r| {
                        r.get(2)
                            .is_some_and(|t| xref_folios(t).iter().all(|f| run_folios.contains(f)))
                    })
                    .collect(),
                sql_rows("xrefs"),
            ),
            "findings" => {
                let want_tags = device_tags(&want_set("tags"), run_folios);
                let want = want_set(set)
                    .into_iter()
                    .filter(|r| r.get(2).is_some_and(|t| want_tags.contains(t)))
                    .collect();
                let got = match (&tags_got, &ev.check_json) {
                    (Ok(rows), Some(json)) => {
                        let tags = device_tags(rows, run_folios);
                        finding_rows(json, &tags).map_err(|e| e.to_string())
                    }
                    (Err(e), _) => Err(e.clone()),
                    (_, None) => Err("the run has no check report (`ew check` wrote none)".to_owned()),
                };
                (want, got)
            }
            other => (want_set(other), sql_rows(other)),
        };
        let want: Vec<Vec<String>> = want
            .into_iter()
            .filter(|r| set == "findings" || in_folios(r, run_folios))
            .collect();
        let (got_rows, misses) = match got {
            Ok(rows) => {
                let rows: Vec<Vec<String>> = rows
                    .into_iter()
                    .filter(|r| set == "findings" || in_folios(r, run_folios))
                    .collect();
                let d = row_diff(&want, &rows);
                (rows, d)
            }
            Err(e) => (Vec::new(), vec![e]),
        };
        out.push(Raw {
            id: id.to_owned(),
            what: what.to_owned(),
            want: format!("{} rows", want.len()),
            got: format!("{} rows", got_rows.len()),
            ok: misses.is_empty(),
            misses,
            out_of_run: false,
        });
    }
    out
}

/// The outcome of the hard checks and the run's end (§5.9).
pub fn outcome_of(rj: &RunJson, turns: &[Turn], run_folios: &[String], hard: &[Check]) -> Outcome {
    let stopped_early = matches!(
        rj.end.as_deref(),
        Some("cap" | "quit" | "orchestrator" | "lost" | "diverged")
    );
    let started = |f: &String| {
        turns
            .iter()
            .any(|t| t.kind == "do" && t.folio.as_ref() == Some(f))
    };
    if stopped_early || !run_folios.iter().all(started) {
        Outcome::Incomplete
    } else if hard.iter().any(|c| c.status == Status::Miss) {
        Outcome::Fail
    } else if hard.iter().any(|c| c.status == Status::Gap) {
        Outcome::PassWithGaps
    } else {
        Outcome::Pass
    }
}

/// The protocol part of the score.
pub fn protocol_of(task: Option<&TaskDef>, ev: &Evidence, run_folios: &[String]) -> Protocol {
    let rj = &ev.rj;
    let mut p = Protocol {
        counts: rj.counts,
        end: rj.end.clone(),
        handed_back: rj.handed_back,
        folios: run_folios.to_vec(),
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
    if let Some(task) = task {
        for c in &task.counts {
            if let Some(b) = c.budget_turns {
                p.turns_by_folio.entry(c.folio.clone()).or_default().budget = Some(b);
            }
        }
        for e in &task.prelude {
            if e.class == "drawing" {
                p.prelude.drawing += 1;
            } else {
                p.prelude.setup += 1;
            }
        }
    }
    p
}

/// Scores a run: the checks of the gate task against its oracle, or only the protocol (`task` is `None`, a free run).
pub fn score(task: Option<&TaskDef>, ev: &Evidence) -> Score {
    let rj = ev.rj.clone();
    let run_folios: Vec<String> = if !rj.folios.is_empty() {
        rj.folios.clone()
    } else {
        task.map(TaskDef::folios).unwrap_or_default()
    };
    let protocol = protocol_of(task, ev, &run_folios);
    let (hard, soft, outcome) = match task {
        None => (Vec::new(), Vec::new(), None),
        Some(task) => {
            let mut raw = hard_counts(task, ev, &run_folios);
            raw.push(hard_bom(task, ev, &run_folios));
            raw.extend(hard_agent(task, ev));
            let hard: Vec<Check> = raw.into_iter().map(|r| r.into_check(task)).collect();
            let soft: Vec<Check> = soft_checks(task, ev, &run_folios)
                .into_iter()
                .map(|r| r.into_check(task))
                .collect();
            let outcome = outcome_of(&rj, &ev.turns, &run_folios, &hard);
            (hard, soft, Some(outcome))
        }
    };
    Score {
        schema: SCORE_SCHEMA.to_owned(),
        run: rj.run.clone(),
        task: rj.task.clone(),
        outcome,
        hard,
        soft,
        protocol,
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
    let Some(outcome) = score.outcome else {
        let _ = writeln!(
            md,
            "A free run holds no oracle: the protocol counts below are its score.\n"
        );
        return md;
    };
    let _ = writeln!(md, "Outcome: **{}**\n", outcome.word());
    for (title, checks) in [("Hard checks", &score.hard), ("Soft checks", &score.soft)] {
        let _ = writeln!(md, "### {title}\n");
        let _ = writeln!(md, "| ID | Check | Want | Got | Status |");
        let _ = writeln!(md, "|---|---|---|---|---|");
        for c in checks {
            let status = match (&c.status, &c.gap) {
                (Status::Gap, Some(g)) => format!("gap ({g})"),
                (Status::Ok, _) => "ok".to_owned(),
                _ => "miss".to_owned(),
            };
            let cell = |s: &str| s.replace('|', "/");
            let _ = writeln!(
                md,
                "| {} | {} | {} | {} | {} |",
                c.id,
                cell(&c.what),
                cell(&c.want),
                cell(&c.got),
                status
            );
        }
        let _ = writeln!(md);
        for c in checks.iter().filter(|c| !c.misses.is_empty()) {
            let _ = writeln!(md, "{} ({}):\n", c.id, c.what);
            for m in &c.misses {
                let _ = writeln!(md, "- {m}");
            }
            if c.more > 0 {
                let _ = writeln!(md, "- and {} more", c.more);
            }
            let _ = writeln!(md);
        }
    }
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

/// Runs the task's read-only queries with the run's copy of `ew` over `work/project`.
pub fn collect_sql(
    run: &Run,
    queries: &BTreeMap<String, String>,
) -> BTreeMap<String, Result<String, String>> {
    let ew = run.product_exe("ew");
    let mut out = BTreeMap::new();
    for name in QUERIES {
        let Some(query) = queries.get(name) else { continue };
        let args = ["sql", "work/project", query.as_str(), "--format", "json"];
        let result = match super::session::run_tool(run, &ew, &args, &run.dir) {
            Ok((Some(0), stdout, _)) => Ok(stdout),
            Ok((code, _, stderr)) => Err(format!(
                "exit {}: {}",
                code.map_or("?".to_owned(), |c| c.to_string()),
                stderr.lines().next().unwrap_or("").trim()
            )),
            Err(e) => Err(e.to_string()),
        };
        out.insert(name.to_owned(), result);
    }
    out
}

/// The evidence of a stopped run: its files, and (for a task with queries) the copied `ew`'s answers.
///
/// # Errors
///
/// A file of the run that cannot be read.
pub fn evidence_of(run: &Run, task: Option<&TaskDef>, facts: ExportFacts) -> Result<Evidence> {
    let rj = run.load()?;
    let prelude = super::run::read_script(&run.path("prelude.uiscript"))?;
    let steps = run.steps()?;
    let transcript = run.transcript()?;
    let turns = run.turns()?;
    let read = |rel: &str| std::fs::read_to_string(run.path(rel)).ok();
    Ok(Evidence {
        agent_steps: agent_steps(&steps, &transcript, prelude.len(), &turns)?,
        epilogue: epilogue_texts(&steps, &transcript, prelude.len(), &turns)?,
        turns,
        work: run.path("work"),
        bom_csv: read("out/bom.csv"),
        check_json: read("out/check.json"),
        sql: task.map(|t| collect_sql(run, &t.queries)).unwrap_or_default(),
        prelude_actions: prelude.into_iter().map(|(a, _)| a).collect(),
        pdfs_identical: facts.pdfs_identical,
        ew_check_exit: facts.ew_check_exit,
        rj,
    })
}

/// Scores the stopped run `run` and writes `out/score.json`; returns the score.
///
/// A drafter run of a task with an oracle (`tools/blind/tasks/<task>/oracle/counts.toml`) is scored against it; every
/// other run holds only its protocol.
///
/// # Errors
///
/// A missing or malformed task file, or a file of the run that cannot be read or written.
pub fn score_run(root: &Path, run: &Run, facts: ExportFacts) -> Result<Score> {
    let rj = run.load()?;
    let has_oracle = rj.kind == "gate" && task_dir(root, &rj.task).join("oracle/counts.toml").is_file();
    let task = if has_oracle {
        Some(TaskDef::load(root, &rj.task)?)
    } else {
        None
    };
    let ev = evidence_of(run, task.as_ref(), facts)?;
    let score = score(task.as_ref(), &ev);
    std::fs::create_dir_all(run.path("out"))?;
    super::run::write_atomic(&run.path("out/score.json"), &score_json(&score)?)?;
    Ok(score)
}

#[cfg(test)]
mod tests;

/// The steps of a prelude text, for the checks of `check-task`.
///
/// # Errors
///
/// A line that is not a step.
pub fn prelude_actions(text: &str) -> Result<Vec<String>> {
    Ok(parse_script(text)?.into_iter().map(|(a, _)| a).collect())
}
