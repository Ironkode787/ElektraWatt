// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Trunks that lie further out than what stands on the page (docs/design/06a-drafting.md §3.3, §3.4): the occupancy
//! holds every node of the drawing area, so a trunk laid six steps beyond the pins it taps is in the way of the trunks
//! and the searches after it.

use crate::kit::{Page, p, s};
use crate::route::routed;
use ew_drafting::{Side, WireId, geom, measure};

/// Three potentials wired from pin to pin over the poles of three three-pole parts in a row, nothing else on the
/// page: three trunks, the first pole's nearest, each two steps beyond the one before (§1.3), with no fault and every
/// wire routed. The second trunk lies four steps beyond the pins, the third six.
#[test]
fn three_potentials_over_one_row_of_poles_lie_on_three_trunks_side_by_side() {
    let mut b = Page::new(50, 40);
    for x in [10, 20, 30] {
        let pins = [
            ("1", p(0, 0), Side::Up),
            ("3", p(2, 0), Side::Up),
            ("5", p(4, 0), Side::Up),
        ];
        b.part(&format!("Q{x}"), p(x, 24), (p(-1, 0), p(5, 6)), &pins);
    }
    let names = ["Q10", "Q20", "Q30"];
    let mut poles: Vec<[WireId; 2]> = Vec::new();
    for pin in ["1", "3", "5"] {
        let a = b.wire(
            &format!("{pin}a"),
            b.pin(names[0], pin),
            b.pin(names[1], pin),
            &[],
        );
        let c = b.wire(
            &format!("{pin}b"),
            b.pin(names[1], pin),
            b.pin(names[2], pin),
            &[],
        );
        poles.push([a, c]);
    }
    let page = routed(&b.page);
    let q = measure(&page);
    assert_eq!((q.faults, q.unrouted), (0, 0), "{:?}", q.findings);
    let line = |w: WireId| {
        let route = &page.wires.iter().find(|x| x.id == w).expect("a wire").route;
        let pts = geom::clean(route);
        assert_eq!(pts.len(), 4, "a drop, the trunk, a drop: {pts:?}");
        assert_eq!(pts[1].y, pts[2].y, "{pts:?}");
        pts[1].y
    };
    let ys: Vec<i32> = poles
        .iter()
        .map(|[a, c]| {
            assert_eq!(line(*a), line(*c), "one trunk per potential");
            line(*a)
        })
        .collect();
    assert_eq!(
        ys,
        vec![s(22), s(20), s(18)],
        "the first pole's trunk nearest, two steps apart"
    );
}
