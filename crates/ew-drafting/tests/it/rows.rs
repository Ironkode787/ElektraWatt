// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The row rule of the text placer (FT-1870; docs/design/06a-drafting.md §5.3): like parts that stand level take one
//! place for their texts.

use crate::kit::{Page, id, p, s};
use ew_drafting::{LabelId, Pt, Settled, settle_labels};

fn tag_of(name: &str) -> LabelId {
    LabelId {
        owner: id(name),
        n: 0,
    }
}

fn find(settled: &[Settled], label: LabelId) -> Settled {
    *settled
        .iter()
        .find(|s| s.id == label)
        .expect("the label is placed")
}

/// Where each tag of `names` stands from its part's origin `at`, and its place down its list.
fn offsets(settled: &[Settled], names: &[(&str, Pt)]) -> Vec<((i32, i32), u32)> {
    names
        .iter()
        .map(|(n, at)| {
            let t = find(settled, tag_of(n));
            ((t.rect.x0 - at.x, t.rect.y0 - at.y), t.rank)
        })
        .collect()
}

/// A wire across the first place of the middle one's tag only: the row's tags all take the same other place.
#[test]
fn a_row_of_like_parts_takes_one_place_for_its_tags() {
    let mut b = Page::new(60, 40);
    let row = [("K1", p(10, 10)), ("K2", p(20, 10)), ("K3", p(30, 10))];
    for (name, at) in row {
        b.device(name, at);
    }
    let alone = find(&settle_labels(&b.page), tag_of("K2"));
    assert_eq!(alone.rank, 0);
    // A wire from below runs up through K2's tag at its first place and back down.
    let (a, c) = (b.junction("a", p(17, 30)), b.junction("c", p(18, 30)));
    let y = alone.rect.centre().y;
    b.wire(
        "w",
        a,
        c,
        &[
            p(17, 30),
            Pt::new(s(17), y),
            Pt::new(s(18), y),
            p(18, 30),
        ],
    );
    let settled = settle_labels(&b.page);
    let places = offsets(&settled, &row);
    assert!(places[1].1 > 0, "K2's tag left its first place: {places:?}");
    assert!(
        places.iter().all(|x| *x == places[0]),
        "one place for the row: {places:?}"
    );
}
