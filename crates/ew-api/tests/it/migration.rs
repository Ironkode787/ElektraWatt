// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The migration verifier (`ew_api::migration`, FR-IMP-021, doc 14 §4.15) on small projects: the reference PDF is the
//! project's own export, or the export of the project before an edit.

use std::sync::Arc;

use ew_api::migration::{Expected, MigrationReport, parse_expected, verify_migration};
use ew_api::output::ExportOptions;
use ew_api::{AnyCommand, Session, SessionOptions};
use ew_derive::{PageOrder, PageRef};
use ew_schema::LibDocument;

/// A project with one page per `(number, title)`.
fn project(pages: &[(u32, &str)]) -> Session {
    project_on(pages, None)
}

/// A project with one page per `(number, title)`, each on a frame form of this sheet size in millimetres (the
/// default sheet without one).
fn project_on(pages: &[(u32, &str)], sheet_mm: Option<(i64, i64)>) -> Session {
    let row = serde_json::from_str(
        r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
    )
    .expect("a project row");
    let form_id = "3b1d6e52-8a47-7c09-b5e3-1f2a9d4c7e60";
    let forms: Vec<LibDocument> = sheet_mm
        .map(|(w, h)| {
            let copy = serde_json::from_value(serde_json::json!({
                "id": form_id,
                "library_id": "7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37",
                "item_id": "5e8f0a13-2b64-7d95-8c07-4a3b6d1e9f28",
                "version": "1.0.0",
                "sha256": format!("sha256:{}", "0".repeat(64)),
                "kind": "frame",
                "sheet": {"width": w * 1_000_000, "height": h * 1_000_000}
            }))
            .expect("a frame form");
            LibDocument::Form(copy)
        })
        .into_iter()
        .collect();
    let mut session =
        Session::from_records(row, [], forms, SessionOptions::default()).expect("an empty model");
    for (number, title) in pages {
        let mut params = serde_json::json!({"number": number, "title": title});
        if sheet_mm.is_some() {
            params["form"] = form_id.into();
        }
        session
            .execute(&AnyCommand {
                name: "page.create".into(),
                params,
            })
            .expect("a page");
    }
    // The empty frame prints no text of its own: each page gets its title as a free text.
    let drawn: Vec<_> = session
        .query()
        .get::<PageOrder>(())
        .entries
        .iter()
        .filter_map(|e| match e.page {
            PageRef::Drawn(id) => Some(id),
            _ => None,
        })
        .collect();
    for (page, (_, title)) in drawn.into_iter().zip(pages) {
        session
            .execute(&AnyCommand {
                name: "item.place".into(),
                params: serde_json::json!({
                    "page": page.to_string(),
                    "kind": {"free_text": {"text": title, "at": [50_800_000, 30_480_000]}}
                }),
            })
            .expect("a text");
    }
    session
}

/// The PDF of `session`'s pages.
fn pdf_of(session: &Session) -> Arc<[u8]> {
    session
        .export_pdf(&ExportOptions::default())
        .expect("the export")
        .bytes
        .into()
}

fn kinds(report: &MigrationReport) -> Vec<&str> {
    report.deviations.iter().map(|d| d.kind.as_str()).collect()
}

#[test]
fn a_project_matches_its_own_pdf() {
    let session = project(&[(1, "Cover"), (2, "Power")]);
    let report = verify_migration(&session, pdf_of(&session), &[]).expect("a verification");
    // Every page paired by its folio label; the drawing and every text agree.
    assert!(
        report.pages >= 2 && report.texts_matched >= 2 && report.cells > 0,
        "{report:?}"
    );
    assert!(report.is_clean(), "{report:?}");
}

/// FR-OUT-008: every text the program writes into a PDF is found again, whatever the font does with its letters: "fi"
/// and "fl" are one glyph of the font (a ligature) and still two letters of the text, and the glyph-to-text map of one
/// run must not give wrong letters to the runs that follow it.
#[test]
fn texts_with_ligatures_are_read_back_from_the_pdf_whole() {
    let titles = ["Definition of the flow, five final offices", "Default fine flat"];
    let session = project(&[(1, titles[0]), (2, titles[1])]);
    for (page, title) in (1..).zip(titles) {
        let read = ew_scene::pdf_text::pdf_page_text(pdf_of(&session), page).expect("the page is read");
        let texts: Vec<&str> = read.runs.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts, [title], "page {page}");
    }
    let report = verify_migration(&session, pdf_of(&session), &[]).expect("a verification");
    assert!(report.is_clean(), "{report:?}");
}

/// A text holding a glyph the main font lacks (⇄, ∞) is drawn in two fonts and is one text on both sides.
#[test]
fn a_text_with_a_glyph_of_another_font_is_one_text_on_both_sides() {
    let session = project(&[(1, "B1 (0V ⇄ PE) ∞ 3-B2")]);
    let report = verify_migration(&session, pdf_of(&session), &[]).expect("a verification");
    assert!(report.is_clean(), "{report:?}");
}

/// Golden A against its own PDF: every text of every page, the report tables' cells and the texts with a glyph of
/// another font among them, is found where the project has it.
#[test]
#[ignore = "deep run: Golden A's 36 pages drawn and compared"]
fn golden_a_matches_its_own_pdf() {
    let session = crate::golden_common::session(&crate::golden_common::batch());
    let report = verify_migration(&session, pdf_of(&session), &[]).expect("a verification");
    assert!(report.texts_matched > 0 && report.cells > 0, "{report:?}");
    let found: Vec<String> = report
        .deviations
        .iter()
        .map(|d| format!("{} {} {:?} {:?}", d.page, d.kind, d.text, d.at_mm))
        .collect();
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn a_retitled_page_differs_in_its_texts_only() {
    let before = project(&[(1, "Cover"), (2, "Power")]);
    let after = project(&[(1, "Cover"), (2, "Motors")]);
    let report = verify_migration(&after, pdf_of(&before), &[]).expect("a verification");
    let texts = |kind: &str| -> Vec<String> {
        report
            .deviations
            .iter()
            .filter(|d| d.kind == kind)
            .filter_map(|d| d.text.clone())
            .collect()
    };
    assert!(texts("text-missing").iter().any(|t| t == "Power"), "{report:?}");
    assert!(texts("text-extra").iter().any(|t| t == "Motors"), "{report:?}");
    assert!(report.deviations.iter().all(|d| d.page == "2"), "{report:?}");
}

#[test]
fn a_drawing_the_reference_lacks_is_a_raster_deviation_and_no_text_deviation() {
    let plain = project(&[(1, "Cover")]);
    let mut drawn = project(&[(1, "Cover")]);
    let page = match drawn.query().get::<PageOrder>(()).entries[0].page {
        PageRef::Drawn(id) => id,
        other => panic!("{other:?}"),
    };
    drawn
        .execute(&AnyCommand {
            name: "item.place".into(),
            params: serde_json::json!({
                "page": page.to_string(),
                "kind": {"graphic": {"shape": {"circle": {"centre": [150_000_000, 100_000_000], "r": 20_000_000}}}}
            }),
        })
        .expect("a circle");
    let report = verify_migration(&drawn, pdf_of(&plain), &[]).expect("a verification");
    // The circle crosses several frame cells, each named; nothing else differs.
    assert!(!report.deviations.is_empty(), "{report:?}");
    assert!(
        report
            .deviations
            .iter()
            .all(|d| d.kind == "raster-cell" && d.cell.is_some()),
        "{report:?}"
    );
    assert!(report.deviations.iter().all(|d| d.page == "1"));
}

#[test]
fn a_page_the_reference_lacks_and_a_page_the_project_lacks_are_deviations() {
    let two = project(&[(1, "Cover"), (2, "Power")]);
    let one = project(&[(1, "Cover")]);
    let report = verify_migration(&two, pdf_of(&one), &[]).expect("a verification");
    assert!(kinds(&report).contains(&"page-not-in-reference"), "{report:?}");
    let report = verify_migration(&one, pdf_of(&two), &[]).expect("a verification");
    assert!(kinds(&report).contains(&"page-not-in-project"), "{report:?}");
}

#[test]
fn pages_pair_by_order_when_the_labels_do_not_match() {
    // The reference's pages are labelled 1 and 2; the project's folios are 5 and 6.
    let reference = project(&[(1, "Cover"), (2, "Power")]);
    let renumbered = project(&[(5, "Cover"), (6, "Power")]);
    let report = verify_migration(&renumbered, pdf_of(&reference), &[]).expect("a verification");
    assert!(report.pages >= 2, "{report:?}");
    assert!(report.is_clean(), "{report:?}");
}

/// FT-1748: a page missing on one side leaves the pages after it paired by their labels; the pages no label claims pair
/// in order.
#[test]
fn a_missing_page_does_not_shift_the_pairs_after_it() {
    let three = project(&[(1, "Cover"), (2, "Power"), (3, "Motors")]);
    let without_two = project(&[(1, "Cover"), (3, "Motors")]);
    let report = verify_migration(&three, pdf_of(&without_two), &[]).expect("a verification");
    assert_eq!(kinds(&report), ["page-not-in-reference"], "{report:?}");
    assert_eq!(report.deviations[0].page, "2");
    assert_eq!(report.pages, 2, "folios 1 and 3 compared with their own pages");

    // The reference counts 1, 2, 3; the project's last folio is 9: that page takes the page no folio names.
    let reference = project(&[(1, "Cover"), (2, "Power"), (3, "Motors")]);
    let renumbered = project(&[(1, "Cover"), (2, "Power"), (9, "Motors")]);
    let report = verify_migration(&renumbered, pdf_of(&reference), &[]).expect("a verification");
    assert!(report.pages == 3 && report.is_clean(), "{report:?}");
}

#[test]
fn declared_deltas_are_expected_and_a_declared_delta_not_found_is_reported() {
    let before = project(&[(1, "Cover"), (2, "Power")]);
    let after = project(&[(1, "Cover"), (2, "Motors")]);
    let declared = parse_expected(
        r#"[{"kind":"text-extra","page":"2","text":"Motors"},
            {"kind":"text-missing","page":"2","text":"Power"},
            {"kind":"text-moved","page":"1"}]"#,
    )
    .expect("a list of deltas");
    let report = verify_migration(&after, pdf_of(&before), &declared).expect("a verification");
    assert_eq!(report.expected.len(), 2, "{report:?}");
    assert_eq!(
        report.unmet,
        vec![Expected {
            kind: "text-moved".into(),
            page: Some("1".into()),
            text: None,
            cell: None
        }]
    );
    assert!(!report.is_clean());
}

#[test]
fn a_declaration_that_is_no_list_of_deltas_is_refused_in_words() {
    for bad in [
        "{}",
        "[{\"page\":\"1\"}]",
        "[{\"kind\":\"text-extra\",\"colour\":1}]",
        "not json",
    ] {
        let e = parse_expected(bad).expect_err(bad);
        assert!(e.starts_with("the expected deltas are no list of deltas"), "{e}");
    }
}

#[test]
fn a_file_that_is_no_pdf_is_refused_in_words() {
    let session = project(&[(1, "Cover")]);
    let e = verify_migration(&session, Arc::from(&b"not a pdf"[..]), &[]).expect_err("no PDF");
    assert!(e.to_string().starts_with("the reference PDF: "), "{e}");
}

#[test]
fn the_raster_of_a_page_is_compared_whatever_its_size() {
    // Sheets whose pixel size at the verifier's resolution the reference's own raster does not hit exactly.
    for sheet in [
        (297, 210),
        (420, 297),
        (400, 283),
        (353, 251),
        (211, 149),
        (593, 419),
    ] {
        let session = project_on(&[(1, "Cover")], Some(sheet));
        let report = verify_migration(&session, pdf_of(&session), &[]).expect("a verification");
        assert!(report.cells > 0, "{sheet:?}: {report:?}");
        assert!(report.is_clean(), "{sheet:?}: {report:?}");
    }
}

#[test]
fn a_drawing_is_found_on_a_page_of_any_size() {
    // The circle exists in the project only: were the raster skipped, nothing would be said.
    let plain = project_on(&[(1, "Cover")], Some((353, 251)));
    let mut drawn = project_on(&[(1, "Cover")], Some((353, 251)));
    let page = match drawn.query().get::<PageOrder>(()).entries[0].page {
        PageRef::Drawn(id) => id,
        other => panic!("{other:?}"),
    };
    drawn
        .execute(&AnyCommand {
            name: "item.place".into(),
            params: serde_json::json!({
                "page": page.to_string(),
                "kind": {"graphic": {"shape": {"circle": {"centre": [150_000_000, 100_000_000], "r": 20_000_000}}}}
            }),
        })
        .expect("a circle");
    let report = verify_migration(&drawn, pdf_of(&plain), &[]).expect("a verification");
    assert!(kinds(&report).contains(&"raster-cell"), "{report:?}");
}

#[test]
fn a_page_too_large_for_the_comparison_is_refused_in_words() {
    // A1: about 4970 pixels wide at 150 dpi, more than the raster the reference can be drawn at.
    let session = project_on(&[(1, "Cover")], Some((841, 594)));
    let e = verify_migration(&session, pdf_of(&session), &[]).expect_err("too large");
    let text = e.to_string();
    assert!(text.starts_with("page 1 is too large"), "{text}");
    assert!(text.contains("pixels"), "{text}");
}
