// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Straights before the structure (docs/design/06a-drafting.md §3.1 step 1, §1.3): a wire between two pins that face
//! each other on one line with nothing between is that segment, laid before the tracks and the trunks take a line.

use crate::kit::{Page, p};
use crate::route::routed;
use ew_drafting::{WireId, geom, measure};

/// A wire between two marks that face each other on one line passes between two devices whose wire steps sideways
/// from one to the other, and that wire's first track lies on the marks' line. The straight is laid first: the
/// stepping wire takes its next track and crosses it once, where on the first track it would send the straight round.
#[test]
fn a_straight_keeps_its_line_where_a_stepping_wire_s_first_track_lies_on_it() {
    let mut b = Page::new(44, 24);
    b.mark_in("A", p(2, 10));
    b.mark_out("B", p(40, 10));
    b.device("C", p(10, 4));
    b.device("D", p(20, 14));
    let straight = b.wire("ab", b.pin("A", "1"), b.pin("B", "1"), &[]);
    let step: WireId = b.wire("cd", b.pin("C", "2"), b.pin("D", "1"), &[]);
    let page = routed(&b.page);
    let q = measure(&page);
    assert_eq!((q.faults, q.unrouted), (0, 0), "{:?}", q.findings);
    let route = |w: WireId| geom::clean(&page.wires.iter().find(|x| x.id == w).expect("a wire").route);
    assert_eq!(route(straight), vec![p(2, 10), p(40, 10)], "one straight segment");
    let r = route(step);
    assert_eq!(r.len(), 4, "out, across, in: {r:?}");
    assert!(r[1].y > p(0, 10).y, "the stepping wire's track beyond the straight: {r:?}");
}
