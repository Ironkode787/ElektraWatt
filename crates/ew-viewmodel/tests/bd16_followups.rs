// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Follow-ups of the first blind-drafter dry run (BD16) found while fixing FT-485 (cycle 30, lane O30): the two
//! clamps of a starter terminal print distinct labels, the strip chooser's `picker.choose` takes its own
//! `designation`, and the keyboard move of a terminal placed through the text interface moves it by the grid
//! cursor's displacement.

mod common;

use common::{Tmp, click, done, entity_points};
use ew_viewmodel::ew_api::output::ew_dl::EntityRef;
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::{Value, json};

/// A headless shell with a new project (the builtin template), a new folio 101 shown and one new strip.
fn new_project_with_strip(tmp: &Tmp) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Power"}),
    );
    done(&mut s, "view.zoom_actual", json!({}));
    done(&mut s, "strip.new", json!({"letter": "X"}));
    s
}

/// Places a strip terminal at cell `cell` through the symbol picker and the strip chooser: `first` are the
/// parameters of the symbol row's choice, `second` those of the strip row's.
fn place_terminal(s: &mut ShellModel, cell: &str, first: Value, second: Value) {
    done(s, "tool.place", json!({}));
    done(s, "picker.filter", json!({"text": "iec.terminal@"}));
    done(s, "picker.choose", first);
    done(s, "picker.choose", second);
    done(s, "nav.goto", json!({ "text": cell }));
    done(s, "canvas.activate", json!({"alt": true}));
    done(s, "ui.dismiss", json!({}));
}

/// The labels of the page's connection points, sorted.
fn cp_labels(s: &ShellModel) -> Vec<String> {
    let mut v: Vec<String> = s
        .canvas()
        .unwrap()
        .page()
        .tables
        .cps
        .iter()
        .map(|c| c.label.clone())
        .collect();
    v.sort();
    v
}

// ---------------------------------------------------------------------------------------------------------------
// Item 1: the two clamps of a terminal print distinct labels.

#[test]
fn item1_the_two_clamps_of_a_starter_terminal_print_distinct_labels() {
    let tmp = Tmp::new("bd16-1");
    let mut s = new_project_with_strip(&tmp);
    place_terminal(&mut s, "101-D2", json!({}), json!({}));
    let labels = cp_labels(&s);
    assert_eq!(labels.len(), 2, "{labels:?}");
    assert_ne!(labels[0], labels[1], "{labels:?}");
    // `ew.fn.terminal` names clamp `a` on both sides: the label takes `#n` in CP index order (side 1, then side 2).
    assert!(labels[0].ends_with(":1/a#1"), "{labels:?}");
    assert!(labels[1].ends_with(":1/a#2"), "{labels:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// Item 2: the strip chooser's choice takes its own `designation`.

#[test]
fn item2_the_strip_choosers_designation_names_the_terminal() {
    let tmp = Tmp::new("bd16-2");
    let mut s = new_project_with_strip(&tmp);
    // The drafter's call: the designation on the strip row's choice (the strip proposes `1`).
    place_terminal(&mut s, "101-D2", json!({}), json!({"designation": "PE"}));
    let labels = cp_labels(&s);
    assert!(labels.iter().all(|l| l.contains(":PE/")), "{labels:?}");
}

#[test]
fn item2_the_strip_rows_designation_wins_over_the_symbol_rows() {
    let tmp = Tmp::new("bd16-2b");
    let mut s = new_project_with_strip(&tmp);
    place_terminal(
        &mut s,
        "101-D2",
        json!({"designation": "5"}),
        json!({"designation": "9"}),
    );
    // The symbol row's designation still applies when the strip row's choice names none.
    place_terminal(&mut s, "101-D4", json!({"designation": "7"}), json!({}));
    let labels = cp_labels(&s);
    assert_eq!(
        labels.iter().filter(|l| l.contains(":9/")).count(),
        2,
        "{labels:?}"
    );
    assert_eq!(
        labels.iter().filter(|l| l.contains(":7/")).count(),
        2,
        "{labels:?}"
    );
    assert!(!labels.iter().any(|l| l.contains(":5/")), "{labels:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// Item 3: the keyboard move of a terminal placed through the text interface.

/// The insertion point of the page's only representation.
fn only_rep(s: &ShellModel) -> (EntityRef, (i64, i64)) {
    let reps = &s.canvas().unwrap().page().tables.reps;
    assert_eq!(reps.len(), 1);
    (EntityRef(reps[0].id), reps[0].pos)
}

/// Selects the page's only representation with a click on it.
fn select_only_rep(s: &mut ShellModel) {
    let (rep, _) = only_rep(s);
    let p = entity_points(s.canvas().unwrap())
        .into_iter()
        .find(|(e, _)| *e == rep)
        .map(|(_, p)| p)
        .expect("a point picks the terminal");
    click(s, p, json!({}));
    assert_eq!(s.selection().primary, Some(rep));
}

#[test]
fn item3_the_move_shifts_the_terminal_by_the_cursors_displacement_from_where_it_was_armed() {
    let tmp = Tmp::new("bd16-3");
    let mut s = new_project_with_strip(&tmp);
    place_terminal(&mut s, "101-D2", json!({}), json!({}));
    let (_, before) = only_rep(&s);
    select_only_rep(&mut s);
    // The drafter's first try: the cursor stood in D4 when Move was armed, so going to D3 is one cell to the left;
    // Escape cancels and nothing moves.
    done(&mut s, "nav.goto", json!({"text": "101-D4"}));
    done(&mut s, "tool.move", json!({}));
    done(&mut s, "nav.goto", json!({"text": "101-D3"}));
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(only_rep(&s).1, before);
    // The cursor on the start cell, Move, the target cell, Enter: one cell to the right.
    done(&mut s, "nav.goto", json!({"text": "101-D2"}));
    done(&mut s, "tool.move", json!({}));
    done(&mut s, "nav.goto", json!({"text": "101-D3"}));
    done(&mut s, "canvas.activate", json!({}));
    let (_, after) = only_rep(&s);
    assert_eq!(after.1, before.1);
    assert!(after.0 > before.0, "{before:?} -> {after:?}");
    // The same move back with `cursor.set`: the cursor at the printed position of a CP, Move, the cursor one
    // cell width to the left, Enter.
    let cp = s.canvas().unwrap().page().cps[0].at_nm;
    let dx = after.0 - before.0;
    done(&mut s, "cursor.set", common::at(cp, json!({})));
    done(&mut s, "tool.move", json!({}));
    done(&mut s, "cursor.set", common::at((cp.0 - dx, cp.1), json!({})));
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(only_rep(&s).1, before);
}

/// The representations' insertion points by ID.
fn rep_positions(s: &ShellModel) -> Vec<(EntityRef, (i64, i64))> {
    s.canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .map(|r| (EntityRef(r.id), r.pos))
        .collect()
}

#[test]
fn item3_a_terminal_stacked_on_another_moves_off_it_by_keyboard_and_by_drag() {
    let tmp = Tmp::new("bd16-3b");
    let mut s = new_project_with_strip(&tmp);
    // The drafter's stack: two terminals placed on one point (Alt places without snapping to the other's CPs).
    place_terminal(&mut s, "101-D2", json!({}), json!({}));
    place_terminal(&mut s, "101-D2", json!({}), json!({}));
    let before = rep_positions(&s);
    assert_eq!(before.len(), 2);
    assert_eq!(before[0].1, before[1].1);
    // A click picks the top one.
    let (top, p) = entity_points(s.canvas().unwrap())
        .into_iter()
        .find(|(e, _)| before.iter().any(|(r, _)| r == e))
        .expect("a point picks a terminal");
    click(&mut s, p, json!({}));
    assert_eq!(s.selection().primary, Some(top));
    // Armed from D4 and cancelled, then armed on D2 and committed on D3: the selected terminal moves, the other
    // stays.
    done(&mut s, "nav.goto", json!({"text": "101-D4"}));
    done(&mut s, "tool.move", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "nav.goto", json!({"text": "101-D2"}));
    done(&mut s, "tool.move", json!({}));
    done(&mut s, "nav.goto", json!({"text": "101-D3"}));
    done(&mut s, "canvas.activate", json!({}));
    let after = rep_positions(&s);
    let pos = |v: &[(EntityRef, (i64, i64))], e: EntityRef| v.iter().find(|(r, _)| *r == e).unwrap().1;
    let other = before.iter().find(|(r, _)| *r != top).unwrap().0;
    assert_eq!(pos(&after, other), pos(&before, other));
    let dx = pos(&after, top).0 - pos(&before, top).0;
    assert!(dx > 0, "{before:?} -> {after:?}");
    // The pointer drags it back by the same distance (press on it, move, release).
    let p = entity_points(s.canvas().unwrap())
        .into_iter()
        .find(|(e, _)| *e == top)
        .map(|(_, p)| p)
        .unwrap();
    done(&mut s, "canvas.press", common::at(p, json!({})));
    done(&mut s, "canvas.pointer", common::at((p.0 - dx, p.1), json!({})));
    done(&mut s, "canvas.release", common::at((p.0 - dx, p.1), json!({})));
    assert_eq!(rep_positions(&s), before);
}
