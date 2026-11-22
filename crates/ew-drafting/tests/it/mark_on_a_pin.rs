// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A mark set on a symbol's pin (docs/design/06a-drafting.md §5.2): its cross-reference stands past the pin's number
//! where its places beside the wire and beyond its arrow are taken, not on a text.

use crate::kit::{Page, id, p};
use ew_drafting::consts::LABEL_CLEAR;
use ew_drafting::{FixedText, LabelId, LabelKind, Owner, PinId, Places, Pt, Rect, Side, settle_labels};

/// A contact whose upper pin's number stands right of the pin, its texts dragged left of its body, a wire into that pin
/// from above, and an incoming mark set on the pin: its arrow left of the pin, its name over the pin beside the wire
/// (where its author set it), its cross-reference (as wide as "101-C7") at the places of its kind. Under the wire
/// past the pin stands the number, over it the name, beyond the arrow the contact's texts.
#[test]
fn a_marks_cross_reference_stands_past_the_number_of_the_pin_it_is_set_on() {
    let mut b = Page::new(60, 40);
    // A contact: its blade swings left of its pins' line.
    let s2 = b.part(
        "S2",
        p(20, 12),
        (Pt::new(-8, 0), Pt::new(2, 64)),
        &[("1", p(0, 0), Side::Up), ("2", p(0, 4), Side::Down)],
    );
    let number = b.label(
        Owner::Part(s2),
        1,
        LabelKind::PinNumber(PinId(id("S2:1"))),
        (10, 10),
        Places::At(vec![Pt::new(LABEL_CLEAR, 0)]),
    );
    // Its tag and data where the person dragged them, left of the mark's arrow.
    let texts = Rect::new(Pt::new(238, 160), Pt::new(287, 226));
    b.page.fixed_texts.push(FixedText {
        owner: s2.0,
        rect: texts,
    });
    b.device("K1", p(20, 2));
    let (top, into) = (b.pin("K1", "2"), b.pin("S2", "1"));
    let (a, z) = (b.at(top), b.at(into));
    b.wire("k1", top, into, &[a, z]);
    let mark = b.mark_in("24V", p(20, 12));
    let name = b.label(
        Owner::Part(mark),
        0,
        LabelKind::MarkName,
        (26, 21),
        Places::At(vec![Pt::new(LABEL_CLEAR, -LABEL_CLEAR - 21)]),
    );
    let xref = b.label(Owner::Part(mark), 1, LabelKind::MarkXref, (47, 20), Places::At(Vec::new()));
    let settled = settle_labels(&b.page);
    let at = |l: LabelId| -> Rect { settled.iter().find(|s| s.id == l).expect("placed").rect };
    let x = at(xref);
    // Clear of every other text by LABEL_CLEAR (§1.3), past the pin's number.
    for other in [at(number), at(name), texts] {
        assert!(!x.grown(LABEL_CLEAR).overlaps(other), "the cross-reference {x:?} at {other:?}");
    }
    assert!(x.x0 > at(number).x1, "past the pin's number: {x:?}");
}
