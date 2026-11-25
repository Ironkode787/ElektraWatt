// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A wire into a mark stays on the mark's line (docs/design/06a-drafting.md §1.2, C8): on the sample's PLC
//! page 104, whose marks' wires run straight to their trunks, Arrange and route's plan lays no trunk off a mark's line.

use ew_drafting::{End, Role, Scope, arrange};

#[test]
fn arrange_keeps_a_marks_straight_wire_on_its_line() {
    let s = crate::common::sample();
    let page = s.page_draft("104").expect("the sample's PLC page");
    let marks: Vec<End> = page
        .parts
        .iter()
        .filter(|p| matches!(p.role, Role::MarkIn | Role::MarkOut))
        .flat_map(|p| p.pins.iter().map(|pin| End::Pin(pin.id)))
        .collect();
    let straight =
        |route: &[ew_drafting::Pt]| route.windows(3).all(|w| (w[0].x == w[1].x) == (w[1].x == w[2].x));
    let at_marks: Vec<_> = page
        .wires
        .iter()
        .filter(|w| w.ends.iter().any(|e| marks.contains(e)) && straight(&w.route))
        .collect();
    assert!(!at_marks.is_empty(), "the page's marks are wired straight");
    let plan = arrange(&page, Scope::Page, &[]).plan;
    for w in at_marks {
        if let Some((_, route)) = plan.routes.iter().find(|(id, _)| *id == w.id) {
            assert!(
                straight(route),
                "the mark's wire {:?} stays on its line: {route:?}",
                w.route
            );
        }
    }
}
