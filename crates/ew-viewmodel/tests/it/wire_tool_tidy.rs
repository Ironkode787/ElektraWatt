// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Wire tool never proposes a route that runs back over the wire it draws (docs/design/06a-drafting.md
//! §7.4): after a click on empty space (a fixed bend) the route on from it runs along none of the wire so far.

use crate::common::{Tmp, at, done, tables};

use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// A press and release at the page point `p`.
fn click(s: &mut ShellModel, p: (i64, i64)) {
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(s, "canvas.pointer", at(q, json!({})));
    done(s, "canvas.press", at(q, json!({})));
    done(s, "canvas.release", at(q, json!({})));
}

/// The two segments of `route` that share a stretch of one line, if any.
fn overlap(route: &[(i64, i64)]) -> Option<(usize, usize)> {
    let segs: Vec<_> = route.windows(2).map(|w| (w[0], w[1])).collect();
    let span = |a: i64, b: i64| (a.min(b), a.max(b));
    for (i, &(a, b)) in segs.iter().enumerate() {
        for (j, &(c, d)) in segs.iter().enumerate().skip(i + 1) {
            let shared = if a.0 == b.0 && c.0 == d.0 && a.0 == c.0 {
                Some((span(a.1, b.1), span(c.1, d.1)))
            } else if a.1 == b.1 && c.1 == d.1 && a.1 == c.1 {
                Some((span(a.0, b.0), span(c.0, d.0)))
            } else {
                None
            };
            if shared.is_some_and(|(p, q)| p.0.max(q.0) < p.1.min(q.1)) {
                return Some((i, j));
            }
        }
    }
    None
}

/// From -101F1:2 of the sample's 101 (its lower pin, leaving down) a bend three steps right and two down, then the
/// pointer two steps left of the pin on the bend's line: the router's way on from the bend ran down, back left and up
/// onto the bend's line, and along the wire's own leg to the pointer. Now it keeps off the leg.
#[test]
fn after_a_bend_the_wire_tool_s_route_runs_along_none_of_the_wire_so_far() {
    let tmp = Tmp::new("wire-tidy");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    let pin = tables(&s)
        .cps
        .iter()
        .find(|c| c.label == "-101F1:2")
        .expect("-101F1:2 on 101")
        .at;
    let sn = s.canvas().unwrap().page().snapper.clone();
    let point = |dx: i64, dy: i64| {
        (
            sn.page_of_index(sn.grid_index_of(pin.0) + dx),
            sn.page_of_index(sn.grid_index_of(pin.1) + dy),
        )
    };
    done(&mut s, "tool.wire", json!({}));
    click(&mut s, pin);
    click(&mut s, point(3, 2));
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(point(-2, 2));
    done(&mut s, "canvas.pointer", at(q, json!({})));
    let ghost = s.wire_ghost_route().expect("a route");
    assert_eq!(overlap(&ghost), None, "{ghost:?}");
}
