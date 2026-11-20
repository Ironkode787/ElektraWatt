// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The text placer (docs/design/06a-drafting.md §5) on small pages.

use crate::kit::{Page, TAG, id, p, s};
use ew_drafting::consts::LABEL_CLEAR;
use ew_drafting::{
    DraftPage, FixedText, LabelId, LabelKind, Owner, PartId, Places, Pt, Rect, Settled, settle_labels,
};

fn find(settled: &[Settled], label: LabelId) -> Settled {
    *settled
        .iter()
        .find(|s| s.id == label)
        .expect("the label is placed")
}

fn tag_of(name: &str) -> LabelId {
    LabelId {
        owner: id(name),
        n: 0,
    }
}

/// Whether no two settled labels come nearer than [`LABEL_CLEAR`].
fn apart(settled: &[Settled]) -> bool {
    settled.iter().enumerate().all(|(i, a)| {
        settled[i + 1..]
            .iter()
            .all(|b| !a.rect.grown(LABEL_CLEAR).overlaps(b.rect))
    })
}

/// Two parts side by side whose tags' first places overlap: both tags stand clear.
#[test]
fn two_tags_whose_first_places_overlap_both_stand_clear() {
    let mut b = Page::new(40, 30);
    b.device("K1", p(10, 10));
    b.device("K2", p(13, 10));
    // K2's tag at the upper left of K2 would stand on K1's body.
    let settled = settle_labels(&b.page);
    let (k1, k2) = (find(&settled, tag_of("K1")), find(&settled, tag_of("K2")));
    assert!(apart(&settled), "{settled:?}");
    let bodies: Vec<Rect> = b.page.parts.iter().map(|x| x.bounds()).collect();
    for t in [k1, k2] {
        assert!(!bodies.iter().any(|r| r.overlaps(t.rect)), "{t:?} on a body");
    }
    assert_eq!(k1.rank, 0, "the first keeps its place");
    assert!(k2.rank > 0, "the second steps aside");
}

/// A wire across a tag's first place: the tag stands at its second.
#[test]
fn a_wire_across_a_tags_first_place_sends_it_to_its_second() {
    let mut b = Page::new(40, 30);
    b.device("K1", p(10, 10));
    let alone = find(&settle_labels(&b.page), tag_of("K1"));
    assert_eq!(alone.rank, 0);
    // A wire from elsewhere runs through the upper left corner's place.
    let (a, c) = (b.junction("a", p(2, 10)), b.junction("c", p(2, 20)));
    let y = alone.rect.centre().y;
    b.wire(
        "w",
        a,
        c,
        &[
            p(2, 10),
            Pt::new(s(8), s(10)),
            Pt::new(s(8), y),
            Pt::new(s(8), s(20)),
            p(2, 20),
        ],
    );
    let moved = find(&settle_labels(&b.page), tag_of("K1"));
    assert_eq!(moved.rank, 1, "the upper right: {moved:?}");
    assert!(moved.rect.x0 > s(11), "right of the body: {moved:?}");
}

/// A label leaves its first place only when something stands there: a wire moved elsewhere moves no label.
#[test]
fn moving_an_unrelated_wire_moves_no_label() {
    let mut b = Page::new(60, 40);
    b.device("K1", p(10, 10));
    b.device("K2", p(30, 10));
    let (a, c) = (b.junction("a", p(40, 30)), b.junction("c", p(50, 30)));
    b.wire("w", a, c, &[p(40, 30), p(50, 30)]);
    let before = settle_labels(&b.page);
    for w in &mut b.page.wires {
        w.route = vec![p(40, 30), p(40, 34), p(50, 34), p(50, 30)];
    }
    assert_eq!(settle_labels(&b.page), before);
}

/// The result is the same whatever the order of the page's lists.
#[test]
fn the_result_does_not_depend_on_the_order_of_the_lists() {
    let mut b = Page::new(40, 30);
    for (i, x) in [10, 13, 16, 19].into_iter().enumerate() {
        b.device(&format!("K{i}"), p(x, 10));
    }
    let (a, c) = (b.junction("a", p(2, 12)), b.junction("c", p(30, 12)));
    b.wire("w", a, c, &[p(2, 12), p(30, 12)]);
    let forward = settle_labels(&b.page);
    let mut reversed: DraftPage = b.page.clone();
    reversed.labels.reverse();
    reversed.parts.reverse();
    reversed.wires.reverse();
    reversed.junctions.reverse();
    assert_eq!(settle_labels(&reversed), forward);
}

/// A text never stands on a pin text of a drawn box, even where its first place lies on one.
#[test]
fn a_text_never_stands_on_a_pin_text_of_a_box() {
    let mut b = Page::new(60, 40);
    let card = b.card("A1", p(10, 20), (20, 6), &[2, 7, 12, 17], true);
    let pin_texts: Vec<Rect> = b.page.fixed_texts.iter().map(|t| t.rect).collect();
    // A data text whose own place is inside the box, over the first pin text.
    let data = b.label(
        Owner::Part(card),
        3,
        LabelKind::BoxData,
        (40, 10),
        Places::At(vec![Pt::new(s(2) - 10, 4)]),
    );
    let settled = settle_labels(&b.page);
    let d = find(&settled, data);
    assert!(d.rank > 0);
    for t in &settled {
        assert!(
            !pin_texts.iter().any(|r| r.overlaps(t.rect)),
            "{t:?} on a pin text"
        );
    }
}

/// The block rule: a symbol's tag and the data under it move together, line under line.
#[test]
fn the_texts_of_one_side_move_as_a_block() {
    let mut b = Page::new(40, 30);
    let k = b.part(
        "K1",
        p(10, 10),
        (p(-1, 0), p(1, 4)),
        &[
            ("1", p(0, 0), ew_drafting::Side::Up),
            ("2", p(0, 4), ew_drafting::Side::Down),
        ],
    );
    let left = -s(1) - LABEL_CLEAR - TAG.0;
    let tag = b.label(
        Owner::Part(k),
        0,
        LabelKind::Tag,
        TAG,
        Places::At(vec![Pt::new(left, 0)]),
    );
    let data = b.label(
        Owner::Part(k),
        1,
        LabelKind::Data,
        TAG,
        Places::At(vec![Pt::new(left, 12)]),
    );
    let before = settle_labels(&b.page);
    let offset = |set: &[Settled]| {
        find(set, data)
            .rect
            .top_left()
            .minus(find(set, tag).rect.top_left())
    };
    // Something on the left of the symbol sends the whole block away.
    b.page.fixed_texts.push(FixedText {
        owner: id("note"),
        rect: Rect::new(p(6, 9), p(8, 11)),
    });
    let after = settle_labels(&b.page);
    assert_ne!(find(&after, tag).rect, find(&before, tag).rect);
    assert_eq!(offset(&after), offset(&before), "line under line: {after:?}");
    assert_eq!(find(&after, tag).rank, find(&after, data).rank);
}

/// A block of two lines (a tag and its data) whose places beside the symbol are all taken stays by its symbol: its
/// place "one line lower" is one line past the place under the body, not the block's height, so the block never stands
/// further from its part than one line and the gaps.
#[test]
fn a_block_of_two_lines_stays_within_one_line_of_its_part() {
    let mut b = Page::new(40, 30);
    let k = b.part(
        "K1",
        p(10, 10),
        (p(-1, 0), p(1, 4)),
        &[
            ("1", p(0, 0), ew_drafting::Side::Up),
            ("2", p(0, 4), ew_drafting::Side::Down),
        ],
    );
    let left = -s(1) - LABEL_CLEAR - TAG.0;
    let tag = b.label(
        Owner::Part(k),
        0,
        LabelKind::Tag,
        TAG,
        Places::At(vec![Pt::new(left, 0)]),
    );
    b.label(
        Owner::Part(k),
        1,
        LabelKind::Data,
        TAG,
        Places::At(vec![Pt::new(left, 12)]),
    );
    // Everything round the symbol down to a step under it is taken.
    b.page.fixed_texts.push(FixedText {
        owner: id("note"),
        rect: Rect::new(p(4, 4), p(16, 15)),
    });
    let r = find(&settle_labels(&b.page), tag).rect;
    let body = Rect::new(p(9, 10), p(11, 14));
    let gap = (body.x0 - r.x1)
        .max(r.x0 - body.x1)
        .max(body.y0 - r.y1)
        .max(r.y0 - body.y1);
    assert!(
        gap <= 2 * LABEL_CLEAR + TAG.1,
        "{r:?} is {gap} ticks from its part"
    );
}

/// The two-rows rule: tags of a row wider than its pitch stand in two rows, every second one a line lower.
#[test]
fn tags_wider_than_their_rows_pitch_stand_in_two_rows() {
    let mut b = Page::new(60, 30);
    let xs = [10, 12, 14, 16, 18];
    for (i, x) in xs.into_iter().enumerate() {
        // Lamps of a row two steps apart, their pins sideways, each with a tag wider than the pitch under it.
        let t = b.part(
            &format!("X{i}"),
            p(x, 10),
            (Pt::new(-6, -6), Pt::new(6, 6)),
            &[
                ("1", Pt::new(-6, 0), ew_drafting::Side::Left),
                ("2", Pt::new(6, 0), ew_drafting::Side::Right),
            ],
        );
        b.label(
            Owner::Part(t),
            0,
            LabelKind::Tag,
            (40, 10),
            Places::At(vec![Pt::new(-20, 6 + LABEL_CLEAR)]),
        );
    }
    let settled = settle_labels(&b.page);
    assert!(apart(&settled), "{settled:?}");
    let ys: Vec<i32> = (0..xs.len())
        .map(|i| {
            find(
                &settled,
                LabelId {
                    owner: id(&format!("X{i}")),
                    n: 0,
                },
            )
            .rect
            .y0
        })
        .collect();
    assert_eq!(ys[0], ys[2]);
    assert_eq!(ys[2], ys[4]);
    assert_eq!(ys[1], ys[3]);
    assert!(ys[1] > ys[0], "{ys:?}");
}

/// A wire's number stands along its wire: above a horizontal stretch, right of a vertical one and turned.
#[test]
fn a_wire_number_stands_along_its_wire() {
    let mut b = Page::new(60, 40);
    let (a, c, d) = (
        b.junction("a", p(5, 5)),
        b.junction("c", p(40, 5)),
        b.junction("d", p(40, 35)),
    );
    let flat = b.wire("flat", a, c, &[p(5, 5), p(40, 5)]);
    let tall = b.wire("tall", c, d, &[p(40, 5), p(40, 35)]);
    for w in [flat, tall] {
        b.label(
            Owner::Wire(w),
            0,
            LabelKind::WireNumber,
            (30, 10),
            Places::AlongWire,
        );
    }
    let settled = settle_labels(&b.page);
    let f = find(&settled, LabelId { owner: flat.0, n: 0 });
    let t = find(&settled, LabelId { owner: tall.0, n: 0 });
    assert!(
        !f.turned && f.rect.y1 < s(5) && f.rect.x0 > s(5) && f.rect.x1 < s(40),
        "{f:?}"
    );
    assert!(
        t.turned && t.rect.x0 > s(40) && t.rect.size() == (10, 30),
        "{t:?}"
    );
}

/// A label of a part that is no longer on the page is left out, and nothing panics.
#[test]
fn a_label_without_its_owner_is_left_out() {
    let mut b = Page::new(20, 20);
    let ghost = PartId(id("gone"));
    b.label(
        Owner::Part(ghost),
        0,
        LabelKind::Tag,
        TAG,
        Places::At(vec![Pt::new(0, 0)]),
    );
    assert!(settle_labels(&b.page).is_empty());
}

/// A cable mark across vertical wires: its type before the line, its tag after it.
#[test]
fn a_cable_marks_type_stands_before_its_line_and_its_tag_after_it() {
    let mut b = Page::new(60, 40);
    let (a, c, d, e) = (
        b.junction("a", p(20, 5)),
        b.junction("c", p(20, 35)),
        b.junction("d", p(22, 5)),
        b.junction("e", p(22, 35)),
    );
    let w1 = b.wire("w1", a, c, &[p(20, 5), p(20, 35)]);
    let w2 = b.wire("w2", d, e, &[p(22, 5), p(22, 35)]);
    let m = b.mark(
        "W1",
        Rect::new(Pt::new(s(19), s(20)), Pt::new(s(23), s(20))),
        true,
        &[w1, w2],
    );
    let tag = b.label(
        Owner::Mark(m),
        0,
        LabelKind::CableTag,
        TAG,
        Places::At(Vec::new()),
    );
    let kind = b.label(
        Owner::Mark(m),
        1,
        LabelKind::CableType,
        (40, 8),
        Places::At(Vec::new()),
    );
    let settled = settle_labels(&b.page);
    let (t, k) = (find(&settled, tag), find(&settled, kind));
    assert!(k.rect.x1 < s(19) && t.rect.x0 > s(23), "{k:?} {t:?}");
    assert_eq!((t.rank, k.rank), (0, 0));
}

/// A tag whose author's place leaves the drawing area is moved in to stand clear of the frame's line, as a text beside
/// a line does, not flush against it.
#[test]
fn a_tag_moved_into_the_drawing_area_keeps_clear_of_its_line() {
    let mut b = Page::new(40, 30);
    let k1 = b.device("K1", p(4, 10));
    let tag = b.label(
        Owner::Part(k1),
        0,
        LabelKind::Tag,
        TAG,
        Places::At(vec![Pt::new(s(-6), 0)]),
    );
    let t = find(&settle_labels(&b.page), tag);
    assert_eq!((t.rank, t.rect.x0), (1, LABEL_CLEAR), "{t:?}");
}

/// The stacked-marks rule (§5.3): of marks set two steps apart, the name its author put above the arrow would stand
/// on the mark above it; it stands in line with its wire beyond its arrow, and nothing clashes. A mark that stands
/// alone keeps its name where its author put it.
#[test]
fn the_name_of_a_mark_stacked_under_another_stands_in_line_beyond_its_arrow() {
    let mut b = Page::new(40, 30);
    let size = (30, 24);
    let above = Pt::new(-s(2), -s(1) / 2 - LABEL_CLEAR - size.1);
    let mut named = |mark: &str, at: Pt| {
        let part = b.mark_in(mark, at);
        b.label(
            Owner::Part(part),
            0,
            LabelKind::MarkName,
            size,
            Places::At(vec![above]),
        )
    };
    let (_, under, alone) = (
        named("m0", p(10, 10)),
        named("m1", p(10, 12)),
        named("m2", p(10, 22)),
    );
    let settled = settle_labels(&b.page);
    assert!(apart(&settled), "{settled:?}");
    let bodies: Vec<Rect> = b.page.parts.iter().map(|x| x.bounds()).collect();
    for t in &settled {
        assert!(!bodies.iter().any(|r| r.overlaps(t.rect)), "{t:?} on an arrow");
    }
    // In line: beyond the arrow's tail (the arrow lies from 8 to 10 steps), the wire's line through its height.
    let r = find(&settled, under).rect;
    assert!(r.x1 <= s(8) && r.y0 < s(12) && s(12) < r.y1, "{r:?}");
    assert_eq!(find(&settled, alone).rect.top_left(), p(10, 22).plus(above));
}
