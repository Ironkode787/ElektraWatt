// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The router on a full page, through the window's own path (the owner's rc.4 test: "a deep look into the
//! autorouter, optimised also for larger drawings"). A page of four rows of fourteen two-pin devices is wired one
//! wire at a time with the Wire tool: the columns, a rail above and one below with a tap per column, a branch from
//! each column to the next and a few long links. Reroute with nothing selected then routes every wire of the page
//! again, and must not leave the page worse than the person drew it: no wire without a route, no wire on another, no
//! more bends and crossings. (Routed from nothing with the longest wires first, the long wires took the straight
//! tracks between the pins, the short ones stepped aside and seven kept their old route under the new ones.)
//!
//! And a wire ended on another where a third one crosses it: the crossing is no place for a junction. And the texts
//! beside the pins of a multipin plug, which a wire goes round.

use crate::common;

use common::{Tmp, at, done, tables};
use ew_viewmodel::ew_api::output::{TableCp, TableRep};
use ew_viewmodel::ew_api::routing::{Measures, measure};
use ew_viewmodel::shell::{Outcome, ShellModel};
use serde_json::json;

/// The page point of grid index `k`.
fn page_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

/// A pointer click at the page point `p` as the window sends it; whether every step was taken.
fn click(s: &mut ShellModel, p: (i64, i64), alt: bool) -> bool {
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    ["canvas.pointer", "canvas.press", "canvas.release"]
        .into_iter()
        .all(|a| s.dispatch(a, &at(q, json!({ "alt": alt }))) == Outcome::Done)
}

/// A part with a symbol placed with its anchor at grid index `k`, without joining it to what stands beside it.
fn place(s: &mut ShellModel, part: &str, k: (i64, i64)) -> TableRep {
    let before = tables(s).reps.clone();
    done(s, "place.pick", json!({"part": part, "template": 0}));
    let p = page_point(s, k);
    assert!(click(s, p, true), "{part} at {k:?}");
    done(s, "tool.select", json!({}));
    let t = tables(s);
    let new: Vec<&TableRep> = t
        .reps
        .iter()
        .filter(|r| before.iter().all(|b| b.id != r.id))
        .collect();
    assert_eq!(new.len(), 1, "one new representation");
    new[0].clone()
}

/// The upper and the lower pin of a two-pin device.
fn pins(s: &ShellModel, rep: &TableRep) -> (TableCp, TableCp) {
    let mut cps: Vec<TableCp> = tables(s)
        .cps
        .iter()
        .filter(|c| c.rep == rep.id)
        .cloned()
        .collect();
    cps.sort_by_key(|c| (c.at.1, c.at.0));
    (cps[0].clone(), cps[cps.len() - 1].clone())
}

/// A wire drawn with two clicks, from the page point `a` to `b`.
fn wire(s: &mut ShellModel, a: (i64, i64), b: (i64, i64)) {
    let n = tables(s).connections.len();
    done(s, "tool.wire", json!({}));
    assert!(
        click(s, a, false) && click(s, b, false),
        "the wire {a:?} to {b:?}"
    );
    assert!(
        tables(s).connections.len() > n,
        "the wire {a:?} to {b:?} is drawn"
    );
}

/// The row a rail runs on: the y of the longest level stretch of the wire that ends at `end`.
fn rail_row(s: &ShellModel, end: (i64, i64)) -> i64 {
    let t = tables(s);
    let rail = t
        .connections
        .iter()
        .find(|c| c.polyline.last() == Some(&end))
        .expect("the rail");
    rail.polyline
        .windows(2)
        .filter(|w| w[0].1 == w[1].1)
        .max_by_key(|w| (w[0].0 - w[1].0).abs())
        .map(|w| w[0].1)
        .expect("a level stretch")
}

/// The measures of the page's routed wires, and how many wires have no route.
fn layout(s: &ShellModel) -> (Measures, usize) {
    let mut page = (*s.canvas().unwrap().page().geometry).clone();
    let unrouted = page.connections.iter().filter(|c| !c.routed).count();
    page.connections.retain(|c| c.routed);
    (measure(&page), unrouted)
}

/// What the bends and the crossings of a page weigh, by the router's own weights (a bend four steps, a crossing
/// six).
fn weight(m: &Measures) -> usize {
    8 * m.bend_total() + 12 * m.crossings.len()
}

fn project(tmp: &Tmp) -> ShellModel {
    let mut s = common::new_project(tmp);
    done(&mut s, "page.goto", json!({"folio": "101"}));
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

#[test]
fn a_full_page_is_no_worse_after_reroute() {
    let tmp = Tmp::new("reroute-full-page");
    let mut s = project(&tmp);
    let parts = ["EW-GEN-MCB-1", "EW-GEN-PB-1", "EW-GEN-PB-2", "EW-GEN-LMP-1"];
    let rows: Vec<Vec<TableRep>> = parts
        .iter()
        .zip(0_i64..)
        .map(|(part, r)| {
            (0..14)
                .map(|c| place(&mut s, part, (12 + 6 * c, 14 + 11 * r)))
                .collect()
        })
        .collect();
    // The columns: each device's lower pin to the upper pin of the one below.
    for pair in rows.windows(2) {
        for (upper, lower) in pair[0].iter().zip(&pair[1]) {
            let (a, b) = (pins(&s, upper).1.at, pins(&s, lower).0.at);
            wire(&mut s, a, b);
        }
    }
    // A rail above the first row and one below the last, from the first column to the last, a tap per column.
    for (r, upper) in [(0, true), (3, false)] {
        let end = |s: &ShellModel, c: usize| {
            let (up, down) = pins(s, &rows[r][c]);
            if upper { up.at } else { down.at }
        };
        let (a, b) = (end(&s, 0), end(&s, 13));
        wire(&mut s, a, b);
        let row = rail_row(&s, b);
        for c in 1..13 {
            let p = end(&s, c);
            wire(&mut s, p, (p.0, row));
        }
    }
    // A branch from each pushbutton to the contact of the next column, and a link from every third breaker to the
    // lamp four columns on.
    for (button, contact) in rows[1].iter().zip(rows[2].iter().skip(1)) {
        let (a, b) = (pins(&s, button).1.at, pins(&s, contact).0.at);
        wire(&mut s, a, b);
    }
    for c in (0..10).step_by(3) {
        let (a, b) = (pins(&s, &rows[0][c]).1.at, pins(&s, &rows[3][c + 4]).0.at);
        wire(&mut s, a, b);
    }
    let (drawn, unrouted) = layout(&s);
    assert_eq!(unrouted, 0, "every wire is drawn with a route");
    assert!(tables(&s).connections.len() > 100, "a full page of wires");

    done(&mut s, "tool.select", json!({}));
    common::select_ids(&mut s, &[]);
    done(&mut s, "route.reroute", json!({}));
    let (rerouted, unrouted) = layout(&s);
    assert_eq!(unrouted, 0, "every wire keeps a route");
    assert_eq!(
        (
            rerouted.touches.len(),
            rerouted.overlaps.len(),
            rerouted.through_bodies.len()
        ),
        (0, 0, 0),
        "no wire on a pin, on another wire or through a symbol: {rerouted:?}"
    );
    assert!(rerouted.hard() <= drawn.hard(), "{rerouted:?}");
    assert!(
        weight(&rerouted) <= weight(&drawn),
        "bends and crossings {} after, {} as drawn",
        weight(&rerouted),
        weight(&drawn)
    );
}

#[test]
fn a_wire_does_not_join_another_where_a_third_crosses_it() {
    let tmp = Tmp::new("reroute-crossing");
    let mut s = project(&tmp);
    // A wire from the lamp at the upper left to the one at the lower right, with a level stretch between them, and a
    // wire straight down across that stretch.
    let (left, right) = (
        place(&mut s, "EW-GEN-LMP-1", (20, 20)),
        place(&mut s, "EW-GEN-LMP-1", (40, 30)),
    );
    let (above, below) = (
        place(&mut s, "EW-GEN-LMP-1", (30, 12)),
        place(&mut s, "EW-GEN-LMP-1", (30, 40)),
    );
    let (a, b) = (pins(&s, &left).1.at, pins(&s, &right).0.at);
    wire(&mut s, a, b);
    let row = rail_row(&s, b);
    let (a, b) = (pins(&s, &above).1.at, pins(&s, &below).0.at);
    wire(&mut s, a, b);
    let crossing = (a.0, row);
    let (m, _) = layout(&s);
    assert!(
        m.crossings.iter().any(|c| c.at == crossing),
        "the second wire crosses the first at {crossing:?}: {m:?}"
    );
    // A third wire, from a lamp beside them, clicked onto the crossing: it joins neither wire there.
    let other = place(&mut s, "EW-GEN-LMP-1", (50, 12));
    let from = pins(&s, &other).1.at;
    let wires = tables(&s).connections.len();
    done(&mut s, "tool.wire", json!({}));
    click(&mut s, from, false);
    click(&mut s, crossing, false);
    let t = tables(&s);
    assert!(t.junctions.is_empty(), "no junction on a crossing");
    assert_eq!(t.connections.len(), wires, "and no wire ended there");
    s.dispatch("tool.cancel", &json!({}));
    done(&mut s, "tool.select", json!({}));
    // Two steps along the first wire, off the crossing, it joins.
    let beside = (page_point(&s, (32, 0)).0, row);
    wire(&mut s, from, beside);
    let t = tables(&s);
    assert_eq!(t.junctions.len(), 1, "a junction beside the crossing");
    assert_eq!(t.junctions[0].at, beside);
    let (m, unrouted) = layout(&s);
    assert_eq!((m.touches.len(), unrouted), (0, 0), "{m:?}");
}

/// The texts beside the pins of a multipin plug ("1 · 14", "COM") are texts of the router's page: a wire that would
/// run under the plug across them goes round them, and the wire of a pin runs past its own text, not across it.
#[test]
fn a_wire_goes_round_the_texts_beside_a_plug_s_pins() {
    let tmp = Tmp::new("reroute-plug-notes");
    let mut s = project(&tmp);
    done(&mut s, "place.pick", json!({"part": "EW-GEN-VTP-1"}));
    let (a, b) = (page_point(&s, (30, 20)), page_point(&s, (56, 28)));
    assert!(
        click(&mut s, a, false) && click(&mut s, b, false),
        "the plug's box"
    );
    done(&mut s, "tool.select", json!({}));
    let plug = tables(&s).reps.last().expect("the plug").clone();
    let below: Vec<TableCp> = tables(&s)
        .cps
        .iter()
        .filter(|c| c.rep == plug.id && c.at.1 >= b.1)
        .cloned()
        .collect();
    assert!(below.len() >= 2, "pins on the box's lower edge: {below:?}");
    // The texts under the box, one beside each lower pin.
    let under = |s: &ShellModel| -> Vec<[i64; 4]> {
        let g = &s.canvas().unwrap().page().geometry;
        g.texts
            .iter()
            .filter(|t| t.rep == plug.id && t.rect[1] >= b.1)
            .map(|t| t.rect)
            .collect()
    };
    let notes = under(&s);
    assert!(
        notes.len() >= below.len(),
        "a text beside each lower pin: {notes:?}"
    );
    // A wire between two lamps left and right of the plug, whose pins end one step under the box's lower edge: the
    // level way between them lies across the texts.
    let k = (b.1 - a.1) / 8;
    let row = |n: i64| b.1 + n * k;
    let (left, right) = (
        place(&mut s, "EW-GEN-LMP-1", (24, 25)),
        place(&mut s, "EW-GEN-LMP-1", (62, 25)),
    );
    let (from, to) = (pins(&s, &left).1.at, pins(&s, &right).1.at);
    assert!(from.1 < row(2), "the lamps' lower pins stand beside the texts");
    wire(&mut s, from, to);
    // A lamp under the plug, its upper pin in line with one of the plug's lower pins: the wire runs straight down
    // past that pin's own text.
    let pin = below[below.len() / 2].clone();
    let col = s.canvas().unwrap().page().snapper.grid_index_of(pin.at.0);
    let lamp = place(&mut s, "EW-GEN-LMP-1", (col, 40));
    let upper = pins(&s, &lamp).0.at;
    wire(&mut s, pin.at, upper);
    let t = tables(&s);
    let straight = t.connections.last().expect("the pin's wire");
    assert!(
        straight.polyline.iter().all(|p| (p.0 - pin.at.0).abs() <= 1),
        "straight down from the pin: {:?}",
        straight.polyline
    );
    let (m, unrouted) = layout(&s);
    assert_eq!(unrouted, 0);
    assert_eq!(m.through_texts, vec![], "no wire across a text");
    assert_eq!(m.hard(), 0, "{m:?}");
}
