// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The written PDF of the sample project, read back with `page_output/pdf_read.rs`: the frame lines of a drawn page
//! against its frame form's geometry (AC-01), the document's structure (AC-13), the text layer of a page against the
//! glyph runs it prints (AC-06, AC-13) and the same bytes for a shuffled record order and a parallel export
//! (AC-14).

use std::collections::BTreeMap;

use ew_dl::{Destination, Item, ItemKind, Link};
use ew_export::{Parallelism, PdfOutput, PdfPage, pdf, svg};
use ew_scene::forms::{FrameGeometry, SheetMap};
use ew_units::{Nm, Pt2};

use crate::pdf_read::{PageRead, PdfFile, Segment};
use crate::pdf_support::{
    EXPECTED_DROPPED_NOTDEF_GLYPHS, pdf_options, pdf_pages, report_row_link_count, structure_text,
    svg_options,
};
use crate::view::{fonts, golden_pages, sample_session};

/// The tolerance of a written line against the form's geometry (doc 18 §3.9 "Frame geometry"), mm.
const TOLERANCE_MM: f64 = 0.2;

/// The seed of the record shuffle.
const SHUFFLE_SEED: u64 = 0x5eed_ac14;

fn mm(n: Nm) -> f64 {
    n.0 as f64 / 1e6
}

/// The pages of `pdf` as the in-repo reader reads them back.
fn read_back(out: &PdfOutput) -> Vec<PageRead> {
    PdfFile::parse(&out.bytes)
        .and_then(|f| f.pages())
        .expect("the written PDF reads back")
}

/// The PDF of the sample's page with `folio` alone.
fn page_alone(folio: &str) -> (PdfPage, PdfOutput) {
    let s = sample_session(None);
    let q = s.query();
    let page = pdf_pages(&q, Parallelism::Serial)
        .into_iter()
        .find(|p| p.label == folio)
        .expect("the folio is in the page order");
    let out = pdf(std::slice::from_ref(&page), &pdf_options(&q), &fonts(), None).expect("the PDF is written");
    (page, out)
}

/// The glyph runs a display list prints, in drawing order.
fn printed_runs(items: &[Item], out: &mut Vec<String>) {
    for item in items {
        match &item.kind {
            ItemKind::Glyphs(run) => out.push(run.text.clone()),
            ItemKind::Group(g) => printed_runs(&g.items, out),
            _ => {}
        }
    }
}

/// The link sources a display list draws.
fn link_sources(items: &[Item], out: &mut Vec<Destination>) {
    for item in items {
        if let Some(Link::Source(d)) = item.link {
            out.push(d);
        }
        if let ItemKind::Group(g) = &item.kind {
            link_sources(&g.items, out);
        }
    }
}

/// A line from one sheet point to another, mm.
type Line = ((f64, f64), (f64, f64));

/// The larger endpoint distance between a line and a segment, in the better of the two directions.
fn deviation(l: &Line, s: &Segment) -> f64 {
    let d = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1);
    d(l.0, s.a).max(d(l.1, s.b)).min(d(l.0, s.b).max(d(l.1, s.a)))
}

/// AC-01: folio 101 of the written PDF strokes every line of its frame form within 0.2 mm at the frame's line width
/// (the outer rectangle, the title-block separator, the two header-band lines and the inner column and row ticks),
/// its media box is the form's sheet, and no other line lies in the header bands. The title block's inner lines are
/// not compared: the form's geometry gives its box only.
#[test]
fn the_frame_lines_of_a_written_page_lie_on_the_form_geometry() {
    let s = sample_session(None);
    let q = s.query();
    let frame = golden_pages(&q)
        .into_iter()
        .find(|g| g.entry.folio == "101")
        .and_then(|g| g.view.frame)
        .expect("folio 101 has a frame form");
    let g = FrameGeometry::of(&frame.form, frame.title_block.as_deref()).expect("the form has a grid");
    assert!(g.show_grid, "the drawn pages' form shows its grid");
    let map = SheetMap::of(&frame.form);
    let at = |x: Nm, y: Nm| {
        let p = map.apply(Pt2::new(x, y));
        (mm(p.x), mm(p.y))
    };
    let ((x0, y0), (x1, y1)) = (
        at(g.outer.min().x, g.outer.min().y),
        at(g.outer.max().x, g.outer.max().y),
    );
    let ((dx0, dy0), (dx1, dy1)) = (
        at(g.drawing_area.min().x, g.drawing_area.min().y),
        at(g.drawing_area.max().x, g.drawing_area.max().y),
    );
    let tb = g.title_block_origin.expect("the form places a title block");
    let ty = at(tb.x, tb.y).1;
    let mut lines: Vec<Line> = vec![
        ((x0, y0), (x1, y0)),
        ((x1, y0), (x1, y1)),
        ((x1, y1), (x0, y1)),
        ((x0, y1), (x0, y0)),
        ((x0, ty), (x1, ty)),
        ((x0, dy0), (dx1, dy0)),
        ((dx0, y0), (dx0, dy1)),
    ];
    let inner = |edges: &[Nm]| edges[1..edges.len() - 1].to_vec();
    lines.extend(inner(&g.column_edges).into_iter().map(|x| {
        let x = at(x, Nm::ZERO).0;
        ((x, y0), (x, dy0))
    }));
    lines.extend(inner(&g.row_edges).into_iter().map(|y| {
        let y = at(Nm::ZERO, y).1;
        ((x0, y), (dx0, y))
    }));
    let width = mm(map.scale_len(g.line_width));

    let (_, out) = page_alone("101");
    let page = &read_back(&out)[0];
    assert!(
        (page.width_mm - mm(frame.form.sheet.width)).abs() <= TOLERANCE_MM
            && (page.height_mm - mm(frame.form.sheet.height)).abs() <= TOLERANCE_MM,
        "media box {} x {} mm",
        page.width_mm,
        page.height_mm
    );
    for l in &lines {
        let (d, s) = page
            .segments
            .iter()
            .map(|s| (deviation(l, s), s))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .expect("the page strokes lines");
        assert!(d <= TOLERANCE_MM, "the frame line {l:?} is {d:.3} mm off");
        assert!(
            (s.width - width).abs() <= 0.01,
            "the frame line {l:?} is {} mm wide, the frame {width} mm",
            s.width
        );
    }
    // The reverse direction: a stroked segment in the header bands is a frame line. A segment along the drawing area's
    // or the title block's edge is not judged.
    let inside = |(a0, b0, a1, b1): (f64, f64, f64, f64), p: (f64, f64)| {
        p.0 > a0 + TOLERANCE_MM
            && p.0 < a1 - TOLERANCE_MM
            && p.1 > b0 + TOLERANCE_MM
            && p.1 < b1 - TOLERANCE_MM
    };
    let near = |(a0, b0, a1, b1): (f64, f64, f64, f64), p: (f64, f64)| {
        p.0 >= a0 - TOLERANCE_MM
            && p.0 <= a1 + TOLERANCE_MM
            && p.1 >= b0 - TOLERANCE_MM
            && p.1 <= b1 + TOLERANCE_MM
    };
    let (outer, area, title) = ((x0, y0, x1, y1), (dx0, dy0, dx1, dy1), (x0, ty, x1, y1));
    let in_band = |p| near(outer, p) && !inside(area, p) && !inside(title, p);
    for s in &page.segments {
        let mid = ((s.a.0 + s.b.0) / 2.0, (s.a.1 + s.b.1) / 2.0);
        if in_band(s.a) && in_band(s.b) && in_band(mid) && !near(area, mid) && !near(title, mid) {
            assert!(
                lines.iter().any(|l| deviation(l, s) <= TOLERANCE_MM),
                "an extra line from {:?} to {:?} mm in the header bands",
                s.a,
                s.b
            );
        }
    }
}

/// AC-13: the PDF of the whole sample has its folio labels as page labels in page order; its outline holds every page
/// once in page order, a page of a section under the section's entry and a front-matter page at the top; every link
/// the pages draw is one link annotation that reaches its target (a cross-reference item, a linked row of the PLC
/// overview, a report row), none prints as plain text; and every font it needs is a bundled face embedded in it,
/// none dropped and no Tahoma.
#[test]
fn the_pdf_has_page_labels_an_outline_by_section_links_and_embedded_fonts() {
    let s = sample_session(None);
    let q = s.query();
    let pages = pdf_pages(&q, Parallelism::Serial);
    let out = pdf(&pages, &pdf_options(&q), &fonts(), None).expect("the PDF is written");

    let order: Vec<String> = q
        .get::<ew_derive::PageOrder>(())
        .entries
        .iter()
        .map(|e| e.folio.clone())
        .collect();
    let labels: Vec<String> = pages.iter().map(|p| p.label.clone()).collect();
    assert_eq!(labels, order, "page labels are the folio labels of page_order");

    let mut seen = Vec::new();
    for e in ew_export::outline_entries(&pages) {
        if e.children.is_empty() && pages[e.page].section.is_none() {
            assert_eq!(
                e.title, pages[e.page].title,
                "a front-matter page is a top-level entry"
            );
            seen.push(e.page);
            continue;
        }
        assert!(
            !e.children.is_empty(),
            "{}: a section entry holds its pages",
            e.title
        );
        for c in &e.children {
            assert_eq!(
                pages[c.page].section.as_deref(),
                Some(e.title.as_str()),
                "{}",
                c.title
            );
            assert_eq!(c.title, pages[c.page].title);
            seen.push(c.page);
        }
    }
    assert_eq!(
        seen,
        (0..pages.len()).collect::<Vec<_>>(),
        "every page once, in page order"
    );
    assert!(pages.iter().any(|p| p.section.is_some()) && pages.iter().any(|p| p.section.is_none()));

    let mut drawn = Vec::new();
    for p in &pages {
        link_sources(&p.content.items, &mut drawn);
    }
    assert_eq!(
        out.report.links,
        drawn.len(),
        "one link annotation per drawn link"
    );
    assert_eq!(out.report.plain_text_links, 0, "{:?}", out.report);
    // What the links are: one per printed cross-reference item of the drawn pages (a slot the representation hides
    // prints none), one per linked row of the PLC overview (to the channel's connection point) and one per report
    // row link.
    let views = golden_pages(&q);
    let xref_items: usize = views
        .iter()
        .flat_map(|g| &g.view.reps)
        .map(|r| {
            let Some(def) = &r.symbol else { return 0 };
            r.slots
                .iter()
                .filter(|v| {
                    let default = def.text_slots.get(usize::from(v.slot)).is_some_and(|t| t.visible);
                    r.rep
                        .text_slots
                        .get(&v.slot)
                        .and_then(|o| o.visible)
                        .unwrap_or(default)
                })
                .map(|v| v.item_links.len() + usize::from(v.link.is_some()))
                .sum::<usize>()
        })
        .sum();
    let plc_rows = {
        let block = q
            .report_blocks()
            .iter()
            .find(|(_, b)| b.first_folio == Some(908))
            .map(|(id, _)| id)
            .expect("the PLC overview");
        let set = q.get::<ew_reports::BlockRows>(block);
        set.rows.iter().filter(|r| !r.links.is_empty()).count()
    };
    let to_cp = drawn.iter().filter(|d| matches!(d, Destination::Cp(_))).count();
    assert!(
        plc_rows > 0 && to_cp == plc_rows,
        "{to_cp} links to a connection point, {plc_rows} linked PLC rows"
    );
    let rows = report_row_link_count(&views);
    assert!(xref_items > 0 && rows > 0);
    assert_eq!(
        drawn.len(),
        xref_items + plc_rows + rows,
        "{xref_items} cross-reference items, {plc_rows} PLC rows, {rows} report rows"
    );

    assert!(
        out.report.missing_fonts.is_empty(),
        "{:?}",
        out.report.missing_fonts
    );
    assert!(!out.report.fonts.is_empty());
    assert_eq!(out.report.dropped_notdef_glyphs, EXPECTED_DROPPED_NOTDEF_GLYPHS);
    assert!(
        !structure_text(&pages, &out).contains("Tahoma"),
        "a Tahoma font is embedded"
    );
}

/// AC-13, AC-06: the text layer of folio 101, read back from its content stream through the fonts' ToUnicode maps,
/// equals the glyph runs the page prints, run by run and character by character: a lost, unmapped (U+FFFD) or merged
/// space is a mismatch, and every device tag the page prints reads back as often as it prints.
#[test]
fn the_text_layer_of_a_written_page_equals_its_printed_texts_with_spaces() {
    let (page, out) = page_alone("101");
    let mut printed = Vec::new();
    printed_runs(&page.content.items, &mut printed);
    let read: Vec<String> = read_back(&out)[0].runs.iter().map(|r| r.text.clone()).collect();
    assert_eq!(read, printed);
    assert!(
        printed.iter().any(|t| t.contains(' ')),
        "the page prints a text with a space"
    );
    assert!(
        printed.iter().any(|t| t == "-101F1"),
        "the page prints a device tag"
    );
}

/// AC-14: the sample loaded with its records in a shuffled order, and the export's page phase on 2 and 8 threads,
/// give the serial file-order export byte for byte: the page display lists, the PDF, the SVG files and the CSV of every
/// report block and of the cable and parts exports. Two runs give the same PDF.
#[test]
fn a_record_shuffle_and_a_parallel_export_give_the_same_files() {
    fn csvs(s: &ew_api::Session) -> BTreeMap<String, String> {
        let q = s.query();
        let mut out: BTreeMap<String, String> = q
            .report_blocks()
            .iter()
            .map(|(id, _)| {
                (
                    id.to_string(),
                    ew_reports::csv::rows_csv(&q, &q.get::<ew_reports::BlockRows>(id)),
                )
            })
            .collect();
        for what in [
            ew_api::Export::Cables,
            ew_api::Export::Bom(ew_api::BomMode::Component),
        ] {
            out.insert(
                format!("{what:?}"),
                s.export_csv(what).expect("the sample exports it"),
            );
        }
        out
    }
    let base = sample_session(None);
    let q = base.query();
    let (options, f) = (pdf_options(&q), fonts());
    let pages = pdf_pages(&q, Parallelism::Serial);
    let written = pdf(&pages, &options, &f, None).expect("the PDF is written");
    let svg_of = |pages: &[PdfPage], p| svg(pages, &svg_options(), None, p).expect("the SVG is written");
    let svgs = svg_of(&pages, Parallelism::Serial);
    assert_eq!(svgs.files.len(), pages.len(), "one SVG file per page");
    assert!(
        pdf(&pages, &options, &f, None).unwrap().bytes == written.bytes,
        "two runs differ"
    );

    let shuffled = sample_session(Some(SHUFFLE_SEED));
    let qs = shuffled.query();
    let pages_s = pdf_pages(&qs, Parallelism::Serial);
    assert!(
        pages_s == pages,
        "a record shuffle changes the page display lists"
    );
    assert!(
        pdf(&pages_s, &pdf_options(&qs), &f, None).unwrap().bytes == written.bytes,
        "a record shuffle changes the PDF"
    );
    assert!(
        svg_of(&pages_s, Parallelism::Serial) == svgs,
        "a record shuffle changes the SVG files"
    );
    assert_eq!(csvs(&shuffled), csvs(&base), "a record shuffle changes a CSV");

    for threads in [2, 8] {
        let p = Parallelism::threads(threads);
        let pages_p = pdf_pages(&q, p);
        assert!(
            pages_p == pages,
            "{threads} threads: the page display lists differ"
        );
        assert!(
            pdf(&pages_p, &options, &f, None).unwrap().bytes == written.bytes,
            "{threads} threads: the PDF differs"
        );
        assert!(
            svg_of(&pages_p, p) == svgs,
            "{threads} threads: the SVG files differ"
        );
    }
}
