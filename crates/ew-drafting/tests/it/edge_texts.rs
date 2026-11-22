// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A symbol placed near the drawing area's edge (docs/design/06a-drafting.md §5.3): its texts whose places the author set
//! left of it, where they leave the area, are not moved in onto the symbol and its pins' numbers.

use crate::kit::{Page, id, p};
use ew_drafting::consts::LABEL_CLEAR;
use ew_drafting::{LabelId, LabelKind, Owner, PartId, PinId, Places, Pt, Side, settle_labels};

/// A switch of three poles two steps apart, its first pole three steps from the area's left edge, with each pin's
/// number right of its pin and its tag where the author set it: beside its upper pins, right-aligned half a step left
/// of its body, close to three steps wide, so that it leaves the area. Placed just now, with no wire.
#[test]
fn a_tag_that_leaves_the_area_is_not_moved_in_onto_its_own_symbol() {
    let mut b = Page::new(60, 40);
    let at = p(3, 10);
    let mut pins = Vec::new();
    for (k, x) in [0, 2, 4].into_iter().enumerate() {
        pins.push((format!("{}", 2 * k + 1), p(x, 0), Side::Up));
        pins.push((format!("{}", 2 * k + 2), p(x, 4), Side::Down));
    }
    let named: Vec<(&str, Pt, Side)> = pins.iter().map(|(n, at, s)| (n.as_str(), *at, *s)).collect();
    // The body reaches half a step left of the first pole (its blade).
    let q1 = b.part("Q1", at, (Pt::new(-8, 0), p(5, 4)), &named);
    for (n, (name, pin_at, _)) in named.iter().enumerate() {
        let pin = PinId(id(&format!("Q1:{name}")));
        let number = Pt::new(pin_at.x + 3, pin_at.y + 2);
        b.label(
            Owner::Part(q1),
            10 + n as u32,
            LabelKind::PinNumber(pin),
            (7, 10),
            Places::At(vec![number]),
        );
    }
    let tag = b.label(
        Owner::Part(q1),
        0,
        LabelKind::Tag,
        (44, 13),
        Places::At(vec![Pt::new(-8 - LABEL_CLEAR - 44, 3)]),
    );
    let settled = settle_labels(&b.page);
    let placed = |label: LabelId| settled.iter().find(|s| s.id == label).expect("placed").rect;
    let body = b
        .page
        .parts
        .iter()
        .find(|x| x.id == PartId(id("Q1")))
        .expect("Q1")
        .bounds();
    let t = placed(tag);
    assert!(b.page.area.holds(t), "{t:?} in the area");
    assert!(!t.overlaps(body), "{t:?} on its symbol {body:?}");
    for s in settled.iter().filter(|s| s.id != tag) {
        assert!(
            !t.grown(LABEL_CLEAR).overlaps(s.rect),
            "{t:?} on a pin's number {s:?}"
        );
    }
}
