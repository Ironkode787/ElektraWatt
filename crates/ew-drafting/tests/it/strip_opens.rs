// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A strip at a narrow pitch over a card of a wide one opens as one, in the order it stands in
//! (docs/design/06a-drafting.md §4.4): the channels' terminals that stand in the order of their channels take
//! their columns, every other terminal keeps its place in the order, and no terminal passes another.

use crate::arrange::{at, run};
use crate::kit::{Page, p};
use ew_drafting::{DraftPage, Role, Side, StripHeld, measure};

/// The terminals `names` stand on one line in that order, on `page` as `before` had them and after.
fn in_order(before: &DraftPage, after: &DraftPage, names: &[&str]) {
    for page in [before, after] {
        let line = at(page, names[0]).y;
        for pair in names.windows(2) {
            let (a, z) = (at(page, pair[0]), at(page, pair[1]));
            assert_eq!(z.y, line, "{} on the strip's line", pair[1]);
            assert!(a.x < z.x, "{} before {}", pair[0], pair[1]);
        }
    }
}

/// A strip of seven terminals two steps apart over a card of four inputs eight steps apart: the channels' terminals
/// X1, X3, X4 and X6, a feed terminal X2 among them (from a 24 V mark at the left, on to the first sensor's supply
/// pin), a common terminal X5 among them (to a 0 V mark at the right) and a spare X7 last; a sensor over each
/// channel's terminal, three steps apart.
fn strip_with_a_feed_and_a_common() -> Page {
    let mut b = Page::new(70, 50);
    let card = b.card("A1", p(10, 40), (34, 6), &[4, 12, 20, 28], true);
    b.role(card, Role::Card);
    for (k, name) in ["X1", "X2", "X3", "X4", "X5", "X6", "X7"].iter().enumerate() {
        b.terminal(name, p(18 + 2 * k as i32, 32));
    }
    for (k, terminal) in ["X1", "X3", "X4", "X6"].into_iter().enumerate() {
        let sensor = format!("B{k}");
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
    b.wire("feed", b.pin("24V", "1"), b.pin("X2", "2"), &[]);
    b.wire("supply", b.pin("X2", "1"), b.pin("B0", "1"), &[]);
    b.mark_out("0V", p(60, 36));
    b.wire("common", b.pin("X5", "2"), b.pin("0V", "1"), &[]);
    b
}

#[test]
fn a_strip_over_a_card_opens_to_the_channels_columns_in_its_own_order() {
    let b = strip_with_a_feed_and_a_common();
    let (_, done) = run(&b.page, &[]);
    let q = measure(&done);
    assert_eq!((q.faults, q.unrouted), (0, 0), "{:?}", q.findings);
    in_order(&b.page, &done, &["X1", "X2", "X3", "X4", "X5", "X6", "X7"]);
    // The channels' terminals in their channels' columns, the feed and the common terminal between their neighbours.
    for (k, terminal) in ["X1", "X3", "X4", "X6"].into_iter().enumerate() {
        assert_eq!(at(&done, terminal).x, p(14 + 8 * k as i32, 0).x, "{terminal}");
        assert_eq!(
            at(&done, &format!("B{k}")).x,
            at(&done, terminal).x,
            "B{k} over its terminal"
        );
    }
    assert_eq!(
        at(&done, "X2").x,
        p(18, 0).x,
        "the feed terminal halfway between X1 and X3"
    );
    assert_eq!(
        at(&done, "X5").x,
        p(34, 0).x,
        "the common terminal halfway between X4 and X6"
    );
}

/// A card at the foot with its inputs in two rows, as a card drawn before step 7: inputs 0 to 3 on its upper edge,
/// eight steps apart, inputs 4 and 5 on its lower edge under inputs 0 and 1. Over it a strip of six terminals two
/// steps apart wired to inputs 0, 1, 4, 2, 3 and 5 in their order: the third and the last would take a column before
/// the first. The strip keeps its order: the terminals of inputs 0 to 3 take their columns, the others stand in their
/// places between and after them.
#[test]
fn a_strip_over_a_card_with_two_rows_of_pins_keeps_its_order() {
    let mut b = Page::new(70, 56);
    let pins: Vec<(String, ew_drafting::Pt, Side)> =
        [(0, 4, true), (1, 12, true), (2, 20, true), (3, 28, true)]
            .into_iter()
            .chain([(4, 4, false), (5, 12, false)])
            .map(|(n, x, up)| {
                if up {
                    (format!("{n}"), p(x, 0), Side::Up)
                } else {
                    (format!("{n}"), p(x, 6), Side::Down)
                }
            })
            .collect();
    let named: Vec<(&str, ew_drafting::Pt, Side)> =
        pins.iter().map(|(n, at, s)| (n.as_str(), *at, *s)).collect();
    let card = b.part("A1", p(10, 40), (p(0, 0), p(34, 6)), &named);
    b.role(card, Role::Card);
    let strip = ["X1", "X2", "X3", "X4", "X5", "X6"];
    for (k, name) in strip.iter().enumerate() {
        b.terminal(name, p(18 + 2 * k as i32, 32));
    }
    for (terminal, input) in strip.iter().zip(["0", "1", "4", "2", "3", "5"]) {
        b.wire(
            &format!("c{terminal}"),
            b.pin(terminal, "2"),
            b.pin("A1", input),
            &[],
        );
    }
    for (k, terminal) in strip.iter().enumerate() {
        let sensor = format!("B{k}");
        b.device(&sensor, p(18 + 3 * k as i32, 20));
        b.wire(&format!("s{k}"), b.pin(&sensor, "2"), b.pin(terminal, "1"), &[]);
    }
    let (_, done) = run(&b.page, &[]);
    in_order(&b.page, &done, &strip);
    for (terminal, k) in [("X1", 0), ("X2", 1), ("X4", 2), ("X5", 3)] {
        assert_eq!(
            at(&done, terminal).x,
            p(14 + 8 * k, 0).x,
            "{terminal} in its channel's column"
        );
    }
}

/// The same page with the channels' terminals wired to the card's channels from the last to the first: no two of them
/// stand in their channels' order, so the strip does not open, it keeps its order, and the plan says why, naming the
/// strip by its first terminal.
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
    let (a, done) = run(&b.page, &[]);
    in_order(&b.page, &done, &strip);
    assert_eq!(
        a.held,
        vec![(ew_drafting::PartId(crate::kit::id("X1")), StripHeld::Order)]
    );
}
