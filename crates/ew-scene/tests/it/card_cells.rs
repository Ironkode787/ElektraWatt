// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A PLC card drawn in one row prints in cells (doc 10 §3.6, "A card drawn in cells"): a thin line between each two
//! neighbouring channel pins across the box, and each channel's designation and address, with its function text under
//! them, level in its cell, centred on its pin and kept within the cell.

use std::sync::Arc;

use ew_dl::{ItemKind, Layer, PathEl};
use ew_ids::Uuid;
use ew_scene::page::{PageView, PinText, RepView, page_display_list};
use ew_schema::{LibCopy, LibDocument, Representation};
use ew_text::{builtin, bundled};
use ew_units::Pt2;
use serde_json::json;

use crate::common::blank_page;

const BLACKBOX: &str = include_str!("../../../../content/symbols/qet10/ew.gen.blackbox@qet10.json");
const PAGE: u128 = 0x100;

/// QET 10 px = 7 937 500 / 3 nm.
fn g(k: i64) -> i64 {
    k * 7_937_500 / 3
}

/// A card of outputs at grid (20, 20) in `orient`: a box 26 × 6 steps with three channel pins on its lower edge, 8
/// steps apart from x 5, and a supply pin on its left edge; the channels' cells carry their function texts.
fn view(orient: &str) -> PageView {
    let (id, def) = match LibDocument::from_json(BLACKBOX).unwrap() {
        LibDocument::Symbol(LibCopy { id, item, .. }) => (id, Arc::new(item)),
        _ => panic!("not a symbol"),
    };
    let r: Representation = serde_json::from_value(json!({
        "id": Uuid::from_u128(0x11).hyphenated().to_string(),
        "functions": [Uuid::from_u128(0x21).hyphenated().to_string()],
        "page": Uuid::from_u128(PAGE).hyphenated().to_string(),
        "symbol": id.as_uuid().hyphenated().to_string(), "pos": [g(20), g(20)], "orient": orient,
        "box": {
            "outline": [[0, 0], [g(26), 0], [g(26), g(6)], [0, g(6)]],
            "cps": [
                {"idx": 0, "pos": [g(5), g(6)], "dir": "down"},
                {"idx": 1, "pos": [g(13), g(6)], "dir": "down"},
                {"idx": 2, "pos": [g(21), g(6)], "dir": "down"},
                {"idx": 3, "pos": [0, g(3)], "dir": "left"},
            ],
        },
    }))
    .unwrap();
    let pin = |idx: u16, text: &str, cell: Option<&str>| PinText {
        idx,
        text: text.to_owned(),
        cell: cell.map(str::to_owned),
    };
    PageView {
        reps: vec![RepView {
            rep: Arc::new(r),
            symbol: Some(def),
            terminal: false,
            slots: Vec::new(),
            terminals: Vec::new(),
            pins: vec![
                pin(0, "0 Q0.0", Some("Pump on")),
                pin(1, "1 Q0.1", Some("")),
                pin(
                    2,
                    "2 Q0.2",
                    Some("Conveyor belt runs forward to the clamp station"),
                ),
                pin(3, "L+", None),
            ],
        }],
        ..blank_page(PAGE)
    }
}

/// The labels layer's runs: text, layout box (min, max) and whether it reads level.
fn runs(v: &PageView) -> Vec<(String, Pt2, Pt2, bool)> {
    page_display_list(v, Layer::Labels, bundled(), builtin())
        .list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => {
                let q = run.layout_quad();
                let min = Pt2::new(q.iter().map(|p| p.x).min()?, q.iter().map(|p| p.y).min()?);
                let max = Pt2::new(q.iter().map(|p| p.x).max()?, q.iter().map(|p| p.y).max()?);
                Some((run.text.clone(), min, max, q[1].y == q[2].y))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_card_in_one_row_prints_its_channels_level_in_cells() {
    let v = view("r0");
    let r = runs(&v);
    let (x0, y0) = (g(20), g(20));
    let (top, bottom) = (y0, y0 + g(6));
    for (k, (line, function)) in [
        ("0 Q0.0", Some("Pump on")),
        ("1 Q0.1", None),
        ("2 Q0.2", Some("Conveyor belt runs forward to the clamp station")),
    ]
    .into_iter()
    .enumerate()
    {
        let pin_x = x0 + g(5 + 8 * i64::try_from(k).unwrap());
        let (left, right) = (pin_x - g(4), pin_x + g(4));
        let run = r
            .iter()
            .find(|t| t.0 == line)
            .unwrap_or_else(|| panic!("{line}: {r:?}"));
        assert!(run.3, "{line} reads level");
        // Inside the box, in its own cell, centred on its pin, next to the pin's edge.
        assert!(run.1.x.0 > left && run.2.x.0 < right, "{line}: {run:?}");
        assert!(
            (run.1.x.0 + run.2.x.0) / 2 - pin_x < 50_000,
            "{line} centred: {run:?}"
        );
        assert!(run.1.y.0 > top && run.2.y.0 < bottom, "{line}: {run:?}");
        if let Some(function) = function {
            // A function text too wide for its cell is made smaller, and below half its size wrapped at its words.
            let lines: Vec<_> = r
                .iter()
                .filter(|t| t.0 != line && function.contains(t.0.as_str()))
                .collect();
            assert_eq!(
                lines.iter().map(|t| t.0.as_str()).collect::<Vec<_>>().join(" "),
                function,
                "{r:?}"
            );
            for f in lines {
                assert!(f.3, "{function} reads level");
                assert!(
                    f.1.x.0 > left && f.2.x.0 < right,
                    "{function} within its cell: {f:?}"
                );
                // Further inside than the channel's line: an output card's pins are on its lower edge.
                assert!(f.2.y.0 <= run.1.y.0, "{function} over {line}: {f:?} {run:?}");
                assert!(f.1.y.0 > top, "{function}: {f:?}");
            }
        }
    }
    // The supply pin keeps its text as a box pin's.
    assert!(r.iter().any(|t| t.0 == "L+"), "{r:?}");
    // A thin line between each two neighbouring channel pins, across the box (the labels layer draws them).
    let symbols = page_display_list(&v, Layer::Labels, bundled(), builtin());
    let dividers: Vec<i64> = symbols
        .list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Path(p) => match p.path.els.as_slice() {
                [PathEl::MoveTo(a), PathEl::LineTo(b)] if a.x == b.x && a.y.0 == top && b.y.0 == bottom => {
                    Some(a.x.0)
                }
                _ => None,
            },
            _ => None,
        })
        .collect();
    let middle = |a: i64, b: i64| x0 + g(a) + (g(b) - g(a)) / 2;
    assert_eq!(dividers, [middle(5, 13), middle(13, 21)], "{dividers:?}");
}

/// The same card's box turned a quarter: its channel pins leave sideways, so it prints no cells, neither their lines
/// nor their function texts, and each pin keeps its text as any box pin's.
#[test]
fn a_card_turned_a_quarter_prints_no_cells() {
    for orient in ["r90", "r270"] {
        let v = view(orient);
        let labels = page_display_list(&v, Layer::Labels, bundled(), builtin());
        let lines = labels
            .list
            .items
            .iter()
            .filter(|i| matches!(i.kind, ItemKind::Path(_)))
            .count();
        assert_eq!(lines, 0, "{orient}: no cell lines");
        let r = runs(&v);
        for pin in ["0 Q0.0", "1 Q0.1", "2 Q0.2", "L+"] {
            assert!(r.iter().any(|t| t.0 == pin), "{orient} {pin}: {r:?}");
        }
        assert!(!r.iter().any(|t| t.0.contains("Pump")), "{orient}: {r:?}");
    }
}
