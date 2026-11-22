// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A strip's feed terminal on a page with cards (docs/design/06a-drafting.md §4.4, §4.8, rule E4): a terminal wired to
//! no channel keeps its place in the strip's order, between the channels' terminals, which take their columns.

use crate::arrange::{at, run};
use crate::kit::{Page, p, s};
use ew_drafting::{Role, measure};

/// A card of four inputs four steps apart at the foot, a strip of five terminals over it six steps apart, the third of
/// them the feed terminal (wired from a 24 V mark at the left to the first sensor's supply pin, the sensors' supply
/// chained from there), the others each wired to a channel, with a sensor over each.
#[test]
fn a_feed_terminal_among_the_channels_terminals_keeps_its_place_in_the_strip() {
    let mut b = Page::new(100, 60);
    let card = b.card("A1", p(30, 48), (16, 6), &[2, 6, 10, 14], true);
    b.role(card, Role::Card);
    b.mark_in("L+", p(4, 40));
    let mut k = 0;
    for slot in 0..5 {
        let x = 12 + 6 * slot;
        if slot == 2 {
            b.terminal("X0", p(x, 33));
            b.wire("feed", b.pin("L+", "1"), b.pin("X0", "2"), &[]);
            continue;
        }
        let (terminal, sensor) = (format!("X{}", k + 1), format!("B{k}"));
        b.terminal(&terminal, p(x, 33));
        b.device(&sensor, p(10 + 9 * k, 14));
        b.wire(&format!("s{k}"), b.pin(&sensor, "2"), b.pin(&terminal, "1"), &[]);
        b.wire(
            &format!("c{k}"),
            b.pin(&terminal, "2"),
            b.pin("A1", &format!("{k}")),
            &[],
        );
        k += 1;
    }
    b.wire("p0", b.pin("X0", "1"), b.pin("B0", "1"), &[]);
    for k in 1..4 {
        b.wire(
            &format!("p{k}"),
            b.pin(&format!("B{}", k - 1), "1"),
            b.pin(&format!("B{k}"), "1"),
            &[],
        );
    }
    let (_, done) = run(&b.page, &[]);
    let q = measure(&done);
    assert_eq!((q.faults, q.unrouted), (0, 0), "{:?}", q.findings);
    for k in 0..4 {
        assert_eq!(
            at(&done, &format!("X{}", k + 1)).x,
            s(32 + 4 * k),
            "channel {k}'s column"
        );
    }
    // In its place in the order, halfway between the terminals of channels 1 and 2.
    assert_eq!(at(&done, "X0").x, s(38), "the feed terminal between X2 and X3");
    assert_eq!(at(&done, "X0").y, at(&done, "X1").y, "on the strip's line");
}
