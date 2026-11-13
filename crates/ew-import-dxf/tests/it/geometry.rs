// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The geometry of single entities (doc 14 §6.1): bulges within the tolerance, mirrored and rotated inserts, MINSERT
//! arrays, R12 binary files, layouts with their viewports.

use std::collections::BTreeSet;

use ew_import_dxf::ew_interop_api::Limits;
use ew_import_dxf::ew_interop_api::ew_schema::{GraphicShape, PageItemKind};
use ew_import_dxf::{Space, convert, probe};

use crate::common::Dxf;

/// The points of each graphic of a drawing in mm (y as on the sheet).
fn points_mm(bytes: &[u8], space: &Space) -> Vec<Vec<(f64, f64)>> {
    let c = convert(bytes, &Limits::default(), None, space, &BTreeSet::new(), "a.dxf").unwrap();
    c.items
        .iter()
        .filter_map(|(_, _, k)| match k {
            PageItemKind::Graphic(g) => Some(match &g.shape {
                GraphicShape::Line(l) => vec![l.a, l.b],
                GraphicShape::Polyline(p) | GraphicShape::Polygon(p) => p.points.clone(),
                GraphicShape::Circle(c) => vec![c.centre],
                GraphicShape::Arc(a) => vec![a.centre],
                _ => Vec::new(),
            }),
            _ => None,
        })
        .map(|pts| {
            pts.iter()
                .map(|p| (p.x.0 as f64 / 1e6, p.y.0 as f64 / 1e6))
                .collect()
        })
        .collect()
}

#[test]
fn a_bulge_is_an_arc_within_the_tolerance() {
    // A half circle from (0, 0) to (10, 0), bulge 1 (counter-clockwise): through (5, −5) in the drawing's y-up frame.
    let mut d = Dxf::new("AC1015", Some(4));
    d.section("ENTITIES");
    d.ps(&[
        (0, "LWPOLYLINE"),
        (5, "A"),
        (8, "0"),
        (90, "2"),
        (70, "0"),
        (10, "0"),
        (20, "0"),
        (42, "1"),
    ]);
    d.ps(&[(10, "10"), (20, "0")]);
    d.ps(&[(0, "POINT"), (5, "B"), (8, "0"), (10, "0"), (20, "10")]);
    d.end();
    let all = points_mm(&d.ascii(), &Space::Model);
    let pts = &all[0];
    // y_top is 10 (the point), so a drawing y of y lands at 10 − y.
    assert_eq!(pts.first(), Some(&(0.0, 10.0)));
    assert_eq!(pts.last(), Some(&(10.0, 10.0)));
    for (x, y) in pts {
        let r = ((x - 5.0).powi(2) + (y - 10.0).powi(2)).sqrt();
        assert!((r - 5.0).abs() <= 0.01, "({x}, {y}) is {r} mm from the centre");
        assert!(
            *y >= 10.0 - 1e-9,
            "the arc bulges below the chord in the drawing, above y = 10 on the sheet"
        );
    }
    assert!(pts.iter().any(|(_, y)| (*y - 15.0).abs() < 0.01));
}

#[test]
fn a_mirrored_rotated_insert_maps_its_block() {
    // Block B: a line from (0, 0) to (10, 0). Inserted at (100, 100), x scale −1, rotated 90°: the line runs from
    // (100, 100) to (100, 90) (mirror: (−10, 0); turn 90°: (0, −10)).
    let mut d = Dxf::new("AC1015", Some(4));
    d.section("BLOCKS");
    d.ps(&[(0, "BLOCK"), (8, "0"), (2, "B"), (70, "0"), (10, "0"), (20, "0")]);
    d.ps(&[(0, "LINE"), (8, "0"), (10, "0"), (20, "0"), (11, "10"), (21, "0")]);
    d.ps(&[(0, "ENDBLK"), (8, "0")]);
    d.end();
    d.section("ENTITIES");
    d.ps(&[
        (0, "INSERT"),
        (5, "I"),
        (8, "0"),
        (2, "B"),
        (10, "100"),
        (20, "100"),
        (41, "-1"),
        (42, "1"),
    ]);
    d.ps(&[(50, "90")]);
    d.end();
    // y_top = 100: the line's ends (100, 100) and (100, 90) land at y 0 and 10.
    assert_eq!(
        points_mm(&d.ascii(), &Space::Model),
        [vec![(100.0, 0.0), (100.0, 10.0)]]
    );
}

#[test]
fn a_minsert_draws_its_array() {
    let mut d = Dxf::new("AC1015", Some(4));
    d.section("BLOCKS");
    d.ps(&[(0, "BLOCK"), (8, "0"), (2, "B"), (70, "0"), (10, "0"), (20, "0")]);
    d.ps(&[(0, "CIRCLE"), (8, "0"), (10, "0"), (20, "0"), (40, "1")]);
    d.ps(&[(0, "ENDBLK"), (8, "0")]);
    d.end();
    d.section("ENTITIES");
    d.ps(&[
        (0, "INSERT"),
        (5, "I"),
        (8, "0"),
        (2, "B"),
        (10, "0"),
        (20, "0"),
        (70, "3"),
        (71, "2"),
    ]);
    d.ps(&[(44, "5"), (45, "5")]);
    d.end();
    assert_eq!(points_mm(&d.ascii(), &Space::Model).len(), 6);
}

#[test]
fn an_r12_binary_file_with_one_byte_codes_is_read() {
    let mut d = Dxf::new("AC1009", Some(4));
    d.section("ENTITIES");
    d.ps(&[(0, "LINE"), (8, "0"), (10, "0"), (20, "0"), (11, "10"), (21, "0")]);
    d.end();
    // R12 binary: one byte per code.
    let mut out = b"AutoCAD Binary DXF\r\n\x1a\0".to_vec();
    let mut all = d.pairs.clone();
    all.push((0, "EOF".to_owned()));
    for (c, v) in &all {
        out.push(u8::try_from(*c).unwrap());
        match c {
            10..=59 => out.extend_from_slice(&v.parse::<f64>().unwrap().to_le_bytes()),
            60..=79 => out.extend_from_slice(&v.parse::<i16>().unwrap().to_le_bytes()),
            _ => {
                out.extend_from_slice(v.as_bytes());
                out.push(0);
            }
        }
    }
    let c = convert(
        &out,
        &Limits::default(),
        None,
        &Space::Model,
        &BTreeSet::new(),
        "a.dxf",
    )
    .unwrap();
    assert_eq!(c.items.len(), 1);
    assert_eq!(c.version, "AC1009");
}

#[test]
fn a_layout_shows_its_own_entities_and_the_model_through_a_viewport_clipped() {
    let mut d = Dxf::new("AC1015", Some(4));
    d.section("TABLES");
    d.ps(&[(0, "TABLE"), (2, "BLOCK_RECORD"), (70, "2")]);
    d.ps(&[(0, "BLOCK_RECORD"), (5, "1F"), (2, "*Model_Space")]);
    d.ps(&[(0, "BLOCK_RECORD"), (5, "1E"), (2, "*Paper_Space")]);
    d.p(0, "ENDTAB");
    d.end();
    d.section("ENTITIES");
    // Model space: a line from (0, 0) to (100, 0).
    d.ps(&[
        (0, "LINE"),
        (5, "M"),
        (330, "1F"),
        (8, "0"),
        (10, "0"),
        (20, "0"),
        (11, "100"),
        (21, "0"),
    ]);
    // Paper space: a frame line and a viewport of 20 × 20 at (10, 10) showing the model's (25, 0) at scale 1/5.
    d.ps(&[
        (0, "LINE"),
        (5, "P"),
        (330, "1E"),
        (67, "1"),
        (8, "0"),
        (10, "0"),
        (20, "0"),
        (11, "0"),
        (21, "50"),
    ]);
    d.ps(&[
        (0, "VIEWPORT"),
        (5, "V"),
        (330, "1E"),
        (67, "1"),
        (8, "0"),
        (10, "10"),
        (20, "10"),
        (40, "20"),
    ]);
    d.ps(&[(41, "20"), (12, "25"), (22, "0"), (45, "100"), (69, "2")]);
    d.end();
    d.section("OBJECTS");
    d.ps(&[(0, "LAYOUT"), (5, "L"), (1, "Sheet 1"), (71, "1"), (340, "1E")]);
    d.end();
    let bytes = d.ascii();
    let p = probe(&bytes, &Limits::default()).unwrap();
    assert_eq!(p.layouts, ["Sheet 1"]);
    let lines = points_mm(&bytes, &Space::Layout("Sheet 1".to_owned()));
    assert_eq!(lines.len(), 2, "{lines:?}");
    // The model line maps to x = 10 + (x − 25) / 5: from 5 to 25, cut to the viewport's 0 … 20.
    let model = lines.iter().find(|l| l[0].1 == l[1].1).unwrap();
    let xs: Vec<f64> = model.iter().map(|p| p.0).collect();
    assert_eq!(xs, [5.0, 20.0]);
    // The model space alone has only the model line.
    assert_eq!(points_mm(&bytes, &Space::Model).len(), 1);
}
