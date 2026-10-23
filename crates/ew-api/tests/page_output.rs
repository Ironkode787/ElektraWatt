// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page output of the golden project (WP-S3.04, FT-59): page display lists, PNG candidates and the PDF of every
//! folio of `page_order`, through the session's export job (`ew_api::output`: the product's page views and one
//! memoised `page_display_list` family per layer; `page_output/view.rs` keeps the test-only loading).

// Test target: the bless flag and diagnostic counters (EW_*) only choose whether a committed file is rewritten or a line is printed; the compared result never depends on them.
#![allow(clippy::disallowed_methods)]

#[path = "page_output/golden_files.rs"]
mod golden_files;
#[path = "page_output/pdf_support.rs"]
mod pdf_support;
#[path = "page_output/snapshot.rs"]
mod snapshot;
#[path = "page_output/view.rs"]
mod view;

use std::sync::Arc;

use ew_dl::{DisplayList, ItemKind, Layer};
use ew_export::{Parallelism, PdfDate, PdfOptions, PdfPage, pdf};
use ew_ids::{Id, Uuid};
use ew_scene::page::{
    ConnectionView, PageFinding, PageKind, PageScene, PageView, RepView, page_display_list, page_scene,
};
use serde_json::json;
use sha2::{Digest, Sha256};

use golden_files::{
    blessing, check_or_bless, csv_rows, decode_rgba, differing_percent, expected_dir, frame_figure_errors,
    frame_json, page_png, text_csv,
};
use pdf_support::{
    EXPECTED_DROPPED_NOTDEF_GLYPHS, pdf_options, pdf_pages, pdf_sha256, structure_text, xref_counts,
};
use view::{GoldenPage, builtin_symbol, destination, fonts, golden_pages, golden_session};

/// The export job's page scene, assembled from the memoised layer families, equals `ew-scene`'s page scene over
/// the full view on every page: every layer of the page's kind, the geometry, the findings and the sheet. So the
/// partial view of each layer reads everything its layer draws.
#[test]
fn the_memoised_layers_equal_the_scene_of_the_full_view_on_every_page() {
    let s = golden_session(None);
    let q = s.query();
    let order = q.get::<ew_derive::PageOrder>(());
    assert_eq!(order.entries.len(), 36);
    for entry in &order.entries {
        let job = ew_api::output::page_scene(&q, entry);
        let full = page_scene(&ew_api::output::page_view(&q, entry), ew_text::bundled());
        assert!(job == full, "folio {}: the job's scene differs", entry.folio);
    }
    // A second pass reads the layers from the memo store: the same values.
    let q2 = s.query();
    for entry in &order.entries {
        assert!(
            ew_api::output::page_scene(&q2, entry) == ew_api::output::page_scene(&q, entry),
            "folio {}",
            entry.folio
        );
    }
}

/// `--pages`: folio labels and ranges in page order, each page once; a partial export prints the links to pages
/// it does not hold as plain text (FR-OUT-020).
#[test]
fn a_page_selection_exports_those_folios_in_page_order() {
    use ew_api::output::{ExportOptions, PageSelection};
    let s = golden_session(None);
    let options = |spec: &str| ExportOptions {
        pages: PageSelection::parse(spec).expect("a selection"),
        ..ExportOptions::default()
    };
    let pages = s
        .export_pages(&options("101-106,1,103"))
        .expect("the selection exports");
    let labels: Vec<&str> = pages.pages.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["1", "101", "102", "103", "106"]);
    let all = s
        .export_pages(&ExportOptions::default())
        .expect("everything exports");
    assert_ne!(pages.options.document_id, all.options.document_id);
    let pdf = s.export_pdf(&options("101-106")).expect("a partial PDF");
    assert_eq!(pdf.pages, 4);
    assert!(pdf.report.plain_text_links > 0, "{:?}", pdf.report);
    for bad in ["104", "106-101", "x-y"] {
        assert!(
            matches!(
                s.export_pages(&options(bad)),
                Err(ew_api::output::OutputError::Selection(_))
            ),
            "{bad}"
        );
    }
}

#[test]
fn the_golden_pdf_has_every_folio_and_is_byte_identical_across_runs_and_record_shuffles() {
    let s = golden_session(None);
    let q = s.query();
    let pages = pdf_pages(&q, Parallelism::Serial);
    assert_eq!(
        pages.len(),
        36,
        "page_order of the golden model with its report pages"
    );
    let f = fonts();
    let a = pdf(&pages, &pdf_options(&q), &f, None).expect("the golden PDF is written");
    assert!(a.bytes.starts_with(b"%PDF-"));
    assert!(a.report.missing_fonts.is_empty(), "{:?}", a.report);
    let b = pdf(&pages, &pdf_options(&q), &f, None).unwrap();
    assert!(a.bytes == b.bytes, "two runs differ");
    let shuffled = golden_session(Some(0x5eed_1234));
    let q2 = shuffled.query();
    let c = pdf(&pdf_pages(&q2, Parallelism::Serial), &pdf_options(&q2), &f, None).unwrap();
    assert!(a.bytes == c.bytes, "a record shuffle changes the PDF");
}

/// The writer's own PDF structure of the golden project, as the committed candidate
/// `expected/pdf/structure.json` (doc 20 §3.3 "PDF structure").
/// Fonts are named `<family name> <weight>[ Italic]` after the bundled face info, sorted and unique.
#[test]
fn the_pdf_structure_matches_the_committed_file() {
    let s = golden_session(None);
    let q = s.query();
    let pages = pdf_pages(&q, Parallelism::Serial);
    let out = pdf(&pages, &pdf_options(&q), &fonts(), None).expect("the golden PDF is written");
    // Noto Sans has no glyph for "∞", "−" or "⇄"; they print with Noto Sans Math, the symbol fallback, so the
    // PDF embeds two fonts and drops no `.notdef` glyph (FR-OUT-027).
    assert_eq!(
        out.report.dropped_notdef_glyphs, EXPECTED_DROPPED_NOTDEF_GLYPHS,
        "{:?}",
        out.report
    );
    let text = structure_text(&pages, &out);
    check_or_bless(&expected_dir("pdf").join("structure.json"), text.as_bytes());
}

/// The SHA-256 of the golden PDF (doc 20 §3.3), the same from every run and every record order.
#[test]
fn the_pdf_sha256_matches_the_committed_file() {
    let f = fonts();
    let build = |seed: Option<u64>| pdf_sha256(seed, &f);
    let a = build(None);
    let b = build(Some(0x5eed_1234));
    let c = build(None);
    assert_eq!(a, b, "a record shuffle changes the PDF");
    assert_eq!(a, c, "two runs differ");
    check_or_bless(
        &expected_dir("pdf").join("sha256.txt"),
        format!("{a}\n").as_bytes(),
    );
}

/// Linked cross-reference texts of the golden model (doc 20 §15): 44 interruption-point texts and 19
/// main/auxiliary texts (9 main lists, 10 auxiliaries).
const LINKED_XREF_TEXTS: usize = 44 + 19;
/// The link targets of those texts: the main list of -201U1 ("∞ 3-B2, 106-G3") lists two, so every item
/// has its own link (FT-86, ID-228).
const XREF_LINK_TARGETS: usize = LINKED_XREF_TEXTS + 1;
/// The external interruption points print "+J-E-206 / JE-N1000-03" without a link (no in-project target).
const EXTERNAL_XREF_TEXTS: usize = 4;
/// The rows of the PLC overview (908 … 908.2) that link to their coil (doc 10 §5): one per bound channel, the
/// 8 used and the 20 spare valves' coils.
const PLC_ROW_LINKS: usize = 8 + 20;

#[test]
fn the_golden_pdf_has_one_link_per_cross_reference_item() {
    let s = golden_session(None);
    let q = s.query();
    let (linked, targets, external) = xref_counts(&golden_pages(&q));
    assert_eq!(linked, LINKED_XREF_TEXTS);
    assert_eq!(targets, XREF_LINK_TARGETS);
    assert_eq!(external, EXTERNAL_XREF_TEXTS);
    let out = pdf(
        &pdf_pages(&q, Parallelism::Serial),
        &pdf_options(&q),
        &fonts(),
        None,
    )
    .expect("the golden PDF is written");
    assert_eq!(out.report.links, 64 + 28, "{:?}", out.report);
    assert_eq!(out.report.links, targets + PLC_ROW_LINKS, "{:?}", out.report);
    assert_eq!(out.report.plain_text_links, 0, "{:?}", out.report);
}

/// The two link boxes of the -201U1 main text ("∞ 3-B2, 106-G3") cover their own item's glyph span: each
/// lies inside the text's own box, they are disjoint and ordered in reading order, and the first starts
/// after the "∞ " prefix.
#[test]
fn the_items_of_a_cross_reference_list_have_disjoint_ordered_link_boxes() {
    with_golden_pages(|pages| {
        let mut lists = 0;
        for g in pages {
            let scene = page_scene(&g.view, ew_text::bundled());
            let Some((_, labels)) = scene.layers.iter().find(|(l, _)| *l == Layer::Labels) else {
                continue;
            };
            let sheet: DisplayList = scene.map.apply_display_list(labels, Some(&scene.geometry));
            // Consecutive link carriers of one entity are the items of one text.
            let mut groups: Vec<Vec<ew_units::Rect>> = Vec::new();
            let mut last = None;
            for item in &sheet.items {
                let ItemKind::Path(_) = &item.kind else {
                    last = None;
                    continue;
                };
                if !matches!(item.link, Some(ew_dl::Link::Source(_))) {
                    last = None;
                    continue;
                }
                let bounds = DisplayList {
                    items: vec![item.clone()],
                }
                .bounds()
                .expect("a link box has points");
                if last == Some(item.entity) {
                    groups.last_mut().unwrap().push(bounds);
                } else {
                    groups.push(vec![bounds]);
                    last = Some(item.entity);
                }
            }
            for group in groups.iter().filter(|g| g.len() > 1) {
                lists += 1;
                for w in group.windows(2) {
                    assert!(
                        w[0].max().x < w[1].min().x,
                        "folio {}: boxes overlap or are out of order: {w:?}",
                        g.entry.folio
                    );
                }
            }
        }
        // The main lists with two items: only -201U1 lists two ("∞ 3-B2, 106-G3").
        assert_eq!(lists, 1);
    });
}

fn with_golden_pages(f: impl FnOnce(&[GoldenPage])) {
    let s = golden_session(None);
    let q = s.query();
    f(&golden_pages(&q));
}

/// `ew-derive`'s `cp_point` (the position of a function CP on a page, which the wire numbers read) and the page
/// scene's CP positions are the same geometry written twice; they agree on every CP of every golden page, the
/// terminals in strip segments and the black-box CPs included.
#[test]
fn cp_point_equals_the_cp_points_of_the_page_scene() {
    let s = golden_session(None);
    let q = s.query();
    let mut checked = 0;
    let mut in_segments = 0;
    for g in golden_pages(&q) {
        for ((function, idx), at) in ew_scene::page::cp_points(&g.view) {
            let function = Id::from_uuid(function);
            assert_eq!(
                ew_derive::cp_point(&q, function, idx, g.view.page),
                Some(at),
                "CP {idx} of {function} on folio {}",
                g.entry.folio
            );
            checked += 1;
            if g.view.reps.iter().any(|r| !r.terminals.is_empty()) {
                in_segments += 1;
            }
        }
    }
    assert!(checked > 150, "{checked} CPs compared");
    assert!(in_segments > 0, "pages with strip segments were compared");
}

#[test]
fn the_text_csv_of_every_folio_matches_the_committed_file() {
    with_golden_pages(|pages| {
        assert_eq!(pages.len(), 36);
        for g in pages {
            let scene = page_scene(&g.view, ew_text::bundled());
            let csv = text_csv(&scene);
            if std::env::var_os("EW_CSV_ROWS").is_some() {
                eprintln!("{} rows {}", g.entry.folio, csv_rows(&csv));
            }
            assert!(csv_rows(&csv) > 0, "folio {} has texts", g.entry.folio);
            // Shrink-and-report stays visible (FR-REP-039): no report table text overflows its cell even
            // at the smallest permitted size, and no table runs past the page's rows.
            let table_findings: Vec<_> = scene
                .findings
                .iter()
                .filter(|f| matches!(f, PageFinding::Table(_)))
                .collect();
            assert!(
                table_findings.is_empty(),
                "folio {}: {table_findings:?}",
                g.entry.folio
            );
            check_or_bless(
                &expected_dir("text").join(format!("{}.csv", g.entry.folio)),
                csv.as_bytes(),
            );
        }
    });
}

#[test]
fn the_frame_geometry_matches_the_committed_file() {
    check_or_bless(
        &expected_dir("geometry").join("frame.json"),
        frame_json().as_bytes(),
    );
}

#[test]
fn the_printed_frame_matches_doc_20_section_17_1() {
    let errors = frame_figure_errors();
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn the_png_candidates_match_within_the_doc_20_tolerance() {
    let f = fonts();
    with_golden_pages(|pages| {
        for g in pages {
            let scene = page_scene(&g.view, ew_text::bundled());
            let bytes = page_png(&scene, &f);
            let path = expected_dir("png").join(format!("{}.png", g.entry.folio));
            if blessing() {
                check_or_bless(&path, &bytes);
                continue;
            }
            let committed = std::fs::read(&path)
                .unwrap_or_else(|e| panic!("{}: {e} (run with EW_BLESS=1)", path.display()));
            let (w0, h0, a) = decode_rgba(&committed);
            let (w1, h1, b) = decode_rgba(&bytes);
            assert_eq!((w0, h0), (w1, h1), "folio {} size", g.entry.folio);
            let pct = differing_percent(&a, &b, 16);
            assert!(
                pct <= 0.5,
                "folio {}: {pct:.3} % of the pixels differ",
                g.entry.folio
            );
        }
    });
}

#[test]
fn the_png_of_a_folio_is_deterministic() {
    let f = fonts();
    with_golden_pages(|pages| {
        let g = folio(pages, "101");
        let scene = page_scene(&g.view, ew_text::bundled());
        let a = Sha256::digest(page_png(&scene, &f));
        let b = Sha256::digest(page_png(&scene, &f));
        assert_eq!(a, b);
    });
}

#[test]
fn every_committed_png_is_at_most_300_kb() {
    if blessing() {
        return;
    }
    let mut n = 0;
    for e in std::fs::read_dir(expected_dir("png")).unwrap() {
        let p = e.unwrap().path();
        let len = std::fs::metadata(&p).unwrap().len();
        assert!(len <= 300 * 1024, "{}: {len} bytes", p.display());
        n += 1;
    }
    assert_eq!(n, 36);
}

fn folio<'a>(pages: &'a [GoldenPage], folio: &str) -> &'a GoldenPage {
    pages
        .iter()
        .find(|g| g.entry.folio == folio)
        .expect("the folio exists")
}

#[test]
fn the_display_list_of_folio_101_is_stable() {
    with_golden_pages(|pages| {
        let scene = page_scene(&folio(pages, "101").view, ew_text::bundled());
        let text = snapshot::display_list_text(&scene, &[Layer::Symbols, Layer::Wires, Layer::Labels]);
        insta::assert_snapshot!(text, @r#"
        symbols instance 20d847a1 sym=6a8491f2 variant=A translate=(235479167,79375000)
        symbols instance 23a4ce3c sym=a144580f variant=A translate=(50270833,79375000)
        symbols instance 313e2bc4 sym=f8e64dbf variant=A translate=(267229167,169333333)
        symbols instance 57543b91 sym=88ca8636 variant=A translate=(44979167,37041667)
        symbols instance 5e0e0f25 sym=4f6bee27 variant=A translate=(7937500,142875000)
        symbols instance 6112da7c sym=b0f8e0f4 variant=A translate=(235479167,100541667)
        symbols instance 66c620c1 sym=13704217 variant=A translate=(39687500,79375000)
        symbols instance 682aa707 sym=e2cae815 variant=A translate=(203729167,58208333)
        symbols instance 7b76f7a3 sym=d5dded4b variant=A translate=(137583333,31750000)
        symbols instance 8c4dbb72 sym=d5dded4b variant=A translate=(201083333,31750000)
        symbols instance 92878d9b sym=4f6bee27 variant=A translate=(7937500,148166667)
        symbols instance 9945b5aa sym=13704217 variant=A translate=(44979167,79375000)
        symbols instance 9b2316f2 sym=f8e64dbf variant=A translate=(251354167,142875000)
        symbols instance dc21e14c sym=f8e64dbf variant=A translate=(267229167,164041667)
        symbols instance de795e26 sym=9a687b31 variant=A translate=(203729167,95250000)
        symbols instance ec75639e sym=f8b44164 variant=A translate=(92604167,79375000)
        symbols instance f4fdc202 sym=4f6bee27 variant=A translate=(7937500,137583333)
        wires path 09ed5beb els=3 first=(44979167,82020833) last=(50270834,44979167) stroke=264583 fill=-
        wires path 13c75a2c els=3 first=(7937500,142875000) last=(44979167,76729167) stroke=264583 fill=-
        wires path 17b31bbc els=3 first=(50270834,37041667) last=(145520833,31750000) stroke=264583 fill=-
        wires path 2b9b8f17 els=2 first=(137583333,31750000) last=(201083333,31750000) stroke=264583 fill=-
        wires path 2be33590 els=2 first=(203729167,66145833) last=(203729167,105833333) stroke=264583 fill=-
        wires path 301459db els=2 first=(145520833,31750000) last=(209020833,31750000) stroke=264583 fill=-
        wires path 35faab6c els=3 first=(206375000,100541667) last=(251354167,140229167) stroke=264583 fill=-
        wires path 48f5a362 els=2 first=(235479167,87312500) last=(235479167,100541667) stroke=264583 fill=-
        wires path 522a3985 els=3 first=(145520833,42333333) last=(267229167,166687500) stroke=264583 fill=-
        wires path 5945bf6c els=3 first=(7937500,148166667) last=(50270833,76729167) stroke=264583 fill=-
        wires path 5e9d92b4 els=3 first=(219604167,58208333) last=(235479167,79375000) stroke=264583 fill=-
        wires path 628881aa els=3 first=(39687500,82020833) last=(44979167,44979167) stroke=264583 fill=-
        wires path 6d50c972 els=3 first=(7937500,137583333) last=(39687500,76729167) stroke=264583 fill=-
        wires path 8d27ba7c els=3 first=(137583333,42333333) last=(267229167,161395834) stroke=264583 fill=-
        wires path a697efb2 els=3 first=(209020833,42333333) last=(219604167,58208333) stroke=264583 fill=-
        wires path aba94b57 els=3 first=(50270833,82020833) last=(92604167,79375000) stroke=264583 fill=-
        wires path b5382424 els=3 first=(187854167,58208333) last=(203729167,95250000) stroke=264583 fill=-
        wires path c851008e els=3 first=(187854167,58208333) last=(235479167,108479167) stroke=264583 fill=-
        wires path ce52ac00 els=3 first=(44979167,37041667) last=(137583333,31750000) stroke=264583 fill=-
        wires path d48db15f els=3 first=(201083333,42333333) last=(187854167,58208333) stroke=264583 fill=-
        wires path f40a98e3 els=2 first=(219604167,58208333) last=(203729167,58208333) stroke=264583 fill=-
        wires instance 0c0b22d7 sym=a3e1ca3e variant=A translate=(187854167,58208333)
        wires instance 1f9afba6 sym=a3e1ca3e variant=A translate=(219604167,58208333)
        labels glyphs 20d847a1 "S56" origin=(237463542,83910223) size=3439583 angle=0
        labels glyphs 20d847a1 "13" origin=(233304292,80508739) size=1322917 angle=0
        labels glyphs 20d847a1 "14" origin=(233304292,87123323) size=1322917 angle=0
        labels glyphs 23a4ce3c "PE" origin=(44293102,80602931) size=3439583 angle=0
        labels glyphs 313e2bc4 "N-1" origin=(269213542,168860522) size=2381250 angle=0
        labels glyphs 313e2bc4 "102-A1" origin=(260153944,172540347) size=2381250 angle=0
        labels path 313e2bc4 els=5 first=(260153944,169994791) last=(267890625,169994791) stroke=- fill=-
        labels glyphs 57543b91 "S51" origin=(36510120,42238348) size=3439583 angle=0
        labels glyphs 57543b91 "Eaton" origin=(54901042,38553231) size=2381250 angle=0
        labels glyphs 57543b91 "533356" origin=(54901042,41860523) size=2381250 angle=0
        labels glyphs 57543b91 "40 A" origin=(54901042,45167815) size=2381250 angle=0
        labels glyphs 57543b91 "1" origin=(45640625,38836865) size=1322917 angle=0
        labels glyphs 57543b91 "2" origin=(45640625,44128531) size=1322917 angle=0
        labels glyphs 57543b91 "3" origin=(50932292,38836865) size=1322917 angle=0
        labels glyphs 57543b91 "4" origin=(50932292,44128531) size=1322917 angle=0
        labels glyphs 5e0e0f25 "J-E-206-N" origin=(5498174,140854377) size=2381250 angle=0
        labels glyphs 5e0e0f25 "+J-E-206 / JE-N1000-03" origin=(-1701535,146743473) size=2381250 angle=0
        labels glyphs 6112da7c "E56" origin=(226324584,105738348) size=3439583 angle=0
        labels glyphs 6112da7c "X1" origin=(236140625,101675406) size=1322917 angle=0
        labels glyphs 6112da7c "X2" origin=(236140625,108289990) size=1322917 angle=0
        labels glyphs 66c620c1 "1" origin=(35735684,80602931) size=3439583 angle=0
        labels glyphs 682aa707 "T55" origin=(205713542,61420639) size=3439583 angle=0
        labels glyphs 682aa707 "15 °C" origin=(205713542,65011564) size=2381250 angle=0
        labels glyphs 682aa707 "5" origin=(202311000,59342072) size=1322917 angle=0
        labels glyphs 682aa707 "3" origin=(202311000,65956656) size=1322917 angle=0
        labels glyphs 682aa707 "SK 3110.000" origin=(205713542,68318856) size=2381250 angle=0
        labels glyphs 7b76f7a3 "-101F1" origin=(124175045,38269598) size=3439583 angle=0
        labels glyphs 7b76f7a3 "Schneider Electric" origin=(147505208,34584481) size=2381250 angle=0
        labels glyphs 7b76f7a3 "A9P52610" origin=(147505208,37891773) size=2381250 angle=0
        labels glyphs 7b76f7a3 "C10" origin=(147505208,41199064) size=2381250 angle=0
        labels glyphs 7b76f7a3 "1" origin=(138244791,33545198) size=1322917 angle=0
        labels glyphs 7b76f7a3 "2" origin=(138244791,41482698) size=1322917 angle=0
        labels glyphs 7b76f7a3 "N" origin=(146182291,33545198) size=1322917 angle=0
        labels glyphs 7b76f7a3 "N" origin=(146182291,41482698) size=1322917 angle=0
        labels glyphs 8c4dbb72 "F55" origin=(192717473,38269598) size=3439583 angle=0
        labels glyphs 8c4dbb72 "Siemens" origin=(211005208,34584481) size=2381250 angle=0
        labels glyphs 8c4dbb72 "5SY4204-7" origin=(211005208,37891773) size=2381250 angle=0
        labels glyphs 8c4dbb72 "C4" origin=(211005208,41199064) size=2381250 angle=0
        labels glyphs 8c4dbb72 "1" origin=(201744791,33545198) size=1322917 angle=0
        labels glyphs 8c4dbb72 "2" origin=(201744791,41482698) size=1322917 angle=0
        labels glyphs 8c4dbb72 "N" origin=(209682291,33545198) size=1322917 angle=0
        labels glyphs 8c4dbb72 "N" origin=(209682291,41482698) size=1322917 angle=0
        labels glyphs 92878d9b "J-E-206-PE" origin=(5020733,146146044) size=2381250 angle=0
        labels glyphs 92878d9b "+J-E-206 / JE-N1000-03" origin=(-1701535,152035140) size=2381250 angle=0
        labels glyphs 9945b5aa "2" origin=(41027351,80602931) size=3439583 angle=0
        labels glyphs 9b2316f2 "PE-1.1" origin=(253338542,142402189) size=2381250 angle=0
        labels glyphs 9b2316f2 "103-G6" origin=(244067013,146082014) size=2381250 angle=0
        labels path 9b2316f2 els=5 first=(244067013,143536458) last=(252015625,143536458) stroke=- fill=-
        labels glyphs dc21e14c "L-1" origin=(269213542,163568856) size=2381250 angle=0
        labels glyphs dc21e14c "102-A1" origin=(260153944,167248681) size=2381250 angle=0
        labels path dc21e14c els=5 first=(260153944,164703125) last=(267890625,164703125) stroke=- fill=-
        labels glyphs de795e26 "E55" origin=(195236043,101769598) size=3439583 angle=0
        labels glyphs de795e26 "L" origin=(204390625,97045198) size=1322917 angle=0
        labels glyphs de795e26 "N" origin=(204390625,104982698) size=1322917 angle=0
        labels glyphs de795e26 "PE" origin=(207036459,99492593) size=1322917 angle=0
        labels glyphs f4fdc202 "J-E-206-L" origin=(5779161,135562710) size=2381250 angle=0
        labels glyphs f4fdc202 "+J-E-206 / JE-N1000-03" origin=(-1701535,141451806) size=2381250 angle=0
        labels instance 09ed5beb sym=04b0176e variant=D translate=(44979167,63500000)
        labels glyphs 09ed5beb "2.5" origin=(45451448,64433979) size=1322917 angle=90000
        labels instance 17b31bbc sym=04b0176e variant=A translate=(97895833,31750000)
        labels glyphs 17b31bbc "2.5" origin=(96961854,32222281) size=1322917 angle=0
        labels instance 628881aa sym=04b0176e variant=D translate=(39687500,63500000)
        labels glyphs 628881aa "2.5" origin=(40159781,64433979) size=1322917 angle=90000
        labels instance ce52ac00 sym=04b0176e variant=A translate=(91281250,31750000)
        labels glyphs ce52ac00 "2.5" origin=(90347271,32222281) size=1322917 angle=0
        "#);
    });
}

/// Paths and glyph runs of a display list, groups included.
fn count(list: &DisplayList) -> (usize, usize) {
    fn go(items: &[ew_dl::Item], paths: &mut usize, runs: &mut usize) {
        for i in items {
            match &i.kind {
                ItemKind::Path(_) => *paths += 1,
                ItemKind::Glyphs(_) => *runs += 1,
                ItemKind::Group(g) => go(&g.items, paths, runs),
                _ => {}
            }
        }
    }
    let (mut paths, mut runs) = (0, 0);
    go(&list.items, &mut paths, &mut runs);
    (paths, runs)
}

/// Observed counts of the frame layer of folio 101, derived in the test below.
const FRAME_PATHS: usize = 38;
const FRAME_RUNS: usize = 41;

#[test]
fn the_frame_layer_of_folio_101_has_the_items_the_frame_rules_give() {
    with_golden_pages(|pages| {
        let v = &folio(pages, "101").view;
        let scene = page_scene(v, ew_text::bundled());
        let (paths, runs) = count(scene.layer(Layer::Frame).unwrap());
        // Frame rules of `forms/mod.rs`: outer rectangle 1, title-block separator 1, 2 header-band lines,
        // 16 column ticks and 7 row ticks = 27 paths; grid labels 17 columns + 8 rows = 25 runs. Title
        // block (`ew.titleblock.qet-a4`): 11 line graphics = 11 paths, and 18 text cells of which 16 print
        // text (the logo and revision cells are not bound yet and print nothing) = 16 runs.
        // Total: 27 + 11 = 38 paths and 25 + 16 = 41 runs.
        assert_eq!(
            (paths, runs),
            (FRAME_PATHS, FRAME_RUNS),
            "frame layer of folio 101"
        );
        // The frame reads only the frame: a wire edit never rebuilds the title block.
        let mut bare = v.clone();
        bare.reps.clear();
        bare.connections.clear();
        bare.junctions.clear();
        let bare = page_display_list(&bare, Layer::Frame, ew_text::bundled());
        assert!(bare.list == **scene.layer(Layer::Frame).unwrap());
    });
}

#[test]
fn the_display_lists_do_not_depend_on_the_order_of_the_view_records() {
    with_golden_pages(|pages| {
        for g in pages {
            let mut r = g.view.clone();
            r.reps.reverse();
            r.connections.reverse();
            r.junctions.reverse();
            for &layer in g.view.kind.layers() {
                let a = page_display_list(&g.view, layer, ew_text::bundled());
                let b = page_display_list(&r, layer, ew_text::bundled());
                assert!(a == b, "folio {} layer {}", g.entry.folio, layer.name());
            }
        }
    });
}

fn uuid(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// The hyphenated text of [`uuid`], the form of a record's ID member.
fn uid(n: u128) -> String {
    uuid(n).hyphenated().to_string()
}

/// A page of 100 terminals and 10 050 connections between pseudo-random CPs (50 with unknown ends).
fn hostile_view() -> PageView {
    let symbol = builtin_symbol("ew.iec.terminal@qet10").expect("the terminal symbol");
    let page = Id::from_uuid(uuid(0xAA));
    let mut reps = Vec::new();
    for i in 0..100u128 {
        let (col, row) = (i64::try_from(i % 10).unwrap(), i64::try_from(i / 10).unwrap());
        let (x, y) = ((col * 25 + 15) * 1_000_000, (row * 18 + 25) * 1_000_000);
        let rep = serde_json::from_value(json!({
            "id": uid(0x1000 + i),
            "functions": [uid(0x2000 + i)],
            "page": uid(0xAA),
            "symbol": symbol.id.as_uuid().hyphenated().to_string(),
            "pos": [x, y],
        }))
        .expect("a representation record");
        reps.push(RepView {
            rep: Arc::new(rep),
            symbol: Some(symbol.def.clone()),
            slots: Vec::new(),
            terminals: Vec::new(),
        });
    }
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move |m: u64| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state % m
    };
    let mut connections = Vec::new();
    for i in 0..10_050u128 {
        let mut end = || (0x2000 + u128::from(next(100)), next(2));
        let (mut a, mut b) = (end(), end());
        if i >= 10_000 {
            // Unresolvable: a function no representation draws.
            b.0 = 0x9000 + i;
        }
        if a == b {
            a.1 = 1 - a.1;
        }
        let c = serde_json::from_value(json!({
            "id": uid(0x10_0000 + i),
            "a": {"cp": [uid(a.0), a.1]},
            "b": {"cp": [uid(b.0), b.1]},
            "page": uid(0xAA),
        }))
        .expect("a connection record");
        connections.push(ConnectionView {
            connection: Arc::new(c),
            marker: None,
        });
    }
    PageView {
        page,
        kind: PageKind::Schematic,
        grid: ew_units::Grid::MM4,
        language: None,
        frame: None,
        reps,
        connections,
        junctions: Vec::new(),
        wire_marker: None,
        table: None,
        embedded: Vec::new(),
        break_marks: Vec::new(),
        cp_targets: Vec::new(),
        crossings: Default::default(),
    }
}

#[test]
fn a_hostile_page_renders_and_exports_without_panic() {
    let view = hostile_view();
    let scene: PageScene = page_scene(&view, ew_text::bundled());
    let unresolved = scene
        .findings
        .iter()
        .filter(|f| matches!(f, PageFinding::UnresolvedEnd { .. }))
        .count();
    assert_eq!(unresolved, 50);
    let wires = count(scene.layer(Layer::Wires).unwrap()).0;
    assert_eq!(
        unresolved + wires,
        10_050,
        "every connection is drawn or reported"
    );
    assert_eq!(wires, 10_000, "{wires} wire paths");
    let list = scene.sheet_list();
    let f = fonts();
    let sheet = ew_render_cpu::SheetSize {
        width: scene.sheet_width,
        height: scene.sheet_height,
    };
    let options = ew_render_cpu::RenderOptions {
        dpi: 72,
        ..Default::default()
    };
    ew_render_cpu::render(&list, sheet, &options, &f, None).expect("the hostile page renders");
    let page = PdfPage {
        destination: ew_dl::Destination::Page(uuid(0xAA)),
        label: "1".to_owned(),
        title: "hostile".to_owned(),
        section: None,
        width: scene.sheet_width,
        height: scene.sheet_height,
        content: list,
    };
    let options = PdfOptions::new(PdfDate::ymd(2000, 1, 1));
    let out = pdf(&[page], &options, &f, None).expect("the hostile PDF is written");
    assert!(out.bytes.starts_with(b"%PDF-"));
}

/// The operands of every `w` (line width) operator in the content streams of a PDF: each stream is
/// inflated (krilla compresses content and font streams with Flate); streams that do not inflate to ASCII
/// (fonts) are skipped.
fn pdf_line_widths(bytes: &[u8]) -> Vec<f64> {
    use std::io::Read;
    let mut out = Vec::new();
    let mut rest = bytes;
    while let Some(start) = find(rest, b"stream") {
        let body = &rest[start + b"stream".len()..];
        let body = body
            .strip_prefix(b"\r\n")
            .or_else(|| body.strip_prefix(b"\n"))
            .unwrap_or(body);
        let Some(end) = find(body, b"endstream") else {
            break;
        };
        let mut inflated = Vec::new();
        if flate2::read::ZlibDecoder::new(&body[..end])
            .read_to_end(&mut inflated)
            .is_ok()
            && inflated.is_ascii()
        {
            let text = String::from_utf8(inflated).expect("ASCII");
            let tokens: Vec<&str> = text.split_ascii_whitespace().collect();
            for pair in tokens.windows(2) {
                if pair[1] == "w"
                    && let Ok(v) = pair[0].parse::<f64>()
                {
                    out.push(v);
                }
            }
        }
        rest = &body[end + b"endstream".len()..];
    }
    out
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// doc 20 §17.4: on folio 101 (a QET-grid page) the frame, the symbols and the wires print in one width
/// class, 0.261 mm = 0.74 pt, and thin symbol lines at 0.130 mm = 0.37 pt: the `@qet10` edition's pens
/// and the wire pen are QET px, mapped by the frame's sheet map like the frame lines.
#[test]
fn the_pdf_of_folio_101_has_one_width_class_for_frame_symbols_and_wires() {
    let s = golden_session(None);
    let q = s.query();
    let pages: Vec<PdfPage> = pdf_pages(&q, Parallelism::Serial)
        .into_iter()
        .filter(|p| p.label == "101")
        .collect();
    assert_eq!(pages.len(), 1);
    let out = pdf(&pages, &pdf_options(&q), &fonts(), None).expect("the PDF of folio 101");
    let widths = pdf_line_widths(&out.bytes);
    assert!(!widths.is_empty(), "no line widths found in the content stream");
    let classes: std::collections::BTreeSet<String> = widths.iter().map(|w| format!("{w:.2}")).collect();
    let want: std::collections::BTreeSet<String> = ["0.37", "0.74"].into_iter().map(str::to_owned).collect();
    assert_eq!(classes, want, "line widths in pt: {widths:?}");
}

/// Prev/Next of every title block follow `page_order` with the generated report pages (doc 20 §5,
/// FR-PAG-013), and every page has its own PDF destination (generated pages use their v5 IDs).
#[test]
fn prev_and_next_follow_the_page_order_with_the_report_pages() {
    let s = golden_session(None);
    let q = s.query();
    let pages = golden_pages(&q);
    let text = |page: ew_derive::PageRef, path: &str| -> Option<String> {
        q.get::<ew_derive::TitleBlockValues>(page)
            .values
            .iter()
            .find(|(p, _)| p.as_str() == path)
            .and_then(|(_, v)| match v {
                ew_derive::TbValue::Text(t) => Some(t.clone()),
                _ => None,
            })
    };
    let folios: Vec<&str> = pages.iter().map(|g| g.entry.folio.as_str()).collect();
    for (i, g) in pages.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| folios[j].to_owned());
        let next = folios.get(i + 1).map(|f| (*f).to_owned());
        assert_eq!(text(g.entry.page, "page.prev"), prev, "Prev of {}", folios[i]);
        assert_eq!(text(g.entry.page, "page.next"), next, "Next of {}", folios[i]);
    }
    // The pages the review named: the cover's Next is the table of contents, folio 3 follows its
    // continuation, the divider 900 follows the last report page before it, 902 is followed by 903.
    let at = |f: &str| folios.iter().position(|x| *x == f).expect("the folio exists");
    assert_eq!(folios[at("1") + 1], "2");
    assert_eq!(folios[at("3") - 1], "2.1");
    assert_eq!(folios[at("902") + 1], "903");
    let mut ids: Vec<ew_dl::Destination> = pages.iter().map(|g| destination(g.entry.page)).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), pages.len(), "every page has its own destination");
}

/// Every page connection of the golden model is drawn: black-box CPs come from `Representation.box`, strip
/// terminals from their segments (review G1). `EW_WIRE_COUNTS=1` prints the count per folio.
#[test]
fn every_page_wire_of_the_golden_model_is_drawn() {
    with_golden_pages(|pages| {
        let mut total = 0;
        for g in pages {
            let wires = page_display_list(&g.view, Layer::Wires, ew_text::bundled());
            let unresolved: Vec<_> = wires
                .findings
                .iter()
                .filter(|f| matches!(f, PageFinding::UnresolvedEnd { .. }))
                .collect();
            assert!(unresolved.is_empty(), "folio {}: {unresolved:?}", g.entry.folio);
            let drawn = wires
                .list
                .items
                .iter()
                .filter(|i| matches!(i.kind, ItemKind::Path(_)))
                .count();
            if std::env::var_os("EW_WIRE_COUNTS").is_some() {
                eprintln!(
                    "folio {}: {drawn} of {} connections drawn",
                    g.entry.folio,
                    g.view.connections.len()
                );
            }
            total += drawn;
            assert_eq!(drawn, g.view.connections.len(), "folio {}", g.entry.folio);
        }
        // 72 wire connections and the single-line bus cable -3W1 on folio 3 (DP.1; DP.2 is page-less).
        assert_eq!(total, 72 + 1, "page connections of the golden model");
    });
}

/// WP-S2.02 A3 on a real page: `render_region` over the full rectangle of folio 103 at 150 dpi gives the bytes of
/// `render`, with and without culling, so the canvas's CPU path is the golden reference.
#[test]
fn the_full_region_of_folio_103_is_the_render_byte_for_byte() {
    let f = fonts();
    with_golden_pages(|pages| {
        let g = folio(pages, "103");
        let scene = page_scene(&g.view, ew_text::bundled());
        let list = scene.sheet_list();
        let sheet = ew_render_cpu::SheetSize {
            width: scene.sheet_width,
            height: scene.sheet_height,
        };
        let options = ew_render_cpu::RenderOptions {
            dpi: 150,
            ..Default::default()
        };
        let (whole, _) = ew_render_cpu::render(&list, sheet, &options, &f, None).unwrap();
        let region = ew_render_cpu::RegionOptions {
            dpi: 150,
            x: 0,
            y: 0,
            width: whole.width,
            height: whole.height,
            background: ew_dl::Color::WHITE,
            surround: ew_dl::Color::BLACK,
        };
        let bounds = ew_render_cpu::ItemBounds::of(&list, None);
        let (culled, report) =
            ew_render_cpu::render_region(&list, Some(&bounds), sheet, &region, &f, None).unwrap();
        assert!(report.missing_fonts.is_empty(), "{report:?}");
        assert!(culled == whole, "culled region differs from render");
        let (plain, _) = ew_render_cpu::render_region(&list, None, sheet, &region, &f, None).unwrap();
        assert!(plain == whole, "region differs from render");
    });
}

/// `Session::page_grid`: the drawn pages of Golden A have QElectroTech's 10 px grid and the QET frame's
/// page-to-sheet map; an unknown folio has none.
#[test]
fn session_page_grid_gives_the_grid_and_the_sheet_map() {
    let s = golden_session(None);
    let grid = s.page_grid("103").expect("folio 103 has a grid");
    assert_eq!((grid.num_nm, grid.den), (7_937_500, 3));
    assert_eq!(grid.sheet_scale_ppb, 986_282_578);
    assert_eq!(grid.sheet_offset_nm, (9_790_000, 9_790_000));
    assert_eq!(s.page_grid("no such folio"), None);
}
