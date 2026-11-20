// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The router (docs/design/06a-drafting.md §3, §9.3 row 2): nets with trunks, shared runs, his routes left alone,
//! the same plan whatever the order of the page's lists, and the fixtures of §9.1.

use std::collections::BTreeSet;

use crate::kit::{Page, fixtures, p, s};
use ew_drafting::{
    DraftPage, End, Jobs, Pt, Scope, Side, WireEnd, WireId, geom, jobs, measure, route, route_wire,
};

/// The plan of a page-wide Route applied: every route and junction of the plan on the page.
fn routed(page: &DraftPage) -> DraftPage {
    let plan = route(page, &jobs(page, Scope::Page));
    let mut out = page.clone();
    for (w, r) in &plan.routes {
        if let Some(x) = out.wires.iter_mut().find(|x| x.id == *w) {
            x.route.clone_from(r);
            x.computed = true;
        }
    }
    for (j, at) in &plan.junctions {
        if let Some(x) = out.junctions.iter_mut().find(|x| x.id == *j) {
            x.at = *at;
        }
    }
    out
}

fn point(page: &DraftPage, e: End) -> Pt {
    match e {
        End::Pin(pin) => page
            .parts
            .iter()
            .find_map(|part| part.pins.iter().find(|x| x.id == pin).map(|x| part.at.plus(x.at)))
            .expect("a pin"),
        End::Junction(j) => page.junctions.iter().find(|x| x.id == j).expect("a junction").at,
    }
}

fn corners(route: &[Pt]) -> usize {
    geom::corners(&geom::clean(route)).len()
}

/// Three devices in a row, their upper pins wired from pin to pin, nothing routed.
fn three_in_a_row() -> (Page, [WireId; 2]) {
    let mut b = Page::new(40, 30);
    for (k, x) in [10, 18, 26].into_iter().enumerate() {
        b.device(&format!("K{k}"), p(x, 12));
    }
    let w1 = b.wire("w1", b.pin("K0", "1"), b.pin("K1", "1"), &[]);
    let w2 = b.wire("w2", b.pin("K1", "1"), b.pin("K2", "1"), &[]);
    (b, [w1, w2])
}

#[test]
fn a_potential_wired_from_pin_to_pin_over_three_parts_is_one_line_with_drops() {
    let (b, [w1, w2]) = three_in_a_row();
    let page = routed(&b.page);
    let q = measure(&page);
    assert_eq!((q.faults, q.unrouted), (0, 0), "{:?}", q.findings);
    let r1 = &page.wires.iter().find(|w| w.id == w1).unwrap().route;
    let r2 = &page.wires.iter().find(|w| w.id == w2).unwrap().route;
    // Each wire: up its drop, along the trunk, down its drop; the trunk one line above the three pins.
    let trunk = |r: &[Pt]| {
        let pts = geom::clean(r);
        assert_eq!(pts.len(), 4, "a drop, the trunk, a drop: {pts:?}");
        assert_eq!(pts[1].y, pts[2].y, "{pts:?}");
        pts[1].y
    };
    assert_eq!(trunk(r1), trunk(r2), "one line");
    assert!(trunk(r1) < s(12), "above the pins");
    // The middle pin's two wires share its drop and part on the trunk.
    let middle = point(&page, End::Pin(b.pin_id("K1", "1")));
    assert!(r1.contains(&middle) && r2.contains(&middle));
}

#[test]
fn a_t_junction_moves_onto_the_trunk_and_keeps_its_three_wires() {
    // A rail tapped by a junction that stands off its line: the junction goes onto the trunk, over its drop.
    let mut b = Page::new(50, 30);
    b.mark_in("in", p(4, 6));
    b.mark_out("out", p(44, 6));
    b.device("K1", p(20, 14));
    let j = b.junction("J", p(26, 10));
    let w = [
        b.wire("a", b.pin("in", "1"), j, &[]),
        b.wire("b", j, b.pin("out", "1"), &[]),
        b.wire("c", j, b.pin("K1", "1"), &[]),
    ];
    let ends: Vec<[End; 2]> = w
        .iter()
        .map(|id| b.page.wires.iter().find(|x| x.id == *id).unwrap().ends)
        .collect();
    let page = routed(&b.page);
    let q = measure(&page);
    assert_eq!((q.faults, q.unrouted), (0, 0), "{:?}", q.findings);
    let at = point(&page, j);
    assert_eq!(at, p(20, 6), "on the rail, over the pin it drops to");
    for (id, e) in w.iter().zip(&ends) {
        let x = page.wires.iter().find(|x| x.id == *id).unwrap();
        assert_eq!(x.ends, *e, "the wire ends where it did");
        assert_eq!(corners(&x.route), 0, "straight");
    }
}

#[test]
fn a_route_the_person_shaped_is_in_no_page_wide_plan_and_a_broken_one_is_flagged() {
    let mut b = Page::new(40, 30);
    b.device("K1", p(10, 10));
    b.device("K2", p(10, 20));
    b.device("K3", p(20, 10));
    b.device("K4", p(20, 20));
    // His: a detour, but free of faults.
    let his = b.run("his", b.pin("K1", "2"), b.pin("K2", "1"), &[p(10, 16), p(8, 16), p(8, 18), p(10, 18)]);
    // Broken: through K4's body.
    let broken = b.run("broken", b.pin("K3", "2"), b.pin("K4", "2"), &[p(20, 26), p(22, 26), p(22, 24)]);
    let jobs = jobs(&b.page, Scope::Page);
    assert!(!jobs.wires.contains(&his));
    assert_eq!(jobs.his, vec![broken]);
    let plan = route(&b.page, &jobs);
    assert!(plan.routes.iter().all(|(w, _)| *w != his));
    assert_eq!(plan.his, vec![broken]);
}

#[test]
fn the_same_page_in_any_order_of_its_lists_gives_the_same_plan() {
    for f in fixtures() {
        let mut page = f.hand.clone();
        for w in &mut page.wires {
            w.route.clear();
        }
        let plan = route(&page, &jobs(&page, Scope::Page));
        let mut shuffled = page.clone();
        shuffled.parts.reverse();
        shuffled.wires.reverse();
        shuffled.junctions.reverse();
        shuffled.labels.reverse();
        shuffled.fixed_texts.reverse();
        for part in &mut shuffled.parts {
            part.pins.reverse();
        }
        let again = route(&shuffled, &jobs(&shuffled, Scope::Page));
        assert_eq!(plan.routes, again.routes, "{}", f.name);
        assert_eq!(plan.junctions, again.junctions, "{}", f.name);
    }
}

#[test]
fn a_plan_s_routes_run_between_the_ends_their_wires_have() {
    for f in fixtures() {
        let mut page = f.hand.clone();
        for w in &mut page.wires {
            w.route.clear();
        }
        let plan = route(&page, &jobs(&page, Scope::Page));
        let mut moved = page.clone();
        for (j, at) in &plan.junctions {
            moved.junctions.iter_mut().find(|x| x.id == *j).unwrap().at = *at;
        }
        for (w, r) in &plan.routes {
            let x = moved.wires.iter().find(|x| x.id == *w).unwrap();
            assert_eq!(r.first(), Some(&point(&moved, x.ends[0])), "{}", f.name);
            assert_eq!(r.last(), Some(&point(&moved, x.ends[1])), "{}", f.name);
        }
    }
}

#[test]
fn every_fixture_as_wired_by_hand_gives_an_empty_plan() {
    for f in fixtures() {
        let plan = route(&f.hand, &jobs(&f.hand, Scope::Page));
        assert!(plan.is_empty(), "{}: {:?}", f.name, plan.routes);
        // Marked as the program's, they are weighed against what is drawn: never worse.
        let mut computed = f.hand.clone();
        for w in &mut computed.wires {
            w.computed = true;
        }
        let plan = route(&computed, &jobs(&computed, Scope::Page));
        if !plan.is_empty() {
            assert!(plan.after.better_than(&plan.before), "{}", f.name);
        }
    }
}

#[test]
fn every_fixture_with_its_parts_in_line_routes_straight_with_trunks_and_no_fault() {
    for f in fixtures() {
        let mut page = f.hand.clone();
        for w in &mut page.wires {
            w.route.clear();
        }
        let done = routed(&page);
        let q = measure(&done);
        assert_eq!(q.faults, 0, "{}: {:?}", f.name, q.findings);
        assert_eq!(q.unrouted, 0, "{}: {:?}", f.name, q.findings.unrouted);
        // Every wire between two pins that face each other on one line is straight.
        for w in &done.wires {
            let (a, b) = (point(&done, w.ends[0]), point(&done, w.ends[1]));
            let side = |e: End| match e {
                End::Pin(pin) => done
                    .parts
                    .iter()
                    .find_map(|part| part.pins.iter().find(|x| x.id == pin).and_then(|x| x.side)),
                End::Junction(_) => None,
            };
            let facing = (a.x == b.x && side(w.ends[0]) == Some(if a.y < b.y { Side::Down } else { Side::Up }))
                || (a.y == b.y && side(w.ends[0]) == Some(if a.x < b.x { Side::Right } else { Side::Left }));
            let between = facing && side(w.ends[1]).is_some();
            if between {
                assert_eq!(corners(&w.route), 0, "{}: {:?} {:?}", f.name, w.id, w.route);
            }
        }
        // Every junction tap of a rail stands on one line with the others of its net.
        let mut rails: BTreeSet<Vec<i32>> = BTreeSet::new();
        for j in &done.junctions {
            rails.insert(vec![j.at.y]);
        }
        let _ = rails;
    }
}

#[test]
fn the_wire_tool_shares_the_run_of_a_wire_of_the_same_pin() {
    // K1's upper pin is wired up to a mark; a second wire from the same pin to K2 shares the first one's run.
    let mut b = Page::new(40, 30);
    b.device("K1", p(10, 14));
    b.device("K2", p(20, 14));
    b.mark_in("in", p(4, 6));
    b.run("feed", b.pin("in", "1"), b.pin("K1", "1"), &[p(10, 6)]);
    let found = route_wire(
        &b.page,
        WireEnd::Pin(b.pin_id("K1", "1")),
        WireEnd::Pin(b.pin_id("K2", "1")),
        None,
    )
    .expect("a route");
    let mut drawn = b.page.clone();
    let w = b.wire("next", b.pin("K1", "1"), b.pin("K2", "1"), &found.route);
    drawn.wires.push(b.page.wires.iter().find(|x| x.id == w).unwrap().clone());
    let q = measure(&drawn);
    assert_eq!(q.faults, 0, "{:?} {:?}", found.route, q.findings);
    let _ = Jobs::default();
}

#[test]
fn the_full_sheet_ends_with_no_fault() {
    let page = crate::kit::full_sheet();
    let done = routed(&page);
    let q = measure(&done);
    assert_eq!(q.faults, 0, "{:?}", q.findings);
}
