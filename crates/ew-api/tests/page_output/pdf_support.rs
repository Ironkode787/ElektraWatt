// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The golden PDF (doc 20 §3.3 "PDF structure"): the page list and options of the golden project through the
//! shipped export job (`ew_api::output`), the committed structure file's text and the SHA-256. Shared by
//! `tests/page_output.rs` and the acceptance runner.

use std::collections::BTreeMap;

use ew_api::output::{self, ExportOptions};
use ew_derive::Q;
use ew_dl::FontKey;
use ew_export::{Parallelism, PdfOptions, PdfOutput, PdfPage, SvgOptions, SvgOutput, pdf};
use ew_render_cpu::FontBytes;
use sha2::{Digest, Sha256};

use super::view::{GoldenPage, golden_session};

/// The glyphs the PDF writer drops because neither bundled font has them. The "∞" of the nine main-function
/// cross-reference texts and the "−" and "⇄" of the report pages come from the symbol fallback face (FT-85,
/// ADR-0028 recommendation 3), so none is dropped.
pub const EXPECTED_DROPPED_NOTDEF_GLYPHS: usize = 0;

/// The PDF page list of the golden project through the shipped export job (`ew_api::output::export_pages`: the
/// memoised layers, then the page phase on `par_map` under `parallelism`; FR-OUT-019, one code path for the
/// serial and the parallel export).
pub fn pdf_pages(q: &Q, parallelism: Parallelism) -> Vec<PdfPage> {
    output::export_pages(
        q,
        &ExportOptions {
            parallelism,
            ..ExportOptions::default()
        },
    )
    .expect("the golden pages export")
    .pages
}

/// The export options of the golden PDF: the shipped job's (`ew_api::output::pdf_options`; dates from the
/// model's edited date, FR-OUT-026).
pub fn pdf_options(q: &Q) -> PdfOptions {
    output::export_pages(q, &ExportOptions::default())
        .expect("the golden pages export")
        .options
}

/// The SVG options of the golden export (used by the acceptance runner, AC-14): the shipped job's.
#[allow(dead_code)]
pub fn svg_options() -> SvgOptions {
    output::svg_options(false)
}

/// The SHA-256 over an SVG export: every file as its name, a newline, its text, in page order.
#[allow(dead_code)]
pub fn svg_sha256(out: &SvgOutput) -> String {
    let mut h = Sha256::new();
    for f in &out.files {
        h.update(f.name.as_bytes());
        h.update(b"\n");
        h.update(f.text.as_bytes());
    }
    format!("{:x}", h.finalize())
}

/// The SHA-256 of the golden PDF built over the golden model with its records shuffled by `seed` (`None`: in
/// file order).
pub fn pdf_sha256(seed: Option<u64>, fonts: &BTreeMap<FontKey, FontBytes>) -> String {
    let s = golden_session(seed);
    let q = s.query();
    let out = pdf(&pdf_pages(&q, Parallelism::Serial), &pdf_options(&q), fonts, None)
        .expect("the golden PDF is written");
    format!("{:x}", Sha256::digest(&out.bytes))
}

/// The text of the committed `expected/pdf/structure.json` for a PDF over `pages`.
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

/// The cross-reference texts of the drawn pages: (linked texts, their link targets, external texts without a
/// link). Doc 20 §15: 63 linked texts with 64 targets and 4 external ones.
pub fn xref_counts(pages: &[GoldenPage]) -> (usize, usize, usize) {
    let (mut linked, mut targets, mut external) = (0, 0, 0);
    for g in pages {
        for r in &g.view.reps {
            let Some(def) = &r.symbol else {
                continue;
            };
            for v in &r.slots {
                let is_xref = def
                    .text_slots
                    .get(usize::from(v.slot))
                    .is_some_and(|t| t.kind == ew_schema::SlotKind::Xref);
                if !v.item_links.is_empty() {
                    linked += 1;
                    targets += v.item_links.len();
                } else if is_xref && !v.text.is_empty() {
                    external += 1;
                }
            }
        }
    }
    (linked, targets, external)
}
