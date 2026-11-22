// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A wire between two pins that stand nearly in line, one of them off the grid (docs/design/06a-drafting.md §3.6): a
//! page whose grid changed under a symbol whose pins its own spacing keeps off the new grid (doc 06 §8 item 3).

use crate::kit::Page;
use crate::route::routed;
use ew_drafting::{Pt, Side, WireEnd, geom, measure, route_wire};

/// Whether every segment of `route` is horizontal or vertical.
fn square(route: &[Pt]) -> bool {
    geom::segments(&geom::clean(route))
        .iter()
        .all(|(a, b)| a.x == b.x || a.y == b.y)
}

/// A valve's port a third of a step beside the line of the terminal's pin under it: the wire runs down from the port,
/// steps across to the pin's line and runs down into it, square; it was one slanted segment.
#[test]
fn a_wire_between_two_pins_nearly_in_line_steps_across_square() {
    let mut b = Page::new(40, 50);
    b.part(
        "V1",
        Pt::new(181, 427),
        (Pt::new(0, 0), Pt::new(64, 53)),
        &[("2", Pt::new(32, 53), Side::Down)],
    );
    b.part(
        "X1",
        Pt::new(187, 618),
        (Pt::new(13, -42), Pt::new(29, -12)),
        &[("1", Pt::new(21, -42), Side::Up)],
    );
    let w = b.wire("w", b.pin("V1", "2"), b.pin("X1", "1"), &[]);
    let page = routed(&b.page);
    let route = &page.wires.iter().find(|x| x.id == w).expect("the wire").route;
    assert!(!route.is_empty() && square(route), "{route:?}");
    let q = measure(&page);
    assert_eq!((q.faults, q.unrouted), (0, 0), "{:?}", q.findings);
    let pin = |e| match e {
        ew_drafting::End::Pin(p) => WireEnd::Pin(p),
        ew_drafting::End::Junction(j) => WireEnd::Junction(j),
    };
    let tool = route_wire(&b.page, pin(b.pin("V1", "2")), pin(b.pin("X1", "1")), &[]).expect("a way");
    assert!(square(&tool.route), "{:?}", tool.route);
}
