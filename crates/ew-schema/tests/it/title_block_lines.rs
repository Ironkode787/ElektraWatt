// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A title block is a table (doc 09 §2.5): its lines are its cells' edges where two cells meet, each once. The starter
//! A4 block's lines are exactly the ones `FormDef::cell_edges` makes from its cells.

use std::collections::BTreeSet;

use ew_schema::{LibDocument, Primitive};

#[test]
fn the_starter_a4_blocks_lines_are_its_cells_edges() {
    let text = include_str!("../../../../content/forms/ew.titleblock.qet-a4.json");
    let LibDocument::Form(copy) = LibDocument::from_json(text).expect("the starter block") else {
        panic!("a form");
    };
    let drawn: BTreeSet<_> = copy
        .item
        .graphics
        .iter()
        .filter_map(|g| match g {
            Primitive::Line(l) => Some(if (l.from.x, l.from.y) <= (l.to.x, l.to.y) {
                (l.from, l.to)
            } else {
                (l.to, l.from)
            }),
            _ => None,
        })
        .collect();
    let edges = copy.item.cell_edges();
    assert_eq!(edges.len(), drawn.len(), "each line once");
    assert_eq!(edges.into_iter().collect::<BTreeSet<_>>(), drawn);
}
