// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A black box's caption (tag, part number, manufacturer) stacks clear of its top CPs; a pin text of
//! signs only prints upright; a device tag that runs into a neighbour's tag or a fixed text, or that a wire runs
//! through, moves to a free place beside its symbol (module documentation of `ew_scene::page`, "Black-box captions"
//! and "Device tags").

use std::sync::Arc;

use ew_dl::{EntityRef, ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_scene::page::{ConnectionView, PageView, PinText, RepView, SlotValue, SymbolRef, page_display_list};
use ew_schema::{Connection, LibCopy, LibDocument, Representation, SlotKind, SymbolDef};
use ew_text::{builtin, bundled};
use ew_units::{Pt2, Rect};
use serde_json::json;

use crate::common::blank_page;

const BLACKBOX: &str = include_str!("../../../../content/symbols/qet10/ew.gen.blackbox@qet10.json");
const COIL: &str = include_str!("../../../../content/symbols/qet10/ew.iec.coil@qet10.json");
const VALVE: &str = include_str!("../../../../content/symbols/qet10/ew.iec.valve-coil@qet10.json");
const CONTACT_NO: &str = include_str!("../../../../content/symbols/qet10/ew.iec.contact-no@qet10.json");
const CABLE: &str = include_str!("../../../../content/symbols/qet10/ew.gen.cable-marker@qet10.json");

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
        reps,
        connections,
        ..blank_page(PAGE)
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
    // The text placer keeps an eighth of a grid step between texts (docs/design/06a-drafting.md §1.5).
    let gap = 7_937_500 / 3 / 8 - 1_000;
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
                cell: None,
            })
            .collect(),
    }
}

/// A pin text of signs only prints upright ("−" would read as a tick turned); one with a letter or a digit
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

/// Three switches in a row, four grid steps apart. Their tags no longer run into
/// each other nor over a neighbour's CP designation; the first stays where the symbol puts it, and every tag stays
/// upright.
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
    for (i, &(n, _, name)) in names.iter().enumerate() {
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
        // Upright: a tag is never turned to make room (at four grid steps a horizontal tag cannot always stand
        // nearer its own contact than its neighbour's).
        assert!(upright(run(&r, n, name)), "{name} upright");
    }
}

/// Tags far enough apart stay where the symbol puts them.
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

/// docs/design/06a-drafting.md §5.2: a wire's number stands along its wire, right of a vertical stretch and turned to
/// read from the bottom; on a horizontal stretch above the line, reading across.
#[test]
fn a_wire_number_stands_along_its_wire_turned_on_a_vertical_stretch() {
    let coil = symbol(COIL);
    let k = |n: u128, x: i64, y: i64| RepView {
        rep: rep(n, coil.id, x, y, json!({})),
        symbol: Some(coil.def.clone()),
        slots: Vec::new(),
        terminals: Vec::new(),
        pins: Vec::new(),
    };
    let numbered = |c: ConnectionView, number: &str| ConnectionView {
        number: Some(number.to_owned()),
        ..c
    };
    // -K1's A2 (grid 40, 14) down to -K2's A1 (grid 40, 50); -K2's A2 (40, 54) across to -K3's A1 at (70, 40).
    let v = view(
        vec![k(0x31, 40, 10), k(0x32, 40, 50), k(0x33, 70, 40)],
        vec![
            numbered(routed(0x41, (0x1031, 1), (0x1032, 0), &[(40, 30)]), "101.3"),
            numbered(
                routed(0x42, (0x1032, 1), (0x1033, 0), &[(40, 60), (70, 60), (70, 40)]),
                "101.4",
            ),
        ],
    );
    let r = runs(&v);
    let tall = run(&r, 0x41, "101.3");
    assert!(!upright(tall), "turned on the vertical stretch");
    assert!(tall.rect.min().x.0 > g(40), "right of the wire: {:?}", tall.rect);
    assert!(
        tall.rect.min().y.0 > g(14) && tall.rect.max().y.0 < g(50),
        "along it: {:?}",
        tall.rect
    );
    let flat = run(&r, 0x42, "101.4");
    assert!(upright(flat));
    assert!(flat.rect.max().y.0 < g(60), "above the line: {:?}", flat.rect);
    assert!(
        flat.rect.min().x.0 > g(40) && flat.rect.max().x.0 < g(70),
        "along it: {:?}",
        flat.rect
    );
}

/// A valve placed four grid steps right of a vertical wire would have its tag on
/// the wire; the tag moves off it, stays upright and stands at another of its places, clear of the valve's own texts
/// and pin numbers (a tag is never turned to make room).
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
        vec![routed(0x41, (0x1031, 1), (0x1032, 0), &[(40, 30)])],
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
    assert!(upright(run(&r, 0x33, "-101M1")));
}

/// A rack run: a tag that had to move took a place on a symbol (a valve's tag on its own valve, a turned
/// cylinder's tag on its neighbour's rod). The valve's tag that the wire would run through moves, but not onto its own
/// valve; where a symbol turned by a half turn and another symbol's tag or body meet, the tag moves off the body.
#[test]
fn a_tag_that_moves_lands_on_no_symbol() {
    let (coil, valve) = (symbol(COIL), symbol(VALVE));
    let k = |n: u128, x: i64, y: i64| RepView {
        rep: rep(n, coil.id, x, y, json!({})),
        symbol: Some(coil.def.clone()),
        slots: Vec::new(),
        terminals: Vec::new(),
        pins: Vec::new(),
    };
    let m = RepView {
        rep: rep(0x33, valve.id, 44, 30, json!({})),
        symbol: Some(valve.def.clone()),
        slots: vec![value(slot(&valve, &SlotKind::Tag), "-101M1")],
        terminals: Vec::new(),
        pins: Vec::new(),
    };
    // The body as the layout sees it: the bounds of the symbol's graphics.
    let body = |def: &SymbolDef, x: i64, y: i64| {
        let b = ew_scene::symbol::lower(def, ew_units::Orient2::R0)
            .graphics
            .bounds()
            .expect("graphics");
        Rect::from_points(
            Pt2::new(b.min().x + ew_units::Nm(g(x)), b.min().y + ew_units::Nm(g(y))),
            Pt2::new(b.max().x + ew_units::Nm(g(x)), b.max().y + ew_units::Nm(g(y))),
        )
    };
    let wired = view(
        vec![k(0x31, 40, 10), k(0x32, 40, 50), m],
        vec![routed(0x41, (0x1031, 1), (0x1032, 0), &[(40, 30)])],
    );
    let r = runs(&wired);
    let tag = run(&r, 0x33, "-101M1").rect;
    assert!(
        !overlap(tag, body(&valve.def, 44, 30)),
        "not on its own valve: {tag:?}"
    );
    // Turned by a half turn, its tag goes to its place for that turn; a coil standing there sends it elsewhere.
    let turned = RepView {
        rep: rep(0x35, valve.id, 44, 30, json!({"orient": "r180"})),
        symbol: Some(valve.def.clone()),
        slots: vec![value(slot(&valve, &SlotKind::Tag), "-101M2")],
        terminals: Vec::new(),
        pins: Vec::new(),
    };
    let alone = run(&runs(&view(vec![turned.clone()], Vec::new())), 0x35, "-101M2").rect;
    let on = (
        (alone.min().x.0 + alone.max().x.0) / 2 * 3 / 7_937_500,
        (alone.min().y.0 + alone.max().y.0) / 2 * 3 / 7_937_500,
    );
    let r = runs(&view(vec![k(0x31, on.0, on.1), turned], Vec::new()));
    let tag = run(&r, 0x35, "-101M2").rect;
    assert!(
        !overlap(tag, body(&coil.def, on.0, on.1)),
        "the turned valve's tag is not on the coil: {tag:?}"
    );
    // A coil turned by a half turn whose body comes to lie on the valve's tag sends the tag off it.
    let plain = RepView {
        rep: rep(0x36, valve.id, 44, 30, json!({})),
        symbol: Some(valve.def.clone()),
        slots: vec![value(slot(&valve, &SlotKind::Tag), "-101M3")],
        terminals: Vec::new(),
        pins: Vec::new(),
    };
    let alone = run(&runs(&view(vec![plain.clone()], Vec::new())), 0x36, "-101M3").rect;
    // The coil's turned body centred on the tag.
    let b = ew_scene::symbol::lower(&coil.def, ew_units::Orient2::R180).bounds;
    let grid = |nm: i64| (nm * 3 + 7_937_500 / 2).div_euclid(7_937_500);
    let on = (
        grid((alone.min().x.0 + alone.max().x.0 - b.min().x.0 - b.max().x.0) / 2),
        grid((alone.min().y.0 + alone.max().y.0 - b.min().y.0 - b.max().y.0) / 2),
    );
    let half = RepView {
        rep: rep(0x37, coil.id, on.0, on.1, json!({"orient": "r180"})),
        ..k(0x37, on.0, on.1)
    };
    let turned_body = Rect::from_points(
        Pt2::new(
            b.min().x + ew_units::Nm(g(on.0)),
            b.min().y + ew_units::Nm(g(on.1)),
        ),
        Pt2::new(
            b.max().x + ew_units::Nm(g(on.0)),
            b.max().y + ew_units::Nm(g(on.1)),
        ),
    );
    assert!(
        overlap(alone, turned_body),
        "the case: the turned coil lies on the tag"
    );
    let r = runs(&view(vec![plain, half], Vec::new()));
    let tag = run(&r, 0x36, "-101M3").rect;
    assert!(
        !overlap(tag, turned_body),
        "the tag moved off the turned coil: {tag:?}"
    );
}

/// The labels layer reports each tag's anchor where it prints, after the tag rule. The valve's
/// tag that the wire would run through moves off it (here above its valve, upright), and the reported anchor stands
/// on the moved text's rectangle; the coils print no tag and report none, and no other layer reports tags. Golden A's
/// folio 3, where the rule moves an anchor, is `ew-api`'s `tag_at_is_where_the_labels_layer_prints_the_tag`.
#[test]
fn the_labels_layer_reports_a_moved_tag_where_it_prints() {
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
        ],
        terminals: Vec::new(),
        pins: Vec::new(),
    };
    let wired = view(
        vec![k(0x31, 10), k(0x32, 50), m.clone()],
        vec![routed(0x41, (0x1031, 1), (0x1032, 0), &[(40, 30)])],
    );
    let unwired = view(vec![k(0x31, 10), k(0x32, 50), m], Vec::new());
    let tags = |v: &PageView| page_display_list(v, Layer::Labels, bundled(), builtin()).tags;
    let moved = tags(&wired);
    let id = Uuid::from_u128(0x33);
    assert_eq!(moved.keys().collect::<Vec<_>>(), [&id], "the valve's tag alone");
    let tag = run(&runs(&wired), 0x33, "-101M1").rect;
    assert_ne!(
        tag,
        run(&runs(&unwired), 0x33, "-101M1").rect,
        "the case: the rule moves the tag"
    );
    let at = moved[&id];
    assert!(
        tag.min().x <= at.x && at.x <= tag.max().x && tag.min().y <= at.y && at.y <= tag.max().y,
        "{at:?} on the printed tag {tag:?}"
    );
    assert_eq!(
        page_display_list(&wired, Layer::Symbols, bundled(), builtin()).tags,
        std::collections::BTreeMap::new(),
        "only the labels layer reports tags"
    );
}

/// A cable marker's tag, wider than its mark, used to print centred on it with the mark's
/// sides through the text ("-1[06W]1"); it stands clear of the mark, beside it, reading horizontally: a mark drawn as a
/// symbol has no pins, and its texts keep off its body.
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
    assert!(
        !overlap(t.rect, mark),
        "the tag {:?} crosses the mark {mark:?}",
        t.rect
    );
    assert!(
        t.rect.min().x >= mark.max().x || t.rect.max().x <= mark.min().x,
        "beside the mark"
    );
    assert!(upright(t));
}

/// A black box `n` (function `n + 0x1000`) at grid `(x, y)` with a `w` × `h`-step outline, the given box CPs and pin
/// texts and, when `caption` is set, its tag, part number and manufacturer.
fn boxed(
    n: u128,
    (x, y): (i64, i64),
    (w, h): (i64, i64),
    cps: &[(u16, (i64, i64), &str)],
    pins: &[(u16, &str)],
    caption: Option<[&str; 3]>,
) -> RepView {
    let s = symbol(BLACKBOX);
    let cps: Vec<serde_json::Value> = cps
        .iter()
        .map(|(idx, (x, y), dir)| json!({"idx": idx, "pos": [g(*x), g(*y)], "dir": dir}))
        .collect();
    RepView {
        rep: rep(
            n,
            s.id,
            x,
            y,
            json!({"box": {"outline": [[0, 0], [g(w), 0], [g(w), g(h)], [0, g(h)]], "cps": cps}}),
        ),
        symbol: Some(s.def.clone()),
        slots: caption.map_or_else(Vec::new, |[tag, part, maker]| {
            vec![
                value(slot(&s, &SlotKind::Tag), tag),
                value(slot(&s, &SlotKind::PartNumber), part),
                value(slot(&s, &SlotKind::Manufacturer), maker),
            ]
        }),
        terminals: Vec::new(),
        pins: pins
            .iter()
            .map(|(idx, text)| PinText {
                idx: *idx,
                text: (*text).to_owned(),
                cell: None,
            })
            .collect(),
    }
}

/// A connection `n` from `a` to `b` with the stored route `route` (grid points). A route of interior points only is a
/// stale one: the scene joins each end to it with a straight segment; one point on the line between two
/// aligned ends makes the straight wire. Without a route a connection is unrouted (D3) and no obstacle of a text.
fn routed(n: u128, a: (u128, u16), b: (u128, u16), route: &[(i64, i64)]) -> ConnectionView {
    let route: Vec<(i64, i64)> = route.iter().map(|&(x, y)| (g(x), g(y))).collect();
    routed_nm(n, a, b, &route)
}

/// [`routed`] with the route in page nm.
fn routed_nm(n: u128, a: (u128, u16), b: (u128, u16), route: &[(i64, i64)]) -> ConnectionView {
    let route: Vec<[i64; 2]> = route.iter().map(|&(x, y)| [x, y]).collect();
    let c: Connection = serde_json::from_value(json!({
        "id": uuid(n), "page": uuid(PAGE),
        "a": {"cp": [uuid(a.0), a.1]}, "b": {"cp": [uuid(b.0), b.1]}, "route": route,
    }))
    .unwrap();
    ConnectionView {
        connection: Arc::new(c),
        marker: None,
        number: None,
    }
}

const IP: &str = include_str!("../../../../content/symbols/qet10/ew.gen.ip@qet10.json");

/// One horizontal wire per page height in `ys` (nm), each between two black boxes at grid x 10 and 70 (representations
/// `0x60 + 2k` and `0x61 + 2k`, connection `0x90 + k`).
fn wires_at(ys: &[i64]) -> (Vec<RepView>, Vec<ConnectionView>) {
    let s = symbol(BLACKBOX);
    let mut reps = Vec::new();
    let mut connections = Vec::new();
    for (k, &y) in ys.iter().enumerate() {
        let n = 0x60 + 2 * k as u128;
        for (m, x, cp_x, dir) in [(n, 10, g(2), "right"), (n + 1, 70, 0, "left")] {
            let outline = json!([[0, 0], [g(2), 0], [g(2), y + g(1)], [0, y + g(1)]]);
            reps.push(RepView {
                rep: rep(
                    m,
                    s.id,
                    x,
                    0,
                    json!({"box": {"outline": outline, "cps": [{"idx": 0, "pos": [cp_x, y], "dir": dir}]}}),
                ),
                symbol: Some(s.def.clone()),
                slots: Vec::new(),
                terminals: Vec::new(),
                pins: Vec::new(),
            });
        }
        connections.push(routed_nm(
            0x90 + k as u128,
            (n + 0x1000, 0),
            (n + 0x1001, 0),
            &[(g(10) + g(2), y), (g(70), y)],
        ));
    }
    (reps, connections)
}

/// An interruption point `0x21` at grid `(40, y)` with the cross-reference "102-A1".
fn arrow_at(y: i64) -> RepView {
    let ip = symbol(IP);
    RepView {
        rep: rep(0x21, ip.id, 40, y, json!({})),
        symbol: Some(ip.def.clone()),
        slots: vec![value(slot(&ip, &SlotKind::Xref), "102-A1")],
        terminals: Vec::new(),
        pins: Vec::new(),
    }
}

/// Whether a horizontal wire at `y` runs through `r`.
fn through(r: Rect, y: i64) -> bool {
    r.min().y.0 < y && y < r.max().y.0
}

/// The rectangle of the cross-reference "102-A1" of [`arrow_at`]`(y)` among the wires at `ys`, and with no wire.
fn arrow_label(y: i64, ys: &[i64], extra: (Vec<RepView>, Vec<ConnectionView>)) -> (Rect, Rect) {
    let (mut reps, mut connections) = wires_at(ys);
    reps.push(arrow_at(y));
    let (more_reps, more) = extra;
    reps.extend(more_reps.clone());
    let bare = view(more_reps.into_iter().chain([arrow_at(y)]).collect(), Vec::new());
    connections.extend(more);
    (
        run(&runs(&view(reps, connections)), 0x21, "102-A1").rect,
        run(&runs(&bare), 0x21, "102-A1").rect,
    )
}

/// A coil's two-line contact mirror, which grows down from its first line, is judged where it
/// prints: a wire through its second line lifts the whole list off it, both lines by the same step.
#[test]
fn a_two_line_list_rises_off_a_wire_through_its_second_line() {
    let c = symbol(COIL);
    let coil = RepView {
        rep: rep(0x22, c.id, 40, 20, json!({})),
        symbol: Some(c.def.clone()),
        slots: vec![value(slot(&c, &SlotKind::Xref), "101-B6\n101-D8")],
        terminals: Vec::new(),
        pins: Vec::new(),
    };
    let bare = runs(&view(vec![coil.clone()], Vec::new()));
    let (first, second) = (run(&bare, 0x22, "101-B6").rect, run(&bare, 0x22, "101-D8").rect);
    // Through the second line's lower part, which a box laid out about its middle would not reach.
    let y = second.max().y.0 - (second.max().y.0 - second.min().y.0) / 5;
    assert!(
        !through(first, y),
        "the case: the wire runs through the second line only"
    );
    let (mut reps, connections) = wires_at(&[y]);
    reps.push(coil);
    let r = runs(&view(reps, connections));
    let (one, two) = (run(&r, 0x22, "101-B6").rect, run(&r, 0x22, "101-D8").rect);
    assert!(
        !through(one, y) && !through(two, y),
        "off the wire: {one:?} {two:?}"
    );
    assert!(two.max().y.0 < y, "above it: {two:?}");
    let dy = one.min().y.0 - first.min().y.0;
    assert!(dy < 0, "risen: {dy}");
    assert_eq!(two.min().y.0 - second.min().y.0, dy, "the list keeps its lines");
}

/// A rise that would leave the drawing area (here the sheet: no frame) does not happen; the text stays.
#[test]
fn a_cross_reference_does_not_rise_out_of_the_drawing_area() {
    let (shown, bare) = arrow_label(0, &[g(1)], (Vec::new(), Vec::new()));
    assert!(through(bare, g(1)), "the case: the wire runs through {bare:?}");
    assert_eq!(shown, bare, "it stays");
}

const IP_TARGET: &str = include_str!("../../../../content/symbols/qet10/ew.gen.ip-target@qet10.json");

/// Whether the segment `a`–`b` (grid) meets the inside of `r`.
fn crosses(r: Rect, a: (i64, i64), b: (i64, i64)) -> bool {
    let (x0, x1) = (g(a.0.min(b.0)), g(a.0.max(b.0)));
    let (y0, y1) = (g(a.1.min(b.1)), g(a.1.max(b.1)));
    x0 < r.max().x.0 && r.min().x.0 < x1.max(x0 + 1) && y0 < r.max().y.0 && r.min().y.0 < y1.max(y0 + 1)
}

/// The cross-reference "102-A1" of a point of `symbol_text` at grid (40, 27) wired along `path` (grid, from the
/// point's CP to a black box's CP at its end), and without the wire.
fn point_label(symbol_text: &str, path: &[(i64, i64)]) -> (Rect, Rect) {
    let s = symbol(symbol_text);
    let point = RepView {
        rep: rep(0x21, s.id, 40, 27, json!({})),
        symbol: Some(s.def.clone()),
        slots: vec![value(slot(&s, &SlotKind::Xref), "102-A1")],
        terminals: Vec::new(),
        pins: Vec::new(),
    };
    let (ex, ey) = path[path.len() - 1];
    let (px, py) = path[path.len() - 2];
    // The box's CP faces the wire's last stretch.
    let dir = if py < ey {
        "up"
    } else if py > ey {
        "down"
    } else if px < ex {
        "left"
    } else {
        "right"
    };
    let end = boxed(0x15, (ex - 1, ey - 1), (2, 2), &[(0, (1, 1), dir)], &[], None);
    let bare = run(
        &runs(&view(vec![point.clone(), end.clone()], Vec::new())),
        0x21,
        "102-A1",
    )
    .rect;
    let wire = routed(0x45, (0x1021, 0), (0x1015, 0), &path[1..path.len() - 1]);
    let shown = run(&runs(&view(vec![point, end], vec![wire])), 0x21, "102-A1").rect;
    (shown, bare)
}

/// (FT-1699: "+Supply / -X0" of a supply's entry printed across its own wire.) A point's cross-reference that its own
/// wire runs through takes another of its places (docs/design/06a-drafting.md §5.2), clear of the wire. A wire that
/// leaves upwards or sideways does not run through it, and it stays where its symbol puts it.
#[test]
fn a_points_cross_reference_stands_clear_of_its_own_wire() {
    // Down: from the tail a step on, then down through the text under the wire.
    let down = [(39, 27), (38, 27), (38, 34)];
    let (shown, bare) = point_label(IP, &down);
    assert!(
        down.windows(2).any(|w| crosses(bare, w[0], w[1])),
        "the case: the wire runs through {bare:?}"
    );
    assert!(
        down.windows(2).all(|w| !crosses(shown, w[0], w[1])),
        "clear of the wire: {shown:?}"
    );
    // Up and sideways: the wire does not run through the text; it stays.
    for path in [
        vec![(39, 27), (39, 26), (40, 26), (40, 20)],
        vec![(39, 27), (32, 27)],
    ] {
        let (shown, bare) = point_label(IP, &path);
        assert!(
            path.windows(2).all(|w| !crosses(bare, w[0], w[1])),
            "the case: {path:?}"
        );
        assert_eq!(shown, bare, "it stays: {path:?}");
    }
    // A target's wire leaves the tip and turns down through its text.
    let target = [(40, 27), (41, 27), (41, 34)];
    let (shown, bare) = point_label(IP_TARGET, &target);
    assert!(
        target.windows(2).any(|w| crosses(bare, w[0], w[1])),
        "the case: the wire runs through {bare:?}"
    );
    assert!(
        target.windows(2).all(|w| !crosses(shown, w[0], w[1])),
        "clear of the wire: {shown:?}"
    );
}
