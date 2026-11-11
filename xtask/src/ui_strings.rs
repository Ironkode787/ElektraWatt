// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The UI-text rule of `cargo xtask rules` (NFR-I18N-02): no string literal reaches an egui text call in `crates/ew-ui/src`
//! except the allowlist `ci/ui-strings-allow.txt`. Every shell text comes from a Fluent key (`strings::text`).
//!
//! The scan is lexical. A text call is a method `label(`, `button(`, `small_button(`, `selectable_label(`,
//! `checkbox(`, `heading(`, `menu_button(`, `collapsing(`, `hint_text(` or `on_hover_text(`, a painter's `text(`
//! (`painter().text(`, `painter.text(`), a constructor `RichText::new(`, `Button::new(` or `Label::new(`, or a
//! `WidgetInfo` name (`WidgetInfo::labeled(`, `WidgetInfo::selected(`); every string literal inside its parentheses
//! is checked (of `menu_button(` and `collapsing(` only the first argument, the title, not the closure),
//! except the key of `key!`, `count_key!` and `Key::run_time`. A literal bound to a variable first and
//! passed by name is out of its reach. A literal that holds only `format!` placeholders
//! (`"{}"`, `"{a}{b}"`) passes; another passes when the allowlist holds it exactly as written in the source (one per
//! line in its quotes, `#` comments). A `#[cfg(test)]` module, comments and the test tool's own files are not scanned.
//! An allowlist entry that no such literal matches is stale and fails the check, named.

use std::path::Path;

use anyhow::{Context, Result};

/// The allowlist, relative to the workspace root.
pub(crate) const ALLOW: &str = "ci/ui-strings-allow.txt";
/// The scanned directory.
const SCANNED: &str = "crates/ew-ui/src";

/// The calls whose arguments reach a text of the window.
const TEXT_CALLS: &[&str] = &[
    "label",
    "button",
    "small_button",
    "selectable_label",
    "checkbox",
    "heading",
    "menu_button",
    "collapsing",
    "hint_text",
    "on_hover_text",
];
/// The constructors `<type>::<fn>` whose arguments reach a text of the window.
const TEXT_PATHS: &[(&str, &str)] = &[
    ("RichText", "new"),
    ("Button", "new"),
    ("Label", "new"),
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

/// The literals of `src` that reach a text call and are not allowed: (line, literal). Each entry of `allow` that such a
/// literal matches is marked in `used`, index for index.
fn findings<'s>(src: &'s str, allow: &[String], used: &mut [bool]) -> Vec<(usize, &'s str)> {
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
        let method = i > 0 && toks[i - 1] == Tok::Punct('.');
        // A painter's `text(`: `painter().text(` or `painter.text(`.
        let painter = *name == "text"
            && method
            && (toks.get(i.wrapping_sub(2)) == Some(&Tok::Ident("painter"))
                || (i >= 4
                    && toks[i - 2] == Tok::Punct(')')
                    && toks[i - 3] == Tok::Punct('(')
                    && toks[i - 4] == Tok::Ident("painter")));
        let is_call = (TEXT_CALLS.contains(name) && method)
            || painter
            || TEXT_PATHS.iter().any(|(ty, f)| f == name && path(ty));
        if !is_call {
            continue;
        }
        // The arguments: up to the matching `)`; the keys inside `key!(…)` and `run_time(…)` are skipped. Of
        // `menu_button(` and `collapsing(` only the title, up to the first comma: the closure's calls are their own.
        let title_only = method && matches!(*name, "menu_button" | "collapsing");
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
                Tok::Punct(',') if title_only && depth == 1 => break,
                Tok::Ident(k) if key_depth.is_none() && KEY_CALLS.contains(k) => {
                    let opens = |n: usize| matches!(toks.get(n), Some(Tok::Punct('(')));
                    if opens(j + 1) || (toks.get(j + 1) == Some(&Tok::Punct('!')) && opens(j + 2)) {
                        key_depth = Some(depth + 1);
                    }
                }
                Tok::Str(lit, line) if key_depth.is_none() && !only_placeholders(lit) => {
                    match allow.iter().position(|a| a == lit) {
                        Some(k) => used[k] = true,
                        None => out.push((*line, *lit)),
                    }
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

/// The violations of the UI-text rule in the repository at `root`: each literal with its file and line.
pub(crate) fn check(root: &Path) -> Result<Vec<String>> {
    let allow_path = root.join(ALLOW);
    let allow = allowlist(
        &std::fs::read_to_string(&allow_path).with_context(|| format!("reading {}", allow_path.display()))?,
    );
    let mut files = Vec::new();
    rust_files(&root.join(SCANNED), &mut files)?;
    let mut out = Vec::new();
    let mut used = vec![false; allow.len()];
    for path in files {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        // The test tool's frames and messages (`--visual`) reach no egui text call.
        if rel.ends_with("/visual.rs") {
            continue;
        }
        let src = std::fs::read_to_string(&path).with_context(|| format!("reading {rel}"))?;
        for (line, lit) in findings(&src, &allow, &mut used) {
            out.push(format!(
                "{rel}:{line}: the literal \"{lit}\" reaches an egui text call; give it a key in \
                 crates/ew-viewmodel/locales/en-GB/shell.ftl or, for a symbol, list it in {ALLOW}"
            ));
        }
    }
    out.extend(stale(&allow, &used));
    Ok(out)
}

/// The entries of `allow` that no text call's literal matched (`used` false): a stale entry, named.
fn stale(allow: &[String], used: &[bool]) -> Vec<String> {
    allow
        .iter()
        .zip(used)
        .filter(|(_, used)| !**used)
        .map(|(entry, _)| {
            format!("{ALLOW}: the entry \"{entry}\" matches no egui text call's literal; remove it")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(src: &str) -> Vec<(usize, &str)> {
        findings(src, &["▸".to_owned()], &mut [false])
    }

    /// An entry a text call's literal matches is used; one no literal matches is stale and named.
    #[test]
    fn a_stale_allowlist_entry_is_named() {
        let allow = ["▸".to_owned(), "◂".to_owned()];
        let mut used = [false, false];
        let src = "fn f(ui: &mut Ui) {\n    ui.label(\"▸\");\n}\n";
        assert_eq!(findings(src, &allow, &mut used), []);
        assert_eq!(used, [true, false]);
        assert_eq!(
            stale(&allow, &used),
            ["ci/ui-strings-allow.txt: the entry \"◂\" matches no egui text call's literal; remove it"]
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

    /// Each call of the widened list is read; a painter's `text(` only on a painter, and another `.text(`
    /// or a `new(` of another type not.
    #[test]
    fn the_widened_calls() {
        let src = r##"
fn f(ui: &mut Ui) {
    ui.heading("Heading");
    ui.selectable_label(on, "Selectable");
    ui.checkbox(&mut on, "Check");
    ui.small_button("Small");
    ui.menu_button("Menu", |ui| {});
    ui.collapsing("Collapsing", |ui| {});
    TextEdit::singleline(&mut s).hint_text("Hint");
    Button::new("Button");
    egui::Label::new("Label");
    ui.painter().text(pos, align, "Painted", font, colour);
    painter.text(pos, align, "Painter", font, colour);
    chord.text("Not a text call");
    Id::new("not a text");
}
"##;
        assert_eq!(
            found(src),
            [
                (3, "Heading"),
                (4, "Selectable"),
                (5, "Check"),
                (6, "Small"),
                (7, "Menu"),
                (8, "Collapsing"),
                (9, "Hint"),
                (10, "Button"),
                (11, "Label"),
                (12, "Painted"),
                (13, "Painter"),
            ]
        );
    }

    /// Of `menu_button(` and `collapsing(` only the title is read, so a literal in the closure that reaches
    /// no text (`Id::new`) is not found.
    #[test]
    fn a_menu_closure_literal_is_not_a_text() {
        let src = r##"
fn f(ui: &mut Ui) {
    ui.menu_button("Menu", |ui| {
        ui.push_id(Id::new("menu id"), |ui| {});
    });
    ui.collapsing(title, |ui| {
        let _ = egui::Id::new("collapsing id");
    });
}
"##;
        assert_eq!(found(src), [(3, "Menu")]);
    }

    /// A text call nested in the closure of `menu_button(` or `collapsing(` is found once, by its own call.
    #[test]
    fn a_nested_label_is_found_once() {
        let src = r##"
fn f(ui: &mut Ui) {
    ui.menu_button(title, |ui| {
        ui.label("In the menu");
    });
    ui.collapsing("Section", |ui| {
        ui.label("In the section");
    });
}
"##;
        assert_eq!(
            found(src),
            [(4, "In the menu"), (6, "Section"), (7, "In the section")]
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
}
