// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A project's PDF (doc 20 §3.3 "PDF structure"): the page list and options of a project through the
//! shipped export job (`ew_api::output`) and the text of its structure. Shared by `tests/it/page_output.rs` and
//! `tests/it/pdf_output.rs`.

use ew_api::output::{self, ExportAssets, ExportOptions};
use ew_derive::Q;
use ew_export::{Parallelism, PdfOptions, PdfOutput, PdfPage, SvgOptions};

use super::view::ViewedPage;

/// The glyphs the PDF writer drops because neither bundled font has them. The "∞" of the nine main-function
/// cross-reference texts and the "−" and "⇄" of the report pages come from the symbol fallback face
/// (ADR-0028 recommendation 3), so none is dropped.
pub const EXPECTED_DROPPED_NOTDEF_GLYPHS: usize = 0;

/// The PDF page list of the project of `q` through the shipped export job (`ew_api::output::export_pages_with`: the
/// memoised layers, then the page phase on `par_map` under `parallelism`; FR-OUT-019, one code path for the
/// serial and the parallel export).
pub fn pdf_pages(q: &Q, parallelism: Parallelism) -> Vec<PdfPage> {
    output::export_pages_with(
        q,
        &ExportOptions {
            parallelism,
            ..ExportOptions::default()
        },
        &ExportAssets::default(),
    )
    .expect("the pages export")
    .pages
}

/// The export options of the PDF: the shipped job's (`ew_api::output::pdf_options`; dates from the
/// model's edited date, FR-OUT-026).
pub fn pdf_options(q: &Q) -> PdfOptions {
    output::export_pages_with(q, &ExportOptions::default(), &ExportAssets::default())
        .expect("the pages export")
        .options
}

/// The SVG options of the export: the shipped job's.
pub fn svg_options() -> SvgOptions {
    output::svg_options(ew_text::bundled(), false)
}

/// The text of a PDF's structure over `pages` (its pages, labels, outline and fonts), as JSON.
pub fn structure_text(pages: &[PdfPage], out: &PdfOutput) -> String {
    let faces: Vec<_> = ew_text::bundled().faces().collect();
    let mut names: Vec<String> = out
        .report
        .fonts
        .iter()
        .map(|key| {
            let f = faces
                .iter()
                .find(|f| f.key == *key)
                .expect("an embedded font is a bundled face");
            let italic = if f.italic { " Italic" } else { "" };
            format!("{} {:?}{italic}", f.family_name, f.weight)
        })
        .collect();
    names.sort();
    names.dedup();
    let labels: Vec<String> = pages.iter().map(|p| p.label.clone()).collect();
    // The JSON map of the workspace's serde_json sorts its keys, so the file is written by hand in the
    // documented key order.
    let mut text = String::from("{\n");
    text += &format!("  \"pages\": {},\n", pages.len());
    text += &format!("  \"page_labels\": {},\n", string_list(&labels, 1));
    text += &format!(
        "  \"outline\": {},\n",
        outline_json(&ew_export::outline_entries(pages), 1)
    );
    text += &format!("  \"links\": {},\n", out.report.links);
    text += &format!("  \"plain_text_links\": {},\n", out.report.plain_text_links);
    text += &format!(
        "  \"dropped_notdef_glyphs\": {},\n",
        out.report.dropped_notdef_glyphs
    );
    text += &format!("  \"destinations\": {},\n", out.report.destinations);
    text += &format!("  \"fonts\": {}\n}}\n", string_list(&names, 1));
    text
}

/// A JSON array of strings, one per line, indented for nesting level `depth`.
pub fn string_list(items: &[String], depth: usize) -> String {
    if items.is_empty() {
        return "[]".to_owned();
    }
    let pad = "  ".repeat(depth + 1);
    let lines: Vec<String> = items
        .iter()
        .map(|i| format!("{pad}{}", serde_json::to_string(i).unwrap()))
        .collect();
    format!("[\n{}\n{}]", lines.join(",\n"), "  ".repeat(depth))
}

/// The outline as JSON objects `{title, page, children}` in that key order.
pub fn outline_json(entries: &[ew_export::OutlineEntry], depth: usize) -> String {
    if entries.is_empty() {
        return "[]".to_owned();
    }
    let pad = "  ".repeat(depth + 1);
    let items: Vec<String> = entries
        .iter()
        .map(|e| {
            let title = serde_json::to_string(&e.title).unwrap();
            let children = outline_json(&e.children, depth + 2);
            format!(
                "{pad}{{\n{pad}  \"title\": {title},\n{pad}  \"page\": {},\n{pad}  \"children\": {children}\n{pad}}}",
                e.page
            )
        })
        .collect();
    format!("[\n{}\n{}]", items.join(",\n"), "  ".repeat(depth))
}

/// The report row links the generated pages draw other than the PLC overview's connection-point links, per
/// folio in page order: every link of a drawn data row whose cell prints a text. Each is one link annotation
/// of the PDF, so the PDF's links are the cross-reference items, the PLC overview rows and these.
pub fn report_row_links(pages: &[ViewedPage]) -> Vec<(String, usize)> {
    pages
        .iter()
        .filter_map(|g| {
            let t = g.view.table.as_ref()?;
            let n: usize = t
                .rows
                .iter()
                .take(usize::try_from(ew_schema::HOUSE_ROWS_PER_PAGE).unwrap_or(usize::MAX))
                .filter(|r| r.kind == ew_scene::table::TableRowKind::Data)
                .map(|r| {
                    r.links
                        .iter()
                        .filter(|l| !matches!(l.dest, ew_dl::Destination::Cp(_)))
                        .filter(|l| {
                            t.columns.get(l.column).is_some_and(|c| c.weight > 0)
                                && r.cells.get(l.column).is_some_and(|s| !s.is_empty())
                        })
                        .count()
                })
                .sum();
            (n > 0).then(|| (g.entry.folio.clone(), n))
        })
        .collect()
}

/// The sum of [`report_row_links`].
pub fn report_row_link_count(pages: &[ViewedPage]) -> usize {
    report_row_links(pages).iter().map(|(_, n)| n).sum()
}
