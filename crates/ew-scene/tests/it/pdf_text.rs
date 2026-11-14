// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The text a PDF page draws as the migration verifier reads it (`ew_scene::pdf_text`): glyphs drawn one by one
//! (as Qt's PDF engine writes them) join into the run they continue, and the labels of the pages are read from the
//! page-label tree.

use std::sync::Arc;

use ew_scene::pdf_text::{pdf_page_labels, pdf_page_text};

/// A PDF of one 200 × 100 pt page with the content stream `content` in Helvetica, and `labels` (a `/PageLabels`
/// dictionary body) when given. The cross-reference table is computed.
fn pdf(content: &str, labels: Option<&str>) -> Arc<[u8]> {
    let catalog = format!(
        "<< /Type /Catalog /Pages 2 0 R{} >>",
        labels.map_or_else(String::new, |l| format!(" /PageLabels << /Nums [{l}] >>"))
    );
    let bodies = [
        catalog,
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R \
         /Resources << /Font << /F1 5 0 R >> >> >>"
            .to_owned(),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in bodies.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{body}\nendobj\n", i + 1).bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", bodies.len() + 1).bytes());
    for o in offsets {
        out.extend(format!("{o:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            bodies.len() + 1
        )
        .bytes(),
    );
    out.into()
}

#[test]
fn glyphs_drawn_one_by_one_join_into_the_run_they_continue() {
    // Helvetica's A, B and C advance 6.67, 6.67 and 7.22 pt at 10 pt: "ABC" is one run drawn glyph by glyph; Z starts
    // 50 pt on, a new run; XY is one operator on a line of its own.
    let content = "BT /F1 10 Tf 20 50 Td (A) Tj 6.67 0 Td (B) Tj 6.67 0 Td (C) Tj 57.22 0 Td (Z) Tj ET \
                   BT /F1 10 Tf 20 20 Td (XY) Tj ET";
    let page = pdf_page(content);
    let texts: Vec<_> = page.runs.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(texts, ["ABC", "Z", "XY"]);
    // The anchor is the first glyph's origin, in millimetres from the top left of the page.
    let mm = 25.4 / 72.0;
    let (x, y) = page.runs[0].anchor;
    assert!(
        (x - 20.0 * mm).abs() < 1e-6 && (y - 50.0 * mm).abs() < 1e-6,
        "{:?}",
        page.runs[0]
    );
    assert!((page.width_mm - 200.0 * mm).abs() < 1e-6 && (page.height_mm - 100.0 * mm).abs() < 1e-6);
}

fn pdf_page(content: &str) -> ew_scene::pdf_text::PdfPageText {
    pdf_page_text(pdf(content, None), 1).expect("the page is read")
}

#[test]
fn the_page_labels_come_from_the_label_tree() {
    let with = pdf("", Some("0 << /P (Front-) /S /D /St 5 >>"));
    assert_eq!(
        pdf_page_labels(with).expect("labels"),
        [Some("Front-5".to_owned())]
    );
    let without = pdf("", None);
    assert_eq!(pdf_page_labels(without).expect("labels"), [None]);
}

#[test]
fn a_page_the_file_lacks_is_refused_in_words() {
    let e = pdf_page_text(pdf("", None), 2).expect_err("no page 2");
    assert_eq!(e.to_string(), "the PDF has no page 2; it has 1");
}
