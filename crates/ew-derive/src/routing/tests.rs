// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The layout measures on small hand-built pages, each with and without its violation (WP-S2.19, ID-1498). Units are
//! millimetres written as nm through [`mm`].

use ew_ids::Uuid;
use ew_schema::Dir4;

use super::*;

const MM: i64 = 1_000_000;

fn mm(x: i64, y: i64) -> Point {
    (x * MM, y * MM)
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn wire(n: u128, points: &[Point]) -> DrawnConnection {
    DrawnConnection {
        id: id(n),
        polyline: points.to_vec(),
        ..DrawnConnection::default()
    }
}

fn cp(n: u128, at: Point, dir: Dir4, rep: u128, connections: &[u128]) -> CpPoint {
    CpPoint {
        id: id(n),
        at,
        dir: Some(dir),
        rep: id(rep),
        connections: connections.iter().map(|c| id(*c)).collect(),
    }
}

fn rect(a: Point, b: Point) -> Rect {
    [a.0, a.1, b.0, b.1]
}

/// Two CPs 40 mm apart on one row, `-A` at (0, 0) leaving right and `-B` at (40, 0) leaving left, joined by wire 1.
fn two_ends(line: &[Point]) -> PageGeometry {
    PageGeometry {
        cps: vec![
            cp(10, mm(0, 0), Dir4::Right, 100, &[1]),
            cp(11, mm(40, 0), Dir4::Left, 101, &[1]),
        ],
        connections: vec![wire(1, line)],
        ..PageGeometry::default()
    }
}

#[test]
fn a_a_wire_through_a_foreign_cp_touches_it_and_one_beside_it_does_not() {
    let mut page = two_ends(&[mm(0, 0), mm(40, 0)]);
    page.cps.push(cp(12, mm(20, 0), Dir4::Down, 102, &[]));
    page.junctions.push(JunctionPoint {
        id: id(20),
        at: mm(30, 0),
        connections: vec![],
    });
    let m = measure(&page);
    assert_eq!(
        m.touches,
        vec![
            Touch {
                connection: id(1),
                point: TouchedPoint::Cp(id(12)),
                at: mm(20, 0),
            },
            Touch {
                connection: id(1),
                point: TouchedPoint::Junction(id(20)),
                at: mm(30, 0),
            },
        ]
    );
    // Around them: down, along, and up again.
    let page = PageGeometry {
        cps: page.cps.clone(),
        junctions: page.junctions.clone(),
        connections: vec![wire(1, &[mm(0, 0), mm(0, 8), mm(40, 8), mm(40, 0)])],
        ..PageGeometry::default()
    };
    assert_eq!(measure(&page).touches, vec![]);
}

#[test]
fn a_the_ends_of_a_wire_are_no_touch_even_drawn_twice() {
    // A stored route drawn with its ends twice (the convention conflict): equal consecutive points are skipped.
    let page = two_ends(&[mm(0, 0), mm(0, 0), mm(40, 0), mm(40, 0)]);
    let m = measure(&page);
    assert_eq!(m, Measures::default());
}

#[test]
fn b_two_wires_on_one_line_overlap_and_two_side_by_side_do_not() {
    let page = PageGeometry {
        connections: vec![
            wire(1, &[mm(0, 0), mm(40, 0)]),
            wire(2, &[mm(30, 0), mm(30, 0), mm(60, 0), mm(60, 20)]),
            wire(3, &[mm(0, 4), mm(40, 4)]),
        ],
        ..PageGeometry::default()
    };
    let m = measure(&page);
    assert_eq!(
        m.overlaps,
        vec![Overlap {
            connections: [id(1), id(2)],
            stretches: vec![(mm(30, 0), mm(40, 0))],
        }]
    );
    // The ends of the shared stretch are no crossing.
    assert_eq!(m.crossings, vec![]);
    // End to end on one line: one point shared, no overlap (a crossing, as no CP or junction is there).
    let page = PageGeometry {
        connections: vec![wire(1, &[mm(0, 0), mm(40, 0)]), wire(2, &[mm(40, 0), mm(60, 0)])],
        ..PageGeometry::default()
    };
    let m = measure(&page);
    assert_eq!(m.overlaps, vec![]);
    assert_eq!(
        m.crossings,
        vec![Crossing {
            connections: [id(1), id(2)],
            at: mm(40, 0),
        }]
    );
}

#[test]
fn b_two_wires_leaving_one_junction_along_one_line_overlap() {
    let page = PageGeometry {
        junctions: vec![JunctionPoint {
            id: id(20),
            at: mm(0, 0),
            connections: vec![id(1), id(2)],
        }],
        connections: vec![
            wire(1, &[mm(0, 0), mm(0, 20)]),
            wire(2, &[mm(0, 0), mm(0, 30), mm(10, 30)]),
        ],
        ..PageGeometry::default()
    };
    let m = measure(&page);
    assert_eq!(
        m.overlaps,
        vec![Overlap {
            connections: [id(1), id(2)],
            stretches: vec![(mm(0, 0), mm(0, 20))],
        }]
    );
    assert_eq!(m.crossings, vec![]);
}

#[test]
fn c_a_wire_through_a_body_is_named_and_its_own_escape_is_not() {
    // -A's body reaches from x = −4 to 4 around its CP at the origin, which leaves right: the escape.
    let mut page = two_ends(&[mm(0, 0), mm(40, 0)]);
    page.bodies = vec![
        Body {
            rep: id(100),
            rect: rect(mm(-4, -4), mm(4, 4)),
        },
        Body {
            rep: id(101),
            rect: rect(mm(36, -4), mm(44, 4)),
        },
    ];
    assert_eq!(measure(&page).through_bodies, vec![]);
    // A body of a third symbol in the way.
    page.bodies.push(Body {
        rep: id(102),
        rect: rect(mm(16, -2), mm(24, 2)),
    });
    assert_eq!(
        measure(&page).through_bodies,
        vec![Through {
            connection: id(1),
            rep: id(102),
            stretches: vec![(mm(16, 0), mm(24, 0))],
        }]
    );
    // Leaving -A against its direction runs through its own body.
    let mut page = two_ends(&[mm(0, 0), mm(-10, 0), mm(-10, 10), mm(40, 10), mm(40, 0)]);
    page.bodies = vec![Body {
        rep: id(100),
        rect: rect(mm(-4, -4), mm(4, 4)),
    }];
    assert_eq!(
        measure(&page).through_bodies,
        vec![Through {
            connection: id(1),
            rep: id(100),
            stretches: vec![(mm(0, 0), mm(-4, 0))],
        }]
    );
}

#[test]
fn c_a_wire_along_a_body_s_edge_is_not_through_it() {
    let mut page = two_ends(&[mm(0, 0), mm(40, 0)]);
    page.bodies = vec![Body {
        rep: id(102),
        rect: rect(mm(10, 0), mm(20, 10)),
    }];
    assert_eq!(measure(&page).through_bodies, vec![]);
}

#[test]
fn d_a_wire_through_a_text_is_named_and_one_past_it_is_not() {
    let mut page = two_ends(&[mm(0, 0), mm(40, 0)]);
    page.texts = vec![TextBox {
        rep: id(102),
        rect: rect(mm(18, -1), mm(26, 2)),
    }];
    assert_eq!(
        measure(&page).through_texts,
        vec![Through {
            connection: id(1),
            rep: id(102),
            stretches: vec![(mm(18, 0), mm(26, 0))],
        }]
    );
    page.texts[0].rect = rect(mm(18, 1), mm(26, 4));
    assert_eq!(measure(&page).through_texts, vec![]);
}

#[test]
fn e_two_wires_crossing_are_named_and_two_meeting_at_a_shared_end_are_not() {
    let page = PageGeometry {
        connections: vec![
            wire(1, &[mm(0, 0), mm(40, 0)]),
            wire(2, &[mm(20, -10), mm(20, 10)]),
        ],
        ..PageGeometry::default()
    };
    assert_eq!(
        measure(&page).crossings,
        vec![Crossing {
            connections: [id(1), id(2)],
            at: mm(20, 0),
        }]
    );
    // A diagonal crossing, rounded to the nm.
    let page = PageGeometry {
        connections: vec![wire(1, &[(0, 0), (3, 3)]), wire(2, &[(0, 2), (3, 0)])],
        ..PageGeometry::default()
    };
    assert_eq!(
        measure(&page).crossings,
        vec![Crossing {
            connections: [id(1), id(2)],
            at: (1, 1),
        }]
    );
    let page = PageGeometry {
        junctions: vec![JunctionPoint {
            id: id(20),
            at: mm(20, 0),
            connections: vec![id(1), id(2)],
        }],
        connections: vec![wire(1, &[mm(0, 0), mm(20, 0)]), wire(2, &[mm(20, 0), mm(20, 10)])],
        ..PageGeometry::default()
    };
    assert_eq!(measure(&page).crossings, vec![]);
}

#[test]
fn e_a_wire_ending_on_another_is_a_touch_not_a_crossing() {
    let mut page = two_ends(&[mm(0, 0), mm(40, 0)]);
    page.cps.push(cp(12, mm(20, 0), Dir4::Down, 102, &[2]));
    page.connections.push(wire(2, &[mm(20, 0), mm(20, 10)]));
    let m = measure(&page);
    assert_eq!(m.crossings, vec![]);
    assert_eq!(
        m.touches,
        vec![Touch {
            connection: id(1),
            point: TouchedPoint::Cp(id(12)),
            at: mm(20, 0),
        }]
    );
}

#[test]
fn f_bends_count_the_turns_and_not_a_straight_vertex() {
    let page = PageGeometry {
        connections: vec![
            wire(1, &[mm(0, 0), mm(10, 0), mm(20, 0)]),
            wire(2, &[mm(0, 10), mm(10, 10), mm(10, 20), mm(20, 20)]),
            wire(3, &[mm(0, 30), mm(10, 30), mm(5, 30)]),
        ],
        ..PageGeometry::default()
    };
    let m = measure(&page);
    assert_eq!(
        m.bends,
        vec![
            Bends {
                connection: id(2),
                bends: 2,
            },
            Bends {
                connection: id(3),
                bends: 1,
            },
        ]
    );
    assert_eq!(m.bend_total(), 3);
}

#[test]
fn g_a_junction_with_four_arms_is_named_and_one_with_three_is_not() {
    let arms = [mm(0, -10), mm(10, 0), mm(0, 10), mm(-10, 0)];
    let page = |n: usize| PageGeometry {
        junctions: vec![JunctionPoint {
            id: id(20),
            at: mm(0, 0),
            connections: (1..=4).take(n).map(id).collect(),
        }],
        connections: arms[..n]
            .iter()
            .zip(1u128..)
            .map(|(&p, k)| wire(k, &[mm(0, 0), p]))
            .collect(),
        ..PageGeometry::default()
    };
    assert_eq!(
        measure(&page(4)).four_way,
        vec![FourWay {
            junction: id(20),
            arms: 4,
            at: mm(0, 0),
        }]
    );
    assert_eq!(measure(&page(3)).four_way, vec![]);
    assert_eq!(measure(&page(4)).hard(), 1);
}

#[test]
fn e_a_slanted_crossing_at_the_record_limit_does_not_overflow() {
    // ADR-0005's largest coordinate: the crossing's fraction is shortened before it scales a difference.
    let l = (1_i64 << 53) - 1;
    let page = PageGeometry {
        connections: vec![wire(1, &[(-l, -l), (l, l)]), wire(2, &[(-l, l), (l, -l)])],
        ..PageGeometry::default()
    };
    let m = measure(&page);
    assert_eq!(m.crossings.len(), 1);
    let at = m.crossings[0].at;
    assert!(at.0.abs() <= 1 && at.1.abs() <= 1, "{at:?}");
}

proptest::proptest! {
    /// WP-S2.19 step 1: every route the search finds breaks no hard rule by the measures, an oracle
    /// independent of the search: no touch of another CP (the reserved points), no run along another wire, no
    /// stretch inside an obstacle. The ends, their stubs and the stub moves are the search's own (never blocked), so
    /// the cases keep the obstacles, wires and reserved points off them.
    #[test]
    fn every_found_route_breaks_no_hard_rule_by_the_measures(
        fx in -6i64..6, fy in -6i64..6, tx in -6i64..6, ty in -6i64..6,
        fd in proptest::option::of(0usize..4), td in proptest::option::of(0usize..4),
        ob in proptest::collection::vec((-8i64..8, -8i64..8, 1i64..4, 1i64..4), 0..3),
        ws in proptest::collection::vec((-9i64..9, -9i64..9, -6i64..6, proptest::bool::ANY), 0..4),
        rs in proptest::collection::vec((-8i64..8, -8i64..8), 0..3),
    ) {
        let dir = |k: Option<usize>| k.map(|k| Heading::ALL[k]);
        let (from, to) = (
            RouteEnd { at: mm(fx, fy), dir: dir(fd) },
            RouteEnd { at: mm(tx, ty), dir: dir(td) },
        );
        let stub = |e: RouteEnd| e.dir.map_or(e.at, |d| (e.at.0 + d.step().0 * MM, e.at.1 + d.step().1 * MM));
        let own = [(from.at, stub(from)), (to.at, stub(to))];
        let on = |p: Point, (a, b): (Point, Point)| {
            crate::on_segment(
                ew_units::Pt2::new(ew_units::Nm(p.0), ew_units::Nm(p.1)),
                ew_units::Pt2::new(ew_units::Nm(a.0), ew_units::Nm(a.1)),
                ew_units::Pt2::new(ew_units::Nm(b.0), ew_units::Nm(b.1)),
            )
        };
        let obstacles: Vec<Rect> = ob.iter().map(|&(x, y, w, h)| rect(mm(x, y), mm(x + w, y + h))).collect();
        let wires: Vec<(Point, Point)> = ws
            .iter()
            .map(|&(x, y, len, horizontal)| (mm(x, y), if horizontal { mm(x + len, y) } else { mm(x, y + len) }))
            .collect();
        let reserved: Vec<Point> = rs.iter().map(|&(x, y)| mm(x, y)).collect();
        let strictly = |p: Point, r: &Rect| r[0] < p.0 && p.0 < r[2] && r[1] < p.1 && p.1 < r[3];
        for (a, b) in own {
            let mid = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
            proptest::prop_assume!(obstacles.iter().all(|r| !strictly(a, r) && !strictly(b, r) && !strictly(mid, r)));
            proptest::prop_assume!(reserved.iter().all(|&q| !on(q, (a, b))));
            proptest::prop_assume!(wires.iter().all(|&(c, d)| !on(c, (a, b)) && !on(d, (a, b)) && !on(a, (c, d)) && !on(b, (c, d))));
        }
        let rq = RouteRequest {
            from,
            to,
            obstacles: obstacles.clone(),
            texts: Vec::new(),
            grid: (MM, 1),
            area: [
                fx.min(tx) * MM - 4 * MM,
                fy.min(ty) * MM - 4 * MM,
                fx.max(tx) * MM + 4 * MM,
                fy.max(ty) * MM + 4 * MM,
            ],
            arrive: None,
            own: [Vec::new(), Vec::new()],
            wires: wires.iter().map(|&(a, b)| [a, b]).collect(),
            reserved: reserved.clone(),
            cross_cost: 0,
            mates: Vec::new(),
        };
        let routed = route(&rq);
        if !routed.direct {
            let mut page = PageGeometry {
                bodies: obstacles.iter().map(|r| Body { rep: id(900), rect: *r }).collect(),
                cps: reserved.iter().zip(500u128..).map(|(&at, k)| cp(k, at, Dir4::Up, 901, &[])).collect(),
                connections: vec![wire(1, &routed.points)],
                ..PageGeometry::default()
            };
            page.connections.extend(wires.iter().zip(2u128..).map(|(&(a, b), k)| wire(k, &[a, b])));
            let m = measure(&page);
            let mine = |c: &Uuid| *c == id(1);
            proptest::prop_assert!(!m.touches.iter().any(|t| mine(&t.connection)), "{:?} {:?}", routed.points, m.touches);
            proptest::prop_assert!(!m.overlaps.iter().any(|o| o.connections.iter().any(mine)), "{:?} {:?}", routed.points, m.overlaps);
            proptest::prop_assert!(!m.through_bodies.iter().any(|t| mine(&t.connection)), "{:?} {:?}", routed.points, m.through_bodies);
        }
    }
}

#[test]
fn a_a_cp_stacked_on_the_wire_s_own_end_is_no_touch() {
    // A terminal's clamps b and c drawn at one point: the wire ending at b does not touch c.
    let mut page = two_ends(&[mm(0, 0), mm(40, 0)]);
    page.cps.push(cp(12, mm(0, 0), Dir4::Right, 100, &[]));
    assert_eq!(measure(&page).touches, vec![]);
}

#[test]
fn b_two_wires_sharing_a_cp_s_stub_do_not_overlap_there() {
    // Two wires leave one CP along its stub (one grid step, 1 mm here) and part: the stub is theirs to share.
    let shared = |second: &[Point]| PageGeometry {
        grid: (MM, 1),
        cps: vec![cp(10, mm(0, 0), Dir4::Right, 100, &[1, 2])],
        connections: vec![wire(1, &[mm(0, 0), mm(1, 0), mm(1, 10)]), wire(2, second)],
        ..PageGeometry::default()
    };
    assert_eq!(
        measure(&shared(&[mm(0, 0), mm(1, 0), mm(1, -10)])).overlaps,
        vec![]
    );
    // Beyond the stub they overlap.
    assert_eq!(
        measure(&shared(&[mm(0, 0), mm(1, 0), mm(1, 5), mm(5, 5)])).overlaps,
        vec![Overlap {
            connections: [id(1), id(2)],
            stretches: vec![(mm(1, 0), mm(1, 5))],
        }]
    );
}

#[test]
fn b_a_shared_stub_ends_on_the_grid_point_where_a_step_is_no_whole_nm() {
    // The QET grid (7 937 500 / 3 nm): the CP at grid index 38 (100 541 667 nm) leaves up, and its stub ends on the
    // grid point of index 37 (97 895 833 nm), where both wires turn, not at the CP less a rounded step (97 895 834).
    let (num, den) = (7_937_500, 3);
    let at = |k: i64| (k * num + 1) / den;
    let page = PageGeometry {
        grid: (num, den),
        cps: vec![cp(10, (at(53), at(38)), Dir4::Up, 100, &[1, 2])],
        connections: vec![
            wire(1, &[(at(23), at(29)), (at(53), at(29)), (at(53), at(38))]),
            wire(2, &[(at(53), at(38)), (at(53), at(37)), (at(68), at(37))]),
        ],
        ..PageGeometry::default()
    };
    assert_eq!(at(37), 97_895_833);
    assert_eq!(measure(&page).overlaps, vec![]);
}
