// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Provisional connection routing: the three rules, stored routes, clamping and properties.

use ew_scene::route::{RouteEnd, connection_polyline, provisional_route};
use ew_schema::Dir4;
use ew_units::{Nm, Pt2, Rect};
use proptest::prelude::*;
use serde_json::Value;

fn p(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm(x), Nm(y))
}

fn end(x: i64, y: i64, dir: Option<Dir4>) -> RouteEnd {
    RouteEnd { pos: p(x, y), dir }
}

fn page() -> Rect {
    Rect::from_points(p(0, 0), p(1000, 1000))
}

fn route(a: RouteEnd, b: RouteEnd) -> Vec<Pt2> {
    provisional_route(&a, &b, page())
}

/// Every direction a CP end can carry, plus `None` and an unknown one.
fn all_dirs() -> Vec<Option<Dir4>> {
    vec![
        Some(Dir4::Up),
        Some(Dir4::Right),
        Some(Dir4::Down),
        Some(Dir4::Left),
        Some(Dir4::Unknown(Value::Null)),
        None,
    ]
}

#[test]
fn rule1_same_position_is_a_single_point() {
    for a_dir in all_dirs() {
        for b_dir in all_dirs() {
            let r = route(end(100, 200, a_dir.clone()), end(100, 200, b_dir));
            assert_eq!(r, vec![p(100, 200)]);
        }
    }
}

#[test]
fn rule2_same_x_or_same_y_is_straight() {
    for a_dir in all_dirs() {
        assert_eq!(
            route(end(100, 100, a_dir.clone()), end(100, 400, Some(Dir4::Up))),
            vec![p(100, 100), p(100, 400)]
        );
        assert_eq!(
            route(end(100, 100, a_dir), end(700, 100, None)),
            vec![p(100, 100), p(700, 100)]
        );
    }
}

#[test]
fn rule3_a_up_or_down_goes_vertical_first() {
    for a_dir in [Dir4::Up, Dir4::Down] {
        for b_dir in all_dirs() {
            assert_eq!(
                route(end(100, 100, Some(a_dir.clone())), end(400, 300, b_dir)),
                vec![p(100, 100), p(100, 300), p(400, 300)]
            );
        }
    }
}

#[test]
fn rule3_a_left_or_right_goes_horizontal_first() {
    for a_dir in [Dir4::Left, Dir4::Right] {
        for b_dir in all_dirs() {
            assert_eq!(
                route(end(100, 100, Some(a_dir.clone())), end(400, 300, b_dir)),
                vec![p(100, 100), p(400, 100), p(400, 300)]
            );
        }
    }
}

#[test]
fn rule3_unknown_a_dir_defers_to_b() {
    for a_dir in [None, Some(Dir4::Unknown(Value::Null))] {
        // b leaves vertically: arrive along b's vertical axis, corner (b.x, a.y).
        for b_dir in [Dir4::Up, Dir4::Down] {
            assert_eq!(
                route(end(100, 100, a_dir.clone()), end(400, 300, Some(b_dir))),
                vec![p(100, 100), p(400, 100), p(400, 300)]
            );
        }
        // b leaves horizontally: corner (a.x, b.y).
        for b_dir in [Dir4::Left, Dir4::Right] {
            assert_eq!(
                route(end(100, 100, a_dir.clone()), end(400, 300, Some(b_dir))),
                vec![p(100, 100), p(100, 300), p(400, 300)]
            );
        }
    }
}

#[test]
fn rule3_both_unknown_is_horizontal_first() {
    assert_eq!(
        route(end(100, 100, None), end(400, 300, None)),
        vec![p(100, 100), p(400, 100), p(400, 300)]
    );
    assert_eq!(
        route(
            end(100, 100, Some(Dir4::Unknown(Value::Null))),
            end(400, 300, Some(Dir4::Unknown(Value::Null)))
        ),
        vec![p(100, 100), p(400, 100), p(400, 300)]
    );
}

#[test]
fn upward_and_leftward_targets_work_too() {
    assert_eq!(
        route(end(400, 300, Some(Dir4::Up)), end(100, 100, None)),
        vec![p(400, 300), p(400, 100), p(100, 100)]
    );
    assert_eq!(
        route(end(400, 300, Some(Dir4::Left)), end(100, 100, None)),
        vec![p(400, 300), p(100, 300), p(100, 100)]
    );
}

#[test]
fn stored_route_keeps_interior_vertices_in_order() {
    let a = end(0, 0, None);
    let b = end(300, 300, None);
    let stored = [p(0, 100), p(200, 100), p(200, 300)];
    let r = connection_polyline(&a, &b, &stored, page());
    assert_eq!(r, vec![p(0, 0), p(0, 100), p(200, 100), p(200, 300), p(300, 300)]);
}

#[test]
fn stored_route_is_drawn_as_stored() {
    let a = end(0, 0, None);
    let b = end(300, 100, None);
    // (0,0) duplicates the start, (50,0) and (100,0) are collinear, (300,100) duplicates the end: all kept.
    let stored = [p(0, 0), p(50, 0), p(100, 0), p(300, 0), p(300, 100)];
    let r = connection_polyline(&a, &b, &stored, page());
    assert_eq!(
        r,
        vec![
            p(0, 0),
            p(0, 0),
            p(50, 0),
            p(100, 0),
            p(300, 0),
            p(300, 100),
            p(300, 100)
        ]
    );
    assert_eq!(r.len(), stored.len() + 2);
}

#[test]
fn stored_route_keeps_a_spike() {
    let a = end(0, 0, None);
    let b = end(0, 0, None);
    let r = connection_polyline(&a, &b, &[p(0, 500)], page());
    assert_eq!(r, vec![p(0, 0), p(0, 500), p(0, 0)]);
}

#[test]
fn empty_stored_route_falls_back_to_provisional() {
    let a = end(100, 100, Some(Dir4::Down));
    let b = end(400, 300, None);
    assert_eq!(
        connection_polyline(&a, &b, &[], page()),
        provisional_route(&a, &b, page())
    );
}

#[test]
fn clamping_pulls_ends_onto_the_page_edge() {
    let r = route(end(-500, 200, Some(Dir4::Right)), end(1500, 700, None));
    assert_eq!(r, vec![p(0, 200), p(1000, 200), p(1000, 700)]);
    // Both ends clamp onto the same point.
    let r = route(end(-5, -5, None), end(-9, -1, None));
    assert_eq!(r, vec![p(0, 0)]);
    // Stored points are clamped as well.
    let r = connection_polyline(
        &end(0, 0, None),
        &end(0, 1000, None),
        &[p(-50, 500), p(-50, 600)],
        page(),
    );
    assert_eq!(r, vec![p(0, 0), p(0, 500), p(0, 600), p(0, 1000)]);
    // An off-page stored point lands on the page edge.
    let r = connection_polyline(&end(0, 0, None), &end(300, 300, None), &[p(-50, 300)], page());
    assert_eq!(r, vec![p(0, 0), p(0, 300), p(300, 300)]);
}

fn coord() -> impl Strategy<Value = i64> {
    -1_000_000_000i64..=1_000_000_000
}

fn dir() -> impl Strategy<Value = Option<Dir4>> {
    prop_oneof![
        Just(None),
        Just(Some(Dir4::Up)),
        Just(Some(Dir4::Right)),
        Just(Some(Dir4::Down)),
        Just(Some(Dir4::Left)),
        Just(Some(Dir4::Unknown(Value::Null))),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn inside_page_routes_are_well_formed(
        ax in coord(), ay in coord(), bx in coord(), by in coord(),
        px in coord(), py in coord(), ad in dir(), bd in dir(),
    ) {
        // A page containing both ends (and one more random corner).
        let a = end(ax, ay, ad);
        let b = end(bx, by, bd);
        let pg = Rect::from_points(
            p(ax.min(bx).min(px), ay.min(by).min(py)),
            p(ax.max(bx).max(px), ay.max(by).max(py)),
        );
        let r = provisional_route(&a, &b, pg);
        prop_assert_eq!(r.first().copied(), Some(a.pos));
        prop_assert_eq!(r.last().copied(), Some(b.pos));
        prop_assert!((1..=3).contains(&r.len()));
        for w in r.windows(2) {
            prop_assert!(w[0].x == w[1].x || w[0].y == w[1].y);
            prop_assert!(w[0] != w[1]);
        }
        for v in &r {
            prop_assert!(pg.contains(*v));
        }
        prop_assert_eq!(&r, &provisional_route(&a, &b, pg));
        prop_assert_eq!(&r, &connection_polyline(&a, &b, &[], pg));
    }

    #[test]
    fn outside_page_routes_stay_on_the_page(
        ax in any::<i64>(), ay in any::<i64>(), bx in any::<i64>(), by in any::<i64>(),
        sx in any::<i64>(), sy in any::<i64>(),
        px0 in coord(), py0 in coord(), px1 in coord(), py1 in coord(),
        ad in dir(), bd in dir(),
        huge in any::<bool>(),
    ) {
        let (a, b, s) = if huge {
            let q = i64::MIN / 4;
            (end(q, ay / 4, ad), end(bx / 4, q, bd), p(sx / 4, q))
        } else {
            (end(ax, ay, ad), end(bx, by, bd), p(sx, sy))
        };
        let pg = Rect::from_points(p(px0, py0), p(px1, py1));
        for r in [
            provisional_route(&a, &b, pg),
            connection_polyline(&a, &b, &[s], pg),
        ] {
            prop_assert!(!r.is_empty());
            for v in &r {
                prop_assert!(pg.contains(*v));
            }
        }
    }
}
