// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A layout page that can be read (doc 12 §5.3, the owner's finding of the first 0.3.0 candidate: at 1:10 the tags of
//! one rail ran into each other): crowded labels move apart, item numbers print in balloons, and a label a person
//! moved stands where he put it with a leader to its device.

use ew_dl::{ItemKind, Layer, PathEl};
use ew_ids::{Id, Uuid};
use ew_scene::page::{LayerList, PageKind, PageView, page_display_list};
use ew_scene::panel::{LabelShow, PanelLabel, PanelView, PlacementDraw, Projection};
use ew_schema::Scale;
use ew_text::{builtin, bundled};
use ew_units::{Nm, Pt2, Rect};

use crate::common::blank_page;

/// A surface point in millimetres.
fn mm(x: f64, y: f64) -> Pt2 {
    Pt2::new(Nm((x * 1e6).round() as i64), Nm((y * 1e6).round() as i64))
}

/// Seven 18 × 45 mm devices side by side on one rail at 1:10 (each 1.8 mm wide on paper), each tagged with a long
/// tag that fits neither inside nor turned; the surface is 600 × 600 mm.
fn row(show: LabelShow, moved: Option<Pt2>) -> PageView {
    let fp = |i: usize| {
        let x = 40.0 + 18.0 * i as f64;
        Rect::from_points(mm(x, 260.0), mm(x + 18.0, 305.0))
    };
    let id = |i: usize| Id::from_uuid(Uuid::from_u128(0x100 + i as u128));
    PageView {
        kind: PageKind::Panel,
        grid: ew_units::Grid::MM4,
        panel: Some(PanelView {
            item: Id::from_uuid(Uuid::from_u128(2)),
            projection: Projection::of(mm(20.0, 120.0), &Scale::OneTo(10)),
            surface: None,
            bounds: Some(Rect::from_points(mm(0.0, 0.0), mm(600.0, 600.0))),
            carriers: Vec::new(),
            placements: (0..7)
                .map(|i| PlacementDraw {
                    placement: id(i),
                    footprint: fp(i),
                    unknown: false,
                    outline: Vec::new(),
                })
                .collect(),
            brackets: Vec::new(),
            labels: (0..7)
                .map(|i| PanelLabel {
                    placement: id(i),
                    text: format!("-10{i}F1"),
                    footprint: fp(i),
                    shaped: None,
                    item: Some(u32::try_from(i + 3).unwrap()),
                    moved: if i == 0 { moved } else { None },
                })
                .collect(),
            show,
        }),
        ..blank_page(0x01)
    }
}

fn labels(view: &PageView) -> LayerList {
    page_display_list(view, Layer::Labels, bundled(), &builtin())
}

fn overlap(a: Rect, b: Rect) -> bool {
    a.min().x < b.max().x && b.min().x < a.max().x && a.min().y < b.max().y && b.min().y < a.max().y
}

/// The texts printed, in paint order.
fn texts(l: &LayerList) -> Vec<String> {
    l.list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => Some(run.text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn crowded_tags_move_apart_and_stay_on_the_surface() {
    let l = labels(&row(LabelShow::Tags, None));
    let boxes: Vec<Rect> = l.panel_labels.values().copied().collect();
    assert_eq!(boxes.len(), 7, "every device keeps its label");
    for (i, a) in boxes.iter().enumerate() {
        for b in &boxes[i + 1..] {
            assert!(!overlap(*a, *b), "{a:?} overlaps {b:?}");
        }
    }
    // Above and below the row both take labels.
    let page = Projection::of(mm(20.0, 120.0), &Scale::OneTo(10));
    let top = page.point(mm(0.0, 305.0)).unwrap().y;
    let bottom = page.point(mm(0.0, 260.0)).unwrap().y;
    assert!(boxes.iter().any(|b| b.max().y <= top), "a label above the row");
    assert!(boxes.iter().any(|b| b.min().y >= bottom), "a label below the row");
    let surface = page
        .rect(Rect::from_points(mm(0.0, 0.0), mm(600.0, 600.0)))
        .unwrap();
    for b in &boxes {
        assert!(
            b.min().x >= surface.min().x && b.max().x <= surface.max().x,
            "{b:?} on the surface"
        );
    }
}

#[test]
fn item_numbers_print_in_balloons_and_both_shows_the_tag_beside_its_balloon() {
    let circles = |l: &LayerList| {
        l.list
            .items
            .iter()
            .filter(|i| match &i.kind {
                // A circle is four cubic arcs.
                ItemKind::Path(p) => p.path.els.iter().filter(|e| matches!(e, PathEl::CubicTo(..))).count() == 4,
                _ => false,
            })
            .count()
    };
    let numbers = labels(&row(LabelShow::ItemNumbers, None));
    assert_eq!(circles(&numbers), 7);
    let printed = texts(&numbers);
    assert!(printed.contains(&"3".to_owned()) && printed.contains(&"9".to_owned()), "{printed:?}");
    assert!(!printed.iter().any(|t| t.starts_with('-')), "no tag with item numbers alone");
    let both = labels(&row(LabelShow::Both, None));
    assert_eq!(circles(&both), 7);
    let printed = texts(&both);
    assert!(printed.contains(&"-100F1".to_owned()) && printed.contains(&"3".to_owned()), "{printed:?}");
}

#[test]
fn a_moved_label_stands_where_it_was_put_with_a_leader() {
    let off = mm(0.0, 120.0);
    let l = labels(&row(LabelShow::Tags, Some(off)));
    let page = Projection::of(mm(20.0, 120.0), &Scale::OneTo(10));
    // The footprint's centre (49, 282.5) mm plus the offset, on the page.
    let at = page.point(mm(49.0, 402.5)).unwrap();
    let id = Uuid::from_u128(0x100);
    let bx = l.panel_labels[&id];
    assert!(
        bx.min().x <= at.x && at.x <= bx.max().x && bx.min().y <= at.y && at.y <= bx.max().y,
        "{bx:?} holds {at:?}"
    );
    let leaders = l
        .list
        .items
        .iter()
        .filter(|i| i.entity.is_some_and(|e| e.0 == id) && matches!(i.kind, ItemKind::Path(_)))
        .count();
    assert_eq!(leaders, 1, "one leader to the device");
}
