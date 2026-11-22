// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A supply page drawn side by side (docs/design/06a-drafting.md §4.7, the judge): Arrange and route sets the feeder in
//! one column and routes the wire that had no way, though another wire then runs across a mark's text.

use crate::arrange::{at, run};
use crate::kit::{Page, TAG, p};
use ew_drafting::{LabelKind, Owner, Places, Pt, Scope, Side, measure, route};

/// A two-pole symbol `name` at `at` (its left pole): pins `1`, `3` on top and `2`, `4` at the bottom, `pitch` steps
/// apart and four steps high, with its tag and one line of data.
fn two_pole(b: &mut Page, name: &str, at: Pt, pitch: i32) {
    let part = b.part(
        name,
        at,
        (p(-1, 0), p(pitch + 1, 4)),
        &[
            ("1", p(0, 0), Side::Up),
            ("3", p(pitch, 0), Side::Up),
            ("2", p(0, 4), Side::Down),
            ("4", p(pitch, 4), Side::Down),
        ],
    );
    b.tag(part, TAG);
    b.label(Owner::Part(part), 1, LabelKind::Data, TAG, Places::At(Vec::new()));
}

/// The marks L, N and PE stacked over the main switch's first pole, each with the external point it comes from, so
/// that L's wire has no way into the switch; the main switch, a breaker of another pole pitch and a power supply level
/// side by side, each feeding the next from its lower pins into the upper pins of the one beside it. Every wire but
/// L's routed as Route draws it with the parts where they stand.
fn side_by_side() -> Page {
    let mut b = Page::new(100, 60);
    for (name, y) in [("L", 4), ("N", 6), ("PE", 8)] {
        let mark = b.mark_in(name, p(8, y));
        b.label(Owner::Part(mark), 1, LabelKind::MarkXref, (64, 10), Places::At(Vec::new()));
    }
    two_pole(&mut b, "Q1", p(8, 9), 2);
    two_pole(&mut b, "F1", p(20, 9), 3);
    let g1 = b.part(
        "G1",
        p(30, 7),
        (p(0, 0), p(10, 8)),
        &[
            ("L", p(2, 0), Side::Up),
            ("N", p(4, 0), Side::Up),
            ("PE", p(6, 0), Side::Up),
        ],
    );
    b.tag(g1, TAG);
    for (w, a, z) in [
        ("l", ("L", "1"), ("Q1", "1")),
        ("n", ("N", "1"), ("Q1", "3")),
        ("pe", ("PE", "1"), ("G1", "PE")),
        ("q2", ("Q1", "2"), ("F1", "1")),
        ("q4", ("Q1", "4"), ("F1", "3")),
        ("f2", ("F1", "2"), ("G1", "L")),
        ("f4", ("F1", "4"), ("G1", "N")),
    ] {
        b.wire(w, b.pin(a.0, a.1), b.pin(z.0, z.1), &[]);
    }
    let plan = route(&b.page, &ew_drafting::jobs(&b.page, Scope::Page));
    for (w, r) in &plan.routes {
        let wire = b.page.wires.iter_mut().find(|x| x.id == *w).expect("a wire");
        wire.route.clone_from(r);
        wire.computed = !r.is_empty();
    }
    b
}

/// The plan that routes L's wire lays N's across a mark's text: the dashed wire weighs more (§4.7), so it is offered,
/// where before no plan that routed it was.
#[test]
fn a_feeder_drawn_side_by_side_takes_one_column_and_its_dashed_wire_is_routed() {
    let b = side_by_side();
    assert_eq!(measure(&b.page).unrouted, 1, "L's wire has no way as the page stands");
    let (_, done) = run(&b.page, &[]);
    let q = measure(&done);
    assert_eq!(q.unrouted, 0, "{:?}", q.findings.unrouted);
    assert_eq!(q.faults, 0, "{:?}", q.findings);
    assert!(at(&done, "F1").y > at(&done, "Q1").y, "the breaker under the main switch");
    assert!(at(&done, "G1").y > at(&done, "F1").y, "the supply under the breaker");
}
