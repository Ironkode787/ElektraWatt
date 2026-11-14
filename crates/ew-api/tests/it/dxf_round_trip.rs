// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! AC-37's DXF half (doc 09 §4.4, doc 14 §6.4; FR-OUT-015, FR-IMP-032): the DXF (AC1015) of Golden A's folios
//! 101–106 has the configured layers, its wires on the wires layer and its symbols as inserts, and re-imports as
//! graphic pages of Golden A within 0.01 mm: every corner and curve end the page draws, and every text's baseline,
//! prints within 0.01 mm of the re-imported page's on the sheet, through the frame's page-to-sheet map; as an underlay
//! too. The export's bytes do not depend on the run. A symbol insert and a text come back where
//! they were written, mirrored, turned and with their characters (`Ää°²`), through the file's ASCII.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_api::dxf::{DxfOptions, Space};
use ew_api::output::{DxfLayers, ExportOptions, PageSelection};
use ew_api::Session;
use ew_dl::{
    Brush, DisplayList, FontKey, Item, ItemKind, Layer, Path, PathEl, ShapedRun, Stroke, SymbolGeomId,
    Transform,
};
use ew_schema::{GraphicShape, PageItemKind};
use ew_units::{Angle2, Nm, Orient2, Pt2, Rect};

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
                        PathEl::MoveTo(q)
                        | PathEl::LineTo(q)
                        | PathEl::QuadTo(_, q)
                        | PathEl::CubicTo(_, _, q) => {
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
    // Re-imported into Golden A itself, whose frame maps page to sheet (scale 0.986, offset 9.79 mm): each new page
    // must print where the drawing was drawn.
    let mut golden = golden_common::session(&golden_common::batch());
    for (file, page) in dxf.files.iter().zip(&pages.pages) {
        assert_eq!(file.name, format!("{}.dxf", page.label));
        let text = &file.text;
        assert!(text.contains("\n  1\nAC1015\n"), "{}: AC1015", file.name);
        assert!(text.contains("$INSUNITS\n 70\n4\n"), "{}: millimetres", file.name);
        for name in [
            &layers.frame,
            &layers.symbols,
            &layers.wires,
            &layers.texts,
            &layers.graphics,
            &layers.tables,
        ] {
            assert!(
                text.contains(&format!("AcDbLayerTableRecord\n  2\n{name}\n")),
                "{}: layer {name}",
                file.name
            );
        }
        // A wire: a polyline entity on the wires layer (its lineweight follows the layer).
        assert!(
            text.contains(&format!("  8\n{}\n370\n", layers.wires)),
            "{}: wires on {}",
            file.name,
            layers.wires
        );
        assert!(
            text.contains(&format!("  8\n{}\n100\nAcDbBlockReference", layers.symbols)),
            "{}: symbol inserts",
            file.name
        );
        assert!(
            text.contains(&format!("  8\n{}\n100\nAcDbText", layers.frame)),
            "{}: title block texts",
            file.name
        );

        // Every drawn point of the page within 0.01 mm of a re-imported one.
        let converted = ew_import_dxf::convert(
            text.as_bytes(),
            &Default::default(),
            None,
            &Space::Model,
            &BTreeSet::new(),
            &file.name,
        )
        .unwrap();
        let mut drawn = Vec::new();
        drawn_points(&page.content.items, Transform::IDENTITY, &mut drawn);
        assert!(!drawn.is_empty());
        all_near(&file.name, "drawn point", &drawn, &imported_points(&converted.items));

        // And it re-imports as a graphic page that prints where the drawing was drawn: its drawn points and its texts'
        // baselines on the sheet within 0.01 mm of the original's.
        let imported = golden
            .import_dxf_page(&file.name, text.clone().into_bytes(), &DxfOptions::default())
            .unwrap();
        let sheet = golden
            .export_pages(&ExportOptions {
                pages: PageSelection::Folios(vec![imported.folio.clone()]),
                ..ExportOptions::default()
            })
            .unwrap()
            .pages
            .remove(0);
        let mut original = Vec::new();
        drawn_points(&page.content.items, Transform::IDENTITY, &mut original);
        let mut printed = Vec::new();
        drawn_points(&sheet.content.items, Transform::IDENTITY, &mut printed);
        all_near(&file.name, "drawn point", &original, &printed);
        let mut original = Vec::new();
        baselines(&page.content.items, Transform::IDENTITY, &mut original);
        let mut printed = Vec::new();
        baselines(&sheet.content.items, Transform::IDENTITY, &mut printed);
        assert!(!original.is_empty(), "{}: texts", file.name);
        all_near(&file.name, "text", &original, &printed);
    }

    // The first page's drawing as an underlay of the second prints where it was drawn too: the underlay's items are
    // the first ones of the sheet the canvas draws.
    let (file, page) = (&dxf.files[0], &pages.pages[0]);
    let on = pages.pages[1].label.clone();
    let sheet = |s: &Session, underlays: bool| {
        s.export_pages(&ExportOptions {
            pages: PageSelection::Folios(vec![on.clone()]),
            underlays,
            ..ExportOptions::default()
        })
        .unwrap()
        .pages
        .remove(0)
        .content
        .items
    };
    let bare = sheet(&golden, true).len();
    let source = ew_api::output::UnderlaySource::Dxf(DxfOptions::default());
    golden
        .place_underlay(&on, &file.name, file.text.clone().into_bytes(), &source, true, Vec::new())
        .unwrap();
    let with = sheet(&golden, true);
    let underlay = &with[..with.len() - bare];
    let mut original = Vec::new();
    drawn_points(&page.content.items, Transform::IDENTITY, &mut original);
    let mut printed = Vec::new();
    drawn_points(underlay, Transform::IDENTITY, &mut printed);
    all_near(&file.name, "underlay's drawn point", &original, &printed);
}

/// The baseline start of every glyph run of `items` on the sheet.
fn baselines(items: &[Item], t: Transform, out: &mut Vec<Pt2>) {
    for item in items {
        match &item.kind {
            ItemKind::Glyphs(run) if !run.text.trim().is_empty() => out.push(
                run.transform
                    .then(Transform::from_translation(run.origin))
                    .then(t)
                    .apply(Pt2::ORIGIN),
            ),
            ItemKind::Group(g) => baselines(&g.items, g.transform.map_or(t, |gt| gt.then(t)), out),
            _ => {}
        }
    }
}

/// Every point of `want` within 0.01 mm of a point of `got`.
fn all_near(file: &str, what: &str, want: &[Pt2], got: &[Pt2]) {
    let cell = |p: Pt2| (p.x.0.div_euclid(TOLERANCE), p.y.0.div_euclid(TOLERANCE));
    let mut grid: BTreeMap<(i64, i64), Vec<Pt2>> = BTreeMap::new();
    for p in got {
        grid.entry(cell(*p)).or_default().push(*p);
    }
    for p in want {
        let (cx, cy) = cell(*p);
        let found = (cx - 1..=cx + 1)
            .any(|x| (cy - 1..=cy + 1).any(|y| grid.get(&(x, y)).is_some_and(|ps| ps.iter().any(|q| near(*q, *p)))));
        assert!(
            found,
            "{file}: the {what} at ({}, {}) nm has none within 0.01 mm",
            p.x.0, p.y.0
        );
    }
}

/// A point of `x`, `y` millimetres.
fn mm(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm(x * 1_000_000), Nm(y * 1_000_000))
}

fn near(a: Pt2, b: Pt2) -> bool {
    (a.x.0 - b.x.0).abs() <= TOLERANCE && (a.y.0 - b.y.0).abs() <= TOLERANCE
}

fn run(origin: Pt2, transform: Transform) -> Item {
    Item::glyphs(Arc::new(ShapedRun {
        font: FontKey {
            sha256: [0; 32],
            face_index: 0,
        },
        size: Nm(5_000_000),
        origin,
        transform,
        glyphs: Vec::new(),
        text: "Ää°²".to_owned(),
        brush: Brush::BLACK,
        layout_box: Rect::from_points(mm(0, 0), mm(1, 1)),
    }))
}

#[test]
fn an_insert_and_a_text_come_back_where_they_were_written_with_their_characters() {
    // One symbol geometry (an open polyline of three points) inserted four times: moved, mirrored, turned by a
    // quarter turn of its variant and by 90° on the sheet; and two runs of "Ää°²", one of them turned.
    let symbol = SymbolGeomId {
        symbol: ew_ids::Uuid::from_u128(7),
        variant: Orient2::R0,
        extra: None,
    };
    let outline = [mm(0, 0), mm(4, 0), mm(4, 2)];
    let geometry: BTreeMap<_, _> = [(
        symbol,
        Arc::new(DisplayList {
            items: vec![Item::path(
                Path::polyline(&outline),
                Some(Stroke::solid(Nm(250_000))),
                None,
            )],
        }),
    )]
    .into();
    let placed = [
        Transform::from_translation(mm(30, 40)),
        Transform::new(mm(100, 60), Orient2::M0, Angle2(0)),
        Transform::new(mm(150, 100), Orient2::R0, Angle2(90_000)),
        Transform::new(mm(200, 40), Orient2::R90, Angle2(0)),
    ];
    let turned = Transform::from_angle(Angle2(90_000));
    let sheet = ew_export::DxfSheet {
        label: "T1".to_owned(),
        width: Nm(297_000_000),
        height: Nm(210_000_000),
        layers: vec![
            (
                Layer::Symbols,
                DisplayList {
                    items: placed.iter().map(|t| Item::instance(symbol, *t)).collect(),
                },
            ),
            (
                Layer::Frame,
                DisplayList {
                    items: vec![run(mm(50, 60), Transform::IDENTITY), run(mm(250, 150), turned)],
                },
            ),
        ],
        geometry,
        scale_ppb: 1_000_000_000,
    };
    let file = ew_export::dxf(&[sheet], &ew_export::DxfOptions::default()).remove(0);
    assert!(
        file.text.is_ascii(),
        "a reader may assume any code page: the file is ASCII"
    );
    let converted = ew_import_dxf::convert(
        file.text.as_bytes(),
        &Default::default(),
        None,
        &Space::Model,
        &BTreeSet::new(),
        &file.name,
    )
    .unwrap();

    // The inserts: every point of every placed geometry is a vertex of the re-imported drawing, and no other.
    let wanted: Vec<Pt2> = placed
        .iter()
        .flat_map(|t| outline.iter().map(|q| t.apply(*q)))
        .collect();
    let got = imported_points(&converted.items);
    assert_eq!(got.len(), wanted.len());
    for w in &wanted {
        assert!(
            got.iter().any(|g| near(*g, *w)),
            "no vertex near {w:?} in {got:?}"
        );
    }

    // The texts: the characters as written, at the anchor the run's origin and angle give (the importer anchors a
    // text on its vertical middle, half the cap height, 0.714 em, up from the baseline).
    let half_cap = 5_000_000 * 714 / 1000 / 2;
    let texts: Vec<(String, Pt2)> = converted
        .items
        .iter()
        .filter_map(|(_, _, k)| match k {
            PageItemKind::FreeText(t) => Some((t.text.as_neutral()?.to_owned(), t.at)),
            _ => None,
        })
        .collect();
    assert_eq!(texts.len(), 2, "{texts:?}");
    assert!(texts.iter().all(|(t, _)| t == "Ää°²"), "{texts:?}");
    for want in [
        mm(50, 60) + Pt2::new(Nm(0), Nm(-half_cap)),
        mm(250, 150) + Pt2::new(Nm(-half_cap), Nm(0)),
    ] {
        assert!(
            texts.iter().any(|(_, at)| near(*at, want)),
            "no text near {want:?} in {texts:?}"
        );
    }
}
