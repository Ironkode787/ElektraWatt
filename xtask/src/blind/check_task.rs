// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-task <task>`: checks a task folder without a product. The sheet `task.md` exists and holds no address or
//! path, the prelude's steps parse, and the sheet of a brief that starts on the sample ([`SAMPLE_SCRIPT`]) holds no
//! identifier-shaped token and no proper name that the sample's script does not hold: the script is the sheet's only
//! source (D4).
//!
//! A brief's replay and acceptance list are checked by `check-brief` ([`super::brief`]).

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, bail};

use super::score::{prelude_actions, task_dir};

/// The sample's script: the only source of the sheet of a brief that starts on the sample (`start = "sample"`).
pub const SAMPLE_SCRIPT: &str = "fixtures/sample/sample.uiscript";

// ---------------------------------------------------------------------------------------------------------------
// `check-task`.

/// Whether `w` is a word of the sheet that must be found in its source: an identifier (a letter and a digit together, a
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
/// begin a sentence, a table cell or a line. `doc` is the source's text, `source` its name in the problems (the
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
/// A file that cannot be read at all (a missing script of the sample, an unreadable folder).
pub fn check_task(root: &Path, task: &str) -> Result<Vec<String>> {
    let dir = task_dir(root, task);
    let mut problems: Vec<String> = Vec::new();
    if !dir.is_dir() {
        bail!("no task `{task}` (looked for {})", dir.display());
    }
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
    Ok(problems)
}

#[cfg(test)]
mod tests;
