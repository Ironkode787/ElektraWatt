// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A strip segment draws its bridges (doc 08 §6.5): the terminals of the bridge that the segment shows are
//! joined by a line across their bridged leads, half way between each terminal's centre and its clamp, with a dot on
//! each; a terminal of the segment that the bridge does not join is crossed without a dot.

use std::sync::Arc;

use ew_dl::{Brush, Item, ItemKind, Layer, PathEl};
use ew_ids::{Id, Uuid};
use ew_scene::page::{BridgeView, PageView, RepView, SegmentTerminal, cp_points, page_display_list};
use ew_schema::{LibCopy, LibDocument};
use ew_text::{builtin, bundled};
use ew_units::{Nm, Pt2};
use serde_json::json;

use crate::common::blank_page;

const SEGMENT: &str = include_str!("../../../../content/symbols/ew.gen.strip-segment.json");
const PAGE: u128 = 0x100;

fn uuid(n: u128) -> String {
    Uuid::from_u128(n).hyphenated().to_string()
}

/// Terminal function `0xF0 + n`.
fn function(n: u128) -> Id<ew_schema::Function> {
    Id::from_uuid(Uuid::from_u128(0xF0 + n))
}

/// A strip segment at (40, 60) mm drawing the terminals 1 to 4, each with clamp 0 on side 1 and clamp 1 on side 2.
fn segment() -> RepView {
    let LibDocument::Symbol(LibCopy { id, item, .. }) = LibDocument::from_json(SEGMENT).unwrap() else {
        panic!("not a symbol")
    };
    let rep = serde_json::from_value(json!({
        "id": uuid(1), "functions": [uuid(0xE0)], "page": uuid(PAGE), "rep_type": "strip_segment",
        "symbol": id.as_uuid().hyphenated().to_string(), "pos": [40_000_000, 60_000_000], "orient": "r0",
    }))
    .unwrap();
    RepView {
        rep: Arc::new(rep),
        symbol: Some(Arc::new(item)),
        terminal: false,
        slots: Vec::new(),
        terminals: (1..=4)
            .map(|n| SegmentTerminal {
                function: function(n),
                side1: vec![0],
                side2: vec![1],
            })
            .collect(),
        pins: Vec::new(),
    }
}

/// The bridge lines (paths without an entity, stroked, of two points) and the dots (filled, by their centres).
fn drawn(v: &PageView) -> (Vec<(Pt2, Pt2)>, Vec<Pt2>) {
    let list = page_display_list(v, Layer::Wires, bundled(), builtin()).list;
    let (mut lines, mut dots) = (Vec::new(), Vec::new());
    for item in list.items.iter().filter(|i: &&Item| i.entity.is_none()) {
        let ItemKind::Path(p) = &item.kind else { continue };
        match (&p.path.els[..], &p.fill) {
            ([PathEl::MoveTo(a), PathEl::LineTo(b)], None) => lines.push((*a, *b)),
            (_, Some(fill)) if *fill == Brush::BLACK => {
                let b = p.path.bounds().expect("a dot");
                dots.push(Pt2::new(
                    b.min().x + Nm(b.width().0 / 2),
                    b.min().y + Nm(b.height().0 / 2),
                ));
            }
            _ => {}
        }
    }
    (lines, dots)
}

#[test]
fn a_strip_segment_draws_the_bus_of_its_bridge() {
    // The bridge joins clamp 0 (side 1, leaving up) of terminals 1, 2 and 4; terminal 3 stands between them.
    let v = PageView {
        reps: vec![segment()],
        bridges: vec![BridgeView {
            clamps: [1, 2, 4].map(|n| (function(n), 0)).to_vec(),
        }],
        ..blank_page(PAGE)
    };
    // Half way between a terminal's centre (half way between its two sides) and its side-1 point.
    let points = cp_points(&v);
    let at = |n: u128, idx: u16| points[&(function(n).as_uuid(), idx)];
    let half = |a: Pt2, b: Pt2| {
        Pt2::new(
            Nm(a.x.0 + (b.x.0 - a.x.0).div_euclid(2)),
            Nm(a.y.0 + (b.y.0 - a.y.0).div_euclid(2)),
        )
    };
    let mid = |n: u128| half(half(at(n, 0), at(n, 1)), at(n, 0));
    let (lines, dots) = drawn(&v);
    assert_eq!(lines, [(mid(1), mid(4))]);
    assert_eq!(dots, [mid(1), mid(2), mid(4)]);
    // The bus stands between the segment's top and its middle, across the leads.
    assert!(mid(1).y > at(1, 0).y && mid(1).y < at(1, 1).y);
    // Without the bridge, nothing of it is drawn.
    let plain = PageView {
        bridges: Vec::new(),
        ..v
    };
    assert_eq!(drawn(&plain), (Vec::new(), Vec::new()));
}
