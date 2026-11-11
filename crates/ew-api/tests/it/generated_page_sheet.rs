// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property sheet of a generated page is titled by the page's folio and
//! derived title ("901 Partlist 1"), not by its report block's title format; the block's own Title row keeps the
//! format, which is what the block stores.

use crate::golden_common::read_only as golden;

use ew_api::Session;
use ew_api::output::{PropertySheet, RowValue};

/// The sheet of the page of the navigator labelled `folio`.
fn sheet_of(s: &Session, folio: &str) -> PropertySheet {
    let row = s
        .page_rows()
        .into_iter()
        .find(|r| r.folio == folio)
        .unwrap_or_else(|| panic!("no page {folio}"));
    assert!(row.generated, "{folio} is a generated page");
    s.property_sheet(&[row.id])
}

/// The text of the report block's Title row.
fn block_title(sheet: &PropertySheet) -> String {
    let section = sheet
        .sections
        .iter()
        .find(|x| x.kind == "report_block")
        .expect("the report block's section");
    let row = section
        .rows
        .iter()
        .find(|r| r.key == "report_block/title")
        .unwrap_or_else(|| {
            let keys: Vec<&str> = section.rows.iter().map(|r| r.key.as_str()).collect();
            panic!("no Title row: {keys:?}")
        });
    match &row.value {
        RowValue::One { text, .. } => text.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn p1_a_generated_page_is_titled_by_its_folio_and_derived_title() {
    let s = golden();
    let first = sheet_of(&s, "901");
    assert_eq!(first.header.title, "901 Partlist 1");
    assert_eq!(block_title(&first), "Partlist {n}");
    let second = sheet_of(&s, "901.1");
    assert_eq!(second.header.title, "901.1 Partlist 2");
    assert_eq!(block_title(&second), "Partlist {n}");
}
