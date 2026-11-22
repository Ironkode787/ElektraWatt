// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Two cards whose channels face each other (docs/design/06a-drafting.md §4.4, a card steps along its edge): where the
//! columns of their channels in use interleave, the card with fewer in use steps aside.

use crate::arrange::{at, corners, run, sound};
use crate::kit::{Page, p, s};
use ew_drafting::{Role, measure};

/// A strip in one row between a card of outputs at the head and a card of inputs at the foot, both of one pitch and
/// one place along: three inputs wired through the strip's first terminals to sensors, the output's second channel,
/// in the column of the second input, through its last terminal to a lamp. The card of outputs, with one channel in
/// use, steps right until that channel stands two steps right of the inputs': every terminal stands in its channel's
/// column, the strip in its order, and no wire bends or crosses.
#[test]
fn a_card_whose_used_column_falls_among_another_card_s_steps_aside() {
    let mut b = Page::new(100, 62);
    let out = b.card("A2", p(10, 6), (26, 6), &[5, 13, 21], false);
    b.role(out, Role::Card);
    let inputs = b.card("A1", p(10, 50), (26, 6), &[5, 13, 21], true);
    b.role(inputs, Role::Card);
    for k in 0..5 {
        b.terminal(&format!("X{k}"), p(60 + 2 * k, 30));
    }
    for k in 0..3 {
        let sensor = format!("B{k}");
        b.device(&sensor, p(60 + 9 * k, 36));
        b.wire(&format!("s{k}"), b.pin(&sensor, "2"), b.pin(&format!("X{k}"), "1"), &[]);
        b.wire(
            &format!("c{k}"),
            b.pin(&format!("X{k}"), "2"),
            b.pin("A1", &format!("{k}")),
            &[],
        );
    }
    b.device("E", p(72, 42));
    b.wire("q", b.pin("A2", "1"), b.pin("X4", "1"), &[]);
    b.wire("e", b.pin("X4", "2"), b.pin("E", "1"), &[]);
    let (_, done) = run(&b.page, &[]);
    assert_eq!(sound(&done), (0, 0, 0), "{:?}", measure(&done).findings);
    assert_eq!(at(&done, "A1"), p(10, 50), "the card with more channels in use stays");
    for k in 0..3 {
        assert_eq!(at(&done, &format!("X{k}")).x, s(15 + 8 * k), "input {k}'s column");
    }
    let output = at(&done, "A2").x + s(13);
    assert_eq!((at(&done, "X4").x, at(&done, "E").x), (output, output));
    assert!(output >= at(&done, "X2").x + s(2), "the output's column right of the inputs'");
    for w in ["c0", "c1", "c2", "q", "e"] {
        assert_eq!(corners(&done, w), 0, "{w}");
    }
}
