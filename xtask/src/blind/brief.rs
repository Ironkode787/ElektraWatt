// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The open design brief (WP-S6.13 part 4, visual spec §6.1 and §6.2) and `check-brief <task>`.
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
    let starter = || {
        std::fs::read_to_string(root.join("crates/ew-library/src/starter.rs"))
            .is_ok_and(|t| t.contains(value))
            || tree_holds(&root.join("content"), value)
    };
    let met = match form.trim() {
        "action" => actions.contains(value),
        "fndef" | "symbol" => starter(),
        "part" => std::fs::read_to_string(dir.join("prelude.uiscript")).is_ok_and(|t| t.contains(value)),
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
        for f in ["task.md", "task.toml", "acceptance.toml", "solution.uiscript"] {
            files.push((
                format!("tools/blind/tasks/{task}/{f}"),
                std::fs::read_to_string(dir.join(f))?,
            ));
        }
        c.problems.extend(crate::anonymise::check(&files, &denylist));
    }
    Ok((c, acceptance))
}

/// A scratch folder for one check of a brief.
fn scratch(task: &str) -> PathBuf {
    let token: String = super::keeper::new_token().chars().take(8).collect();
    std::env::temp_dir()
        .join("ew-blind-brief")
        .join(format!("{task}-{token}"))
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
    let scratch = scratch(task);
    let result = prove(root, &mut c, &acceptance, &prelude, &solution, &scratch, &exe);
    let _ = std::fs::remove_dir_all(&scratch);
    result?;
    Ok(c)
}

/// The replay and the decisions of [`check_brief`], in `scratch`.
fn prove(
    root: &Path,
    c: &mut BriefCheck,
    acceptance: &Acceptance,
    prelude: &[(String, Value)],
    solution: &[(String, Value)],
    scratch: &Path,
    exe: &dyn Fn(&str) -> PathBuf,
) -> Result<()> {
    for sub in ["work", "config", "empty-sample", "book", "out"] {
        std::fs::create_dir_all(scratch.join(sub))?;
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
    let (check_exit, _) = ew(root, &ew_exe, &["check", "project", "--format", "text"], &work)?;
    let (_, bom) = ew(
        root,
        &ew_exe,
        &["export", "bom", "project", "--mode", "parts", "--format", "csv"],
        &work,
    )?;
    for it in &acceptance.item {
        let decision: Result<(), String> = match (&it.query, &it.expect) {
            (Some(q), Some(e)) => {
                let (code, rows) = ew(root, &ew_exe, &["sql", "project", q, "--format", "csv"], &work)?;
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
            _ => {
                let hows: Vec<&str> = it.how.split(',').map(str::trim).collect();
                if hows.contains(&"pdf") {
                    match std::fs::metadata(work.join("project.pdf")) {
                        Ok(m) if m.len() > 0 => Ok(()),
                        _ => Err("the solution exported no project.pdf".to_owned()),
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
                    Err("no query decides it on the reference solution".to_owned())
                }
            }
        };
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
        // The starter library's index, where a `symbol:` requirement is looked up.
        std::fs::create_dir_all(tmp.0.join("crates/ew-library/src")).unwrap();
        std::fs::copy(
            root.join("crates/ew-library/src/starter.rs"),
            tmp.0.join("crates/ew-library/src/starter.rs"),
        )
        .unwrap();
        for e in std::fs::read_dir(task_dir(&root, brief)).unwrap().flatten() {
            std::fs::copy(e.path(), dir.join(e.file_name())).unwrap();
        }
        (tmp, dir)
    }

    /// BV9: the committed briefs pass the checks that need no product: files, `task.toml`, at least three hard items
    /// each with a `how`, a setup-only prelude, no action ID in the brief, and requirements the build meets.
    #[test]
    fn bv9_the_committed_briefs_have_valid_files() {
        let root = crate::workspace_root();
        for brief in ["brief-valve-switch", "brief-relays-valves"] {
            let (c, acceptance) = check_files(&root, brief).unwrap();
            assert!(c.problems.is_empty(), "{brief}: {:?}", c.problems);
            assert!(
                acceptance
                    .unwrap()
                    .item
                    .iter()
                    .filter(|i| i.kind == "hard")
                    .count()
                    >= 3
            );
        }
    }

    /// BV9: a brief with two hard items, a drawing line in its prelude, an action ID in its text, an unknown `how`
    /// and an unmet requirement is not runnable, and each problem is named.
    #[test]
    fn bv9_a_broken_brief_is_not_runnable_and_says_why() {
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

    /// BV9 and the feasibility rule: the reference solutions replayed on the built product. Both briefs are runnable
    /// (every hard item met, the soft ones met or marked infeasible): `brief-relays-valves` since WP-S2.18 part 1, whose
    /// `place.pick device=<tag>` puts each relay's contact on its relay (FT-599). Needs `elektrawatt` and `ew` built in
    /// `target/debug` (the workspace run builds them); without them the test says so and checks nothing.
    #[test]
    fn bv9_check_brief_replays_the_reference_solutions() {
        let root = crate::workspace_root();
        let bin = root.join("target/debug");
        let built = ["elektrawatt", "ew"]
            .iter()
            .all(|n| bin.join(format!("{n}{}", std::env::consts::EXE_SUFFIX)).is_file());
        if !built {
            eprintln!("bv9: elektrawatt and ew are not built in target/debug; the replay is not checked");
            return;
        }
        for brief in ["brief-valve-switch", "brief-relays-valves"] {
            let c = check_brief(&root, brief, &bin).unwrap();
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
