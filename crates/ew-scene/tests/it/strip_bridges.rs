// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A strip's bridge drawn on a schematic page (FT-1908, doc 08 §6.5): a line across the leads of the terminals it
//! joins where they stand in one row, with a dot on each; a terminal between them that it does not join is crossed
//! without a dot; a terminal of it in another row joins no line, and a row with one terminal of it draws nothing.

use std::sync::Arc;

use ew_dl::{Brush, Item, ItemKind, Layer, PathEl};
use ew_ids::{Id, Uuid};
use ew_scene::page::{BridgeView, JUNCTION_DOT_RADIUS, PageView, RepView, page_display_list};
use ew_schema::{LibCopy, LibDocument, SymbolDef};
use ew_text::{builtin, bundled};
use ew_units::{Nm, Pt2};
use serde_json::json;

use crate::common::blank_page;

const TERMINAL: &str = include_str!("../../../../content/symbols/qet10/ew.iec.terminal@qet10.json");
const PAGE: u128 = 0x100;

fn uuid(n: u128) -> String {
    Uuid::from_u128(n).hyphenated().to_string()
}

/// QET 10 px = 7 937 500 / 3 nm.
fn g(k: i64) -> i64 {
    k * 7_937_500 / 3
}

fn symbol() -> (Id<SymbolDef>, Arc<SymbolDef>) {
    match LibDocument::from_json(TERMINAL).unwrap() {
        LibDocument::Symbol(LibCopy { id, item, .. }) => (id, Arc::new(item)),
        _ => panic!("not a symbol"),
    }
}

/// Terminal `n` (function `0xF0 + n`) at grid `(x, y)`, unturned: its clamp 0 one step below its centre, leaving down.
fn terminal(n: u128, x: i64, y: i64) -> RepView {
    let (id, def) = symbol();
    let rep = serde_json::from_value(json!({
        "id": uuid(n), "functions": [uuid(0xF0 + n)], "page": uuid(PAGE),
        "symbol": id.as_uuid().hyphenated().to_string(), "pos": [g(x), g(y)], "orient": "r0",
    }))
    .unwrap();
    RepView {
        rep: Arc::new(rep),
        symbol: Some(def),
        slots: Vec::new(),
        terminals: Vec::new(),
        pins: Vec::new(),
    }
}

/// The bridge joining clamp 0 of the terminals `ns`.
fn bridge(ns: &[u128]) -> BridgeView {
    BridgeView {
        clamps: ns
            .iter()
            .map(|n| (Id::from_uuid(Uuid::from_u128(0xF0 + n)), 0))
            .collect(),
    }
}

/// Half way along terminal `(x, y)`'s lower lead, from its centre to its clamp 0 one step below.
fn mid(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm(g(x)), Nm(g(y) + (g(y + 1) - g(y)).div_euclid(2)))
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
                assert_eq!(
                    b.width(),
                    Nm(2 * JUNCTION_DOT_RADIUS.0),
                    "a dot of the junction's size"
                );
                dots.push(Pt2::new(
                    b.min().x + Nm(JUNCTION_DOT_RADIUS.0),
                    b.min().y + Nm(JUNCTION_DOT_RADIUS.0),
                ));
            }
            _ => {}
        }
    }
    (lines, dots)
}

#[test]
fn a_bridge_across_a_row_of_terminals_is_a_line_with_a_dot_on_each() {
    // Four terminals in a row six steps apart; the bridge joins the first, the second and the fourth. A fifth
    // terminal of the bridge stands in another row.
    let v = PageView {
        reps: vec![
            terminal(1, 10, 20),
            terminal(2, 16, 20),
            terminal(3, 22, 20),
            terminal(4, 28, 20),
            terminal(5, 34, 40),
        ],
        bridges: vec![bridge(&[1, 2, 4, 5])],
        ..blank_page(PAGE)
    };
    let (lines, dots) = drawn(&v);
    assert_eq!(lines, [(mid(10, 20), mid(28, 20))]);
    assert_eq!(dots, [mid(10, 20), mid(16, 20), mid(28, 20)]);
}

#[test]
fn a_bridge_with_one_terminal_in_a_row_draws_nothing() {
    let v = PageView {
        reps: vec![terminal(1, 10, 20), terminal(2, 16, 30)],
        bridges: vec![bridge(&[1, 2])],
        ..blank_page(PAGE)
    };
    assert_eq!(drawn(&v), (Vec::new(), Vec::new()));
    // The same terminals without the bridge: the layer holds no line without an entity either.
    let plain = PageView {
        bridges: Vec::new(),
        ..v
    };
    assert_eq!(drawn(&plain), (Vec::new(), Vec::new()));
}
