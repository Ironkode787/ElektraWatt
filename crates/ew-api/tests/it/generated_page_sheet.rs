// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property sheet of a generated page of the sample is titled by the page's folio and
//! derived title ("901 Partlist 1"), not by its report block's title format, and its Title row shows the title the
//! page prints.

use crate::common;

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
fn a_generated_page_is_titled_by_its_folio_and_derived_title() {
    let s = common::sample();
    let first = sheet_of(&s, "901");
    assert_eq!(first.header.title, "901 Partlist 1");
    assert_eq!(block_title(&first), "Partlist 1");
    let second = sheet_of(&s, "901.1");
    assert_eq!(second.header.title, "901.1 Partlist 2");
    assert_eq!(block_title(&second), "Partlist 2");
}

/// Two pages of one report picked together print a title each: their Title row shows the block's format they share,
/// in English and Dutch (doc 09 §3.8), not the first page's title.
#[test]
fn pages_picked_together_show_the_title_format_they_share() {
    let s = common::sample();
    let ids: Vec<_> = s
        .page_rows()
        .into_iter()
        .filter(|r| r.folio == "901" || r.folio == "901.1")
        .map(|r| r.id)
        .collect();
    assert_eq!(ids.len(), 2, "901 and 901.1");
    assert_eq!(block_title(&s.property_sheet(&ids)), "Partlist {n} [+1]");
}
