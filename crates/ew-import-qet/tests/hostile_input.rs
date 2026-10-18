// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The doc 14 §11 time-bound regressions: hostile element definitions are rejected, or reported with
//! bounded output, in linear time. The bounds are generous (a debug build is allowed 15 s where a release
//! build must stay below 1 s) so that the tests cannot flake; a quadratic implementation misses them by
//! orders of magnitude.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use ew_ids::{Id, Uuid};
use ew_import_qet::MAX_ENTRIES_PER_DEFINITION;
use ew_import_qet::QetElementImporter;
use ew_import_qet::decimal::Decimal;
use ew_import_qet::prims::shapes::polygon;
use ew_import_qet::prims::{Parsed, PrimCtx};
use ew_interop_api::{Code, ImportBatch, ImportContext, ImportError, ImportInput, Importer};
use ew_schema::Primitive;

/// The time a hostile input may take: 1 s in release (doc 14 §11), with slack for debug builds.
fn budget() -> Duration {
    if cfg!(debug_assertions) {
        Duration::from_secs(15)
    } else {
        Duration::from_secs(1)
    }
}

/// Imports `text` as `x.elmt` and returns the result with the time it took.
fn timed_import(text: &str) -> (Result<ImportBatch, ImportError>, Duration) {
    let input = ImportInput::new("x.elmt", text.as_bytes().to_vec()).unwrap();
    let mut ctx = ImportContext::new(Id::from_uuid(Uuid::from_u128(1)));
    let start = Instant::now();
    let result = QetElementImporter.import(&input, &mut ctx);
    (result, start.elapsed())
}

/// The shared checks of the two finding-flood tests.
fn check_flood(text: &str, name: &str) {
    const CHILDREN: usize = 200_000;
    let (result, elapsed) = timed_import(text);
    eprintln!("{name}: {elapsed:?}");
    assert!(elapsed < budget(), "{name} took {elapsed:?}");
    let report = result.unwrap().report;
    assert_eq!(report.entries.len(), MAX_ENTRIES_PER_DEFINITION + 1);
    let last = report.entries.last().unwrap();
    assert_eq!(last.code, Code::LIMIT);
    // 200 000 unsupported elements and one black-box finding, of which 1 000 are listed.
    let suppressed = CHILDREN + 1 - MAX_ENTRIES_PER_DEFINITION;
    assert_eq!(suppressed, 199_001);
    assert!(
        last.message
            .starts_with(&format!("{suppressed} further findings")),
        "{}",
        last.message
    );
    // The location of a finding is the line `roxmltree` reports for its element.
    let doc = roxmltree::Document::parse(text).unwrap();
    let k = MAX_ENTRIES_PER_DEFINITION - 1;
    let child = doc.descendants().filter(|n| n.has_tag_name("x")).nth(k).unwrap();
    let expected = doc.text_pos_at(child.range().start).row;
    assert_eq!(expected, u32::try_from(k + 2).unwrap());
    assert_eq!(report.entries[k].location.as_ref().unwrap().line, Some(expected));
}

/// C1: 200 000 unsupported root children give a capped, linear report.
#[test]
fn many_root_findings_are_linear() {
    let mut text = String::from("<definition type=\"element\">\n");
    for _ in 0..200_000 {
        text.push_str("<x/>\n");
    }
    text.push_str("</definition>");
    check_flood(&text, "many_root_findings_are_linear");
}

/// C1: the same inside `<description>`.
#[test]
fn many_description_findings_are_linear() {
    let mut text = String::from("<definition type=\"element\"><description>\n");
    for _ in 0..200_000 {
        text.push_str("<x/>\n");
    }
    text.push_str("</description></definition>");
    check_flood(&text, "many_description_findings_are_linear");
}

/// A `<polygon>` with `n` points as attributes `x1 y1 x2 y2 …`.
fn polygon_element(n: usize) -> String {
    let mut s = String::from("<polygon");
    for i in 1..=n {
        let (x, y) = if i % 2 == 0 { (10, 10) } else { (0, 0) };
        write!(s, " x{i}=\"{x}\" y{i}=\"{y}\"").unwrap();
    }
    s.push_str("/>");
    s
}

/// C2 mechanism: `polygon()` walks the attributes once, not once per point.
#[test]
fn long_polygon_is_linear() {
    let text = polygon_element(16_000);
    // roxmltree's duplicate-attribute check is quadratic, bounded by the attribute limit and not under
    // test here, so the parse stays outside the timed section.
    let doc = roxmltree::Document::parse(&text).unwrap();
    let node = doc.root_element();
    let mut ctx = PrimCtx::new("x.elmt", None, Decimal::ZERO, Decimal::ZERO);
    let mut out = Parsed::default();
    let start = Instant::now();
    polygon(node, &mut ctx, &mut out).unwrap();
    let elapsed = start.elapsed();
    eprintln!("long_polygon_is_linear: {elapsed:?}");
    let bound = if cfg!(debug_assertions) {
        Duration::from_secs(5)
    } else {
        Duration::from_millis(500)
    };
    assert!(elapsed < bound, "polygon() took {elapsed:?}");
    let [Primitive::Polygon(p)] = out.graphics.as_slice() else {
        panic!("expected one polygon, got {:?}", out.graphics.len());
    };
    assert_eq!(p.points.len(), 16_000);
}

/// C2 limit: an element with more attributes than `max_xml_attributes_per_element` is refused before
/// the XML is parsed.
#[test]
fn hostile_polygon_is_refused_before_parsing() {
    let text = format!(
        "<definition type=\"element\"><description>{}</description></definition>",
        polygon_element(100_000)
    );
    let (result, elapsed) = timed_import(&text);
    eprintln!("hostile_polygon_is_refused_before_parsing: {elapsed:?}");
    assert!(elapsed < budget(), "took {elapsed:?}");
    assert_eq!(
        result.err(),
        Some(ImportError::LimitExceeded {
            what: "XML attributes per element".into(),
            limit: 8_192
        })
    );
}

/// C2 limit: a polygon just under the per-element limit still imports, end to end, in linear time.
#[test]
fn polygon_just_under_the_attribute_limit_imports_within_budget() {
    let text = format!(
        "<definition type=\"element\"><description>{}</description></definition>",
        polygon_element(4_095)
    );
    let (result, elapsed) = timed_import(&text);
    eprintln!("polygon_just_under_the_attribute_limit_imports_within_budget: {elapsed:?}");
    assert!(elapsed < budget(), "took {elapsed:?}");
    let batch = result.unwrap();
    let symbols: Vec<_> = batch
        .lib
        .iter()
        .filter_map(|d| match d {
            ew_schema::LibDocument::Symbol(s) => Some(s),
            _ => None,
        })
        .collect();
    let [symbol] = symbols.as_slice() else {
        panic!("expected one symbol");
    };
    let polygons: Vec<_> = symbol
        .item
        .graphics
        .iter()
        .filter_map(|p| match p {
            Primitive::Polygon(p) => Some(p),
            _ => None,
        })
        .collect();
    let [p] = polygons.as_slice() else {
        panic!("expected one polygon");
    };
    assert_eq!(p.points.len(), 4_095);
}

/// C2 limit: many small elements add up to more than `max_xml_attributes`.
#[test]
fn attribute_flood_across_elements_is_refused() {
    let mut element = String::from("<x");
    for i in 0..16 {
        write!(element, " a{i}=\"0\"").unwrap();
    }
    element.push_str("/>\n");
    let mut text = String::from("<definition type=\"element\">\n");
    for _ in 0..65_537 {
        text.push_str(&element);
    }
    text.push_str("</definition>");
    let (result, elapsed) = timed_import(&text);
    eprintln!("attribute_flood_across_elements_is_refused: {elapsed:?}");
    assert!(elapsed < budget(), "took {elapsed:?}");
    assert_eq!(
        result.err(),
        Some(ImportError::LimitExceeded {
            what: "XML attribute count".into(),
            limit: 1_048_576
        })
    );
}
