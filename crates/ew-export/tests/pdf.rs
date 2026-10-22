// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! PDF structure and determinism of the writer (doc 09 §4.2, §4.3; AC-13 and AC-14 in miniature).
//!
//! No PDF parser is in the dependency graph, so the structure checks read the object dictionaries
//! textually: krilla compresses content and font streams only, never dictionaries.

// EW_PDF_DUMP names a file that the test writes the PDF to for inspection; it never changes the compared bytes.
#![allow(clippy::disallowed_methods)]

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_dl::{Brush, Color, Destination, DisplayList, FontKey, Item, Link, Path, Pen, Stroke};
use ew_export::ew_render_cpu::FontBytes;
use ew_export::{
    Conformance, OutlineEntry, PdfDate, PdfError, PdfOptions, PdfPage, outline_entries, pdf, pdf_bytes,
};
use ew_text::{builtin, bundled};
use ew_units::{Nm, Pt2, Rect};
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn fonts() -> BTreeMap<FontKey, FontBytes> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut map = BTreeMap::new();
    for name in ["NotoSans-Regular.ttf", "NotoSans-Bold.ttf"] {
        let bytes = std::fs::read(root.join(name)).unwrap();
        let sha256: [u8; 32] = Sha256::digest(&bytes).into();
        let key = FontKey {
            sha256,
            face_index: 0,
        };
        map.insert(
            key,
            FontBytes {
                data: Arc::from(bytes),
                index: 0,
            },
        );
    }
    map
}

fn mm(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm::from_mm(x), Nm::from_mm(y))
}

fn text(s: &str, at: Pt2) -> Item {
    let style = builtin().resolve("text.normal").unwrap();
    let mut run = (*bundled().shape(s, &style, None).run).clone();
    run.origin = at;
    Item::glyphs(Arc::new(run))
}

fn page(n: u128, label: &str, section: Option<&str>, items: Vec<Item>) -> PdfPage {
    PdfPage {
        destination: Destination::Page(Uuid::from_u128(n)),
        label: label.to_owned(),
        title: format!("{label} Page {label}"),
        section: section.map(str::to_owned),
        width: Nm::from_mm(297),
        height: Nm::from_mm(210),
        content: DisplayList { items },
    }
}

fn rep(n: u128) -> Destination {
    Destination::Representation(Uuid::from_u128(n))
}

/// Three pages: front matter "1"; section pages "101" (an xref text to a target on "103", one to a
/// representation not in the export) and "103" (the target, a frame, a dashed and a grey-filled shape).
fn pages() -> Vec<PdfPage> {
    let frame = Item::path(
        Path::rect(Rect::from_points(mm(10, 10), mm(287, 200))),
        Some(Stroke::pen(Pen::P026)),
        None,
    );
    let target = Item::path(
        Path::circle(mm(100, 80), Nm::from_mm(2)),
        None,
        Some(Brush::BLACK),
    )
    .with_link(Link::Target(rep(7)));
    let xref = text("103-G6", mm(50, 50)).with_link(Link::Source(rep(7)));
    let outside = text("206-A1", mm(50, 60)).with_link(Link::Source(rep(99)));
    vec![
        page(1, "1", None, vec![frame.clone(), text("Title Page", mm(20, 20))]),
        page(
            101,
            "101",
            Some("Section 1 – Wiring Definition"),
            vec![frame.clone(), xref, outside, text("SK 3110.000", mm(30, 100))],
        ),
        page(
            103,
            "103",
            Some("Section 1 – Wiring Definition"),
            vec![
                frame,
                target,
                Item::path(
                    Path::line(mm(20, 150), mm(200, 150)),
                    Some(Stroke::pen(Pen::P026).dashed(ew_dl::DashPattern::Dash42)),
                    None,
                ),
                Item::path(
                    Path::line(mm(20, 170), mm(200, 170)),
                    Some(Stroke::pen(Pen::P026).with_brush(Brush::Solid(Color::from_rgb8(200, 0, 0)))),
                    None,
                ),
                Item::path(
                    Path::rect(Rect::from_points(mm(30, 160), mm(60, 180))),
                    None,
                    Some(Brush::Solid(Color::from_rgb8(192, 192, 192))),
                ),
            ],
        ),
    ]
}

fn options() -> PdfOptions {
    PdfOptions {
        title: Some("Test project".into()),
        description: Some("drawing <drawing id>, revision V2.0".into()),
        keywords: vec!["ElektraWatt".into()],
        language: Some("en".into()),
        creator: Some("ElektraWatt 0.0.0".into()),
        document_id: "test-export-input".into(),
        ..PdfOptions::new(PdfDate::ymd(2026, 3, 14))
    }
}

fn count(hay: &[u8], needle: &str) -> usize {
    hay.windows(needle.len())
        .filter(|w| *w == needle.as_bytes())
        .count()
}

fn contains(hay: &[u8], needle: &str) -> bool {
    count(hay, needle) > 0
}

#[test]
fn structure_pages_labels_outline_links_destinations_fonts() {
    let out = pdf(&pages(), &options(), &fonts(), None).unwrap();
    let b = &out.bytes;
    if let Some(path) = std::env::var_os("EW_PDF_DUMP") {
        std::fs::write(path, b).unwrap();
    }
    assert!(b.starts_with(b"%PDF-"), "header");
    assert!(b.ends_with(b"%%EOF\n") || b.ends_with(b"%%EOF"), "trailer");
    assert!(contains(b, "startxref") && contains(b, "/Root"));
    assert_eq!(count(b, "/Type/Page/"), 3, "pages");
    assert!(contains(b, "/Type/Pages/Count 3"));
    // /PageLabels with the folios as literal prefixes, one range per page.
    assert!(contains(b, "/PageLabels"));
    assert!(contains(b, "/Nums[0 "));
    for label in ["(1)", "(101)", "(103)"] {
        assert!(
            contains(b, &format!("/Type/PageLabel/P{label}")),
            "page label {label}"
        );
    }
    assert_eq!(count(b, "/Type/PageLabel/"), 3);
    // Outline: front matter page, the section, its two pages.
    assert!(contains(b, "/Outlines"));
    assert!(contains(b, "(1 Page 1)"));
    assert!(contains(b, "(101 Page 101)") && contains(b, "(103 Page 103)"));
    // One link annotation (the xref into the export); the other xref is plain text.
    assert_eq!(out.report.links, 1);
    assert_eq!(out.report.plain_text_links, 1);
    assert_eq!(count(b, "/Subtype/Link"), 1);
    assert!(contains(b, &format!("/Dest({})", rep(7).pdf_name())));
    // Named destinations: the three pages and the linked target, nothing for the missing one.
    assert_eq!(out.report.destinations, 4);
    assert!(contains(b, &format!("({})", rep(7).pdf_name())));
    assert!(!contains(b, &rep(99).pdf_name()));
    for n in [1u128, 101, 103] {
        assert!(contains(b, &Destination::Page(Uuid::from_u128(n)).pdf_name()));
    }
    // Subset-embedded TrueType font with a ToUnicode map, PDF/A-2b identification, fixed dates.
    assert!(contains(b, "/FontFile2"));
    assert!(contains(b, "/ToUnicode"));
    assert!(contains(b, "+NotoSans"), "subset tag prefix");
    assert!(contains(b, "pdfaid:part"));
    assert!(contains(b, "D:20260314000000"));
    assert!(contains(b, "/OutputIntents"));
    assert!(out.report.missing_fonts.is_empty());
}

/// The `/Rect[...]` of every link annotation, in file order.
fn link_rects(b: &[u8]) -> Vec<[f64; 4]> {
    let text = String::from_utf8_lossy(b);
    text.match_indices("/Rect[")
        .map(|(i, m)| {
            let rest = &text[i + m.len()..];
            let end = rest.find(']').unwrap();
            let v: Vec<f64> = rest[..end]
                .split_whitespace()
                .map(|n| n.parse().unwrap())
                .collect();
            [v[0], v[1], v[2], v[3]]
        })
        .collect()
}

#[test]
fn every_linked_item_gets_its_own_annotation_over_its_own_box() {
    // The two items of one cross-reference text (FT-86): invisible boxes side by side, each with its own
    // target on page 103, drawn after the text they cover.
    let box_of = |x0: i64, x1: i64, to: u128| {
        Item::path(
            Path::polygon(&[mm(x0, 48), mm(x0, 52), mm(x1, 52), mm(x1, 48)]),
            None,
            None,
        )
        .with_link(Link::Source(rep(to)))
    };
    let target = |n: u128, x: i64| {
        Item::path(Path::circle(mm(x, 80), Nm::from_mm(2)), None, Some(Brush::BLACK))
            .with_link(Link::Target(rep(n)))
    };
    let pages = vec![
        page(
            101,
            "101",
            None,
            vec![
                text("3-B2, 106-G3", mm(50, 50)),
                box_of(50, 58, 7),
                box_of(62, 75, 8),
            ],
        ),
        page(103, "103", None, vec![target(7, 100), target(8, 120)]),
    ];
    let out = pdf(&pages, &options(), &fonts(), None).unwrap();
    assert_eq!(out.report.links, 2);
    assert_eq!(out.report.plain_text_links, 0);
    assert_eq!(count(&out.bytes, "/Subtype/Link"), 2);
    let rects = link_rects(&out.bytes);
    assert_eq!(rects.len(), 2, "{rects:?}");
    assert!(rects[0][2] < rects[1][0], "disjoint and ordered: {rects:?}");
}

#[test]
fn byte_identical_across_runs_and_independent_of_font_map_order() {
    let a = pdf_bytes(&pages(), &options(), &fonts()).unwrap();
    let b = pdf_bytes(&pages(), &options(), &fonts()).unwrap();
    assert_eq!(Sha256::digest(&a), Sha256::digest(&b));
    // A different date or document ID changes the bytes (they are inputs, not noise).
    let mut later = options();
    later.date = PdfDate::ymd(2026, 3, 15);
    assert_ne!(pdf_bytes(&pages(), &later, &fonts()).unwrap(), a);
}

#[test]
fn monochrome_and_plain_conformance() {
    let mut mono = options();
    mono.monochrome = true;
    let m = pdf_bytes(&pages(), &mono, &fonts()).unwrap();
    assert_ne!(m, pdf_bytes(&pages(), &options(), &fonts()).unwrap());
    let mut plain = options();
    plain.conformance = Conformance::None;
    let p = pdf_bytes(&pages(), &plain, &fonts()).unwrap();
    assert!(!contains(&p, "pdfaid:part"));
}

#[test]
fn errors_and_missing_fonts() {
    assert_eq!(pdf_bytes(&[], &options(), &fonts()), Err(PdfError::NoPages));
    let mut bad = pages();
    bad[1].width = Nm::ZERO;
    assert_eq!(
        pdf_bytes(&bad, &options(), &fonts()),
        Err(PdfError::PageSize { index: 1 })
    );
    // Two pages with one destination (2 and 2.1 sharing a report block's ID) are refused.
    let mut dup = pages();
    dup[2].destination = dup[1].destination;
    assert_eq!(
        pdf_bytes(&dup, &options(), &fonts()),
        Err(PdfError::DuplicateDestination { index: 2 })
    );
    let mut d = options();
    d.date.month = 13;
    assert_eq!(pdf_bytes(&pages(), &d, &fonts()), Err(PdfError::Date));
    // Without fonts the runs are skipped and reported; the PDF is still written.
    let none: BTreeMap<FontKey, FontBytes> = BTreeMap::new();
    let out = pdf(&pages(), &options(), &none, None).unwrap();
    assert_eq!(out.report.missing_fonts.len(), 1);
}

#[test]
fn hostile_page_with_ten_thousand_paths() {
    let mut items = Vec::new();
    let mut s: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s % 280_000_000) as i64
    };
    for _ in 0..10_000 {
        let a = Pt2::new(Nm(next()), Nm(next() % 200_000_000));
        let b = Pt2::new(Nm(next()), Nm(next() % 200_000_000));
        items.push(Item::path(Path::line(a, b), Some(Stroke::pen(Pen::P026)), None));
    }
    let p = vec![page(5, "5", None, items)];
    let bytes = pdf_bytes(&p, &options(), &fonts()).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
}

fn date_result(y: u16, m: u8, d: u8) -> Result<Vec<u8>, PdfError> {
    let options = PdfOptions::new(PdfDate::ymd(y, m, d));
    pdf_bytes(&pages(), &options, &fonts())
}

#[test]
fn calendar_dates_are_validated() {
    for (y, m, d) in [
        (2024, 2, 29),
        (2000, 2, 29),
        (2024, 12, 31),
        (2023, 2, 28),
        (1, 1, 1),
    ] {
        assert!(date_result(y, m, d).is_ok(), "{y}-{m}-{d} is valid");
    }
    for (y, m, d) in [
        (2023, 2, 29),
        (1900, 2, 29),
        (2024, 4, 31),
        (0, 1, 1),
        (2024, 13, 1),
        (2024, 0, 1),
        (2024, 1, 0),
        (2024, 1, 32),
    ] {
        assert_eq!(
            date_result(y, m, d),
            Err(PdfError::Date),
            "{y}-{m}-{d} is invalid"
        );
    }
    let mut late = PdfDate::ymd(2024, 1, 1);
    late.hour = 24;
    assert_eq!(
        pdf_bytes(&pages(), &PdfOptions::new(late), &fonts()),
        Err(PdfError::Date)
    );
}

#[test]
fn outline_entries_group_sections_and_keep_front_matter_top_level() {
    let p = |n: u128, section: Option<&str>| page(n, &n.to_string(), section, Vec::new());
    let pages = [
        p(1, None),
        p(101, Some("A")),
        p(102, Some("A")),
        p(2, None),
        p(201, Some("B")),
        p(103, Some("A")),
    ];
    let leaf = |title: &str, page: usize| OutlineEntry {
        title: title.to_owned(),
        page,
        children: Vec::new(),
    };
    let node = |title: &str, page: usize, children: Vec<OutlineEntry>| OutlineEntry {
        title: title.to_owned(),
        page,
        children,
    };
    assert_eq!(
        outline_entries(&pages),
        vec![
            leaf("1 Page 1", 0),
            node("A", 1, vec![leaf("101 Page 101", 1), leaf("102 Page 102", 2)]),
            leaf("2 Page 2", 3),
            node("B", 4, vec![leaf("201 Page 201", 4)]),
            node("A", 5, vec![leaf("103 Page 103", 5)]),
        ]
    );
    assert!(outline_entries(&[]).is_empty());
}

#[test]
fn the_report_lists_the_embedded_fonts_sorted_and_unique() {
    let out = pdf(&pages(), &options(), &fonts(), None).unwrap();
    let fonts = fonts();
    assert_eq!(out.report.fonts.len(), 1, "only the regular face is drawn");
    assert!(fonts.contains_key(&out.report.fonts[0]));
    // Two runs of the same font and a missing one: unique, and the missing font is not listed.
    let ghost = FontKey {
        sha256: [3; 32],
        face_index: 0,
    };
    let mut run = (*bundled()
        .shape("X", &builtin().resolve("text.normal").unwrap(), None)
        .run)
        .clone();
    run.origin = mm(10, 10);
    let mut bad = run.clone();
    bad.font = ghost;
    let items = vec![
        Item::glyphs(Arc::new(run.clone())),
        Item::glyphs(Arc::new(run)),
        Item::glyphs(Arc::new(bad)),
    ];
    let out = pdf(&[page(1, "1", None, items)], &options(), &fonts, None).unwrap();
    assert_eq!(out.report.fonts.len(), 1);
    assert_eq!(out.report.missing_fonts, vec![ghost]);
    assert!(out.report.fonts.windows(2).all(|w| w[0] < w[1]));
}
