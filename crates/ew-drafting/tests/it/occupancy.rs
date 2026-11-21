// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The occupancy (docs/design/06a-drafting.md §3.3, `ew_drafting::occupancy`): what it holds per node and per move
//! as wires are laid and lifted, texts added and junctions moved. How the search reads it is tested through the
//! router (`route.rs`).

use crate::kit::{Page, id, p};
use ew_drafting::occupancy::{Holder, Move, Node, Occupancy, Text, node_of};
use ew_drafting::{DraftPage, End, Pt, Rect};

/// The occupancy of `page` with `texts`, no pin wired.
fn occupancy(page: &DraftPage, texts: Vec<Text>) -> Occupancy {
    Occupancy::new(page, texts, &|_| false).expect("a page of a few steps has an occupancy")
}

/// The move right (`down: false`) or down from node `n`.
fn mv(n: Node, down: bool) -> Move {
    (n, down)
}

/// A point off the half grid takes its nearest node; what stands on the edge of the drawing area is held.
#[test]
fn a_point_takes_its_nearest_node_and_the_edge_of_the_area_is_covered() {
    // Nodes are half a step (8 ticks) apart.
    assert_eq!(node_of(Pt::new(3, 0)), (0, 0));
    assert_eq!(node_of(Pt::new(5, 0)), (1, 0));
    assert_eq!(node_of(Pt::new(0, 13)), (0, 2));
    assert_eq!(node_of(Pt::new(-5, -3)), (-1, 0));
    let mut b = Page::new(10, 10);
    let End::Junction(j) = b.junction("J", p(0, 0)) else {
        unreachable!()
    };
    let occ = occupancy(&b.page, Vec::new());
    assert_eq!(occ.holders_at((0, 0)), [Holder::Junction(j)]);
}

/// A laid wire: one vertex on each of its corners, one wire along each move it runs on (and only those), one across
/// each move of the other axis whose line it passes strictly inside its extent (the move's far node included); a
/// slanted segment runs along and across nothing. Lifted, it leaves nothing.
#[test]
fn a_laid_wire_is_along_and_across_the_moves_it_passes_and_lifted_leaves_nothing() {
    let b = Page::new(12, 12);
    let mut occ = occupancy(&b.page, Vec::new());
    // Across row 4 (y = 32 ticks) from x = 16 to x = 80: nodes 2 to 10.
    let across_row = [p(1, 2), p(5, 2)];
    // Down column 16 (x = 128 ticks) from y = 16 to y = 96: nodes 2 to 12.
    let down_column = [p(8, 1), p(8, 6)];
    let slant = [p(1, 7), p(4, 9)];
    for route in [&across_row[..], &down_column, &slant] {
        occ.lay(route, true);
    }
    let held = |occ: &Occupancy| {
        (
            [
                occ.vertices_at((2, 4)),
                occ.vertices_at((10, 4)),
                occ.vertices_at((6, 4)),
                occ.vertices_at((2, 14)),
            ],
            [
                // Along the row: its first and last move, and none beyond.
                occ.along(mv((2, 4), false)),
                occ.along(mv((9, 4), false)),
                occ.along(mv((1, 4), false)),
                occ.along(mv((10, 4), false)),
                // Along the column.
                occ.along(mv((16, 2), true)),
                occ.along(mv((16, 11), true)),
                occ.along(mv((16, 1), true)),
                occ.along(mv((16, 12), true)),
                // Nothing along the slant, read either way.
                occ.along(mv((2, 14), true)),
                occ.along(mv((2, 14), false)),
            ],
            [
                // The row crosses the moves down onto its line strictly between its ends.
                occ.across(mv((3, 3), true)),
                occ.across(mv((9, 3), true)),
                occ.across(mv((3, 4), true)),
                occ.across(mv((2, 3), true)),
                occ.across(mv((10, 3), true)),
                // The column crosses the moves right onto its line strictly between its ends.
                occ.across(mv((15, 3), false)),
                occ.across(mv((15, 11), false)),
                occ.across(mv((16, 5), false)),
                occ.across(mv((15, 2), false)),
                occ.across(mv((15, 12), false)),
                // A wire runs along a move, never across it.
                occ.across(mv((3, 4), false)),
            ],
        )
    };
    assert_eq!(
        held(&occ),
        (
            [1, 1, 0, 1],
            [1, 1, 0, 0, 1, 1, 0, 0, 0, 0],
            [1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0]
        )
    );
    for route in [&across_row[..], &down_column, &slant] {
        occ.lay(route, false);
    }
    assert_eq!(held(&occ), ([0; 4], [0; 10], [0; 11]));
}

/// A text is on the moves that run through its box shrunk by a tick: the moves along the lines strictly inside it, each
/// with the ticks it runs inside; two texts on one move are both listed.
#[test]
fn a_text_is_on_the_moves_through_it_with_the_ticks_inside() {
    let b = Page::new(12, 12);
    let text = |x0, y0, x1, y1| Text {
        rect: Rect::new(Pt::new(x0, y0), Pt::new(x1, y1)),
        owner: id("T"),
        label: None,
    };
    // Shrunk by a tick: x 17 to 47, y 17 to 39; the lines inside it are rows 3 and 4 and columns 3 to 5. The second
    // text, shrunk to x 41 to 55 and y 31 to 33, holds row 4 only.
    let occ = occupancy(&b.page, vec![text(16, 16, 48, 40), text(40, 30, 56, 34)]);
    let on = |n: Node, down: bool| occ.texts_on(mv(n, down)).to_vec();
    // Moves right along row 3: from the box's left edge to its right one, a part move at either end.
    assert_eq!(on((2, 3), false), [(0, 7)]);
    assert_eq!(on((3, 3), false), [(0, 8)]);
    assert_eq!(on((5, 3), false), [(0, 7)]);
    assert!(on((1, 3), false).is_empty() && on((6, 3), false).is_empty());
    // The rows on the box's edges are not inside it.
    assert!(on((3, 2), false).is_empty() && on((3, 5), false).is_empty());
    // Moves down along column 4.
    assert_eq!(on((4, 2), true), [(0, 7)]);
    assert_eq!(on((4, 3), true), [(0, 8)]);
    assert_eq!(on((4, 4), true), [(0, 7)]);
    assert!(on((2, 3), true).is_empty() && on((4, 5), true).is_empty());
    // The move right from (5, 4) runs through both.
    assert_eq!(on((5, 4), false), [(0, 7), (1, 7)]);
    assert_eq!(on((6, 4), false), [(1, 7)]);
}

/// A junction moved holds its new node and leaves its old one, which keeps what else holds it.
#[test]
fn a_junction_moved_holds_its_new_node_only() {
    let mut b = Page::new(12, 12);
    let End::Junction(j) = b.junction("J", p(3, 3)) else {
        unreachable!()
    };
    let End::Junction(k) = b.junction("K", p(3, 3)) else {
        unreachable!()
    };
    let mut occ = occupancy(&b.page, Vec::new());
    let mut both = [Holder::Junction(j), Holder::Junction(k)];
    both.sort();
    assert_eq!(occ.holders_at((6, 6)), both);
    occ.move_junction(j, p(3, 3), p(4, 3));
    assert_eq!(occ.holders_at((6, 6)), [Holder::Junction(k)]);
    assert_eq!(occ.holders_at((8, 6)), [Holder::Junction(j)]);
}
