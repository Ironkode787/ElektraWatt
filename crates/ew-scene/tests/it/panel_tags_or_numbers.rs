// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tags where they fit, balloons where they do not (doc 12 §5.3, `LabelShow::TagsOrNumbers`), and the links between a
//! layout and its legend page (doc 12 §5.4, `PanelView::linked`).
//!
//! At 1:10 the label style prints a tag of six characters about 6 mm long and 1.5 mm high with its margins (doc 12
//! §5.3's example: "-101F1" at 1.83 mm em). The fixture's three placements state the three cases by their sizes:
//! a 100 × 50 mm contactor is 10 × 5 mm on paper and holds its tag across; an 18 × 90 mm breaker is 1.8 × 9 mm and holds
//! it only turned; a 6 × 45 mm terminal is 0.6 × 4.5 mm and holds it neither way, so it prints its item number in a
//! balloon.

use ew_dl::{Destination, Item, ItemKind, Layer, Link, PathEl};
use ew_ids::{Id, Uuid};
use ew_scene::page::{LayerList, PageKind, PageView, page_display_list};
use ew_scene::panel::{LabelShow, PanelLabel, PanelView, PlacementDraw, Projection};
use ew_schema::{Placement, Scale};
use ew_text::{builtin, bundled};
use ew_units::{Angle2, Nm, Pt2, Rect};

use crate::common::blank_page;

/// A surface point in millimetres.
fn mm(x: f64, y: f64) -> Pt2 {
    Pt2::new(Nm((x * 1e6).round() as i64), Nm((y * 1e6).round() as i64))
}

fn id(n: u128) -> Id<Placement> {
    Id::from_uuid(Uuid::from_u128(0x200 + n))
}

/// The contactor, the breaker and the terminal: (placement, tag, footprint, item number).
fn devices() -> [(Id<Placement>, &'static str, Rect, u32); 3] {
    [
        (
            id(0),
            "-102Q1",
            Rect::from_points(mm(20.0, 300.0), mm(120.0, 350.0)),
            4,
        ),
        (
            id(1),
            "-101F1",
            Rect::from_points(mm(150.0, 260.0), mm(168.0, 350.0)),
            5,
        ),
        (
            id(2),
            "-X1:1",
            Rect::from_points(mm(200.0, 305.0), mm(206.0, 350.0)),
            7,
        ),
    ]
}

fn page(show: LabelShow, linked: bool) -> PageView {
    PageView {
        kind: PageKind::Panel,
        grid: ew_units::Grid::MM4,
        panel: Some(PanelView {
            item: Id::from_uuid(Uuid::from_u128(2)),
            projection: Projection::of(mm(20.0, 120.0), &Scale::OneTo(10)),
            surface: None,
            bounds: Some(Rect::from_points(mm(0.0, 0.0), mm(600.0, 600.0))),
            carriers: Vec::new(),
            placements: devices()
                .iter()
                .map(|(p, _, fp, _)| PlacementDraw {
                    placement: *p,
                    footprint: *fp,
                    unknown: false,
                    outline: Vec::new(),
                })
                .collect(),
            brackets: Vec::new(),
            labels: devices()
                .iter()
                .map(|(p, tag, fp, n)| PanelLabel {
                    placement: *p,
                    text: (*tag).to_owned(),
                    footprint: *fp,
                    shaped: None,
                    item: Some(*n),
                    moved: None,
                })
                .collect(),
            show,
            linked,
            behind: Vec::new(),
        }),
        ..blank_page(0x01)
    }
}

fn layer(view: &PageView, layer: Layer) -> LayerList {
    page_display_list(view, layer, bundled(), builtin())
}

/// The texts printed with their angles, in paint order.
fn texts(l: &LayerList) -> Vec<(String, Angle2)> {
    l.list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => Some((run.text.clone(), run.transform.angle)),
            _ => None,
        })
        .collect()
}

/// Whether an item is a circle (four cubic arcs): a balloon.
fn circle(i: &Item) -> bool {
    matches!(&i.kind, ItemKind::Path(p) if p.path.els.iter().filter(|e| matches!(e, PathEl::CubicTo(..))).count() == 4)
}

#[test]
fn a_tag_prints_where_it_fits_and_a_balloon_where_it_does_not() {
    let l = layer(&page(LabelShow::TagsOrNumbers, false), Layer::Labels);
    let printed = texts(&l);
    let words: Vec<&str> = printed.iter().map(|(t, _)| t.as_str()).collect();
    // The contactor's tag across, the breaker's turned, the terminal's number in its balloon.
    assert!(
        printed.contains(&("-102Q1".to_owned(), Angle2::ZERO)),
        "{printed:?}"
    );
    assert!(
        printed.contains(&("-101F1".to_owned(), Angle2(90_000))),
        "{printed:?}"
    );
    assert!(words.contains(&"7"), "{words:?}");
    assert!(
        !words.contains(&"-X1:1"),
        "the terminal's tag does not fit it: {words:?}"
    );
    // No number for a device whose tag fits, and one balloon, the terminal's.
    assert!(!words.contains(&"4") && !words.contains(&"5"), "{words:?}");
    assert_eq!(l.list.items.iter().filter(|i| circle(i)).count(), 1);
    // Tags alone print every tag, the terminal's outside it with a leader.
    let tags = texts(&layer(&page(LabelShow::Tags, false), Layer::Labels));
    assert!(tags.iter().any(|(t, _)| t == "-X1:1"), "{tags:?}");
}

#[test]
fn with_a_legend_page_the_footprints_are_where_the_rows_link_to_and_the_labels_link_nowhere() {
    let view = page(LabelShow::TagsOrNumbers, true);
    // A label is the device's handle: a click selects and drags it, so it is no link.
    let labels = layer(&view, Layer::Labels);
    assert!(labels.list.items.iter().all(|i| i.link.is_none()));
    let placements = layer(&view, Layer::Placements);
    for (p, ..) in devices() {
        let target = Some(Link::Target(Destination::Placement(p.as_uuid())));
        assert!(
            placements.list.items.iter().any(|i| i.link == target),
            "{p:?}: its footprint is where its row links to"
        );
    }
    // Without a legend page nothing links, so an older layout draws as it did.
    let plain = page(LabelShow::TagsOrNumbers, false);
    for l in [Layer::Labels, Layer::Placements] {
        assert!(layer(&plain, l).list.items.iter().all(|i| i.link.is_none()));
    }
}
