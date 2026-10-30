// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pin texts of a black box (WP-S1.21 D7, module documentation of `ew_scene::page`, "Pin texts"): each
//! [`RepView::pins`] entry prints in `data.small` inside the outline, 1 mm from its box CP along the inward normal,
//! vertical for a pin leaving up or down and horizontal for one leaving left or right; an entry without a box CP prints
//! nothing; a turned box turns the rule with it.

use std::sync::Arc;

use ew_dl::{ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_scene::page::{PageKind, PageView, PinText, RepView, page_display_list};
use ew_schema::{LibCopy, LibDocument, Representation, SymbolDef};
use ew_text::{builtin, bundled};
use ew_units::Pt2;
use serde_json::json;

const BLACKBOX: &str = include_str!("../../../content/symbols/qet10/ew.gen.blackbox@qet10.json");
const PAGE: u128 = 0x100;
const MM: i64 = 1_000_000;

fn uuid(n: u128) -> String {
    Uuid::from_u128(n).hyphenated().to_string()
}

/// QET 10 px = 7 937 500 / 3 nm.
fn g(k: i64) -> i64 {
    k * 7_937_500 / 3
}

fn blackbox() -> (Id<SymbolDef>, Arc<SymbolDef>) {
    match LibDocument::from_json(BLACKBOX).unwrap() {
        LibDocument::Symbol(LibCopy { id, item, .. }) => (id, Arc::new(item)),
        _ => panic!("not a symbol"),
    }
}

/// A 12 × 12 box at grid (30, 30) with one CP on each edge: 0 top (up), 1 bottom (down), 2 left (left), 3 right
/// (right), in `orient`; pins for CPs 0 … 3 and for CP 9, which the box does not have.
fn view(orient: &str) -> PageView {
    let (id, def) = blackbox();
    let r: Representation = serde_json::from_value(json!({
        "id": uuid(0x11), "functions": [uuid(0x21)], "page": uuid(PAGE),
        "symbol": id.as_uuid().hyphenated().to_string(), "pos": [g(30), g(30)], "orient": orient,
        "box": {
            "outline": [[0, 0], [g(12), 0], [g(12), g(12)], [0, g(12)]],
            "cps": [
                {"idx": 0, "pos": [g(6), 0], "dir": "up"},
                {"idx": 1, "pos": [g(6), g(12)], "dir": "down"},
                {"idx": 2, "pos": [0, g(6)], "dir": "left"},
                {"idx": 3, "pos": [g(12), g(6)], "dir": "right"},
            ],
        },
    }))
    .unwrap();
    let pins = [(0, "Q0.0"), (1, "Q0.1"), (2, "Q0.2"), (3, "Q0.3"), (9, "Q9.9")]
        .map(|(idx, text)| PinText {
            idx,
            text: text.to_owned(),
        })
        .to_vec();
    PageView {
        page: Id::from_uuid(Uuid::from_u128(PAGE)),
        kind: PageKind::Schematic,
        grid: ew_units::Grid::QET_10PX,
        language: None,
        frame: None,
        reps: vec![RepView {
            rep: Arc::new(r),
            symbol: Some(def),
            slots: Vec::new(),
            terminals: Vec::new(),
            pins,
        }],
        connections: Vec::new(),
        junctions: Vec::new(),
        wire_marker: None,
        table: None,
        embedded: Vec::new(),
        break_marks: Vec::new(),
        cp_targets: Vec::new(),
        crossings: Default::default(),
        items: Vec::new(),
        structure_boxes: Vec::new(),
    }
}

/// The labels layer's runs: (text, layout quad's min, its max).
fn runs(v: &PageView) -> Vec<(String, (i64, i64), (i64, i64))> {
    let list = page_display_list(v, Layer::Labels, bundled(), builtin());
    list.list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => {
                let q: [Pt2; 4] = run.layout_quad();
                let min = (
                    q.iter().map(|p| p.x.0).min().unwrap(),
                    q.iter().map(|p| p.y.0).min().unwrap(),
                );
                let max = (
                    q.iter().map(|p| p.x.0).max().unwrap(),
                    q.iter().map(|p| p.y.0).max().unwrap(),
                );
                Some((run.text.clone(), min, max))
            }
            _ => None,
        })
        .collect()
}

fn near(a: i64, b: i64) -> bool {
    (a - b).abs() <= 1_000
}

#[test]
fn each_pin_text_sits_inside_the_box_one_millimetre_from_its_pin() {
    let r = runs(&view("r0"));
    let texts: Vec<&str> = r.iter().map(|(t, _, _)| t.as_str()).collect();
    assert_eq!(
        texts,
        ["Q0.0", "Q0.1", "Q0.2", "Q0.3"],
        "box-CP order; CP 9 prints nothing"
    );
    let (x0, y0, side) = (g(30), g(30), g(12));
    let mid = (x0 + g(6), y0 + g(6));
    let at = |t: &str| {
        r.iter()
            .find(|(x, _, _)| x == t)
            .map(|(_, a, b)| (*a, *b))
            .unwrap()
    };
    // top pin: below it, vertical (taller than wide), centred on its x
    let (a, b) = at("Q0.0");
    assert!(
        near(a.1, y0 + MM) && b.1 > a.1 && b.1 - a.1 > b.0 - a.0,
        "{a:?} {b:?}"
    );
    assert!(a.0 < mid.0 && b.0 > mid.0);
    // bottom pin: above it, vertical
    let (a, b) = at("Q0.1");
    assert!(near(b.1, y0 + side - MM) && b.1 - a.1 > b.0 - a.0, "{a:?} {b:?}");
    // left pin: right of it, horizontal
    let (a, b) = at("Q0.2");
    assert!(near(a.0, x0 + MM) && b.0 - a.0 > b.1 - a.1, "{a:?} {b:?}");
    assert!(a.1 < mid.1 && b.1 > mid.1);
    // right pin: left of it, horizontal
    let (a, b) = at("Q0.3");
    assert!(near(b.0, x0 + side - MM) && b.0 - a.0 > b.1 - a.1, "{a:?} {b:?}");
}

#[test]
fn a_turned_box_turns_the_rule() {
    // under a quarter turn the top pin leaves sideways: its text becomes horizontal
    let r = runs(&view("r90"));
    let (_, a, b) = r.iter().find(|(t, _, _)| t == "Q0.0").unwrap().clone();
    assert!(b.0 - a.0 > b.1 - a.1, "{a:?} {b:?}");
}

#[test]
fn a_representation_without_a_box_prints_no_pin_text() {
    let mut v = view("r0");
    Arc::make_mut(&mut v.reps[0].rep).box_ = None;
    assert!(runs(&v).is_empty());
}
