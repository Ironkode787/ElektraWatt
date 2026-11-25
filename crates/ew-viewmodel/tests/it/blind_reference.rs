// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The persona harness's action list (`actions.txt`) is made from this crate's registry. It is not committed: it is
//! part of the reference (`cargo xtask gen`, `cargo xtask docs-gen`), which runs the first test below with
//! `EW_REFERENCE_DIR` set; the harness reads `target/reference/actions.txt`.
//!
//! `actions.txt` has one line per action in registry order,
//! `<id> <View|Command> "<label>" menu "<menu>[ ▸ <submenu>]"|none shortcut "<first>"|none context <context>[ headless]
//! [ hidden][ needs-parameters] params "<signature>"`. It carries the licence of this crate (it is made of its
//! strings), as the rest of the reference does.

use ew_viewmodel::actions::{ACTIONS, ActionKind, parse_signature};
use ew_viewmodel::keymap::context_word;
use ew_viewmodel::strings;
use ew_viewmodel::strings::Key;

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
        let label = strings::text(Key::run_time(&a.label_key()));
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
            (a.needs_parameters(), "needs-parameters"),
        ] {
            if on {
                out.push(' ');
                out.push_str(word);
            }
        }
        out.push_str(&format!(" params \"{}\"\n", a.signature()));
    }
    out
}

/// Writes `actions.txt` where `EW_REFERENCE_DIR` asks for it, for the persona harness.
// The variable only chooses whether the generated file is also written out.
#[allow(clippy::disallowed_methods)]
#[test]
fn the_action_list_is_written_for_the_harness() {
    if let Some(dir) = std::env::var_os("EW_REFERENCE_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("actions.txt"), actions_text()).unwrap();
    }
}

/// Every line reads back as the persona harness reads it (`ew_signature::parse_action_line`, the window's own grammar):
/// its ID, its kind and its signature's parameters, the named sets spelt out. A type the harness could not read fails
/// here, not in a persona run's first step (`page` did).
#[test]
fn the_action_lines_read_back_as_the_harness_reads_them_and_cover_the_registry() {
    let text = actions_text();
    assert_eq!(text.lines().count(), ACTIONS.len());
    for (line, a) in text.lines().zip(ACTIONS) {
        let read = ew_signature::parse_action_line(line).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(read.id, a.id, "{line}");
        assert_eq!(read.view, matches!(a.kind, ActionKind::View), "{line}");
        assert_eq!(Ok(read.params), parse_signature(a.params), "{line}");
    }
    // The harness reads these two actions' lines for its filter; their flags are what the specification says.
    let page = text.lines().find(|l| l.starts_with("page.describe ")).unwrap();
    assert!(
        page.contains(" View ") && page.contains(" headless") && page.ends_with("params \"folio?:text\"")
    );
    let command = text.lines().find(|l| l.starts_with("command.run ")).unwrap();
    assert!(command.contains(" needs-parameters"), "{command}");
}
