// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page items and structure boxes drawn (WP-S2.04 part 4a, §10.3): graphics and box outlines in the symbols layer,
//! free texts, wire-marker groups and captions in the labels layer, each tagged with its entity, in ID order, and a
//! page without items unchanged.

use std::sync::Arc;

use ew_dl::{Brush, EntityRef, ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_scene::items::{DASH_DOT, DOTTED, ItemView, StructureBoxView};
use ew_scene::page::{PageKind, PageView, SymbolRef, page_display_list, page_scene, wire_stroke};
use ew_schema::{LibCopy, LibDocument, PageItem, StructureBox};
use ew_text::bundled;
use ew_units::{Grid, Nm};
use serde_json::{Value, json};

const MARKER: &str = include_str!("../../../content/symbols/qet10/ew.gen.wire-marker@qet10.json");
const PAGE: u128 = 0x100;

fn uuid(n: u128) -> String {
    Uuid::from_u128(n).hyphenated().to_string()
}

fn item(n: u128, kind: Value, text: &str, shown: bool) -> ItemView {
    let item: PageItem =
        serde_json::from_value(json!({"id": uuid(n), "page": uuid(PAGE), "kind": kind})).unwrap();
    ItemView {
        item: Arc::new(item),
        text: text.to_owned(),
        shown,
    }
}

fn sbox(n: u128, style: &str, caption: &str) -> StructureBoxView {
    let b: StructureBox = serde_json::from_value(json!({
        "id": uuid(n), "page": uuid(PAGE), "style": style, "assigns": {},
        "outline": [[20_000_000, 40_000_000], [80_000_000, 40_000_000], [80_000_000, 60_000_000], [20_000_000, 60_000_000]],
    }))
    .unwrap();
    StructureBoxView {
        sbox: Arc::new(b),
        caption: caption.to_owned(),
    }
}

fn marker_symbol() -> SymbolRef {
    match LibDocument::from_json(MARKER).unwrap() {
        LibDocument::Symbol(LibCopy { id, item, .. }) => SymbolRef {
            id,
            def: Arc::new(item),
        },
        _ => panic!("a symbol"),
    }
}

fn view(items: Vec<ItemView>, structure_boxes: Vec<StructureBoxView>) -> PageView {
    PageView {
        page: Id::from_uuid(Uuid::from_u128(PAGE)),
        kind: PageKind::Schematic,
        grid: Grid::QET_10PX,
        language: None,
        frame: None,
        reps: Vec::new(),
        connections: Vec::new(),
        junctions: Vec::new(),
        wire_marker: Some(marker_symbol()),
        table: None,
        embedded: Vec::new(),
        break_marks: Vec::new(),
        cp_targets: Vec::new(),
        crossings: Default::default(),
        items,
        structure_boxes,
    }
}

fn entity(n: u128) -> Option<EntityRef> {
    Some(EntityRef(Uuid::from_u128(n)))
}

fn sample() -> PageView {
    view(
        vec![
            item(
                0x12,
                json!({"graphic": {"shape": {"rect": {"min": [0, 0], "max": [10_000_000, 5_000_000]}}, "fill": true}}),
                "",
                false,
            ),
            item(
                0x11,
                json!({"graphic": {"shape": {"line": {"a": [0, 0], "b": [10_000_000, 0]}}, "stroke": {"width": 500_000}, "dash": "dashed"}}),
                "",
                false,
            ),
            item(
                0x13,
                json!({"free_text": {"text": "40x60", "at": [50_800_000, 30_480_000], "angle": 180_000, "align": "right"}}),
                "40x60",
                true,
            ),
            item(
                0x14,
                json!({"wire_marker": {"connections": [uuid(0x99)], "at": [90_000_000, 24_000_000], "angle": 90_000, "fields": ["csa", "colour"]}}),
                "2.5 BK BU",
                true,
            ),
            item(
                0x15,
                json!({"wire_marker": {"connections": [uuid(0x98)], "at": [90_000_000, 34_000_000], "fields": ["csa", "colour"]}}),
                "1.5 DBU",
                false,
            ),
            item(0x16, json!({"macro_box": {"key": "m"}}), "", false),
        ],
        vec![
            sbox(0x21, "dash_dot", "External Connection (+J-E-206 / JE-N1000-03)"),
            sbox(0x20, "dotted", ""),
        ],
    )
}

/// Graphics in ID order with their stroke, dash and fill, then the box outlines with their dash patterns; each tagged
/// with its record.
#[test]
fn graphics_and_box_outlines_are_drawn_in_the_symbols_layer() {
    let l = page_display_list(&sample(), Layer::Symbols, bundled());
    let paths: Vec<_> = l
        .list
        .items
        .iter()
        .map(|i| match &i.kind {
            ItemKind::Path(p) => (i.entity, p.stroke.clone(), p.fill),
            other => panic!("a path, not {other:?}"),
        })
        .collect();
    assert_eq!(
        paths.len(),
        4,
        "two graphics and two boxes; the unknown variant draws nothing"
    );
    // The line: its own width, dashed 4:2.
    assert_eq!(paths[0].0, entity(0x11));
    let line = paths[0].1.as_ref().unwrap();
    assert_eq!(line.width, Nm(500_000));
    assert_eq!(
        line.dash.as_ref().map(|d| d.segments.clone()),
        Some(vec![Nm(2_000_000), Nm(1_000_000)])
    );
    // The filled rectangle in the page's wire stroke.
    assert_eq!(paths[1].0, entity(0x12));
    assert_eq!(
        paths[1].1.as_ref().unwrap().width,
        wire_stroke(Grid::QET_10PX).width
    );
    assert_eq!(paths[1].2, Some(Brush::BLACK));
    // The boxes by ID: dotted, then dash-dot, in the wire stroke width.
    assert_eq!(paths[2].0, entity(0x20));
    assert_eq!(
        paths[2].1.as_ref().unwrap().dash.as_ref().unwrap().segments,
        DOTTED.to_vec()
    );
    assert_eq!(paths[3].0, entity(0x21));
    assert_eq!(
        paths[3].1.as_ref().unwrap().dash.as_ref().unwrap().segments,
        DASH_DOT.to_vec()
    );
    assert_eq!(
        paths[3].1.as_ref().unwrap().width,
        wire_stroke(Grid::QET_10PX).width
    );
}

/// The free text, the shown marker group (symbol and text) and the caption are drawn in the labels layer; the hidden
/// group and the empty caption draw nothing.
#[test]
fn texts_markers_and_captions_are_drawn_in_the_labels_layer() {
    let l = page_display_list(&sample(), Layer::Labels, bundled());
    assert!(l.findings.is_empty(), "{:?}", l.findings);
    let entities: Vec<Option<EntityRef>> = l.list.items.iter().map(|i| i.entity).collect();
    assert!(entities.contains(&entity(0x13)), "the free text");
    assert!(entities.contains(&entity(0x14)), "the shown marker group");
    assert!(
        !entities.contains(&entity(0x15)),
        "a group none of whose wires shows a marker"
    );
    assert!(entities.contains(&entity(0x21)), "the caption");
    assert!(
        !entities.contains(&entity(0x20)),
        "an empty caption prints nothing"
    );
    let marker = l
        .list
        .items
        .iter()
        .find(|i| i.entity == entity(0x14) && matches!(i.kind, ItemKind::Instance(_)));
    assert!(marker.is_some(), "the marker symbol under the group text");
    // Text items carry the texts.
    let texts: Vec<String> = l
        .list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => Some(run.text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|t| t == "40x60"), "{texts:?}");
    assert!(texts.iter().any(|t| t == "2.5 BK BU"), "{texts:?}");
    assert!(
        texts.iter().any(|t| t.starts_with("External Connection")),
        "{texts:?}"
    );
}

/// The order of the view's items does not change a display list, and a page without items draws what it drew
/// before (no item in either layer).
#[test]
fn items_are_drawn_in_id_order_and_an_empty_page_is_unchanged() {
    let a = sample();
    let mut b = sample();
    b.items.reverse();
    b.structure_boxes.reverse();
    for layer in [Layer::Symbols, Layer::Labels] {
        assert_eq!(
            page_display_list(&a, layer, bundled()),
            page_display_list(&b, layer, bundled())
        );
    }
    let empty = view(Vec::new(), Vec::new());
    let scene = page_scene(&empty, bundled());
    assert!(scene.layers.iter().all(|(_, l)| l.is_empty()));
}
