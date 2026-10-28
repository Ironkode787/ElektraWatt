// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.17 part 3, acceptance check DK9: the data grid of doc 16a §6.21 on Golden A's Terminal strips navigator in
//! table mode. Every key and pointer gesture of §6.21 dispatches the actions it dispatched before the restyle (the log
//! `tests/expected/table_keys.txt`, recorded from the as-built grid before the restyle; `EW_BLESS=1` rewrites it).
//! The accessible-name walker runs after every step (`settle`).

mod support;

use std::fmt::Write as _;

use egui::{Key, Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::{Outcome, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn done(shell: &mut ShellModel, action: &str, params: Value) {
    assert_eq!(
        shell.dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// The window on a writable copy of Golden A, folio 102 shown, the Terminal strips navigator in table mode filtered
/// `x1031` and sorted by Terminal, with the focus; Properties hidden so that the grid has the whole column.
fn strips(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h: Harness<'static, App> = harness(tmp, Scripted::default(), Some(dir));
    h.set_size(egui::Vec2::new(1000.0, 900.0));
    settle(&mut h);
    wait_for_page(&mut h);
    let shell = h.state_mut().shell_mut();
    done(shell, "props.show", json!({"on": false}));
    done(shell, "navigator.show", json!({"kind": "strips"}));
    done(shell, "navigator.table", json!({"kind": "strips"}));
    done(
        shell,
        "navigator.filter",
        json!({"kind": "strips", "text": "x1031"}),
    );
    done(
        shell,
        "navigator.sort",
        json!({"kind": "strips", "by": "terminal"}),
    );
    done(shell, "ui.focus", json!({"area": "navigator", "kind": "strips"}));
    settle(&mut h);
    h
}

// Program boundary of the test binary: the bless switch of the repository's tests (`EW_BLESS=1`).
#[expect(clippy::disallowed_methods, reason = "the test binary's bless switch")]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// One input of the dispatch log.
enum Input {
    Key(Modifiers, Key),
    Text(&'static str),
    /// A click on the node named so (see [`node`]), with modifiers.
    Click(&'static str, Modifiers),
    /// A double click on the node named so (four button events in one frame).
    DoubleClick(&'static str),
    /// The shell takes the focus back to the grid (not a gesture of the grid; nothing is logged for it).
    Refocus,
}

/// The centre of the node named `target`; `<column>@<row>` is the cell of that column in that row, whatever its text.
fn node<'t>(h: &'t Harness<'static, App>, target: &'t str) -> egui_kittest::Node<'t> {
    match target.split_once('@') {
        Some((column, row)) => {
            let row = format!(", row {row} of ");
            let column = format!("{column} ");
            h.get_all_by_role(egui::Role::Cell)
                .find(|n| {
                    n.accesskit_node()
                        .label()
                        .is_some_and(|l| l.starts_with(&column) && l.contains(&row))
                })
                .unwrap_or_else(|| panic!("the cell {target}"))
        }
        None => h.get_by_label(target),
    }
}

/// The four pointer events of a double click at `at`, all in one frame (no clock decides it).
fn double_click(h: &mut Harness<'static, App>, at: egui::Pos2) {
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    h.run_ok();
    for pressed in [true, false, true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    settle(h);
}

/// The gestures of doc 16a §6.21 in order, each with what the as-built grid dispatched for it.
fn gestures() -> Vec<(&'static str, Input)> {
    use Input::*;
    vec![
        ("Ctrl+Home", Key(Modifiers::COMMAND, egui::Key::Home)),
        ("Down", Key(Modifiers::NONE, egui::Key::ArrowDown)),
        ("Right", Key(Modifiers::NONE, egui::Key::ArrowRight)),
        ("Tab", Key(Modifiers::NONE, egui::Key::Tab)),
        ("Shift+Tab", Key(Modifiers::SHIFT, egui::Key::Tab)),
        ("Left", Key(Modifiers::NONE, egui::Key::ArrowLeft)),
        ("Up", Key(Modifiers::NONE, egui::Key::ArrowUp)),
        ("End", Key(Modifiers::NONE, egui::Key::End)),
        ("Home", Key(Modifiers::NONE, egui::Key::Home)),
        ("Ctrl+End", Key(Modifiers::COMMAND, egui::Key::End)),
        ("Ctrl+Home", Key(Modifiers::COMMAND, egui::Key::Home)),
        ("Shift+Down", Key(Modifiers::SHIFT, egui::Key::ArrowDown)),
        ("Shift+Down", Key(Modifiers::SHIFT, egui::Key::ArrowDown)),
        ("Shift+Up", Key(Modifiers::SHIFT, egui::Key::ArrowUp)),
        ("Ctrl+Enter (no card)", Key(Modifiers::COMMAND, egui::Key::Enter)),
        ("Right", Key(Modifiers::NONE, egui::Key::ArrowRight)),
        ("Right", Key(Modifiers::NONE, egui::Key::ArrowRight)),
        ("type Q", Text("Q")),
        ("editor: type x", Text("x")),
        ("editor: Escape", Key(Modifiers::NONE, egui::Key::Escape)),
        ("type Z", Text("Z")),
        ("editor: Enter", Key(Modifiers::NONE, egui::Key::Enter)),
        ("Shift+Down", Key(Modifiers::SHIFT, egui::Key::ArrowDown)),
        ("type W", Text("W")),
        ("editor: Ctrl+Enter", Key(Modifiers::COMMAND, egui::Key::Enter)),
        ("click", Click("Terminal@3", Modifiers::NONE)),
        ("Ctrl+click", Click("Terminal@5", Modifiers::COMMAND)),
        ("Shift+click", Click("Terminal@7", Modifiers::SHIFT)),
        ("double click", DoubleClick("Function text@2")),
        ("editor: Escape", Key(Modifiers::NONE, egui::Key::Escape)),
        (
            "header click",
            Click("Sort by Terminal, sorted ascending", Modifiers::NONE),
        ),
        (
            "header click again",
            Click("Sort by Terminal, sorted descending", Modifiers::NONE),
        ),
        ("refocus", Refocus),
        ("Enter", Key(Modifiers::NONE, egui::Key::Enter)),
    ]
}

/// DK9: every key of doc 16a §6.21 (arrows, Tab, Shift+Tab, Home, End, Ctrl+Home, Ctrl+End, Shift+Up, Shift+Down,
/// Enter, Ctrl+Enter, a printable character; in the editor Enter, Ctrl+Enter and Escape) and the pointer gestures
/// (click, Ctrl+click, Shift+click, double click, header clicks) dispatch the same actions with the same parameters as
/// before the restyle: the log equals the expected log recorded from the as-built grid.
#[test]
fn dk9_every_key_dispatches_the_as_built_actions() {
    let tmp = Tmp::new();
    let mut h = strips(&tmp);
    let mut log = String::new();
    for (label, input) in gestures() {
        let before = h.state().dispatched_calls().len();
        assert!(before < 200, "the dispatch log keeps its last 256 entries");
        match input {
            Input::Key(m, k) => {
                h.key_press_modifiers(m, k);
                settle(&mut h);
            }
            Input::Text(t) => {
                h.event(egui::Event::Text(t.to_owned()));
                settle(&mut h);
            }
            Input::Click(target, m) => {
                node(&h, target).click_modifiers(m);
                settle(&mut h);
            }
            Input::DoubleClick(target) => {
                let at = node(&h, target).rect().center();
                double_click(&mut h, at);
            }
            Input::Refocus => {
                let shell = h.state_mut().shell_mut();
                done(shell, "ui.focus", json!({"area": "navigator", "kind": "strips"}));
                settle(&mut h);
            }
        }
        let calls = h.state().dispatched_calls();
        writeln!(log, "{label}").unwrap();
        for (action, params) in &calls[before..] {
            writeln!(log, "  {action} {params}").unwrap();
        }
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/expected/table_keys.txt");
    if bless() {
        std::fs::write(&path, &log).unwrap();
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{} is blessed (EW_BLESS=1)", path.display()))
        .replace("\r\n", "\n");
    assert_eq!(log, expected, "the grid's dispatch log");
    h.state_mut().shell_mut().shutdown();
}
