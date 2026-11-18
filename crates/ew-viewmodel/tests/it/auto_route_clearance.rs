// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Wire tool's auto-route keeps wires apart and out of
//! boxes. In a new project, the play-test's situation is built through the window's own path: a 1P+N breaker from
//! its starter part, the power supply's black box from its part (two clicks), then the L wire from the breaker's
//! first pole to the box's L and the N wire from its second pole to the box's N, each a click on the one CP and a
//! click on the other. The L wire must not run across the box or over a representation's texts (but where it leaves
//! and enters its own CPs); the N wire must not run on the L wire's track. A route that cannot be found is drawn
//! direct and the tool says so.

use crate::common;

use common::{Tmp, at, done, tables};
use ew_viewmodel::ew_api::output::{TableConnection, TableCp, TableRep};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

/// A new project with the multi-line page 101 shown at actual size.
fn new_project(tmp: &Tmp) -> ShellModel {
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
        json!({"folio": "101", "type": "multi_line", "title": "Power supply"}),
    );
    done(&mut s, "page.goto", json!({"folio": "101"}));
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

/// The page point of grid index `k`.
fn page_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

/// The grid index of the page point `p`.
fn index_of(s: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.grid_index_of(p.0), sn.grid_index_of(p.1))
}

/// A pointer click at the page point `p` as the window sends it: the pointer there, the press, the release.
fn click(s: &mut ShellModel, p: (i64, i64), alt: bool) {
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(s, "canvas.pointer", at(q, json!({ "alt": alt })));
    done(s, "canvas.press", at(q, json!({ "alt": alt })));
    done(s, "canvas.release", at(q, json!({ "alt": alt })));
}

/// The pointer moved to the page point `p` (the ghost routes to it).
fn hover(s: &mut ShellModel, p: (i64, i64)) {
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(s, "canvas.pointer", at(q, json!({})));
}

/// The representations new since `before`.
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

/// The CPs of `rep`.
fn cps_of(s: &ShellModel, rep: &TableRep) -> Vec<TableCp> {
    tables(s)
        .cps
        .iter()
        .filter(|c| c.rep == rep.id)
        .cloned()
        .collect()
}

/// The play-test's page: the breaker -F1 (EW-GEN-MCB-2) with its two bottom pins, the black box of the power supply
/// (EW-GEN-PSU-1) whose top-left corner is 15 grid steps right of and 5 above the breaker's first bottom pin, 12 × 10
/// steps (frame t0014 of the play-test, measured).
struct Page {
    s: ShellModel,
    breaker: TableRep,
    psu: TableRep,
    /// The breaker's bottom pins, left to right.
    bottom: [TableCp; 2],
    /// The box's L and N.
    l: TableCp,
    n: TableCp,
}

fn page(tmp: &Tmp) -> Page {
    let mut s = new_project(tmp);
    let before = tables(&s).reps.clone();
    done(
        &mut s,
        "place.pick",
        json!({"part": "EW-GEN-MCB-2", "template": 0}),
    );
    let p = page_point(&s, (40, 20));
    click(&mut s, p, true);
    done(&mut s, "tool.select", json!({}));
    let breaker = new_rep(&s, &before);
    let mut bottom: Vec<TableCp> = cps_of(&s, &breaker)
        .into_iter()
        .filter(|c| c.dir == Some(ew_viewmodel::ew_api::geometry::Dir::Down))
        .collect();
    bottom.sort_by_key(|c| c.at.0);
    let bottom: [TableCp; 2] = bottom.try_into().expect("two bottom pins");
    let k = index_of(&s, bottom[0].at);
    let before = tables(&s).reps.clone();
    done(&mut s, "place.pick", json!({"part": "EW-GEN-PSU-1"}));
    assert_eq!(s.tool().name(), "black_box");
    let (a, b) = (
        page_point(&s, (k.0 + 15, k.1 - 5)),
        page_point(&s, (k.0 + 27, k.1 + 5)),
    );
    click(&mut s, a, false);
    click(&mut s, b, false);
    done(&mut s, "tool.select", json!({}));
    let psu = new_rep(&s, &before);
    let box_cps = cps_of(&s, &psu);
    let named = |n: &str| {
        box_cps
            .iter()
            .find(|c| c.label.ends_with(&format!(":{n}")))
            .unwrap_or_else(|| panic!("no {n} in {box_cps:?}"))
            .clone()
    };
    let (l, n) = (named("L"), named("N"));
    Page {
        s,
        breaker,
        psu,
        bottom,
        l,
        n,
    }
}

/// Draws a wire from `a` to `b` with two clicks; the new connection.
fn wire(s: &mut ShellModel, a: (i64, i64), b: (i64, i64)) -> TableConnection {
    let before = tables(s);
    let rev = s.revision();
    done(s, "tool.wire", json!({}));
    click(s, a, false);
    assert_eq!(s.tool().state(), "routing");
    hover(s, b);
    click(s, b, false);
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    let after = tables(s);
    let new: Vec<TableConnection> = after
        .connections
        .iter()
        .filter(|c| before.connections.iter().all(|o| o.id != c.id))
        .cloned()
        .collect();
    assert_eq!(new.len(), 1, "one new connection");
    new[0].clone()
}

/// The segments of a polyline, consecutive duplicates removed.
fn segments(line: &[(i64, i64)]) -> Vec<[(i64, i64); 2]> {
    let mut pts = line.to_vec();
    pts.dedup();
    pts.windows(2).map(|w| [w[0], w[1]]).collect()
}

/// Whether the axis-parallel segment `s` passes through the open interior of the rectangle `r`.
fn crosses(s: [(i64, i64); 2], r: [i64; 4]) -> bool {
    let [a, b] = s;
    let open = |lo: i64, hi: i64, v: i64| lo < v && v < hi;
    let overlap = |a0: i64, a1: i64, lo: i64, hi: i64| a0.min(a1).max(lo) < a0.max(a1).min(hi);
    if a.1 == b.1 {
        open(r[1], r[3], a.1) && overlap(a.0, b.0, r[0], r[2])
    } else {
        open(r[0], r[2], a.0) && overlap(a.1, b.1, r[1], r[3])
    }
}

/// Every rectangle the scene draws for a representation (its symbol and its texts) and a black box's outline, page
/// nm, with the representation.
fn rep_boxes(s: &ShellModel) -> Vec<(ew_viewmodel::ew_api::output::Uuid, [i64; 4])> {
    let page = s.canvas().unwrap().page();
    let mut out = Vec::new();
    for (e, r) in &page.entities {
        if !page.tables.reps.iter().any(|rep| rep.id == e.0) {
            continue;
        }
        let a = page.snapper.sheet_to_page((r.min().x.0, r.min().y.0));
        let b = page.snapper.sheet_to_page((r.max().x.0, r.max().y.0));
        out.push((e.0, [a.0, a.1, b.0, b.1]));
    }
    for rep in &page.tables.reps {
        if let Some(b) = &rep.black_box {
            let xs: Vec<i64> = b.outline.iter().map(|p| p.0).collect();
            let ys: Vec<i64> = b.outline.iter().map(|p| p.1).collect();
            let r = [
                *xs.iter().min().unwrap(),
                *ys.iter().min().unwrap(),
                *xs.iter().max().unwrap(),
                *ys.iter().max().unwrap(),
            ];
            out.push((rep.id, r));
        }
    }
    out
}

/// The places where a connection's route passes through a representation's box or over its texts: none, but for
/// its first segment over its first end's own representation and its last over its second end's (a CP is left and
/// entered straight through its own texts when they stand in its way).
fn through_boxes(s: &ShellModel, c: &TableConnection, ends: [&TableRep; 2]) -> Vec<String> {
    let segs = segments(&c.polyline);
    let n = segs.len();
    let mut out = Vec::new();
    for (k, seg) in segs.iter().enumerate() {
        for (rep, r) in rep_boxes(s) {
            let own = (k == 0 && rep == ends[0].id) || (k + 1 == n && rep == ends[1].id);
            if !own && crosses(*seg, r) {
                out.push(format!("segment {k} {seg:?} crosses {r:?}"));
            }
        }
    }
    out
}

/// Where the segments of `a` run on the track of a segment of `b` (parallel, closer than a grid step, overlapping),
/// and where an interior vertex of either lies on the other: a run on top of the other wire, or a touch that reads as
/// a junction.
fn overlaps(s: &ShellModel, a: &TableConnection, b: &TableConnection) -> Vec<String> {
    let g = s.canvas().unwrap().page().snapper.page_of_index(1)
        - s.canvas().unwrap().page().snapper.page_of_index(0);
    let mut out = Vec::new();
    let (sa, sb) = (segments(&a.polyline), segments(&b.polyline));
    for x in &sa {
        for y in &sb {
            let horizontal = |s: &[(i64, i64); 2]| s[0].1 == s[1].1;
            if horizontal(x) != horizontal(y) {
                continue;
            }
            let (track, lo, hi, ylo, yhi, ytrack) = if horizontal(x) {
                (
                    x[0].1,
                    x[0].0.min(x[1].0),
                    x[0].0.max(x[1].0),
                    y[0].0.min(y[1].0),
                    y[0].0.max(y[1].0),
                    y[0].1,
                )
            } else {
                (
                    x[0].0,
                    x[0].1.min(x[1].1),
                    x[0].1.max(x[1].1),
                    y[0].1.min(y[1].1),
                    y[0].1.max(y[1].1),
                    y[0].0,
                )
            };
            if (track - ytrack).abs() < g - 1 && lo.max(ylo) < hi.min(yhi) {
                out.push(format!("{x:?} runs on {y:?}"));
            }
        }
    }
    let on = |p: (i64, i64), s: &[(i64, i64); 2]| {
        let [a, b] = *s;
        (a.0 == b.0 && p.0 == a.0 && a.1.min(b.1) <= p.1 && p.1 <= a.1.max(b.1))
            || (a.1 == b.1 && p.1 == a.1 && a.0.min(b.0) <= p.0 && p.0 <= a.0.max(b.0))
    };
    for (one, other) in [(&sa, &sb), (&sb, &sa)] {
        let mut pts: Vec<(i64, i64)> = one.iter().map(|s| s[0]).collect();
        pts.remove(0);
        for p in pts {
            if other.iter().any(|s| on(p, s)) {
                out.push(format!("vertex {p:?} touches the other wire"));
            }
        }
    }
    out
}

#[test]
fn friction_7_the_l_wire_goes_round_the_box_and_over_no_text() {
    let tmp = Tmp::new("ft706-l");
    let mut p = page(&tmp);
    let l = wire(&mut p.s, p.bottom[0].at, p.l.at);
    assert_eq!(
        through_boxes(&p.s, &l, [&p.breaker, &p.psu]),
        Vec::<String>::new(),
        "{:?}",
        l.polyline
    );
    // It enters L from above, as the CP leaves (up), not from below through the box.
    let segs = segments(&l.polyline);
    let last = segs.last().unwrap();
    assert!(last[0].0 == p.l.at.0 && last[0].1 < p.l.at.1, "{:?}", l.polyline);
    assert_eq!(
        p.s.tool_hint(),
        "Click or Enter on a connection point, a junction or a wire to start, then on free points to lead the wire \
         through them"
    );
}

#[test]
fn friction_8_the_n_wire_keeps_off_the_l_wire() {
    let tmp = Tmp::new("ft706-n");
    let mut p = page(&tmp);
    let l = wire(&mut p.s, p.bottom[0].at, p.l.at);
    let n = wire(&mut p.s, p.bottom[1].at, p.n.at);
    assert_eq!(
        overlaps(&p.s, &n, &l),
        Vec::<String>::new(),
        "N {:?}, L {:?}",
        n.polyline,
        l.polyline
    );
    assert_eq!(
        through_boxes(&p.s, &n, [&p.breaker, &p.psu]),
        Vec::<String>::new(),
        "{:?}",
        n.polyline
    );
    // The stored routes of both are the ghosts' (the L wire is not re-routed by the second draw).
    let t = tables(&p.s);
    let l_now = t.connections.iter().find(|c| c.id == l.id).unwrap();
    assert_eq!(l_now.polyline, l.polyline);
}

#[test]
fn the_same_page_routes_the_same_wires() {
    let routes = |tag: &str| {
        let tmp = Tmp::new(tag);
        let mut p = page(&tmp);
        let l = wire(&mut p.s, p.bottom[0].at, p.l.at);
        let n = wire(&mut p.s, p.bottom[1].at, p.n.at);
        (l.polyline, n.polyline)
    };
    assert_eq!(routes("ft706-det-a"), routes("ft706-det-b"));
}

/// Without a clear route the ghost is the unrouted line, right-angled and never a diagonal, the hint says so,
/// and the commit stores the wire unrouted, drawn on the page as the ghost showed it; with a fixed bend from which no
/// clear way leads on, the same: no L tail through the box is stored (rc.3: such a tail ran through a pin and along
/// another wire).
#[test]
fn without_a_clear_route_the_wire_stays_unrouted_and_the_tool_says_so() {
    let tmp = Tmp::new("ft706-direct");
    let mut p = page(&tmp);
    // A second box drawn round the breaker's first bottom pin: the pin can leave by its stub only, into the box.
    let k = index_of(&p.s, p.bottom[0].at);
    done(&mut p.s, "place.pick", json!({"part": "EW-GEN-PSU-1"}));
    let (a, b) = (
        page_point(&p.s, (k.0 - 2, k.1 - 1)),
        page_point(&p.s, (k.0 + 2, k.1 + 3)),
    );
    click(&mut p.s, a, false);
    click(&mut p.s, b, false);
    done(&mut p.s, "tool.select", json!({}));
    let before = tables(&p.s);
    let rev = p.s.revision();
    done(&mut p.s, "tool.wire", json!({}));
    click(&mut p.s, p.bottom[0].at, false);
    assert_eq!(
        p.s.tool_hint(),
        "Click a connection point to end, a free point for a bend; Backspace removes the last bend; Esc cancels"
    );
    hover(&mut p.s, p.l.at);
    let unrouted = "No clear route: the wire stays a dashed line without a route. Click free points to lead it, or \
                    route it later with Reroute.";
    assert_eq!(p.s.tool_hint(), unrouted);
    let (from, to) = (p.bottom[0].at, p.l.at);
    let ghost = p.s.wire_ghost_route().expect("the unrouted line");
    assert_eq!((ghost.first(), ghost.last()), (Some(&from), Some(&to)));
    assert!(
        ghost.windows(2).all(|w| w[0].0 == w[1].0 || w[0].1 == w[1].1),
        "a diagonal: {ghost:?}"
    );
    click(&mut p.s, p.l.at, false);
    // Not a refusal: the wire is drawn, without a route.
    assert_eq!(p.s.revision(), rev.map(|r| r + 1), "one commit");
    let after = tables(&p.s);
    let new: Vec<&TableConnection> = after
        .connections
        .iter()
        .filter(|c| before.connections.iter().all(|o| o.id != c.id))
        .collect();
    assert_eq!(new.len(), 1);
    assert!(!new[0].stored_route, "stored unrouted");
    assert_eq!(new[0].polyline, ghost, "the page draws the line the ghost showed");
    // The hint stays with the drawn wire until the next one starts.
    assert_eq!(
        (p.s.tool().state(), p.s.tool_hint().as_str()),
        ("armed", unrouted)
    );
    click(&mut p.s, p.bottom[1].at, false);
    assert_eq!(
        p.s.tool_hint(),
        "Click a connection point to end, a free point for a bend; Backspace removes the last bend; Esc cancels"
    );
    done(&mut p.s, "ui.dismiss", json!({}));
    // A fixed bend in the box, then the same end: no clear way from the start either, so the wire is stored unrouted.
    let before = tables(&p.s);
    click(&mut p.s, p.bottom[0].at, false);
    let bend = page_point(&p.s, (k.0, k.1 + 2));
    click(&mut p.s, bend, false);
    click(&mut p.s, p.l.at, false);
    let after = tables(&p.s);
    let new: Vec<&TableConnection> = after
        .connections
        .iter()
        .filter(|c| before.connections.iter().all(|o| o.id != c.id))
        .collect();
    assert_eq!(new.len(), 1);
    assert!(!new[0].stored_route, "no route that breaks the rules");
    assert_eq!(new[0].polyline.first(), Some(&from));
    assert_eq!(new[0].polyline.last(), Some(&to));
}

/// A click that places a bend instead of ending on a connection point (a miss snaps to the grid) is told
/// in the hint and the HUD's prompt: "Bend placed: click a connection point to end the wire."; the wire then ends on a
/// connection point as before, and the next wire starts with the routing hint.
#[test]
fn a_click_that_places_a_bend_says_so() {
    let tmp = Tmp::new("id1497-bend");
    let mut p = page(&tmp);
    done(&mut p.s, "tool.wire", json!({}));
    click(&mut p.s, p.bottom[0].at, false);
    let k = index_of(&p.s, p.bottom[0].at);
    let bend_at = page_point(&p.s, (k.0 + 4, k.1 + 4));
    click(&mut p.s, bend_at, false);
    let bend = "Bend placed: click a connection point to end the wire.";
    assert_eq!(p.s.tool_hint(), bend);
    assert_eq!(ew_viewmodel::command_bar::hud(&p.s).prompt, bend);
    let rev = p.s.revision();
    click(&mut p.s, p.l.at, false);
    assert_eq!(
        p.s.revision(),
        rev.map(|r| r + 1),
        "the wire ends on the connection point"
    );
    click(&mut p.s, p.bottom[1].at, false);
    assert_eq!(
        p.s.tool_hint(),
        "Click a connection point to end, a free point for a bend; Backspace removes the last bend; Esc cancels"
    );
}

/// The rc.2 persona run on a cabinet: aiming at the power supply's pin, a click landed on the pin's text inside the box,
/// fixed a bend there, and the wire ran through the box. A click inside a symbol is no bend: it is refused in words,
/// the wire keeps routing, and ends on the next connection point clear of the box.
#[test]
fn a_click_inside_a_symbol_places_no_bend() {
    let tmp = Tmp::new("rc2-bend-in-box");
    let mut p = page(&tmp);
    done(&mut p.s, "tool.wire", json!({}));
    click(&mut p.s, p.bottom[1].at, false);
    // A step above the box's bottom edge, where its pin texts stand.
    let k = index_of(&p.s, p.bottom[0].at);
    let inside = page_point(&p.s, (k.0 + 21, k.1 + 4));
    let q = p.s.canvas().unwrap().page().snapper.page_to_sheet(inside);
    done(&mut p.s, "canvas.pointer", at(q, json!({})));
    assert_eq!(
        common::refused(&mut p.s, "canvas.press", at(q, json!({}))),
        "reason-wire-bend-in-symbol"
    );
    done(&mut p.s, "canvas.release", at(q, json!({})));
    assert_eq!(p.s.tool().state(), "routing");
    let before = tables(&p.s);
    hover(&mut p.s, p.n.at);
    click(&mut p.s, p.n.at, false);
    let after = tables(&p.s);
    let new: Vec<&TableConnection> = after
        .connections
        .iter()
        .filter(|c| before.connections.iter().all(|o| o.id != c.id))
        .collect();
    assert_eq!(new.len(), 1);
    assert!(new[0].stored_route, "{:?}", new[0].polyline);
    assert_eq!(
        through_boxes(&p.s, new[0], [&p.breaker, &p.psu]),
        Vec::<String>::new(),
        "{:?}",
        new[0].polyline
    );
}

/// The owner's case (rc.3): a two-pole breaker over another, two grid steps between the upper's bottom pins and the
/// lower's top pins, in a new project; the upper's bottom pins and the lower's top pins, each left to right.
fn breakers_over_one_another(tmp: &Tmp) -> (ShellModel, Vec<TableCp>, Vec<TableCp>) {
    use ew_viewmodel::ew_api::geometry::Dir;
    let mut s = new_project(tmp);
    let place = |s: &mut ShellModel, k: (i64, i64)| {
        let before = tables(s).reps.clone();
        done(s, "place.pick", json!({"part": "EW-GEN-MCB-2", "template": 0}));
        let p = page_point(s, k);
        click(s, p, true);
        done(s, "tool.select", json!({}));
        new_rep(s, &before)
    };
    let pins = |s: &ShellModel, rep: &TableRep, dir: Dir| {
        let mut v: Vec<TableCp> = cps_of(s, rep)
            .into_iter()
            .filter(|c| c.dir == Some(dir))
            .collect();
        v.sort_by_key(|c| c.at.0);
        v
    };
    let upper = place(&mut s, (40, 20));
    let bottom = pins(&s, &upper, Dir::Down);
    let height = index_of(&s, bottom[0].at).1 - index_of(&s, pins(&s, &upper, Dir::Up)[0].at).1;
    let lower = place(&mut s, (40, 20 + height + 2));
    let top = pins(&s, &lower, Dir::Up);
    assert_eq!(
        index_of(&s, top[0].at).1 - index_of(&s, bottom[0].at).1,
        2,
        "two grid steps apart"
    );
    (s, bottom, top)
}

/// The two wires wired cross-over read as the owner asked (ADR-0033 amendment 3): every hard rule kept, one crossing
/// between the two, no corner of one on a corner or an end of the other (it reads as a joint), and neither a detour
/// (more than 8/5 of the way between its pins).
#[track_caller]
fn crossed_once(s: &ShellModel, wires: [ew_viewmodel::ew_api::output::Uuid; 2]) {
    let page = s.canvas().unwrap().page();
    let m = ew_viewmodel::ew_api::routing::measure(&page.geometry);
    assert_eq!(m.hard(), 0, "{m:?}");
    let crossings: Vec<_> = m
        .crossings
        .iter()
        .filter(|c| wires.contains(&c.connections[0]) && wires.contains(&c.connections[1]))
        .collect();
    assert_eq!(crossings.len(), 1, "{:?}", m.crossings);
    let line = |id| {
        let c = page
            .tables
            .connections
            .iter()
            .find(|c| c.id == id)
            .expect("the wire");
        assert!(c.stored_route, "routed");
        c.polyline.clone()
    };
    let (a, b) = (line(wires[0]), line(wires[1]));
    let corners = |l: &[(i64, i64)]| l[1..l.len() - 1].to_vec();
    assert!(
        corners(&a).iter().all(|p| !b.contains(p)) && corners(&b).iter().all(|p| !a.contains(p)),
        "corners meet: {a:?} {b:?}"
    );
    for l in [&a, &b] {
        let (first, last) = (l[0], l[l.len() - 1]);
        let direct = (first.0 - last.0).abs() + (first.1 - last.1).abs();
        let length: i64 = l
            .windows(2)
            .map(|w| (w[0].0 - w[1].0).abs() + (w[0].1 - w[1].1).abs())
            .sum();
        assert!(length * 5 <= direct * 8, "a detour: {l:?}");
    }
}

/// The open findings of Run checks that say a wire runs through a connection point (EW-CORE-067) or along another wire
/// (EW-CORE-068), as `rule subject`: the cross-over's wires must give none, drawn or rerouted.
fn through_pins_or_along(s: &mut ShellModel) -> Vec<String> {
    use ew_viewmodel::navigators::{NavKind, NavRole};
    done(s, "messages.show", json!({"on": true}));
    done(s, "messages.check", json!({}));
    let t = s
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .cloned()
        .expect("the Messages table");
    t.nodes
        .iter()
        .filter(|n| {
            n.role == NavRole::Finding && ["EW-CORE-067", "EW-CORE-068"].contains(&n.cells[1].as_str())
        })
        .map(|n| n.cells.join(" "))
        .collect()
}

/// rc.3 (the owner's case): the two breakers wired cross-over with two clicks per wire, as freshly drawn, cross once
/// and run straight otherwise; Reroute on the page gives the same.
#[test]
fn two_breakers_wired_cross_over_cross_once_drawn_and_rerouted() {
    let tmp = Tmp::new("rc3-cross-drawn");
    let (mut s, bottom, top) = breakers_over_one_another(&tmp);
    let first = wire(&mut s, bottom[1].at, top[0].at);
    let second = wire(&mut s, top[1].at, bottom[0].at);
    done(&mut s, "tool.select", json!({}));
    crossed_once(&s, [first.id, second.id]);
    assert_eq!(through_pins_or_along(&mut s), Vec::<String>::new(), "drawn");
    done(&mut s, "route.reroute", json!({}));
    crossed_once(&s, [first.id, second.id]);
    assert_eq!(through_pins_or_along(&mut s), Vec::<String>::new(), "rerouted");
}

/// rc.3 (the owner's case): the second wire drawn with a free point on the first wire's corner, the way he drew it,
/// keeps every hard rule; Reroute on both, in at most one undo step, leaves them crossing once and running straight
/// otherwise, with no more bends than drawn (rc.5: a reroute starts from the drawing and keeps it where routing the
/// two from nothing gives more corners, as it did here).
#[test]
fn two_breakers_wired_cross_over_keep_the_hard_rules_drawn_and_rerouted() {
    let tmp = Tmp::new("rc3-cross-over");
    let (mut s, bottom, top) = breakers_over_one_another(&tmp);
    let clean = |s: &ShellModel| {
        let m = ew_viewmodel::ew_api::routing::measure(&s.canvas().unwrap().page().geometry);
        assert!(
            m.touches.is_empty() && m.overlaps.is_empty() && m.through_bodies.is_empty(),
            "{:?} {:?} {:?}",
            m.touches,
            m.overlaps,
            m.through_bodies
        );
    };
    // The first wire, upper right pin to lower left pin.
    let first = wire(&mut s, bottom[1].at, top[0].at);
    clean(&s);
    // The second, from the lower right pin, with a free point where the first wire turns below the upper right pin,
    // to the upper left pin.
    let before = tables(&s);
    done(&mut s, "tool.wire", json!({}));
    click(&mut s, top[1].at, false);
    let k = index_of(&s, bottom[1].at);
    let corner = page_point(&s, (k.0, k.1 + 1));
    click(&mut s, corner, true);
    click(&mut s, bottom[0].at, false);
    done(&mut s, "tool.select", json!({}));
    let second = tables(&s)
        .connections
        .iter()
        .find(|c| before.connections.iter().all(|o| o.id != c.id))
        .expect("the second wire")
        .clone();
    clean(&s);
    assert_eq!(through_pins_or_along(&mut s), Vec::<String>::new(), "drawn");
    // Reroute both: at most one undo step, then one crossing.
    done(
        &mut s,
        "select.set",
        json!({"ids": [first.id.to_string(), second.id.to_string()]}),
    );
    let rev = s.revision();
    let bends = |s: &ShellModel| {
        ew_viewmodel::ew_api::routing::measure(&s.canvas().unwrap().page().geometry).bend_total()
    };
    let drawn = bends(&s);
    done(&mut s, "route.reroute", json!({}));
    assert!(s.revision() <= rev.map(|r| r + 1), "at most one step");
    assert!(bends(&s) <= drawn, "no more bends than drawn");
    crossed_once(&s, [first.id, second.id]);
    assert_eq!(through_pins_or_along(&mut s), Vec::<String>::new(), "rerouted");
}
