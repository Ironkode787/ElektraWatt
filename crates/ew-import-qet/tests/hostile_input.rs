// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The doc 14 §11 time-bound regressions: hostile element definitions are rejected, or reported with
//! bounded output, in linear time. The bounds are generous (a debug build is allowed 15 s where a release
//! build must stay below 1 s) so that the tests cannot flake; a quadratic implementation misses them by
//! orders of magnitude.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use ew_ids::{Id, Uuid};
use ew_import_qet::QetElementImporter;
use ew_import_qet::decimal::Decimal;
use ew_import_qet::prims::shapes::polygon;
use ew_import_qet::prims::{Parsed, PrimCtx};
use ew_import_qet::xml::{LineIndex, prescan};
use ew_import_qet::{
    MAX_ENTRIES_PER_DEFINITION, MAX_UNSUPPORTED_STYLE_PAIRS_KEPT, MAX_UNSUPPORTED_STYLE_PAIRS_PER_DEFINITION,
};
use ew_interop_api::{Code, ImportBatch, ImportContext, ImportError, ImportInput, Importer, Limits};
use ew_schema::{LibDocument, Primitive};

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

/// N1: a finding after a 64 MiB comment of newlines gets its line from a sparse index, not one entry
/// per newline.
#[test]
fn finding_after_many_newlines_uses_a_small_line_index() {
    const NEWLINES: usize = 64 * 1024 * 1024;
    let mut text = String::with_capacity(NEWLINES + 128);
    text.push_str("<definition type=\"element\">");
    text.push_str("<!--");
    text.push_str(&"\n".repeat(NEWLINES));
    text.push_str("-->");
    text.push_str("<x/>");
    text.push_str("</definition>");
    let (result, elapsed) = timed_import(&text);
    eprintln!("finding_after_many_newlines_uses_a_small_line_index: {elapsed:?}");
    assert!(elapsed < budget(), "took {elapsed:?}");
    let report = result.unwrap().report;
    let entry = report
        .entries
        .iter()
        .find(|e| e.location.as_ref().is_some_and(|l| l.item == Some("x".into())))
        .expect("a finding located at <x>");
    assert_eq!(
        entry.location.as_ref().unwrap().line,
        Some(u32::try_from(NEWLINES).unwrap() + 1)
    );
    // The old index held one `usize` per newline (512 MiB here).
    let index = LineIndex::new(&text);
    assert!(index.heap_bytes() < 1 << 20, "{}", index.heap_bytes());
    assert!(index.checkpoint_count() <= text.len() / 65_536 + 1);
}

/// `count` root children `<x/>` with `attrs` attributes each, inside an element definition.
fn flood_text(count: usize, attrs: usize) -> String {
    let mut element = String::from("<x");
    for i in 0..attrs {
        write!(element, " a{i}=\"0\"").unwrap();
    }
    element.push_str("/>");
    let mut text = String::from("<definition type=\"element\">");
    for _ in 0..count {
        text.push_str(&element);
    }
    text.push_str("</definition>");
    text
}

/// N2: the sum of squared attribute counts is refused by the pre-scan, before the quadratic duplicate
/// check of the parser can run.
#[test]
fn attribute_work_flood_is_refused_before_parsing() {
    let text = flood_text(128, 8_100);
    let (result, elapsed) = timed_import(&text);
    eprintln!("attribute_work_flood_is_refused_before_parsing: {elapsed:?}");
    assert_eq!(
        result.err(),
        Some(ImportError::LimitExceeded {
            what: "XML attribute work".into(),
            limit: 67_108_864
        })
    );
    assert!(elapsed < budget(), "took {elapsed:?}");
    if !cfg!(debug_assertions) {
        assert!(elapsed < Duration::from_millis(200), "took {elapsed:?}");
    }
}

/// N2: a moderate attribute work (1 000 x 200^2 = 40 000 000, below 2^26) still imports in time.
#[test]
fn many_moderate_elements_import_within_budget() {
    let text = flood_text(1_000, 200);
    let (result, elapsed) = timed_import(&text);
    eprintln!("many_moderate_elements_import_within_budget: {elapsed:?}");
    assert!(elapsed < budget(), "took {elapsed:?}");
    let report = result.unwrap().report;
    assert!(report.entries.len() <= MAX_ENTRIES_PER_DEFINITION + 1);
}

/// `count` unsupported root children without whitespace between them (which would add text nodes).
fn node_flood_text(count: usize) -> String {
    let mut text = String::from("<definition type=\"element\">");
    text.push_str(&"<x/>".repeat(count));
    text.push_str("</definition>");
    text
}

/// N3: a node count just under the default limit imports, with a capped report.
#[test]
fn nodes_just_under_the_default_import_within_budget() {
    let text = node_flood_text(999_000);
    let (result, elapsed) = timed_import(&text);
    eprintln!("nodes_just_under_the_default_import_within_budget: {elapsed:?}");
    assert!(elapsed < budget(), "took {elapsed:?}");
    let report = result.unwrap().report;
    assert_eq!(report.entries.len(), MAX_ENTRIES_PER_DEFINITION + 1);
    assert_eq!(report.entries.last().unwrap().code, Code::LIMIT);
}

/// N3: more nodes than the default limit are refused by the pre-scan, not by the parser.
#[test]
fn node_flood_is_refused_by_the_prescan() {
    let text = node_flood_text(1_000_001);
    let expected = ImportError::LimitExceeded {
        what: "XML node count".into(),
        limit: 1_000_000,
    };
    assert_eq!(prescan(&text, &Limits::default()), Err(expected.clone()));
    let (result, elapsed) = timed_import(&text);
    eprintln!("node_flood_is_refused_by_the_prescan: {elapsed:?}");
    assert_eq!(result.err(), Some(expected));
    assert!(elapsed < budget(), "took {elapsed:?}");
}

/// N4: `roxmltree` reserves a node per `<` and an attribute per `=` of the whole input (about 72 bytes
/// each) before its node limit applies, so 16 million of them in a comment or a text would reserve 1.1 GB.
/// The pre-scan refuses such an input before the parser allocates anything.
#[test]
fn markup_character_flood_is_refused_before_parsing() {
    let expected = ImportError::LimitExceeded {
        what: "XML markup characters".into(),
        limit: 3_048_576,
    };
    let flood = 16_000_000;
    for (name, body) in [
        ("comment", format!("<!--{}-->", "<".repeat(flood))),
        ("text", "=".repeat(flood)),
    ] {
        let text = format!("<definition type=\"element\">{body}</definition>");
        let (result, elapsed) = timed_import(&text);
        eprintln!("markup_character_flood_is_refused_before_parsing ({name}): {elapsed:?}");
        assert_eq!(result.err(), Some(expected.clone()), "{name}");
        assert!(elapsed < budget(), "{name} took {elapsed:?}");
    }
}

/// N4: just under the bound, the input imports within the budget.
#[test]
fn markup_characters_under_the_bound_import_within_budget() {
    let text = format!(
        "<definition type=\"element\"><!--{}-->{}</definition>",
        "<".repeat(1_000_000),
        "=".repeat(2_000_000)
    );
    let (result, elapsed) = timed_import(&text);
    eprintln!("markup_characters_under_the_bound_import_within_budget: {elapsed:?}");
    assert!(elapsed < budget(), "took {elapsed:?}");
    assert!(result.is_ok(), "{result:?}");
}

/// The number of `qet.style.*` entries over all primitives of the imported symbol, counted in its debug
/// form (the keys are the only places that spelling occurs in these inputs).
fn style_entries(batch: &ImportBatch) -> usize {
    let symbols: Vec<_> = batch
        .lib
        .iter()
        .filter_map(|d| match d {
            LibDocument::Symbol(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(symbols.len(), 1);
    format!("{:?}", symbols[0].item).matches("qet.style.").count()
}

/// The shared checks of the two style-pair floods (N5).
fn check_style_flood(text: &str, name: &str) -> ImportBatch {
    let (result, elapsed) = timed_import(text);
    eprintln!("{name}: {elapsed:?}");
    assert!(elapsed < budget(), "{name} took {elapsed:?}");
    let batch = result.unwrap();
    let kept = style_entries(&batch);
    assert!(
        kept <= MAX_UNSUPPORTED_STYLE_PAIRS_PER_DEFINITION,
        "{name}: {kept} kept"
    );
    assert!(kept > 0, "{name}: nothing kept");
    assert!(batch.report.entries.len() <= MAX_ENTRIES_PER_DEFINITION + 2);
    batch
}

/// N5: one `<line>` whose `style` attribute holds 1 500 000 distinct unsupported pairs (about 14 MiB) keeps
/// at most 8 of them.
#[test]
fn one_huge_style_attribute_is_capped() {
    let mut style = String::with_capacity(15_000_000);
    for i in 0..1_500_000 {
        write!(style, "k{i}:v;").unwrap();
    }
    let text = format!(
        "<definition type=\"element\"><description><line x1=\"0\" y1=\"0\" x2=\"10\" y2=\"0\" style=\"{style}\"/></description></definition>"
    );
    assert!(text.len() > 13 << 20);
    let batch = check_style_flood(&text, "one_huge_style_attribute_is_capped");
    assert_eq!(style_entries(&batch), MAX_UNSUPPORTED_STYLE_PAIRS_KEPT);
}

/// N5: 200 000 lines of 3 unsupported pairs each keep at most 1 000 pairs in total.
#[test]
fn many_small_style_attributes_are_capped_per_definition() {
    let mut text = String::from("<definition type=\"element\"><description>\n");
    for i in 0..200_000 {
        writeln!(
            text,
            "<line x1=\"0\" y1=\"0\" x2=\"1\" y2=\"0\" style=\"a{i}:v;b:v;c:v\"/>"
        )
        .unwrap();
    }
    text.push_str("</description></definition>");
    let batch = check_style_flood(&text, "many_small_style_attributes_are_capped_per_definition");
    assert_eq!(style_entries(&batch), MAX_UNSUPPORTED_STYLE_PAIRS_PER_DEFINITION);
    let summary = batch
        .report
        .entries
        .iter()
        .find(|e| e.message.contains("unsupported style pairs dropped: at most"))
        .expect("the summary entry");
    assert!(
        summary
            .message
            .starts_with("599000 unsupported style pairs dropped"),
        "{}",
        summary.message
    );
}
