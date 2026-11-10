// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1019 on Golden A: the rows of panel page 902's Placement category in Properties read in the user's words, and
//! the scale is typed as it shows.

use crate::golden_common;

use ew_api::Session;
use ew_api::output::{Category, InputError, RowValue};
use ew_derive::ew_ids::Uuid;
use serde_json::json;

fn page_of(s: &Session, folio: &str) -> Uuid {
    s.page_rows()
        .into_iter()
        .find(|r| r.folio == folio)
        .unwrap_or_else(|| panic!("no folio {folio}"))
        .id
}

/// "a placement's rows show labels and values in the user's words, and the members with no meaning
/// to the user move to Advanced (doc 16a §6.8)". 902's Placement rows: the scale reads "1:10", not its JSON; the form's
/// UUID and the connection grid's fraction are the Advanced rows (`ew_viewmodel::properties::is_advanced`).
#[test]
fn the_placement_rows_of_902_read_in_words() {
    let s = golden_common::session(&golden_common::batch());
    let sheet = s.property_sheet(&[page_of(&s, "902")]);
    let rows: Vec<(String, String)> = sheet
        .sections
        .iter()
        .flat_map(|sec| &sec.rows)
        .filter(|r| r.category == Category::Placement)
        .map(|r| {
            let text = match &r.value {
                RowValue::One { text, .. } => text.clone(),
                other => format!("{other:?}"),
            };
            (r.key.clone(), text)
        })
        .collect();
    let want: Vec<(String, String)> = [
        ("page/form", "c80af135-34f8-574c-8f52-756bd24c7bd0"),
        ("page/grid", ""),
        ("page/grid.den", "3"),
        ("page/grid.num", "7937500"),
        ("page/scale", "1:10"),
    ]
    .iter()
    .map(|(k, t)| ((*k).to_owned(), (*t).to_owned()))
    .collect();
    assert_eq!(rows, want);
}

/// The scale's editor reads the words it shows: `1:5` and `1 : 20`, `not to scale`; JSON as before; `1:0` refused in
/// words.
#[test]
fn the_scale_is_typed_as_it_shows() {
    let s = golden_common::session(&golden_common::batch());
    let value = |text: &str| s.parse_field_input("page", "scale", text).map(|p| p.value);
    assert_eq!(value("1:5").unwrap(), json!({"one_to": 5}));
    assert_eq!(value(" 1 : 20 ").unwrap(), json!({"one_to": 20}));
    assert_eq!(value("not to scale").unwrap(), json!("not_to_scale"));
    assert_eq!(value(r#"{"one_to": 7}"#).unwrap(), json!({"one_to": 7}));
    match value("1:0") {
        Err(e @ InputError::Invalid(_)) => assert_eq!(
            e.text().unwrap(),
            "`1:0` is not a scale; type 1:n with n a whole number from 1 to 4294967295"
        ),
        other => panic!("{other:?}"),
    }
}
