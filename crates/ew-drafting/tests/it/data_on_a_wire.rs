// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A symbol's data text that its author set on the line of its own pin (docs/design/06a-drafting.md §5.4, FT-1980):
//! the text placer moves it off the wire that leaves the pin, as it moves every label a wire runs through.

use crate::kit::{Page, p, s};
use ew_drafting::{LabelKind, Owner, Places, Pt, measure, settle_labels};

/// A coil whose make and type its author set right above its upper pin, where the coil's own wire leaves upward: the
/// text stands clear of the wire, and no wire runs across a text.
#[test]
fn a_data_text_set_on_its_own_pin_s_line_gives_way_to_the_pin_s_wire() {
    let mut b = Page::new(30, 30);
    let coil = b.device("K1", p(10, 14));
    b.device("S1", p(10, 4));
    b.run("w", b.pin("S1", "2"), b.pin("K1", "1"), &[]);
    // A box three steps wide centred on the pin's line, a step above the pin.
    let data = b.label(
        Owner::Part(coil),
        1,
        LabelKind::Data,
        (s(3), 10),
        Places::At(vec![Pt::new(-s(3) / 2, -s(1) - 10)]),
    );
    let settled = settle_labels(&b.page);
    let placed = settled.iter().find(|x| x.id == data).expect("the data text is placed");
    assert!(placed.rank > 0, "not at its author's place: {placed:?}");
    let q = measure(&b.page);
    assert!(q.findings.across_texts.is_empty(), "{:?}", q.findings.across_texts);
}
