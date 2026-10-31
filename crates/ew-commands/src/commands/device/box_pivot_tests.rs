// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pivot of a black box's turns (cycle 41 review, finding 1): every box of 1 to 8 by 1 to 8 grid steps, at
//! positive and negative grid positions and away from the origin, from a plain and from a mirrored orientation, on
//! the rational QET grid and on a metric one.

use ew_schema::{BoxCp, BoxSpec, Dir4, Extra};
use ew_units::{Grid, Nm, Orient2, Pt2};

use super::box_pivot;

const GRIDS: [Grid; 2] = [Grid::QET_10PX, Grid::MM4];
const ORIGINS: [(i64, i64); 4] = [(31, 46), (-37, -23), (0, 0), (400, -401)];
const STARTS: [Orient2; 4] = [Orient2::R0, Orient2::R90, Orient2::M0, Orient2::M270];

type Turn = fn(Orient2) -> Orient2;

fn cw(o: Orient2) -> Orient2 {
    o.then(Orient2::R90)
}

fn ccw(o: Orient2) -> Orient2 {
    o.then(Orient2::R270)
}

const TURNS: [Turn; 2] = [cw, ccw];

fn at(g: Grid, k: i64) -> i64 {
    g.at(k).expect("in range").0
}

/// The black-box tool's box of `w` by `h` steps with its top-left corner at grid index `k0`: the outline relative to
/// the insertion point and a pin at every step of the top and the bottom edge, each stored as `grid.at(k) − pos`
/// (`ew_viewmodel::tools::black_box::layout`).
fn tool_box(g: Grid, k0: (i64, i64), w: i64, h: i64) -> (Pt2, BoxSpec) {
    let p = |x: i64, y: i64| Pt2::new(Nm(x), Nm(y));
    let pos = p(at(g, k0.0), at(g, k0.1));
    let dx = |k: i64| at(g, k0.0 + k) - pos.x.0;
    let dy = |k: i64| at(g, k0.1 + k) - pos.y.0;
    let outline = vec![p(0, 0), p(dx(w), 0), p(dx(w), dy(h)), p(0, dy(h))];
    let mut cps = Vec::new();
    for i in 0..=w {
        for (y, dir) in [(0, Dir4::Up), (dy(h), Dir4::Down)] {
            cps.push(BoxCp {
                idx: u16::try_from(cps.len()).expect("few CPs"),
                pos: p(dx(i), y),
                dir,
                extra: Extra::new(),
            });
        }
    }
    let spec = BoxSpec {
        outline,
        cps,
        inner: vec![],
        extra: Extra::new(),
    };
    (pos, spec)
}

/// The insertion point and orientation after turning `(pos, o)` to `to`; asserts the point is a grid point.
fn turn(g: Grid, b: &BoxSpec, (pos, o): (Pt2, Orient2), to: Orient2) -> (Pt2, Orient2) {
    let next = box_pivot(g, pos, o, to, b).expect("in range").expect("a box");
    for v in [next.x, next.y] {
        assert_eq!(g.snap_point(v), Some(v), "the insertion point is a grid point");
    }
    (next, to)
}

/// The grid indices of the top-left corner of the box's bounding box.
fn top_left(g: Grid, b: &BoxSpec, (pos, o): (Pt2, Orient2)) -> (i64, i64) {
    let pts: Vec<Pt2> = b.outline.iter().map(|v| pos + o.apply(*v)).collect();
    let x = pts.iter().map(|p| g.snap(p.x)).min().expect("an outline");
    let y = pts.iter().map(|p| g.snap(p.y)).min().expect("an outline");
    (x, y)
}

/// Runs `f` on every case: grid, box size, position and starting orientation.
fn each(mut f: impl FnMut(&str, Grid, &BoxSpec, (Pt2, Orient2))) {
    for g in GRIDS {
        for w in 1..=8 {
            for h in 1..=8 {
                for k0 in ORIGINS {
                    for o in STARTS {
                        let (pos, b) = tool_box(g, k0, w, h);
                        let case = format!("grid {}/{}, {w} by {h} at {k0:?} from {o:?}", g.num(), g.den());
                        f(&case, g, &b, (pos, o));
                    }
                }
            }
        }
    }
}

#[test]
fn four_quarter_turns_either_way_bring_every_box_back() {
    each(|case, g, b, start| {
        for (n, step) in TURNS.into_iter().enumerate() {
            let mut s = start;
            for _ in 0..4 {
                s = turn(g, b, s, step(s.1));
            }
            assert_eq!(s, start, "{case}, direction {n}");
        }
    });
}

#[test]
fn a_quarter_turn_and_its_reverse_bring_every_box_back() {
    each(|case, g, b, start| {
        let there = turn(g, b, start, cw(start.1));
        assert_eq!(turn(g, b, there, ccw(there.1)), start, "{case}: clockwise first");
        let there = turn(g, b, start, ccw(start.1));
        assert_eq!(
            turn(g, b, there, cw(there.1)),
            start,
            "{case}: counter-clockwise first"
        );
    });
}

#[test]
fn two_quarter_turns_land_where_a_half_turn_does_which_keeps_the_bounding_box() {
    each(|case, g, b, start| {
        let half = turn(g, b, start, start.1.then(Orient2::R180));
        assert_eq!(
            top_left(g, b, half),
            top_left(g, b, start),
            "{case}: the half turn keeps the bounding box"
        );
        for (n, step) in TURNS.into_iter().enumerate() {
            let one = turn(g, b, start, step(start.1));
            assert_eq!(turn(g, b, one, step(one.1)), half, "{case}, direction {n}");
        }
    });
}

#[test]
fn a_quarter_turn_moves_every_box_by_the_same_steps_wherever_it_lies() {
    for g in GRIDS {
        for w in 1..=8 {
            for h in 1..=8 {
                for o in STARTS {
                    for (n, step) in TURNS.into_iter().enumerate() {
                        let offsets: Vec<(i64, i64)> = ORIGINS
                            .iter()
                            .map(|k0| {
                                let (pos, b) = tool_box(g, *k0, w, h);
                                let before = top_left(g, &b, (pos, o));
                                let after = top_left(g, &b, turn(g, &b, (pos, o), step(o)));
                                (after.0 - before.0, after.1 - before.1)
                            })
                            .collect();
                        assert!(
                            offsets.windows(2).all(|p| p[0] == p[1]),
                            "grid {}/{}, {w} by {h} from {o:?}, direction {n}: {offsets:?}",
                            g.num(),
                            g.den()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn every_pin_stays_on_the_grid_through_quarter_turns() {
    each(|case, g, b, start| {
        for (n, step) in TURNS.into_iter().enumerate() {
            let mut s = start;
            for i in 1..=4 {
                s = turn(g, b, s, step(s.1));
                for cp in &b.cps {
                    let pin = s.0 + s.1.apply(cp.pos);
                    for v in [pin.x, pin.y] {
                        let near = g.snap_point(v).expect("in range");
                        assert!(
                            (v.0 - near.0).abs() <= 1,
                            "{case}, direction {n}: pin {} after {i} turns at {v:?}",
                            cp.idx
                        );
                    }
                }
            }
        }
    });
}
