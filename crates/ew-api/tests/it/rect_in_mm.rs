// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A rectangle (an embedded block's area, a legend's) reads as its place and size in millimetres and is typed back
//! that way, never as the stored nanometres.

use ew_api::output::{field_catalogue, format_field_value};
use serde_json::json;

#[test]
fn a_rectangle_shows_and_is_typed_as_its_place_and_size_in_mm() {
    let catalogue = field_catalogue("report_block").expect("the report block's fields");
    let rect = catalogue
        .iter()
        .find(|f| f.path == "embedded.rect")
        .expect("the embedded block's area");
    let stored = json!([[20_000_000, 15_500_000], [80_000_000, 55_500_000]]);
    assert_eq!(
        format_field_value(&rect.ty, &stored),
        "20.000, 15.500 mm · 60.000 × 40.000 mm"
    );
    let s = crate::common::sample();
    let value = |text: &str| {
        s.parse_field_input("report_block", "embedded.rect", text)
            .map(|p| p.value)
    };
    assert_eq!(value("20.000, 15.500 mm · 60.000 × 40.000 mm").unwrap(), stored);
    assert_eq!(value("20, 15,5 · 60 x 40 mm").unwrap(), stored);
    // The stored JSON still reads; a size of nothing is refused in words.
    assert_eq!(value(&stored.to_string()).unwrap(), stored);
    assert!(value("20, 15 mm · 0 × 40 mm").is_err());
}
