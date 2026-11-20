// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The guides of a part placed by hand and their light nudge (docs/design/06a-drafting.md §6, §9.3 row 6).

use crate::kit::{Page, p, s};
use ew_drafting::consts::TICKS_PER_STEP;
use ew_drafting::{GuideKind, Guided, Part, Pt, guides};

/// About 100 % zoom on a native page: a 4 mm step is some 15 screen points.
const NATIVE: f64 = 15.0;

/// A two-pin upright device `name` hanging at `at` (its ordinary snap's place), not on any page.
fn hanging(name: &str, at: Pt) -> Part {
    let mut kit = Page::new(60, 40);
    kit.device(name, at);
    kit.page.parts.pop().expect("the device")
}

/// A page with the device -F1 at (10, 10): its lower pin faces down at (10, 14).
fn above() -> Page {
    let mut page = Page::new(60, 40);
    page.device("F1", p(10, 10));
    page
}

/// The guides of a device hanging with the pointer asking for its origin at `pointer` (ticks), on the grid point
/// the ordinary snap gives.
fn hang(page: &Page, pointer: Pt, zoom: f64) -> Guided {
    let snap = |v: i32| (v as f64 / TICKS_PER_STEP as f64).round() as i32 * TICKS_PER_STEP;
    let ghost = hanging("K1", Pt::new(snap(pointer.x), snap(pointer.y)));
    guides(&page.page, &ghost, pointer, zoom)
}

#[test]
fn a_pointer_within_three_quarters_of_a_step_takes_the_guide_and_beyond_it_lets_go() {
    let page = above();
    // 0.7 step right of -F1's line (11 ticks): the ordinary snap gives x 11, the nudge 10, in line.
    let near = hang(&page, Pt::new(s(10) + 11, s(20)), NATIVE);
    assert_eq!(near.at, p(10, 20));
    let g = &near.guides[0];
    assert_eq!(g.kind, GuideKind::InLine);
    assert!(g.in_line && g.tick.is_some(), "{g:?}");
    // From -F1's pin along its side, down through the hanging device.
    assert_eq!(g.line.0, p(10, 14));
    assert!(g.line.1.x == s(10) && g.line.1.y > s(24), "{g:?}");
    // 0.8 step (13 ticks): the ordinary snap's grid line, the guide shown dotted beside it.
    let far = hang(&page, Pt::new(s(10) + 13, s(20)), NATIVE);
    assert_eq!(far.at, p(11, 20));
    assert_eq!(far.guides.len(), 1);
    assert!(!far.guides[0].in_line && far.guides[0].tick.is_none());
    // Within the ordinary half step the snap already takes the line; past a whole step there is nothing to take.
    assert_eq!(hang(&page, Pt::new(s(10) + 7, s(20)), NATIVE).at, p(10, 20));
    assert_eq!(hang(&page, Pt::new(s(10) + 20, s(20)), NATIVE).at, p(11, 20));
}

#[test]
fn zoomed_in_the_band_is_never_more_than_eight_points_beyond_the_half_step() {
    let page = above();
    // At 64 points a step, 8 points are 2 ticks: the band ends at 10 ticks, so 0.7 step lets go.
    let zoomed = hang(&page, Pt::new(s(10) + 11, s(20)), 64.0);
    assert_eq!(zoomed.at, p(11, 20));
    assert_eq!(hang(&page, Pt::new(s(10) + 10, s(20)), 64.0).at, p(10, 20));
    // A zoom that is no number nudges nothing; one of no points a step takes the whole band.
    assert_eq!(hang(&page, Pt::new(s(10) + 9, s(20)), f64::NAN).at, p(11, 20));
    assert_eq!(hang(&page, Pt::new(s(10) + 12, s(20)), 0.0).at, p(10, 20));
}

#[test]
fn the_place_is_always_a_grid_point_and_the_same_whatever_the_order_of_the_page() {
    let mut page = above();
    page.device("F2", p(14, 10));
    page.three_pole("Q1", p(30, 8), 4);
    let mut shuffled = Page::new(60, 40);
    shuffled.page = page.page.clone();
    shuffled.page.parts.reverse();
    for dx in -40..=40 {
        for dy in [-9, 0, 5, 12] {
            let pointer = Pt::new(s(12) + dx, s(20) + dy);
            let g = hang(&page, pointer, NATIVE);
            assert!(
                g.at.x % TICKS_PER_STEP == 0 && g.at.y % TICKS_PER_STEP == 0,
                "{pointer:?}: {:?}",
                g.at
            );
            assert_eq!(g, hang(&shuffled, pointer, NATIVE), "{pointer:?}");
        }
    }
}

#[test]
fn with_no_pin_to_face_there_is_no_guide_and_the_place_is_the_ordinary_snaps() {
    let page = above();
    // Three steps across of -F1's line.
    let wide = hang(&page, Pt::new(s(13) + 6, s(20)), NATIVE);
    assert_eq!((wide.at, wide.guides.len()), (p(13, 20), 0));
    // Above -F1 its upper pin faces up and the hanging device's lower pin down: they face; below it with a part
    // between, nothing faces.
    let mut blocked = above();
    blocked.part("X", p(10, 16), (p(-2, 0), p(2, 2)), &[]);
    let g = hang(&blocked, Pt::new(s(10) + 11, s(20)), NATIVE);
    assert_eq!((g.at, g.guides.len()), (p(11, 20), 0));
    // A part without pins hangs without a guide.
    let mut bare = hanging("K1", p(11, 20));
    bare.pins.clear();
    let g = guides(&page.page, &bare, Pt::new(s(10) + 11, s(20)), NATIVE);
    assert_eq!((g.at, g.guides.len()), (p(11, 20), 0));
}

#[test]
fn a_pin_it_is_already_wired_to_guides_it_whatever_stands_between() {
    let mut page = above();
    page.part("X", p(10, 16), (p(-2, 0), p(2, 2)), &[]);
    // The device being dragged is -K1 of the page, wired from its upper pin to -F1's lower pin.
    page.device("K1", p(11, 20));
    let (a, b) = (page.pin("F1", "2"), page.pin("K1", "1"));
    page.wire("W1", a, b, &[]);
    let ghost = page.page.parts.last().cloned().expect("-K1");
    let g = guides(&page.page, &ghost, Pt::new(s(10) + 11, s(20)), NATIVE);
    assert_eq!(g.at, p(10, 20));
}

#[test]
fn a_level_guide_shows_but_never_changes_the_place() {
    let mut page = Page::new(60, 40);
    page.device("K2", p(10, 10));
    // A device drawn alike, 0.7 step below -K2's level: the snap takes y 11 and the Level guide does not move it.
    let g = hang(&page, Pt::new(s(20), s(10) + 11), NATIVE);
    assert_eq!(g.at, p(20, 11));
    let level: Vec<_> = g.guides.iter().filter(|g| g.kind == GuideKind::Level).collect();
    assert_eq!(level.len(), 1, "{:?}", g.guides);
    assert!(!level[0].in_line);
    assert_eq!(level[0].line.0.y, s(10));
    // Level with it: the guide runs through both upper pins.
    let g = hang(&page, Pt::new(s(20), s(10) + 3), NATIVE);
    assert_eq!(g.at, p(20, 10));
    assert!(g.guides.iter().any(|g| g.kind == GuideKind::Level && g.in_line));
}
