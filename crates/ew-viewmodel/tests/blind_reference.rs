// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S6.13 part 1, acceptance check BD3 (closes FT-314): the blind drafter's reference files are generated from
//! this crate's registry and word lists and compared with the committed copies, so that a lane that adds an action,
//! a signature or a describe word without updating the harness's help fails here. `EW_BLESS=1` rewrites them:
//!
//! ```text
//! EW_BLESS=1 cargo nextest run -p ew-viewmodel --test blind_reference
//! ```
//!
//! `tools/blind/reference/actions.txt` has one line per action in registry order,
//! `<id> <View|Command> "<label>" menu "<menu>[ ▸ <submenu>]"|none shortcut "<first>"|none context <context>[ headless]
//! [ hidden][ needs-parameters] params "<signature>"`; `vocabulary.txt` has a header line, the words of
//! [`VOCABULARY`] in order, one per line, then `markers:` and the [`FREE_TEXT_MARKERS`], one per line. Both carry the
//! licence of this crate (they are generated from its strings; REUSE.toml and `ci/licence-map.toml`).

mod common;

use std::path::PathBuf;

use common::bless;
use ew_viewmodel::actions::{ACTIONS, ActionKind, Context, parse_signature};
use ew_viewmodel::describe::{FREE_TEXT_MARKERS, VOCABULARY};
use ew_viewmodel::strings;

const BLESS_HINT: &str = "EW_BLESS=1 cargo nextest run -p ew-viewmodel --test blind_reference";

fn reference_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("blind")
        .join("reference")
}

fn context_word(c: Context) -> &'static str {
    match c {
        Context::Global => "global",
        Context::Canvas => "canvas",
        Context::Navigator => "navigator",
        Context::Properties => "properties",
        Context::Find => "find",
    }
}

fn menu_text(a: &ew_viewmodel::actions::ActionMeta) -> String {
    match a.menu {
        None => "none".to_owned(),
        Some(slot) => {
            let menu = strings::text(slot.menu.label_key());
            match slot.submenu {
                None => format!("\"{menu}\""),
                Some(sub) => format!("\"{menu} \u{25b8} {}\"", strings::text(sub.label_key())),
            }
        }
    }
}

/// `actions.txt`: the registry as the drafter reads it.
fn actions_text() -> String {
    let mut out = String::new();
    for a in ACTIONS {
        let label = strings::text(a.label_key);
        assert!(
            !label.contains('"') && !label.contains('\n'),
            "the label of {}",
            a.id
        );
        let kind = match a.kind {
            ActionKind::View => "View",
            ActionKind::Command => "Command",
        };
        let shortcut = a
            .shortcut
            .first()
            .map_or_else(|| "none".to_owned(), |s| format!("\"{s}\""));
        out.push_str(&format!(
            "{} {kind} \"{label}\" menu {} shortcut {shortcut} context {}",
            a.id,
            menu_text(a),
            context_word(a.context)
        ));
        for (on, word) in [
            (a.headless_only, "headless"),
            (a.hidden, "hidden"),
            (a.needs_parameters, "needs-parameters"),
        ] {
            if on {
                out.push(' ');
                out.push_str(word);
            }
        }
        out.push_str(&format!(" params \"{}\"\n", a.params));
    }
    out
}

/// `vocabulary.txt`: the describe text's structural words, then its free-text markers.
fn vocabulary_text() -> String {
    let mut out = format!(
        "# ew.blindvocab/1: the {} structural words of the describe text, then its free-text markers\n",
        VOCABULARY.len()
    );
    for w in VOCABULARY {
        out.push_str(w);
        out.push('\n');
    }
    out.push_str("markers:\n");
    for m in FREE_TEXT_MARKERS {
        out.push_str(m);
        out.push('\n');
    }
    out
}

#[track_caller]
fn check(name: &str, text: &str) {
    let path = reference_dir().join(name);
    if bless() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
    }
    let committed = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}; `{BLESS_HINT}` generates it", path.display()))
        .replace("\r\n", "\n");
    if committed != text {
        let first = committed
            .lines()
            .zip(text.lines())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| committed.lines().count().min(text.lines().count()));
        panic!(
            "tools/blind/reference/{name} is not the registry's text (first difference at line {}): \
             rewrite it with `{BLESS_HINT}` and commit it",
            first + 1
        );
    }
}

#[test]
fn the_reference_files_equal_the_registry_and_the_word_lists() {
    check("actions.txt", &actions_text());
    check("vocabulary.txt", &vocabulary_text());
}

#[test]
fn the_action_lines_parse_back_and_cover_the_registry() {
    let text = actions_text();
    assert_eq!(text.lines().count(), ACTIONS.len());
    for (line, a) in text.lines().zip(ACTIONS) {
        assert!(line.starts_with(&format!("{} ", a.id)), "{line}");
        let at = line
            .rfind(" params \"")
            .expect("every line ends with its signature");
        let sig = line[at + " params \"".len()..].strip_suffix('"').unwrap();
        assert_eq!(sig, a.params);
        parse_signature(sig).unwrap_or_else(|e| panic!("{}: {e}", a.id));
    }
    // The harness reads these two actions' lines for its filter; their flags are what the specification says.
    let page = text.lines().find(|l| l.starts_with("page.describe ")).unwrap();
    assert!(
        page.contains(" View ") && page.contains(" headless") && page.ends_with("params \"folio?:text\"")
    );
    let command = text.lines().find(|l| l.starts_with("command.run ")).unwrap();
    assert!(command.contains(" needs-parameters"), "{command}");
}

#[test]
fn a_changed_word_fails_with_the_bless_hint() {
    // The comparison itself: a text that differs in one word is not equal, and the hint names the command.
    let text = vocabulary_text();
    assert_ne!(text, text.replacen("\npage\n", "\npagE\n", 1));
    assert!(BLESS_HINT.starts_with("EW_BLESS=1 cargo nextest run"));
}
