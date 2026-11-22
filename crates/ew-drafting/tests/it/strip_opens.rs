// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A strip at a narrow pitch over a card of a wide one opens as one (docs/design/06a-drafting.md §4.4, FT-2110): its
//! channels' terminals take their channels' columns, its other terminals keep the strip's order at its ends, and the
//! sensors over it stand over their terminals.

use crate::arrange::{at, run};
use crate::kit::{Page, p};
use ew_drafting::{Role, StripHeld, measure};

/// A card of four inputs at the foot, eight steps apart; over it a strip of seven terminals two steps apart: a feed
/// terminal first (from a 24 V mark at the left, on to the first sensor's supply pin), the four channels' terminals,
/// a common terminal (to a 0 V mark at the right) and a spare; four sensors over the channels' terminals, three steps
/// apart, each wired to its terminal, each terminal to its channel.
#[test]
fn a_strip_over_a_card_opens_to_the_channels_columns_in_its_own_order() {
    let mut b = Page::new(70, 50);
    let card = b.card("A1", p(10, 40), (34, 6), &[4, 12, 20, 28], true);
    b.role(card, Role::Card);
    let strip = ["X1", "X2", "X3", "X4", "X5", "X6", "X7"];
    for (k, name) in strip.iter().enumerate() {
        b.terminal(name, p(18 + 2 * k as i32, 32));
    }
    for k in 0..4 {
        let (sensor, terminal) = (format!("B{k}"), strip[k + 1]);
        b.device(&sensor, p(18 + 3 * k as i32, 20));
        b.wire(&format!("s{k}"), b.pin(&sensor, "2"), b.pin(terminal, "1"), &[]);
        b.wire(
            &format!("c{k}"),
            b.pin(terminal, "2"),
            b.pin("A1", &format!("{k}")),
            &[],
        );
    }
    b.mark_in("24V", p(4, 36));
    b.wire("feed", b.pin("24V", "1"), b.pin("X1", "2"), &[]);
    b.wire("supply", b.pin("X1", "1"), b.pin("B0", "1"), &[]);
    b.mark_out("0V", p(60, 36));
    b.wire("common", b.pin("X6", "2"), b.pin("0V", "1"), &[]);
    let (_, done) = run(&b.page, &[]);
    let q = measure(&done);
    assert_eq!((q.faults, q.unrouted), (0, 0), "{:?}", q.findings);
    // The channels' terminals in their channels' columns.
    for k in 0..4 {
        assert_eq!(
            at(&done, strip[k + 1]).x,
            p(14 + 8 * k as i32, 0).x,
            "{}",
            strip[k + 1]
        );
    }
    // One strip: every terminal on one line, in the order it was numbered.
    let line = at(&done, "X1").y;
    for pair in strip.windows(2) {
        let (a, z) = (at(&done, pair[0]), at(&done, pair[1]));
        assert_eq!(z.y, line, "{} on the strip's line", pair[1]);
        assert!(a.x < z.x, "{} before {}", pair[0], pair[1]);
    }
    // The sensors over their terminals.
    for k in 0..4 {
        assert_eq!(
            at(&done, &format!("B{k}")).x,
            at(&done, strip[k + 1]).x,
            "B{k} over its terminal"
        );
    }
}

/// The same page with the channels' terminals wired to the card's channels from the last to the first: the strip does
/// not open (its order would cross every wire), and the plan says so, naming the strip by its first terminal.
#[test]
fn a_strip_wired_against_its_order_does_not_open_and_the_plan_says_why() {
    let mut b = Page::new(70, 50);
    let card = b.card("A1", p(10, 40), (34, 6), &[4, 12, 20, 28], true);
    b.role(card, Role::Card);
    let strip = ["X1", "X2", "X3", "X4", "X5", "X6", "X7"];
    for (k, name) in strip.iter().enumerate() {
        b.terminal(name, p(18 + 2 * k as i32, 32));
    }
    for k in 0..4 {
        let (sensor, terminal) = (format!("B{k}"), strip[k + 1]);
        b.device(&sensor, p(18 + 3 * k as i32, 20));
        b.wire(&format!("s{k}"), b.pin(&sensor, "2"), b.pin(terminal, "1"), &[]);
        b.wire(
            &format!("c{k}"),
            b.pin(terminal, "2"),
            b.pin("A1", &format!("{}", 3 - k)),
            &[],
        );
    }
    b.mark_in("24V", p(4, 36));
    b.wire("feed", b.pin("24V", "1"), b.pin("X1", "2"), &[]);
    b.wire("supply", b.pin("X1", "1"), b.pin("B0", "1"), &[]);
    let a = ew_drafting::arrange(&b.page, ew_drafting::Scope::Page, &[]);
    assert_eq!(
        a.held,
        vec![(ew_drafting::PartId(crate::kit::id("X1")), StripHeld::Order)]
    );
}
