// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-500: every wire of a page can be picked by a click. On Golden A's folio 101 the provisional routes run along
//! one another and through or along symbol bodies; a click on a point of a wire's route that no other wire covers
//! picks that wire (WP-S2.04 §6.6 as FT-500 amends it: a wire's line nearer than the centre of the representation
//! rectangle the press is in goes first), and a repeated click at one place walks the pick stack and wraps, so a
//! wire that other wires cover for its whole length is reached too.

use crate::common;

use common::{Tmp, click, done, empty_point};
use ew_viewmodel::canvas::HOVER_SLOP_PT;
use ew_viewmodel::ew_api::output::ew_dl::EntityRef;
use ew_viewmodel::shell::{Mode, ShellModel};
use ew_viewmodel::tools::{pick_stack, segment_dist2};
use serde_json::json;

/// The wires of the shown page with their drawn polylines on the sheet.
fn wires(s: &ShellModel) -> Vec<(EntityRef, Vec<(i64, i64)>)> {
    let page = s.canvas().unwrap().page();
    page.tables
        .connections
        .iter()
        .map(|c| {
            let pts = c
                .polyline
                .iter()
                .map(|p| page.snapper.page_to_sheet(*p))
                .collect();
            (EntityRef(c.id), pts)
        })
        .collect()
}

/// The pick reach of the shown canvas, nm (as the Select tool's connection pick floors it).
fn reach(s: &ShellModel) -> i128 {
    (HOVER_SLOP_PT / s.canvas().unwrap().points_per_nm()).floor() as i128
}

/// The points of a polyline every 0.1 mm along each segment, both ends included.
fn samples(pts: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    for w in pts.windows(2) {
        let n = (((w[1].0 - w[0].0).abs() + (w[1].1 - w[0].1).abs()) / 100_000).max(1);
        for k in 0..=n {
            out.push((
                w[0].0 + (w[1].0 - w[0].0) * k / n,
                w[0].1 + (w[1].1 - w[0].1) * k / n,
            ));
        }
    }
    out
}

/// The first sample of `own`'s route that no other wire passes within the reach of, and that no link is under.
fn own_point(s: &ShellModel, own: EntityRef, all: &[(EntityRef, Vec<(i64, i64)>)]) -> Option<(i64, i64)> {
    let r = reach(s);
    let line = &all.iter().find(|(e, _)| *e == own)?.1;
    samples(line).into_iter().find(|p| {
        s.canvas().unwrap().link_at(*p).is_none()
            && all
                .iter()
                .all(|(e, pts)| *e == own || pts.windows(2).all(|w| segment_dist2(*p, w[0], w[1]) > r * r))
    })
}

/// The pick stack of the Select tool at `p` on the shown canvas.
fn stack(s: &ShellModel, p: (i64, i64)) -> Vec<EntityRef> {
    let c = s.canvas().unwrap();
    let page = c.page();
    pick_stack(page, &page.snapper, p, HOVER_SLOP_PT / c.points_per_nm())
}

/// A fresh click at `p`: a click on empty canvas first, so that it is no repeated click.
fn fresh_click(s: &mut ShellModel, p: (i64, i64)) -> Vec<EntityRef> {
    let empty = empty_point(s.canvas().unwrap());
    click(s, empty, json!({}));
    assert!(s.selection().is_empty());
    click(s, p, json!({}));
    s.selection().items.clone()
}

/// Every wire of a copy of Golden A's folio 101, at the fitted view and at actual size: a click on a point of its
/// route that no other wire covers selects that wire alone (eight did not before FT-500: four whose free stretch lies
/// in or on a symbol's rectangle picked the symbol, and four that other wires cover for their whole length have no
/// such point); each of the four covered ones is reached by repeated clicks on its route, within its pick stack.
#[test]
fn every_wire_of_folio_101_is_picked_by_a_click_on_a_point_only_it_covers() {
    for actual in [false, true] {
        let tmp = Tmp::new("ft500-every");
        let mut s = tmp.shell_in(Mode::Headless, false, "101");
        if actual {
            done(&mut s, "view.zoom_actual", json!({}));
        }
        let all = wires(&s);
        assert_eq!(all.len(), 21);
        let mut covered = Vec::new();
        for (wire, line) in &all {
            match own_point(&s, *wire, &all) {
                Some(p) => {
                    assert_eq!(
                        fresh_click(&mut s, p),
                        vec![*wire],
                        "a click at {p:?} on {wire:?} (actual size: {actual})"
                    );
                    assert_eq!(s.current_folio(), Some("101"));
                }
                None => covered.push((*wire, line.clone())),
            }
        }
        // Four routes lie on other wires end to end: -101F1:1 -> F55:1, S56:14 -> E56:X1, junction 2 -> S56:13 and
        // junction 2 -> T55:5.
        assert_eq!(covered.len(), 4, "actual size: {actual}");
        for (wire, line) in &covered {
            // The middle of the longest segment.
            let w = line
                .windows(2)
                .max_by_key(|w| (w[1].0 - w[0].0).abs() + (w[1].1 - w[0].1).abs())
                .unwrap();
            let p = ((w[0].0 + w[1].0) / 2, (w[0].1 + w[1].1) / 2);
            let st = stack(&s, p);
            assert!(st.contains(wire), "{wire:?} in the stack at {p:?}");
            let mut picked = fresh_click(&mut s, p);
            let mut clicks = 1;
            while picked != vec![*wire] {
                assert!(clicks < st.len(), "{wire:?} not reached in {} clicks", st.len());
                click(&mut s, p, json!({}));
                picked = s.selection().items.clone();
                clicks += 1;
            }
        }
        s.shutdown();
    }
}

/// A click where wires overlap picks the first of the pick stack, every repeated click the next, and the last wraps
/// to the first; the stack is deterministic (connections at one distance by ID, before the representation whose
/// rectangle holds the point). A Shift click never walks, and a click elsewhere starts again at the first.
#[test]
fn a_repeated_click_at_an_overlap_walks_the_candidates_and_wraps() {
    let tmp = Tmp::new("ft500-walk");
    let mut s = tmp.shell_in(Mode::Headless, false, "101");
    done(&mut s, "view.zoom_actual", json!({}));
    // On the line y = 41.104 mm, -101F1:1 -> F55:1 and S51:3 -> -101F1:N#1 overlap between x = 145.5 and 153.3 mm,
    // inside -101F1's rectangle.
    let all = wires(&s);
    let p = (150_000_000, all[0].1[0].1);
    let st = stack(&s, p);
    assert_eq!(st, stack(&s, p), "deterministic");
    let tables = std::sync::Arc::clone(&s.canvas().unwrap().page().tables);
    let is_wire = |e: &EntityRef| tables.connections.iter().any(|c| c.id == e.0);
    let n_wires = st.iter().take_while(|e| is_wire(e)).count();
    assert_eq!(n_wires, 2, "{st:?}");
    assert!(st[0] < st[1], "wires at one distance by ID");
    assert!(st.len() > n_wires, "the representation follows: {st:?}");
    assert!(tables.reps.iter().any(|r| r.id == st[2].0), "{st:?}");
    // The walk: first, next, …, and around.
    assert_eq!(fresh_click(&mut s, p), vec![st[0]]);
    for k in 1..=st.len() {
        click(&mut s, p, json!({}));
        assert_eq!(s.selection().items, vec![st[k % st.len()]], "click {}", k + 1);
    }
    // Within the click slop of the first press (1 mm < 3 points at actual size) the walk goes on.
    click(&mut s, (p.0 + 500_000, p.1), json!({}));
    assert_eq!(s.selection().items, vec![st[1]]);
    // A Shift click adds the first, without walking.
    click(&mut s, p, json!({"shift": true}));
    assert_eq!(s.selection().items, vec![st[1], st[0]]);
    // A plain click now is no repeat (the selection is not what the last plain click selected): the first.
    click(&mut s, p, json!({}));
    assert_eq!(s.selection().items, vec![st[0]]);
    click(&mut s, p, json!({}));
    assert_eq!(s.selection().items, vec![st[1]]);
    // A click elsewhere, then here again: the first.
    let q = own_point(&s, st[0], &all).expect("a point of the first wire alone");
    click(&mut s, q, json!({}));
    assert_eq!(s.selection().items, vec![st[0]]);
    click(&mut s, p, json!({}));
    assert_eq!(s.selection().items, vec![st[0]]);
    // Enter at the grid cursor walks the same way.
    done(&mut s, "cursor.set", common::at(p, json!({})));
    let c = s.canvas().unwrap().cursor().unwrap();
    let sc = stack(&s, c);
    done(&mut s, "canvas.activate", json!({}));
    let first = s.selection().items.clone();
    done(&mut s, "canvas.activate", json!({}));
    let second = s.selection().items.clone();
    let i = sc
        .iter()
        .position(|e| first == vec![*e])
        .expect("Enter picks from the stack");
    assert_eq!(second, vec![sc[(i + 1) % sc.len()]]);
    s.shutdown();
}

/// A press on a wire's line inside a representation's rectangle selects the wire on a click, and a drag from there
/// still moves the representation (the drag is as it was before FT-500); a click at the representation's centre
/// selects the representation.
#[test]
fn a_wire_through_a_symbol_is_clicked_and_the_symbol_still_drags_from_there() {
    let tmp = Tmp::new("ft500-drag");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut s, "view.zoom_actual", json!({}));
    let tables = std::sync::Arc::clone(&s.canvas().unwrap().page().tables);
    let heater = tables
        .reps
        .iter()
        .find(|r| r.symbol_key == "ew.iec.heater@qet10")
        .expect("E55")
        .clone();
    let snap = s.canvas().unwrap().page().snapper.clone();
    let centre = snap.page_to_sheet((
        (heater.body[0] + heater.body[2]) / 2,
        (heater.body[1] + heater.body[3]) / 2,
    ));
    assert_eq!(fresh_click(&mut s, centre), vec![EntityRef(heater.id)]);
    // T55:3 -> E55:N runs through E55's body on x = 210.725 mm.
    let all = wires(&s);
    let (wire, line) = all
        .iter()
        .find(|(_, pts)| {
            pts.len() == 2 && pts[0].0 == pts[1].0 && {
                let x = pts[0].0;
                let b0 = snap.page_to_sheet((heater.body[0], heater.body[1]));
                let b1 = snap.page_to_sheet((heater.body[2], heater.body[3]));
                x > b0.0 && x < b1.0
            }
        })
        .expect("a vertical wire through E55");
    let b1 = snap.page_to_sheet((heater.body[2], heater.body[3]));
    let on = (line[0].0, b1.1 - 1_000_000);
    assert_eq!(fresh_click(&mut s, on), vec![*wire]);
    // A drag from the same point moves E55 by one grid step.
    let empty = empty_point(s.canvas().unwrap());
    click(&mut s, empty, json!({}));
    let i = snap.grid_index_of(heater.pos.0);
    let step = snap.sheet_of_index(i + 1, 0) - snap.sheet_of_index(i, 0);
    done(&mut s, "canvas.press", common::at(on, json!({})));
    let to = (on.0 + step, on.1);
    done(&mut s, "canvas.pointer", common::at(to, json!({})));
    assert_eq!(s.tool().state(), "dragging");
    done(&mut s, "canvas.release", common::at(to, json!({})));
    let moved = s
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.id == heater.id)
        .unwrap()
        .pos;
    assert_eq!(moved, (snap.page_of_index(i + 1), heater.pos.1));
    s.shutdown();
}
