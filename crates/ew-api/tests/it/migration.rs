// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The migration verifier (`ew_api::migration`, FR-IMP-021, doc 14 §4.15) on small projects: the reference PDF is the
//! project's own export, or the export of the project before an edit.

use std::sync::Arc;

use ew_api::migration::{Expected, MigrationReport, parse_expected, verify_migration};
use ew_api::output::ExportOptions;
use ew_api::{AnyCommand, Session, SessionOptions};
use ew_derive::{PageOrder, PageRef};

/// A project with one page per `(number, title)`.
fn project(pages: &[(u32, &str)]) -> Session {
    let row = serde_json::from_str(
        r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
    )
    .expect("a project row");
    let mut session = Session::from_records(row, [], [], SessionOptions::default()).expect("an empty model");
    for (number, title) in pages {
        session
            .execute(&AnyCommand {
                name: "page.create".into(),
                params: serde_json::json!({"number": number, "title": title}),
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
