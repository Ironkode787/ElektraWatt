// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The placer opens the room the rings need (docs/design/06a-drafting.md §3.5, Rings, FT-2085): where a part wired
//! round a body stays behind it, the body steps away from it until the rings round each end of it, a step each, and a
//! step clear of the part's stubs fit between.

use crate::arrange::{at, run};
use crate::kit::{Page, p, s};
use ew_drafting::{Role, measure};

/// A card 50 steps wide, its three channels in use in its middle and facing up, and three terminals the person kept in
/// place a step under it, each wired by its upper pin to the channel over it: there is no room beside the card for it
/// to stand level with them, and between the card and their stubs none for a wire, so Route finds no way. Arrange
/// lifts the card by the rings' room (two rings round its left end, one round its right, and a step), and every wire
/// goes round it.
#[test]
fn a_card_steps_away_from_terminals_kept_behind_it_until_its_rings_fit() {
    let mut b = Page::new(80, 60);
    let card = b.card("A1", p(15, 20), (50, 6), &[20, 24, 28], true);
    b.role(card, Role::Card);
    for k in 0..3 {
        let terminal = b.terminal(&format!("X{k}"), p(35 + 4 * k, 28));
        if let Some(part) = b.page.parts.iter_mut().find(|x| x.id == terminal) {
            part.locked = true;
        }
        b.wire(
            &format!("w{k}"),
            b.pin("A1", &format!("{k}")),
            b.pin(&format!("X{k}"), "1"),
            &[],
        );
    }
    let (a, done) = run(&b.page, &[]);
    assert_eq!(
        measure(&done).unrouted,
        0,
        "every wire round the card: {:?}",
        a.moves
    );
    // The terminals' upper pins stand on line 27: the card's lower edge on line 23 or above (two rings, a step and
    // the stubs between).
    assert!(
        at(&done, "A1").y + s(6) <= s(23),
        "the card lifted: {:?}",
        at(&done, "A1")
    );
    for k in 0..3 {
        assert_eq!(
            at(&done, &format!("X{k}")),
            p(35 + 4 * k, 28),
            "terminal {k} kept"
        );
    }
}
