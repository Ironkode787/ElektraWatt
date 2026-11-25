// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Texts placed clear of one another: the number of a terminal in
//! a stack two grid steps apart stands by its own terminal, not under the one above; a turned breaker's part number
//! and rating, which the turn put on one row, do not lie on each other; an interruption point's name placed at a box's
//! pin does not lie on the pin's text.

use std::sync::Arc;

use ew_dl::{ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_scene::page::{PageView, PinText, RepView, SlotValue, page_display_list};
use ew_schema::{LibCopy, LibDocument, SlotKind, SymbolDef};
use ew_text::{builtin, bundled};
use serde_json::json;

use crate::common::blank_page;

const TERMINAL: &str = include_str!("../../../../content/symbols/qet10/ew.iec.terminal@qet10.json");
const MCB: &str = include_str!("../../../../content/symbols/qet10/ew.iec.mcb-1pn@qet10.json");
const BLACKBOX: &str = include_str!("../../../../content/symbols/qet10/ew.gen.blackbox@qet10.json");
const IP: &str = include_str!("../../../../content/symbols/qet10/ew.gen.ip-target@qet10.json");
const PAGE: u128 = 0x100;

fn uuid(n: u128) -> String {
    Uuid::from_u128(n).hyphenated().to_string()
}

/// QET 10 px = 7 937 500 / 3 nm.
fn g(k: i64) -> i64 {
    k * 7_937_500 / 3
}

fn symbol(text: &str) -> (Id<SymbolDef>, Arc<SymbolDef>) {
    match LibDocument::from_json(text).unwrap() {
        LibDocument::Symbol(LibCopy { id, item, .. }) => (id, Arc::new(item)),
        _ => panic!("not a symbol"),
    }
}

fn slot(def: &SymbolDef, kind: &SlotKind) -> u16 {
    let i = def
        .text_slots
        .iter()
        .position(|t| &t.kind == kind)
        .expect("the slot");
    u16::try_from(i).unwrap()
}

fn value(slot: u16, text: &str) -> SlotValue {
    SlotValue {
        slot,
        text: text.into(),
        link: None,
        item_links: Vec::new(),
    }
}

/// A representation `n` of `text`'s symbol at `pos` turned by `orient`, with its texts and pins.
fn rep(
    n: u128,
    text: &str,
    pos: (i64, i64),
    orient: &str,
    slots: &[(SlotKind, &str)],
    more: serde_json::Value,
) -> RepView {
    let (id, def) = symbol(text);
    let mut r = json!({
        "id": uuid(n), "functions": [uuid(n + 0x1000)], "page": uuid(PAGE),
        "symbol": id.as_uuid().hyphenated().to_string(), "pos": [pos.0, pos.1], "orient": orient,
    });
    if let (Some(r), Some(m)) = (r.as_object_mut(), more.as_object()) {
        r.extend(m.clone());
    }
    RepView {
        rep: Arc::new(serde_json::from_value(r).unwrap()),
        terminal: false,
        slots: slots.iter().map(|(k, t)| value(slot(&def, k), t)).collect(),
        symbol: Some(def),
        terminals: Vec::new(),
        pins: Vec::new(),
    }
}

/// A glyph run: its text, the minimum and the maximum of its layout box.
type Run = (String, (i64, i64), (i64, i64));

/// The labels layer's glyph runs.
fn runs(v: &PageView) -> Vec<Run> {
    let list = page_display_list(v, Layer::Labels, bundled(), builtin());
    list.list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => {
                let q = run.layout_quad();
                let xs = q.iter().map(|p| p.x.0);
                let ys = q.iter().map(|p| p.y.0);
                Some((
                    run.text.clone(),
                    (xs.clone().min().unwrap(), ys.clone().min().unwrap()),
                    (xs.max().unwrap(), ys.max().unwrap()),
                ))
            }
            _ => None,
        })
        .collect()
}

fn run_of(v: &PageView, text: &str) -> ((i64, i64), (i64, i64)) {
    runs(v)
        .into_iter()
        .find(|(t, _, _)| t == text)
        .map(|(_, a, b)| (a, b))
        .unwrap_or_else(|| panic!("no text {text}"))
}

fn overlap(a: ((i64, i64), (i64, i64)), b: ((i64, i64), (i64, i64))) -> bool {
    a.0.0 < b.1.0 && b.0.0 < a.1.0 && a.0.1 < b.1.1 && b.0.1 < a.1.1
}

#[test]
fn a_terminals_number_in_a_stack_stands_by_its_own_terminal() {
    // Three terminals turned a quarter (their wires run across), two grid steps apart, one under the other.
    let terminal = |n: u128, k: i64, number: &str| {
        rep(
            n,
            TERMINAL,
            (g(12), g(16 + 2 * k)),
            "r90",
            &[(SlotKind::Tag, number)],
            json!({}),
        )
    };
    let v = PageView {
        reps: vec![
            terminal(0x11, 0, "1"),
            terminal(0x12, 1, "2"),
            terminal(0x13, 2, "3"),
        ],
        ..blank_page(PAGE)
    };
    for (number, k) in [("2", 1), ("3", 2)] {
        let (a, b) = run_of(&v, number);
        let mid = (a.1 + b.1) / 2;
        let (own, above) = (g(16 + 2 * k), g(16 + 2 * (k - 1)));
        assert!(
            (mid - own).abs() < (mid - above).abs(),
            "{number} at {mid} stands nearer the terminal above ({above}) than its own ({own})"
        );
    }
}

#[test]
fn a_turned_breakers_part_number_and_rating_do_not_lie_on_each_other() {
    let mcb = rep(
        0x11,
        MCB,
        (g(30), g(20)),
        "m270",
        &[
            (SlotKind::PartNumber, "EW-GEN-MCB-2"),
            (SlotKind::TechData, "C10"),
        ],
        json!({}),
    );
    let v = PageView {
        reps: vec![mcb],
        ..blank_page(PAGE)
    };
    assert!(!overlap(run_of(&v, "EW-GEN-MCB-2"), run_of(&v, "C10")));
}

#[test]
fn an_interruption_points_name_at_a_boxs_pin_does_not_lie_on_the_pins_text() {
    // A 12 × 8 box with a pin leaving down at (4, 8), its text "+"; the point's tip on the pin.
    let mut b = rep(
        0x11,
        BLACKBOX,
        (g(30), g(30)),
        "r0",
        &[],
        json!({"box": {
            "outline": [[0, 0], [g(12), 0], [g(12), g(8)], [0, g(8)]],
            "cps": [{"idx": 0, "pos": [g(4), g(8)], "dir": "down"}],
        }}),
    );
    b.pins = vec![PinText {
        idx: 0,
        text: "+".to_owned(),
        cell: None,
    }];
    let ip = rep(
        0x12,
        IP,
        (g(34), g(38)),
        "r0",
        &[(SlotKind::FunctionText, "24V")],
        json!({}),
    );
    let v = PageView {
        reps: vec![b, ip],
        ..blank_page(PAGE)
    };
    assert!(!overlap(run_of(&v, "24V"), run_of(&v, "+")));
}
