// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The golden frame's geometry against doc 20 §17.1 and §17.2: native QET pixels, printed millimetres
//! (FR-PAG-019, REF-096), the title-block cells, cell lookup (FR-PAG-020) and the exactness of the
//! page-to-sheet transform (FR-PAG-029).

use std::sync::Arc;

use ew_dl::{
    Brush, Dash, DisplayList, FontKey, Glyph, ImageData, ImageFormat, Item, Path, PathEl, ShapedRun, Stroke,
    Transform,
};
use ew_scene::forms::{FrameGeometry, SheetMap, title_block_origin};
use ew_schema::FormDef;
use ew_units::{Grid, Nm, Pt2, Rect};
use serde_json::json;

/// REF-096 tolerance: 0.2 mm.
const TOL: i64 = 200_000;
/// The doc rounds to 0.01 mm.
const TIGHT: i64 = 10_000;

fn frame_json(show_grid: bool) -> serde_json::Value {
    let mut grid = json!({
        "header_band": {"num": 15875000, "den": 3},
        "columns": {"count": 17, "width": {"num": 15875000, "den": 1}},
        "rows": {"count": 8, "height": {"num": 63500000, "den": 3}},
        "ew.frame.line_width": 264583
    });
    if !show_grid {
        grid["ew.frame.show_grid"] = json!(false);
    }
    json!({
        "kind": "frame",
        "sheet": {"width": 297000000, "height": 210000000},
        "page_to_sheet": {"scale_ppb": 986282578, "offset": [9790000, 9790000]},
        "grid": grid
    })
}

fn frame(show_grid: bool) -> FormDef {
    serde_json::from_value(frame_json(show_grid)).expect("frame form")
}

fn title_block() -> FormDef {
    serde_json::from_value(json!({
        "kind": "title_block",
        "sheet": {"width": 275166667, "height": 17991667}
    }))
    .expect("title block form")
}

/// `k` QET pixels in nm.
fn px(k: i64) -> Nm {
    Grid::QET_1PX.at(k).expect("px")
}

fn pt(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm(x), Nm(y))
}

/// Hundredths of a millimetre in nm.
fn cmm(h: i64) -> Nm {
    Nm(h * 10_000)
}

fn assert_near(what: &str, got: Nm, want: Nm, bound: i64) {
    let d = (got.0 - want.0).abs();
    assert!(
        d <= bound,
        "{what}: got {} nm, want {} nm, off by {d} nm (bound {bound})",
        got.0,
        want.0
    );
}

#[test]
fn native_geometry_in_qet_px() {
    let (f, tb) = (frame(true), title_block());
    let g = FrameGeometry::of(&f, Some(&tb)).expect("geometry");
    assert_eq!(g.column_edges.len(), 18);
    for (k, e) in g.column_edges.iter().enumerate() {
        assert_eq!(*e, px(20 + 60 * k as i64), "column edge {k}");
    }
    assert_eq!(g.row_edges.len(), 9);
    for (k, e) in g.row_edges.iter().enumerate() {
        assert_eq!(*e, px(20 + 80 * k as i64), "row edge {k}");
    }
    assert_eq!(g.header_band, px(20));
    assert_eq!(
        g.drawing_area,
        Rect::from_points(pt(px(20).0, px(20).0), pt(px(1040).0, px(660).0))
    );
    assert_eq!(g.outer, Rect::from_points(Pt2::ORIGIN, pt(px(1040).0, px(728).0)));
    assert_eq!(g.title_block_origin, Some(pt(0, px(660).0)));
    assert_eq!(title_block_origin(&f, &tb), pt(0, px(660).0));
    let cols: Vec<String> = (0..=16).map(|n| n.to_string()).collect();
    assert_eq!(g.column_labels, cols);
    let rows: Vec<String> = "ABCDEFGH".chars().map(String::from).collect();
    assert_eq!(g.row_labels, rows);
    assert_eq!(g.line_width, Nm(264_583));
    assert!(g.show_grid);

    let bare = FrameGeometry::of(&f, None).expect("geometry");
    assert_eq!(
        bare.outer,
        Rect::from_points(Pt2::ORIGIN, pt(px(1040).0, px(660).0))
    );
    assert_eq!(bare.title_block_origin, None);

    let nf = frame(false);
    let n = FrameGeometry::of(&nf, Some(&tb)).expect("geometry");
    assert!(!n.show_grid);
    assert_eq!(n.column_edges, g.column_edges);
    assert_eq!(n.row_edges, g.row_edges);
    assert_eq!(n.outer, g.outer);
}

#[test]
fn printed_geometry_matches_doc_20() {
    let (f, tb) = (frame(true), title_block());
    let g = FrameGeometry::of(&f, Some(&tb)).expect("geometry");
    let m = SheetMap::of(&f);

    let outer = m.apply_rect(g.outer);
    for (what, got, want) in [
        ("outer x0", outer.min().x, 979),
        ("outer x1", outer.max().x, 28118),
        ("outer y0", outer.min().y, 979),
        ("outer y1", outer.max().y, 19976),
    ] {
        assert_near(what, got, cmm(want), TOL);
        assert_near(what, got, cmm(want), TIGHT);
    }

    // Header bands: y 9.79-15.01 and x 9.79-15.01.
    let band = m.apply(pt(g.header_band.0, g.header_band.0));
    assert_near("column band bottom", band.y, cmm(1501), TIGHT);
    assert_near("row band right", band.x, cmm(1501), TIGHT);

    let area = m.apply_rect(g.drawing_area);
    for (what, got, want) in [
        ("area x0", area.min().x, 1501),
        ("area x1", area.max().x, 28118),
        ("area y0", area.min().y, 1501),
        ("area y1", area.max().y, 18202),
    ] {
        assert_near(what, got, cmm(want), TOL);
        assert_near(what, got, cmm(want), TIGHT);
    }

    // Columns x0(n) = 15.01 + 15.66 n, rows y0(n) = 15.01 + 20.88 n; the doc's pitch is rounded.
    let cx: Vec<Nm> = g.column_edges.iter().map(|e| m.apply(pt(e.0, 0)).x).collect();
    for (n, x) in cx.iter().enumerate() {
        assert_near(
            &format!("column edge {n}"),
            *x,
            cmm(1501) + Nm(n as i64 * 15_660_000),
            TOL,
        );
    }
    for w in cx.windows(2) {
        assert_near("column width", w[1] - w[0], cmm(1566), TIGHT);
    }
    let ry: Vec<Nm> = g.row_edges.iter().map(|e| m.apply(pt(0, e.0)).y).collect();
    for (n, y) in ry.iter().enumerate() {
        assert_near(
            &format!("row edge {n}"),
            *y,
            cmm(1501) + Nm(n as i64 * 20_880_000),
            TOL,
        );
    }
    for w in ry.windows(2) {
        assert_near("row height", w[1] - w[0], cmm(2088), TIGHT);
    }

    // Title block: x 9.79-281.18, y 182.02-199.76.
    let origin = title_block_origin(&f, &tb);
    let tbr = m.apply_rect(Rect::from_points(
        origin,
        pt(origin.x.0 + tb.sheet.width.0, origin.y.0 + tb.sheet.height.0),
    ));
    for (what, got, want) in [
        ("title block x0", tbr.min().x, 979),
        ("title block x1", tbr.max().x, 28118),
        ("title block y0", tbr.min().y, 18202),
        ("title block y1", tbr.max().y, 19976),
    ] {
        assert_near(what, got, cmm(want), TIGHT);
    }
}

/// A title-block cell: name, native px x0, x1, y0, y1, printed x0, x1, y0, y1 in 0.01 mm.
type Cell = (&'static str, i64, i64, i64, i64, [i64; 4]);

#[test]
fn title_block_cells_print_as_in_doc_20() {
    let (f, tb) = (frame(true), title_block());
    let m = SheetMap::of(&f);
    let o = title_block_origin(&f, &tb);
    // (cell, native px x0, x1, y0, y1; printed x0, x1, y0, y1 in 0.01 mm)
    #[rustfmt::skip]
    let cells: [Cell; 18] = [
        ("logo", 0, 46, 0, 68, [979, 2179, 18202, 19976]),
        ("page description", 46, 624, 0, 34, [2179, 17262, 18202, 19089]),
        ("project", 46, 468, 34, 51, [2179, 13192, 19089, 19533]),
        ("description", 46, 468, 51, 68, [2179, 13192, 19533, 19976]),
        ("drawing", 468, 624, 34, 51, [13192, 17262, 19089, 19533]),
        ("revision", 468, 624, 51, 68, [13192, 17262, 19533, 19976]),
        ("function", 624, 728, 0, 34, [17262, 19976, 18202, 19089]),
        ("location", 624, 728, 34, 68, [17262, 19976, 19089, 19976]),
        ("party", 728, 832, 0, 17, [19976, 22690, 18202, 18646]),
        ("object", 728, 832, 17, 34, [19976, 22690, 18646, 19089]),
        ("checked", 728, 832, 34, 51, [19976, 22690, 19089, 19533]),
        ("author", 728, 832, 51, 68, [19976, 22690, 19533, 19976]),
        ("creation", 832, 936, 0, 17, [22690, 25404, 18202, 18646]),
        ("saved", 832, 936, 17, 34, [22690, 25404, 18646, 19089]),
        ("page", 936, 1040, 0, 17, [25404, 28118, 18202, 18646]),
        ("prev", 936, 988, 17, 34, [25404, 26761, 18646, 19089]),
        ("next", 988, 1040, 17, 34, [26761, 28118, 18646, 19089]),
        ("notice", 832, 1040, 34, 68, [22690, 28118, 19089, 19976]),
    ];
    for (name, x0, x1, y0, y1, [px0, px1, py0, py1]) in cells {
        let r = m.apply_rect(Rect::from_points(
            pt(o.x.0 + px(x0).0, o.y.0 + px(y0).0),
            pt(o.x.0 + px(x1).0, o.y.0 + px(y1).0),
        ));
        assert_near(&format!("{name} x0"), r.min().x, cmm(px0), TIGHT);
        assert_near(&format!("{name} x1"), r.max().x, cmm(px1), TIGHT);
        assert_near(&format!("{name} y0"), r.min().y, cmm(py0), TIGHT);
        assert_near(&format!("{name} y1"), r.max().y, cmm(py1), TIGHT);
    }
}

#[test]
fn cell_h16_prints_where_fr_pag_015_says() {
    let (f, tb) = (frame(true), title_block());
    let g = FrameGeometry::of(&f, Some(&tb)).expect("geometry");
    let m = SheetMap::of(&f);
    let r = m.apply_rect(Rect::from_points(
        pt(g.column_edges[16].0, g.row_edges[7].0),
        pt(g.column_edges[17].0, g.row_edges[8].0),
    ));
    let bound = 100_000;
    assert_near("H16 x0", r.min().x, Nm(265_600_000), bound);
    assert_near("H16 x1", r.max().x, Nm(281_200_000), bound);
    assert_near("H16 y0", r.min().y, Nm(161_200_000), bound);
    assert_near("H16 y1", r.max().y, Nm(182_000_000), bound);
}

#[test]
fn column_lookup_boundaries_belong_to_the_right_cell() {
    let (f, tb) = (frame(true), title_block());
    let g = FrameGeometry::of(&f, Some(&tb)).expect("geometry");
    let col = Grid::new(47_625_000, 3).expect("60 px");
    let origin = g.header_band;
    for (k, e) in g.column_edges.iter().take(17).enumerate() {
        assert_eq!(col.cell(*e, origin), k as i64, "column edge {k}");
        assert_eq!(
            col.cell(Nm(e.0 - 1), origin),
            k as i64 - 1,
            "column edge {k} - 1 nm"
        );
    }
    assert_eq!(col.cell(Nm(px(1040).0 - 1), origin), 16);
}

#[test]
fn row_lookup_boundaries_belong_to_the_lower_cell() {
    let (f, tb) = (frame(true), title_block());
    let g = FrameGeometry::of(&f, Some(&tb)).expect("geometry");
    let row = Grid::new(63_500_000, 3).expect("80 px");
    let origin = g.header_band;
    // 80 px is not a whole number of nm, so a rounded row edge can lie up to 0.5 nm above the exact
    // boundary (the pitfall of `Grid::cell`, doc 06 §5.1): FR-PAG-020 looks an on-grid point up at its
    // exact grid position. On the 1 px grid that is integer arithmetic on px indices.
    let exact_row = |y_px: i64| (y_px - 20).div_euclid(80);
    for (k, e) in g.row_edges.iter().take(8).enumerate() {
        let k = k as i64;
        assert_eq!(exact_row(20 + 80 * k), k, "row edge {k} on the exact grid");
        assert_eq!(exact_row(20 + 80 * k - 1), k - 1, "one px above row edge {k}");
        // On rounded nm: the edge itself lands in k or, rounded up-sheet, in k − 1; one nm below it is
        // always in k, two nm above it always in k − 1.
        let c = row.cell(*e, origin);
        assert!(c == k || c == k - 1, "row edge {k}: {c}");
        assert_eq!(row.cell(Nm(e.0 + 1), origin), k, "row edge {k} + 1 nm");
        assert_eq!(row.cell(Nm(e.0 - 2), origin), k - 1, "row edge {k} - 2 nm");
    }
    assert_eq!(row.cell(Nm(px(660).0 - 1), origin), 7);
    let label = &g.row_labels[exact_row(20 + 80 * 7) as usize];
    assert_eq!(label, "H");
}

fn half(offset: Pt2) -> SheetMap {
    SheetMap {
        scale_ppb: 500_000_000,
        offset,
    }
}

#[test]
fn scale_len_is_odd_symmetric_and_ties_go_toward_zero() {
    for m in [SheetMap::of(&frame(true)), half(Pt2::ORIGIN)] {
        for n in (0..2000).chain([999_999_999, 275_166_667, 1 << 40]) {
            assert_eq!(m.scale_len(Nm(-n)), -m.scale_len(Nm(n)), "n = {n}");
        }
    }
    let m = half(Pt2::ORIGIN);
    assert_eq!(m.scale_len(Nm(3)), Nm(1));
    assert_eq!(m.scale_len(Nm(-3)), Nm(-1));
    assert_eq!(m.scale_len(Nm(5)), Nm(2));
    assert_eq!(m.scale_len(Nm(-5)), Nm(-2));
    assert_eq!(m.scale_len(Nm(4)), Nm(2));

    let id = SheetMap::IDENTITY;
    assert!(id.is_identity());
    for p in [pt(0, 0), pt(1, -1), pt(275_166_667, 192_616_667), pt(-7, 13)] {
        assert_eq!(id.apply(p), p);
    }
    // A form without page_to_sheet maps as the identity.
    let plain: FormDef = serde_json::from_value(json!({
        "kind": "title_block",
        "sheet": {"width": 1, "height": 1}
    }))
    .expect("form");
    assert!(SheetMap::of(&plain).is_identity());
}

fn run(origin: Pt2) -> ShapedRun {
    ShapedRun {
        font: FontKey {
            sha256: [7; 32],
            face_index: 0,
        },
        size: Nm(2_500_000),
        origin,
        transform: Transform::IDENTITY,
        glyphs: vec![
            Glyph {
                id: 36,
                x: Nm(0),
                y: Nm(0),
            },
            Glyph {
                id: 37,
                x: Nm(1_500_001),
                y: Nm(-3),
            },
        ],
        text: "AB".into(),
        brush: Brush::BLACK,
        layout_box: Rect::from_points(pt(0, -2_000_000), pt(3_000_000, 500_000)),
    }
}

fn image() -> Arc<ImageData> {
    Arc::new(ImageData {
        sha256: [1; 32],
        format: ImageFormat::Png,
        bytes: Arc::from(vec![1u8, 2, 3]),
    })
}

fn line(a: Pt2, b: Pt2) -> Path {
    Path {
        els: vec![PathEl::MoveTo(a), PathEl::LineTo(b)],
    }
}

fn dashed(width: i64, segments: [i64; 2], offset: i64) -> Stroke {
    let mut s = Stroke::solid(Nm(width));
    s.dash = Some(Dash {
        segments: segments.into_iter().map(Nm).collect(),
        offset: Nm(offset),
    });
    s
}

#[test]
fn display_list_maps_points_with_offset_and_lengths_without() {
    // Scale 1/2, offset (1 mm, 2 mm).
    let m = half(pt(1_000_000, 2_000_000));
    let input = DisplayList {
        items: vec![
            Item::path(
                line(pt(0, 0), pt(10_000_000, 4_000_000)),
                Some(dashed(264_583, [1_040_000, 520_000], 101)),
                None,
            ),
            Item::glyphs(Arc::new(run(pt(4_000_000, 6_000_000)))),
            Item::image(
                image(),
                Rect::from_points(pt(2_000_000, 2_000_000), pt(6_000_000, 4_000_000)),
            ),
            Item::group(
                Some(Transform {
                    translate: pt(10_000_000, 20_000_000),
                    ..Transform::IDENTITY
                }),
                Some(Rect::from_points(pt(0, 0), pt(8_000_000, 6_000_000))),
                vec![Item::path(
                    line(pt(2_000_000, 2_000_000), pt(4_000_000, 6_000_000)),
                    Some(Stroke::solid(Nm(264_583))),
                    None,
                )],
            ),
        ],
    };
    let expected = DisplayList {
        items: vec![
            // Points map with the offset; 264 583 / 2 = 132 291.5 ties toward zero; 101 / 2 = 50.5 too.
            Item::path(
                line(pt(1_000_000, 2_000_000), pt(6_000_000, 4_000_000)),
                Some(dashed(132_291, [520_000, 260_000], 50)),
                None,
            ),
            // Origin maps with the offset; size, glyph x/y and layout box scale without it.
            Item::glyphs(Arc::new({
                let mut r = run(pt(3_000_000, 5_000_000));
                r.size = Nm(1_250_000);
                r.glyphs[1].x = Nm(750_000);
                r.glyphs[1].y = Nm(-1);
                r.layout_box = Rect::from_points(pt(0, -1_000_000), pt(1_500_000, 250_000));
                r
            })),
            Item::image(
                image(),
                Rect::from_points(pt(2_000_000, 3_000_000), pt(4_000_000, 4_000_000)),
            ),
            // The translation maps with the offset; clip and children scale without it.
            Item::group(
                Some(Transform {
                    translate: pt(6_000_000, 12_000_000),
                    ..Transform::IDENTITY
                }),
                Some(Rect::from_points(pt(0, 0), pt(4_000_000, 3_000_000))),
                vec![Item::path(
                    line(pt(1_000_000, 1_000_000), pt(2_000_000, 3_000_000)),
                    Some(Stroke::solid(Nm(132_291))),
                    None,
                )],
            ),
        ],
    };
    assert_eq!(m.apply_display_list(&input, None), expected);
    // The identity leaves a list unchanged.
    assert_eq!(SheetMap::IDENTITY.apply_display_list(&input, None), input);
}
