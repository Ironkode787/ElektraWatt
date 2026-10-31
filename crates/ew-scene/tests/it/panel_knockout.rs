// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The knockout of a panel page's labels (WP-S4.01 D10 as built; cycle 41's window check W41a, the orchestrator's
//! ruling 2): the carriers layer cuts a rail's outline and its axis around every label's knockout box, a horizontal
//! label's and a turned one's alike, and the footprints and the labels stay whole.
//!
//! - `the_carriers_are_cut_around_a_horizontal_and_a_turned_label`: no carrier stroke enters either box, the cuts end
//!   on the boxes' edges, the footprints are whole rectangles and both labels print.
//! - `a_carrier_without_labels_on_it_is_drawn_whole`: without labels the outline is one closed rectangle and the axis
//!   one line, as before.

use ew_dl::{DisplayList, EntityRef, ItemKind, Layer, Path, PathEl};
use ew_ids::{Id, Uuid};
use ew_scene::page::{PageKind, PageView, page_display_list};
use ew_scene::panel::{
    CarrierDraw, LAYOUT_LABEL_STYLE, LabelFit, PanelLabel, PanelView, PlacedLabel, PlacementDraw, Projection,
    place_label,
};
use ew_schema::Scale;
use ew_text::{builtin, bundled};
use ew_units::{Nm, Pt2, Rect};

/// A surface point in millimetres.
fn mm(x: f64, y: f64) -> Pt2 {
    Pt2::new(Nm((x * 1e6).round() as i64), Nm((y * 1e6).round() as i64))
}

const RAIL: u128 = 0x51;
const WIDE: u128 = 0x61;
const NARROW: u128 = 0x62;

/// The projection: 1:10 from the datum (10, 50) mm on a frame without a sheet map.
fn projection() -> Projection {
    Projection::of(mm(10.0, 50.0), &Scale::OneTo(10))
}

/// A rail 200 × 10 mm (2 × 1 mm on the page) with two placements on it: a 50 × 80 mm footprint whose label "-X1"
/// fits horizontally and an 18 × 90 mm one whose label "-101F1" fits only turned; both labels are taller (across
/// the text) than the rail is on the page, so each crosses both of its long edges and its axis.
fn view(labels: bool) -> PageView {
    let id = |n: u128| Uuid::from_u128(n);
    let wide = Rect::from_points(mm(20.0, -40.0), mm(70.0, 40.0));
    let narrow = Rect::from_points(mm(120.0, -45.0), mm(138.0, 45.0));
    let label = |n: u128, text: &str, footprint: Rect| PanelLabel {
        placement: Id::from_uuid(id(n)),
        text: text.to_owned(),
        footprint,
        shaped: None,
    };
    PageView {
        page: Id::from_uuid(id(0x01)),
        kind: PageKind::Panel,
        grid: ew_units::Grid::MM4,
        language: None,
        frame: None,
        reps: Vec::new(),
        connections: Vec::new(),
        junctions: Vec::new(),
        wire_marker: None,
        table: None,
        embedded: Vec::new(),
        break_marks: Vec::new(),
        cp_targets: Vec::new(),
        crossings: Default::default(),
        items: Vec::new(),
        structure_boxes: Vec::new(),
        panel: Some(PanelView {
            item: Id::from_uuid(id(0x02)),
            projection: projection(),
            surface: None,
            bounds: None,
            carriers: vec![CarrierDraw {
                carrier: Id::from_uuid(id(RAIL)),
                axis: Some((mm(0.0, 0.0), mm(200.0, 0.0))),
                outline: vec![Rect::from_points(mm(0.0, -5.0), mm(200.0, 5.0))],
                duct: None,
            }],
            placements: [(WIDE, wide), (NARROW, narrow)]
                .into_iter()
                .map(|(n, footprint)| PlacementDraw {
                    placement: Id::from_uuid(id(n)),
                    footprint,
                    unknown: false,
                    outline: Vec::new(),
                })
                .collect(),
            labels: if labels {
                vec![label(WIDE, "-X1", wide), label(NARROW, "-101F1", narrow)]
            } else {
                Vec::new()
            },
        }),
    }
}

fn layer(v: &PageView, l: Layer) -> DisplayList {
    page_display_list(v, l, bundled(), builtin()).list
}

/// The placed label of `text` on the footprint `fp` (surface frame), as the scene places it.
fn placed(text: &str, fp: Rect) -> PlacedLabel {
    let style = builtin().resolve(LAYOUT_LABEL_STYLE).expect("the label style");
    let shaped = bundled().shape(text, &style, None);
    place_label(
        bundled(),
        &style,
        text,
        &shaped,
        projection().rect(fp).expect("in range"),
        None,
    )
}

/// The straight segments of a path (every element is a move or a line here).
fn segments(p: &Path) -> Vec<(Pt2, Pt2)> {
    let mut out = Vec::new();
    let mut at: Option<Pt2> = None;
    let mut start: Option<Pt2> = None;
    for el in &p.els {
        match *el {
            PathEl::MoveTo(a) => {
                at = Some(a);
                start = Some(a);
            }
            PathEl::LineTo(b) => {
                if let Some(a) = at {
                    out.push((a, b));
                }
                at = Some(b);
            }
            PathEl::ClosePath => {
                if let (Some(a), Some(s)) = (at, start) {
                    out.push((a, s));
                }
                at = start;
            }
            _ => panic!("a carrier draws straight lines: {el:?}"),
        }
    }
    out
}

/// Whether the axis-parallel segment `a`–`b` runs through the open interior of `r`.
fn enters(a: Pt2, b: Pt2, r: Rect) -> bool {
    let open = |lo: i64, hi: i64, p: i64, q: i64| p.min(q) < hi && p.max(q) > lo;
    let inside = |lo: i64, hi: i64, v: i64| lo < v && v < hi;
    if a.y == b.y {
        inside(r.min().y.0, r.max().y.0, a.y.0) && open(r.min().x.0, r.max().x.0, a.x.0, b.x.0)
    } else {
        inside(r.min().x.0, r.max().x.0, a.x.0) && open(r.min().y.0, r.max().y.0, a.y.0, b.y.0)
    }
}

#[test]
fn the_carriers_are_cut_around_a_horizontal_and_a_turned_label() {
    let v = view(true);
    let wide = placed("-X1", Rect::from_points(mm(20.0, -40.0), mm(70.0, 40.0)));
    let narrow = placed("-101F1", Rect::from_points(mm(120.0, -45.0), mm(138.0, 45.0)));
    assert_eq!((wide.fit, narrow.fit), (LabelFit::Horizontal, LabelFit::Turned));
    let boxes = [wide.knockout, narrow.knockout];
    // The rail on the page: (10, 49.5)–(30, 50.5) mm; each box is taller than the rail and inside its length.
    for b in boxes {
        assert!(
            b.min().y < mm(0.0, 49.5).y && b.max().y > mm(0.0, 50.5).y,
            "{b:?} crosses the rail"
        );
        assert!(
            b.min().x > mm(10.0, 0.0).x && b.max().x < mm(30.0, 0.0).x,
            "{b:?} on the rail"
        );
    }
    assert!(
        narrow.knockout.height() > narrow.knockout.width(),
        "the turned box is upright"
    );
    assert!(
        wide.knockout.width() > wide.knockout.height(),
        "the horizontal box lies flat"
    );

    let carriers = layer(&v, Layer::Carriers);
    let rail = EntityRef(Uuid::from_u128(RAIL));
    assert_eq!(carriers.items.len(), 2, "one outline item and one axis item");
    let mut points = Vec::new();
    for item in &carriers.items {
        let ItemKind::Path(p) = &item.kind else {
            panic!("a carrier draws paths")
        };
        assert!(p.fill.is_none(), "a cut, not a fill");
        // The axis: three runs between two gaps. The outline: two gaps in each long edge leave four runs, two of
        // them around the rail's ends.
        let dashed = p.stroke.as_ref().is_some_and(|s| s.dash.is_some());
        let moves = p
            .path
            .els
            .iter()
            .filter(|e| matches!(e, PathEl::MoveTo(_)))
            .count();
        assert_eq!(moves, if dashed { 3 } else { 4 }, "{:?}", p.path.els);
        for (a, b) in segments(&p.path) {
            for r in boxes {
                assert!(!enters(a, b, r), "{a:?}–{b:?} runs through the label box {r:?}");
            }
            points.extend([a, b]);
        }
        // The outline keeps the rail's entity; the axis carries none.
        assert_eq!(item.entity.is_some(), !dashed);
        assert!(item.entity.is_none_or(|e| e == rail));
    }
    // The cuts end on the boxes' left and right edges, on both long edges of the rail and on the axis.
    for r in boxes {
        for x in [r.min().x, r.max().x] {
            for y in [mm(0.0, 49.5).y, mm(0.0, 50.0).y, mm(0.0, 50.5).y] {
                assert!(points.contains(&Pt2::new(x, y)), "a cut ends at ({x:?}, {y:?})");
            }
        }
    }

    // The footprints stay whole: one closed rectangle each.
    let placements = layer(&v, Layer::Placements);
    let expected: Vec<Vec<PathEl>> = [
        Rect::from_points(mm(20.0, -40.0), mm(70.0, 40.0)),
        Rect::from_points(mm(120.0, -45.0), mm(138.0, 45.0)),
    ]
    .iter()
    .map(|r| Path::rect(projection().rect(*r).expect("in range")).els)
    .collect();
    let drawn: Vec<Vec<PathEl>> = placements
        .items
        .iter()
        .map(|i| match &i.kind {
            ItemKind::Path(p) => p.path.els.clone(),
            other => panic!("a footprint is a path: {other:?}"),
        })
        .collect();
    assert_eq!(drawn, expected);

    // The labels stay whole: both print, in full.
    let labels = layer(&v, Layer::Labels);
    let texts: Vec<&str> = labels
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(r) => Some(r.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["-X1", "-101F1"]);
}

#[test]
fn a_carrier_without_labels_on_it_is_drawn_whole() {
    let v = view(false);
    let carriers = layer(&v, Layer::Carriers);
    let p = projection();
    let paths: Vec<Vec<PathEl>> = carriers
        .items
        .iter()
        .map(|i| match &i.kind {
            ItemKind::Path(p) => p.path.els.clone(),
            other => panic!("a carrier draws paths: {other:?}"),
        })
        .collect();
    let outline = p
        .rect(Rect::from_points(mm(0.0, -5.0), mm(200.0, 5.0)))
        .expect("in range");
    let axis = (
        p.point(mm(0.0, 0.0)).expect("in range"),
        p.point(mm(200.0, 0.0)).expect("in range"),
    );
    assert_eq!(
        paths,
        vec![Path::rect(outline).els, Path::line(axis.0, axis.1).els]
    );
}
