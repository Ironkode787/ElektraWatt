// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! AC-37's DXF half (doc 09 §4.4, doc 14 §6.4; FR-OUT-015, FR-IMP-032): the DXF (AC1015) of Golden A's folios
//! 101–106 has the configured layers, its wires on the wires layer and its symbols as inserts, and re-imports as
//! graphic pages within 0.01 mm: every corner and curve end the page draws lies within 0.01 mm of a point the
//! re-imported drawing holds. The export's bytes do not depend on the run.

use std::collections::{BTreeMap, BTreeSet};

use ew_api::dxf::{DxfOptions, Space};
use ew_api::output::{DxfLayers, ExportOptions, PageSelection};
use ew_api::{Session, SessionOptions};
use ew_dl::{Item, ItemKind, PathEl, Transform};
use ew_schema::{GraphicShape, PageItemKind};
use ew_units::Pt2;

use crate::golden_common;

/// 0.01 mm.
const TOLERANCE: i64 = 10_000;

/// The corners and curve ends of every drawn path of `items` on the sheet.
fn drawn_points(items: &[Item], t: Transform, out: &mut Vec<Pt2>) {
    for item in items {
        match &item.kind {
            ItemKind::Path(p) if p.stroke.is_some() || p.fill.is_some() => {
                for el in &p.path.els {
                    match el {
                        PathEl::MoveTo(q) | PathEl::LineTo(q) | PathEl::QuadTo(_, q) | PathEl::CubicTo(_, _, q) => {
                            out.push(t.apply(*q));
                        }
                        PathEl::ClosePath => {}
                    }
                }
            }
            ItemKind::Group(g) => drawn_points(&g.items, g.transform.map_or(t, |gt| gt.then(t)), out),
            _ => {}
        }
    }
}

/// The points of the re-imported graphics.
fn imported_points(items: &[(String, Option<ew_mltext::Ident>, PageItemKind)]) -> Vec<Pt2> {
    let mut out = Vec::new();
    for (_, _, kind) in items {
        if let PageItemKind::Graphic(g) = kind {
            match &g.shape {
                GraphicShape::Line(l) => out.extend([l.a, l.b]),
                GraphicShape::Polyline(p) | GraphicShape::Polygon(p) => out.extend(p.points.iter().copied()),
                _ => {}
            }
        }
    }
    out
}

#[test]
fn folios_101_to_106_export_to_dxf_with_their_layers_and_re_import_within_a_hundredth_of_a_millimetre() {
    let session = golden_common::session(&golden_common::batch());
    let options = ExportOptions {
        pages: PageSelection::parse("101-106").unwrap(),
        ..ExportOptions::default()
    };
    let layers = DxfLayers::default();
    let dxf = session.export_dxf(&options, &layers).unwrap();
    let pages = session.export_pages(&options).unwrap();
    assert!(!dxf.files.is_empty());
    assert_eq!(dxf.files.len(), pages.pages.len());
    assert_eq!(
        session.export_dxf(&options, &layers).unwrap(),
        dxf,
        "the export does not depend on the run"
    );
    let mut fresh = Session::from_records(
        serde_json::from_str(r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b38","derivation_profile":"2027.1"}"#).unwrap(),
        [],
        [],
        SessionOptions::default(),
    )
    .unwrap();
    for (file, page) in dxf.files.iter().zip(&pages.pages) {
        assert_eq!(file.name, format!("{}.dxf", page.label));
        let text = &file.text;
        assert!(text.contains("\n  1\nAC1015\n"), "{}: AC1015", file.name);
        assert!(text.contains("$INSUNITS\n 70\n4\n"), "{}: millimetres", file.name);
        for name in [&layers.frame, &layers.symbols, &layers.wires, &layers.texts, &layers.graphics, &layers.tables] {
            assert!(text.contains(&format!("AcDbLayerTableRecord\n  2\n{name}\n")), "{}: layer {name}", file.name);
        }
        // A wire: a polyline entity on the wires layer (its lineweight follows the layer).
        assert!(
            text.contains(&format!("  8\n{}\n370\n", layers.wires)),
            "{}: wires on {}",
            file.name,
            layers.wires
        );
        assert!(text.contains(&format!("  8\n{}\n100\nAcDbBlockReference", layers.symbols)), "{}: symbol inserts", file.name);
        assert!(text.contains(&format!("  8\n{}\n100\nAcDbText", layers.frame)), "{}: title block texts", file.name);

        // Every drawn point of the page within 0.01 mm of a re-imported one.
        let converted =
            ew_import_dxf::convert(text.as_bytes(), &Default::default(), None, &Space::Model, &BTreeSet::new(), &file.name)
                .unwrap();
        let mut grid: BTreeMap<(i64, i64), Vec<Pt2>> = BTreeMap::new();
        let cell = |p: Pt2| (p.x.0.div_euclid(TOLERANCE), p.y.0.div_euclid(TOLERANCE));
        for p in imported_points(&converted.items) {
            grid.entry(cell(p)).or_default().push(p);
        }
        let mut drawn = Vec::new();
        drawn_points(&page.content.items, Transform::IDENTITY, &mut drawn);
        assert!(!drawn.is_empty());
        for p in drawn {
            let (cx, cy) = cell(p);
            let near = (cx - 1..=cx + 1).any(|x| {
                (cy - 1..=cy + 1).any(|y| {
                    grid.get(&(x, y)).is_some_and(|ps| {
                        ps.iter().any(|q| (q.x.0 - p.x.0).abs() <= TOLERANCE && (q.y.0 - p.y.0).abs() <= TOLERANCE)
                    })
                })
            });
            assert!(near, "{}: ({}, {}) nm has no re-imported point within 0.01 mm", file.name, p.x.0, p.y.0);
        }

        // And it re-imports as a graphic page.
        let imported = fresh
            .import_dxf_page(&file.name, text.clone().into_bytes(), &DxfOptions::default())
            .unwrap();
        assert!(imported.report.stats.per_kind.get("page_item").is_some_and(|n| *n > 0), "{}", file.name);
    }
    let graphic = fresh.query().pages().iter().filter(|(_, p)| p.kind == ew_schema::PageType::Graphic).count();
    assert_eq!(graphic, pages.pages.len());
}
