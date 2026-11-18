// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pin texts of a black box (module documentation of `ew_scene::page`, "Pin texts"): each
//! [`RepView::pins`] entry prints in `data.small` inside the outline, 1 mm from its box CP along the inward normal,
//! upright on every side and under every turn (an imported box keeps its source's turned texts); an entry without a
//! box CP prints nothing.

use std::sync::Arc;

use ew_dl::{ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_scene::page::{
    BLACK_BOX_CAPTION_GAP, PageView, PinText, RepView, SLOT_TEXT_GAP, SlotValue, page_display_list,
};
use ew_schema::{LibCopy, LibDocument, Representation, SlotKind, SymbolDef};
use ew_text::{builtin, bundled};
use ew_units::{Nm, Pt2};
use serde_json::json;

use crate::common::blank_page;

const BLACKBOX: &str = include_str!("../../../../content/symbols/qet10/ew.gen.blackbox@qet10.json");
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
        reps: vec![RepView {
            rep: Arc::new(r),
            symbol: Some(def),
            slots: Vec::new(),
            terminals: Vec::new(),
            pins,
        }],
        ..blank_page(PAGE)
    }
}

/// A glyph run: its text, its layout quad's minimum and maximum.
type Run = (String, (i64, i64), (i64, i64));

/// The labels layer's runs.
fn runs(v: &PageView) -> Vec<Run> {
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
    // top pin: below it, upright (wider than tall), centred on its x
    let (a, b) = at("Q0.0");
    assert!(
        near(a.1, y0 + MM) && b.1 > a.1 && b.0 - a.0 > b.1 - a.1,
        "{a:?} {b:?}"
    );
    assert!(near(a.0 + b.0, 2 * mid.0), "{a:?} {b:?}");
    // bottom pin: above it, upright
    let (a, b) = at("Q0.1");
    assert!(near(b.1, y0 + side - MM) && b.0 - a.0 > b.1 - a.1, "{a:?} {b:?}");
    // left pin: right of it, horizontal
    let (a, b) = at("Q0.2");
    assert!(near(a.0, x0 + MM) && b.0 - a.0 > b.1 - a.1, "{a:?} {b:?}");
    assert!(a.1 < mid.1 && b.1 > mid.1);
    // right pin: left of it, horizontal
    let (a, b) = at("Q0.3");
    assert!(near(b.0, x0 + side - MM) && b.0 - a.0 > b.1 - a.1, "{a:?} {b:?}");
}

#[test]
fn every_pin_text_of_a_turned_box_stays_upright() {
    for orient in ["r90", "r180", "r270"] {
        for (t, a, b) in runs(&view(orient)) {
            assert!(b.0 - a.0 > b.1 - a.1, "{orient} {t}: {a:?} {b:?}");
        }
    }
}

/// A box imported from another drawing keeps the turned texts its source drew on its top and bottom.
#[test]
fn an_imported_box_keeps_its_turned_pin_texts() {
    let mut v = view("r0");
    Arc::make_mut(&mut v.reps[0].rep).source_ref =
        Some(serde_json::from_value(json!({"lineage": uuid(0x99), "key": "representations|-A1"})).unwrap());
    let r = runs(&v);
    let (_, a, b) = r.iter().find(|(t, _, _)| t == "Q0.0").unwrap().clone();
    assert!(b.1 - a.1 > b.0 - a.0, "{a:?} {b:?}");
}

/// Long upright texts on pins one grid step apart would lie on one another: they take two lines in turn, and no two
/// texts of the side overlap.
#[test]
fn neighbouring_upright_pin_texts_take_two_lines_in_turn() {
    let (id, def) = blackbox();
    let cps: Vec<serde_json::Value> = (0..4)
        .map(|i| json!({"idx": i, "pos": [g(2 + 2 * i), 0], "dir": "up"}))
        .collect();
    let r: Representation = serde_json::from_value(json!({
        "id": uuid(0x11), "functions": [uuid(0x21)], "page": uuid(PAGE),
        "symbol": id.as_uuid().hyphenated().to_string(), "pos": [g(30), g(30)],
        "box": {"outline": [[0, 0], [g(10), 0], [g(10), g(10)], [0, g(10)]], "cps": cps},
    }))
    .unwrap();
    let pins = (0..4)
        .map(|idx| PinText {
            idx,
            text: format!("24VDC{idx}"),
        })
        .collect();
    let v = PageView {
        reps: vec![RepView {
            rep: Arc::new(r),
            symbol: Some(def),
            slots: Vec::new(),
            terminals: Vec::new(),
            pins,
        }],
        ..blank_page(PAGE)
    };
    let r = runs(&v);
    assert_eq!(r.len(), 4);
    let tops: Vec<i64> = r.iter().map(|(_, a, _)| a.1).collect();
    assert!(
        near(tops[0], tops[2]) && near(tops[1], tops[3]) && tops[1] > tops[0],
        "{tops:?}"
    );
    for (i, x) in r.iter().enumerate() {
        for y in &r[i + 1..] {
            let apart = x.2.0 <= y.1.0 || y.2.0 <= x.1.0 || x.2.1 <= y.1.1 || y.2.1 <= x.1.1;
            assert!(apart, "{x:?} {y:?}");
        }
    }
}

#[test]
fn a_representation_without_a_box_prints_no_pin_text() {
    let mut v = view("r0");
    Arc::make_mut(&mut v.reps[0].rep).box_ = None;
    assert!(runs(&v).is_empty());
}

/// rc.5 (the owner's box made smaller had its cross-reference on its edge): a cross-reference text that would cross a
/// narrow black box's right edge stands outside it, left of its top-left corner, clear of the outline.
#[test]
fn a_narrow_boxs_cross_reference_stands_outside_it() {
    let mut v = view("r0");
    {
        let r = Arc::make_mut(&mut v.reps[0].rep);
        let b = r.box_.as_mut().unwrap();
        b.outline = serde_json::from_value(json!([[0, 0], [g(4), 0], [g(4), g(12)], [0, g(12)]])).unwrap();
        b.cps.clear();
    }
    v.reps[0].pins.clear();
    let xref = v.reps[0]
        .symbol
        .as_ref()
        .unwrap()
        .text_slots
        .iter()
        .position(|s| matches!(s.kind, SlotKind::Xref))
        .unwrap();
    v.reps[0].slots = vec![SlotValue {
        slot: u16::try_from(xref).unwrap(),
        text: "902-D1, 902-D4".to_owned(),
        link: None,
        item_links: Vec::new(),
    }];
    let r = runs(&v);
    let (_, a, b) = r.iter().find(|(t, _, _)| t.contains("902-D1")).unwrap().clone();
    assert!(b.0 < g(30), "left of the box: {a:?} {b:?}");
    assert!(a.1 >= g(30) && a.1 < g(34), "at its top: {a:?} {b:?}");
}

/// A pin text is a fixed obstacle of the cross-reference text. A pin near the box's top-left corner prints
/// its text where the box's cross-reference slot sits; the cross-reference drops below it, `SLOT_TEXT_GAP` apart,
/// and the pin text stays 1 mm from its pin.
#[test]
fn a_cross_reference_drops_below_a_pin_text_it_would_overlap() {
    let mut v = view("r0");
    Arc::make_mut(&mut v.reps[0].rep).box_.as_mut().unwrap().cps[0].pos = Pt2::new(Nm(g(1)), Nm(0));
    v.reps[0].pins = vec![PinText {
        idx: 0,
        text: "1".to_owned(),
    }];
    let xref = v.reps[0]
        .symbol
        .as_ref()
        .unwrap()
        .text_slots
        .iter()
        .position(|s| matches!(s.kind, SlotKind::Xref))
        .unwrap();
    v.reps[0].slots = vec![SlotValue {
        slot: u16::try_from(xref).unwrap(),
        text: "(201-E1)".to_owned(),
        link: None,
        item_links: Vec::new(),
    }];
    let find = |r: &[Run], t: &str| {
        r.iter()
            .find(|(x, _, _)| x == t)
            .map(|(_, a, b)| (*a, *b))
            .unwrap()
    };
    let r = runs(&v);
    let (pa, pb) = find(&r, "1");
    let (xa, xb) = find(&r, "(201-E1)");
    // the pin text has not moved: 1 mm below its pin
    assert!(near(pa.1, g(30) + MM), "{pa:?}");
    // the cross-reference no longer overlaps it and sits the gap below it
    let overlap = xa.0 < pb.0 && pa.0 < xb.0 && xa.1 < pb.1 && pa.1 < xb.1;
    assert!(!overlap, "pin {pa:?} {pb:?}, xref {xa:?} {xb:?}");
    assert!(
        near(xa.1 - pb.1, SLOT_TEXT_GAP.0),
        "pin {pa:?} {pb:?}, xref {xa:?} {xb:?}"
    );
    // without the pin text the cross-reference keeps its place at the top of the box, where the pin text would be
    v.reps[0].pins.clear();
    let (ya, _) = find(&runs(&v), "(201-E1)");
    assert!(ya.1 < pb.1, "{ya:?}");
}

/// A black box's caption that would stand over a neighbour box's pin text rises
/// `SLOT_TEXT_GAP` above it; the pin text stays 1 mm from its pin. The neighbour stands on the box's top edge, its
/// one pin leaving down one grid step right of the box's left edge, where the caption starts.
#[test]
fn a_black_box_caption_rises_above_a_neighbours_pin_text() {
    let mut v = view("r0");
    let tag = v.reps[0]
        .symbol
        .as_ref()
        .unwrap()
        .text_slots
        .iter()
        .position(|s| matches!(s.kind, SlotKind::Tag))
        .unwrap();
    {
        let r = Arc::make_mut(&mut v.reps[0].rep);
        r.box_.as_mut().unwrap().cps.clear();
    }
    v.reps[0].pins.clear();
    v.reps[0].slots = vec![SlotValue {
        slot: u16::try_from(tag).unwrap(),
        text: "-A1".to_owned(),
        link: None,
        item_links: Vec::new(),
    }];
    let mut neighbour = v.reps[0].clone();
    {
        let r = Arc::make_mut(&mut neighbour.rep);
        r.id = Id::from_uuid(Uuid::from_u128(0x12));
        r.pos = Pt2::new(Nm(g(30)), Nm(g(18)));
        r.box_.as_mut().unwrap().cps =
            serde_json::from_value(json!([{"idx": 1, "pos": [g(1), g(12)], "dir": "down"}])).unwrap();
    }
    neighbour.slots.clear();
    neighbour.pins = vec![PinText {
        idx: 1,
        text: "Q1.0".to_owned(),
    }];
    v.reps.push(neighbour);
    let r = runs(&v);
    let find = |t: &str| {
        r.iter()
            .find(|(x, _, _)| x == t)
            .map(|(_, a, b)| (*a, *b))
            .unwrap()
    };
    let (pa, pb) = find("Q1.0");
    let (ca, cb) = find("-A1");
    // the pin text has not moved: 1 mm above its pin on the box's top edge
    assert!(near(pb.1, g(30) - MM), "{pa:?} {pb:?}");
    let overlap = ca.0 < pb.0 && pa.0 < cb.0 && ca.1 < pb.1 && pa.1 < cb.1;
    assert!(!overlap, "pin {pa:?} {pb:?}, caption {ca:?} {cb:?}");
    assert!(
        near(pa.1 - cb.1, SLOT_TEXT_GAP.0),
        "pin {pa:?} {pb:?}, caption {ca:?} {cb:?}"
    );
}

/// The same caption near the sheet's top edge would rise off the sheet above the neighbour's pin text; it
/// stays at its slot, its anchor `BLACK_BOX_CAPTION_GAP` above the box, inside the page.
#[test]
fn a_black_box_caption_never_rises_off_the_sheet() {
    let mut v = view("r0");
    let tag = v.reps[0]
        .symbol
        .as_ref()
        .unwrap()
        .text_slots
        .iter()
        .position(|s| matches!(s.kind, SlotKind::Tag))
        .unwrap();
    {
        let r = Arc::make_mut(&mut v.reps[0].rep);
        r.pos = Pt2::new(Nm(g(30)), Nm(g(3)));
        r.box_.as_mut().unwrap().cps.clear();
    }
    v.reps[0].pins.clear();
    v.reps[0].slots = vec![SlotValue {
        slot: u16::try_from(tag).unwrap(),
        text: "-A1".to_owned(),
        link: None,
        item_links: Vec::new(),
    }];
    let mut neighbour = v.reps[0].clone();
    {
        let r = Arc::make_mut(&mut neighbour.rep);
        r.id = Id::from_uuid(Uuid::from_u128(0x12));
        r.pos = Pt2::new(Nm(g(30)), Nm(g(-9)));
        r.box_.as_mut().unwrap().cps =
            serde_json::from_value(json!([{"idx": 1, "pos": [g(1), g(12)], "dir": "down"}])).unwrap();
    }
    neighbour.slots.clear();
    neighbour.pins = vec![PinText {
        idx: 1,
        text: "Q1.0".to_owned(),
    }];
    v.reps.push(neighbour);
    let r = runs(&v);
    let (ca, cb) = r
        .iter()
        .find(|(x, _, _)| x == "-A1")
        .map(|(_, a, b)| (*a, *b))
        .unwrap();
    assert!(
        near(cb.1, g(3) - BLACK_BOX_CAPTION_GAP.0),
        "caption {ca:?} {cb:?}"
    );
    assert!(ca.1 >= 0, "caption {ca:?} {cb:?}");
}
