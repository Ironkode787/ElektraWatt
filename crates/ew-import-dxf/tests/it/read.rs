// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reading (doc 14 §6.1, FR-IMP-032): every entity type of the coverage table in an R12, a 2000 and a 2018 drawing,
//! ASCII and binary, imported as a graphic page; units, the y flip, the reports.

use std::collections::{BTreeMap, BTreeSet};

use ew_import_dxf::ew_interop_api::ew_schema::{GraphicShape, PageItemKind, Record};
use ew_import_dxf::ew_interop_api::{ImportError, Limits};
use ew_import_dxf::{DxfOptions, PageSpec, Space, Unit, convert, import_page, probe};
use ew_units::{Grid, Nm, Pt2};

use crate::common::{Dxf, ctx, input};

/// The tables of a drawing: layers `0`, `WIRES` (dashed), `OFF` (off) and an SHX text style.
fn tables(d: &mut Dxf) {
    d.section("TABLES");
    d.ps(&[(0, "TABLE"), (2, "LTYPE"), (70, "1")]);
    d.ps(&[
        (0, "LTYPE"),
        (2, "DASHED"),
        (70, "0"),
        (73, "2"),
        (40, "0.75"),
        (49, "0.5"),
        (49, "-0.25"),
    ]);
    d.p(0, "ENDTAB");
    d.ps(&[(0, "TABLE"), (2, "LAYER"), (70, "3")]);
    d.ps(&[(0, "LAYER"), (2, "0"), (70, "0"), (62, "7"), (6, "CONTINUOUS")]);
    d.ps(&[(0, "LAYER"), (2, "WIRES"), (70, "0"), (62, "1"), (6, "DASHED")]);
    d.ps(&[(0, "LAYER"), (2, "OFF"), (70, "0"), (62, "-3"), (6, "CONTINUOUS")]);
    d.p(0, "ENDTAB");
    d.ps(&[(0, "TABLE"), (2, "STYLE"), (70, "1")]);
    d.ps(&[
        (0, "STYLE"),
        (2, "STANDARD"),
        (70, "0"),
        (40, "0"),
        (41, "1"),
        (3, "romans.shx"),
    ]);
    d.p(0, "ENDTAB");
    d.end();
}

/// One of every entity of the coverage table (handles from `100`), in model space.
fn coverage(version: &str, units: Option<i32>, handles: bool) -> Dxf {
    let mut d = Dxf::new(version, units);
    tables(&mut d);
    let mut h = 0x100;
    let mut head = |d: &mut Dxf, kind: &str, layer: &str| {
        d.p(0, kind);
        if handles {
            d.p(5, format!("{h:X}"));
            h += 1;
        }
        d.p(8, layer);
    };
    d.section("BLOCKS");
    // A block with a line, a constant and a variable attribute definition, nested in a second block.
    d.ps(&[
        (0, "BLOCK"),
        (8, "0"),
        (2, "INNER"),
        (70, "2"),
        (10, "0"),
        (20, "0"),
        (3, "INNER"),
    ]);
    d.ps(&[(0, "LINE"), (8, "0"), (10, "0"), (20, "0"), (11, "10"), (21, "0")]);
    d.ps(&[
        (0, "ATTDEF"),
        (8, "0"),
        (10, "0"),
        (20, "5"),
        (40, "2.5"),
        (1, "K1"),
        (3, "Tag?"),
        (2, "TAG"),
        (70, "0"),
    ]);
    d.ps(&[(0, "ENDBLK"), (8, "0")]);
    d.ps(&[
        (0, "BLOCK"),
        (8, "0"),
        (2, "OUTER"),
        (70, "0"),
        (10, "0"),
        (20, "0"),
        (3, "OUTER"),
    ]);
    d.ps(&[(0, "CIRCLE"), (8, "0"), (10, "0"), (20, "0"), (40, "2")]);
    d.ps(&[(0, "INSERT"), (8, "0"), (2, "INNER"), (10, "5"), (20, "5")]);
    d.ps(&[(0, "ENDBLK"), (8, "0")]);
    d.ps(&[
        (0, "BLOCK"),
        (8, "0"),
        (2, "*D1"),
        (70, "1"),
        (10, "0"),
        (20, "0"),
        (3, "*D1"),
    ]);
    d.ps(&[
        (0, "LINE"),
        (8, "0"),
        (10, "100"),
        (20, "100"),
        (11, "150"),
        (21, "100"),
    ]);
    d.ps(&[(0, "ENDBLK"), (8, "0")]);
    d.end();
    d.section("ENTITIES");
    head(&mut d, "LINE", "WIRES");
    d.ps(&[(10, "0"), (20, "0"), (11, "100"), (21, "0")]);
    head(&mut d, "LWPOLYLINE", "0");
    d.ps(&[
        (90, "3"),
        (70, "0"),
        (43, "0.5"),
        (10, "0"),
        (20, "10"),
        (42, "1"),
        (10, "10"),
        (20, "10"),
        (10, "20"),
        (20, "20"),
    ]);
    head(&mut d, "POLYLINE", "0");
    d.ps(&[(66, "1"), (70, "1"), (10, "0"), (20, "0")]);
    for (x, y) in [("30", "30"), ("40", "30"), ("40", "40")] {
        d.ps(&[(0, "VERTEX"), (8, "0"), (10, x), (20, y), (42, "0")]);
    }
    d.ps(&[(0, "SEQEND"), (8, "0")]);
    head(&mut d, "ARC", "0");
    d.ps(&[(10, "50"), (20, "50"), (40, "5"), (50, "0"), (51, "90")]);
    head(&mut d, "CIRCLE", "0");
    d.ps(&[(10, "60"), (20, "50"), (40, "5")]);
    head(&mut d, "ELLIPSE", "0");
    d.ps(&[
        (10, "70"),
        (20, "50"),
        (11, "10"),
        (21, "0"),
        (40, "0.5"),
        (41, "0"),
        (42, "6.283185307179586"),
    ]);
    head(&mut d, "SPLINE", "0");
    d.ps(&[(70, "8"), (71, "3"), (72, "8"), (73, "4")]);
    for k in ["0", "0", "0", "0", "1", "1", "1", "1"] {
        d.p(40, k);
    }
    for (x, y) in [("80", "0"), ("85", "10"), ("90", "-10"), ("95", "0")] {
        d.p(10, x).p(20, y);
    }
    head(&mut d, "SOLID", "0");
    d.ps(&[
        (10, "0"),
        (20, "60"),
        (11, "5"),
        (21, "60"),
        (12, "0"),
        (22, "65"),
        (13, "5"),
        (23, "65"),
    ]);
    head(&mut d, "TRACE", "0");
    d.ps(&[
        (10, "10"),
        (20, "60"),
        (11, "15"),
        (21, "60"),
        (12, "10"),
        (22, "65"),
        (13, "15"),
        (23, "65"),
    ]);
    head(&mut d, "POINT", "0");
    d.ps(&[(10, "20"), (20, "60")]);
    head(&mut d, "HATCH", "0");
    d.ps(&[
        (10, "0"),
        (20, "0"),
        (2, "SOLID"),
        (70, "1"),
        (71, "0"),
        (91, "1"),
        (92, "3"),
        (72, "0"),
        (73, "1"),
        (93, "3"),
    ]);
    d.ps(&[
        (10, "30"),
        (20, "60"),
        (10, "35"),
        (20, "60"),
        (10, "35"),
        (20, "65"),
        (97, "0"),
        (75, "0"),
        (76, "1"),
        (98, "0"),
    ]);
    head(&mut d, "HATCH", "0");
    d.ps(&[
        (10, "0"),
        (20, "0"),
        (2, "ANSI31"),
        (70, "0"),
        (71, "0"),
        (91, "1"),
        (92, "1"),
        (93, "2"),
    ]);
    d.ps(&[
        (72, "1"),
        (10, "40"),
        (20, "60"),
        (11, "45"),
        (21, "60"),
        (72, "2"),
        (10, "40"),
        (20, "60"),
        (40, "5"),
        (50, "0"),
        (51, "180"),
        (73, "1"),
    ]);
    d.ps(&[
        (97, "0"),
        (75, "0"),
        (76, "1"),
        (52, "0"),
        (41, "1"),
        (77, "0"),
        (78, "0"),
        (98, "0"),
    ]);
    head(&mut d, "TEXT", "0");
    d.ps(&[
        (10, "0"),
        (20, "80"),
        (40, "2.5"),
        (1, "%%c10 mm"),
        (50, "0"),
        (7, "STANDARD"),
    ]);
    head(&mut d, "MTEXT", "0");
    d.ps(&[
        (10, "0"),
        (20, "90"),
        (40, "2.5"),
        (71, "1"),
        (1, "{\\fArial|b1;Line one}\\PLine \\S1^2;"),
    ]);
    head(&mut d, "INSERT", "0");
    d.ps(&[
        (66, "1"),
        (2, "OUTER"),
        (10, "100"),
        (20, "100"),
        (41, "-1"),
        (42, "1"),
        (50, "90"),
    ]);
    d.ps(&[
        (0, "ATTRIB"),
        (8, "0"),
        (10, "100"),
        (20, "105"),
        (40, "2.5"),
        (1, "-K1"),
        (2, "TAG"),
        (70, "0"),
    ]);
    d.ps(&[(0, "SEQEND"), (8, "0")]);
    head(&mut d, "INSERT", "0");
    d.ps(&[
        (2, "INNER"),
        (10, "120"),
        (20, "100"),
        (70, "2"),
        (71, "2"),
        (44, "20"),
        (45, "20"),
    ]);
    head(&mut d, "DIMENSION", "0");
    d.ps(&[(2, "*D1"), (10, "100"), (20, "100"), (70, "0")]);
    head(&mut d, "LEADER", "0");
    d.ps(&[(76, "2"), (10, "0"), (20, "100"), (10, "10"), (20, "110")]);
    head(&mut d, "IMAGE", "0");
    d.ps(&[
        (10, "150"),
        (20, "0"),
        (11, "0.1"),
        (21, "0"),
        (12, "0"),
        (22, "0.1"),
        (13, "100"),
        (23, "50"),
    ]);
    head(&mut d, "3DFACE", "0");
    d.ps(&[
        (10, "0"),
        (20, "0"),
        (11, "1"),
        (21, "0"),
        (12, "1"),
        (22, "1"),
        (13, "0"),
        (23, "1"),
    ]);
    head(&mut d, "LINE", "OFF");
    d.ps(&[(10, "0"), (20, "0"), (11, "1"), (21, "1")]);
    d.end();
    d
}

fn page() -> PageSpec {
    PageSpec {
        section: None,
        number: 900,
        grid: Grid::MM4,
        title: "plant".to_owned(),
    }
}

/// The shapes of a batch's page items by variant (`line`, `polyline`, `polygon`, `circle`, `arc`, `free_text`).
fn kinds(records: &[Record]) -> BTreeMap<&'static str, usize> {
    let mut out = BTreeMap::new();
    for r in records {
        let Record::PageItem(i) = r else { continue };
        let k = match &i.kind {
            PageItemKind::Graphic(g) => match &g.shape {
                GraphicShape::Line(_) => "line",
                GraphicShape::Polyline(_) => "polyline",
                GraphicShape::Polygon(_) if g.fill => "filled",
                GraphicShape::Polygon(_) => "polygon",
                GraphicShape::Circle(_) => "circle",
                GraphicShape::Arc(_) => "arc",
                _ => "other",
            },
            PageItemKind::FreeText(_) => "free_text",
            _ => "other",
        };
        *out.entry(k).or_default() += 1;
    }
    out
}

fn codes(batch: &ew_import_dxf::ew_interop_api::ImportBatch) -> BTreeSet<String> {
    batch.report.entries.iter().map(|e| e.code.to_string()).collect()
}

fn texts(records: &[Record]) -> Vec<String> {
    let mut t: Vec<String> = records
        .iter()
        .filter_map(|r| match r {
            Record::PageItem(i) => match &i.kind {
                PageItemKind::FreeText(f) => f.text.as_neutral().map(str::to_owned),
                _ => None,
            },
            _ => None,
        })
        .collect();
    t.sort();
    t
}

#[test]
fn every_entity_of_the_coverage_table_imports_in_r12_2000_and_2018_ascii_and_binary() {
    for (version, handles) in [("AC1009", false), ("AC1015", true), ("AC1032", true)] {
        let d = coverage(version, Some(4), handles);
        for (form, bytes) in [("ascii", d.ascii()), ("binary", d.binary())] {
            let batch = import_page(
                &input("plant.dxf", bytes),
                &ctx(),
                &DxfOptions::default(),
                &page(),
                None,
            )
            .unwrap_or_else(|e| panic!("{version} {form}: {e}"));
            let k = kinds(&batch.records);
            for kind in [
                "line",
                "polyline",
                "polygon",
                "filled",
                "circle",
                "arc",
                "free_text",
            ] {
                assert!(
                    k.get(kind).is_some_and(|n| *n > 0),
                    "{version} {form}: no {kind} in {k:?}"
                );
            }
            assert_eq!(
                texts(&batch.records),
                ["-K1", "Line 1/2", "Line one", "Ø10 mm"],
                "{version} {form}"
            );
            let c = codes(&batch);
            for code in [
                "EW-IMP-004",
                "EW-IMP-252",
                "EW-IMP-253",
                "EW-IMP-254",
                "EW-IMP-255",
            ] {
                assert!(c.contains(code), "{version} {form}: {code} missing from {c:?}");
            }
            assert!(
                !c.contains("EW-IMP-251"),
                "{version} {form}: the header names a unit"
            );
        }
    }
}

#[test]
fn ascii_and_binary_give_the_same_batch() {
    let d = coverage("AC1015", Some(4), true);
    let a = import_page(
        &input("a.dxf", d.ascii()),
        &ctx(),
        &DxfOptions::default(),
        &page(),
        None,
    )
    .unwrap();
    let b = import_page(
        &input("a.dxf", d.binary()),
        &ctx(),
        &DxfOptions::default(),
        &page(),
        None,
    )
    .unwrap();
    // The lineages differ (another file hash); the pages and items are equal once their IDs are set aside.
    let strip = |r: &[Record]| -> Vec<String> {
        r.iter()
            .filter_map(|r| match r {
                Record::PageItem(i) => Some(format!("{:?}{:?}", i.layer, i.kind)),
                _ => None,
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    assert_eq!(strip(&a.records), strip(&b.records));
}

/// What follows EOF is not read: Windows' `copy` of several files appends an MS-DOS end-of-file mark (Ctrl+Z).
#[test]
fn bytes_after_eof_are_not_read() {
    let mut bytes = coverage("AC1015", Some(4), true).ascii();
    let clean = import_page(&input("a.dxf", bytes.clone()), &ctx(), &DxfOptions::default(), &page(), None).unwrap();
    bytes.push(0x1a);
    let marked = import_page(&input("a.dxf", bytes), &ctx(), &DxfOptions::default(), &page(), None).unwrap();
    assert_eq!(kinds(&marked.records), kinds(&clean.records));
}

#[test]
fn a_drawing_without_a_unit_asks_for_one_and_reports_the_choice() {
    let d = coverage("AC1015", None, true).ascii();
    let e = import_page(
        &input("a.dxf", d.clone()),
        &ctx(),
        &DxfOptions::default(),
        &page(),
        None,
    )
    .unwrap_err();
    assert_eq!(
        e,
        ImportError::Unsupported {
            message: ew_import_dxf::NO_UNIT.to_owned()
        }
    );
    assert_eq!(probe(&d, &Limits::default()).unwrap().unit, None);
    let options = DxfOptions {
        unit: Some(Unit::Inch),
        ..DxfOptions::default()
    };
    let batch = import_page(&input("a.dxf", d), &ctx(), &options, &page(), None).unwrap();
    assert!(codes(&batch).contains("EW-IMP-251"));
}

#[test]
fn units_scale_and_the_y_axis_is_flipped_to_the_top_of_the_extents() {
    let line = |units: i32| {
        let mut d = Dxf::new("AC1015", Some(units));
        d.section("ENTITIES");
        d.ps(&[
            (0, "LINE"),
            (5, "A"),
            (8, "0"),
            (10, "1"),
            (20, "2"),
            (11, "3"),
            (21, "5"),
        ]);
        d.end();
        let c = convert(
            &d.ascii(),
            &Limits::default(),
            None,
            &Space::Model,
            &BTreeSet::new(),
            "a.dxf",
        )
        .unwrap();
        match &c.items[0].2 {
            PageItemKind::Graphic(g) => match &g.shape {
                GraphicShape::Line(l) => (l.a, l.b),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
    };
    let p = |x: i64, y: i64| Pt2::new(Nm(x), Nm(y));
    // Millimetres: y_top = 5 mm, so (1, 2) lands 3 mm below the top.
    assert_eq!(line(4), (p(1_000_000, 3_000_000), p(3_000_000, 0)));
    assert_eq!(line(1), (p(25_400_000, 76_200_000), p(76_200_000, 0)));
}

#[test]
fn the_import_is_deterministic_and_keys_follow_the_handles() {
    let d = coverage("AC1015", Some(4), true).ascii();
    let a = import_page(
        &input("a.dxf", d.clone()),
        &ctx(),
        &DxfOptions::default(),
        &page(),
        None,
    )
    .unwrap();
    let b = import_page(&input("a.dxf", d), &ctx(), &DxfOptions::default(), &page(), None).unwrap();
    assert_eq!(a, b);
    let ids: BTreeSet<_> = a.records.iter().filter_map(Record::id).collect();
    assert_eq!(ids.len(), a.records.len(), "every record has its own ID");
}

#[test]
fn a_hidden_layer_is_imported_when_shown() {
    let d = coverage("AC1015", Some(4), true).ascii();
    let count = |show: &[&str]| {
        let options = DxfOptions {
            show: show.iter().map(|s| (*s).to_owned()).collect(),
            ..DxfOptions::default()
        };
        let b = import_page(&input("a.dxf", d.clone()), &ctx(), &options, &page(), None).unwrap();
        b.records
            .iter()
            .filter(
                |r| matches!(r, Record::PageItem(i) if i.layer.as_ref().is_some_and(|l| l.as_str() == "OFF")),
            )
            .count()
    };
    assert_eq!(count(&[]), 0);
    assert_eq!(count(&["OFF"]), 1);
}

#[test]
fn a_file_that_is_no_dxf_is_refused() {
    let e = import_page(
        &input("a.dxf", b"hello".to_vec()),
        &ctx(),
        &DxfOptions::default(),
        &page(),
        None,
    )
    .unwrap_err();
    assert!(matches!(e, ImportError::Malformed { .. }), "{e:?}");
}
