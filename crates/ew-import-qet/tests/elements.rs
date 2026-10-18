// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Acceptance tests of the element reader over the synthetic fixtures under `tests/fixtures/` (written by
//! hand from the documented grammar; no QElectroTech collection element is used).

use std::collections::BTreeMap;

use ew_ids::{Id, Uuid, v5};
use ew_import_qet::{BUILTIN_LIBRARY, ElementSource, QetElementImporter, read_element};
use ew_interop_api::{
    Code, Detection, ImportBatch, ImportContext, ImportError, ImportInput, Importer, InputNameError, Limits,
    LineageInfo, Severity,
};
use ew_schema::{
    AnchorPolicy, Colour, ContactKind, Dash, Dir4, FnDef, InternalKind, LibCopy, LibDocument,
    PassThroughSpec, Pen, PotentialType, Primitive, Project, Record, RepType, Rgb, SlotKind, SymbolDef,
};
use ew_units::{Angle2, Grid, Nm, Pt2};
use sha2::Digest;

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(path).unwrap()
}

/// The project imported into.
const PROJECT: Uuid = Uuid::from_u128(0x0192_f3a4_5b6c_7d8e_8f01_2345_6789_ab00);

fn ctx() -> ImportContext {
    ImportContext::new(Id::<Project>::from_uuid(PROJECT))
}

fn import(name: &str) -> ImportBatch {
    let input = ImportInput::new(name, fixture(name)).unwrap();
    QetElementImporter.import(&input, &mut ctx()).unwrap()
}

fn parts(batch: &ImportBatch) -> (&LibCopy<SymbolDef>, Option<&LibCopy<FnDef>>) {
    let mut symbol = None;
    let mut fndef = None;
    for d in &batch.lib {
        match d {
            LibDocument::Symbol(s) => symbol = Some(s),
            LibDocument::Fndef(f) => fndef = Some(f),
            other => panic!("unexpected {other:?}"),
        }
    }
    (symbol.unwrap(), fndef)
}

fn g(k: i64) -> Nm {
    Grid::QET_10PX.at(k).unwrap()
}

fn px(s: &str) -> Nm {
    ew_import_qet::decimal::Decimal::parse(s)
        .unwrap()
        .px_to_nm()
        .unwrap()
}

fn codes(batch: &ImportBatch) -> Vec<(Code, Severity)> {
    batch
        .report
        .entries
        .iter()
        .map(|e| (e.code, e.severity))
        .collect()
}

const FIXTURES: [&str; 6] = [
    "contact_no.elmt",
    "box_shadow.elmt",
    "arc_lamp.elmt",
    "polygon_coil.elmt",
    "rotated_text.elmt",
    "heavy_thumbnail.elmt",
];

#[test]
fn contact_no() {
    let batch = import("contact_no.elmt");
    // "Sans Serif" maps to the bundled family.
    assert_eq!(codes(&batch), [(Code::FONT_SUBSTITUTED, Severity::Info)]);
    let (s, f) = parts(&batch);
    let sym = &s.item;
    assert_eq!(sym.grid, Grid::QET_10PX);
    assert_eq!(sym.anchor, AnchorPolicy::Origin);
    assert_eq!(sym.rep_type, RepType::MultiLine);
    assert_eq!(sym.cps.len(), 2);
    assert_eq!((sym.cps[0].idx, sym.cps[0].fn_cp), (0, 0));
    assert_eq!(sym.cps[0].pos, Pt2::new(Nm(0), g(-2)));
    assert_eq!(sym.cps[0].dir, Dir4::Up);
    assert_eq!(sym.cps[1].pos, Pt2::new(Nm(0), g(2)));
    assert_eq!(sym.cps[1].dir, Dir4::Down);
    assert!(sym.cps[0].extra.contains_key("qet.uuid"));
    assert_eq!(sym.graphics.len(), 3);
    assert_eq!(sym.text_slots.len(), 1);
    assert_eq!(sym.text_slots[0].kind, SlotKind::Tag);
    assert_eq!(sym.extra["qet.link_type"], "slave");

    let f = &f.unwrap().item;
    assert_eq!(
        (
            f.category.0.as_str(),
            f.category.1.as_str(),
            f.category.2.as_str()
        ),
        ("Contact", "simple", "NO")
    );
    assert_eq!(f.contact_kind, ContactKind::No);
    let des: Vec<&str> = f.cps.iter().map(|c| c.designation.as_str()).collect();
    assert_eq!(des, ["13", "14"]);
    assert_eq!(f.internal.len(), 1);
    assert_eq!((f.internal[0].a, f.internal[0].b), (0, 1));
    assert_eq!(
        f.internal[0].kind,
        InternalKind::PassThrough(PassThroughSpec {
            opens: true,
            extra: Default::default()
        })
    );
    assert_eq!(f.extra["qet.kind"]["state"], "NO");
    assert_eq!(batch.report.source.format_version.as_deref(), Some("0.80"));
    assert_eq!(batch.report.source.detail.as_deref(), Some("link type slave"));
}

/// Identity, lineage and provenance (doc 14 §2.3, §4.3).
#[test]
fn identity_and_provenance() {
    let bytes = fixture("contact_no.elmt");
    let batch = import("contact_no.elmt");
    let digest: [u8; 32] = sha2::Sha256::digest(&bytes).into();
    let namespace = v5::lineage_namespace("qet", &v5::LineageAnchor::FirstImportSha256(digest)).unwrap();

    // The new lineage record.
    assert_eq!(batch.records.len(), 1);
    let Record::ImportLineage(l) = &batch.records[0] else {
        panic!("lineage expected");
    };
    assert_eq!(l.namespace.as_uuid(), namespace);
    assert_eq!(l.importer.as_str(), "qet");
    assert_eq!(l.source_name, "contact_no.elmt");
    assert_eq!(l.id.as_uuid(), Uuid::new_v5(&namespace, b"qet:lineage"));

    // Item and copy IDs.
    let (s, f) = parts(&batch);
    let key = s
        .item
        .meta
        .sources
        .iter()
        .find_map(|x| x.strip_prefix("qet.key:"))
        .unwrap();
    assert!(key.starts_with("qet:eldef:") && key.len() == 10 + 64);
    assert_eq!(s.item_id.as_uuid(), Uuid::new_v5(&namespace, key.as_bytes()));
    // H2: the QET import library is project-local, so its `library_id` is the project ID (doc 05 §13.2).
    assert_eq!(s.library_id.as_uuid(), PROJECT);
    assert_eq!(s.id, v5::lib_copy_id(PROJECT, s.item_id.as_uuid()));
    let f = f.unwrap();
    assert_eq!(
        f.item_id.as_uuid(),
        Uuid::new_v5(&namespace, format!("{key}#fndef").as_bytes())
    );
    assert_eq!(s.item.fndef.item, f.item_id);
    assert_eq!(s.item.fndef.library.as_uuid(), PROJECT);
    assert_eq!(s.version, "1.0.0");

    // The copy hash is the hash of the item's canonical compact JSON.
    let json = serde_json::to_string(&s.item).unwrap();
    assert_eq!(
        s.sha256.0,
        <[u8; 32]>::from(sha2::Sha256::digest(json.as_bytes()))
    );

    // Provenance: file name only, never a path; source_ref names the lineage and the key.
    let sources = &s.item.meta.sources;
    assert_eq!(sources[0], "qet.file:contact_no.elmt");
    assert!(sources.contains(&"qet.name:Synthetic NO contact".to_owned()));
    assert!(sources.contains(&"qet.version:0.80".to_owned()));
    assert!(sources.iter().any(|x| x.starts_with("qet.uuid:{")));
    assert!(
        sources
            .iter()
            .all(|x| !x.contains('\\') && !x.contains("/fixtures"))
    );
    assert!(s.item.meta.foreign_terms);
    assert_eq!(s.item.extra["source_ref"]["lineage"], l.id.to_string());
    assert_eq!(s.item.extra["source_ref"]["key"], key);

    // The same bytes under another file name: the same definition, the same IDs (content-addressed key,
    // file-anchored lineage).
    let renamed = ImportInput::new("other-name.elmt", bytes.clone()).unwrap();
    let again = QetElementImporter.import(&renamed, &mut ctx()).unwrap();
    assert_eq!(parts(&again).0.id, s.id);

    // Another project: the same items (lineage-derived), other copies (library = project).
    let other = Uuid::from_u128(0x0192_f3a4_5b6c_7d8e_8f01_2345_6789_ab01);
    let input = ImportInput::new("contact_no.elmt", bytes).unwrap();
    let elsewhere = QetElementImporter
        .import(&input, &mut ImportContext::new(Id::from_uuid(other)))
        .unwrap();
    let (s2, f2) = parts(&elsewhere);
    assert_eq!(s2.item_id, s.item_id);
    assert_eq!(s2.library_id.as_uuid(), other);
    assert_eq!(s2.id, v5::lib_copy_id(other, s.item_id.as_uuid()));
    assert_ne!(s2.id, s.id);
    assert_eq!(s2.item.fndef.library.as_uuid(), other);
    assert_eq!(f2.unwrap().library_id.as_uuid(), other);
}

#[test]
fn deterministic_and_schema_valid() {
    for name in FIXTURES {
        let a = import(name);
        let b = import(name);
        assert_eq!(a, b, "{name}");
        // Every library document survives a JSON round trip through the strict schema types.
        for d in &a.lib {
            let json = serde_json::to_string(d).unwrap();
            let back: LibDocument = serde_json::from_str(&json).unwrap();
            assert_eq!(&back, d, "{name}");
            d.validate().unwrap();
            let strict = LibDocument::from_value(serde_json::to_value(d).unwrap()).unwrap();
            assert_eq!(&strict, d, "{name}");
        }
        for r in &a.records {
            let json = serde_json::to_string(r).unwrap();
            let back: Record = serde_json::from_str(&json).unwrap();
            assert_eq!(&back, r, "{name}");
            r.validate().unwrap();
            let strict = Record::from_value(serde_json::to_value(r).unwrap()).unwrap();
            assert_eq!(&strict, r, "{name}");
        }
        assert!(!a.report.has_errors(), "{name}: {:?}", a.report.entries);
    }
}

/// FR-IMP-008: every CP of every fixture lies exactly on the 10 px grid.
#[test]
fn cps_on_the_qet_grid() {
    let grid = Grid::QET_10PX;
    for name in FIXTURES {
        let batch = import(name);
        for cp in &parts(&batch).0.item.cps {
            for c in [cp.pos.x, cp.pos.y] {
                assert_eq!(grid.at(grid.snap(c)), Some(c), "{name}");
            }
        }
    }
}

#[test]
fn box_shadow_black_box() {
    let batch = import("box_shadow.elmt");
    let c = codes(&batch);
    assert!(c.contains(&(Code::BLACK_BOX, Severity::Warning)), "{c:?}");
    assert!(c.contains(&(Code::FONT_SUBSTITUTED, Severity::Info)), "{c:?}");
    let (s, f) = parts(&batch);
    let f = &f.unwrap().item;
    assert_eq!(f.contact_kind, ContactKind::BlackBox);
    // The empty terminal name falls back to the 1-based definition order.
    assert_eq!(f.cps[0].designation.as_str(), "1");
    assert_eq!(f.cps[0].extra["qet.name"], "");
    assert_eq!(f.cps[1].designation.as_str(), "B");
    assert_eq!(f.cps[1].extra["qet.type"], "Outer");
    let sym = &s.item;
    assert_eq!(sym.text_slots[0].kind, SlotKind::Tag);
    let Primitive::Rect(shadow) = &sym.graphics[0] else {
        panic!()
    };
    assert!(shadow.stroke.is_none());
    assert_eq!(shadow.fill, Some(Colour::ByLayer));
    let Primitive::Rect(rect) = &sym.graphics[1] else {
        panic!()
    };
    assert_eq!((rect.rx, rect.ry), (px("4"), px("4")));
    assert_eq!(rect.fill, Some(Colour::Rgb(Rgb([255, 255, 255]))));
    let Primitive::Text(t) = &sym.graphics[2] else {
        panic!()
    };
    assert_eq!(t.style.as_ref().unwrap().as_str(), "qet.8.b");
}

#[test]
fn arc_lamp_and_category_path() {
    let batch = import("arc_lamp.elmt");
    let (s, _) = parts(&batch);
    let Primitive::Arc(arc) = &s.item.graphics[0] else {
        panic!()
    };
    assert_eq!((arc.start, arc.sweep), (Angle2(0), Angle2(180_000)));
    assert_eq!(arc.rect.0.min(), Pt2::new(g(-1), g(-1)));
    let Primitive::Circle(c) = &s.item.graphics[1] else {
        panic!()
    };
    assert_eq!((c.centre, c.r), (Pt2::ORIGIN, g(1)));
    // Without a path: black box. With a collection path: the category table (step 3).
    assert!(codes(&batch).contains(&(Code::BLACK_BOX, Severity::Warning)));
    let lineage = LineageInfo {
        id: Id::from_uuid(Uuid::from_u128(1)),
        namespace: Uuid::from_u128(2),
    };
    let xml = String::from_utf8(fixture("arc_lamp.elmt")).unwrap();
    let src = ElementSource::new(
        "project.qet",
        Some("embed://import/10_electric/40_signaling/lamp.elmt"),
    )
    .unwrap();
    let el = read_element(
        &xml,
        &src,
        &lineage,
        Id::from_uuid(PROJECT),
        &Limits::element_definition(),
    )
    .unwrap();
    let f = &el.fndef.unwrap().item;
    assert_eq!(f.category.0.as_str(), "Lamp");
    assert!(el.entries.iter().all(|e| e.code != Code::BLACK_BOX));
    assert!(
        el.symbol
            .item
            .meta
            .sources
            .contains(&"qet.path:embed://import/10_electric/40_signaling/lamp.elmt".to_owned())
    );
    // IDs follow the given lineage namespace.
    assert_eq!(
        el.symbol.item_id.as_uuid(),
        Uuid::new_v5(&lineage.namespace, el.key.as_bytes())
    );
}

#[test]
fn polygon_coil() {
    let batch = import("polygon_coil.elmt");
    let (s, f) = parts(&batch);
    let f = &f.unwrap().item;
    assert_eq!(f.contact_kind, ContactKind::Coil);
    assert!(f.internal.is_empty());
    let g0 = &s.item.graphics;
    let Primitive::Polygon(p) = &g0[0] else { panic!() };
    assert!(p.closed);
    assert_eq!(p.fill, Some(Colour::ByLayer));
    let Primitive::Polyline(pl) = &g0[1] else { panic!() };
    assert_eq!(pl.points[1], Pt2::new(px("-3.33333"), px("5")));
    assert_eq!(pl.points[2], Pt2::new(px("12.3457"), px("7.5")));
    // Line with a triangle end, then the line with a circle start.
    assert!(matches!(g0[2], Primitive::Line(_)));
    let Primitive::Polygon(tri) = &g0[3] else { panic!() };
    assert_eq!(tri.points[0], Pt2::new(Nm(0), g(-1)));
    assert!(matches!(g0[4], Primitive::Line(_)));
    let Primitive::Circle(c) = &g0[5] else { panic!() };
    assert_eq!(c.r, Nm(px("2").0 / 2));
}

#[test]
fn rotated_text_pe_terminal() {
    let batch = import("rotated_text.elmt");
    let (s, f) = parts(&batch);
    let f = &f.unwrap().item;
    assert_eq!(f.contact_kind, ContactKind::Terminal);
    assert!(f.cps.iter().all(|c| c.potential_role == Some(PotentialType::Pe)));
    assert_eq!(f.internal[0].kind, InternalKind::Short);
    assert_eq!(f.cps[0].designation.as_str(), "PE");
    assert_eq!(f.cps[1].designation.as_str(), "2");
    let sym = &s.item;
    let Primitive::Text(t) = &sym.graphics[1] else {
        panic!()
    };
    assert_eq!(t.angle, Angle2(-90_000));
    let slot = &sym.text_slots[0];
    assert_eq!(slot.kind, SlotKind::FunctionText);
    assert_eq!(slot.rot, Angle2(-270_000));
    assert_eq!(slot.max_width, Some(px("40")));
    assert_eq!(slot.extra["qet.style"], "qet.9.75.b.i");
    let comp = &sym.text_slots[1];
    assert_eq!(comp.extra["qet.format"], "%{label} %{function}");
}

#[test]
fn heavy_thumbnail_pens() {
    let batch = import("heavy_thumbnail.elmt");
    let (s, f) = parts(&batch);
    assert_eq!(s.item.rep_type, RepType::Graphic);
    assert!(s.item.cps.is_empty());
    assert_eq!(f.unwrap().item.category.0.as_str(), "Graphic");
    assert_eq!(codes(&batch), []);
    let stroke = |i: usize| match &s.item.graphics[i] {
        Primitive::Line(l) => l.stroke.clone().unwrap(),
        Primitive::Rect(r) => r.stroke.clone().unwrap(),
        other => panic!("{other:?}"),
    };
    assert_eq!((stroke(0).pen, stroke(0).dash), (Pen::P052, Dash::Solid));
    assert_eq!((stroke(1).pen, stroke(1).dash), (Pen::P026, Dash::Dash42));
    assert_eq!((stroke(2).pen, stroke(2).dash), (Pen::P013, Dash::Dot12));
    assert_eq!(stroke(3).dash, Dash::DashDot);
    assert_eq!(stroke(3).colour, Colour::Rgb(Rgb([0, 0, 255])));
    assert_eq!(stroke(4).pen, Pen::P130);
    let names: BTreeMap<String, String> = s
        .item
        .name
        .languages()
        .map(|l| (l.as_str().to_owned(), s.item.name.get(l).unwrap().to_owned()))
        .collect();
    assert_eq!(names["fr"], "Vignette synthetique");
}

#[test]
fn interruption_points_reference_the_builtin() {
    let xml = r#"<definition type="element" version="0.80" link_type="next_report">
        <description><terminal x="0" y="0" orientation="w" name=""/></description></definition>"#;
    let input = ImportInput::new("ip.elmt", xml.as_bytes().to_vec()).unwrap();
    let batch = QetElementImporter.import(&input, &mut ctx()).unwrap();
    let (s, f) = parts(&batch);
    assert!(f.is_none());
    assert_eq!(s.item.fndef.library.as_uuid(), BUILTIN_LIBRARY);
    assert_eq!(s.item.fndef.extra["key"], "ew.fn.ip");
    assert_eq!(s.item.fndef.version_req, "^0.1");
    assert_eq!(
        s.item.fndef.item.as_uuid(),
        Uuid::new_v5(&BUILTIN_LIBRARY, b"ew.fn.ip")
    );
    assert_eq!(batch.report.stats.per_kind.get("fndef"), None);
    assert_eq!(batch.report.stats.per_kind["symbol"], 1);
}

#[test]
fn unsupported_constructs_are_reported_not_fatal() {
    let xml = r#"<definition type="element" link_type="simple">
        <names><name lang="??">x</name></names>
        <extra_stuff/>
        <description>
          <line x1="0" y1="0" x2="10" y2="0" end1="weird" style="line-style:wavy;foo:bar"/>
          <line x1="nan" y1="0" x2="10" y2="0"/>
          <sphere r="3"/>
          <polygon x1="0" y1="0" closed="true"/>
          <terminal x="0" y="0" orientation="q"/>
        </description></definition>"#;
    let input = ImportInput::new("bad.elmt", xml.as_bytes().to_vec()).unwrap();
    let batch = QetElementImporter.import(&input, &mut ctx()).unwrap();
    let c = codes(&batch);
    let count = |code| c.iter().filter(|(k, _)| *k == code).count();
    // Unknown language, unknown root child, unknown end, two style pairs, unknown element: info.
    assert_eq!(count(Code::DROPPED_BY_DESIGN), 6, "{c:?}");
    // NaN coordinate, one-point polygon, bad orientation: skipped items.
    assert_eq!(count(Code::MALFORMED_ITEM), 3, "{c:?}");
    assert!(batch.report.has_errors());
    let (s, _) = parts(&batch);
    assert_eq!(s.item.graphics.len(), 1);
    assert!(s.item.cps.is_empty());
    let loc = batch.report.entries[0].location.as_ref().unwrap();
    assert_eq!(loc.file, "bad.elmt");
    assert!(loc.line.is_some());
}

#[test]
fn file_level_errors() {
    let run = |xml: &[u8]| {
        let input = ImportInput::new("x.elmt", xml.to_vec()).unwrap();
        QetElementImporter.import(&input, &mut ctx())
    };
    assert!(matches!(
        run(b"<definition type=\"element\">"),
        Err(ImportError::Malformed { .. })
    ));
    assert!(matches!(
        run(b"<!DOCTYPE d [<!ENTITY a \"b\">]><definition type=\"element\"/>"),
        Err(ImportError::Malformed { .. })
    ));
    assert!(matches!(run(b"<project/>"), Err(ImportError::Unsupported { .. })));
    assert!(matches!(
        run(b"<definition type=\"diagram\"/>"),
        Err(ImportError::Unsupported { .. })
    ));
    assert!(matches!(
        run(b"<definition type=\"element\">\xff</definition>"),
        Err(ImportError::Unsupported { .. })
    ));
    let deep = format!(
        "<definition type=\"element\">{}{}</definition>",
        "<a>".repeat(300),
        "</a>".repeat(300)
    );
    assert!(matches!(
        run(deep.as_bytes()),
        Err(ImportError::LimitExceeded { .. })
    ));
    let ok = format!(
        "<definition type=\"element\">{}{}</definition>",
        "<a>".repeat(255),
        "</a>".repeat(255)
    );
    assert!(run(ok.as_bytes()).is_ok());
    // A BOM is accepted.
    assert!(run(b"\xEF\xBB\xBF<definition type=\"element\"/>").is_ok());
    // Size limit.
    let input = ImportInput::new("x.elmt", b"<definition type=\"element\"/>".to_vec()).unwrap();
    let mut ctx = ctx();
    ctx.limits.max_input_bytes = 4;
    assert!(matches!(
        QetElementImporter.import(&input, &mut ctx),
        Err(ImportError::LimitExceeded { .. })
    ));
}

#[test]
fn existing_lineage_emits_no_lineage_record() {
    let lineage = LineageInfo {
        id: Id::from_uuid(Uuid::from_u128(7)),
        namespace: Uuid::from_u128(8),
    };
    let input = ImportInput::new("contact_no.elmt", fixture("contact_no.elmt")).unwrap();
    let mut ctx = ImportContext {
        lineage: Some(lineage),
        ..ctx()
    };
    let batch = QetElementImporter.import(&input, &mut ctx).unwrap();
    assert!(batch.records.is_empty());
    let (s, _) = parts(&batch);
    assert_eq!(s.item.extra["source_ref"]["lineage"], lineage.id.to_string());
}

#[test]
fn detection() {
    let d = QetElementImporter;
    assert_eq!(d.id(), "qet");
    assert_eq!(d.detect(&fixture("contact_no.elmt"), "x.bin"), Detection::Certain);
    assert_eq!(d.detect(b"garbage", "A.ELMT"), Detection::Maybe(30));
    assert_eq!(d.detect(b"<project>", "a.qet"), Detection::No);
}

#[test]
fn element_source_refuses_paths() {
    assert_eq!(
        ElementSource::new("dir/x.elmt", None).unwrap_err(),
        InputNameError::NotAFileName
    );
    assert_eq!(ElementSource::new("", None).unwrap_err(), InputNameError::Empty);
    let src = ElementSource::new("x.elmt", Some("embed://import/a/b.elmt")).unwrap();
    assert_eq!(src.file(), "x.elmt");
    assert_eq!(src.path(), Some("embed://import/a/b.elmt"));
}

/// A definition of `n` terminals.
fn definition_with_terminals(n: usize) -> String {
    let mut xml = String::from("<definition type=\"element\"><description>");
    for _ in 0..n {
        xml.push_str("<terminal x=\"0\" y=\"0\" orientation=\"n\"/>");
    }
    xml.push_str("</description></definition>");
    xml
}

fn fixed_lineage() -> LineageInfo {
    LineageInfo {
        id: Id::from_uuid(Uuid::from_u128(1)),
        namespace: Uuid::from_u128(2),
    }
}

#[test]
fn more_than_65535_terminals_are_refused() {
    let src = ElementSource::new("big.elmt", None).unwrap();
    let project = Id::from_uuid(PROJECT);
    // The project-file default: 65,535 terminals of 3 attributes exceed the element preset's attribute cap.
    let limits = Limits::default();
    let err = read_element(
        &definition_with_terminals(65_536),
        &src,
        &fixed_lineage(),
        project,
        &limits,
    )
    .unwrap_err();
    assert!(matches!(
        &err,
        ImportError::LimitExceeded { what, limit: 65_535 } if what == "terminals per element definition"
    ));
    let el = read_element(
        &definition_with_terminals(65_535),
        &src,
        &fixed_lineage(),
        project,
        &limits,
    )
    .unwrap();
    let last = el.symbol.item.cps.iter().last().unwrap();
    assert_eq!((last.idx, last.fn_cp), (65_534, 65_534));
}

#[test]
fn names_are_whitespace_collapsed_like_the_identity() {
    let make = |name: &str| {
        format!(
            "<definition type=\"element\"><names><name lang=\"en\">{name}</name></names>\
             <description><terminal x=\"0\" y=\"0\" orientation=\"n\"/></description></definition>"
        )
    };
    let src = ElementSource::new("x.elmt", None).unwrap();
    let read = |xml: &str| {
        read_element(
            xml,
            &src,
            &fixed_lineage(),
            Id::from_uuid(PROJECT),
            &Limits::element_definition(),
        )
        .unwrap()
    };
    let plain = read(&make("Synthetic NO contact"));
    for messy in [
        "Synthetic\n   NO  contact",
        "  Synthetic NO <!-- c -->contact\t",
        "Synthetic <!-- c --> NO contact",
    ] {
        let other = read(&make(messy));
        assert_eq!(other.key, plain.key, "{messy:?}");
        assert_eq!(other.symbol.item.name, plain.symbol.item.name, "{messy:?}");
        assert_eq!(other.symbol, plain.symbol, "{messy:?}");
        assert_eq!(other.symbol.sha256, plain.symbol.sha256, "{messy:?}");
    }
    let en = ew_mltext::LangTag::parse_lenient("en").unwrap();
    assert_eq!(plain.symbol.item.name.get(&en), Some("Synthetic NO contact"));
    assert!(
        plain
            .symbol
            .item
            .meta
            .sources
            .contains(&"qet.name:Synthetic NO contact".to_owned())
    );
}

#[test]
fn detection_checks_the_root_element() {
    let d = QetElementImporter;
    let project = b"<?xml version=\"1.0\"?><project version=\"0.80\"><collection><element name=\"a.elmt\"><definition type=\"element\" version=\"0.80\">";
    assert_eq!(d.detect(project, "p.qet"), Detection::No);
    assert_eq!(d.detect(project, "p.elmt"), Detection::Maybe(30));
    assert_eq!(
        d.detect(
            b"<!-- <project> --><definition version=\"0.8\" type = 'element'>",
            "x"
        ),
        Detection::Certain
    );
    assert_ne!(
        d.detect(b"<definitionx type=\"element\">", "x"),
        Detection::Certain
    );
    assert_eq!(
        d.detect(
            b"\xEF\xBB\xBF<?xml version=\"1.0\"?>\n<?pi x?><definition type=\"element\">",
            "x"
        ),
        Detection::Certain
    );
    assert_ne!(
        d.detect(b"<definition type=\"thumbnail\">", "x"),
        Detection::Certain
    );
    // An incomplete start tag falls back to the extension rule.
    assert_eq!(
        d.detect(b"<definition type=\"ele", "a.elmt"),
        Detection::Maybe(30)
    );
    assert_eq!(d.detect(b"<definition", "a.bin"), Detection::No);
}
