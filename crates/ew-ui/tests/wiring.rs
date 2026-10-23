// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 2 in the window without rendering (acceptance checks WR3 and WR10's window part): FR-UI-015's
//! keyboard wiring on folio 102 (W, `]`, `[`, Enter) with exactly one new connection and one revision, the junction
//! sequence editor popup driven by its keys, and the exact `Session` reads of the Wire tool in the window. The
//! accessible-name walker runs after every step.

mod support;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use ew_ui::app::App;
use ew_ui::ew_viewmodel::ew_api::output::TableEnd;
use ew_ui::ew_viewmodel::shell::Outcome;
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn on_golden(tmp: &Tmp, folio: &str) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({ "folio": folio }));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

fn press(h: &mut Harness<'_, App>, key: Key) {
    h.key_press(key);
    settle(h);
}

/// The sheet point of the CP labelled `label` on the focused page.
fn cp_sheet(h: &Harness<'_, App>, label: &str) -> (i64, i64) {
    let c = h.state().shell().canvas().unwrap();
    let cp = c
        .page()
        .tables
        .cps
        .iter()
        .find(|x| x.label == label)
        .unwrap_or_else(|| panic!("{label}"))
        .clone();
    c.page().snapper.page_to_sheet(cp.at)
}

/// `]` (or `[`) until the grid cursor stands on the CP labelled `label`.
fn step_to(h: &mut Harness<'_, App>, label: &str, key: Key) {
    let target = cp_sheet(h, label);
    let n = h.state().shell().canvas().unwrap().page().cps.len();
    for _ in 0..=n {
        if h.state().shell().canvas().unwrap().cursor() == Some(target) {
            return;
        }
        press(h, key);
    }
    panic!("the cursor never reached {label}");
}

fn connections(h: &Harness<'_, App>) -> Vec<ew_ui::ew_viewmodel::ew_api::output::TableConnection> {
    h.state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .connections
        .clone()
}

#[test]
fn wr3_fr_ui_015_a_connection_drawn_from_the_keyboard_alone() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    let before = connections(&h);
    let rev = h.state().shell().revision();
    // W arms the Wire tool.
    press(&mut h, Key::W);
    assert_eq!(h.state().shell().tool().name(), "wire");
    // `]` to -102F1:2, Enter starts the wire there.
    step_to(&mut h, "-102F1:2", Key::CloseBracket);
    press(&mut h, Key::Enter);
    assert_eq!(h.state().shell().tool().state(), "routing");
    let live = h
        .state()
        .shell()
        .tool_announcement()
        .unwrap_or_default()
        .to_owned();
    assert!(live.starts_with("Wire: routing from -102F1:2."), "{live}");
    // `[` to -102F2:24VDC#1, Enter ends it.
    step_to(&mut h, "-102F2:24VDC#1", Key::OpenBracket);
    press(&mut h, Key::Enter);
    assert_eq!(h.state().shell().revision(), rev.map(|r| r + 1), "one revision");
    let after = connections(&h);
    let new: Vec<_> = after
        .iter()
        .filter(|c| !before.iter().any(|b| b.id == c.id))
        .collect();
    assert_eq!(new.len(), 1, "exactly one new connection");
    let tables = h.state().shell().canvas().unwrap().page().tables.clone();
    let label = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => tables
            .cps
            .iter()
            .find(|c| c.function == *function && c.idx == *idx)
            .map(|c| c.label.clone())
            .unwrap_or_default(),
        TableEnd::Junction { .. } => String::new(),
    };
    let mut ends = [label(&new[0].a), label(&new[0].b)];
    ends.sort();
    assert_eq!(ends, ["-102F1:2".to_owned(), "-102F2:24VDC#1".to_owned()]);
    // N steps to the next unconnected CP; Escape twice ends the tool.
    press(&mut h, Key::N);
    press(&mut h, Key::Escape);
    assert_eq!(h.state().shell().tool().name(), "select");
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn the_junction_editor_popup_reorders_with_alt_down_and_closes_with_escape() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    let j = h.state().shell().canvas().unwrap().page().tables.junctions[0].clone();
    dispatch(&mut h, "select.set", json!({"ids": [j.id.to_string()]}));
    dispatch(&mut h, "junction.open", json!({}));
    settle(&mut h);
    let first = h.state().shell().junction_editor().unwrap().sequence_labels();
    let rev = h.state().shell().revision();
    // Alt+Down moves the focused (first) member down: one `junction.edit`.
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowDown);
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().revision(), rev.map(|r| r + 1));
    let now = h.state().shell().junction_editor().unwrap().sequence_labels();
    assert_eq!(now, vec![first[1].clone(), first[0].clone(), first[2].clone()]);
    // Escape closes the popup, not the selection.
    press(&mut h, Key::Escape);
    assert!(h.state().shell().junction_editor().is_none());
    assert_eq!(h.state().shell().selection().items.len(), 1);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn wr10_the_wire_tool_reads_nothing_while_the_cursor_moves_in_the_window() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    press(&mut h, Key::W);
    step_to(&mut h, "-102F1:2", Key::CloseBracket);
    press(&mut h, Key::Enter);
    let calls = h.state().shell().session_calls();
    for _ in 0..8 {
        press(&mut h, Key::ArrowRight);
    }
    press(&mut h, Key::ArrowDown);
    let after = h.state().shell().session_calls();
    assert_eq!(after.page_content, calls.page_content, "{after:?}");
    assert_eq!(after.page_tables, calls.page_tables);
    assert_eq!(after.page_cps, calls.page_cps);
    assert_eq!(after.symbol_shape, calls.symbol_shape);
    // (Previews are not counted here: the window's dwell timer may fire between frames on a slow machine, a wall-clock
    // matter ID-559 keeps out of the assertions; the view-model test counts them with the dwell as an action.)
    assert_eq!(
        after.commit + after.execute_with,
        calls.commit + calls.execute_with
    );
    h.state_mut().shell_mut().shutdown();
}
