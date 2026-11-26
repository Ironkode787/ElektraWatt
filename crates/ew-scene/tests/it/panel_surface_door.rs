// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A door's page (doc 12 §5.2): the surface layer draws the sheet, the margin, the hinge's ticks outside the hinge
//! edge with their word, the datum's "0" at its corner and the caption under the drawing; the plate's footprints behind
//! the door are in the highlights layer, which no export reads.

use ew_dl::{DisplayList, Item, ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_scene::page::{PageKind, PageView, page_display_list};
use ew_scene::panel::{PanelView, Projection, SurfaceOutline};
use ew_schema::Scale;
use ew_text::{builtin, bundled};
use ew_units::{Nm, Pt2, Rect};

use crate::common::blank_page;

/// The bounds of one item.
fn bounds(i: &Item) -> Option<Rect> {
    DisplayList {
        items: vec![i.clone()],
    }
    .bounds()
}

fn mm(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm(x * 1_000_000), Nm(y * 1_000_000))
}

/// A 600 × 800 mm door at 1:5 from the datum (100, 250) mm, hinged on the `right` edge or the left.
fn door(right: bool) -> PageView {
    let sheet = Rect::from_points(mm(0, 0), mm(600, 800));
    PageView {
        kind: PageKind::Panel,
        panel: Some(PanelView {
            item: Id::from_uuid(Uuid::from_u128(0x71)),
            projection: Projection::of(mm(100, 250), &Scale::OneTo(5)),
            surface: Some(SurfaceOutline {
                surface: Id::from_uuid(Uuid::from_u128(0x72)),
                enclosure: None,
                plate: None,
                usable: Vec::new(),
                sheet: Some(sheet),
                margin: Some(Rect::from_points(mm(30, 30), mm(570, 770))),
                hinge: Some((right, "Hinge".to_owned())),
                datum: Some((mm(0, 0), (1, 1), "0".to_owned())),
                caption: Some("Door, seen from outside".to_owned()),
            }),
            bounds: Some(sheet),
            carriers: Vec::new(),
            placements: Vec::new(),
            brackets: Vec::new(),
            labels: Vec::new(),
            show: Default::default(),
            linked: false,
            behind: vec![Rect::from_points(mm(100, 500), mm(118, 590))],
        }),
        ..blank_page(0x01)
    }
}

/// The texts of a layer with where each run starts.
fn texts(v: &PageView, l: Layer) -> Vec<(String, Rect)> {
    page_display_list(v, l, bundled(), builtin())
        .list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(r) => Some((r.text.clone(), bounds(i)?)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_door_s_page_draws_its_hinge_on_its_hinge_side_and_its_caption_under_it() {
    // The sheet on the page: x 100 … 220 mm, y 90 … 250 mm (paper y down).
    let left = texts(&door(false), Layer::Surface);
    let word = |t: &[(String, Rect)], s: &str| t.iter().find(|(x, _)| x == s).map(|(_, r)| *r).unwrap();
    assert!(
        word(&left, "Hinge").max().x <= Nm(100_000_000),
        "left of the left edge: {left:?}"
    );
    let right = texts(&door(true), Layer::Surface);
    assert!(
        word(&right, "Hinge").min().x >= Nm(220_000_000),
        "right of the right edge"
    );
    let caption = word(&left, "Door, seen from outside");
    assert!(caption.min().y > Nm(250_000_000), "under the drawing");
    let zero = word(&left, "0");
    assert!(
        zero.max().x <= Nm(100_000_000) && zero.min().y >= Nm(250_000_000),
        "outside the corner"
    );
}

#[test]
fn the_plate_behind_the_door_is_a_highlight_only() {
    let v = door(false);
    let behind = page_display_list(&v, Layer::Highlights, bundled(), builtin()).list;
    assert_eq!(behind.items.len(), 1, "the plate's footprint");
    assert!(!Layer::Highlights.is_exported());
    for l in [Layer::Surface, Layer::Placements] {
        let list = page_display_list(&v, l, bundled(), builtin()).list;
        assert!(
            list.items.iter().all(|i| bounds(i) != bounds(&behind.items[0])),
            "{l:?} draws no plate footprint"
        );
    }
}
