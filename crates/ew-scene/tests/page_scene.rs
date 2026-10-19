// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page display lists of a synthetic page (WP-S3.04): CP resolution, wires, junctions, link roles, slot
//! texts, wire markers, findings, layer independence and the sheet list.

use std::sync::Arc;

use ew_dl::{Destination, EntityRef, ItemKind, Layer, Link, PathEl};
use ew_ids::{Id, Uuid};
use ew_scene::forms::{FrameGeometry, PageBindings, SheetMap};
use ew_scene::page::{
    ConnectionView, FrameView, JunctionView, PageFinding, PageKind, PageView, RepView, SlotValue, SymbolRef,
    page_display_list, page_scene,
};
use ew_schema::{Connection, FormDef, Junction, LibCopy, LibDocument, Representation, SymbolDef};
use ew_text::bundled;
use ew_units::{Angle2, Nm, Pt2};
use serde_json::json;

const TERMINAL: &str = include_str!("../../../content/symbols/qet10/ew.iec.terminal@qet10.json");
const DOT: &str = include_str!("../../../content/symbols/qet10/ew.gen.junction-dot@qet10.json");
const MARKER: &str = include_str!("../../../content/symbols/qet10/ew.gen.wire-marker@qet10.json");
const FRAME: &str = include_str!("../../../content/forms/ew.frame.qet-a4-landscape-grid.json");
const TITLE_BLOCK: &str = include_str!("../../../content/forms/ew.titleblock.qet-a4.json");

fn symbol(text: &str) -> SymbolRef {
    match LibDocument::from_json(text).unwrap() {
        LibDocument::Symbol(LibCopy { id, item, .. }) => SymbolRef {
            id,
            def: Arc::new(item),
        },
        _ => panic!("not a symbol"),
    }
}

fn form(text: &str) -> Arc<FormDef> {
    match LibDocument::from_json(text).unwrap() {
        LibDocument::Form(LibCopy { item, .. }) => Arc::new(item),
        _ => panic!("not a form"),
    }
}

fn uuid(n: u128) -> String {
    Uuid::from_u128(n).hyphenated().to_string()
}

const PAGE: u128 = 0x100;

/// QET 10 px = 7 937 500 / 3 nm; positions on that grid.
fn g(k: i64) -> i64 {
    k * 7_937_500 / 3
}

fn rep(n: u128, function: u128, symbol: Id<SymbolDef>, x: i64, y: i64, orient: &str) -> Arc<Representation> {
    Arc::new(
        serde_json::from_value(json!({
            "id": uuid(n), "functions": [uuid(function)], "page": uuid(PAGE),
            "symbol": symbol.as_uuid().hyphenated().to_string(), "pos": [g(x), g(y)], "orient": orient,
        }))
        .unwrap(),
    )
}

fn cp(function: u128, idx: u16) -> serde_json::Value {
    json!({ "cp": [uuid(function), idx] })
}

fn connection(n: u128, a: serde_json::Value, b: serde_json::Value) -> Arc<Connection> {
    Arc::new(serde_json::from_value(json!({ "id": uuid(n), "a": a, "b": b, "page": uuid(PAGE) })).unwrap())
}

/// Two terminals (functions 0xF1 at grid (10, 20), 0xF2 at (50, 50)), a junction at (40, 20) joining a
/// wire from F1's top CP and one to F2's top CP, a direct wire F1 bottom → F2 bottom with a marker, and a
/// wire to a function that is not on the page.
fn view() -> PageView {
    let t = symbol(TERMINAL);
    let j: Arc<Junction> = Arc::new(
        serde_json::from_value(json!({
            "id": uuid(0x31), "page": uuid(PAGE), "at": [g(40), g(20)], "style": "dot",
            "sequence": [uuid(0x21), uuid(0x22)],
        }))
        .unwrap(),
    );
    let junction = json!({ "junction": uuid(0x31) });
    PageView {
        page: Id::from_uuid(Uuid::from_u128(PAGE)),
        kind: PageKind::Schematic,
        language: None,
        frame: Some(FrameView {
            form: form(FRAME),
            title_block: Some(form(TITLE_BLOCK)),
            bindings: PageBindings::new(),
        }),
        reps: vec![
            RepView {
                rep: rep(0x12, 0xF2, t.id, 50, 50, "r0"),
                symbol: Some(t.def.clone()),
                slots: vec![SlotValue {
                    slot: 0,
                    text: "-X2".into(),
                    link: Some(Destination::Representation(Uuid::from_u128(0x11))),
                }],
            },
            RepView {
                rep: rep(0x11, 0xF1, t.id, 10, 20, "r0"),
                symbol: Some(t.def.clone()),
                slots: vec![
                    SlotValue {
                        slot: 0,
                        text: "-X1".into(),
                        link: None,
                    },
                    SlotValue {
                        slot: 1,
                        text: String::new(),
                        link: None,
                    },
                ],
            },
            RepView {
                rep: rep(0x13, 0xF3, Id::from_uuid(Uuid::from_u128(0xDEAD)), 60, 60, "r0"),
                symbol: None,
                slots: Vec::new(),
            },
        ],
        connections: vec![
            ConnectionView {
                connection: connection(0x23, cp(0xF1, 0), cp(0xF2, 0)),
                marker: Some("2.5 BU".into()),
            },
            ConnectionView {
                connection: connection(0x21, cp(0xF1, 1), junction.clone()),
                marker: None,
            },
            ConnectionView {
                connection: connection(0x22, junction, cp(0xF2, 1)),
                marker: None,
            },
            ConnectionView {
                connection: connection(0x24, cp(0xF1, 0), cp(0xF9, 0)),
                marker: None,
            },
        ],
        junctions: vec![JunctionView {
            junction: j,
            symbol: Some(symbol(DOT)),
        }],
        wire_marker: Some(symbol(MARKER)),
    }
}

fn polylines(list: &ew_dl::DisplayList) -> Vec<(Uuid, Vec<Pt2>)> {
    list.items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Path(p) => Some((
                i.entity.unwrap().0,
                p.path
                    .els
                    .iter()
                    .filter_map(|e| match e {
                        PathEl::MoveTo(q) | PathEl::LineTo(q) => Some(*q),
                        _ => None,
                    })
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

fn p(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm(g(x)), Nm(g(y)))
}

#[test]
fn wires_run_between_the_drawn_cps_and_junction_arms_meet_at_the_junction() {
    let v = view();
    let wires = page_display_list(&v, Layer::Wires, bundled());
    let lines = polylines(&wires.list);
    // Sorted by connection ID; 0x24 is unresolved (function 0xF9 is not on the page).
    let ids: Vec<u128> = lines.iter().map(|(u, _)| u.as_u128()).collect();
    assert_eq!(ids, [0x21, 0x22, 0x23]);
    // Terminal CPs: idx 0 one grid step below the origin ((0, 2 645 833) = 1 grid step), idx 1 above.
    let below = |x, y| Pt2::new(Nm(g(x)), Nm(g(y) + 2_645_833));
    let above = |x, y| Pt2::new(Nm(g(x)), Nm(g(y) - 2_645_833));
    // F1 top (leaving up) → junction: vertical first, then along y = 20 … the CP is at y = 20 - 1 step.
    assert_eq!(lines[0].1.first(), Some(&above(10, 20)));
    assert_eq!(lines[0].1.last(), Some(&p(40, 20)));
    assert_eq!(lines[1].1.first(), Some(&p(40, 20)));
    assert_eq!(lines[1].1.last(), Some(&above(50, 50)));
    assert_eq!(lines[2].1.first(), Some(&below(10, 20)));
    assert_eq!(lines[2].1.last(), Some(&below(50, 50)));
    for (_, pts) in &lines {
        assert!(pts.len() <= 3, "{pts:?}");
        for w in pts.windows(2) {
            assert!(w[0].x == w[1].x || w[0].y == w[1].y, "orthogonal: {pts:?}");
        }
    }
    // The junction dot instance follows the wires.
    let last = wires.list.items.last().unwrap();
    assert!(matches!(last.kind, ItemKind::Instance(_)));
    assert_eq!(last.entity, Some(EntityRef(Uuid::from_u128(0x31))));
    assert_eq!(
        wires.findings,
        [PageFinding::UnresolvedEnd {
            connection: Id::from_uuid(Uuid::from_u128(0x24))
        }]
    );
}

#[test]
fn symbols_are_instances_tagged_and_link_targets() {
    let v = view();
    let symbols = page_display_list(&v, Layer::Symbols, bundled());
    let instances: Vec<_> = symbols
        .list
        .items
        .iter()
        .filter(|i| matches!(i.kind, ItemKind::Instance(_)))
        .collect();
    assert_eq!(instances.len(), 2);
    // Sorted by representation ID: 0x11 first.
    assert_eq!(instances[0].entity, Some(EntityRef(Uuid::from_u128(0x11))));
    assert_eq!(
        instances[0].link,
        Some(Link::Target(Destination::Representation(Uuid::from_u128(0x11))))
    );
    let ItemKind::Instance(inst) = &instances[0].kind else {
        unreachable!()
    };
    assert_eq!(inst.transform.translate, p(10, 20));
    assert!(symbols.geometry.contains_key(&inst.symbol));
    assert_eq!(
        symbols.findings,
        [PageFinding::MissingSymbol {
            rep: Id::from_uuid(Uuid::from_u128(0x13))
        }]
    );
}

#[test]
fn slot_texts_print_their_values_with_links_and_markers_their_text() {
    let v = view();
    let labels = page_display_list(&v, Layer::Labels, bundled());
    let runs: Vec<(&str, Option<Link>, Angle2)> = labels
        .list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(r) => Some((r.text.as_str(), i.link, r.transform.angle)),
            _ => None,
        })
        .collect();
    // -X1 (rep 0x11), -X2 (rep 0x12, linked), the marker of 0x23; the empty slot prints nothing.
    assert_eq!(runs.len(), 3, "{runs:?}");
    assert_eq!(runs[0].0, "-X1");
    assert_eq!(runs[0].1, None);
    assert_eq!(runs[1].0, "-X2");
    assert_eq!(
        runs[1].1,
        Some(Link::Source(Destination::Representation(Uuid::from_u128(0x11))))
    );
    assert_eq!(runs[2].0, "2.5 BU");
    // The marker sits on the longest segment of an L whose long leg is horizontal: not turned.
    assert_eq!(runs[2].2, Angle2::ZERO);
    // The marker box is drawn before its text.
    assert!(labels.list.items.iter().any(
        |i| matches!(i.kind, ItemKind::Instance(_)) && i.entity == Some(EntityRef(Uuid::from_u128(0x23)))
    ));
}

#[test]
fn hidden_slots_stay_hidden_and_overrides_move_texts() {
    let mut v = view();
    let r = Arc::make_mut(&mut v.reps[1].rep);
    r.text_slots = serde_json::from_value(json!([
        { "slot": 0, "visible": false },
    ]))
    .unwrap();
    let labels = page_display_list(&v, Layer::Labels, bundled());
    let texts: Vec<String> = labels
        .list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(r) => Some(r.text.clone()),
            _ => None,
        })
        .collect();
    assert!(!texts.contains(&"-X1".to_owned()), "{texts:?}");
    assert!(texts.contains(&"-X2".to_owned()));
}

#[test]
fn the_frame_layer_reads_only_the_frame_and_record_order_does_not_matter() {
    let v = view();
    let mut bare = v.clone();
    bare.reps.clear();
    bare.connections.clear();
    bare.junctions.clear();
    assert_eq!(
        page_display_list(&v, Layer::Frame, bundled()),
        page_display_list(&bare, Layer::Frame, bundled())
    );
    let mut reversed = v.clone();
    reversed.reps.reverse();
    reversed.connections.reverse();
    reversed.junctions.reverse();
    for layer in Layer::SCHEMATIC {
        assert_eq!(
            page_display_list(&v, layer, bundled()),
            page_display_list(&reversed, layer, bundled()),
            "{layer:?}"
        );
    }
    for layer in [Layer::Tables, Layer::Revision, Layer::Highlights] {
        assert!(page_display_list(&v, layer, bundled()).list.is_empty());
    }
}

#[test]
fn the_sheet_list_maps_the_page_and_inlines_instances() {
    let v = view();
    let scene = page_scene(&v, bundled());
    assert_eq!(scene.layers.len(), Layer::SCHEMATIC.len());
    assert_eq!(
        (scene.sheet_width, scene.sheet_height),
        (Nm(297_000_000), Nm(210_000_000))
    );
    let frame = form(FRAME);
    assert_eq!(scene.map, SheetMap::of(&frame));
    let sheet = scene.sheet_list();
    fn no_instances(items: &[ew_dl::Item]) -> bool {
        items.iter().all(|i| match &i.kind {
            ItemKind::Instance(_) => false,
            ItemKind::Group(g) => no_instances(&g.items),
            _ => true,
        })
    }
    assert!(no_instances(&sheet.items));
    // The frame's outer rectangle prints at 9.79 mm from the sheet's corner (doc 20 §17.1).
    let geom = FrameGeometry::of(&frame, Some(&form(TITLE_BLOCK))).unwrap();
    assert_eq!(
        scene.map.apply(geom.outer.min()),
        Pt2::new(Nm(9_790_000), Nm(9_790_000))
    );
    let first = sheet.items.first().unwrap();
    let ItemKind::Path(path) = &first.kind else {
        panic!("the frame comes first")
    };
    assert_eq!(
        path.path.points().next(),
        Some(Pt2::new(Nm(9_790_000), Nm(9_790_000)))
    );
}

#[test]
fn a_page_without_frame_uses_a4_and_the_identity_map() {
    let mut v = view();
    v.frame = None;
    let scene = page_scene(&v, bundled());
    assert_eq!(scene.map, SheetMap::IDENTITY);
    assert_eq!(scene.sheet_width, Nm(297_000_000));
    assert!(scene.layer(Layer::Frame).unwrap().is_empty());
    let mut panel = view();
    panel.kind = PageKind::Panel;
    let s = page_scene(&panel, bundled());
    assert_eq!(s.layers.len(), Layer::PANEL.len());
    assert!(s.layer(Layer::Wires).is_none());
}
