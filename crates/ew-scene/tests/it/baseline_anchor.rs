// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The baseline anchor (doc 09 §4.8): a free text with `anchor: baseline` and a symbol text aligned on the baseline
//! stand on their point's line in any typeface; a middle-anchored text's baseline follows the face's cap height; a
//! baseline text turned upright or brought below its point is placed by the top of its box.

use std::sync::Arc;

use ew_dl::{EntityRef, ItemKind, Layer};
use ew_ids::Uuid;
use ew_scene::items::ItemView;
use ew_scene::page::{PageView, page_display_list};
use ew_scene::symbol::placed_align;
use ew_schema::{Align, PageItem};
use ew_text::{FontSet, builtin, bundled, typeface_set};
use ew_units::{Angle2, Orient2};
use serde_json::{Value, json};

use crate::common::blank_page;

const PAGE: u128 = 0x100;
const AT_Y: i64 = 30_480_000;

/// A typeface whose cap height is not Noto Sans's: a bundled file of another role (Routed Gothic, 736 of 1024 units).
fn typeface() -> &'static FontSet {
    let data: Arc<[u8]> = Arc::from(&include_bytes!("../../../../assets/fonts/RoutedGothic-Regular.ttf")[..]);
    typeface_set(&data).unwrap()
}

/// A page of one free text `kind`, printing `text`.
fn page(kind: Value, text: &str) -> PageView {
    let id = Uuid::from_u128(0x11).hyphenated().to_string();
    let page = Uuid::from_u128(PAGE).hyphenated().to_string();
    let item: PageItem = serde_json::from_value(json!({"id": id, "page": page, "kind": kind})).unwrap();
    PageView {
        items: vec![ItemView {
            item: Arc::new(item),
            text: text.to_owned(),
            shown: true,
            asset: None,
            cable: None,
        }],
        ..blank_page(PAGE)
    }
}

/// The baseline of the free text's one line on the page: its run's origin `y`.
fn baseline(view: &PageView, fonts: &FontSet) -> i64 {
    let l = page_display_list(view, Layer::Labels, fonts, builtin());
    let runs: Vec<i64> = l
        .list
        .items
        .iter()
        .filter(|i| i.entity == Some(EntityRef(Uuid::from_u128(0x11))))
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => Some(run.origin.y.0),
            _ => None,
        })
        .collect();
    assert_eq!(runs.len(), 1, "one line in one face");
    runs[0]
}

#[test]
fn a_baseline_text_stands_on_its_point_in_any_typeface() {
    let on_baseline = page(
        json!({"free_text": {"text": "K1 13", "at": [50_800_000, AT_Y], "anchor": "baseline"}}),
        "K1 13",
    );
    assert_eq!(baseline(&on_baseline, bundled()), AT_Y);
    assert_eq!(baseline(&on_baseline, typeface()), AT_Y);
    // Centred and right-aligned texts stand on it too.
    for align in ["centre", "right"] {
        let v = page(
            json!({"free_text": {"text": "K1 13", "at": [50_800_000, AT_Y], "anchor": "baseline", "align": align}}),
            "K1 13",
        );
        assert_eq!(baseline(&v, typeface()), AT_Y, "{align}");
    }
}

/// Without the member (a 0.3.0 file) or with `middle` the text is centred as before: below its point by half the cap
/// height of the face that prints it, so the typeface moves it.
#[test]
fn a_middle_text_follows_the_faces_cap_height() {
    for anchor in [json!(null), json!("middle")] {
        let mut kind = json!({"free_text": {"text": "K1 13", "at": [50_800_000, AT_Y]}});
        if !anchor.is_null() {
            kind["free_text"]["anchor"] = anchor;
        }
        let v = page(kind, "K1 13");
        let noto = baseline(&v, bundled());
        let other = baseline(&v, typeface());
        assert!(noto > AT_Y && other > AT_Y, "below the point: {noto} {other}");
        assert_ne!(noto, other, "the cap heights differ, so the baselines do");
    }
}

/// Turned half round, the text is drawn upright and its body stays below the point's line, where the turned text
/// had it: placed by the top of its box.
#[test]
fn a_turned_baseline_text_keeps_its_body_on_its_side() {
    let turned = page(
        json!({"free_text": {"text": "K1 13", "at": [50_800_000, AT_Y], "angle": 180_000, "anchor": "baseline"}}),
        "K1 13",
    );
    assert!(baseline(&turned, bundled()) > AT_Y);
    assert_eq!(
        placed_align(&Align::BaselineLeft, Angle2(180_000), Orient2::R0),
        Align::TopRight
    );
}

/// A symbol's baseline text keeps the baseline where its body stays above the point, and becomes a top or middle
/// alignment where the variant brings the body below or beside it.
#[test]
fn a_symbol_texts_baseline_follows_the_variant() {
    let cases = [
        (Orient2::R0, Align::BaselineLeft),
        (Orient2::M0, Align::BaselineRight),
        (Orient2::R180, Align::TopRight),
        (Orient2::R90, Align::TopLeft),
    ];
    for (variant, want) in cases {
        assert_eq!(
            placed_align(&Align::BaselineLeft, Angle2::ZERO, variant),
            want,
            "{variant:?}"
        );
    }
    assert_eq!(
        placed_align(&Align::BaselineCentre, Angle2::ZERO, Orient2::R0),
        Align::BaselineCentre
    );
}
