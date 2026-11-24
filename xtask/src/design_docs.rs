// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The rule of `cargo xtask rules` that a design document says what is (docs/design/04-architecture.md §19): no file of
//! `docs/design` holds a note headed "As built", a dated amendment (the word "amended" or "amendment" with a date on
//! its line) or the number of a backlog row, a decision of the owner's list or a work package (`FT-`, `ID-`, `WP-`); and
//! none is larger than [`CEILING`], but the documents read by identifier ([`BY_IDENTIFIER`]).

use std::path::Path;

use anyhow::{Context, Result};

/// The folder of the design documents.
const DESIGN: &str = "docs/design";

/// The largest design document, in bytes: the documents after their split are below it (80 KiB).
const CEILING: usize = 80 * 1024;

/// The documents read by identifier, which stay whole whatever their size.
const BY_IDENTIFIER: &[&str] = &[
    "02a-requirements-core.md",
    "02b-requirements-extended.md",
    "02c-nonfunctional-requirements.md",
    "glossary.md",
];

/// The violations of every design document.
pub(crate) fn check(root: &Path) -> Result<Vec<String>> {
    let dir = root.join(DESIGN);
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".md"))
        .collect();
    names.sort();
    let mut out = Vec::new();
    for name in names {
        let path = dir.join(&name);
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        if text.len() > CEILING && !BY_IDENTIFIER.contains(&name.as_str()) {
            out.push(format!(
                "{DESIGN}/{name}: {} KiB, over the {} KiB of a design document; split it where a chapter is read alone",
                text.len() / 1024,
                CEILING / 1024
            ));
        }
        out.extend(history(&format!("{DESIGN}/{name}"), &text));
    }
    Ok(out)
}

/// The lines of the document `rel` (its text `text`) that tell history, each with what it holds.
fn history(rel: &str, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let what = if line
            .trim_start_matches(|c: char| c.is_whitespace() || "#>*-_".contains(c))
            .starts_with("As built")
        {
            Some("a note headed \"As built\"".to_owned())
        } else if (line.contains("amended") || line.contains("amendment")) && has_date(line) {
            Some("a dated amendment".to_owned())
        } else {
            row_number(line).map(|n| format!("the number {n}"))
        };
        if let Some(what) = what {
            out.push(format!(
                "{rel}:{}: {what}; a design document says what is, and git holds how it came to be (§19)",
                i + 1
            ));
        }
    }
    out
}

/// Whether `line` holds a date `YYYY-MM-DD`.
fn has_date(line: &str) -> bool {
    let b = line.as_bytes();
    b.windows(10).any(|w| {
        w[..4].iter().all(u8::is_ascii_digit)
            && w[4] == b'-'
            && w[5..7].iter().all(u8::is_ascii_digit)
            && w[7] == b'-'
            && w[8..].iter().all(u8::is_ascii_digit)
    })
}

/// The first number of a backlog row, an item of the owner's list or a work package in `line` (`FT-1234`, `ID-1488`,
/// `WP-S2.18`), as it stands.
fn row_number(line: &str) -> Option<&str> {
    for prefix in ["FT-", "ID-", "WP-"] {
        let mut from = 0;
        while let Some(at) = line[from..].find(prefix) {
            let start = from + at;
            let after = &line[start + prefix.len()..];
            let word_start = start == 0 || !line[..start].ends_with(|c: char| c.is_alphanumeric() || c == '-');
            let digits = after.trim_start_matches('S');
            if word_start && digits.starts_with(|c: char| c.is_ascii_digit()) {
                let len = prefix.len()
                    + after
                        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '.'))
                        .unwrap_or(after.len());
                return Some(line[start..start + len].trim_end_matches('.'));
            }
            from = start + prefix.len();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::history;

    /// A note headed "As built", a dated amendment and a row's number are named by their line; the words "built"
    /// and "amendment" alone, a requirement's identifier and a date of another kind are not.
    #[test]
    fn history_is_named_by_its_line() {
        let text = "# Title\n\n**As built (0.3.5).** The grid.\nA rule (ADR-0034, amended 2026-11-20).\n\
                    The fix of FT-1936 and WP-S2.18.\nShipped as built-in exporters; ADR-0003 amendment 1.\n\
                    FR-PLC-021 and NFR-SEC-04, released 2026-10-20.\n";
        assert_eq!(
            history("docs/design/x.md", text),
            [
                "docs/design/x.md:3: a note headed \"As built\"; a design document says what is, and git holds how \
                 it came to be (§19)",
                "docs/design/x.md:4: a dated amendment; a design document says what is, and git holds how it came \
                 to be (§19)",
                "docs/design/x.md:5: the number FT-1936; a design document says what is, and git holds how it came \
                 to be (§19)",
            ]
        );
    }
}
