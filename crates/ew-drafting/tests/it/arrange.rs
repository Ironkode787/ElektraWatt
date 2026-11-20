// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arrange and route (docs/design/06a-drafting.md §4, §1.6, §9.2, §9.3 rows 4 and 5): parts where the page is best for
//! them within the rules of their kind, on small pages.

use crate::kit::{Page, fixtures, p, s};
use ew_drafting::{
    Arranged, DraftPage, End, PageKind, PartId, Pt, Role, Rule, Scope, Side, arrange, geom, measure, rules_kept,
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
        out.junctions.iter_mut().find(|x| x.id == *j).expect("a junction").at = *at;
    }
    for (m, d) in &a.plan.marks {
        let mark = out.marks.iter_mut().find(|x| x.id == *m).expect("a mark");
        mark.line = mark.line.moved(*d);
    }
    out
}

/// Arrange and route over the page, checked: `rules_kept` holds and the page is no worse.
fn run(page: &DraftPage, fixed: &[PartId]) -> (Arranged, DraftPage) {
    let a = arrange(page, Scope::Page, fixed);
    let done = after(page, &a);
    assert_eq!(rules_kept(page, &done), Ok(()), "the plan keeps the rules");
    if !a.is_empty() {
        assert!(measure(&done).better_than(&measure(page)), "a plan is offered only when better");
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

fn set_role(b: &mut Page, name: &str, role: Role) {
    b.page
        .parts
        .iter_mut()
        .find(|x| x.id == PartId(crate::kit::id(name)))
        .expect("a part")
        .role = role;
}

/// The fixture's page as wired by hand, its wires routed anew: what the router gives with every part in place.
fn routed_by_hand(hand: &DraftPage) -> DraftPage {
    let mut bare = hand.clone();
    for w in &mut bare.wires {
        w.route.clear();
    }
    let plan = ew_drafting::route(&bare, &ew_drafting::jobs(&bare, Scope::Page));
    after(&bare, &Arranged { plan, ..Arranged::default() })
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
        assert!(q.counts.turn_backs <= hand.counts.turn_backs, "{}: {:?}", f.name, q.findings.turn_backs);
        assert!(q.findings.across_texts.is_empty(), "{}: {:?}", f.name, q.findings.across_texts);
        assert!(q.findings.texts_on_texts.is_empty(), "{}: {:?}", f.name, q.findings.texts_on_texts);
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
            if facing && side(w.ends[1]).is_some() {
                assert_eq!(geom::corners(&geom::clean(&w.route)).len(), 0, "{}: {:?}", f.name, w.route);
            }
        }
    }
}

#[test]
fn every_fixture_as_wired_by_hand_gives_an_empty_plan() {
    for f in fixtures() {
        let a = arrange(&f.hand, Scope::Page, &[]);
        assert!(a.is_empty(), "{}: {:?} {:?}", f.name, a.moves, a.plan.routes);
    }
}
