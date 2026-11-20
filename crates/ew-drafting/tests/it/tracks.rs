// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tracks (docs/design/06a-drafting.md §3.5, §9.3 row 3): wires that step sideways between two rows of pins nest and
//! do not cross, two steps apart where there is room; the count the placer reads.

use crate::kit::{Page, p, s};
use crate::route::routed;
use ew_drafting::consts::TRACK_PITCH;
use ew_drafting::{DraftPage, Pt, Side, Tick, WireId, geom, measure, tracks_needed};

fn route_of(page: &DraftPage, w: WireId) -> Vec<Pt> {
    geom::clean(&page.wires.iter().find(|x| x.id == w).expect("a wire").route)
}

/// The line of a wire's horizontal middle, when it steps once (a Z or a U: three segments).
fn track(page: &DraftPage, w: WireId) -> Tick {
    let r = route_of(page, w);
    assert_eq!(r.len(), 4, "out, across, in: {r:?}");
    assert_eq!(r[1].y, r[2].y, "{r:?}");
    r[1].y
}

/// The page routed with no fault, no wire left unrouted; its crossings.
fn routed_clean(page: &DraftPage) -> (DraftPage, usize) {
    let done = routed(page);
    let q = measure(&done);
    assert_eq!((q.faults, q.unrouted), (0, 0), "{:?}", q.findings);
    (done, q.counts.crossings)
}

#[test]
fn a_two_pole_device_over_one_of_another_pitch_has_one_pole_straight_and_one_step() {
    let mut b = Page::new(40, 30);
    let upper = [
        ("1", p(0, 0), Side::Up),
        ("3", p(2, 0), Side::Up),
        ("2", p(0, 4), Side::Down),
        ("4", p(2, 4), Side::Down),
    ];
    let lower = [
        ("1", p(0, 0), Side::Up),
        ("3", p(3, 0), Side::Up),
        ("2", p(0, 4), Side::Down),
        ("4", p(3, 4), Side::Down),
    ];
    b.part("Q1", p(10, 10), (p(-1, 0), p(3, 4)), &upper);
    b.part("K1", p(10, 19), (p(-1, 0), p(4, 4)), &lower);
    let straight = b.wire("a", b.pin("Q1", "2"), b.pin("K1", "1"), &[]);
    let step = b.wire("b", b.pin("Q1", "4"), b.pin("K1", "3"), &[]);
    let (done, crossings) = routed_clean(&b.page);
    assert_eq!(crossings, 0);
    assert_eq!(route_of(&done, straight).len(), 2, "one pole straight");
    // The other steps once, on the first track: two steps under the row it leaves.
    assert_eq!(track(&done, step), s(14) + TRACK_PITCH);
}

#[test]
fn five_sensors_over_pins_two_steps_apart_step_in_and_none_crosses() {
    let mut b = Page::new(40, 30);
    let xs = [10, 15, 20, 25, 30];
    for (k, x) in xs.iter().enumerate() {
        b.device(&format!("B{k}"), p(*x, 4));
    }
    b.card("A1", p(14, 16), (12, 4), &[2, 4, 6, 8, 10], true);
    let w: Vec<WireId> = (0..xs.len())
        .map(|k| {
            b.wire(
                &format!("w{k}"),
                b.pin(&format!("B{k}"), "2"),
                b.pin("A1", &format!("{k}")),
                &[],
            )
        })
        .collect();
    let (done, crossings) = routed_clean(&b.page);
    assert_eq!(crossings, 0);
    assert_eq!(
        route_of(&done, w[2]).len(),
        2,
        "the middle sensor stands over its pin"
    );
    // On each side the inner wire takes the track nearer the sensors, the outer one the next, two steps further.
    for (inner, outer) in [(w[1], w[0]), (w[3], w[4])] {
        assert_eq!(track(&done, inner), s(8) + TRACK_PITCH);
        assert_eq!(track(&done, outer), track(&done, inner) + TRACK_PITCH);
    }
}

#[test]
fn four_wires_between_two_rows_in_the_same_order_take_four_tracks_and_cross_nowhere() {
    let mut b = Page::new(40, 30);
    b.card("U", p(10, 6), (20, 4), &[2, 4, 6, 8], false);
    b.card("L", p(10, 22), (20, 4), &[12, 14, 16, 18], true);
    let w: Vec<WireId> = (0..4)
        .map(|k| {
            b.wire(
                &format!("w{k}"),
                b.pin("U", &format!("{k}")),
                b.pin("L", &format!("{k}")),
                &[],
            )
        })
        .collect();
    let (done, crossings) = routed_clean(&b.page);
    assert_eq!(crossings, 0);
    let mut tracks: Vec<Tick> = w.iter().map(|x| track(&done, *x)).collect();
    tracks.sort();
    tracks.dedup();
    assert_eq!(tracks.len(), 4, "{tracks:?}");
    assert!(
        tracks.windows(2).all(|t| t[1] - t[0] == TRACK_PITCH),
        "{tracks:?}"
    );
}

#[test]
fn two_wires_that_must_swap_sides_cross_exactly_once() {
    let mut b = Page::new(40, 30);
    b.card("U", p(10, 6), (8, 4), &[2, 4], false);
    b.card("L", p(6, 20), (14, 4), &[4, 10], true);
    // From 12 to 16, and from 14 to 10: each leaves inside the other's span.
    let a = b.wire("a", b.pin("U", "0"), b.pin("L", "1"), &[]);
    let c = b.wire("c", b.pin("U", "1"), b.pin("L", "0"), &[]);
    let (done, crossings) = routed_clean(&b.page);
    assert_eq!(crossings, 1);
    assert_ne!(track(&done, a), track(&done, c));
}

#[test]
fn a_channel_with_one_free_track_sends_the_second_wire_to_the_search() {
    // Rows three steps apart: one track, two steps under the upper row. The wire from 14 to 20 would run over the
    // stub of the pin at 18, so the wire from 12 to 18 takes the track and the other is searched.
    let mut b = Page::new(40, 30);
    b.card("U", p(10, 6), (12, 4), &[2, 4], false);
    b.card("L", p(10, 13), (12, 4), &[8, 10], true);
    let first = b.wire("a", b.pin("U", "0"), b.pin("L", "0"), &[]);
    let second = b.wire("b", b.pin("U", "1"), b.pin("L", "1"), &[]);
    let (done, _) = routed_clean(&b.page);
    assert_eq!(track(&done, first), s(10) + TRACK_PITCH);
    assert!(!route_of(&done, second).is_empty());
}

#[test]
fn a_u_inside_another_u_takes_the_nearer_track() {
    let mut b = Page::new(40, 30);
    b.card("U", p(10, 6), (12, 4), &[2, 4, 8, 10], false);
    let outer = b.wire("outer", b.pin("U", "0"), b.pin("U", "3"), &[]);
    let inner = b.wire("inner", b.pin("U", "1"), b.pin("U", "2"), &[]);
    let (done, crossings) = routed_clean(&b.page);
    assert_eq!(crossings, 0);
    assert_eq!(track(&done, inner), s(10) + TRACK_PITCH);
    assert_eq!(track(&done, outer), track(&done, inner) + TRACK_PITCH);
}

#[test]
fn the_placer_s_count_follows_the_assignment() {
    // Four wires that step the same way over each other's legs: four tracks.
    assert_eq!(
        tracks_needed(&[(s(0), s(10)), (s(2), s(12)), (s(4), s(14)), (s(6), s(16))]),
        4
    );
    // Steps side by side, their spans apart: one track.
    assert_eq!(tracks_needed(&[(s(0), s(2)), (s(5), s(7)), (s(10), s(12))]), 1);
    // A pair that swaps sides: two tracks; a wire in line: none.
    assert_eq!(tracks_needed(&[(s(2), s(6)), (s(4), s(0))]), 2);
    assert_eq!(tracks_needed(&[(s(3), s(3))]), 0);
}
