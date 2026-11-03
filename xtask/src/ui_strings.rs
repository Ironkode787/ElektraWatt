// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-ui-strings` (WP-S2.08 D11, NFR-I18N-02): no string literal reaches an egui text call in `crates/ew-ui/src`
//! except the allowlist `ci/ui-strings-allow.txt`. Every shell text comes from a Fluent key (`strings::text`).
//!
//! The scan is lexical. A text call is `label(`, `button(`, `on_hover_text(`, `RichText::new(` or a `WidgetInfo`
//! name (`WidgetInfo::labeled(`, `WidgetInfo::selected(`); every string literal inside its parentheses is checked,
//! except the key of `key!`, `count_key!` and `Key::run_time`. A literal that holds only `format!` placeholders
//! (`"{}"`, `"{a}{b}"`) passes; another passes when the allowlist holds it exactly as written in the source (one per
//! line in its quotes, `#` comments). A `#[cfg(test)]` module, comments and the test tool's own files are not scanned.

use std::path::Path;

use anyhow::{Context, Result};

/// The allowlist, relative to the workspace root.
pub(crate) const ALLOW: &str = "ci/ui-strings-allow.txt";
/// The scanned directory.
const SCANNED: &str = "crates/ew-ui/src";

/// The calls whose arguments reach a text of the window.
const TEXT_CALLS: &[&str] = &["label", "button", "on_hover_text"];
/// The constructors `<type>::<fn>` whose arguments reach a text of the window.
const TEXT_PATHS: &[(&str, &str)] = &[
    ("RichText", "new"),
    ("WidgetInfo", "labeled"),
    ("WidgetInfo", "selected"),
];
/// The calls whose string arguments are text keys, not texts.
const KEY_CALLS: &[&str] = &["key", "count_key", "run_time"];

#[derive(Clone, Debug, PartialEq)]
enum Tok<'s> {
    Ident(&'s str),
    /// A string literal as written between its quotes, and its line.
    Str(&'s str, usize),
    Punct(char),
}

/// The tokens of `src` that matter here: identifiers, string literals and punctuation; comments, char literals,
/// lifetimes and numbers are dropped. The code after a line `#[cfg(test)]` is not read (the test module ends a file).
fn tokens(src: &str) -> Vec<Tok<'_>> {
    let src = src.find("\n#[cfg(test)]").map_or(src, |at| &src[..at]);
    let b = src.as_bytes();
    let mut out = Vec::new();
    let (mut i, mut line) = (0, 1);
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            line += 1;
            i += 1;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                line += usize::from(b[i] == b'\n');
                i += 1;
            }
            i += 2;
        } else if c == b'"' || (c == b'r' && b[i + 1..].iter().find(|x| **x != b'#') == Some(&b'"')) {
            // A raw string `r#"…"#` or a plain one with escapes.
            let (start_line, hashes) = (
                line,
                if c == b'r' {
                    let mut h = 0;
                    i += 1;
                    while b.get(i) == Some(&b'#') {
                        h += 1;
                        i += 1;
                    }
                    Some(h)
                } else {
                    None
                },
            );
            i += 1;
            let start = i;
            loop {
                if i >= b.len() {
                    break;
                }
                match (b[i], hashes) {
                    (b'\\', None) => i += 2,
                    (b'"', None) => break,
                    (b'"', Some(h)) if b[i + 1..].iter().take(h).filter(|x| **x == b'#').count() == h => {
                        break;
                    }
                    (b'\n', _) => {
                        line += 1;
                        i += 1;
                    }
                    _ => i += 1,
                }
            }
            let end = i.min(b.len());
            out.push(Tok::Str(&src[start..end], start_line));
            i = end + 1 + hashes.unwrap_or(0);
        } else if c == b'\'' {
            // A char literal ('x', '\n', '\u{..}') or a lifetime ('a).
            let close = if b.get(i + 1) == Some(&b'\\') {
                b[i + 3..].iter().position(|x| *x == b'\'').map(|p| i + 3 + p)
            } else if b.get(i + 2) == Some(&b'\'') {
                Some(i + 2)
            } else {
                src[i + 1..]
                    .char_indices()
                    .nth(1)
                    .filter(|(_, ch)| *ch == '\'')
                    .map(|(p, _)| i + 1 + p)
            };
            i = close.map_or(i + 1, |c| c + 1);
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(Tok::Ident(&src[start..i]));
        } else if c.is_ascii_digit() {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'.') {
                i += 1;
            }
        } else if c.is_ascii() {
            if !c.is_ascii_whitespace() {
                out.push(Tok::Punct(char::from(c)));
            }
            i += 1;
        } else {
            i += src[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

/// Whether the literal `lit` holds nothing but `format!` placeholders.
fn only_placeholders(lit: &str) -> bool {
    let mut rest = lit;
    while let Some(open) = rest.find('{') {
        if !rest[..open].is_empty() {
            return false;
        }
        match rest[open..].find('}') {
            Some(close) => rest = &rest[open + close + 1..],
            None => return false,
        }
    }
    rest.is_empty()
}

/// The literals of `src` that reach a text call and are not allowed: (line, literal).
fn findings<'s>(src: &'s str, allow: &[String]) -> Vec<(usize, &'s str)> {
    let toks = tokens(src);
    let mut out = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        let Tok::Ident(name) = t else { continue };
        if toks.get(i + 1) != Some(&Tok::Punct('(')) {
            continue;
        }
        let path = |ty: &str| {
            i >= 3
                && toks[i - 1] == Tok::Punct(':')
                && toks[i - 2] == Tok::Punct(':')
                && toks[i - 3] == Tok::Ident(ty)
        };
        let is_call = (TEXT_CALLS.contains(name) && i > 0 && toks[i - 1] == Tok::Punct('.'))
            || TEXT_PATHS.iter().any(|(ty, f)| f == name && path(ty));
        if !is_call {
            continue;
        }
        // The arguments: up to the matching `)`; the keys inside `key!(…)` and `run_time(…)` are skipped.
        let mut depth = 0usize;
        let mut key_depth: Option<usize> = None;
        let mut j = i + 1;
        while j < toks.len() {
            match &toks[j] {
                Tok::Punct('(' | '[' | '{') => depth += 1,
                Tok::Punct(')' | ']' | '}') => {
                    depth -= 1;
                    if key_depth.is_some_and(|d| depth < d) {
                        key_depth = None;
                    }
                    if depth == 0 {
                        break;
                    }
                }
                Tok::Ident(k) if key_depth.is_none() && KEY_CALLS.contains(k) => {
                    let opens = |n: usize| matches!(toks.get(n), Some(Tok::Punct('(')));
                    if opens(j + 1) || (toks.get(j + 1) == Some(&Tok::Punct('!')) && opens(j + 2)) {
                        key_depth = Some(depth + 1);
                    }
                }
                Tok::Str(lit, line)
                    if key_depth.is_none() && !only_placeholders(lit) && !allow.iter().any(|a| a == lit) =>
                {
                    out.push((*line, *lit));
                }
                _ => {}
            }
            j += 1;
        }
    }
    out
}

/// The `.rs` files under `dir`, sorted.
fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            rust_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out.sort();
    Ok(())
}

/// The allowlist's entries: one literal per line in its quotes, as written in the source; blank lines and `#`
/// comments skipped.
fn allowlist(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            l.strip_prefix('"')
                .and_then(|l| l.strip_suffix('"'))
                .unwrap_or(l)
                .to_owned()
        })
        .collect()
}

/// The violations of `check-ui-strings` in the repository at `root`: each literal with its file and line.
pub(crate) fn check(root: &Path) -> Result<Vec<String>> {
    let allow_path = root.join(ALLOW);
    let allow = allowlist(
        &std::fs::read_to_string(&allow_path).with_context(|| format!("reading {}", allow_path.display()))?,
    );
    let mut files = Vec::new();
    rust_files(&root.join(SCANNED), &mut files)?;
    let mut out = Vec::new();
    for path in files {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        // The test tool's frames and messages (`--visual`) reach no egui text call (ID-1219 ruling 7).
        if rel.ends_with("/visual.rs") {
            continue;
        }
        let src = std::fs::read_to_string(&path).with_context(|| format!("reading {rel}"))?;
        for (line, lit) in findings(&src, &allow) {
            out.push(format!(
                "{rel}:{line}: the literal \"{lit}\" reaches an egui text call; give it a key in \
                 crates/ew-viewmodel/locales/en-GB/shell.ftl or, for a symbol, list it in {ALLOW}"
            ));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(src: &str) -> Vec<(usize, &str)> {
        findings(src, &["▸".to_owned()])
    }

    /// LO-6: a literal in a label is found; the fixture of the acceptance.
    #[test]
    fn a_literal_label_is_found() {
        assert_eq!(
            found("fn f(ui: &mut Ui) {\n    ui.label(\"Save\");\n}\n"),
            [(2, "Save")]
        );
    }

    /// Every text call is read, a literal inside `format!` too; keys, placeholders, allowed symbols, comments, other
    /// calls and the test module are not.
    #[test]
    fn the_calls_keys_and_exceptions() {
        let src = r##"
fn f(ui: &mut Ui) {
    ui.button(format!("{} items", n));
    let _ = r.on_hover_text("Tip");
    RichText::new("Big");
    WidgetInfo::labeled(WidgetType::Button, true, "Name");
    WidgetInfo::selected(WidgetType::Checkbox, true, on, r#"Raw "x""#);
    ui.label(strings::text(key!("menu-file")));
    ui.label(strings::text(Key::run_time(&format!("tool-{name}"))));
    ui.label(format!("{a}{b}"));
    ui.label("▸");
    // ui.label("Comment");
    let c = '"'; let q = '\''; let l: &'static str = "x"; field_label("device", "tag");
    egui::Id::new("id");
}
#[cfg(test)]
mod tests { fn t(ui: &mut Ui) { ui.label("Test"); } }
"##;
        assert_eq!(
            found(src),
            [
                (3, "{} items"),
                (4, "Tip"),
                (5, "Big"),
                (6, "Name"),
                (7, r#"Raw "x""#),
            ]
        );
    }

    #[test]
    fn placeholders_alone_pass() {
        assert!(only_placeholders("{}"));
        assert!(only_placeholders("{a}{b:>3}"));
        assert!(only_placeholders(""));
        assert!(!only_placeholders("{a} {b}"));
        assert!(!only_placeholders("{} items"));
    }

    /// The tree passes.
    #[test]
    fn the_tree_passes() {
        assert_eq!(check(&crate::workspace_root()).unwrap(), Vec::<String>::new());
    }
}
