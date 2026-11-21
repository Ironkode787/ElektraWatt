// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-task <task>`: checks a task folder without a product. The files exist and parse, the prelude's classes fit
//! its steps, the gaps name checks, and every identifier-shaped token of `task.md` occurs in the golden project's
//! description (doc 20), which is the sheet's only source (D4), and the sheet holds no other proper name. The sheet of
//! a brief that starts on the sample ([`SAMPLE_SCRIPT`]) is held to the sample's script the same way.
//!
//! The oracle of a gate task is made by `bless-oracle` ([`super::oracle`]); a brief's replay and acceptance list are
//! checked by `check-brief` ([`super::brief`]).

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, bail};

use super::compose::filter_folios;
use super::oracle::{BOM_HEADER, header_of};
use super::score::{
    CountsFile, GapsFile, PreludeFile, QUERIES, SOFT, parse_csv, parse_queries, prelude_actions, task_dir,
};

/// The sample's script: the only source of the sheet of a brief that starts on the sample (`start = "sample"`).
pub const SAMPLE_SCRIPT: &str = "fixtures/sample/sample.uiscript";

/// The design document that is the task sheet's only source.
pub const DOC20: &str = "docs/design/20-golden-project.md";

// ---------------------------------------------------------------------------------------------------------------
// `check-task`.

/// Whether `w` is a word of the sheet that must be found in doc 20: an identifier (a letter and a digit together, a
/// folio of 3 digits or more, a sign-prefixed designation, an all-capitals code of 2 letters or more).
fn identifier_shaped(w: &str) -> bool {
    let has_digit = w.chars().any(|c| c.is_ascii_digit());
    let has_letter = w.chars().any(|c| c.is_ascii_alphabetic());
    let all_digits = w.chars().all(|c| c.is_ascii_digit());
    let signed =
        w.starts_with(['-', '+', '=']) && w.chars().nth(1).is_some_and(|c| c.is_ascii_alphanumeric());
    let caps = w.chars().filter(|c| c.is_ascii_alphabetic()).count() >= 2
        && w.chars()
            .filter(|c| c.is_ascii_alphabetic())
            .all(|c| c.is_ascii_uppercase());
    (has_digit && has_letter) || (all_digits && w.len() >= 3) || signed || caps
}

/// Whether `w` is a capitalised word (`Eaton`): an upper-case letter then lower-case letters only, 2 or more.
fn capitalised(w: &str) -> bool {
    let mut it = w.chars();
    it.next().is_some_and(|c| c.is_ascii_uppercase()) && w.len() >= 2 && it.all(|c| c.is_ascii_lowercase())
}

fn trim_word(w: &str) -> &str {
    w.trim_matches(|c: char| {
        matches!(
            c,
            '(' | ')'
                | '"'
                | '\''
                | '`'
                | '*'
                | '['
                | ']'
                | ','
                | ';'
                | '.'
                | ':'
                | '!'
                | '?'
                | '“'
                | '”'
                | '_'
        )
    })
}

/// The words of the sheet that its source does not hold: identifier-shaped tokens, and capitalised words that do not
/// begin a sentence, a table cell or a line. `doc` is the source's text, `source` its name in the problems (doc 20, the
/// sample's script); both searches are case-sensitive.
pub fn sheet_problems(sheet: &str, doc: &str, source: &str) -> Vec<String> {
    // Text that is not the sheet's own words: comments and the placeholders of `compose`.
    let mut text = String::new();
    let mut rest = sheet;
    while let Some(i) = rest.find("<!--") {
        text.push_str(&rest[..i]);
        match rest[i..].find("-->") {
            Some(j) => rest = &rest[i + j + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    text.push_str(rest);
    let doc_words: BTreeSet<&str> = doc
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| !w.is_empty())
        .collect();
    let mut problems: BTreeSet<String> = BTreeSet::new();
    for line in text.lines() {
        let cells: Vec<&str> = if line.trim_start().starts_with('|') {
            line.trim().trim_matches('|').split('|').collect()
        } else {
            vec![line]
        };
        for cell in cells {
            if cell.trim().chars().all(|c| matches!(c, '-' | ':' | ' ')) {
                continue;
            }
            let mut sentence_start = true;
            for raw in cell.split_whitespace() {
                let raw = raw.trim_start_matches(['#', '*', '>']);
                let word = trim_word(raw);
                let starts = sentence_start;
                sentence_start = raw.trim_end_matches(['*', '_']).ends_with(['.', ':', '!', '?'])
                    || matches!(raw, "-" | "*" | "#" | "");
                if word.is_empty() || (word.starts_with('{') && word.ends_with('}')) {
                    continue;
                }
                // `101-B2,` and `S51:1/a` etc.: the word as written, trailing punctuation off.
                if identifier_shaped(word) && !doc.contains(word) {
                    problems.insert(format!("`{word}` is not in {source}"));
                } else if !starts && capitalised(word) && !doc_words.contains(word) {
                    problems.insert(format!("`{word}` is a name that {source} does not hold"));
                }
            }
        }
    }
    for bad in ["@", "://", "\\", "C:"] {
        if text.contains(bad) {
            problems.insert(format!("the sheet holds `{bad}` (an address or a path)"));
        }
    }
    problems.into_iter().collect()
}

/// Checks the folder of a task (§5.7): returns the problems, one per line (empty: the task is sound).
///
/// # Errors
///
/// A file that cannot be read at all (a missing doc 20, an unreadable folder).
pub fn check_task(root: &Path, task: &str) -> Result<Vec<String>> {
    let dir = task_dir(root, task);
    let mut problems: Vec<String> = Vec::new();
    if !dir.is_dir() {
        bail!("no task `{task}` (looked for {})", dir.display());
    }
    // A task without an oracle (`free`) has a sheet and perhaps a prelude, and is not scored: no extract of doc 20.
    if !dir.join("oracle").is_dir() {
        match std::fs::read_to_string(dir.join("task.md")) {
            Err(_) => problems.push("task.md is missing".to_owned()),
            Ok(sheet) => {
                for bad in ["@", "://", "\\"] {
                    if sheet.contains(bad) {
                        problems.push(format!("task.md holds `{bad}` (an address or a path)"));
                    }
                }
                // A brief that starts on the sample: its sheet's tokens come from the sample's script.
                let starts_on_the_sample = std::fs::read_to_string(dir.join("task.toml"))
                    .ok()
                    .and_then(|t| toml::from_str::<toml::Value>(&t).ok())
                    .is_some_and(|t| t.get("start").and_then(toml::Value::as_str) == Some("sample"));
                if starts_on_the_sample {
                    let script = std::fs::read_to_string(root.join(SAMPLE_SCRIPT))
                        .with_context(|| format!("reading {SAMPLE_SCRIPT}"))?;
                    // The title and the History name the task, its rounds and its versions, not the project.
                    let body: String = sheet
                        .replace("\r\n", "\n")
                        .split("\n## History")
                        .next()
                        .unwrap_or_default()
                        .lines()
                        .filter(|l| !l.starts_with("# "))
                        .map(|l| format!("{l}\n"))
                        .collect();
                    for p in sheet_problems(&body, &script, "the sample's script") {
                        problems.push(format!("task.md: {p}"));
                    }
                }
            }
        }
        if let Ok(p) = std::fs::read_to_string(dir.join("prelude.uiscript"))
            && let Err(e) = prelude_actions(&p)
        {
            problems.push(format!("prelude.uiscript: {e}"));
        }
        return Ok(problems);
    }
    let need = |rel: &str, problems: &mut Vec<String>| -> Option<String> {
        match std::fs::read_to_string(dir.join(rel)) {
            Ok(t) => Some(t.replace("\r\n", "\n")),
            Err(_) => {
                problems.push(format!("{rel} is missing"));
                None
            }
        }
    };
    let sheet = need("task.md", &mut problems);
    let prelude = need("prelude.uiscript", &mut problems);
    let counts: Option<CountsFile> =
        need("oracle/counts.toml", &mut problems).and_then(|t| match toml::from_str(&t) {
            Ok(c) => Some(c),
            Err(e) => {
                problems.push(format!("oracle/counts.toml: {e}"));
                None
            }
        });
    let prelude_toml: Option<PreludeFile> =
        need("prelude.toml", &mut problems).and_then(|t| match toml::from_str(&t) {
            Ok(c) => Some(c),
            Err(e) => {
                problems.push(format!("prelude.toml: {e}"));
                None
            }
        });
    let gaps: Option<GapsFile> = need("gaps.toml", &mut problems).and_then(|t| match toml::from_str(&t) {
        Ok(c) => Some(c),
        Err(e) => {
            problems.push(format!("gaps.toml: {e}"));
            None
        }
    });
    if need("oracle/bom.csv", &mut problems)
        .is_some_and(|t| parse_csv(&t).first().map(Vec::as_slice) != Some(&BOM_HEADER.map(str::to_owned)[..]))
    {
        problems.push(
            "oracle/bom.csv: the header is not Pos,Qty,Unit,Manufacturer,Part number,Description".to_owned(),
        );
    }
    if let Some(sql) = need("oracle/rows.sql", &mut problems) {
        match parse_queries(&sql) {
            Ok(q) => {
                for name in QUERIES {
                    if !q.contains_key(name) {
                        problems.push(format!("oracle/rows.sql has no query `{name}`"));
                    }
                }
            }
            Err(e) => problems.push(e.to_string()),
        }
    }
    for (_, set, _) in SOFT {
        if let Some(text) = need(&format!("oracle/rows/{set}.csv"), &mut problems) {
            let rows = parse_csv(&text);
            let header: Vec<String> = header_of(set).iter().map(|s| (*s).to_owned()).collect();
            if rows.first() != Some(&header) {
                problems.push(format!(
                    "oracle/rows/{set}.csv: the header is not {}",
                    header.join(",")
                ));
            }
            if rows.iter().any(|r| r.len() != header.len()) {
                problems.push(format!(
                    "oracle/rows/{set}.csv: a row has not {} columns",
                    header.len()
                ));
            }
        }
    }
    let folios: Vec<String> = counts
        .as_ref()
        .map(|c| c.folio.iter().map(|f| f.folio.clone()).collect())
        .unwrap_or_default();
    if let Some(c) = &counts {
        if c.folio.is_empty() {
            problems.push("oracle/counts.toml holds no folio".to_owned());
        }
        let unique: BTreeSet<&String> = folios.iter().collect();
        if unique.len() != folios.len() {
            problems.push("oracle/counts.toml repeats a folio".to_owned());
        }
        for f in &c.folio {
            if f.source.trim().is_empty() {
                problems.push(format!("oracle/counts.toml: folio {} names no source", f.folio));
            }
        }
        for m in &c.marker {
            if !folios.contains(&m.folio) {
                problems.push(format!(
                    "oracle/counts.toml: a marker on folio {}, which has no counts",
                    m.folio
                ));
            }
        }
    }
    // The prelude: one entry per step, the actions agree, a drawing line names its follow-up.
    if let (Some(p), Some(t)) = (&prelude, &prelude_toml) {
        match prelude_actions(p) {
            Err(e) => problems.push(format!("prelude.uiscript: {e}")),
            Ok(actions) => {
                if actions.len() != t.step.len() {
                    problems.push(format!(
                        "prelude.toml has {} entries for {} prelude steps",
                        t.step.len(),
                        actions.len()
                    ));
                }
                for (i, (a, e)) in actions.iter().zip(&t.step).enumerate() {
                    if *a != e.action {
                        problems.push(format!(
                            "prelude.toml entry {}: action `{}`, the step is `{a}`",
                            i + 1,
                            e.action
                        ));
                    }
                }
            }
        }
    }
    if let Some(t) = &prelude_toml {
        for (i, e) in t.step.iter().enumerate() {
            match (e.class.as_str(), &e.gap) {
                ("setup", None) => {}
                ("setup", Some(_)) => {
                    problems.push(format!("prelude.toml entry {}: a setup line has no gap", i + 1))
                }
                ("drawing", Some(g)) if valid_gap(g) => {}
                ("drawing", _) => problems.push(format!(
                    "prelude.toml entry {}: a drawing line names its follow-up",
                    i + 1
                )),
                (other, _) => problems.push(format!(
                    "prelude.toml entry {}: class `{other}` is setup or drawing",
                    i + 1
                )),
            }
        }
    }
    // The gaps: known checks, a follow-up or a package part, a reason.
    if let Some(g) = &gaps {
        let hard = folios.len() + 4;
        let known: BTreeSet<String> = (1..=hard)
            .map(|i| format!("H{i}"))
            .chain(SOFT.iter().map(|(id, _, _)| (*id).to_owned()))
            .collect();
        let mut seen = BTreeSet::new();
        for e in &g.gap {
            if !known.contains(&e.check) {
                problems.push(format!("gaps.toml: `{}` is not a check of this task", e.check));
            }
            if !seen.insert(e.check.clone()) {
                problems.push(format!("gaps.toml: `{}` is listed twice", e.check));
            }
            if !valid_gap(&e.gap) {
                problems.push(format!(
                    "gaps.toml: `{}` is neither a follow-up (FT-n) nor a package part (WP-…)",
                    e.gap
                ));
            }
            if e.reason.trim().is_empty() {
                problems.push(format!("gaps.toml: `{}` has no reason", e.check));
            }
        }
    }
    // The sheet: balanced folio blocks for the oracle's folios, and every token in doc 20.
    if let Some(s) = &sheet {
        match filter_folios(s, &[]) {
            Ok(_) => {}
            Err(e) => problems.push(format!("task.md: {e}")),
        }
        for line in s.lines() {
            if let Some(f) = line
                .trim()
                .strip_prefix("<!-- folio ")
                .and_then(|r| r.strip_suffix(" -->"))
                && !folios.iter().any(|x| x == f.trim())
                && !folios.is_empty()
            {
                problems.push(format!(
                    "task.md: a block for folio {}, which the oracle has no counts for",
                    f.trim()
                ));
            }
        }
        let doc = std::fs::read_to_string(root.join(DOC20))
            .with_context(|| format!("reading {DOC20}"))?
            .replace("\r\n", "\n");
        for p in sheet_problems(s, &doc, "doc 20") {
            problems.push(format!("task.md: {p}"));
        }
    }
    Ok(problems)
}

#[cfg(test)]
mod tests;

/// Whether `g` is a follow-up (`FT-408`) or a package part (`WP-S2.07`, `WP-S2.04 part 4b`).
pub fn valid_gap(g: &str) -> bool {
    let ft = g
        .strip_prefix("FT-")
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    ft || g.starts_with("WP-S")
}
