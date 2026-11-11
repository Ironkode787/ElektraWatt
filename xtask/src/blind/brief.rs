// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The open design brief (visual spec §6.1 and §6.2) and `check-brief <task>`.
//!
//! A brief lives in `tools/blind/tasks/brief-<name>/`: `task.md` (the brief in a user's words), `task.toml` (`kind`,
//! `grade`, `version`, `turn_budget`, `senses`, `requires`, `areas`), `acceptance.toml` (the acceptance list),
//! `prelude.uiscript` and `prelude.toml` (setup only) and `solution.uiscript`, the reference solution in the text
//! vocabulary.
//!
//! **Feasibility (the owner's rule of 2026-10-29):** a brief is runnable only if the current build can complete it.
//! `check-brief` therefore replays the prelude and the reference solution headless in a scratch folder, makes the
//! exports with the built `ew` and decides every acceptance item from them: each item names a read-only `query` over
//! the `api_v1_*` views and what its row count must `expect`, or is decided by its output (`check`: the check report has
//! no error; `pdf`: the solution exported `project.pdf` itself; `bom`: the parts list has rows). Every hard item must
//! be met; every soft item met or marked `infeasible = "<reason / WP>"`. A step of the solution that only the
//! prelude's command form can do (`command.run`) is a finding: the window has no path for it. `start --kind brief`
//! refuses a brief that does not pass.
//!
//! **The usability rounds' tasks:** `task.toml` may name the task (`task_id`, `T1`), its rounds
//! (`rounds`) and its start project (`start`: `gp-ut` or `golden-a`), which every replay makes as `work/project` from a
//! copy of the golden project ([`super::start::make_start_project`]) before the prelude opens it. An item may name
//! `actions`, an ordered list of action IDs (alternatives joined by ` or `) that must be done in that order, decided
//! from the outcomes. A task may have `hints.toml` (one to three hints, [`super::round`]). [`decide_items`] decides
//! the items here and at a brief run's `stop`, with the same code.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

use super::run::{parse_script, script_header, step_line};
use super::score::{PreludeFile, read_toml, task_dir};

/// The outputs an acceptance item may be decided from (§6.2).
pub const HOW: [&str; 8] = [
    "check",
    "bom",
    "connections",
    "pages",
    "pdf",
    "outcomes",
    "work",
    "handback",
];

/// `task.toml` of a brief.
#[derive(Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct TaskToml {
    /// `brief`.
    pub kind: String,
    /// 1 to 3.
    pub grade: u8,
    /// The brief's version (a change of meaning bumps it).
    pub version: u32,
    /// The turns a user would need, reported and never enforced.
    pub turn_budget: u32,
    /// `text`, `visual` or both.
    pub senses: Vec<String>,
    /// What the brief needs of the product: `action: <id>`, `fndef: <id>`, `symbol: <name>`, `part: <number>`.
    #[serde(default)]
    pub requires: Vec<String>,
    /// The window's areas the brief exercises ("symbol picker", "wiring", …): a visual round of a UI cycle picks briefs
    /// by them, and friction in an area still under feature work is parked.
    #[serde(default)]
    pub areas: Vec<String>,
    /// The task's ID in its plan (`T1`, `A7`).
    #[serde(default)]
    pub task_id: Option<String>,
    /// The usability rounds that run the task (`R0`, `R1`, `R2`): a round task's prompt adds `ROUND.md` (D11).
    #[serde(default)]
    pub rounds: Vec<String>,
    /// The start project: `gp-ut` or `golden-a` (D5); absent, the prelude makes the project.
    #[serde(default)]
    pub start: Option<String>,
}

/// K9: a `task.toml` with another start.
pub const START_VALUES: &str = "task.toml: `start` is `gp-ut` or `golden-a`";

/// The start projects a brief may name (D5).
pub const STARTS: [&str; 2] = ["gp-ut", "golden-a"];

/// The brief's `task.toml`, or `None` for a task without one (every task that is not a brief).
///
/// # Errors
///
/// A `task.toml` that does not parse.
pub fn task_toml(root: &Path, task: &str) -> Result<Option<TaskToml>> {
    let path = task_dir(root, task).join("task.toml");
    if !path.is_file() {
        return Ok(None);
    }
    read_toml(&path).map(Some)
}

/// One item of `acceptance.toml`.
#[derive(Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// `A1`, `A2`, …
    pub id: String,
    /// `hard` or `soft`.
    pub kind: String,
    /// One sentence in engineering terms.
    pub what: String,
    /// The outputs that decide it, joined by `, ` (of [`HOW`]).
    pub how: String,
    /// A read-only SELECT over the `api_v1_*` views that proves the item on the reference solution.
    #[serde(default)]
    pub query: Option<String>,
    /// What the query's row count must be: `>= 3`, `== 1`, `<= 0`.
    #[serde(default)]
    pub expect: Option<String>,
    /// A soft item the current build cannot meet: the reason and the package that brings it.
    #[serde(default)]
    pub infeasible: Option<String>,
    /// Action IDs that must be done in this order (alternatives joined by ` or `), decided from the outcomes (D7).
    #[serde(default)]
    pub actions: Option<Vec<String>>,
}

/// `acceptance.toml`.
#[derive(Deserialize, Clone, PartialEq, Eq, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct Acceptance {
    /// The items in order.
    #[serde(default)]
    pub item: Vec<Item>,
}

/// What `check-brief` found.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct BriefCheck {
    /// What makes the brief not runnable now.
    pub problems: Vec<String>,
    /// What does not block it but is worth a look (a solution step without a window path).
    pub findings: Vec<String>,
    /// Per acceptance item: its ID, kind and the decision on the reference solution.
    pub items: Vec<(String, String, String)>,
}

impl BriefCheck {
    /// Whether the brief is runnable now.
    pub fn runnable(&self) -> bool {
        self.problems.is_empty()
    }

    /// The text `check-brief` prints.
    pub fn text(&self, task: &str) -> String {
        let mut out = String::new();
        for (id, kind, decision) in &self.items {
            let _ = writeln!(out, "{id} {kind}: {decision}");
        }
        for f in &self.findings {
            let _ = writeln!(out, "finding: {f}");
        }
        for p in &self.problems {
            let _ = writeln!(out, "problem: {p}");
        }
        if self.runnable() {
            let _ = writeln!(out, "brief `{task}` is runnable");
        } else {
            let _ = writeln!(
                out,
                "brief `{task}` is NOT runnable: {} problem(s)",
                self.problems.len()
            );
        }
        out
    }
}

/// The comparison of an `expect`: an operator and a number.
fn parse_expect(text: &str) -> Option<(&str, u64)> {
    let t = text.trim();
    for op in [">=", "<=", "==", ">", "<", "="] {
        if let Some(rest) = t.strip_prefix(op) {
            return rest.trim().parse().ok().map(|n| (op, n));
        }
    }
    None
}

fn holds(op: &str, got: u64, want: u64) -> bool {
    match op {
        ">=" => got >= want,
        "<=" => got <= want,
        ">" => got > want,
        "<" => got < want,
        _ => got == want,
    }
}

/// Whether `text` holds `word` with no letter, digit, `.`, `_` or `-` on either side.
fn has_word(text: &str, word: &str) -> bool {
    let joins = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    text.match_indices(word).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let mut rest = text[i + word.len()..].chars();
        // A full stop that ends the sentence does not join: `file.save.` names the action, `file.save.x` does not.
        let after = match rest.next() {
            Some('.') => rest.next().filter(|c| c.is_ascii_alphanumeric()),
            other => other,
        };
        !before.is_some_and(joins) && !after.is_some_and(joins)
    })
}

/// The action IDs of `tools/blind/reference/actions.txt`.
fn action_ids(root: &Path) -> Result<BTreeSet<String>> {
    let text = std::fs::read_to_string(root.join("tools/blind/reference/actions.txt"))
        .context("reading tools/blind/reference/actions.txt")?;
    Ok(text
        .lines()
        .filter_map(|l| l.split(' ').next())
        .filter(|id| id.contains('.'))
        .map(str::to_owned)
        .collect())
}

/// Whether any text file under `dir` holds `needle`.
fn tree_holds(dir: &Path, needle: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        let p = e.path();
        if p.is_dir() {
            tree_holds(&p, needle)
        } else {
            std::fs::read_to_string(&p).is_ok_and(|t| t.contains(needle))
        }
    })
}

/// Whether the build can see the requirement `req` met, or why not.
fn requirement(root: &Path, dir: &Path, actions: &BTreeSet<String>, req: &str) -> Result<(), String> {
    let Some((form, value)) = req.split_once(':') else {
        return Err(format!(
            "cannot see `{req}` met (a requirement is action:, fndef:, symbol: or part: and a value)"
        ));
    };
    let value = value.trim();
    // The starter library is the content folder: a symbol or a function definition is a file there.
    let starter = || tree_holds(&root.join("content"), value);
    let met = match form.trim() {
        "action" => actions.contains(value),
        "fndef" | "symbol" => starter(),
        // A part the prelude embeds, or one of the shipped starter parts, which every new project lists.
        "part" => {
            std::fs::read_to_string(dir.join("prelude.uiscript")).is_ok_and(|t| t.contains(value))
                || tree_holds(&root.join("content/libraries"), value)
        }
        _ => {
            return Err(format!(
                "cannot see `{req}` met (a requirement is action:, fndef:, symbol: or part: and a value)"
            ));
        }
    };
    if met {
        Ok(())
    } else {
        Err(format!("cannot see `{req}` met in the current build"))
    }
}

/// The checks that need no product: the files, the classes of the prelude, the words of the brief, the requirements.
///
/// # Errors
///
/// A file that cannot be read.
pub fn check_files(root: &Path, task: &str) -> Result<(BriefCheck, Option<Acceptance>)> {
    let dir = task_dir(root, task);
    let mut c = BriefCheck::default();
    if !task.starts_with("brief-") || !dir.is_dir() {
        c.problems.push(format!(
            "`{task}` is not a brief folder (tools/blind/tasks/brief-<name>/)"
        ));
        return Ok((c, None));
    }
    for f in [
        "task.md",
        "task.toml",
        "acceptance.toml",
        "prelude.uiscript",
        "prelude.toml",
        "solution.uiscript",
    ] {
        if !dir.join(f).is_file() {
            c.problems.push(format!("{f} is missing"));
        }
    }
    if !c.problems.is_empty() {
        return Ok((c, None));
    }
    let actions = action_ids(root)?;
    match read_toml::<TaskToml>(&dir.join("task.toml")) {
        Err(e) => c.problems.push(format!("task.toml: {e:#}")),
        Ok(t) => {
            if t.kind != "brief" {
                c.problems.push("task.toml: kind is `brief`".to_owned());
            }
            if !(1..=3).contains(&t.grade) {
                c.problems.push("task.toml: grade is 1 to 3".to_owned());
            }
            if t.version == 0 || t.turn_budget == 0 {
                c.problems
                    .push("task.toml: version and turn_budget are 1 or more".to_owned());
            }
            if t.senses.is_empty() || t.senses.iter().any(|s| s != "text" && s != "visual") {
                c.problems
                    .push("task.toml: senses is a list of text and visual".to_owned());
            }
            if t.areas.is_empty() {
                c.problems
                    .push("task.toml: areas names the window's areas the brief exercises".to_owned());
            }
            for req in &t.requires {
                if let Err(e) = requirement(root, &dir, &actions, req) {
                    c.problems.push(format!("requires: {e}"));
                }
            }
            if t.start.as_deref().is_some_and(|s| !STARTS.contains(&s)) {
                c.problems.push(START_VALUES.to_owned());
            }
        }
    }
    // The hints: one to three, in the user's words.
    if dir.join("hints.toml").is_file() {
        match read_toml::<super::round::Hints>(&dir.join("hints.toml")) {
            Err(e) => c.problems.push(format!("hints.toml: {e:#}")),
            Ok(h) => {
                if !(1..=3).contains(&h.hint.len()) {
                    c.problems.push("hints.toml: one to three hints".to_owned());
                }
                for id in &actions {
                    if h.hint.iter().any(|x| has_word(&x.text, id)) {
                        c.problems.push(format!(
                            "hints.toml names the action {id}: a hint is in a user's words"
                        ));
                    }
                }
            }
        }
    }
    let acceptance = match read_toml::<Acceptance>(&dir.join("acceptance.toml")) {
        Err(e) => {
            c.problems.push(format!("acceptance.toml: {e:#}"));
            None
        }
        Ok(a) => {
            let mut ids = BTreeSet::new();
            for it in &a.item {
                if !ids.insert(it.id.clone()) {
                    c.problems.push(format!("acceptance.toml: {} twice", it.id));
                }
                if it.kind != "hard" && it.kind != "soft" {
                    c.problems
                        .push(format!("acceptance.toml: {} is hard or soft", it.id));
                }
                let hows: Vec<&str> = it.how.split(',').map(str::trim).collect();
                if it.how.trim().is_empty() || hows.iter().any(|h| !HOW.contains(h)) {
                    c.problems.push(format!(
                        "acceptance.toml: {} decides by {} (of {})",
                        it.id,
                        it.how,
                        HOW.join(", ")
                    ));
                }
                if it.query.is_some() != it.expect.is_some()
                    || it.expect.as_deref().is_some_and(|e| parse_expect(e).is_none())
                {
                    c.problems.push(format!(
                        "acceptance.toml: {} has a query with an expect like `>= 3`, or neither",
                        it.id
                    ));
                }
                if it.kind == "hard" && it.infeasible.is_some() {
                    c.problems.push(format!(
                        "acceptance.toml: {} is hard and marked infeasible: a hard item the build cannot meet makes the brief not runnable",
                        it.id
                    ));
                }
            }
            let hard = a.item.iter().filter(|i| i.kind == "hard").count();
            if hard < 3 {
                c.problems.push(format!(
                    "acceptance.toml: at least three hard items, each with a how (has {hard})"
                ));
            }
            Some(a)
        }
    };
    // Every prelude line is setup: a brief's prelude never draws.
    let prelude = parse_script(&std::fs::read_to_string(dir.join("prelude.uiscript"))?)?;
    match read_toml::<PreludeFile>(&dir.join("prelude.toml")) {
        Err(e) => c.problems.push(format!("prelude.toml: {e:#}")),
        Ok(p) => {
            if p.step.len() != prelude.len() {
                c.problems.push(format!(
                    "prelude.toml has {} entries for {} prelude lines",
                    p.step.len(),
                    prelude.len()
                ));
            }
            for (i, (e, (action, _))) in p.step.iter().zip(&prelude).enumerate() {
                if &e.action != action {
                    c.problems.push(format!(
                        "prelude.toml entry {} names {} and the prelude line is {action}",
                        i + 1,
                        e.action
                    ));
                }
                if e.class != "setup" {
                    c.problems.push(format!(
                        "prelude line {} ({action}) is `{}`: a brief's prelude is setup only",
                        i + 1,
                        e.class
                    ));
                }
            }
        }
    }
    // The brief in a user's words: no action ID of the product, no customer or personal data.
    let brief = std::fs::read_to_string(dir.join("task.md"))?.replace("\r\n", "\n");
    for id in &actions {
        if has_word(&brief, id) {
            c.problems.push(format!(
                "task.md names the action {id}: a brief is in a user's words"
            ));
        }
    }
    let list = root.join("ci/denylist.sha256");
    if let Ok(text) = std::fs::read_to_string(&list) {
        let denylist = crate::anonymise::parse_denylist(&text)?;
        let mut files = Vec::new();
        for f in [
            "task.md",
            "task.toml",
            "acceptance.toml",
            "solution.uiscript",
            "hints.toml",
        ] {
            if !dir.join(f).is_file() {
                continue;
            }
            files.push((
                format!("tools/blind/tasks/{task}/{f}"),
                std::fs::read_to_string(dir.join(f))?,
            ));
        }
        c.problems.extend(crate::anonymise::check(&files, &denylist));
    }
    Ok((c, acceptance))
}

static CHECKS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// A fresh scratch folder for one check of a brief under `parent`: `<parent>/<task>-<token>-<pid>-<n>`, so no two
/// checks share one, in one process or in several.
fn scratch(parent: &Path, task: &str) -> PathBuf {
    let n = CHECKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let token: String = super::keeper::new_token().chars().take(8).collect();
    parent.join(format!("{task}-{token}-{}-{n}", std::process::id()))
}

/// Runs the copied tools' `ew` with `args` in `dir`; its exit code and standard output.
fn ew(root: &Path, exe: &Path, args: &[&str], dir: &Path) -> Result<(Option<i32>, String)> {
    let out = Command::new(exe)
        .args(args)
        .current_dir(dir)
        .env("EW_CONTENT_DIR", root.join("content"))
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("running {}", exe.display()))?;
    Ok((
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    ))
}

/// `check-brief <task>`: the files, then the reference solution replayed from a new project on the build in `bin`
/// (`elektrawatt` and `ew`), its exports and the acceptance list decided on them.
///
/// # Errors
///
/// A file that cannot be read, a binary that does not start.
pub fn check_brief(root: &Path, task: &str, bin: &Path) -> Result<BriefCheck> {
    check_brief_in(root, task, bin, &std::env::temp_dir().join("ew-blind-brief"))
}

/// [`check_brief`] with its scratch folder under `parent` (a test passes its own folder, removed after it).
///
/// # Errors
///
/// As [`check_brief`].
pub fn check_brief_in(root: &Path, task: &str, bin: &Path, parent: &Path) -> Result<BriefCheck> {
    let (mut c, acceptance) = check_files(root, task)?;
    let Some(acceptance) = acceptance else {
        return Ok(c);
    };
    let dir = task_dir(root, task);
    let exe = |n: &str| bin.join(format!("{n}{}", std::env::consts::EXE_SUFFIX));
    for n in ["elektrawatt", "ew"] {
        if !exe(n).is_file() {
            c.problems.push(format!(
                "{} is not built (cargo build -p elektrawatt -p ew): the reference solution is not proven",
                exe(n).display()
            ));
            return Ok(c);
        }
    }
    let prelude = parse_script(&std::fs::read_to_string(dir.join("prelude.uiscript"))?)?;
    let solution = parse_script(&std::fs::read_to_string(dir.join("solution.uiscript"))?)?;
    for (i, (action, params)) in solution.iter().enumerate() {
        if action == "command.run" {
            c.findings.push(format!(
                "solution line {} uses the command form ({}): the window has no path for it",
                i + 1,
                params["cmd"].as_str().unwrap_or("?")
            ));
        }
    }
    let start = task_toml(root, task)?.and_then(|t| t.start);
    let scratch = scratch(parent, task);
    let script = Script {
        prelude: &prelude,
        solution: &solution,
        start: start.as_deref(),
    };
    let result = prove(root, &mut c, &acceptance, &script, &scratch, &exe);
    let _ = std::fs::remove_dir_all(&scratch);
    result?;
    Ok(c)
}

/// What [`prove`] replays: the prelude and the reference solution, from the start project if the task names one.
struct Script<'a> {
    prelude: &'a [(String, Value)],
    solution: &'a [(String, Value)],
    start: Option<&'a str>,
}

/// The replay and the decisions of [`check_brief`], in `scratch`.
fn prove(
    root: &Path,
    c: &mut BriefCheck,
    acceptance: &Acceptance,
    script: &Script,
    scratch: &Path,
    exe: &dyn Fn(&str) -> PathBuf,
) -> Result<()> {
    let Script {
        prelude,
        solution,
        start,
    } = *script;
    for sub in ["work", "config", "empty-sample", "book", "out"] {
        std::fs::create_dir_all(scratch.join(sub))?;
    }
    // The start project (D5): a copy of the golden project in `work/project`, which the prelude opens.
    if let Some(start) = start
        && let Err(e) =
            super::start::make_start_project(root, start, &exe("ew"), &scratch.join("work/project"))
    {
        c.problems.push(format!("{e:#}"));
        return Ok(());
    }
    let mut script = format!("{}\n", script_header("check-brief"));
    for (a, p) in prelude.iter().chain(solution) {
        script.push_str(&step_line(a, p));
        script.push('\n');
    }
    std::fs::write(scratch.join("solution.uiscript"), &script)?;
    let out = Command::new(exe("elektrawatt"))
        .args(["--headless", "--script"])
        .arg(scratch.join("solution.uiscript"))
        .args(["--keep-going", "--config-dir"])
        .arg(scratch.join("config"))
        .current_dir(scratch.join("work"))
        .env("EW_SAMPLE_DIR", scratch.join("empty-sample"))
        .env("EW_DOCS_DIR", scratch.join("book"))
        .env("EW_CONTENT_DIR", root.join("content"))
        .env_remove("EW_CONFIG_DIR")
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .context("running the reference solution")?;
    let text = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = text.lines().skip(1).collect();
    if lines.len() < prelude.len() + solution.len() {
        let err = String::from_utf8_lossy(&out.stderr);
        c.problems.push(format!(
            "the reference solution stopped after {} of {} lines ({})",
            lines.len(),
            prelude.len() + solution.len(),
            err.lines().next().unwrap_or("no message")
        ));
    }
    for (i, line) in lines.iter().enumerate() {
        let v: Value = serde_json::from_str(line).unwrap_or(Value::Null);
        if v["outcome"].as_str() != Some("done") {
            let (part, k) = if i < prelude.len() {
                ("prelude", i + 1)
            } else {
                ("solution", i + 1 - prelude.len())
            };
            c.problems.push(format!(
                "{part} line {k} ({}) was {} {}",
                v["action"].as_str().unwrap_or("?"),
                v["outcome"].as_str().unwrap_or("?"),
                v["reason"]
                    .as_str()
                    .or_else(|| v["message"].as_str())
                    .unwrap_or("")
            ));
        }
    }
    if out.status.code() != Some(0) {
        let err = String::from_utf8_lossy(&out.stderr);
        c.problems.push(format!(
            "the reference solution exits {} ({})",
            out.status.code().map_or("?".to_owned(), |n| n.to_string()),
            err.lines().next().unwrap_or("no message")
        ));
    }
    let work = scratch.join("work");
    let project = work.join("project");
    if !project.is_dir() {
        c.problems
            .push("the reference solution saves no project `project`".to_owned());
        return Ok(());
    }
    let ew_exe = exe("ew");
    // The actions the reference solution did, after the prelude (an `actions` item reads them).
    let done = done_actions(&agent_outcomes(&lines, prelude.len()));
    let decisions = decide_items(acceptance, &work, &done, &mut |args: &[&str]| {
        ew(root, &ew_exe, args, &work)
    })?;
    for (it, decision) in acceptance.item.iter().zip(decisions) {
        let text = match (&decision, &it.infeasible) {
            (Ok(()), _) => "met".to_owned(),
            (Err(why), Some(inf)) if it.kind == "soft" => format!("infeasible ({inf}); {why}"),
            (Err(why), _) => {
                c.problems.push(format!(
                    "{} ({}) is not met by the reference solution: {why}",
                    it.id, it.kind
                ));
                format!("NOT met: {why}")
            }
        };
        c.items.push((it.id.clone(), it.kind.clone(), text));
    }
    Ok(())
}

/// The canvas's clicks: a click on a cross-reference is the Select tool's follow, which dispatches
/// these and not `nav.follow`.
const CLICKS: [&str; 4] = [
    "canvas.activate",
    "canvas.activate_alt",
    "canvas.press",
    "canvas.release",
];

/// The actions and their outcomes, in order, in the transcript `lines` (after their header): a text line's action, and
/// each action that a visual input or frame line dispatched. A line whose canvas click was done and after which
/// another page shows than before it is a followed cross-reference: a `nav.follow` done follows its actions.
pub fn step_outcomes<'a>(lines: impl IntoIterator<Item = &'a str>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut page: Option<String> = None;
    for line in lines {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let steps: Vec<&Value> = match v["dispatched"].as_array() {
            Some(d) => d.iter().collect(),
            None => vec![&v],
        };
        let mut clicked = false;
        for s in steps {
            if let (Some(a), Some(o)) = (s["action"].as_str(), s["outcome"].as_str()) {
                clicked |= o == "done" && CLICKS.contains(&a);
                out.push((a.to_owned(), o.to_owned()));
            }
        }
        let now = v["state"]["page"].as_str().map(str::to_owned);
        if clicked && page.is_some() && now.is_some() && now != page {
            out.push(("nav.follow".to_owned(), "done".to_owned()));
        }
        if now.is_some() {
            page = now;
        }
    }
    out
}

/// The outcomes of the steps after the first `prelude` lines of `lines` ([`step_outcomes`], read from the start so that
/// the page before the first step is known).
pub fn agent_outcomes(lines: &[&str], prelude: usize) -> Vec<(String, String)> {
    let before = step_outcomes(lines.iter().take(prelude).copied()).len();
    step_outcomes(lines.iter().copied())
        .into_iter()
        .skip(before)
        .collect()
}

/// The actions done, in order ([`step_outcomes`] whose outcome is `done`).
pub fn done_actions(outcomes: &[(String, String)]) -> Vec<String> {
    outcomes
        .iter()
        .filter(|(_, o)| o == "done")
        .map(|(a, _)| a.clone())
        .collect()
}

/// Whether the actions `wanted` (each an action ID, or alternatives joined by ` or `) were done in this order in
/// `done`, other actions between them allowed (D7).
///
/// # Errors
///
/// The first wanted action that was not done after the ones before it.
pub fn actions_met(wanted: &[String], done: &[String]) -> Result<(), String> {
    let mut at = 0;
    for w in wanted {
        let alternatives: Vec<&str> = w.split(" or ").map(str::trim).collect();
        match done[at..].iter().position(|d| alternatives.contains(&d.as_str())) {
            Some(i) => at += i + 1,
            None => {
                return Err(format!(
                    "`{w}` was not done after the actions before it (in order: {})",
                    wanted.join(", ")
                ));
            }
        }
    }
    Ok(())
}

/// Whether `dir` holds a PDF file that is not empty, at any depth.
fn holds_pdf(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        let p = e.path();
        if p.is_dir() {
            holds_pdf(&p)
        } else {
            p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pdf"))
                && std::fs::metadata(&p).is_ok_and(|m| m.len() > 0)
        }
    })
}

/// Runs `ew` with the arguments in the work folder: its exit code and standard output.
pub type EwFn<'a> = dyn FnMut(&[&str]) -> Result<(Option<i32>, String)> + 'a;

/// Decides every item of `acceptance` on the saved project `<work>/project`, in order: `check-brief` on the reference
/// solution and `stop` on a brief run (D7) decide with this one code. `done` are the actions done, in order (an
/// `actions` item reads them); `ew` runs the product's `ew` in `work`. A query decides an item by its row count, else
/// its outputs do: `pdf` a PDF in `work`, `check` a check report without an error, `bom` a parts list with a row. An
/// `actions` item must also have its actions done in order.
///
/// # Errors
///
/// `ew` does not start.
pub fn decide_items(
    acceptance: &Acceptance,
    work: &Path,
    done: &[String],
    ew: &mut EwFn,
) -> Result<Vec<Result<(), String>>> {
    let (check_exit, _) = ew(&["check", "project", "--format", "text"])?;
    let (_, bom) = ew(&["export", "bom", "project", "--mode", "parts", "--format", "csv"])?;
    let mut out = Vec::new();
    for it in &acceptance.item {
        if let Some(wanted) = &it.actions
            && let Err(why) = actions_met(wanted, done)
        {
            out.push(Err(why));
            continue;
        }
        let decision: Result<(), String> = match (&it.query, &it.expect) {
            (Some(q), Some(e)) => {
                let (code, rows) = ew(&["sql", "project", q, "--format", "csv"])?;
                let n = rows.lines().skip(1).filter(|l| !l.trim().is_empty()).count() as u64;
                match (code, parse_expect(e)) {
                    (Some(0), Some((op, want))) if holds(op, n, want) => Ok(()),
                    (Some(0), _) => Err(format!("the query returns {n} row(s), expected {e}")),
                    (code, _) => Err(format!(
                        "the query fails (exit {})",
                        code.map_or("?".to_owned(), |n| n.to_string())
                    )),
                }
            }
            _ if it.actions.is_some() => Ok(()),
            _ => {
                let hows: Vec<&str> = it.how.split(',').map(str::trim).collect();
                if hows.contains(&"pdf") {
                    if holds_pdf(work) {
                        Ok(())
                    } else {
                        Err("no PDF was exported into the work folder".to_owned())
                    }
                } else if hows.contains(&"check") {
                    if check_exit == Some(0) {
                        Ok(())
                    } else {
                        Err(format!(
                            "the check report has errors (ew check exit {})",
                            check_exit.map_or("?".to_owned(), |n| n.to_string())
                        ))
                    }
                } else if hows.contains(&"bom") {
                    if bom.lines().skip(1).any(|l| !l.trim().is_empty()) {
                        Ok(())
                    } else {
                        Err("the parts list has no row".to_owned())
                    }
                } else {
                    Err("no query or output decides it".to_owned())
                }
            }
        };
        out.push(decision);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_expect_is_an_operator_and_a_count() {
        assert_eq!(parse_expect(">= 3"), Some((">=", 3)));
        assert_eq!(parse_expect("== 0"), Some(("==", 0)));
        assert_eq!(parse_expect("3"), None);
        assert!(holds(">=", 3, 3) && !holds(">", 3, 3) && holds("=", 2, 2));
    }

    #[test]
    fn an_action_id_is_a_word_not_a_part_of_one() {
        assert!(has_word("then file.save it", "file.save"));
        assert!(!has_word("file.save_as", "file.save"));
        assert!(!has_word("profile.saved", "file.save"));
        assert!(
            has_word("then use file.save.", "file.save"),
            "a full stop ends the sentence"
        );
        assert!(!has_word("file.save.x", "file.save"));
    }

    /// A copy of `brief` (and the action list) under a temporary root, for a test to break.
    fn copy_brief(brief: &str) -> (crate::blind::testkit::Tmp, PathBuf) {
        let tmp = crate::blind::testkit::Tmp::new("brief");
        let root = crate::workspace_root();
        let dir = tmp.0.join("tools/blind/tasks").join(brief);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(tmp.0.join("tools/blind/reference")).unwrap();
        std::fs::copy(
            root.join("tools/blind/reference/actions.txt"),
            tmp.0.join("tools/blind/reference/actions.txt"),
        )
        .unwrap();
        // The starter content, where a `symbol:` or `fndef:` requirement is looked up.
        for folder in ["content/symbols", "content/symbols/qet10", "content/fndefs"] {
            std::fs::create_dir_all(tmp.0.join(folder)).unwrap();
            for e in std::fs::read_dir(root.join(folder)).unwrap().flatten() {
                if e.path().is_file() {
                    std::fs::copy(e.path(), tmp.0.join(folder).join(e.file_name())).unwrap();
                }
            }
        }
        for e in std::fs::read_dir(task_dir(&root, brief)).unwrap().flatten() {
            std::fs::copy(e.path(), dir.join(e.file_name())).unwrap();
        }
        (tmp, dir)
    }

    /// BV9: a brief with two hard items, a drawing line in its prelude, an action ID in its text, an unknown `how`
    /// and an unmet requirement is not runnable, and each problem is named.
    #[test]
    fn a_broken_brief_is_not_runnable_and_says_why() {
        let (tmp, dir) = copy_brief("brief-valve-switch");
        let acceptance = std::fs::read_to_string(dir.join("acceptance.toml")).unwrap();
        // Two hard items only: A3 to A6 become soft; A1 decides by a word that is not an output.
        let acceptance = acceptance
            .replace("id = \"A3\"\nkind = \"hard\"", "id = \"A3\"\nkind = \"soft\"")
            .replace("id = \"A4\"\nkind = \"hard\"", "id = \"A4\"\nkind = \"soft\"")
            .replace("id = \"A5\"\nkind = \"hard\"", "id = \"A5\"\nkind = \"soft\"")
            .replace("id = \"A6\"\nkind = \"hard\"", "id = \"A6\"\nkind = \"soft\"")
            .replacen("how = \"connections, pages\"", "how = \"looks\"", 1);
        std::fs::write(dir.join("acceptance.toml"), acceptance).unwrap();
        let toml = std::fs::read_to_string(dir.join("prelude.toml")).unwrap();
        std::fs::write(
            dir.join("prelude.toml"),
            toml.replacen(
                "action = \"file.new\"\nclass = \"setup\"",
                "action = \"file.new\"\nclass = \"drawing\"",
                1,
            ),
        )
        .unwrap();
        let task = std::fs::read_to_string(dir.join("task.md")).unwrap();
        std::fs::write(
            dir.join("task.md"),
            format!("{task}\nThen use file.save to keep it.\n"),
        )
        .unwrap();
        let t = std::fs::read_to_string(dir.join("task.toml")).unwrap();
        std::fs::write(
            dir.join("task.toml"),
            t.replace("\"action: strip.new\"", "\"action: strip.teleport\""),
        )
        .unwrap();
        let (c, _) = check_files(&tmp.0, "brief-valve-switch").unwrap();
        assert!(!c.runnable());
        let all = c.problems.join("\n");
        for want in [
            "acceptance.toml: at least three hard items, each with a how (has 2)",
            "acceptance.toml: A1 decides by looks",
            "prelude line 4 (file.new) is `drawing`: a brief's prelude is setup only",
            "task.md names the action file.save: a brief is in a user's words",
            "requires: cannot see `action: strip.teleport` met in the current build",
        ] {
            assert!(all.contains(want), "{want}\n{all}");
        }
        assert!(
            c.text("brief-valve-switch")
                .ends_with("is NOT runnable: 5 problem(s)\n"),
            "{all}"
        );
        // A hard item marked infeasible makes the brief not runnable too.
        let a = std::fs::read_to_string(dir.join("acceptance.toml")).unwrap();
        std::fs::write(
            dir.join("acceptance.toml"),
            a.replacen(
                "expect = \">= 3\"",
                "expect = \">= 3\"\ninfeasible = \"not yet\"",
                1,
            ),
        )
        .unwrap();
        let (c, _) = check_files(&tmp.0, "brief-valve-switch").unwrap();
        assert!(
            c.problems
                .iter()
                .any(|p| p.contains("A1 is hard and marked infeasible")),
            "{:?}",
            c.problems
        );
    }

    /// UR-2: an `actions` item is met when its actions were done in that order (others between them allowed, an
    /// alternative joined by ` or ` counts), and not met when one is missing, refused or out of order.
    #[test]
    fn actions_are_met_in_order_only() {
        let want: Vec<String> = ["nav.follow or props.follow", "nav.back"]
            .map(str::to_owned)
            .to_vec();
        let done = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            actions_met(
                &want,
                &done(&["page.goto", "nav.follow", "ui.describe", "nav.back"])
            ),
            Ok(())
        );
        assert_eq!(actions_met(&want, &done(&["props.follow", "nav.back"])), Ok(()));
        let missing = Err(
            "`nav.back` was not done after the actions before it (in order: nav.follow or props.follow, nav.back)"
                .to_owned(),
        );
        assert_eq!(actions_met(&want, &done(&["nav.follow"])), missing);
        assert_eq!(
            actions_met(&want, &done(&["nav.back", "nav.follow"])),
            missing,
            "out of order"
        );
        assert_eq!(
            actions_met(&want, &done(&[])),
            Err("`nav.follow or props.follow` was not done after the actions before it (in order: nav.follow or \
                 props.follow, nav.back)"
                .to_owned())
        );
        // The outcomes of a text transcript and a visual one: only what was done counts, a refused step does not.
        let text = [
            r#"{"step":1,"action":"nav.follow","outcome":"done","reason":null}"#,
            r#"{"step":2,"action":"nav.back","outcome":"refused","reason":"reason-x"}"#,
        ];
        let done_of = |lines: &[&str]| done_actions(&step_outcomes(lines.iter().copied()));
        assert_eq!(done_of(&text), ["nav.follow"]);
        assert_eq!(
            actions_met(&want, &done_of(&text)),
            missing,
            "a refused step is not done"
        );
        let visual = [
            r#"{"frame":"t0000.png","sha256":null,"dispatched":[]}"#,
            r#"{"step":1,"input":{"input":"key"},"dispatched":[{"action":"nav.follow","outcome":"done","reason":null}]}"#,
            r#"{"step":2,"input":{"input":"key"},"dispatched":[{"action":"edit.undo","outcome":"refused","reason":"r"},{"action":"nav.back","outcome":"done","reason":null}]}"#,
        ];
        assert_eq!(done_of(&visual), ["nav.follow", "nav.back"]);
        assert_eq!(actions_met(&want, &done_of(&visual)), Ok(()));
        assert_eq!(
            step_outcomes(visual)[1],
            ("edit.undo".to_owned(), "refused".to_owned())
        );
    }

    /// D7 as built: a click on a cross-reference is the Select tool's follow and dispatches the canvas's press and
    /// release, so a canvas click done after which another page shows counts as `nav.follow`; a click that leaves the
    /// page counts as nothing more; the prelude's lines give the page before the first step.
    #[test]
    fn a_canvas_click_that_changes_the_page_is_a_follow() {
        let lines = [
            r#"{"step":1,"action":"page.goto","outcome":"done","reason":null,"state":{"page":"101"}}"#,
            r#"{"step":2,"input":{"input":"click"},"dispatched":[{"action":"canvas.press","outcome":"done","reason":null},{"action":"canvas.release","outcome":"done","reason":null}],"state":{"page":"102"}}"#,
            r#"{"step":3,"input":{"input":"click"},"dispatched":[{"action":"canvas.release","outcome":"done","reason":null}],"state":{"page":"102"}}"#,
            r#"{"step":4,"action":"nav.back","outcome":"done","reason":null,"state":{"page":"101"}}"#,
            r#"{"step":5,"action":"canvas.activate","outcome":"done","reason":null,"state":{"page":"102"}}"#,
        ];
        assert_eq!(
            done_actions(&agent_outcomes(&lines, 1)),
            [
                "canvas.press",
                "canvas.release",
                "nav.follow",
                "canvas.release",
                "nav.back",
                "canvas.activate",
                "nav.follow"
            ]
        );
    }

    /// UR-2, D7: [`decide_items`] decides an `actions` item from the actions, a query item from its rows and an output
    /// item from the outputs, in the items' order.
    #[test]
    fn decide_items_reads_the_actions_the_queries_and_the_outputs() {
        let tmp = crate::blind::testkit::Tmp::new("brief-decide");
        let acceptance: Acceptance = toml::from_str(
            "[[item]]\nid = \"A1\"\nkind = \"hard\"\nwhat = \"x\"\nhow = \"outcomes\"\nactions = [\"nav.follow\", \"nav.back\"]\n\n\
             [[item]]\nid = \"A2\"\nkind = \"hard\"\nwhat = \"y\"\nhow = \"connections\"\nquery = \"SELECT 1\"\nexpect = \">= 2\"\n\n\
             [[item]]\nid = \"A3\"\nkind = \"hard\"\nwhat = \"z\"\nhow = \"pdf\"\n",
        )
        .unwrap();
        let mut asked = Vec::new();
        let mut fake = |args: &[&str]| -> Result<(Option<i32>, String)> {
            asked.push(args.join(" "));
            Ok((Some(0), "n\n1\n".to_owned()))
        };
        let done = vec!["nav.follow".to_owned(), "nav.back".to_owned()];
        let d = decide_items(&acceptance, &tmp.0, &done, &mut fake).unwrap();
        assert_eq!(
            d,
            [
                Ok(()),
                Err("the query returns 1 row(s), expected >= 2".to_owned()),
                Err("no PDF was exported into the work folder".to_owned())
            ]
        );
        std::fs::write(tmp.0.join("job.pdf"), "%PDF").unwrap();
        let d = decide_items(&acceptance, &tmp.0, &done[..1], &mut fake).unwrap();
        assert!(d[0].is_err() && d[2].is_ok(), "{d:?}");
        assert_eq!(asked[2], "sql project SELECT 1 --format csv");
    }

    /// K9: a start other than `gp-ut` or `golden-a` makes the brief not runnable; a task's hints are one to three and in
    /// the user's words.
    #[test]
    fn k9_a_start_project_is_gp_ut_or_golden_a_and_the_hints_are_words() {
        let (tmp, dir) = copy_brief("brief-valve-switch");
        let t = std::fs::read_to_string(dir.join("task.toml")).unwrap();
        std::fs::write(dir.join("task.toml"), format!("{t}start = \"golden-b\"\n")).unwrap();
        std::fs::write(
            dir.join("hints.toml"),
            "[[hint]]\ntext = \"Use file.save.\"\n[[hint]]\ntext = \"b\"\n[[hint]]\ntext = \"c\"\n[[hint]]\ntext = \"d\"\n",
        )
        .unwrap();
        let (c, _) = check_files(&tmp.0, "brief-valve-switch").unwrap();
        assert_eq!(
            c.problems,
            [
                "task.toml: `start` is `gp-ut` or `golden-a`",
                "hints.toml: one to three hints",
                "hints.toml names the action file.save: a hint is in a user's words",
            ]
        );
        for start in STARTS {
            std::fs::write(dir.join("task.toml"), format!("{t}start = \"{start}\"\n")).unwrap();
            std::fs::write(dir.join("hints.toml"), "[[hint]]\ntext = \"Look at the page.\"\n").unwrap();
            let (c, _) = check_files(&tmp.0, "brief-valve-switch").unwrap();
            assert!(c.problems.is_empty(), "{start}: {:?}", c.problems);
        }
    }

    /// The usability tasks (`tools/blind/tasks/brief-ut-*`), however many there are.
    fn usability_tasks(root: &Path) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(root.join("tools/blind/tasks"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("brief-ut-"))
            .collect();
        out.sort();
        out
    }

    /// UR-8 and §9.3 item 1: every committed usability task is runnable on the build, every hard item met by its
    /// reference solution from its start project. Needs `elektrawatt` and `ew` built in `target/debug`; without them
    /// the test says so and checks nothing.
    #[test]
    fn the_usability_tasks_replay_on_the_build() {
        let root = crate::workspace_root();
        let bin = root.join("target/debug");
        let built = ["elektrawatt", "ew"]
            .iter()
            .all(|n| bin.join(format!("{n}{}", std::env::consts::EXE_SUFFIX)).is_file());
        if !built {
            eprintln!("ur8: elektrawatt and ew are not built in target/debug; the replay is not checked");
            return;
        }
        let tmp = crate::blind::testkit::Tmp::new("ur8-check");
        for task in usability_tasks(&root) {
            let c = check_brief_in(&root, &task, &bin, &tmp.0).unwrap();
            assert!(c.runnable(), "{}", c.text(&task));
            assert!(
                c.items
                    .iter()
                    .filter(|(_, k, _)| k == "hard")
                    .all(|(_, _, d)| d == "met"),
                "{}",
                c.text(&task)
            );
        }
    }

    /// Two checks of one brief never share a scratch folder, in one process or under one parent.
    #[test]
    fn every_check_gets_its_own_scratch_folder() {
        let parent = Path::new("parent");
        let (a, b) = (scratch(parent, "brief-x"), scratch(parent, "brief-x"));
        assert_ne!(a, b);
        assert_eq!(a.parent(), Some(parent));
    }

    /// BV9 and the feasibility rule: the reference solutions replayed on the built product. Both briefs are runnable
    /// (every hard item met, the soft ones met or marked infeasible): `brief-relays-valves`, whose
    /// `place.pick function=<tag>:13-14` puts each relay's contact on its relay's auxiliary slot
    /// (the contactor's contacts are template slots and `device=<tag>` fills the first, 1-2).
    /// The other is `brief-plc-io`.
    /// Needs `elektrawatt` and `ew` built in `target/debug` (the workspace run builds them); without them the test says
    /// so and checks nothing. Its scratch folders live in a temporary folder of its own, removed after it, so a parallel
    /// run shares none of them.
    #[test]
    fn check_brief_replays_the_reference_solutions() {
        let root = crate::workspace_root();
        let bin = root.join("target/debug");
        let built = ["elektrawatt", "ew"]
            .iter()
            .all(|n| bin.join(format!("{n}{}", std::env::consts::EXE_SUFFIX)).is_file());
        if !built {
            eprintln!("bv9: elektrawatt and ew are not built in target/debug; the replay is not checked");
            return;
        }
        let tmp = crate::blind::testkit::Tmp::new("bv9-check");
        for brief in ["brief-valve-switch", "brief-relays-valves", "brief-plc-io"] {
            let c = check_brief_in(&root, brief, &bin, &tmp.0).unwrap();
            assert!(c.runnable(), "{}", c.text(brief));
            assert!(
                c.items
                    .iter()
                    .filter(|(_, k, _)| k == "hard")
                    .all(|(_, _, d)| d == "met"),
                "{}",
                c.text(brief)
            );
        }
    }
}
