// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-734 and FT-758: a black box's caption (tag, part number, manufacturer) stacks clear of its top CPs; a pin text of
//! signs only prints upright; a device tag that runs into a neighbour's tag or a fixed text, or that a wire runs
//! through, moves to a free place beside its symbol (module documentation of `ew_scene::page`, "Black-box captions"
//! and "Device tags").

use std::sync::Arc;

use ew_dl::{EntityRef, ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_scene::page::{
    BLACK_BOX_CAPTION_GAP, ConnectionView, PageKind, PageView, PinText, RepView, SLOT_TEXT_GAP, SlotValue,
    SymbolRef, page_display_list,
};
use ew_schema::{Connection, LibCopy, LibDocument, Representation, SlotKind, SymbolDef};
use ew_text::{builtin, bundled};
use ew_units::{Pt2, Rect};
use serde_json::json;

const BLACKBOX: &str = include_str!("../../../content/symbols/qet10/ew.gen.blackbox@qet10.json");
const COIL: &str = include_str!("../../../content/symbols/qet10/ew.iec.coil@qet10.json");
const VALVE: &str = include_str!("../../../content/symbols/qet10/ew.iec.valve-coil@qet10.json");
const CONTACT_NO: &str = include_str!("../../../content/symbols/qet10/ew.iec.contact-no@qet10.json");
const CABLE: &str = include_str!("../../../content/symbols/qet10/ew.gen.cable-marker@qet10.json");

const PAGE: u128 = 0x100;

fn symbol(text: &str) -> SymbolRef {
    match LibDocument::from_json(text).unwrap() {
        LibDocument::Symbol(LibCopy { id, item, .. }) => SymbolRef {
            id,
            def: Arc::new(item),
        },
        _ => panic!("not a symbol"),
    }
}

fn uuid(n: u128) -> String {
    Uuid::from_u128(n).hyphenated().to_string()
}

/// QET 10 px = 7 937 500 / 3 nm; positions on that grid.
fn g(k: i64) -> i64 {
    k * 7_937_500 / 3
}

/// The index of the slot of kind `kind` in `s`.
fn slot(s: &SymbolRef, kind: &SlotKind) -> u16 {
    s.def
        .text_slots
        .iter()
        .position(|t| &t.kind == kind)
        .map(|i| u16::try_from(i).unwrap())
        .expect("the slot")
}

fn value(slot: u16, text: &str) -> SlotValue {
    SlotValue {
        slot,
        text: text.into(),
        link: None,
        item_links: Vec::new(),
    }
}

/// A representation `n` of `symbol` at grid `(x, y)`, drawing function `n + 0x1000`, with `extra` fields.
fn rep(n: u128, symbol: Id<SymbolDef>, x: i64, y: i64, extra: serde_json::Value) -> Arc<Representation> {
    let mut v = json!({
        "id": uuid(n), "functions": [uuid(n + 0x1000)], "page": uuid(PAGE),
        "symbol": symbol.as_uuid().hyphenated().to_string(), "pos": [g(x), g(y)], "orient": "r0",
    });
    if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        o.extend(e.clone());
    }
    Arc::new(serde_json::from_value(v).unwrap())
}

fn view(reps: Vec<RepView>, connections: Vec<ConnectionView>) -> PageView {
    PageView {
        page: Id::from_uuid(Uuid::from_u128(PAGE)),
        kind: PageKind::Schematic,
        grid: ew_units::Grid::QET_10PX,
        language: None,
        frame: None,
        reps,
        connections,
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

/// A labels-layer glyph run: its entity, text, layout quad and its bounds.
struct Run {
    entity: Option<EntityRef>,
    text: String,
    quad: [Pt2; 4],
    rect: Rect,
}

fn runs(v: &PageView) -> Vec<Run> {
    let list = page_display_list(v, Layer::Labels, bundled(), builtin());
    list.list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => {
                let quad = run.layout_quad();
                let rect = quad[1..]
                    .iter()
                    .fold(Rect::from_point(quad[0]), |r, q| r.union(Rect::from_point(*q)));
                Some(Run {
                    entity: i.entity,
                    text: run.text.clone(),
                    quad,
                    rect,
                })
            }
            _ => None,
        })
        .collect()
}

fn run<'a>(r: &'a [Run], entity: u128, text: &str) -> &'a Run {
    r.iter()
        .find(|x| x.entity == Some(EntityRef(Uuid::from_u128(entity))) && x.text == text)
        .unwrap_or_else(|| panic!("no text {text} of {entity:#x}"))
}

/// Whether two rectangles share interior points.
fn overlap(a: Rect, b: Rect) -> bool {
    a.min().x < b.max().x && b.min().x < a.max().x && a.min().y < b.max().y && b.min().y < a.max().y
}

/// Whether two texts run into each other: they overlap, or sit side by side less than the gap apart.
fn run_into(a: Rect, b: Rect) -> bool {
    let gap = SLOT_TEXT_GAP.0 - 1_000;
    a.min().x.0 < b.max().x.0 + gap
        && b.min().x.0 < a.max().x.0 + gap
        && a.min().y < b.max().y
        && b.min().y < a.max().y
}

/// Whether a run reads horizontally (its baseline, bottom-left to bottom-right, is level).
fn upright(r: &Run) -> bool {
    r.quad[1].y == r.quad[2].y
}

/// A 30 × 12-step black box `-101A1` at grid (30, 30) with the given box CPs `(idx, (x, y), dir)` and pin texts.
fn black_box(cps: &[(u16, (i64, i64), &str)], pins: &[(u16, &str)]) -> RepView {
    let s = symbol(BLACKBOX);
    let cps: Vec<serde_json::Value> = cps
        .iter()
        .map(|(idx, (x, y), dir)| json!({"idx": idx, "pos": [g(*x), g(*y)], "dir": dir}))
        .collect();
    RepView {
        rep: rep(
            0x11,
            s.id,
            30,
            30,
            json!({"box": {"outline": [[0, 0], [g(30), 0], [g(30), g(12)], [0, g(12)]], "cps": cps}}),
        ),
        symbol: Some(s.def.clone()),
        slots: vec![
            value(slot(&s, &SlotKind::Tag), "-101A1"),
            value(slot(&s, &SlotKind::PartNumber), "EW-GEN-PSU-1"),
            value(slot(&s, &SlotKind::Manufacturer), "Generic"),
        ],
        terminals: Vec::new(),
        pins: pins
            .iter()
            .map(|(idx, text)| PinText {
                idx: *idx,
                text: (*text).to_owned(),
            })
            .collect(),
    }
}

/// FT-734: with CPs on its top edge, the caption stacks tag, part number and manufacturer from the box upward, each a
/// gap above the one before, right-aligned one grid step left of the leftmost top CP: no caption text stands over a
/// CP, its pin text or each other.
#[test]
fn a_black_box_caption_stacks_clear_of_its_top_cps() {
    let v = view(
        vec![black_box(
            &[(0, (6, 0), "up"), (1, (12, 0), "up"), (2, (6, 12), "down")],
            &[(0, "L"), (1, "N"), (2, "+")],
        )],
        Vec::new(),
    );
    let r = runs(&v);
    let (tag, part, maker) = (
        run(&r, 0x11, "-101A1").rect,
        run(&r, 0x11, "EW-GEN-PSU-1").rect,
        run(&r, 0x11, "Generic").rect,
    );
    let top = g(30);
    assert!(tag.max().y.0 <= top, "above the box: {tag:?}");
    for (low, high) in [(tag, part), (part, maker)] {
        let gap = low.min().y.0 - high.max().y.0;
        assert!(
            (gap - SLOT_TEXT_GAP.0).abs() <= 1_000,
            "stacked one gap apart: {low:?} {high:?}"
        );
    }
    let right = g(30 + 6) - g(1);
    for t in [tag, part, maker] {
        assert!(t.max().x.0 <= right + 1_000, "left of the first top CP: {t:?}");
        assert!(upright(run(&r, 0x11, "-101A1")));
    }
    // The right edges line up.
    assert_eq!(tag.max().x, part.max().x);
    assert_eq!(part.max().x, maker.max().x);
    for pin in ["L", "N", "+"] {
        let p = run(&r, 0x11, pin).rect;
        for t in [tag, part, maker] {
            assert!(!overlap(t, p), "{t:?} over the pin text {p:?}");
        }
    }
}

/// FT-734: without a CP on its top edge, the tag stands where the symbol puts it (the outline's top-left corner,
/// `BLACK_BOX_CAPTION_GAP` above it) and the part number and manufacturer stack above it, left-aligned with it.
#[test]
fn without_a_top_cp_the_tag_stays_and_the_caption_stacks_above_it() {
    let v = view(
        vec![black_box(&[(0, (0, 6), "left"), (1, (30, 6), "right")], &[])],
        Vec::new(),
    );
    let r = runs(&v);
    let (tag, part, maker) = (
        run(&r, 0x11, "-101A1").rect,
        run(&r, 0x11, "EW-GEN-PSU-1").rect,
        run(&r, 0x11, "Generic").rect,
    );
    // The tag as the symbol places it on a representation that is no black box.
    let mut plain = black_box(&[], &[]);
    Arc::make_mut(&mut plain.rep).box_ = None;
    let alone = runs(&view(vec![plain], Vec::new()));
    assert_eq!(tag, run(&alone, 0x11, "-101A1").rect);
    assert_eq!(tag.min().x.0, g(30), "on the outline's left edge");
    assert!(tag.max().y.0 <= g(30) - BLACK_BOX_CAPTION_GAP.0 + 1_000);
    assert_eq!(part.min().x, tag.min().x);
    assert_eq!(maker.min().x, tag.min().x);
    assert!(part.max().y < tag.min().y && maker.max().y < part.min().y);
    assert!(!run_into(tag, part) && !run_into(part, maker));
}

/// FT-758: a pin text of signs only prints upright ("−" would read as a tick turned); one with a letter or a digit
/// turns with a pin leaving up or down, as before.
#[test]
fn a_pin_text_of_signs_only_prints_upright() {
    let v = view(
        vec![black_box(
            &[(0, (6, 12), "down"), (1, (12, 12), "down"), (2, (18, 12), "down")],
            &[(0, "+"), (1, "\u{2212}"), (2, "1 I0.0")],
        )],
        Vec::new(),
    );
    let r = runs(&v);
    for sign in ["+", "\u{2212}"] {
        let p = run(&r, 0x11, sign);
        assert!(upright(p), "{sign} upright");
        // Centred on its pin, 1 mm inside the box.
        let mid = (p.rect.min().x.0 + p.rect.max().x.0) / 2;
        let pin = if sign == "+" { g(36) } else { g(42) };
        assert!((mid - pin).abs() <= 1_000, "{sign} centred: {mid} {pin}");
        assert!(p.rect.max().y.0 <= g(42) - 1_000_000 + 1_000);
    }
    assert!(!upright(run(&r, 0x11, "1 I0.0")), "a designation still turns");
}

/// A make contact `n` tagged `name` at grid `(x, 20)` with its CP designations 13 and 14.
fn contact(c: &SymbolRef, n: u128, x: i64, name: &str) -> RepView {
    RepView {
        rep: rep(n, c.id, x, 20, json!({})),
        symbol: Some(c.def.clone()),
        slots: vec![
            value(slot(c, &SlotKind::Tag), name),
            value(slot(c, &SlotKind::CpDesignation(0)), "13"),
            value(slot(c, &SlotKind::CpDesignation(1)), "14"),
        ],
        terminals: Vec::new(),
        pins: Vec::new(),
    }
}

/// FT-758 (play-test rv-vis-2, turn 15): three switches in a row, four grid steps apart. Their tags no longer run into
/// each other nor over a neighbour's CP designation; the first stays where the symbol puts it, and each tag stays
/// beside its own symbol.
#[test]
fn neighbouring_tags_do_not_run_into_each_other() {
    let c = symbol(CONTACT_NO);
    let names = [(0x21, 20, "-101S1"), (0x22, 24, "-101S2"), (0x23, 28, "-101S3")];
    let v = view(
        names
            .iter()
            .map(|&(n, x, name)| contact(&c, n, x, name))
            .collect(),
        Vec::new(),
    );
    let r = runs(&v);
    let alone = runs(&view(vec![contact(&c, 0x21, 20, "-101S1")], Vec::new()));
    assert_eq!(
        run(&r, 0x21, "-101S1").rect,
        run(&alone, 0x21, "-101S1").rect,
        "the first tag stays"
    );
    for (i, &(n, x, name)) in names.iter().enumerate() {
        let t = run(&r, n, name).rect;
        for &(m, _, other) in &names[i + 1..] {
            let o = run(&r, m, other).rect;
            assert!(!run_into(t, o), "{name} {t:?} runs into {other} {o:?}");
        }
        for &(m, _, _) in names.iter().filter(|(m, _, _)| *m != n) {
            for d in ["13", "14"] {
                let o = run(&r, m, d).rect;
                assert!(!overlap(t, o), "{name} {t:?} over {d} of {m:#x} {o:?}");
            }
        }
        // Beside its own symbol: within a tag's height of its band, and nearer its own axis than any other.
        let axis = g(x);
        let mid = (t.min().x.0 + t.max().x.0) / 2;
        assert!(
            names
                .iter()
                .all(|&(_, y, _)| (mid - axis).abs() <= (mid - g(y)).abs()),
            "{name} {t:?} nearest its own contact"
        );
    }
}

/// FT-758: tags far enough apart stay where the symbol puts them.
#[test]
fn tags_with_room_stay_put() {
    let c = symbol(CONTACT_NO);
    let v = view(
        vec![contact(&c, 0x21, 20, "-101S1"), contact(&c, 0x22, 32, "-101S2")],
        Vec::new(),
    );
    let r = runs(&v);
    for (n, x, name) in [(0x21, 20, "-101S1"), (0x22, 32, "-101S2")] {
        let alone = runs(&view(vec![contact(&c, n, x, name)], Vec::new()));
        assert_eq!(run(&r, n, name).rect, run(&alone, n, name).rect, "{name}");
        assert!(upright(run(&r, n, name)));
    }
}

fn connection(n: u128, a: (u128, u16), b: (u128, u16)) -> ConnectionView {
    let c: Connection = serde_json::from_value(json!({
        "id": uuid(n), "page": uuid(PAGE),
        "a": {"cp": [uuid(a.0), a.1]}, "b": {"cp": [uuid(b.0), b.1]},
    }))
    .unwrap();
    ConnectionView {
        connection: Arc::new(c),
        marker: None,
    }
}

/// FT-758 (play-test rv-vis-2, turn 33): a valve placed four grid steps right of a vertical wire would have its tag on
/// the wire; the tag moves off it and stays beside its valve, clear of the valve's own texts.
#[test]
fn a_tag_does_not_sit_on_a_wire() {
    let (coil, valve) = (symbol(COIL), symbol(VALVE));
    let k = |n: u128, y: i64| RepView {
        rep: rep(n, coil.id, 40, y, json!({})),
        symbol: Some(coil.def.clone()),
        slots: Vec::new(),
        terminals: Vec::new(),
        pins: Vec::new(),
    };
    let m = RepView {
        rep: rep(0x33, valve.id, 44, 30, json!({})),
        symbol: Some(valve.def.clone()),
        slots: vec![
            value(slot(&valve, &SlotKind::Tag), "-101M1"),
            value(slot(&valve, &SlotKind::TechData), "24 V"),
            value(slot(&valve, &SlotKind::CpDesignation(0)), "A1"),
            value(slot(&valve, &SlotKind::CpDesignation(1)), "A2"),
        ],
        terminals: Vec::new(),
        pins: Vec::new(),
    };
    // -K1's A2 (grid 40, 14) straight down to -K2's A1 (grid 40, 50): a vertical wire at x = grid 40.
    let wired = view(
        vec![k(0x31, 10), k(0x32, 50), m.clone()],
        vec![connection(0x41, (0x1031, 1), (0x1032, 0))],
    );
    let unwired = view(vec![k(0x31, 10), k(0x32, 50), m], Vec::new());
    let before = run(&runs(&unwired), 0x33, "-101M1").rect;
    let wire_x = g(40);
    assert!(
        before.min().x.0 < wire_x && wire_x < before.max().x.0,
        "the case: the tag would sit on the wire {before:?}"
    );
    let r = runs(&wired);
    let tag = run(&r, 0x33, "-101M1").rect;
    assert!(
        !(tag.min().x.0 < wire_x && wire_x < tag.max().x.0),
        "off the wire: {tag:?}"
    );
    for own in ["24 V", "A1", "A2"] {
        assert!(!overlap(tag, run(&r, 0x33, own).rect), "clear of {own}");
    }
    // Beside its valve: between the wire and the valve's axis, level with the valve.
    assert!(tag.max().x.0 < g(44) && tag.min().x.0 > wire_x, "{tag:?}");
    assert!(tag.min().y.0 < g(34) && tag.max().y.0 > g(30));
}

/// Cycle 39 (window check): a cable marker's tag, wider than its mark, used to print centred on it with the mark's
/// sides through the text ("-1[06W]1"); it stands clear of the mark, beside it on the right, reading horizontally.
#[test]
fn a_cable_tag_stands_clear_of_its_mark() {
    let c = symbol(CABLE);
    let v = view(
        vec![RepView {
            rep: rep(0x31, c.id, 40, 40, json!({})),
            symbol: Some(c.def.clone()),
            slots: vec![value(slot(&c, &SlotKind::Tag), "-106W1")],
            terminals: Vec::new(),
            pins: Vec::new(),
        }],
        Vec::new(),
    );
    let r = runs(&v);
    let t = run(&r, 0x31, "-106W1");
    let b = ew_scene::symbol::lower(&c.def, ew_units::Orient2::R0).bounds;
    let mark = Rect::from_points(
        Pt2::new(ew_units::Nm(g(40)) + b.min().x, ew_units::Nm(g(40)) + b.min().y),
        Pt2::new(ew_units::Nm(g(40)) + b.max().x, ew_units::Nm(g(40)) + b.max().y),
    );
    assert!(!overlap(t.rect, mark), "the tag {:?} crosses the mark {mark:?}", t.rect);
    assert!(t.rect.min().x >= mark.max().x, "beside the mark on the right");
    assert!(upright(t));
}
