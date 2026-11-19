// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Ctrl+Z in the window with a tool on the canvas (rc.5 persona runs): with something in flight on the pointer the
//! first Ctrl+Z puts it away and undoes nothing, and the status line says so; with a tool merely armed it undoes the
//! last step at once and says what it took back and where; a placement and the wires it connected by itself go back as
//! one step.

use crate::support::{Tmp, dispatch, on_golden, settle};

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use ew_ui::app::App;
use serde_json::json;

type Window = Harness<'static, App>;

/// The grid cursor on the page point `at` of the page shown.
fn cursor_at(h: &mut Window, at: (i64, i64)) {
    let (x, y) = {
        let c = h.state().shell().canvas().expect("a page");
        let s = c.page().snapper.page_to_sheet(at);
        (s.0 as f64 / 1e6, s.1 as f64 / 1e6)
    };
    dispatch(h, "cursor.set", json!({"x_mm": x, "y_mm": y}));
}

/// The points of the page's free electrical connection points (no wire ends at them).
fn free_cps(h: &Window) -> Vec<(i64, i64)> {
    let c = h.state().shell().canvas().expect("a page");
    c.page()
        .tables
        .cps
        .iter()
        .filter(|p| !p.fluid && p.connections.is_empty() && p.dir.is_some())
        .map(|p| p.at)
        .collect()
}

fn undo(h: &mut Window) {
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    settle(h);
}

fn wires(h: &Window) -> usize {
    h.state()
        .shell()
        .canvas()
        .map_or(0, |c| c.page().tables.connections.len())
}

fn toast(h: &Window) -> String {
    h.state()
        .shell()
        .toast()
        .map(|t| t.text.clone())
        .unwrap_or_default()
}

/// The cabinet run (rc5-e): a wire begun at its first click is put away by Ctrl+Z, which undoes nothing and says so;
/// a finished wire, the Wire tool still armed, is taken back by the first Ctrl+Z, and the toast says what and where.
#[test]
fn ctrl_z_puts_away_a_wire_begun_and_takes_back_a_finished_one() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    let before = wires(&h);
    let free = free_cps(&h);
    assert!(free.len() >= 2, "Golden A's page 102 has free connection points");
    dispatch(&mut h, "tool.wire", json!({}));
    cursor_at(&mut h, free[0]);
    dispatch(&mut h, "canvas.activate", json!({}));
    assert_eq!(h.state().shell().tool().state(), "routing", "a wire begun");
    let rev = h.state().shell().revision();
    undo(&mut h);
    assert_eq!(h.state().shell().revision(), rev, "nothing undone");
    assert_ne!(
        h.state().shell().tool().state(),
        "routing",
        "the wire begun is put away"
    );
    assert_eq!(
        h.state().shell().tool_notice(),
        Some("Undo put away what was on the pointer and undid nothing; Undo again takes back the last step.")
    );
    // A wire drawn to its end; the Wire tool stays armed with nothing begun.
    assert_eq!(h.state().shell().tool().name(), "wire");
    for at in &free[..2] {
        cursor_at(&mut h, *at);
        dispatch(&mut h, "canvas.activate", json!({}));
    }
    settle(&mut h);
    assert_eq!(wires(&h), before + 1, "the wire");
    assert_eq!(h.state().shell().tool().state(), "armed");
    undo(&mut h);
    assert_eq!(wires(&h), before, "the first Ctrl+Z takes the wire back");
    let said = toast(&h);
    assert!(said.starts_with("Undone: "), "{said}");
    h.state_mut().shell_mut().shutdown();
}

/// A placement and the line it connected by itself to the port it faced are one step: one Ctrl+Z, the Place tool still
/// armed between two placements, takes both back.
#[test]
fn a_placement_and_the_line_it_connected_go_back_as_one_step() {
    use ew_ui::ew_viewmodel::tools::Ghost;
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    let (free, step) = {
        let c = h.state().shell().canvas().unwrap();
        let t = &c.page().tables;
        let free: Vec<(i64, i64)> = t
            .cps
            .iter()
            .filter(|p| p.fluid && p.connections.is_empty() && p.dir.is_some())
            .map(|p| p.at)
            .collect();
        (free, t.grid.0 / t.grid.1.max(1))
    };
    let reps = |h: &Window| {
        h.state()
            .shell()
            .canvas()
            .map_or(0, |c| c.page().tables.reps.len())
    };
    let (lines, symbols) = (wires(&h), reps(&h));
    dispatch(&mut h, "tool.place", json!({}));
    dispatch(
        &mut h,
        "place.pick",
        json!({"symbol": "ew.iso.silencer@qet10", "letter": "R"}),
    );
    settle(&mut h);
    let mut found = false;
    'search: for at in &free {
        for dy in (-10..=10).filter(|d| *d != 0) {
            for dx in -3..=3 {
                cursor_at(&mut h, (at.0 + dx * step, at.1 + dy * step));
                if matches!(h.state().shell().tool_ghost(), Some(Ghost::Place { hints, .. }) if hints > 0) {
                    found = true;
                    break 'search;
                }
            }
        }
    }
    assert!(found, "a place where the silencer faces a free port");
    dispatch(&mut h, "canvas.activate", json!({}));
    settle(&mut h);
    assert_eq!(
        (wires(&h), reps(&h)),
        (lines + 1, symbols + 1),
        "placed and connected"
    );
    assert_eq!(
        h.state().shell().tool().name(),
        "place",
        "still armed for the next one"
    );
    undo(&mut h);
    assert_eq!(
        (wires(&h), reps(&h)),
        (lines, symbols),
        "one Ctrl+Z takes back the silencer and its line"
    );
    h.state_mut().shell_mut().shutdown();
}
