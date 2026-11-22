// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A drawn box's caption clear of the ways out of its wired pins (docs/design/06a-drafting.md §5.2).

use crate::kit::{Page, TAG, p};
use ew_drafting::{LabelKind, Owner, Places, settle_labels};

/// A box at the left edge of the drawing area, so that its caption has no room above its left corner; its first top
/// pin, a step from its left edge, is wired to a device above it. The caption stands at a place clear of that pin's
/// way (above the box's right corner), not over the pin, where the wire into it would have to go round it.
#[test]
fn a_box_s_caption_stands_clear_of_the_way_out_of_a_wired_top_pin() {
    let mut b = Page::new(60, 40);
    let part = b.part(
        "U1",
        p(1, 24),
        (p(0, 0), p(12, 6)),
        &[
            ("0", p(1, 0), ew_drafting::Side::Up),
            ("1", p(6, 0), ew_drafting::Side::Up),
        ],
    );
    let tag = b.label(
        Owner::Part(part),
        0,
        LabelKind::BoxTag,
        TAG,
        Places::At(Vec::new()),
    );
    b.device("D", p(2, 10));
    b.wire("w", b.pin("D", "2"), b.pin("U1", "0"), &[]);
    let settled = settle_labels(&b.page);
    let at = settled.iter().find(|x| x.id == tag).expect("the caption").rect;
    let pin = p(2, 24);
    assert!(
        !(at.y1 <= pin.y && at.x0 <= pin.x && pin.x <= at.x1),
        "the caption {at:?} stands over the wired pin at {pin:?}"
    );
    assert!(at.x0 >= p(13, 0).x, "above the box's right corner: {at:?}");
}
