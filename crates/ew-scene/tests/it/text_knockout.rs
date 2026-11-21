// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A text a wire runs through stays readable (FT-1786): the labels layer names the texts a line of the page runs
//! through, and the wires layer leaves the line out across each, with the gap on both sides; a page where no line runs
//! through a text draws its wires as before.

use std::sync::Arc;

use ew_dl::{DisplayList, ItemKind, Layer, PathEl};
use ew_ids::Uuid;
use ew_scene::items::ItemView;
use ew_scene::knockout::KNOCKOUT_GAP;
use ew_scene::page::{ConnectionView, JunctionView, PageView, page_display_list};
use ew_schema::{Connection, Junction, PageItem};
use ew_text::{builtin, bundled};
use ew_units::{Nm, Pt2, Rect};
use serde_json::json;

use crate::common::blank_page;

const PAGE: u128 = 0x100;
const WIRE: u128 = 0x21;
const MM: i64 = 1_000_000;

fn uuid(n: u128) -> String {
    Uuid::from_u128(n).hyphenated().to_string()
}

/// A junction dot `n` at `(x, y)` mm.
fn junction(n: u128, x: i64, y: i64) -> JunctionView {
    let j: Junction = serde_json::from_value(json!({
        "id": uuid(n), "page": uuid(PAGE), "at": [x * MM, y * MM], "style": "dot", "sequence": [uuid(WIRE)],
    }))
    .unwrap();
    JunctionView {
        junction: Arc::new(j),
        symbol: None,
    }
}

/// A page with a vertical wire from (110, 40) to (110, 80) mm between two junctions, and a free text "CONTROL
/// CIRCUIT" whose start stands at `text_x` mm, 60 mm down, aligned left.
fn page(text_x: i64) -> PageView {
    let wire: Connection = serde_json::from_value(json!({
        "id": uuid(WIRE), "page": uuid(PAGE),
        "a": {"junction": uuid(0x31)}, "b": {"junction": uuid(0x32)},
        "route": [[110 * MM, 40 * MM], [110 * MM, 80 * MM]],
    }))
    .unwrap();
    let text: PageItem = serde_json::from_value(json!({
        "id": uuid(0x41), "page": uuid(PAGE),
        "kind": {"free_text": {"text": "CONTROL CIRCUIT", "at": [text_x * MM, 60 * MM], "align": "left"}},
    }))
    .unwrap();
    PageView {
        connections: vec![ConnectionView {
            connection: Arc::new(wire),
            marker: None,
            number: None,
        }],
        junctions: vec![junction(0x31, 110, 40), junction(0x32, 110, 80)],
        items: vec![ItemView {
            item: Arc::new(text),
            text: "CONTROL CIRCUIT".to_owned(),
            shown: true,
            asset: None,
            cable: None,
        }],
        ..blank_page(PAGE)
    }
}

/// The wires layer of `v` with the labels layer's boxes, as every writer draws it.
fn wires(v: &PageView) -> DisplayList {
    let labels = page_display_list(v, Layer::Labels, bundled(), builtin());
    let v = PageView {
        knockouts: labels.knockouts,
        ..v.clone()
    };
    page_display_list(&v, Layer::Wires, bundled(), builtin()).list
}

/// The elements of the wire's path.
fn wire_path(list: &DisplayList) -> Vec<PathEl> {
    list.items
        .iter()
        .find(|i| i.entity.map(|e| e.0) == Some(Uuid::from_u128(WIRE)))
        .and_then(|i| match &i.kind {
            ItemKind::Path(p) => Some(p.path.els.clone()),
            _ => None,
        })
        .expect("the wire")
}

/// The box of the free text where the labels layer prints it: its glyph runs' layout boxes.
fn text_box(v: &PageView) -> Rect {
    let labels = page_display_list(v, Layer::Labels, bundled(), builtin());
    labels
        .list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => Some(run.layout_quad()),
            _ => None,
        })
        .flatten()
        .map(Rect::from_point)
        .reduce(Rect::union)
        .expect("the text")
}

#[test]
fn a_wire_through_a_fixed_text_is_cut_round_it() {
    let v = page(100);
    let b = text_box(&v);
    assert!(
        b.min().x < Nm(110 * MM) && Nm(110 * MM) < b.max().x,
        "the text spans the wire: {b:?}"
    );
    let labels = page_display_list(&v, Layer::Labels, bundled(), builtin());
    assert_eq!(labels.knockouts, [b], "the labels layer names the text the wire runs through");
    let x = Nm(110 * MM);
    assert_eq!(
        wire_path(&wires(&v)),
        [
            PathEl::MoveTo(Pt2::new(x, Nm(40 * MM))),
            PathEl::LineTo(Pt2::new(x, b.min().y - KNOCKOUT_GAP)),
            PathEl::MoveTo(Pt2::new(x, b.max().y + KNOCKOUT_GAP)),
            PathEl::LineTo(Pt2::new(x, Nm(80 * MM))),
        ]
    );
}

#[test]
fn a_page_where_no_wire_runs_through_a_text_draws_its_wires_as_before() {
    // The text ends left of the wire.
    let v = page(40);
    assert!(text_box(&v).max().x < Nm(110 * MM));
    let labels = page_display_list(&v, Layer::Labels, bundled(), builtin());
    assert!(labels.knockouts.is_empty(), "{:?}", labels.knockouts);
    let plain = page_display_list(&v, Layer::Wires, bundled(), builtin()).list;
    let bytes = |list: &DisplayList| {
        let mut c = ew_dl::Canonical::new();
        c.display_list(list);
        c.into_bytes()
    };
    assert_eq!(
        bytes(&wires(&v)),
        bytes(&plain),
        "the wires layer is the one drawn without knock-outs, byte for byte"
    );
    assert_eq!(
        wire_path(&plain),
        [
            PathEl::MoveTo(Pt2::new(Nm(110 * MM), Nm(40 * MM))),
            PathEl::LineTo(Pt2::new(Nm(110 * MM), Nm(80 * MM))),
        ]
    );
}
