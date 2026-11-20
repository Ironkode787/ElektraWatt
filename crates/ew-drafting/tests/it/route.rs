// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The router (docs/design/06a-drafting.md §3, §9.3 row 2): nets with trunks, shared runs, his routes left alone,
//! the same plan whatever the order of the page's lists, and the fixtures of §9.1.

use crate::kit::{Page, fixtures, p, s};
use ew_drafting::{DraftPage, End, Pt, Scope, Side, WireEnd, WireId, geom, jobs, measure, route, route_wire};

/// The plan of a page-wide Route applied: every route and junction of the plan on the page.
pub fn routed(page: &DraftPage) -> DraftPage {
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
    let his = b.run(
        "his",
        b.pin("K1", "2"),
        b.pin("K2", "1"),
        &[p(10, 16), p(8, 16), p(8, 18), p(10, 18)],
    );
    // Broken: through K4's body.
    let broken = b.run(
        "broken",
        b.pin("K3", "2"),
        b.pin("K4", "2"),
        &[p(20, 26), p(22, 26), p(22, 24)],
    );
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
            let facing = (a.x == b.x
                && side(w.ends[0]) == Some(if a.y < b.y { Side::Down } else { Side::Up }))
                || (a.y == b.y && side(w.ends[0]) == Some(if a.x < b.x { Side::Right } else { Side::Left }));
            let between = facing && side(w.ends[1]).is_some();
            if between {
                assert_eq!(corners(&w.route), 0, "{}: {:?} {:?}", f.name, w.id, w.route);
            }
        }
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
    drawn
        .wires
        .push(b.page.wires.iter().find(|x| x.id == w).unwrap().clone());
    let q = measure(&drawn);
    assert_eq!(q.faults, 0, "{:?} {:?}", found.route, q.findings);
}

#[test]
fn the_full_sheet_ends_with_no_fault() {
    let page = crate::kit::full_sheet();
    let done = routed(&page);
    let q = measure(&done);
    assert_eq!(q.faults, 0, "{:?}", q.findings);
}

#[test]
fn route_over_a_person_s_route_takes_a_cheaper_way_with_no_new_finding() {
    // His wire from K1's upper pin to K2's runs high over K5, round the wire that drops from K5 to K6 between them;
    // the short way crosses that wire. A crossing is weighed in the cost, not a finding (§1.2): a Route of the
    // selection takes the short way, as the page costs less and no wire or text gets a finding it did not have.
    let mut b = Page::new(40, 36);
    b.device("K1", p(10, 18));
    b.device("K2", p(20, 18));
    b.device("K5", p(15, 8));
    b.device("K6", p(15, 26));
    b.run("drop", b.pin("K5", "2"), b.pin("K6", "1"), &[]);
    let his = b.run("his", b.pin("K1", "1"), b.pin("K2", "1"), &[p(10, 6), p(20, 6)]);
    let selected = [his.0];
    let page = b.page.clone();
    let plan = route(&page, &jobs(&page, Scope::Selection(&selected)));
    assert!(plan.routes.iter().any(|(w, _)| *w == his), "{:?}", plan.routes);
    let mut done = page.clone();
    for (w, r) in &plan.routes {
        if let Some(x) = done.wires.iter_mut().find(|x| x.id == *w) {
            x.route.clone_from(r);
        }
    }
    let (before, after) = (measure(&page), measure(&done));
    assert!(after.better_than(&before), "{:?} {:?}", before.key(), after.key());
    let f = &after.findings;
    assert!(
        f.through_points.is_empty()
            && f.on_wires.is_empty()
            && f.through_parts.is_empty()
            && f.four_way.is_empty()
            && f.false_joints.is_empty()
            && f.across_texts.is_empty()
            && f.texts_on_texts.is_empty()
            && f.texts_on_parts.is_empty()
            && f.texts_outside.is_empty(),
        "{f:?}"
    );
}

#[test]
fn a_wire_leaves_a_pin_that_shares_its_point_with_a_free_connection_point() {
    // A terminal with two connection points at one point of its lower edge, one of them wired to a mark at the right:
    // the free point's stub is the wired one's way out, its half step as well as its full one.
    let mut b = Page::new(40, 30);
    b.part(
        "X1",
        p(10, 10),
        (p(-1, -2), p(1, 0)),
        &[
            ("1", p(0, -2), Side::Up),
            ("2", p(0, 0), Side::Down),
            ("2b", p(0, 0), Side::Down),
        ],
    );
    b.mark_out("out", p(20, 14));
    let w = b.wire("w", b.pin("X1", "2"), b.pin("out", "1"), &[]);
    let done = routed(&b.page);
    let q = measure(&done);
    assert_eq!((q.faults, q.unrouted), (0, 0), "{:?}", q.findings);
    assert_eq!(corners(&done.wires.iter().find(|x| x.id == w).unwrap().route), 1);
}

/// A small fluid page: a valve terminal's outline round a valve, a service unit below it and a cylinder above, in line.
/// The tube between them goes round the terminal, not across it; a tube stored straight across it is F4, the valve's
/// own tube straight out of its port is not.
#[test]
fn a_tube_goes_round_a_valve_terminal_and_one_across_it_is_f4() {
    let mut b = Page::new(60, 60);
    let frame = b.part("VT", p(20, 30), (p(0, 0), p(20, 10)), &[]);
    if let Some(part) = b.page.parts.iter_mut().find(|x| x.id == frame) {
        part.frame_only = true;
    }
    b.part("V0", p(22, 30), (p(0, 0), p(6, 10)), &[("2", p(2, 0), Side::Up)]);
    b.part(
        "FRL",
        p(28, 50),
        (p(0, 0), p(4, 4)),
        &[("out", p(2, 0), Side::Up)],
    );
    b.part("C", p(28, 16), (p(0, 0), p(4, 6)), &[("a", p(2, 6), Side::Down)]);
    b.part("C2", p(22, 16), (p(0, 0), p(4, 6)), &[("a", p(2, 6), Side::Down)]);
    let tube = b.wire("t", b.pin("FRL", "out"), b.pin("C", "a"), &[]);
    b.run("v", b.pin("V0", "2"), b.pin("C2", "a"), &[]);
    // Routed, and with no fault: round the terminal, as F4 holds its outline.
    let done = routed(&b.page);
    let q = measure(&done);
    assert_eq!((q.faults, q.unrouted), (0, 0), "{:?}", q.findings);
    let line = &done.wires.iter().find(|w| w.id == tube).unwrap().route;
    assert!(line.len() > 2, "not the straight line across it: {line:?}");
    let mut across = b.page.clone();
    across.wires.iter_mut().find(|w| w.id == tube).unwrap().route = vec![p(30, 50), p(30, 22)];
    let f = measure(&across).findings;
    assert!(
        f.through_parts
            .iter()
            .any(|(w, part, _)| *w == tube && *part == frame),
        "{f:?}"
    );
    assert_eq!(
        f.through_parts.len(),
        1,
        "the valve's own tube leaves straight out of its port: {f:?}"
    );
}

/// A junction with two drops stands over one of them (§3.4): its wires leave it different ways, none lies along
/// another, and the net is routed without a fault.
#[test]
fn a_junction_with_two_drops_stands_over_one_of_them() {
    let mut b = Page::new(60, 30);
    b.mark_in("L", p(6, 6));
    for (k, x) in [16, 28, 34].into_iter().enumerate() {
        b.device(&format!("K{k}"), p(x, 12));
    }
    let (j0, j1) = (b.junction("J0", p(16, 6)), b.junction("J1", p(31, 6)));
    b.wire("a", b.pin("L", "1"), j0, &[]);
    b.wire("d0", j0, b.pin("K0", "1"), &[]);
    b.wire("b", j0, j1, &[]);
    b.wire("d1", j1, b.pin("K1", "1"), &[]);
    b.wire("d2", j1, b.pin("K2", "1"), &[]);
    let page = routed(&b.page);
    let q = measure(&page);
    assert_eq!((q.faults, q.unrouted), (0, 0), "{:?}", q.findings);
    let at = point(&page, j1);
    assert!(at.x == s(28) || at.x == s(34), "{at:?}");
}

/// Between a pin and the pins it feeds that face it, a tap may stand on a line across, two steps from either or in the
/// middle, in line with each end of the net (§3.4).
#[test]
fn a_tap_may_stand_on_a_line_across_between_a_pin_and_the_pins_that_face_it() {
    let ends = [
        (p(10, 10), Some(Side::Down)),
        (p(10, 20), Some(Side::Up)),
        (p(16, 20), Some(Side::Up)),
    ];
    let taps = ew_drafting::nets::tap_points(p(10, 15), &ends);
    for want in [p(10, 12), p(16, 12), p(16, 15), p(10, 18), p(16, 18)] {
        assert!(taps.contains(&want), "{want:?} of {taps:?}");
    }
}
