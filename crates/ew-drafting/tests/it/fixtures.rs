// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fixtures of docs/design/06a-drafting.md §9.1 exist in both states; as wired by hand they have no fault and
//! no text on a text.

use crate::kit::fixtures;
use ew_drafting::measure;

#[test]
fn every_fixture_as_wired_by_hand_has_no_fault_and_no_text_on_a_text() {
    for f in fixtures() {
        let q = measure(&f.hand);
        assert_eq!(q.faults, 0, "{}: {:?}", f.name, q.findings);
        assert_eq!(q.unrouted, 0, "{}", f.name);
        assert!(
            q.findings.texts_on_texts.is_empty(),
            "{}: {:?}",
            f.name,
            q.findings.texts_on_texts
        );
        assert!(
            q.findings.texts_on_parts.is_empty(),
            "{}: {:?}",
            f.name,
            q.findings.texts_on_parts
        );
        assert!(
            q.findings.across_texts.is_empty(),
            "{}: {:?}",
            f.name,
            q.findings.across_texts
        );
        assert_eq!(
            q.labels.len(),
            f.hand.labels.len(),
            "{}: every label has its place",
            f.name
        );
    }
}

#[test]
fn every_fixture_untidy_has_its_parts_shifted_two_on_the_wrong_side_and_no_route() {
    for f in fixtures() {
        let untidy = f.untidy();
        let q = measure(&untidy);
        assert_eq!(q.unrouted, f.hand.wires.len(), "{}: every wire unrouted", f.name);
        let moved = |id| {
            let (a, b) = (
                f.hand.parts.iter().find(|p| p.id == id).map(|p| p.at),
                untidy.parts.iter().find(|p| p.id == id).map(|p| p.at),
            );
            a.is_some() && a != b
        };
        assert!(f.wrong_side.iter().all(|(id, _)| moved(*id)), "{}", f.name);
        assert!(
            f.hand.parts.iter().filter(|p| moved(p.id)).count() > f.hand.parts.len() / 2,
            "{}",
            f.name
        );
    }
}
