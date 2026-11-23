// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arrange and route (docs/design/06a-drafting.md §4, §1.6, §9.2, §9.3 rows 4 and 5): parts where the page is best for
//! them within the rules of their kind, on small pages.

use crate::kit::{Page, fixtures, p, s};
use ew_drafting::{
    Arranged, DraftPage, End, InTheWay, PageKind, PartId, Pt, Role, Rule, Scope, Side, Tick, arrange, geom,
    measure, rules_kept,
};

/// The page as `a` leaves it: every part moved, every route and junction of its plan.
fn after(page: &DraftPage, a: &Arranged) -> DraftPage {
    let mut out = page.clone();
    for (id, d) in &a.moves {
        let part = out.parts.iter_mut().find(|x| x.id == *id).expect("a moved part");
        part.at = part.at.plus(*d);
        for t in &mut out.fixed_texts {
            if t.owner == id.0 {
                t.rect = t.rect.moved(*d);
            }
        }
    }
    for (w, r) in &a.plan.routes {
        let wire = out.wires.iter_mut().find(|x| x.id == *w).expect("a routed wire");
        wire.route.clone_from(r);
        wire.computed = !r.is_empty();
    }
    for (j, at) in &a.plan.junctions {
        out.junctions
            .iter_mut()
            .find(|x| x.id == *j)
            .expect("a junction")
            .at = *at;
    }
    for (m, d) in &a.plan.marks {
        let mark = out.marks.iter_mut().find(|x| x.id == *m).expect("a mark");
        mark.line = mark.line.moved(*d);
    }
    out
}

/// Arrange and route over the page, checked: `rules_kept` holds, the page is no worse, and the quality the plan
/// reports is that of the page it leaves (§4.9).
pub(crate) fn run(page: &DraftPage, fixed: &[ew_ids::Uuid]) -> (Arranged, DraftPage) {
    let a = arrange(page, Scope::Page, fixed);
    let done = after(page, &a);
    assert_eq!(rules_kept(page, &done), Ok(()), "the plan keeps the rules");
    assert_eq!(a.plan.after, measure(&done), "the plan's quality after");
    if !a.is_empty() {
        assert!(
            measure(&done).better_than(&measure(page)),
            "a plan is offered only when better"
        );
    }
    (a, done)
}

pub(crate) fn at(page: &DraftPage, name: &str) -> Pt {
    page.parts
        .iter()
        .find(|x| x.id == PartId(crate::kit::id(name)))
        .expect("a part")
        .at
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

/// The fixture's page as wired by hand, its wires routed anew: what the router gives with every part in place.
fn routed_by_hand(hand: &DraftPage) -> DraftPage {
    let mut bare = hand.clone();
    for w in &mut bare.wires {
        w.route.clear();
    }
    let plan = ew_drafting::route(&bare, &ew_drafting::jobs(&bare, Scope::Page));
    after(
        &bare,
        &Arranged {
            plan,
            ..Arranged::default()
        },
    )
}

#[test]
fn every_fixture_untidy_comes_out_tidy() {
    for f in fixtures() {
        let untidy = f.untidy();
        let (_, done) = run(&untidy, &[]);
        let q = measure(&done);
        assert_eq!(q.faults, 0, "{}: {:?}", f.name, q.findings);
        assert_eq!(q.unrouted, 0, "{}: {:?}", f.name, q.findings.unrouted);
        // No turn-back but those the pins' sides force: as many as the router draws with every part in place (a U
        // between two ports that face one way is one).
        let hand = measure(&routed_by_hand(&f.hand));
        assert!(
            q.counts.turn_backs <= hand.counts.turn_backs,
            "{}: {:?}",
            f.name,
            q.findings.turn_backs
        );
        assert!(
            q.findings.across_texts.is_empty(),
            "{}: {:?}",
            f.name,
            q.findings.across_texts
        );
        assert!(
            q.findings.texts_on_texts.is_empty(),
            "{}: {:?}",
            f.name,
            q.findings.texts_on_texts
        );
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
            if facing && side(w.ends[1]).is_some() {
                assert_eq!(
                    geom::corners(&geom::clean(&w.route)).len(),
                    0,
                    "{}: {:?}",
                    f.name,
                    w.route
                );
            }
        }
    }
}

#[test]
fn every_fixture_once_arranged_gives_an_empty_plan_the_second_time() {
    for f in fixtures() {
        // Arrange and route takes every wire of the page: on a page wired by hand it may route the wires anew, and set a
        // mark on the line its rail then takes, only where that is better (`run` checks it); a second run finds nothing.
        let (_, done) = run(&f.hand, &[]);
        assert!(
            arrange(&done, Scope::Page, &[]).is_empty(),
            "{}: a second run",
            f.name
        );
        // And an untidy page, once arranged, gives an empty plan the second time (D§9.3).
        let (_, tidy) = run(&f.untidy(), &[]);
        let again = arrange(&tidy, Scope::Page, &[]);
        assert!(
            again.is_empty(),
            "{}: {:?} {:?}",
            f.name,
            again.moves,
            again.plan.routes.len()
        );
    }
}

/// A card of four inputs along the foot (pins up, five steps apart from x 12), a sensor over each of the first three.
fn inputs() -> Page {
    let mut b = Page::new(60, 40);
    let card = b.card("A1", p(10, 30), (22, 6), &[2, 7, 12, 17], true);
    b.role(card, Role::Card);
    for (i, x) in [12, 17, 22].into_iter().enumerate() {
        b.device(&format!("B{i}"), p(x, 18));
        b.wire(
            &format!("w{i}"),
            b.pin(&format!("B{i}"), "2"),
            b.pin("A1", &format!("{i}")),
            &[],
        );
    }
    b
}

fn put(b: &mut Page, name: &str, at: Pt) {
    b.page
        .parts
        .iter_mut()
        .find(|x| x.id == PartId(crate::kit::id(name)))
        .expect("a part")
        .at = at;
}

fn id(name: &str) -> PartId {
    PartId(crate::kit::id(name))
}

#[test]
fn a_sensor_below_a_card_whose_pin_faces_up_stands_above_it_after_the_run() {
    let mut b = inputs();
    b.page.area = ew_drafting::Rect::new(Pt::new(0, 0), p(60, 46));
    put(&mut b, "B1", p(17, 38));
    let (a, done) = run(&b.page, &[]);
    assert!(a.moves.iter().any(|(x, _)| *x == id("B1")), "{:?}", a.moves);
    assert!(at(&done, "B1").y < at(&done, "A1").y, "B1 above the card");
    assert!(
        ew_drafting::rules::broken(&done)
            .iter()
            .all(|x| x.rule != Rule::E4)
    );
    assert_eq!(measure(&done).faults, 0);
}

#[test]
fn a_circuit_above_the_rail_that_feeds_it_stands_below_it_after_the_run() {
    let mut b = Page::new(60, 50);
    b.mark_in("in", p(4, 20));
    // The rail at y 20: three junctions; two contacts hang under it, the third circuit (K2 over K3) stands above.
    let mut from = b.pin("in", "1");
    for (k, x) in [12, 22, 32].into_iter().enumerate() {
        let j = b.junction(&format!("J{k}"), p(x, 20));
        b.wire(&format!("r{k}"), from, j, &[]);
        from = j;
        let y = if k == 2 { 4 } else { 24 };
        b.device(&format!("K{k}"), p(x, y));
        b.wire(&format!("d{k}"), j, b.pin(&format!("K{k}"), "1"), &[]);
    }
    b.device("K3", p(32, 11));
    b.wire("k2-k3", b.pin("K2", "2"), b.pin("K3", "1"), &[]);
    let (_, done) = run(&b.page, &[]);
    for k in ["K2", "K3"] {
        assert!(at(&done, k).y > s(20), "{k} below the rail: {:?}", at(&done, k));
    }
    assert!(at(&done, "K2").y < at(&done, "K3").y, "current runs down");
}

#[test]
fn an_actuator_below_its_valve_stands_above_it_and_the_supply_below_it() {
    let mut b = Page::new(60, 50);
    b.page.kind = PageKind::Fluid;
    let pins = [
        ("2", p(2, 0), Side::Up),
        ("4", p(4, 0), Side::Up),
        ("1", p(3, 6), Side::Down),
    ];
    let valve = b.part("V", p(20, 20), (p(0, 0), p(6, 6)), &pins);
    b.role(valve, Role::Valve);
    let ends = [("a", p(2, 4), Side::Down), ("b", p(4, 4), Side::Down)];
    let cyl = b.part("C", p(20, 36), (p(0, 0), p(6, 4)), &ends);
    b.role(cyl, Role::Actuator);
    let unit = b.part("S", p(21, 4), (p(0, 0), p(4, 4)), &[("out", p(2, 0), Side::Up)]);
    b.role(unit, Role::Source);
    b.wire("t2", b.pin("V", "2"), b.pin("C", "a"), &[]);
    b.wire("t4", b.pin("V", "4"), b.pin("C", "b"), &[]);
    b.wire("t1", b.pin("S", "out"), b.pin("V", "1"), &[]);
    let (_, done) = run(&b.page, &[]);
    assert!(
        at(&done, "C").y + s(4) <= at(&done, "V").y,
        "the cylinder above the valve"
    );
    assert!(
        at(&done, "S").y >= at(&done, "V").y + s(6),
        "the supply below the valve"
    );
}

#[test]
fn two_circuits_whose_wires_cross_change_places_and_one_that_does_not_keeps_its_own() {
    let mut b = inputs();
    // B0 and B1 stand over each other's pins: their wires cross.
    put(&mut b, "B0", p(17, 18));
    put(&mut b, "B1", p(12, 18));
    let (_, done) = run(&b.page, &[]);
    assert_eq!((at(&done, "B0").x, at(&done, "B1").x), (s(12), s(17)));
    assert_eq!(at(&done, "B2"), p(22, 18), "B2 keeps its place");
}

#[test]
fn a_part_without_a_wire_a_kept_part_and_a_part_outside_the_selection_do_not_move() {
    let mut b = inputs();
    put(&mut b, "B0", p(13, 18));
    put(&mut b, "B1", p(18, 18));
    put(&mut b, "B2", p(23, 18));
    b.device("free", p(45, 10));
    // D§9.3 row 5: a kept part is in no plan, and the parts are arranged round it (it is a host).
    for n in ["A1", "B2"] {
        b.page
            .parts
            .iter_mut()
            .find(|q| q.id == id(n))
            .expect("a part")
            .locked = true;
    }
    let (a, done) = run(&b.page, &[]);
    let moved = |n: &str| a.moves.iter().any(|(x, _)| *x == id(n));
    assert!(!moved("free") && !moved("A1") && !moved("B2"), "{:?}", a.moves);
    assert!(
        moved("B0") && moved("B1"),
        "the others line up under the kept card"
    );
    assert_eq!(at(&done, "B0").x, s(12));
    // With a selection only the selected parts move.
    let sel = [id("B1").0];
    let a = arrange(&b.page, Scope::Selection(&sel), &[]);
    assert!(a.moves.iter().all(|(x, _)| x.0 == sel[0]), "{:?}", a.moves);
}

#[test]
fn a_second_run_gives_an_empty_plan() {
    let mut b = inputs();
    put(&mut b, "B1", p(18, 18));
    let (a, done) = run(&b.page, &[]);
    assert!(!a.moves.is_empty() && !a.is_empty());
    assert!(
        arrange(&done, Scope::Page, &[]).moves.is_empty(),
        "a second run moves nothing"
    );
}

/// §3.2: a route the person shaped that the plan redraws is flagged his; a wire without a route that it routes is not.
#[test]
fn a_route_the_person_shaped_that_the_plan_redraws_is_flagged() {
    let mut b = inputs();
    // B1 one step right of its pin, its wire drawn by hand with a step to the left.
    put(&mut b, "B1", p(18, 18));
    let w1 = b
        .page
        .wires
        .iter()
        .position(|w| w.id.0 == crate::kit::id("w1"))
        .expect("w1");
    b.page.wires[w1].route = vec![p(18, 22), p(18, 26), p(17, 26), p(17, 30)];
    let (a, _) = run(&b.page, &[]);
    assert!(a.moves.iter().any(|(x, _)| *x == id("B1")), "{:?}", a.moves);
    let routed: Vec<_> = a.plan.routes.iter().map(|(w, _)| *w).collect();
    assert!(routed.len() > 1, "the other wires are routed too: {routed:?}");
    assert_eq!(a.plan.his, [b.page.wires[w1].id]);
}

#[test]
fn unticking_a_part_plans_again_without_it_and_nothing_overlaps() {
    let mut b = inputs();
    for (n, x) in [("B0", 13), ("B1", 18), ("B2", 23)] {
        put(&mut b, n, p(x, 18));
    }
    let (a, done) = run(&b.page, &[id("B1").0]);
    assert!(!a.moves.iter().any(|(x, _)| *x == id("B1")), "{:?}", a.moves);
    assert!(!a.moves.is_empty(), "the others are arranged round it");
    assert!(measure(&done).findings.parts_on_parts.is_empty());
}

#[test]
fn a_row_stands_in_the_order_of_its_host_s_pins_and_a_spacing_kept_is_unchanged() {
    let mut b = inputs();
    // Sensors shifted off their pins, at two heights: each lines up over its pin and keeps its height.
    for (n, x, y) in [("B0", 11, 18), ("B1", 16, 14), ("B2", 23, 18)] {
        put(&mut b, n, p(x, y));
    }
    let (_, done) = run(&b.page, &[]);
    let xs: Vec<_> = ["B0", "B1", "B2"].iter().map(|n| at(&done, n).x).collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "{xs:?}");
    assert_eq!(
        at(&done, "B1").y,
        s(14),
        "its distance from its pin is the person's"
    );
}

#[test]
fn a_part_in_line_is_not_pulled_nearer_for_a_shorter_wire() {
    // The price of a move (§4.7) and A5: a contact in line under another, far down, stays where it is.
    let mut b = Page::new(40, 40);
    b.device("K1", p(10, 6));
    b.device("K2", p(10, 24));
    b.run("w", b.pin("K1", "2"), b.pin("K2", "1"), &[]);
    let a = arrange(&b.page, Scope::Page, &[]);
    assert!(a.moves.is_empty(), "{:?}", a.moves);
}

#[test]
fn a_page_whose_parts_do_not_fit_names_each_part_it_leaves_and_has_no_part_on_a_part() {
    let mut b = inputs();
    // The sensors under the card with no room over it: the drawing area ends two steps above the card, and is too low
    // for the card to step down and take the sensors over it.
    b.page.area = ew_drafting::Rect::new(p(0, 28), p(60, 44));
    for n in ["B0", "B1", "B2"] {
        let x = at(&b.page, n).x;
        put(&mut b, n, Pt::new(x, s(38)));
    }
    // Beside them a contact one step off the line of the one above it: it moves, and the sensors are named all the
    // same.
    b.device("K1", p(45, 30));
    b.device("K2", p(46, 36));
    b.wire("k", b.pin("K1", "2"), b.pin("K2", "1"), &[]);
    let a = arrange(&b.page, Scope::Page, &[]);
    assert!(!a.moves.is_empty());
    let done = after(&b.page, &a);
    assert!(measure(&done).findings.parts_on_parts.is_empty());
    // Each with what was in the way of its place over the card: the edge of the drawing area.
    for n in ["B0", "B1", "B2"] {
        assert!(
            a.left.contains(&(id(n), Side::Up, InTheWay::Edge)),
            "{n} named: {:?}",
            a.left
        );
        assert!(!a.moves.iter().any(|(x, _)| *x == id(n)));
    }
}

#[test]
fn a_part_left_against_a_rule_names_the_part_that_stands_in_its_place() {
    // E1 and §4.7: a box fed from the lower pin of a locked fuse stands beside the fuse, above that pin. Under the fuse
    // a locked part fills the page's width, with too little room above it for the box: the box stays, and the plan
    // names that part as what was in its place below the pin.
    let mut b = Page::new(40, 16);
    b.device("F1", p(10, 2));
    b.part("G1", p(16, 2), (p(-1, 0), p(3, 4)), &[("1", p(0, 0), Side::Up)]);
    b.wire("w", b.pin("F1", "2"), b.pin("G1", "1"), &[]);
    let wall = b.part("U9", p(1, 9), (p(0, 0), p(38, 5)), &[]);
    for part in b
        .page
        .parts
        .iter_mut()
        .filter(|x| x.id == id("F1") || x.id == wall)
    {
        part.locked = true;
    }
    let a = arrange(&b.page, Scope::Page, &[]);
    assert!(
        a.left.contains(&(id("G1"), Side::Down, InTheWay::Part(wall))),
        "{:?}",
        a.left
    );
}

#[test]
fn each_rule_of_a_page_names_the_part_that_breaks_it() {
    use ew_drafting::rules::broken;
    let names = |page: &DraftPage, rule: Rule| -> Vec<PartId> {
        broken(page)
            .into_iter()
            .filter(|b| b.rule == rule)
            .flat_map(|b| b.parts)
            .collect()
    };
    // E1: a contact under the one it feeds through a pin facing down.
    let mut b = Page::new(40, 40);
    b.device("K1", p(10, 20));
    b.device("K2", p(10, 6));
    b.wire("w", b.pin("K1", "2"), b.pin("K2", "1"), &[]);
    assert!(names(&b.page, Rule::E1).contains(&id("K2")));
    // E3: an incoming mark at the right end of its wire.
    let mut b = Page::new(40, 40);
    b.mark_in("in", p(30, 10));
    b.device("K", p(10, 12));
    b.wire("w", b.pin("in", "1"), b.pin("K", "1"), &[]);
    assert_eq!(names(&b.page, Rule::E3), vec![id("in")]);
    // E4: a field device under a card's pin that faces up; E5: an earth symbol far from its pin.
    let mut b = inputs();
    put(&mut b, "B2", p(22, 38));
    let earth = b.part("PE", p(50, 5), (p(-1, 0), p(1, 2)), &[("1", p(0, 0), Side::Up)]);
    b.role(earth, Role::Earth);
    b.wire("pe", b.pin("PE", "1"), b.pin("B0", "1"), &[]);
    assert!(names(&b.page, Rule::E4).contains(&id("B2")));
    assert_eq!(names(&b.page, Rule::E5), vec![id("PE")]);
    // A1: a part on a part.
    let mut b = inputs();
    put(&mut b, "B1", p(12, 19));
    assert!(names(&b.page, Rule::A1).contains(&id("B1")));
    // F1 on a fluid page, B2 on a bus page.
    let mut b = Page::new(40, 40);
    b.page.kind = PageKind::Fluid;
    let v = b.part("V", p(10, 10), (p(0, 0), p(4, 4)), &[("1", p(2, 4), Side::Down)]);
    b.role(v, Role::Valve);
    let src = b.part("S", p(10, 2), (p(0, 0), p(4, 4)), &[("o", p(2, 4), Side::Down)]);
    b.role(src, Role::Source);
    assert_eq!(names(&b.page, Rule::F1), vec![id("S")]);
    let mut b = Page::new(60, 40);
    b.page.kind = PageKind::Bus;
    for (n, x, y) in [("D0", 5, 10), ("D1", 25, 14)] {
        let c = b.card(n, p(x, y), (8, 6), &[2, 6], true);
        b.role(c, Role::Station);
    }
    b.wire("bus", b.pin("D0", "1"), b.pin("D1", "0"), &[]);
    let mut stations = names(&b.page, Rule::B2);
    stations.sort();
    let mut want = vec![id("D0"), id("D1")];
    want.sort();
    assert_eq!(stations, want);
}

#[test]
fn rules_kept_names_a_moved_kept_part_a_reshaped_part_and_a_closed_up_chain() {
    let mut b = Page::new(40, 40);
    b.device("K1", p(10, 6));
    b.device("K2", p(10, 16));
    b.run("w", b.pin("K1", "2"), b.pin("K2", "1"), &[]);
    b.page
        .parts
        .iter_mut()
        .find(|q| q.id == id("K1"))
        .expect("K1")
        .locked = true;
    let mut moved = b.page.clone();
    moved.parts.iter_mut().find(|q| q.id == id("K1")).expect("K1").at = p(10, 4);
    let broken = rules_kept(&b.page, &moved).unwrap_err();
    assert!(
        broken
            .iter()
            .any(|x| x.rule == Rule::A2 && x.parts == vec![id("K1")]),
        "{broken:?}"
    );
    let mut closed = b.page.clone();
    closed.parts.iter_mut().find(|q| q.id == id("K2")).expect("K2").at = p(10, 12);
    assert!(
        rules_kept(&b.page, &closed)
            .unwrap_err()
            .iter()
            .any(|x| x.rule == Rule::A5)
    );
    let mut reshaped = b.page.clone();
    reshaped
        .parts
        .iter_mut()
        .find(|q| q.id == id("K2"))
        .expect("K2")
        .body
        .y1 += 1;
    assert!(
        rules_kept(&b.page, &reshaped)
            .unwrap_err()
            .iter()
            .any(|x| x.rule == Rule::A3)
    );
    assert_eq!(rules_kept(&b.page, &b.page), Ok(()));
}

/// The corners of wire `name` of `page`.
pub(crate) fn corners(page: &DraftPage, name: &str) -> usize {
    let w = page
        .wires
        .iter()
        .find(|w| w.id.0 == crate::kit::id(name))
        .expect("a wire");
    geom::corners(&geom::clean(&w.route)).len()
}

/// What counts first on a page after a run: its faults, its wires without a way, its crossings.
pub(crate) fn sound(page: &DraftPage) -> (usize, usize, usize) {
    let q = measure(page);
    (q.faults, q.unrouted, q.counts.crossings)
}

/// A distribution page (§1.4): the marks of a strip, scattered over the sheet, stand in stacks beside the pins they are
/// wired to, the incoming ones above it and the outgoing ones below it, in the order that nests their wires: every
/// wire has its one corner and none crosses another.
#[test]
fn the_marks_of_a_strip_stand_in_stacks_and_their_wires_nest() {
    let mut b = Page::new(80, 60);
    let pins = [
        ("u0", p(2, 0), Side::Up),
        ("u1", p(4, 0), Side::Up),
        ("u2", p(6, 0), Side::Up),
        ("u3", p(8, 0), Side::Up),
        ("d0", p(2, 3), Side::Down),
        ("d1", p(4, 3), Side::Down),
        ("d2", p(6, 3), Side::Down),
        ("d3", p(8, 3), Side::Down),
    ];
    let strip = b.part("X", p(34, 28), (p(0, 0), p(10, 3)), &pins);
    b.tag(strip, crate::kit::TAG);
    for (k, at) in [p(6, 40), p(12, 8), p(20, 50), p(9, 30)].into_iter().enumerate() {
        b.mark_in(&format!("in{k}"), at);
        b.wire(
            &format!("i{k}"),
            b.pin(&format!("in{k}"), "1"),
            b.pin("X", &format!("u{k}")),
            &[],
        );
    }
    for (k, at) in [p(60, 6), p(70, 44), p(56, 14), p(66, 20)]
        .into_iter()
        .enumerate()
    {
        b.mark_out(&format!("out{k}"), at);
        b.wire(
            &format!("o{k}"),
            b.pin("X", &format!("d{k}")),
            b.pin(&format!("out{k}"), "1"),
            &[],
        );
    }
    let (_, done) = run(&b.page, &[]);
    assert_eq!(sound(&done), (0, 0, 0), "{:?}", measure(&done).findings);
    for k in 0..4 {
        assert_eq!(corners(&done, &format!("i{k}")), 1, "i{k}");
        assert_eq!(corners(&done, &format!("o{k}")), 1, "o{k}");
    }
    // The mark of the pin furthest from its stack stands outermost, on both sides.
    let ys = |n: &str| -> Vec<_> { (0..4).map(|k| at(&done, &format!("{n}{k}")).y).collect() };
    let (ins, outs) = (ys("in"), ys("out"));
    assert!(
        ins.iter().all(|y| *y < s(28)) && ins.windows(2).all(|w| w[0] > w[1]),
        "{ins:?}"
    );
    assert!(
        outs.iter().all(|y| *y > s(31)) && outs.windows(2).all(|w| w[0] > w[1]),
        "{outs:?}"
    );
}

/// A rail page (§1.4): a potential wired from pin to pin, without a junction, is drawn as one rail above the parts it
/// feeds, and the part that stood two steps lower than the others stands level with them.
#[test]
fn the_parts_a_potential_feeds_from_pin_to_pin_stand_level_under_one_rail() {
    let mut b = Page::new(70, 40);
    b.mark_in("L", p(6, 6));
    for (k, (x, y)) in [(16, 12), (28, 14), (40, 12), (52, 12)].into_iter().enumerate() {
        b.device(&format!("K{k}"), p(x, y));
    }
    b.wire("r0", b.pin("L", "1"), b.pin("K0", "1"), &[]);
    for k in 0..3 {
        b.wire(
            &format!("r{}", k + 1),
            b.pin(&format!("K{k}"), "1"),
            b.pin(&format!("K{}", k + 1), "1"),
            &[],
        );
    }
    let (_, done) = run(&b.page, &[]);
    assert_eq!(sound(&done), (0, 0, 0), "{:?}", measure(&done).findings);
    let ys: Vec<_> = (0..4).map(|k| at(&done, &format!("K{k}")).y).collect();
    assert!(ys.iter().all(|y| *y == s(12)), "{ys:?}");
    // One line above the parts carries every stretch from part to part.
    let mut lines: Vec<_> = done
        .wires
        .iter()
        .flat_map(|w| geom::segments(&w.route))
        .filter(|(a, b)| a.y == b.y && a.x != b.x && a.x.min(b.x) >= s(16))
        .map(|(a, _)| a.y)
        .collect();
    lines.sort_unstable();
    lines.dedup();
    assert!(matches!(lines.as_slice(), [y] if *y < s(12)), "{lines:?}");
}

/// A PLC page with field devices through terminals (§1.4): each terminal stands in line over its pin, the sensors,
/// wider than the pins' pitch, stand in a row over the terminals in the order of the pins, and the supply's riser
/// passes beside that row: no wire crosses another.
#[test]
fn field_devices_stand_in_a_row_over_their_terminals_and_the_terminals_over_their_pins() {
    let mut b = Page::new(90, 50);
    let card = b.card("A1", p(30, 38), (14, 6), &[2, 4, 6], true);
    b.role(card, Role::Card);
    // The supply's terminal and one for each channel, six steps apart; a sensor over each, nine steps apart.
    b.terminal("X0", p(12, 27));
    b.mark_in("L+", p(4, 32));
    b.wire("feed", b.pin("L+", "1"), b.pin("X0", "2"), &[]);
    for k in 0..3 {
        let (x, sensor) = (18 + 6 * k, format!("B{k}"));
        b.terminal(&format!("X{}", k + 1), p(x, 27));
        b.device(&sensor, p(14 + 9 * k, 12));
        b.wire(
            &format!("s{k}"),
            b.pin(&sensor, "2"),
            b.pin(&format!("X{}", k + 1), "1"),
            &[],
        );
        b.wire(
            &format!("c{k}"),
            b.pin(&format!("X{}", k + 1), "2"),
            b.pin("A1", &format!("{k}")),
            &[],
        );
    }
    b.wire("p0", b.pin("X0", "1"), b.pin("B0", "1"), &[]);
    b.wire("p1", b.pin("B0", "1"), b.pin("B1", "1"), &[]);
    b.wire("p2", b.pin("B1", "1"), b.pin("B2", "1"), &[]);
    let (_, done) = run(&b.page, &[]);
    assert_eq!(sound(&done), (0, 0, 0), "{:?}", measure(&done).findings);
    for k in 0..3 {
        assert_eq!(
            at(&done, &format!("X{}", k + 1)).x,
            s(32 + 2 * k),
            "terminal {} over its pin",
            k + 1
        );
        assert_eq!(corners(&done, &format!("c{k}")), 0, "c{k}");
    }
    let sensors: Vec<_> = (0..3).map(|k| at(&done, &format!("B{k}"))).collect();
    assert!(
        sensors.windows(2).all(|w| w[0].x < w[1].x && w[0].y == w[1].y),
        "{sensors:?}"
    );
    assert!(
        at(&done, "X0").x < at(&done, "X1").x,
        "the supply's terminal stays at the strip's left end"
    );
}

/// A fluid page with a valve terminal (§1.4, rules F1 and F2): the cylinders, each fed through two flow controls,
/// stand above the terminal in the order of their valves and the service unit below it; no tube crosses another.
#[test]
fn on_a_valve_terminal_the_cylinders_stand_above_in_the_valves_order_and_the_tubes_nest() {
    let mut b = Page::new(100, 64);
    b.page.kind = PageKind::Fluid;
    let frame = b.part("VT", p(30, 30), (p(0, 0), p(26, 8)), &[]);
    let unit = b.page.parts.iter().find(|x| x.id == frame).map(|x| x.unit);
    if let Some(part) = b.page.parts.iter_mut().find(|x| x.id == frame) {
        part.frame_only = true;
    }
    for (i, x) in [34, 46].into_iter().enumerate() {
        let valve = b.part(
            &format!("V{i}"),
            p(x, 30),
            (p(0, 0), p(8, 4)),
            &[("4", p(3, 0), Side::Up), ("2", p(5, 0), Side::Up)],
        );
        b.role(valve, Role::Valve);
        if let (Some(u), Some(part)) = (unit, b.page.parts.iter_mut().find(|x| x.id == valve)) {
            part.unit = u;
        }
        // The cylinder and its two flow controls stand below the terminal: on the wrong side of the working ports.
        let cylinder = b.part(
            &format!("C{i}"),
            p(x + 8, 46),
            (p(0, 0), p(2, 6)),
            &[("a", p(0, 1), Side::Left), ("b", p(0, 4), Side::Left)],
        );
        b.role(cylinder, Role::Actuator);
        for (r, port, end, y) in [(0, "4", "a", 47), (1, "2", "b", 50)] {
            let throttle = format!("R{i}{r}");
            b.part(
                &throttle,
                p(x, y),
                (p(0, -1), p(4, 1)),
                &[("1", p(0, 0), Side::Left), ("2", p(4, 0), Side::Right)],
            );
            b.wire(
                &format!("t{i}{r}"),
                b.pin(&format!("V{i}"), port),
                b.pin(&throttle, "1"),
                &[],
            );
            b.wire(
                &format!("u{i}{r}"),
                b.pin(&throttle, "2"),
                b.pin(&format!("C{i}"), end),
                &[],
            );
        }
    }
    let supply = b.part(
        "VTP",
        p(30, 34),
        (p(0, 0), p(4, 4)),
        &[("1", p(2, 4), Side::Down)],
    );
    b.role(supply, Role::Valve);
    if let (Some(u), Some(part)) = (unit, b.page.parts.iter_mut().find(|x| x.id == supply)) {
        part.unit = u;
    }
    let source = b.part(
        "FRL",
        p(10, 8),
        (p(0, 0), p(4, 4)),
        &[("out", p(4, 2), Side::Right)],
    );
    b.role(source, Role::Source);
    b.wire("p", b.pin("FRL", "out"), b.pin("VTP", "1"), &[]);
    for w in &mut b.page.wires {
        w.class = ew_drafting::WireClass::Tube;
    }
    let (_, done) = run(&b.page, &[]);
    assert_eq!(sound(&done), (0, 0, 0), "{:?}", measure(&done).findings);
    for i in 0..2 {
        assert!(
            at(&done, &format!("C{i}")).y + s(6) <= s(30),
            "cylinder {i} above the terminal: {:?}",
            at(&done, &format!("C{i}"))
        );
    }
    assert!(
        at(&done, "C0").x < at(&done, "C1").x,
        "in the order of the valves"
    );
    assert!(at(&done, "FRL").y >= s(38), "the service unit below the terminal");
    assert_eq!(at(&done, "VT"), p(30, 30), "the terminal stays");
}

/// A part wired by a hairpin (its pin and the card's face the same way) that stands far off across the sheet takes
/// its place at the card: just beyond the card's edge and off the pin's line, so that the wire passes it (§4.3).
#[test]
fn a_part_wired_by_a_hairpin_stands_at_its_host_off_the_pin_s_line() {
    let mut b = Page::new(80, 50);
    let card = b.card("A1", p(30, 36), (14, 6), &[2, 4, 6], true);
    b.role(card, Role::Card);
    b.device("H", p(60, 6));
    b.wire("h", b.pin("A1", "0"), b.pin("H", "1"), &[]);
    let (a, done) = run(&b.page, &[]);
    assert!(a.moves.iter().any(|(x, _)| *x == id("H")), "{:?}", a.moves);
    let lamp = at(&done, "H");
    assert!(
        lamp.y + s(4) < s(36) && lamp.y + s(4) >= s(36 - 12),
        "just above the card: {lamp:?}"
    );
    assert!(
        lamp.x < s(32),
        "left of its pin's line, where the card has no other pin: {lamp:?}"
    );
    assert_eq!(corners(&done, "h"), 2, "out, across and back");
    assert_eq!(sound(&done), (0, 0, 0));
}

/// A4 reads the terminals of a strip as one row (§1.6): two terminals side by side whose devices stand the other way
/// round break it, in the pins' order they keep it. A part wired out and back to a card's pin, its own pin facing the
/// same way, is no part of the channel's column: E4 does not name it wherever it stands.
#[test]
fn a_strip_is_one_row_and_a_hairpin_is_not_bound_to_its_channel_s_column() {
    use ew_drafting::rules::broken;
    let mut b = Page::new(60, 40);
    b.terminal("X1", p(20, 20));
    b.terminal("X2", p(22, 20));
    b.device("B1", p(14, 28));
    b.device("B2", p(30, 28));
    b.wire("a", b.pin("X1", "2"), b.pin("B1", "1"), &[]);
    b.wire("c", b.pin("X2", "2"), b.pin("B2", "1"), &[]);
    let with = |xs: [Tick; 2]| {
        let mut moved = b.page.clone();
        for (n, x) in [("B1", xs[0]), ("B2", xs[1])] {
            moved
                .parts
                .iter_mut()
                .find(|q| q.id == id(n))
                .expect("a device")
                .at = p(x, 28);
        }
        rules_kept(&b.page, &moved)
            .err()
            .is_some_and(|e| e.iter().any(|x| x.rule == Rule::A4))
    };
    assert_eq!((with([12, 32]), with([30, 14])), (false, true));
    let mut b = inputs();
    b.device("H", p(50, 40));
    b.wire("h", b.pin("A1", "3"), b.pin("H", "1"), &[]);
    assert!(
        !broken(&b.page)
            .iter()
            .any(|x| x.rule == Rule::E4 && x.parts.contains(&id("H")))
    );
}

/// A5 as it binds a plan (§1.6): a chain may take the chain gap and may open by a row's pull; nearer or further it
/// is closed up or spread.
#[test]
fn a_chain_may_take_the_chain_gap_and_open_by_a_row_s_pull_and_no_more() {
    let mut b = Page::new(40, 50);
    b.device("K1", p(10, 6));
    b.device("K2", p(10, 18));
    b.run("w", b.pin("K1", "2"), b.pin("K2", "1"), &[]);
    // The pins stand 8 steps apart.
    let a5 = |y: Tick| {
        let mut moved = b.page.clone();
        moved.parts.iter_mut().find(|q| q.id == id("K2")).expect("K2").at = p(10, y);
        rules_kept(&b.page, &moved)
            .err()
            .is_some_and(|e| e.iter().any(|x| x.rule == Rule::A5))
    };
    assert_eq!(
        (a5(15), a5(14), a5(24), a5(25)),
        (false, true, false, true),
        "5 and 14 steps apart are kept, 4 and 15 are not"
    );
}

/// The same page gives the same plan whatever the order of its lists (§4.9): every fixture, untidy, with its parts,
/// pins, wires, junctions, labels and fixed texts in the reverse order.
#[test]
fn the_same_page_in_any_order_of_its_lists_gives_the_same_arrangement() {
    for f in fixtures() {
        let page = f.untidy();
        let plan = arrange(&page, Scope::Page, &[]);
        let mut shuffled = page.clone();
        shuffled.parts.reverse();
        shuffled.wires.reverse();
        shuffled.junctions.reverse();
        shuffled.labels.reverse();
        shuffled.fixed_texts.reverse();
        for part in &mut shuffled.parts {
            part.pins.reverse();
        }
        let again = arrange(&shuffled, Scope::Page, &[]);
        assert_eq!(plan.moves, again.moves, "{}", f.name);
        assert_eq!(plan.plan.routes, again.plan.routes, "{}", f.name);
        assert_eq!(plan.plan.junctions, again.plan.junctions, "{}", f.name);
    }
}

/// A strip the person set down in one row between a card of outputs at the head and a card of inputs at the foot, its
/// devices on the wrong side of it (rule E4, §4.4): each terminal stands in the column of the channel it is wired to,
/// whichever card that is, with its device beyond it. A terminal of the strip without a wire between them does not
/// join the two cards into one tree: both cards keep their places.
#[test]
fn a_strip_between_two_cards_stands_by_the_channels_of_both_and_the_cards_stay() {
    let mut b = Page::new(100, 62);
    let out = b.card("A2", p(40, 6), (26, 6), &[5, 13, 21], false);
    b.role(out, Role::Card);
    let inputs = b.card("A1", p(10, 50), (26, 6), &[5, 13, 21], true);
    b.role(inputs, Role::Card);
    for k in 0..5 {
        b.terminal(&format!("X{k}"), p(60 + 2 * k, 30));
    }
    // Three inputs: a sensor under each terminal where it belongs over it; then a spare terminal; then an output
    // with its lamp.
    for k in 0..3 {
        let sensor = format!("B{k}");
        b.device(&sensor, p(60 + 9 * k, 36));
        b.wire(
            &format!("s{k}"),
            b.pin(&sensor, "2"),
            b.pin(&format!("X{k}"), "1"),
            &[],
        );
        b.wire(
            &format!("c{k}"),
            b.pin(&format!("X{k}"), "2"),
            b.pin("A1", &format!("{k}")),
            &[],
        );
    }
    b.device("E", p(72, 42));
    b.wire("q", b.pin("A2", "0"), b.pin("X4", "1"), &[]);
    b.wire("e", b.pin("X4", "2"), b.pin("E", "1"), &[]);
    let (a, done) = run(&b.page, &[]);
    assert_eq!(sound(&done), (0, 0, 0), "{:?}", measure(&done).findings);
    assert!(
        !a.moves.iter().any(|(x, _)| *x == id("A1") || *x == id("A2")),
        "{:?}",
        a.moves
    );
    for k in 0..3 {
        let (x, t, d) = (
            s(15 + 8 * k),
            at(&done, &format!("X{k}")),
            at(&done, &format!("B{k}")),
        );
        assert_eq!((t.x, d.x), (x, x), "channel {k} in its column");
        assert!(d.y < t.y, "sensor {k} above its terminal");
        assert_eq!(
            corners(&done, &format!("c{k}")) + corners(&done, &format!("s{k}")),
            0
        );
    }
    assert_eq!(
        (at(&done, "X4").x, at(&done, "E").x),
        (s(45), s(45)),
        "the output's column"
    );
    assert_eq!(corners(&done, "q") + corners(&done, "e"), 0);
}

/// A row that has no room at the chain gap stands nearer (§4.4): the sensors of a card's channels belong above their
/// terminals, where a box stands at the head. They take, as one row, the furthest line under the box that keeps them
/// clear of it, each in its channel's column.
#[test]
fn a_row_under_a_box_stands_nearer_than_the_chain_gap_and_clear_of_it() {
    let mut b = Page::new(100, 62);
    b.part("K", p(8, 4), (p(0, 0), p(30, 14)), &[("1", p(4, 0), Side::Up)]);
    let card = b.card("A1", p(10, 50), (26, 6), &[5, 13, 21], true);
    b.role(card, Role::Card);
    for k in 0..3 {
        let (terminal, sensor) = (format!("X{k}"), format!("B{k}"));
        b.terminal(&terminal, p(15 + 8 * k, 27));
        b.device(&sensor, p(50 + 9 * k, 34));
        b.wire(&format!("s{k}"), b.pin(&sensor, "2"), b.pin(&terminal, "1"), &[]);
        b.wire(
            &format!("c{k}"),
            b.pin(&terminal, "2"),
            b.pin("A1", &format!("{k}")),
            &[],
        );
    }
    let (_, done) = run(&b.page, &[]);
    assert_eq!(sound(&done), (0, 0, 0), "{:?}", measure(&done).findings);
    let foot = at(&done, "K").y + s(14);
    for k in 0..3 {
        let d = at(&done, &format!("B{k}"));
        assert_eq!(d.x, s(15 + 8 * k), "sensor {k} in its column");
        assert!(d.y >= foot + s(1), "clear of the box: {d:?}");
        assert_eq!(d.y, at(&done, "B0").y, "one row");
        assert!(
            at(&done, &format!("X{k}")).y - (d.y + s(4)) >= s(2),
            "no nearer than the least gap"
        );
    }
}

/// A card of inputs set at the foot under the controller's box, its supply pin on its left side wired to the
/// controller's and to that of a card of outputs at the head (§4.4): where it stands its supply has to cross its
/// first channels. As the root of its tree it steps along the foot until the pins its supply comes from stand left of
/// it, its strip and its devices with it, and no supply wire crosses a channel.
#[test]
fn a_card_under_the_controller_steps_aside_until_its_supply_crosses_no_channel() {
    let mut b = Page::new(100, 62);
    b.part("K", p(4, 6), (p(0, 0), p(14, 12)), &[("M", p(7, 12), Side::Down)]);
    let row = |side: Side, y: Tick| {
        [
            ("0", p(5, y), side),
            ("1", p(13, y), side),
            ("2", p(21, y), side),
            ("M", p(0, 4), Side::Left),
        ]
    };
    let out = b.part("A2", p(22, 6), (p(0, 0), p(26, 6)), &row(Side::Down, 6));
    b.role(out, Role::Card);
    let inputs = b.part("A1", p(4, 50), (p(0, 0), p(26, 6)), &row(Side::Up, 0));
    b.role(inputs, Role::Card);
    b.wire("m1", b.pin("A1", "M"), b.pin("K", "M"), &[]);
    b.wire("m2", b.pin("A2", "M"), b.pin("A1", "M"), &[]);
    for k in 0..3 {
        let (terminal, sensor) = (format!("X{k}"), format!("B{k}"));
        b.terminal(&terminal, p(9 + 8 * k, 34));
        b.device(&sensor, p(9 + 8 * k, 19));
        b.wire(&format!("s{k}"), b.pin(&sensor, "2"), b.pin(&terminal, "1"), &[]);
        b.wire(
            &format!("c{k}"),
            b.pin(&terminal, "2"),
            b.pin("A1", &format!("{k}")),
            &[],
        );
    }
    let (_, done) = run(&b.page, &[]);
    assert_eq!(sound(&done), (0, 0, 0), "{:?}", measure(&done).findings);
    let card = at(&done, "A1");
    assert!(
        card.x >= s(22),
        "the card stands right of the pins its supply comes from: {card:?}"
    );
    assert_eq!(card.y, s(50), "along the foot");
    for k in 0..3 {
        let x = card.x + s(5 + 8 * k);
        assert_eq!(
            (at(&done, &format!("X{k}")).x, at(&done, &format!("B{k}")).x),
            (x, x),
            "channel {k} came with it"
        );
    }
}
