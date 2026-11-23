// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A strip's designation clear of the wire into its terminal (docs/design/06a-drafting.md §5.2).

use crate::kit::{Page, TAG, p};
use ew_drafting::{LabelKind, Owner, Places, TextRef, measure, settle_labels};

/// A terminal whose wire comes up along its left and over its top into its upper pin, as a box's PE wire does when the
/// box stands under its breaker to the left of the strip: every place left of and above the terminal lies on that
/// wire, so the designation stands below or right of it, and no wire runs through it.
#[test]
fn a_strip_designation_stands_clear_of_a_wire_round_the_terminal_s_left_and_top() {
    let mut b = Page::new(60, 40);
    let x = b.terminal("X11", p(20, 20));
    let tag = b.label(Owner::Part(x), 1, LabelKind::StripTag, TAG, Places::At(Vec::new()));
    b.device("G1", p(18, 26));
    b.run("pe", b.pin("G1", "1"), b.pin("X11", "1"), &[p(18, 17), p(20, 17)]);
    let settled = settle_labels(&b.page);
    let at = settled.iter().find(|l| l.id == tag).expect("the designation").rect;
    let q = measure(&b.page);
    assert!(
        !q.findings.across_texts.iter().any(|x| x.1 == TextRef::Label(tag)),
        "the designation at {at:?} lies on the wire: {:?}",
        q.findings.across_texts
    );
}
