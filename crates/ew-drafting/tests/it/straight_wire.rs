// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Wire tool's straight line (docs/design/06a-drafting.md §7.4): a wire drawn between two ends in line that
//! crosses nothing stays straight, one step over a row of pins included, and its taps drop to those pins; a line that
//! a part or a pin stands on still goes round.

use crate::kit::{Page, p, s};
use ew_drafting::{Pt, WireEnd, geom, measure, route_wire};

/// Four upright devices in a row, their upper pins on line 10, and a mark at each end of line 9: one step over them.
fn row() -> Page {
    let mut b = Page::new(40, 30);
    for (k, x) in [10, 14, 18, 22].into_iter().enumerate() {
        b.device(&format!("K{}", k + 1), p(x, 10));
    }
    b.mark_in("in", p(4, 9));
    b.mark_out("out", p(28, 9));
    b
}

#[test]
fn a_wire_drawn_one_step_over_a_row_of_pins_stays_straight_and_its_taps_drop_onto_it() {
    let mut b = row();
    let rail = route_wire(
        &b.page,
        WireEnd::Pin(b.pin_id("in", "1")),
        WireEnd::Pin(b.pin_id("out", "1")),
        None,
    )
    .expect("a route");
    assert_eq!(rail.route, [p(4, 9), p(28, 9)], "the rail as drawn");
    let id = b.wire("rail", b.pin("in", "1"), b.pin("out", "1"), &rail.route);
    for k in ["K1", "K3"] {
        let pin = b.at(b.pin(k, "1"));
        let on_rail = Pt::new(pin.x, s(9));
        let tap = route_wire(
            &b.page,
            WireEnd::Pin(b.pin_id(k, "1")),
            WireEnd::OnWire(id, on_rail),
            None,
        )
        .expect("a tap");
        assert_eq!(
            tap.route,
            [pin, on_rail],
            "{k}'s tap drops straight onto the rail"
        );
    }
}

#[test]
fn a_straight_line_that_a_part_or_a_pin_stands_on_goes_round_without_a_fault() {
    // A device whose body reaches over line 9, and one whose upper pin stands on it.
    for (name, at) in [("body", p(25, 8)), ("pin", p(25, 9))] {
        let mut b = row();
        b.device(name, at);
        let (from, to) = (b.pin("in", "1"), b.pin("out", "1"));
        let found = route_wire(
            &b.page,
            WireEnd::Pin(b.pin_id("in", "1")),
            WireEnd::Pin(b.pin_id("out", "1")),
            None,
        )
        .expect("a route");
        assert!(
            !geom::corners(&geom::clean(&found.route)).is_empty(),
            "{name}: {:?}",
            found.route
        );
        b.wire("rail", from, to, &found.route);
        let q = measure(&b.page);
        assert_eq!(q.faults, 0, "{name}: {:?}", q.findings);
    }
}
