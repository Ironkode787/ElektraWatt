// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page display lists of a synthetic page (WP-S3.04): CP resolution, wires, junctions, link roles, slot
//! texts, wire markers, findings, layer independence and the sheet list.

use std::sync::Arc;

use ew_dl::{Destination, EntityRef, ItemKind, Layer, Link, PathEl};
use ew_ids::{Id, Uuid};
use ew_mltext::Ident;
use ew_scene::forms::{FrameGeometry, PageBindings, SheetMap};
use ew_scene::page::{
    ConnectionView, FrameView, ItemLink, JunctionView, PageFinding, PageKind, PageView, RepView, SlotValue,
    SymbolRef, page_display_list, page_scene,
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
        grid: ew_units::Grid::QET_10PX,
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
                    item_links: Vec::new(),
                }],
                terminals: Vec::new(),
            },
            RepView {
                rep: rep(0x11, 0xF1, t.id, 10, 20, "r0"),
                symbol: Some(t.def.clone()),
                slots: vec![
                    SlotValue {
                        slot: 0,
                        text: "-X1".into(),
                        link: None,
                        item_links: Vec::new(),
                    },
                    SlotValue {
                        slot: 1,
                        text: String::new(),
                        link: None,
                        item_links: Vec::new(),
                    },
                ],
                terminals: Vec::new(),
            },
            RepView {
                rep: rep(0x13, 0xF3, Id::from_uuid(Uuid::from_u128(0xDEAD)), 60, 60, "r0"),
                symbol: None,
                slots: Vec::new(),
                terminals: Vec::new(),
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
        table: None,
        embedded: Vec::new(),
        cp_targets: Vec::new(),
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

/// The em size of the run printing `text` on the labels layer of `v`.
fn run_size(v: &PageView, text: &str) -> Nm {
    let labels = page_display_list(v, Layer::Labels, bundled());
    labels
        .list
        .items
        .iter()
        .find_map(|i| match &i.kind {
            ItemKind::Glyphs(r) if r.text == text => Some(r.size),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no run {text}"))
}

fn style_size(key: &str) -> Nm {
    ew_text::builtin().resolve(key).expect("a built-in style").size
}

#[test]
fn a_slot_style_replaces_the_style_of_the_slot_kind() {
    let mut v = view();
    // Without the member the tag slot prints in `tag.main`.
    assert_eq!(run_size(&v, "-X1"), style_size("tag.main"));
    // `xref.device` is 7 QET px = 5.25 pt native; the QET sheet map prints it at 5.18 pt (doc 20 §17.5).
    let mut def = (**v.reps[1].symbol.as_ref().unwrap()).clone();
    def.text_slots[0].style = Some(Ident::from_input("xref.device").unwrap());
    v.reps[1].symbol = Some(Arc::new(def));
    assert_eq!(run_size(&v, "-X1"), style_size("xref.device"));
    assert_ne!(style_size("xref.device"), style_size("tag.main"));
    let labels = page_display_list(&v, Layer::Labels, bundled());
    assert!(
        !labels
            .findings
            .iter()
            .any(|f| matches!(f, PageFinding::UnknownSlotStyle { .. })),
        "{:?}",
        labels.findings
    );
}

#[test]
fn an_unknown_slot_style_falls_back_to_the_kind_style_with_a_note() {
    let mut v = view();
    let mut def = (**v.reps[1].symbol.as_ref().unwrap()).clone();
    def.text_slots[0].style = Some(Ident::from_input("no.such.style").unwrap());
    v.reps[1].symbol = Some(Arc::new(def));
    assert_eq!(run_size(&v, "-X1"), style_size("tag.main"));
    let labels = page_display_list(&v, Layer::Labels, bundled());
    let notes: Vec<&PageFinding> = labels
        .findings
        .iter()
        .filter(|f| matches!(f, PageFinding::UnknownSlotStyle { .. }))
        .collect();
    assert_eq!(notes.len(), 1, "{:?}", labels.findings);
    let PageFinding::UnknownSlotStyle {
        entity,
        symbol,
        slot,
        name,
    } = notes[0]
    else {
        unreachable!()
    };
    assert_eq!(*entity, Uuid::from_u128(0x11));
    assert_eq!(*symbol, v.reps[1].rep.symbol.as_uuid());
    assert_eq!(*slot, 0);
    assert_eq!(name, "no.such.style");
}

#[test]
fn a_text_with_several_items_carries_one_link_per_item_over_its_own_span() {
    let mut v = view();
    let text = "∞ 3-B2, 106-G3";
    let first = text.find("3-B2").unwrap();
    let second = text.find("106-G3").unwrap();
    let link = |range: std::ops::Range<usize>, n: u128| ItemLink {
        range,
        dest: Destination::Representation(Uuid::from_u128(n)),
    };
    v.reps[1].slots = vec![SlotValue {
        slot: 0,
        text: text.into(),
        link: None,
        item_links: vec![
            link(first..first + "3-B2".len(), 0x71),
            link(second..second + "106-G3".len(), 0x72),
        ],
    }];
    let labels = page_display_list(&v, Layer::Labels, bundled());
    // The text itself carries no link; each item has an invisible box with its own.
    let mut boxes = Vec::new();
    let mut text_items = Vec::new();
    for item in labels
        .list
        .items
        .iter()
        .filter(|i| i.entity == Some(EntityRef(Uuid::from_u128(0x11))))
    {
        match &item.kind {
            ItemKind::Path(p) => {
                let Some(Link::Source(d)) = item.link else {
                    panic!("a path of the text without a link");
                };
                assert!(p.stroke.is_none() && p.fill.is_none(), "an invisible box");
                let list = ew_dl::DisplayList {
                    items: vec![item.clone()],
                };
                boxes.push((d, list.bounds().unwrap()));
            }
            _ => {
                assert_eq!(item.link, None, "the text itself carries no link");
                text_items.push(item.clone());
            }
        }
    }
    assert_eq!(boxes.len(), 2, "{boxes:?}");
    assert_eq!(boxes[0].0, Destination::Representation(Uuid::from_u128(0x71)));
    assert_eq!(boxes[1].0, Destination::Representation(Uuid::from_u128(0x72)));
    // Disjoint and in reading order, both inside the text's own box, the first right of the "∞ " prefix.
    let whole = ew_dl::DisplayList { items: text_items }.bounds().unwrap();
    assert!(boxes[0].1.max().x < boxes[1].1.min().x, "{boxes:?}");
    assert!(boxes[0].1.min().x > whole.min().x, "after the prefix");
    assert!(boxes[1].1.max().x <= whole.max().x, "inside the text");
}

#[test]
fn a_whole_text_link_and_item_links_are_exclusive() {
    let mut v = view();
    let text = "3-B2, 106-G3";
    let dest = |n: u128| Destination::Representation(Uuid::from_u128(n));
    v.reps[1].slots = vec![SlotValue {
        slot: 0,
        text: text.into(),
        link: Some(dest(0x70)),
        item_links: vec![ItemLink {
            range: 0..4,
            dest: dest(0x71),
        }],
    }];
    let labels = page_display_list(&v, Layer::Labels, bundled());
    let notes: Vec<&PageFinding> = labels
        .findings
        .iter()
        .filter(|f| matches!(f, PageFinding::ConflictingLinks { .. }))
        .collect();
    assert_eq!(notes.len(), 1, "{:?}", labels.findings);
    // The item links win: one link box, and the text items carry no link.
    let links: Vec<(bool, Link)> = labels
        .list
        .items
        .iter()
        .filter(|i| i.entity == Some(EntityRef(Uuid::from_u128(0x11))))
        .filter_map(|i| i.link.map(|l| (matches!(i.kind, ItemKind::Path(_)), l)))
        .collect();
    assert_eq!(links, [(true, Link::Source(dest(0x71)))]);
    // Either kind alone raises no finding.
    for (link, item_links) in [
        (Some(dest(0x70)), Vec::new()),
        (
            None,
            vec![ItemLink {
                range: 0..4,
                dest: dest(0x71),
            }],
        ),
    ] {
        v.reps[1].slots[0].link = link;
        v.reps[1].slots[0].item_links = item_links;
        let labels = page_display_list(&v, Layer::Labels, bundled());
        assert!(
            !labels
                .findings
                .iter()
                .any(|f| matches!(f, PageFinding::ConflictingLinks { .. })),
            "{:?}",
            labels.findings
        );
    }
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

const BLACKBOX: &str = include_str!("../../../content/symbols/qet10/ew.gen.blackbox@qet10.json");
const SEGMENT: &str = include_str!("../../../content/symbols/qet10/ew.gen.strip-segment@qet10.json");

/// A view with only `reps` and `connections` (no frame) on the QET grid.
fn bare(reps: Vec<RepView>, connections: Vec<ConnectionView>) -> PageView {
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
        cp_targets: Vec::new(),
    }
}

fn wire(n: u128, a: serde_json::Value, b: serde_json::Value) -> ConnectionView {
    ConnectionView {
        connection: connection(n, a, b),
        marker: None,
    }
}

fn stroke_widths(list: &ew_dl::DisplayList) -> Vec<i64> {
    list.items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Path(p) => p.stroke.as_ref().map(|s| s.width.0),
            _ => None,
        })
        .collect()
}

#[test]
fn wires_use_the_1_px_pen_on_the_qet_grid_and_the_house_pen_elsewhere() {
    let mut v = view();
    let wires = page_display_list(&v, Layer::Wires, bundled());
    let w = stroke_widths(&wires.list);
    assert_eq!(w.len(), 3);
    assert!(w.iter().all(|&w| w == ew_scene::page::QET_PEN_1PX.0), "{w:?}");
    v.grid = ew_units::Grid::MM4;
    let wires = page_display_list(&v, Layer::Wires, bundled());
    assert!(stroke_widths(&wires.list).iter().all(|&w| w == 260_000));
}

#[test]
fn a_black_box_draws_its_box_cps() {
    let bb = symbol(BLACKBOX);
    let t = symbol(TERMINAL);
    let mut boxed: Representation = serde_json::from_value(json!({
        "id": uuid(0x41), "functions": [uuid(0xB1)], "page": uuid(PAGE),
        "symbol": bb.id.as_uuid().hyphenated().to_string(), "pos": [g(10), g(10)],
        "box": {
            "outline": [[0, 0], [g(10), 0], [g(10), g(6)], [0, g(6)]],
            "cps": [{"idx": 3, "pos": [g(1), 0], "dir": "up"}, {"idx": 7, "pos": [g(3), g(6)], "dir": "down"}],
        },
    }))
    .unwrap();
    let reps = |boxed: Representation| {
        vec![
            RepView {
                rep: Arc::new(boxed),
                symbol: Some(bb.def.clone()),
                slots: Vec::new(),
                terminals: Vec::new(),
            },
            RepView {
                rep: rep(0x42, 0xF1, t.id, 40, 40, "r0"),
                symbol: Some(t.def.clone()),
                slots: Vec::new(),
                terminals: Vec::new(),
            },
        ]
    };
    let conns = vec![
        wire(0x51, cp(0xB1, 3), cp(0xF1, 0)),
        wire(0x52, cp(0xB1, 7), cp(0xF1, 1)),
    ];
    let v = bare(reps(boxed.clone()), conns.clone());
    let lines = polylines(&page_display_list(&v, Layer::Wires, bundled()).list);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].1.first(), Some(&p(11, 10)));
    assert_eq!(lines[1].1.first(), Some(&p(13, 16)));
    // The box CPs turn with the representation: R90 maps (x, y) to (-y, x).
    boxed.orient = ew_units::Orient2::R90;
    let v = bare(reps(boxed.clone()), conns.clone());
    let lines = polylines(&page_display_list(&v, Layer::Wires, bundled()).list);
    assert_eq!(lines[0].1.first(), Some(&p(10, 11)));
    // Without a box nothing is drawn: both connections have an unresolved end.
    boxed.box_ = None;
    let v = bare(reps(boxed), conns);
    let wires = page_display_list(&v, Layer::Wires, bundled());
    assert!(polylines(&wires.list).is_empty());
    assert_eq!(wires.findings.len(), 2);
}

#[test]
fn a_strip_segment_draws_one_point_per_terminal_and_side() {
    use ew_scene::page::SegmentTerminal;
    let seg = symbol(SEGMENT);
    let t = symbol(TERMINAL);
    let terminal = |f: u128| SegmentTerminal {
        function: Id::from_uuid(Uuid::from_u128(f)),
        side1: vec![0],
        side2: vec![1, 2],
    };
    let v = bare(
        vec![
            RepView {
                rep: rep(0x61, 0xC0, seg.id, 21, 9, "r0"),
                symbol: Some(seg.def.clone()),
                slots: Vec::new(),
                terminals: vec![terminal(0xC1), terminal(0xC2), terminal(0xC3)],
            },
            RepView {
                rep: rep(0x62, 0xF1, t.id, 60, 60, "r0"),
                symbol: Some(t.def.clone()),
                slots: Vec::new(),
                terminals: Vec::new(),
            },
        ],
        vec![
            wire(0x71, cp(0xC1, 0), cp(0xF1, 0)),
            wire(0x72, cp(0xC3, 2), cp(0xF1, 1)),
            wire(0x73, cp(0xC2, 1), cp(0xF1, 1)),
            wire(0x74, cp(0xC2, 3), cp(0xF1, 1)),
        ],
    );
    let wires = page_display_list(&v, Layer::Wires, bundled());
    let lines = polylines(&wires.list);
    assert_eq!(lines.len(), 3);
    // Terminal k at x = (k + 1) grid steps right of the origin; side 1 on y = 0, side 2 on the segment's
    // bottom edge (3 grid steps, the height of the dashed rectangle).
    let q = |x: i64, y: i64| {
        let at = |k| ew_units::Grid::QET_10PX.at(k).unwrap();
        Pt2::new(at(x), at(y))
    };
    assert_eq!(lines[0].1.first(), Some(&q(22, 9)));
    assert_eq!(lines[1].1.first(), Some(&q(24, 12)));
    assert_eq!(lines[2].1.first(), Some(&q(23, 12)));
    // A CP the caller lists on no side (here idx 3) is not drawn.
    assert_eq!(
        wires.findings,
        [PageFinding::UnresolvedEnd {
            connection: Id::from_uuid(Uuid::from_u128(0x74))
        }]
    );
}

#[test]
fn cp_targets_are_named_destinations_at_the_cp_else_at_the_representation() {
    let t = symbol(TERMINAL);
    let reps = vec![RepView {
        rep: rep(0x42, 0xF1, t.id, 40, 40, "r0"),
        symbol: Some(t.def.clone()),
        slots: Vec::new(),
        terminals: Vec::new(),
    }];
    let mut v = bare(reps, Vec::new());
    let target = |idx: u16| ew_scene::page::CpTarget {
        rep: Id::from_uuid(Uuid::from_u128(0x42)),
        function: Id::from_uuid(Uuid::from_u128(0xF1)),
        idx,
    };
    v.cp_targets = vec![target(9), target(0), target(0)];
    let list = page_display_list(&v, Layer::Symbols, bundled()).list;
    let dest = |idx: u16| {
        let cp: Id<()> = ew_ids::v5::cp_id(Id::<()>::from_uuid(Uuid::from_u128(0xF1)), u32::from(idx));
        Destination::Cp(cp.as_uuid())
    };
    let at = |idx: u16| -> Vec<Pt2> {
        list.items
            .iter()
            .filter(|i| i.link == Some(Link::Target(dest(idx))))
            .filter_map(|i| match &i.kind {
                ItemKind::Group(g) => g.transform.map(|t| t.translate),
                _ => None,
            })
            .collect()
    };
    // CP 0 is drawn by the terminal symbol (once, deduplicated); CP 9 is not, so it sits at the anchor.
    assert_eq!(at(0).len(), 1);
    assert_ne!(at(0), vec![p(40, 40)]);
    assert_eq!(at(9), vec![p(40, 40)]);
}
