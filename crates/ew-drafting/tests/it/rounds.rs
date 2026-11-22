// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Wires round a body laid as nested rings (docs/design/06a-drafting.md §3.5, "Rings").

use crate::kit::{Page, p, s};
use crate::route::routed;
use ew_drafting::{Role, geom};

/// A card in the older form at the head, its six pins up; under it a strip of six terminals, each wired by its upper
/// pin to the card's pin over it: every wire has to go round the card. Route lays each end's three as nested rings:
/// ring `r` runs `r` steps over the card's edge, beside its end and under it, the pin nearest the corner innermost.
#[test]
fn the_wires_round_a_card_are_laid_as_nested_rings() {
    let mut b = Page::new(80, 60);
    let xs: Vec<i32> = (0..6).map(|k| 2 + 3 * k).collect();
    let card = b.card("A1", p(30, 20), (19, 6), &xs, true);
    b.role(card, Role::Card);
    for (k, x) in xs.iter().enumerate() {
        let t = format!("X{k}");
        b.terminal(&t, p(30 + x, 40));
        b.wire(
            &format!("c{k}"),
            b.pin("A1", &format!("{k}")),
            b.pin(&t, "1"),
            &[],
        );
    }
    let done = routed(&b.page);
    let (top, bottom, left, right) = (s(20), s(26), s(30), s(49));
    for (k, x) in xs.iter().enumerate() {
        let w = done
            .wires
            .iter()
            .find(|w| w.id.0 == crate::kit::id(&format!("c{k}")))
            .expect("a wire");
        let route = geom::clean(&w.route);
        // The three nearest the left end pass it, the others the right end; the nearest a corner innermost.
        let (ring, column) = if k < 3 {
            (k as i32 + 1, left - s(k as i32 + 1))
        } else {
            (6 - k as i32, right + s(6 - k as i32))
        };
        let pin = s(30 + x);
        assert_eq!(
            route,
            [
                crate::kit::p(30 + x, 20),
                ew_drafting::Pt::new(pin, top - s(ring)),
                ew_drafting::Pt::new(column, top - s(ring)),
                ew_drafting::Pt::new(column, bottom + s(ring)),
                ew_drafting::Pt::new(pin, bottom + s(ring)),
                crate::kit::p(30 + x, 39),
            ],
            "wire {k} on ring {ring}"
        );
    }
}
