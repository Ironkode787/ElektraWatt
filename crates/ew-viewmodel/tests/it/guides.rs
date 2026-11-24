// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Placing by hand, helped (docs/design/06a-drafting.md §6, §9.3 row 6), through the window's own path: in a new
//! project a two-pole breaker stands on the page; a second one floats below it with the pointer 0.7 of a grid step
//! beside the line of the upper one's pins, and takes that line; at 0.8 it stays on the ordinary snap's grid line; with
//! Alt there is neither guide nor nudge; a drag of a symbol takes the line the same way. What is set down stays.

use crate::common;

use common::{Tmp, at, done, tables};
use ew_api::output::TableRep;
use ew_viewmodel::shell::{Mode, ShellModel};
use ew_viewmodel::tools::{Dash, OverlayItem, Token};
use serde_json::json;

/// A new project with the multi-line page 101 shown at actual size.
fn new_project(tmp: &Tmp) -> ShellModel {
    let mut s = tmp.new_project_in(Mode::Headless, &[("101", "multi_line", "Power supply")]);
    done(&mut s, "page.goto", json!({"folio": "101"}));
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

/// The page point `x` grid steps across (a fraction allowed) and `y` down.
fn page_point(s: &ShellModel, x: f64, y: i64) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    let k = x.floor() as i64;
    let step = sn.page_of_index(k + 1) - sn.page_of_index(k);
    (
        sn.page_of_index(k) + (step as f64 * (x - k as f64)).round() as i64,
        sn.page_of_index(y),
    )
}

/// The sheet point of the page point `p`.
fn sheet(s: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    s.canvas().unwrap().page().snapper.page_to_sheet(p)
}

/// The pointer moved to the page point `x` steps across (a fraction allowed) and `y` down, with Alt held or not.
fn hover(s: &mut ShellModel, (x, y): (f64, i64), alt: bool) {
    let q = sheet(s, page_point(s, x, y));
    done(s, "canvas.pointer", at(q, json!({ "alt": alt })));
}

/// A click at the page point `x` steps across and `y` down, as the window sends it.
fn click(s: &mut ShellModel, (x, y): (f64, i64), alt: bool) {
    hover(s, (x, y), alt);
    let q = sheet(s, page_point(s, x, y));
    done(s, "canvas.press", at(q, json!({ "alt": alt })));
    done(s, "canvas.release", at(q, json!({ "alt": alt })));
}

/// The breaker floating with the Place tool.
fn float_breaker(s: &mut ShellModel) {
    done(s, "place.pick", json!({"part": "EW-GEN-MCB-2", "template": 0}));
}

/// The representation new since `before`.
fn new_rep(s: &ShellModel, before: &[TableRep]) -> TableRep {
    let t = tables(s);
    let new: Vec<&TableRep> = t
        .reps
        .iter()
        .filter(|r| before.iter().all(|b| b.id != r.id))
        .collect();
    assert_eq!(new.len(), 1, "one new representation");
    new[0].clone()
}

/// Where the floating symbol stands: the origin of its outline, page nm.
fn floating_at(s: &ShellModel) -> (i64, i64) {
    s.tool_overlay()
        .items
        .iter()
        .find_map(|i| match i {
            OverlayItem::Paths { transform, .. } => Some(transform.1),
            _ => None,
        })
        .expect("the floating outline")
}

/// The guide lines of the tool's overlay: (solid, the number of them).
fn guides(s: &ShellModel) -> (usize, usize) {
    let lines: Vec<_> = s
        .tool_overlay()
        .items
        .iter()
        .filter_map(|i| match i {
            OverlayItem::Line { style, .. } if style.token == Token::Snap => Some(style.dash),
            _ => None,
        })
        .collect();
    (lines.iter().filter(|d| **d == Dash::Solid).count(), lines.len())
}

/// The page with the upper breaker at grid point (40, 20), and the grid row where the floating breaker's origin
/// stands three steps below its lower pins.
fn page(tmp: &Tmp) -> (ShellModel, TableRep, i64) {
    let mut s = new_project(tmp);
    let before = tables(&s).reps.clone();
    float_breaker(&mut s);
    click(&mut s, (40.0, 20), true);
    let upper = new_rep(&s, &before);
    let sn = s.canvas().unwrap().page().snapper.clone();
    let ys: Vec<i64> = tables(&s)
        .cps
        .iter()
        .filter(|c| c.rep == upper.id)
        .map(|c| sn.grid_index_of(c.at.1))
        .collect();
    let low = *ys.iter().max().expect("pins");
    let high = *ys.iter().min().expect("pins");
    // The origin's row such that the floating breaker's upper pins stand three steps under the upper's lower pins.
    let row = 20 + (low - sn.grid_index_of(upper.pos.1)) + 3 + (sn.grid_index_of(upper.pos.1) - high);
    (s, upper, row)
}

#[test]
fn a_pointer_0_7_step_from_the_guide_takes_its_line_0_8_does_not_and_alt_shows_neither() {
    let tmp = Tmp::new("guides-place");
    let (mut s, upper, row) = page(&tmp);
    float_breaker(&mut s);
    // 0.7 step right of the upper breaker's line: the ordinary snap would take 41, the guide takes 40, in line.
    hover(&mut s, (40.7, row), false);
    assert_eq!(floating_at(&s), page_point(&s, 40.0, row));
    let (solid, all) = guides(&s);
    assert!(
        solid >= 2 && all >= solid,
        "a solid guide with its tick: {solid} of {all}"
    );
    // 0.8 step: the ordinary snap's grid line, the guides dotted beside it.
    hover(&mut s, (40.8, row), false);
    assert_eq!(floating_at(&s), page_point(&s, 41.0, row));
    let (solid, all) = guides(&s);
    assert!(solid == 0 && all >= 1, "dotted guides: {solid} of {all}");
    // With Alt held: no guide and no nudge.
    hover(&mut s, (40.7, row), true);
    assert_eq!(floating_at(&s), page_point(&s, 41.0, row));
    assert_eq!(guides(&s), (0, 0));
    // Set down at 0.7: it stands where the guide showed it, and stays there while the pointer moves on.
    let before = tables(&s).reps.clone();
    let rev = s.revision();
    click(&mut s, (40.7, row), false);
    let lower = new_rep(&s, &before);
    assert_eq!(lower.pos, page_point(&s, 40.0, row));
    let rev = rev.map(|r| r + 1);
    assert_eq!(s.revision(), rev, "one commit");
    hover(&mut s, (41.3, row + 9), false);
    hover(&mut s, (42.6, row), false);
    let t = tables(&s);
    let now = t
        .reps
        .iter()
        .find(|r| r.id == lower.id)
        .expect("the placed breaker");
    assert_eq!((now.pos, s.revision()), (lower.pos, rev));
    assert!(t.reps.iter().any(|r| r.id == upper.id && r.pos == upper.pos));
}

#[test]
fn a_dragged_symbol_takes_the_guides_line_unless_alt_is_held() {
    let tmp = Tmp::new("guides-drag");
    let (mut s, _, row) = page(&tmp);
    // A second breaker one step right of the line, set down with Alt: it does not connect.
    let before = tables(&s).reps.clone();
    float_breaker(&mut s);
    click(&mut s, (41.0, row), true);
    let lower = new_rep(&s, &before);
    done(&mut s, "tool.select", json!({}));
    // Pressed on its body, dragged away and back to 0.3 step left of where it was pressed.
    let page = s.canvas().unwrap().page().clone();
    let rect = page
        .entities
        .iter()
        .find(|(e, _)| e.0 == lower.id)
        .map(|(_, r)| *r)
        .expect("the breaker's rectangle");
    let from = ew_viewmodel::canvas::rect_centre(rect);
    let step = page.snapper.sheet_of_index(1, 0) - page.snapper.sheet_of_index(0, 0);
    let to = (from.0 - step * 3 / 10, from.1);
    let drag = |s: &mut ShellModel, alt: bool| {
        done(s, "canvas.press", at(from, json!({ "alt": alt })));
        done(
            s,
            "canvas.pointer",
            at((from.0 + 2 * step, from.1), json!({ "alt": alt })),
        );
        done(s, "canvas.pointer", at(to, json!({ "alt": alt })));
        done(s, "canvas.release", at(to, json!({ "alt": alt })));
    };
    // With Alt the drag is less than half a step: nothing moves.
    let rev = s.revision();
    drag(&mut s, true);
    assert_eq!(s.revision(), rev, "no move");
    // Without: the guide's line, one undo step.
    drag(&mut s, false);
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    let t = tables(&s);
    let moved = t.reps.iter().find(|r| r.id == lower.id).expect("the breaker");
    assert_eq!(moved.pos, page_point(&s, 40.0, row));
}
