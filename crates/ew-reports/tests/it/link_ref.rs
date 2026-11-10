// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `LinkRef::item`: a link over one listed item of a cell, never one whose column or item
//! index does not fit.

use ew_derive::ew_ids::{Id, Uuid};
use ew_reports::{LinkRef, LinkTarget};

fn target() -> LinkTarget {
    LinkTarget::MainRep(Id::from_uuid(Uuid::from_u128(0x101)))
}

/// The largest indices that fit make their link; one more, in the item or in the column, makes none (a saturated
/// index would draw every later page over the span of page 65 535).
#[test]
fn a_link_whose_index_does_not_fit_is_not_made() {
    let max = usize::from(u16::MAX);
    assert_eq!(
        LinkRef::item(7, max, target()),
        Some(LinkRef {
            column: 7,
            item: Some(u16::MAX),
            target: target(),
        })
    );
    assert_eq!(
        LinkRef::item(max, 0, target()),
        Some(LinkRef {
            column: u16::MAX,
            item: Some(0),
            target: target(),
        })
    );
    assert_eq!(LinkRef::item(7, max + 1, target()), None);
    assert_eq!(LinkRef::item(max + 1, 0, target()), None);
}
