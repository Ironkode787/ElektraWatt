// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arrange and route (docs/design/06a-drafting.md §4, §1.6, §9.2, §9.3 rows 4 and 5): parts where the page is best for
//! them within the rules of their kind, on small pages.

use crate::kit::{Page, fixtures, p, s};
use ew_drafting::{
    Arranged, DraftPage, End, PageKind, PartId, Pt, Role, Rule, Scope, Side, arrange, geom, measure,
    rules_kept,
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

/// Arrange and route over the page, checked: `rules_kept` holds and the page is no worse.
fn run(page: &DraftPage, fixed: &[ew_ids::Uuid]) -> (Arranged, DraftPage) {
    let a = arrange(page, Scope::Page, fixed);
    let done = after(page, &a);
    assert_eq!(rules_kept(page, &done), Ok(()), "the plan keeps the rules");
    if !a.is_empty() {
        assert!(
            measure(&done).better_than(&measure(page)),
            "a plan is offered only when better"
        );
    }
    (a, done)
}

fn at(page: &DraftPage, name: &str) -> Pt {
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
    assert!(!a.moves.is_empty());
    assert!(
        arrange(&done, Scope::Page, &[]).moves.is_empty(),
        "a second run moves nothing"
    );
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
    // The sensors under the card with no room over it: the drawing area ends two steps above the card.
    b.page.area = ew_drafting::Rect::new(p(0, 28), p(60, 46));
    for n in ["B0", "B1", "B2"] {
        let x = at(&b.page, n).x;
        put(&mut b, n, Pt::new(x, s(38)));
    }
    let a = arrange(&b.page, Scope::Page, &[]);
    let done = after(&b.page, &a);
    assert!(measure(&done).findings.parts_on_parts.is_empty());
    for n in ["B0", "B1", "B2"] {
        assert!(
            a.left.iter().any(|(x, s)| *x == id(n) && *s == Side::Up),
            "{n} named: {:?}",
            a.left
        );
        assert!(!a.moves.iter().any(|(x, _)| *x == id(n)));
    }
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
