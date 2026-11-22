// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A kept route is weighed per wire (docs/design/06a-drafting.md §3.9): a net's new routes are not all or nothing.

use crate::kit::{Page, TAG, p};
use ew_drafting::{
    DraftPage, FixedText, LabelKind, Owner, Places, Pt, Rect, Scope, Side, Tick, WireId, jobs, route,
};

/// A drawn box `w` by `h` steps at `at` with pins on its upper edge at `top` and on its lower edge at `bottom` (steps
/// from its left edge, named `t0`.. and `b0`..), each with a pin text inside: a card in the older form, or a CPU.
fn two_rows(b: &mut Page, name: &str, at: Pt, (w, h): (Tick, Tick), top: &[Tick], bottom: &[Tick]) {
    let names: Vec<(String, Tick, bool)> = top
        .iter()
        .enumerate()
        .map(|(i, x)| (format!("t{i}"), *x, true))
        .chain(bottom.iter().enumerate().map(|(i, x)| (format!("b{i}"), *x, false)))
        .collect();
    let pins: Vec<(&str, Pt, Side)> = names
        .iter()
        .map(|(n, x, up)| {
            if *up {
                (n.as_str(), p(*x, 0), Side::Up)
            } else {
                (n.as_str(), p(*x, h), Side::Down)
            }
        })
        .collect();
    let part = b.part(name, at, (p(0, 0), p(w, h)), &pins);
    for (_, x, up) in &names {
        let x = at.x + p(*x, 0).x;
        let y0 = if *up { at.y + 6 } else { at.y + p(0, h).y - 44 };
        b.page.fixed_texts.push(FixedText {
            owner: part.0,
            rect: Rect::new(Pt::new(x - 10, y0), Pt::new(x + 11, y0 + 38)),
        });
    }
    b.label(Owner::Part(part), 0, LabelKind::BoxTag, TAG, Places::At(Vec::new()));
}

/// A two-pin upright symbol three steps high (a lamp, a contact): pins `1` on top and `2` at the bottom.
fn upright(b: &mut Page, name: &str, at: Pt) {
    let part = b.part(
        name,
        at,
        (Pt::new(-12, 0), Pt::new(12, 48)),
        &[("1", p(0, 0), Side::Up), ("2", p(0, 3), Side::Down)],
    );
    b.tag(part, TAG);
}

/// A PLC page with cards in the older form: a CPU, a card of inputs and a card of outputs, each with pins on two
/// edges, between a 24 V mark at the head and a 0 V mark at the foot, a sensor over the input card and two lamps under
/// the output card; every wire on the program's route. The wires, the second lamp's wire to the 0 V mark last.
fn older_card_page() -> (DraftPage, Vec<WireId>) {
    let mut b = Page::new(104, 66);
    b.mark_in("24V", p(5, 6));
    b.mark_in("0V", p(5, 62));
    two_rows(&mut b, "CPU", p(11, 30), (12, 8), &[6], &[6]);
    two_rows(&mut b, "DI", p(35, 30), (18, 8), &[3, 6, 9, 12, 15], &[4, 7, 10, 13]);
    two_rows(&mut b, "DO", p(65, 30), (18, 8), &[3, 6, 9, 12, 15], &[3, 6, 9, 12, 15]);
    upright(&mut b, "B1", p(35, 14));
    upright(&mut b, "E1", p(65, 54));
    upright(&mut b, "E2", p(71, 54));
    let wires = vec![
        b.run("w24V", b.pin("24V", "1"), b.pin("CPU", "t0"), &[p(17, 6)]),
        b.run(
            "wL+",
            b.pin("CPU", "t0"),
            b.pin("DO", "b3"),
            &[p(17, 29), p(29, 29), p(29, 40), p(77, 40)],
        ),
        b.run("w0V", b.pin("0V", "1"), b.pin("CPU", "b0"), &[p(17, 62)]),
        b.run("wM1", b.pin("CPU", "b0"), b.pin("DI", "b3"), &[p(17, 41), p(48, 41)]),
        b.run("wM2", b.pin("DI", "b3"), b.pin("DO", "b4"), &[p(48, 41), p(80, 41)]),
        b.run("wB1", b.pin("CPU", "t0"), b.pin("B1", "1"), &[p(17, 13), p(35, 13)]),
        b.run("wI0", b.pin("B1", "2"), b.pin("DI", "t0"), &[p(35, 18), p(38, 18)]),
        b.run("wQ5", b.pin("DO", "b0"), b.pin("E1", "1"), &[p(68, 53), p(65, 53)]),
        b.run("wQ6", b.pin("DO", "b1"), b.pin("E2", "1"), &[]),
        b.run("wE1", b.pin("E1", "2"), b.pin("0V", "1"), &[p(65, 62)]),
        b.run("wE2", b.pin("E2", "2"), b.pin("0V", "1"), &[p(71, 62)]),
    ];
    for w in &mut b.page.wires {
        w.computed = true;
    }
    (b.page, wires)
}

/// `page` with wire `w`'s route lost.
fn lost(page: &DraftPage, w: WireId) -> DraftPage {
    let mut out = page.clone();
    if let Some(x) = out.wires.iter_mut().find(|x| x.id == w) {
        x.route.clear();
        x.computed = false;
    }
    out
}

/// The second lamp's wire to the 0 V mark lost its route: a page-wide Route routes it, and every other wire keeps the
/// route it has. Its net's other wires, laid anew together with it, are weighed one by one against their stored
/// routes, and a stored route that costs no more stays; weighed as one net, the new drawing's fault sent the whole
/// net back to its stored routes, and the wire without one stayed without one, unnamed.
#[test]
fn a_wire_that_lost_its_route_is_routed_and_its_net_keeps_the_routes_it_has() {
    let (page, wires) = older_card_page();
    let gone = *wires.last().expect("the second lamp's wire");
    let page = lost(&page, gone);
    let jobs = jobs(&page, Scope::Page);
    assert!(jobs.wires.contains(&gone));
    let plan = route(&page, &jobs);
    assert!(plan.unrouted.is_empty(), "{:?}", plan.unrouted);
    let changed: Vec<WireId> = plan.routes.iter().map(|(w, _)| *w).collect();
    assert_eq!(changed, [gone]);
    assert!(plan.routes.iter().all(|(_, r)| !r.is_empty()));
}

/// Whichever wire of the page lost its route, a page-wide Route routes it again with no fault: a new wire that the
/// routes kept leave with a fault is searched again among them.
#[test]
fn whichever_wire_lost_its_route_route_routes_it_again_with_no_fault() {
    let (page, wires) = older_card_page();
    for w in wires {
        let page = lost(&page, w);
        let plan = route(&page, &jobs(&page, Scope::Page));
        assert!(
            plan.routes.iter().any(|(x, r)| *x == w && !r.is_empty()),
            "{w:?} stays without a route: {:?}",
            plan.unrouted
        );
        assert_eq!((plan.after.faults, plan.after.unrouted), (0, 0), "{w:?}");
    }
}

