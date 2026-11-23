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

/// The bridge lines (paths without an entity, stroked, from their first point to their last; a hop's apex off the
/// line ends one of its curves) and the dots (filled, by their centres).
fn drawn(v: &PageView) -> (Vec<(Pt2, Pt2)>, Vec<Pt2>) {
    let (lines, dots) = drawn_with_hops(v);
    (lines.into_iter().map(|(a, b, _)| (a, b)).collect(), dots)
}

/// A bridge line: its first point, its last and the x of each hop's apex, in drawing order.
type Line = (Pt2, Pt2, Vec<Nm>);

/// [`drawn`] with each line's hops.
fn drawn_with_hops(v: &PageView) -> (Vec<Line>, Vec<Pt2>) {
    let list = page_display_list(v, Layer::Wires, bundled(), builtin()).list;
    let (mut lines, mut dots) = (Vec::new(), Vec::new());
    for item in list.items.iter().filter(|i: &&Item| i.entity.is_none()) {
        let ItemKind::Path(p) = &item.kind else { continue };
        match (&p.path.els[..], &p.fill) {
            ([PathEl::MoveTo(a), .., PathEl::LineTo(b)], None) => {
                let apexes = p
                    .path
                    .els
                    .iter()
                    .filter_map(|e| match e {
                        PathEl::CubicTo(_, _, end) if end.y != a.y => Some(end.x),
                        _ => None,
                    })
                    .collect();
                lines.push((*a, *b, apexes));
            }
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
    let (lines, dots) = drawn_with_hops(&v);
    // The third terminal's lead, crossed without a dot, is hopped over (a persona read the line as joining it).
    assert_eq!(lines, [(mid(10, 20), mid(28, 20), vec![Nm(g(22))])]);
    assert_eq!(dots, [mid(10, 20), mid(16, 20), mid(28, 20)]);
}

/// The two symbols of a block of two levels read as one block (doc 08 §6.3): a dashed tie at their bodies' height
/// between them; two symbols of two terminals get none.
#[test]
fn the_two_symbols_of_a_block_are_tied() {
    // Terminal 1's second level drawn by symbol 2 through its pin map, two steps to the right.
    let level_two = {
        let mut r = terminal(2, 12, 20);
        let mut rep = (*r.rep).clone();
        rep.functions = vec![Id::from_uuid(Uuid::from_u128(0xF1))];
        rep.pin_map = serde_json::from_value(json!([
            {"symbol_cp": 0, "function": uuid(0xF1), "idx": 2},
            {"symbol_cp": 1, "function": uuid(0xF1), "idx": 3},
        ]))
        .unwrap();
        r.rep = Arc::new(rep);
        r
    };
    let ties = |v: &PageView| {
        let list = page_display_list(v, Layer::Wires, bundled(), builtin()).list;
        list.items
            .iter()
            .filter(|i| i.entity.is_none())
            .filter_map(|i| match &i.kind {
                ItemKind::Path(p) if p.stroke.as_ref().is_some_and(|s| s.dash.is_some()) => p.path.bounds(),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let block = PageView {
        reps: vec![terminal(1, 10, 20), level_two],
        ..blank_page(PAGE)
    };
    let tie = ties(&block);
    assert_eq!(tie.len(), 1, "one tie");
    let y = Nm(g(20) - block.grid.at(1).expect("a step").0 / 2);
    assert_eq!(
        (tie[0].min().y, tie[0].max().y),
        (y, y),
        "half a step above the bodies' middle, clear of their numbers"
    );
    assert!(
        Nm(g(10)) < tie[0].min().x && tie[0].max().x < Nm(g(12)),
        "between the bodies"
    );
    let two_terminals = PageView {
        reps: vec![terminal(1, 10, 20), terminal(2, 12, 20)],
        ..blank_page(PAGE)
    };
    assert!(ties(&two_terminals).is_empty());
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

/// The layout's page keeps texts off a drawn bridge (FT-2021): its line, from the first terminal's point to the last,
/// lies inside a keep-out of the labels layer's draft page, which the same terminals without the bridge do not have.
#[test]
fn the_layout_keeps_texts_off_a_bridge() {
    let v = PageView {
        reps: vec![terminal(1, 10, 20), terminal(2, 16, 20), terminal(3, 22, 20)],
        bridges: vec![bridge(&[1, 3])],
        ..blank_page(PAGE)
    };
    let keep_out = |v: &PageView| {
        let layer = page_display_list(v, Layer::Labels, bundled(), builtin());
        layer
            .draft
            .expect("the labels layer's draft page")
            .keep_out
            .clone()
    };
    let ticks = ew_scene::ticks::Ticks::of(v.grid);
    let (a, b) = (ticks.point(mid(10, 20)).0, ticks.point(mid(22, 20)).0);
    let line = ew_drafting::Rect {
        x0: a.x,
        y0: a.y,
        x1: b.x,
        y1: b.y,
    };
    assert!(
        keep_out(&v)
            .iter()
            .any(|k| k.holds(line) && k.overlaps(line.grown(1))),
        "{:?} holds {line:?}",
        keep_out(&v)
    );
    let plain = PageView {
        bridges: Vec::new(),
        ..v
    };
    assert!(!keep_out(&plain).iter().any(|k| k.holds(line)));
}
