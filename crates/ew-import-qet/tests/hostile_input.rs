// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The doc 14 §11 hostile-input regressions: element definitions are rejected, or reported with
//! bounded output. Ordinary runs prove correctness and output limits only. Timing acquisition,
//! reporting and generous bounds are opt-in with `EW_PERF_ASSERT` (doc 18 §3.16); a debug build is
//! allowed 15 s where a release build must stay below 1 s.

// Opt-in wall-clock timers only; no timestamp reaches a compared result.
#![allow(clippy::disallowed_methods)]

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use ew_ids::{Id, Uuid};
use ew_import_qet::decimal::Decimal;
use ew_import_qet::prims::shapes::polygon;
use ew_import_qet::prims::{Parsed, PrimCtx};
use ew_import_qet::xml::{LineIndex, prescan};
use ew_import_qet::{ElementImport, ElementSource, QetElementImporter, read_element};
use ew_import_qet::{
    MAX_ENTRIES_PER_DEFINITION, MAX_UNSUPPORTED_STYLE_PAIRS_KEPT, MAX_UNSUPPORTED_STYLE_PAIRS_PER_DEFINITION,
};
use ew_interop_api::{
    Code, ImportBatch, ImportContext, ImportError, ImportInput, Importer, Limits, LineageInfo,
};
use ew_schema::Primitive;

/// The time a hostile input may take: 1 s in release (doc 14 §11), with slack for debug builds.
fn budget() -> Duration {
    if cfg!(debug_assertions) {
        Duration::from_secs(15)
    } else {
        Duration::from_secs(1)
    }
}

/// Starts timing only for an opt-in performance run.
fn start_timing() -> Option<Instant> {
    std::env::var_os("EW_PERF_ASSERT").is_some().then(Instant::now)
}

/// Timing acquisition, reporting and bounds are opt-in; ordinary runs prove correctness/output limits.
fn assert_within_budget(what: &str, elapsed: Option<Duration>) {
    assert_within(what, elapsed, budget());
}

/// As [`assert_within_budget`], against an explicit bound.
fn assert_within(what: &str, elapsed: Option<Duration>, bound: Duration) {
    if let Some(elapsed) = elapsed {
        assert!(elapsed < bound, "{what} took {elapsed:?}, bound {bound:?}");
    }
}

/// Imports `text` as `x.elmt` and returns the result with an optional elapsed time.
fn timed_import(text: &str) -> (Result<ImportBatch, ImportError>, Option<Duration>) {
    let input = ImportInput::new("x.elmt", text.as_bytes().to_vec()).unwrap();
    let mut ctx = ImportContext::new(Id::from_uuid(Uuid::from_u128(1)));
    let start = start_timing();
    let result = QetElementImporter.import(&input, &mut ctx);
    (result, start.map(|start| start.elapsed()))
}

/// Reads `text` as an element definition directly under `limits` (the importer always applies the
/// element-definition preset, so the project-file default is reachable only this way) and returns the
/// result with an optional elapsed time.
fn timed_read(text: &str, limits: &Limits) -> (Result<ElementImport, ImportError>, Option<Duration>) {
    let src = ElementSource::new("x.elmt", None).unwrap();
    let lineage = LineageInfo {
        id: Id::from_uuid(Uuid::from_u128(1)),
        namespace: Uuid::from_u128(2),
    };
    let start = start_timing();
    let result = read_element(text, &src, &lineage, Id::from_uuid(Uuid::from_u128(1)), limits);
    (result, start.map(|start| start.elapsed()))
}

/// The shared checks of the two finding-flood tests.
fn check_flood(text: &str, name: &str) {
    const CHILDREN: usize = 100_000;
    let (result, elapsed) = timed_import(text);
    if let Some(elapsed) = elapsed {
        eprintln!("{name}: {elapsed:?}");
    }
    assert_within_budget("{name} took {elapsed:?}", elapsed);
    let report = result.unwrap().report;
    assert_eq!(report.entries.len(), MAX_ENTRIES_PER_DEFINITION + 1);
    let last = report.entries.last().unwrap();
    assert_eq!(last.code, Code::LIMIT);
    // 100 000 unsupported elements and one black-box finding, of which 1 000 are listed.
    let suppressed = CHILDREN + 1 - MAX_ENTRIES_PER_DEFINITION;
    assert_eq!(suppressed, 99_001);
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

/// C1: 100 000 unsupported root children give a capped, linear report.
#[test]
fn many_root_findings_are_linear() {
    let mut text = String::from("<definition type=\"element\">\n");
    for _ in 0..100_000 {
        text.push_str("<x/>\n");
    }
    text.push_str("</definition>");
    check_flood(&text, "many_root_findings_are_linear");
}

/// C1: the same inside `<description>`.
#[test]
fn many_description_findings_are_linear() {
    let mut text = String::from("<definition type=\"element\"><description>\n");
    for _ in 0..100_000 {
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
    let start = start_timing();
    polygon(node, &mut ctx, &mut out).unwrap();
    let elapsed = start.map(|start| start.elapsed());
    if let Some(elapsed) = elapsed {
        eprintln!("long_polygon_is_linear: {elapsed:?}");
    }
    let bound = if cfg!(debug_assertions) {
        Duration::from_secs(5)
    } else {
        Duration::from_millis(500)
    };
    assert_within("polygon()", elapsed, bound);
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
    if let Some(elapsed) = elapsed {
        eprintln!("hostile_polygon_is_refused_before_parsing: {elapsed:?}");
    }
    assert_within_budget("took {elapsed:?}", elapsed);
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
    if let Some(elapsed) = elapsed {
        eprintln!("polygon_just_under_the_attribute_limit_imports_within_budget: {elapsed:?}");
    }
    assert_within_budget("took {elapsed:?}", elapsed);
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
    // 1,048,592 attributes: over the project-file default (the element preset refuses far earlier).
    let (result, elapsed) = timed_read(&text, &Limits::default());
    if let Some(elapsed) = elapsed {
        eprintln!("attribute_flood_across_elements_is_refused: {elapsed:?}");
    }
    assert_within_budget("took {elapsed:?}", elapsed);
    assert_eq!(
        result.err(),
        Some(ImportError::LimitExceeded {
            what: "XML attribute count".into(),
            limit: 1_048_576
        })
    );
}

/// N1: a finding after a 12 MiB comment of newlines gets its line from a sparse index, not one entry
/// per newline.
#[test]
fn finding_after_many_newlines_uses_a_small_line_index() {
    const NEWLINES: usize = 12 * 1024 * 1024;
    let mut text = String::with_capacity(NEWLINES + 128);
    text.push_str("<definition type=\"element\">");
    text.push_str("<!--");
    text.push_str(&"\n".repeat(NEWLINES));
    text.push_str("-->");
    text.push_str("<x/>");
    text.push_str("</definition>");
    let (result, elapsed) = timed_import(&text);
    if let Some(elapsed) = elapsed {
        eprintln!("finding_after_many_newlines_uses_a_small_line_index: {elapsed:?}");
    }
    assert_within_budget("took {elapsed:?}", elapsed);
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
    // The old index held one `usize` per newline (96 MiB here).
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
    // 1,036,800 attributes: under the default count, so the work budget is what refuses it (the element
    // preset would refuse the count first).
    let text = flood_text(128, 8_100);
    let (result, elapsed) = timed_read(&text, &Limits::default());
    if let Some(elapsed) = elapsed {
        eprintln!("attribute_work_flood_is_refused_before_parsing: {elapsed:?}");
    }
    assert_eq!(
        result.err(),
        Some(ImportError::LimitExceeded {
            what: "XML attribute work".into(),
            limit: 67_108_864
        })
    );
    assert_within_budget("took {elapsed:?}", elapsed);
    if !cfg!(debug_assertions) {
        assert_within(
            "attribute_work_flood_is_refused_before_parsing",
            elapsed,
            Duration::from_millis(200),
        );
    }
}

/// N2: a moderate attribute work (1 000 x 200^2 = 40 000 000, below 2^26) still imports in time under the
/// project-file default (200,000 attributes, over the element preset's count).
#[test]
fn many_moderate_elements_import_within_budget() {
    let text = flood_text(1_000, 200);
    let (result, elapsed) = timed_read(&text, &Limits::default());
    if let Some(elapsed) = elapsed {
        eprintln!("many_moderate_elements_import_within_budget: {elapsed:?}");
    }
    assert_within_budget("took {elapsed:?}", elapsed);
    assert!(result.unwrap().entries.len() <= MAX_ENTRIES_PER_DEFINITION + 1);
}

/// N2/N7: the same shape under the element preset (500 x 200^2 = 20 000 000, 100,000 attributes) imports
/// through the importer in time.
#[test]
fn moderate_elements_under_the_element_preset_import_within_budget() {
    let text = flood_text(500, 200);
    let (result, elapsed) = timed_import(&text);
    if let Some(elapsed) = elapsed {
        eprintln!("moderate_elements_under_the_element_preset_import_within_budget: {elapsed:?}");
    }
    assert_within_budget("took {elapsed:?}", elapsed);
    assert!(result.unwrap().report.entries.len() <= MAX_ENTRIES_PER_DEFINITION + 1);
}

/// `count` unsupported root children without whitespace between them (which would add text nodes).
fn node_flood_text(count: usize) -> String {
    let mut text = String::from("<definition type=\"element\">");
    text.push_str(&"<x/>".repeat(count));
    text.push_str("</definition>");
    text
}

/// N3: a node count just under the project-file default imports, with a capped report.
#[test]
fn nodes_just_under_the_default_import_within_budget() {
    let text = node_flood_text(999_000);
    let (result, elapsed) = timed_read(&text, &Limits::default());
    if let Some(elapsed) = elapsed {
        eprintln!("nodes_just_under_the_default_import_within_budget: {elapsed:?}");
    }
    assert_within_budget("took {elapsed:?}", elapsed);
    let entries = result.unwrap().entries;
    assert_eq!(entries.len(), MAX_ENTRIES_PER_DEFINITION + 1);
    assert_eq!(entries.last().unwrap().code, Code::LIMIT);
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
    let (result, elapsed) = timed_read(&text, &Limits::default());
    if let Some(elapsed) = elapsed {
        eprintln!("node_flood_is_refused_by_the_prescan: {elapsed:?}");
    }
    assert_eq!(result.err(), Some(expected));
    assert_within_budget("took {elapsed:?}", elapsed);
}

/// N7: a node count just under the element preset (262,144) imports through the importer, with a capped
/// report.
#[test]
fn nodes_just_under_the_element_preset_import_within_budget() {
    let text = node_flood_text(262_142);
    let (result, elapsed) = timed_import(&text);
    if let Some(elapsed) = elapsed {
        eprintln!("nodes_just_under_the_element_preset_import_within_budget: {elapsed:?}");
    }
    assert_within_budget("took {elapsed:?}", elapsed);
    let report = result.unwrap().report;
    assert_eq!(report.entries.len(), MAX_ENTRIES_PER_DEFINITION + 1);
    assert_eq!(report.entries.last().unwrap().code, Code::LIMIT);
}

/// N7: the element preset refuses 262,144 nodes, by the pre-scan.
#[test]
fn element_preset_refuses_node_flood() {
    let text = node_flood_text(262_144);
    let expected = ImportError::LimitExceeded {
        what: "XML node count".into(),
        limit: 262_144,
    };
    assert_eq!(
        prescan(&text, &Limits::element_definition()),
        Err(expected.clone())
    );
    let (result, elapsed) = timed_import(&text);
    if let Some(elapsed) = elapsed {
        eprintln!("element_preset_refuses_node_flood: {elapsed:?}");
    }
    assert_eq!(result.err(), Some(expected));
    assert_within_budget("took {elapsed:?}", elapsed);
}

/// N7: the worst case per byte, `<text>` primitives of three attributes each, just under the preset's
/// attribute cap (43,000 x 3 = 129,000 of 131,072), imports within the budget.
#[test]
fn worst_case_text_primitives_under_the_preset_import_within_budget() {
    let mut text = String::from("<definition type=\"element\"><description>");
    text.push_str(&"<text x=\"0\" y=\"0\" font=\"a,1,1,1,1,1\"/>".repeat(43_000));
    text.push_str("</description></definition>");
    let (result, elapsed) = timed_import(&text);
    if let Some(elapsed) = elapsed {
        eprintln!("worst_case_text_primitives_under_the_preset_import_within_budget: {elapsed:?}");
    }
    assert_within_budget("took {elapsed:?}", elapsed);
    assert!(result.is_ok(), "{result:?}");
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
    let preset = ImportError::LimitExceeded {
        what: "XML markup characters".into(),
        limit: 655_360,
    };
    let flood = 16_000_000;
    for (name, body) in [
        ("comment", format!("<!--{}-->", "<".repeat(flood))),
        ("text", "=".repeat(flood)),
    ] {
        let text = format!("<definition type=\"element\">{body}</definition>");
        // Through the importer, under the element preset: 2 x 262,144 + 131,072.
        let (result, elapsed) = timed_import(&text);
        if let Some(elapsed) = elapsed {
            eprintln!("markup_character_flood_is_refused_before_parsing ({name}): {elapsed:?}");
        }
        assert_eq!(result.err(), Some(preset.clone()), "{name}");
        assert_within_budget("{name} took {elapsed:?}", elapsed);
        // Under the project-file default: 2 x 1,000,000 + 1,048,576.
        let (result, elapsed) = timed_read(&text, &Limits::default());
        if let Some(elapsed) = elapsed {
            eprintln!("markup_character_flood_is_refused_before_parsing ({name}, default): {elapsed:?}");
        }
        assert_eq!(result.err(), Some(expected.clone()), "{name} default");
        assert_within_budget("{name} default took {elapsed:?}", elapsed);
    }
}

/// N4: just under the bound, the input imports within the budget.
#[test]
fn markup_characters_under_the_bound_import_within_budget() {
    // The `=` come in texts of 500,000, under the text-node limit, between comments.
    let texts = vec!["=".repeat(500_000); 4].join("<!---->");
    let text = format!(
        "<definition type=\"element\"><!--{}-->{texts}</definition>",
        "<".repeat(1_000_000)
    );
    // The default's bound is 2 x 1,000,000 + 1,048,576; the element preset's is 655,360.
    let (result, elapsed) = timed_read(&text, &Limits::default());
    if let Some(elapsed) = elapsed {
        eprintln!("markup_characters_under_the_bound_import_within_budget: {elapsed:?}");
    }
    assert_within_budget("took {elapsed:?}", elapsed);
    assert!(result.is_ok(), "{result:?}");
}

/// The number of `qet.style.*` entries over all primitives of the imported symbol, counted in its debug
/// form (the keys are the only places that spelling occurs in these inputs).
fn style_entries(el: &ElementImport) -> usize {
    format!("{:?}", el.symbol.item).matches("qet.style.").count()
}

/// The shared checks of the style-pair floods (N5), read under `limits`.
fn check_style_flood(text: &str, name: &str, limits: &Limits) -> ElementImport {
    let (result, elapsed) = timed_read(text, limits);
    if let Some(elapsed) = elapsed {
        eprintln!("{name}: {elapsed:?}");
    }
    assert_within_budget("{name} took {elapsed:?}", elapsed);
    let el = result.unwrap();
    let kept = style_entries(&el);
    assert!(
        kept <= MAX_UNSUPPORTED_STYLE_PAIRS_PER_DEFINITION,
        "{name}: {kept} kept"
    );
    assert!(kept > 0, "{name}: nothing kept");
    assert!(el.entries.len() <= MAX_ENTRIES_PER_DEFINITION + 2);
    el
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
    let el = check_style_flood(
        &text,
        "one_huge_style_attribute_is_capped",
        &Limits::element_definition(),
    );
    assert_eq!(style_entries(&el), MAX_UNSUPPORTED_STYLE_PAIRS_KEPT);
}

/// `lines` `<line>` primitives of 3 unsupported style pairs each.
fn style_lines_text(lines: usize) -> String {
    let mut text = String::from("<definition type=\"element\"><description>\n");
    for i in 0..lines {
        writeln!(
            text,
            "<line x1=\"0\" y1=\"0\" x2=\"1\" y2=\"0\" style=\"a{i}:v;b:v;c:v\"/>"
        )
        .unwrap();
    }
    text.push_str("</description></definition>");
    text
}

/// The shared assertions of the two per-definition style-cap tests: 1 000 kept, the rest summarised.
fn check_style_cap(el: &ElementImport, dropped: usize) {
    assert_eq!(style_entries(el), MAX_UNSUPPORTED_STYLE_PAIRS_PER_DEFINITION);
    let summary = el
        .entries
        .iter()
        .find(|e| e.message.contains("unsupported style pairs dropped: at most"))
        .expect("the summary entry");
    assert!(
        summary
            .message
            .starts_with(&format!("{dropped} unsupported style pairs dropped")),
        "{}",
        summary.message
    );
}

/// N5: 200 000 lines of 3 unsupported pairs each (1,000,000 attributes, over the element preset's count)
/// keep at most 1 000 pairs in total under the project-file default.
#[test]
fn many_small_style_attributes_are_capped_per_definition() {
    let text = style_lines_text(200_000);
    let el = check_style_flood(
        &text,
        "many_small_style_attributes_are_capped_per_definition",
        &Limits::default(),
    );
    check_style_cap(&el, 599_000);
}

/// N5/N7: the same under the element preset, 25 000 lines (125,000 attributes, 75,000 pairs).
#[test]
fn many_small_style_attributes_are_capped_per_definition_under_the_preset() {
    let text = style_lines_text(25_000);
    let el = check_style_flood(
        &text,
        "many_small_style_attributes_are_capped_per_definition_under_the_preset",
        &Limits::element_definition(),
    );
    check_style_cap(&el, 74_000);
}

/// A definition whose `section` element holds `n` bytes of plain text, built as one buffer.
fn text_run(section_open: &str, section_close: &str, n: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(n + 200);
    bytes.extend_from_slice(b"<definition type=\"element\">");
    bytes.extend_from_slice(section_open.as_bytes());
    bytes.resize(bytes.len() + n, b'a');
    bytes.extend_from_slice(section_close.as_bytes());
    bytes.extend_from_slice(b"</definition>");
    bytes
}

/// Imports `bytes` as `x.elmt` and returns the error, optionally reporting elapsed time.
fn timed_refusal(bytes: Vec<u8>, name: &str) -> ImportError {
    let input = ImportInput::new("x.elmt", bytes).unwrap();
    let mut ctx = ImportContext::new(Id::from_uuid(Uuid::from_u128(1)));
    let start = start_timing();
    let result = QetElementImporter.import(&input, &mut ctx);
    let elapsed = start.map(|start| start.elapsed());
    if let Some(elapsed) = elapsed {
        eprintln!("{name}: {elapsed:?}");
    }
    assert_within_budget("{name} took {elapsed:?}", elapsed);
    result.expect_err("the input is refused")
}

/// N6: 200 MiB of text in `<informations>` is refused by the 16 MiB element-definition input preset, not
/// read (it used to import in 1.2 s with 656 MiB extra).
#[test]
fn huge_text_run_is_refused_by_the_input_preset() {
    let bytes = text_run("<informations>", "</informations>", 200 * 1024 * 1024);
    let err = timed_refusal(bytes, "huge_text_run_is_refused_by_the_input_preset");
    assert_eq!(
        err,
        ImportError::LimitExceeded {
            what: "input size".into(),
            limit: 16 * 1024 * 1024
        }
    );
}

/// N6: 17 MiB of text is over the preset.
#[test]
fn seventeen_mib_is_refused_by_the_input_preset() {
    let bytes = text_run("<informations>", "</informations>", 17 * 1024 * 1024);
    let err = timed_refusal(bytes, "seventeen_mib_is_refused_by_the_input_preset");
    assert_eq!(
        err,
        ImportError::LimitExceeded {
            what: "input size".into(),
            limit: 16 * 1024 * 1024
        }
    );
}

/// N6: a 2 MiB `<name>` in a 3 MiB document is within the input preset and refused by the text-node limit.
#[test]
fn big_name_is_refused_by_the_text_node_limit() {
    let mut bytes = text_run("<names><name lang=\"en\">", "</name></names>", 2 * 1024 * 1024);
    // Pad to 3 MiB with one comment, which is not a text node.
    let pad = format!("<!--{}-->", "p".repeat(1024 * 1024));
    let at = bytes.len() - b"</definition>".len();
    bytes.splice(at..at, pad.bytes());
    assert!(bytes.len() >= 3 * 1024 * 1024);
    let err = timed_refusal(bytes, "big_name_is_refused_by_the_text_node_limit");
    assert_eq!(
        err,
        ImportError::LimitExceeded {
            what: "XML text node".into(),
            limit: 1_048_576
        }
    );
}
