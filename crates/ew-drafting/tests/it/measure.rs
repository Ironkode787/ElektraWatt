// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The measure (docs/design/06a-drafting.md §1.2) on pages built to have exactly one of each thing it counts.

use crate::kit::{Page, id, p, s};
use ew_drafting::consts::MARK_JOG;
use ew_drafting::{FixedText, PartId, Quality, Rect, Tick, measure};

/// The number of each fault list, F2 to F7.
fn faults(q: &Quality) -> [usize; 6] {
    let f = &q.findings;
    [
        f.through_points.len(),
        f.on_wires.len(),
        f.through_parts.len(),
        f.four_way.len(),
        f.false_joints.len(),
        f.parts_on_parts.len(),
    ]
}

/// A page with two devices wired straight, which has no fault.
fn clean() -> Page {
    let mut b = Page::new(60, 40);
    b.device("K1", p(10, 10));
    b.device("K2", p(10, 20));
    b.run("w", b.pin("K1", "2"), b.pin("K2", "1"), &[]);
    b
}

#[test]
fn a_clean_page_has_no_fault_and_one_straight_wire() {
    let q = measure(&clean().page);
    assert_eq!(faults(&q), [0; 6]);
    assert_eq!(q.faults, 0);
    assert_eq!(q.unrouted, 0);
    assert_eq!((q.counts.straight, q.counts.wires, q.counts.bends), (1, 1, 0));
    assert_eq!(q.counts.length, i64::from(s(6)));
    assert_eq!(q.counts.detour, 100);
}

#[test]
fn f2_a_wire_through_a_pin_it_does_not_end_at() {
    let mut b = clean();
    b.device("K3", p(30, 10));
    // A wire from K3 that runs on through K2's lower pin.
    let (a, c) = (b.pin("K3", "2"), b.junction("j", p(5, 24)));
    b.run("x", a, c, &[p(30, 24)]);
    let q = measure(&b.page);
    assert_eq!(faults(&q), [1, 0, 0, 0, 0, 0], "{:?}", q.findings);
}

#[test]
fn f3_a_wire_on_another_wire_but_along_their_shared_run_from_a_common_pin() {
    // Both leave K1's lower pin, run together and part: their shared run, no fault.
    let mut b = Page::new(60, 40);
    b.device("K1", p(10, 10));
    let (j1, j2) = (b.junction("j1", p(20, 20)), b.junction("j2", p(4, 20)));
    b.run("a", b.pin("K1", "2"), j1, &[p(10, 17), p(20, 17)]);
    b.run("b", b.pin("K1", "2"), j2, &[p(10, 17), p(4, 17)]);
    assert_eq!(faults(&measure(&b.page)), [0; 6]);
    // One runs on along the other after they part: a fault.
    let mut c = Page::new(60, 40);
    c.device("K1", p(10, 10));
    let (j1, j2) = (c.junction("j1", p(20, 20)), c.junction("j2", p(24, 20)));
    c.run("a", c.pin("K1", "2"), j1, &[p(10, 17), p(20, 17)]);
    c.run(
        "b",
        c.pin("K1", "2"),
        j2,
        &[p(10, 15), p(14, 15), p(14, 17), p(24, 17)],
    );
    assert_eq!(faults(&measure(&c.page)), [0, 1, 0, 0, 0, 0]);
}

#[test]
fn f4_a_wire_through_a_body_but_straight_out_of_its_own_pin() {
    let mut b = clean();
    let (a, c) = (b.junction("a", p(4, 12)), b.junction("c", p(20, 12)));
    b.run("x", a, c, &[]);
    let q = measure(&b.page);
    assert_eq!(faults(&q), [0, 0, 1, 0, 0, 0], "{:?}", q.findings);
    assert_eq!(q.findings.through_parts[0].1, PartId(id("K1")));
}

#[test]
fn f5_a_junction_with_four_arms() {
    let mut b = Page::new(60, 40);
    let j = b.junction("j", p(20, 20));
    for (n, at) in [
        ("u", p(20, 10)),
        ("d", p(20, 30)),
        ("l", p(10, 20)),
        ("r", p(30, 20)),
    ] {
        let e = b.junction(n, at);
        b.run(n, j, e, &[]);
    }
    assert_eq!(faults(&measure(&b.page)), [0, 0, 0, 1, 0, 0]);
}

#[test]
fn f6_two_corners_on_one_point() {
    let mut b = Page::new(60, 40);
    let (a, c, d, e) = (
        b.junction("a", p(10, 20)),
        b.junction("c", p(20, 10)),
        b.junction("d", p(30, 20)),
        b.junction("e", p(20, 30)),
    );
    b.run("x", a, c, &[p(20, 20)]);
    b.run("y", d, e, &[p(20, 20)]);
    let q = measure(&b.page);
    assert_eq!(faults(&q), [0, 0, 0, 0, 1, 0], "{:?}", q.findings);
    assert_eq!(q.counts.crossings, 0, "a false joint is no crossing as well");
}

#[test]
fn f7_a_part_on_a_part_and_a_part_outside_the_drawing_area() {
    let mut b = clean();
    b.device("K3", p(11, 11));
    assert_eq!(faults(&measure(&b.page)), [0, 0, 0, 0, 0, 1]);
    let mut c = clean();
    c.device("K3", p(60, 10));
    let q = measure(&c.page);
    assert_eq!(q.findings.parts_on_parts, vec![(PartId(id("K3")), None)]);
}

#[test]
fn an_unrouted_wire_is_counted_apart() {
    let mut b = clean();
    b.device("K3", p(30, 10));
    b.wire("u", b.pin("K2", "2"), b.pin("K3", "1"), &[]);
    let q = measure(&b.page);
    assert_eq!((q.faults, q.unrouted), (0, 1));
    assert_eq!(q.counts.wires, 2);
}

#[test]
fn a_crossing_and_the_bends_are_counted() {
    let mut b = Page::new(60, 40);
    let (a, c, d, e) = (
        b.junction("a", p(10, 20)),
        b.junction("c", p(30, 20)),
        b.junction("d", p(20, 10)),
        b.junction("e", p(25, 30)),
    );
    b.run("x", a, c, &[]);
    b.run("y", d, e, &[p(20, 25), p(25, 25)]);
    let q = measure(&b.page);
    assert_eq!(q.faults, 0);
    assert_eq!(q.counts.crossings, 1);
    assert_eq!(q.counts.bends, 2);
    assert_eq!(q.counts.straight, 1);
}

/// D§9.3 row 1: the measure counts a turn-back on a wire drawn round a caption.
#[test]
fn a_wire_drawn_round_a_caption_is_a_turn_back() {
    let mut b = Page::new(60, 40);
    let (a, c) = (b.junction("a", p(10, 20)), b.junction("c", p(30, 20)));
    b.page.fixed_texts.push(FixedText {
        owner: id("caption"),
        rect: Rect::new(p(18, 18), p(22, 21)),
    });
    // Up, along, down and on: round the caption's top.
    b.run("w", a, c, &[p(16, 20), p(16, 16), p(24, 16), p(24, 20)]);
    let q = measure(&b.page);
    assert_eq!(q.counts.turn_backs, 1);
    assert!(q.counts.detour > 100);
    // Drawn straight through it, it crosses the text instead.
    let mut d = Page::new(60, 40);
    let (a, c) = (d.junction("a", p(10, 20)), d.junction("c", p(30, 20)));
    d.page.fixed_texts = b.page.fixed_texts.clone();
    d.run("w", a, c, &[]);
    let straight = measure(&d.page);
    assert_eq!((straight.counts.turn_backs, straight.counts.across_texts), (0, 1));
}

/// A label that has no clear place is a clash in the cost, and its rank is counted.
#[test]
fn a_label_on_a_text_is_counted_in_the_cost() {
    let mut b = Page::new(60, 40);
    b.device("K1", p(10, 10));
    let tag = b.page.labels[0].clone();
    // Cover every place of the tag with fixed texts.
    for r in [
        Rect::new(p(6, 9), p(9, 15)),
        Rect::new(p(11, 9), p(14, 15)),
        Rect::new(p(7, 6), p(13, 10)),
        Rect::new(p(7, 14), p(13, 16)),
    ] {
        b.page.fixed_texts.push(FixedText {
            owner: id("cover"),
            rect: r,
        });
    }
    let q = measure(&b.page);
    assert!(q.counts.label_clashes >= 1, "{:?}", q.findings);
    assert!(q.findings.texts_on_texts.iter().any(|(l, _)| *l == tag.id));
}

/// C8 (FT-1909): a mark's wire that bends to the junction its trunk goes on from, the trunk along the mark's axis but
/// off its line, costs [`MARK_JOG`] steps, more than the length and bends a trunk saves by leaving the line; the trunk
/// on the mark's line costs nothing more, and a mark's wire that bends into a pin no other wire ends at is no jog.
#[test]
fn a_marks_wire_that_jogs_off_its_line_to_its_trunk() {
    let page = |dy: Tick| {
        let mut b = Page::new(80, 40);
        b.mark_in("M", p(4, 10));
        let (j, k) = (b.junction("j", p(20, 10 + dy)), b.junction("k", p(40, 10 + dy)));
        let via: &[ew_drafting::Pt] = if dy == 0 { &[] } else { &[p(20, 10)] };
        b.run("m", b.pin("M", "1"), j, via);
        b.run("trunk", j, k, &[]);
        b
    };
    let (on, off) = (measure(&page(0).page), measure(&page(2).page));
    assert_eq!(on.findings.mark_jogs, Vec::new());
    assert_eq!(off.findings.mark_jogs, vec![ew_drafting::WireId(id("m"))]);
    let step = i64::from(ew_drafting::consts::TICKS_PER_STEP);
    let jog = i64::from(s(2));
    assert_eq!(
        off.cost - on.cost,
        jog + ew_drafting::consts::BEND * step + MARK_JOG * step,
        "two steps more, a bend and the jog"
    );
    // A mark wired round a corner into a device's pin taps no trunk.
    let mut b = Page::new(80, 40);
    b.mark_in("M", p(4, 6));
    b.device("K1", p(20, 10));
    b.run("m", b.pin("M", "1"), b.pin("K1", "1"), &[p(20, 6)]);
    assert_eq!(measure(&b.page).findings.mark_jogs, Vec::new());
}

#[test]
fn qualities_compare_by_faults_then_unrouted_then_cost() {
    let good = measure(&clean().page);
    let mut b = clean();
    b.device("K3", p(30, 10));
    b.wire("u", b.pin("K2", "2"), b.pin("K3", "1"), &[]);
    let unrouted = measure(&b.page);
    assert!(good.better_than(&unrouted));
    assert!(!unrouted.better_than(&good));
}
