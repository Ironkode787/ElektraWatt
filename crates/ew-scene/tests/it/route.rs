// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The polyline a connection is drawn along: stored routes as stored, an unrouted connection as the straight line
//! between its ends, clamping and properties.

use ew_scene::route::{RouteEnd, connection_polyline};
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

/// The unrouted line of two ends on [`page`].
fn unrouted(a: RouteEnd, b: RouteEnd) -> Vec<Pt2> {
    connection_polyline(&a, &b, &[], page())
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

/// A connection without a stored route is the straight line between its ends whatever their directions,
/// a single point where they meet (nothing is drawn); never an L.
#[test]
fn an_unrouted_connection_is_the_straight_line_between_its_ends() {
    for da in all_dirs() {
        for db in all_dirs() {
            let (a, b) = (end(100, 100, da.clone()), end(400, 300, db.clone()));
            assert_eq!(unrouted(a, b), vec![p(100, 100), p(400, 300)], "{da:?} {db:?}");
            let (a, b) = (end(100, 100, da.clone()), end(100, 100, db.clone()));
            assert_eq!(unrouted(a, b), vec![p(100, 100)], "{da:?} {db:?}");
        }
    }
}

#[test]
fn a_stale_route_joins_its_ends_by_straight_segments() {
    // A route whose end vertices are not the ends' points (an end moved without the route following) is
    // joined to each end by one straight segment.
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
    // The route holds both ends, drawn once each; (50,0) and (100,0) are collinear and kept.
    let stored = [p(0, 0), p(50, 0), p(100, 0), p(300, 0), p(300, 100)];
    let r = connection_polyline(&a, &b, &stored, page());
    assert_eq!(r, stored.to_vec());
}

#[test]
fn stored_route_keeps_a_spike() {
    let a = end(0, 0, None);
    let b = end(0, 0, None);
    let r = connection_polyline(&a, &b, &[p(0, 500)], page());
    assert_eq!(r, vec![p(0, 0), p(0, 500), p(0, 0)]);
}

#[test]
fn clamping_pulls_ends_onto_the_page_edge() {
    let r = unrouted(end(-500, 200, Some(Dir4::Right)), end(1500, 700, None));
    assert_eq!(r, vec![p(0, 200), p(1000, 700)]);
    // Both ends clamp onto the same point.
    let r = unrouted(end(-5, -5, None), end(-9, -1, None));
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
    fn inside_page_unrouted_lines_are_well_formed(
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
        let r = connection_polyline(&a, &b, &[], pg);
        prop_assert_eq!(r.first().copied(), Some(a.pos));
        prop_assert_eq!(r.last().copied(), Some(b.pos));
        prop_assert!((1..=2).contains(&r.len()));
        for v in &r {
            prop_assert!(pg.contains(*v));
        }
    }

    #[test]
    fn outside_page_lines_stay_on_the_page(
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
            connection_polyline(&a, &b, &[], pg),
            connection_polyline(&a, &b, &[s], pg),
        ] {
            prop_assert!(!r.is_empty());
            for v in &r {
                prop_assert!(pg.contains(*v));
            }
        }
    }
}
