// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The texts of a symbol turned a quarter stay level and are placed anew round its body (doc 07 §4.1,
//! `ew_scene::symbol::slot_places`): lying flat, its tag stands centred above the body and its data and
//! cross-reference centred below it, line under line, so that none stands on another or on the symbol. The symbol
//! editor's strip "Turned and mirrored" shows `lower`'s texts; a page prints the same places.

use std::sync::Arc;

use ew_dl::{EntityRef, ItemKind, Layer};
use ew_ids::Uuid;
use ew_scene::page::{PageView, RepView, SlotValue, SymbolRef, page_display_list};
use ew_scene::symbol::{TextSource, lower};
use ew_schema::{Align, LibCopy, LibDocument, Representation, SlotKind};
use ew_text::{builtin, bundled};
use ew_units::{Orient2, Pt2, Rect};
use serde_json::json;

use crate::common::blank_page;

const COIL: &str = include_str!("../../../../content/symbols/qet10/ew.iec.coil@qet10.json");
const CONTACT: &str = include_str!("../../../../content/symbols/qet10/ew.iec.contact-no@qet10.json");
const LAMP: &str = include_str!("../../../../content/symbols/qet10/ew.iec.lamp@qet10.json");

const PAGE: u128 = 0x100;
const QUARTERS: [Orient2; 4] = [Orient2::R90, Orient2::R270, Orient2::M90, Orient2::M270];

fn symbol(text: &str) -> SymbolRef {
    match LibDocument::from_json(text).unwrap() {
        LibDocument::Symbol(LibCopy { id, item, .. }) => SymbolRef {
            id,
            def: Arc::new(item),
        },
        _ => panic!("not a symbol"),
    }
}

/// Whether two rectangles share interior points.
fn overlap(a: Rect, b: Rect) -> bool {
    a.min().x < b.max().x && b.min().x < a.max().x && a.min().y < b.max().y && b.min().y < a.max().y
}

/// In each quarter turn of the starter coil, contact and lamp, the tag stands centred above the drawing and every
/// other text but a pin's designation centred below it, each on a line of its own in the order the texts stand down
/// the upright symbol; the pins' designations keep their places at their pins.
#[test]
fn a_symbol_turned_a_quarter_stacks_its_texts_above_and_below_its_body() {
    for text in [COIL, CONTACT, LAMP] {
        let s = symbol(text);
        let upright = lower(&s.def, Orient2::R0);
        for v in QUARTERS {
            let g = lower(&s.def, v);
            let body = g.graphics.bounds().expect("graphics");
            let centre = body.min().x.0 + body.width().0 / 2;
            let mut below: Vec<(i64, i64)> = Vec::new();
            for (t, r0) in g.texts.iter().zip(&upright.texts) {
                let TextSource::Slot(kind) = &t.source else {
                    continue;
                };
                if !t.visible {
                    continue;
                }
                match kind {
                    SlotKind::CpDesignation(_) => assert_eq!(t.pos, v.apply(r0.pos), "{kind:?} {v:?}"),
                    SlotKind::Tag => {
                        assert_eq!(t.align, Align::BottomCentre, "{v:?}");
                        assert!(
                            t.pos.y < body.min().y,
                            "{kind:?} {v:?}: {:?} over {body:?}",
                            t.pos
                        );
                        assert_eq!(t.pos.x.0, centre, "{v:?}");
                    }
                    _ => {
                        assert_eq!(t.align, Align::TopCentre, "{kind:?} {v:?}");
                        assert!(
                            t.pos.y > body.max().y,
                            "{kind:?} {v:?}: {:?} under {body:?}",
                            t.pos
                        );
                        assert_eq!(t.pos.x.0, centre, "{kind:?} {v:?}");
                        below.push((r0.pos.y.0, t.pos.y.0));
                    }
                }
            }
            // Line under line, in the upright symbol's order, never two on one line.
            below.sort();
            assert!(below.windows(2).all(|w| w[0].1 < w[1].1), "{v:?}: {below:?}");
        }
    }
}

/// The glyph runs of the labels layer: entity, text, box, and whether the text reads level.
fn runs(view: &PageView) -> Vec<(EntityRef, String, Rect, bool)> {
    page_display_list(view, Layer::Labels, bundled(), builtin())
        .list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => {
                let q = run.layout_quad();
                let rect = q[1..]
                    .iter()
                    .fold(Rect::from_point(q[0]), |r, p| r.union(Rect::from_point(*p)));
                Some((i.entity?, run.text.clone(), rect, q[1].y == q[2].y))
            }
            _ => None,
        })
        .collect()
}

/// A representation `n` of `s` at grid `(x, 40)` in `orient`, with `extra` fields, its slots filled by `texts`
/// (kind and text).
fn rep(
    s: &SymbolRef,
    n: u128,
    x: i64,
    orient: &str,
    texts: &[(SlotKind, &str)],
    extra: serde_json::Value,
) -> RepView {
    let step = 7_937_500 / 3;
    let mut v = json!({
        "id": Uuid::from_u128(n).hyphenated().to_string(),
        "functions": [Uuid::from_u128(n + 0x1000).hyphenated().to_string()],
        "page": Uuid::from_u128(PAGE).hyphenated().to_string(),
        "symbol": s.id.as_uuid().hyphenated().to_string(),
        "pos": [x * step, 40 * step], "orient": orient,
    });
    if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        o.extend(e.clone());
    }
    let slots = texts
        .iter()
        .filter_map(|(kind, text)| {
            let i = s.def.text_slots.iter().position(|t| &t.kind == kind)?;
            Some(SlotValue {
                slot: u16::try_from(i).unwrap(),
                text: (*text).into(),
                link: None,
                item_links: Vec::new(),
            })
        })
        .collect();
    RepView {
        rep: Arc::new(serde_json::from_value::<Representation>(v).unwrap()),
        symbol: Some(s.def.clone()),
        slots,
        terminals: Vec::new(),
        pins: Vec::new(),
    }
}

/// A coil and a make contact, each turned a quarter either way on a page, print their tag, PLC address, technical
/// data, cross-reference and pin numbers level, none on another and none on the symbol's drawing; a tag a person
/// dragged stays where it was dropped. (Twenty steps apart, all on the A4 sheet of a page without a frame: a text
/// never stands outside the drawing area.)
#[test]
fn turned_symbols_on_a_page_print_their_texts_apart() {
    let texts = [
        (SlotKind::Tag, "-K12"),
        (SlotKind::PlcAddress, "%Q2.5"),
        (SlotKind::TechData, "24 V DC"),
        (SlotKind::Xref, "/3.4"),
        (SlotKind::CpDesignation(0), "13"),
        (SlotKind::CpDesignation(1), "14"),
    ];
    let mut reps = Vec::new();
    let mut n = 1;
    for s in [symbol(COIL), symbol(CONTACT)] {
        for orient in ["r90", "r270"] {
            reps.push(rep(
                &s,
                n,
                20 * i64::try_from(n).unwrap(),
                orient,
                &texts,
                json!({}),
            ));
            n += 1;
        }
    }
    let step = 7_937_500 / 3;
    let moved = Pt2::new(ew_units::Nm(-2 * step), ew_units::Nm(-4 * step));
    let coil = symbol(COIL);
    reps.push(rep(
        &coil,
        n,
        20 * i64::try_from(n).unwrap(),
        "r90",
        &texts,
        json!({"text_slots": [{"slot": 0, "pos": [moved.x.0, moved.y.0]}]}),
    ));
    let view = PageView {
        reps,
        ..blank_page(PAGE)
    };
    let runs = runs(&view);
    for rv in &view.reps[..4] {
        let own: Vec<_> = runs.iter().filter(|r| r.0.0 == rv.rep.id.as_uuid()).collect();
        assert_eq!(own.len(), rv.slots.len(), "{runs:?}");
        let def = rv.symbol.as_ref().unwrap();
        let body = lower(def, rv.rep.orient).graphics.bounds().expect("graphics");
        let body = Rect::from_points(rv.rep.pos + body.min(), rv.rep.pos + body.max());
        for (i, a) in own.iter().enumerate() {
            assert!(a.3, "{} reads level", a.1);
            // A pin's number stands at its pin, beside the line and so inside the drawing's bounds.
            let pin = a.1 == "13" || a.1 == "14";
            assert!(
                pin || !overlap(a.2, body),
                "{} on its symbol, {:?}",
                a.1,
                rv.rep.orient
            );
            for b in &own[i + 1..] {
                assert!(!overlap(a.2, b.2), "{} on {}, {:?}", a.1, b.1, rv.rep.orient);
            }
        }
    }
    // The last coil's tag stands where it was dropped, as 0.3.0 printed it: anchored at the override's point turned,
    // with the slot's own alignment (`slot_align`), not the alignment of the texts placed anew round the body.
    let last = &view.reps[4];
    let def = last.symbol.as_ref().unwrap();
    let slot = def.text_slots.iter().find(|s| s.kind == SlotKind::Tag).unwrap();
    let align = ew_scene::symbol::slot_align(def, slot, slot.rot, Orient2::R90);
    let tag = last.rep.pos + Orient2::R90.apply(moved);
    let dropped = runs
        .iter()
        .find(|r| r.0.0 == last.rep.id.as_uuid() && r.1 == "-K12")
        .expect("the dragged tag")
        .2;
    // Where the alignment puts the anchor on the text's box, in halves of its width and of its height.
    let (h, v) = match align {
        Align::TopLeft => (0, 0),
        Align::TopCentre => (1, 0),
        Align::TopRight => (2, 0),
        Align::MiddleLeft => (0, 1),
        Align::Centre => (1, 1),
        Align::MiddleRight => (2, 1),
        Align::BottomLeft => (0, 2),
        Align::BottomCentre => (1, 2),
        Align::BottomRight => (2, 2),
        other => panic!("{other:?} names no corner or side of a box"),
    };
    let anchor = (
        dropped.min().x.0 + h * dropped.width().0 / 2,
        dropped.min().y.0 + v * dropped.height().0 / 2,
    );
    assert_eq!(anchor, (tag.x.0, tag.y.0), "{dropped:?} aligned {align:?}");
}
