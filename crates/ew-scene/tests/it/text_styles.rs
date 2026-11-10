// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The text-style tables (ADR-0032 decision 5): `ew_text::builtin()` holds the same rows as
//! `ew_schema::BUILTIN_TEXT_STYLES`.

use ew_schema::{BUILTIN_TEXT_STYLES, TextWeight, qet_px_em};
use ew_text::{Weight, builtin, builtin_rows};

#[test]
fn the_builtin_table_of_ew_text_equals_the_schema_table() {
    let schema: Vec<(&str, u16, bool)> = BUILTIN_TEXT_STYLES
        .iter()
        .map(|(k, px, w)| (*k, *px, *w == TextWeight::Bold))
        .collect();
    let text: Vec<(&str, u16, bool)> = builtin_rows()
        .iter()
        .map(|r| (r.key, r.qet_px, r.weight == Weight::Bold))
        .collect();
    assert_eq!(text, schema);
    assert_eq!(builtin().rows().len(), BUILTIN_TEXT_STYLES.len());
    for ((key, px, _), row) in BUILTIN_TEXT_STYLES.iter().zip(builtin().rows()) {
        assert_eq!((row.key.as_str(), row.em), (*key, qet_px_em(*px)));
    }
}
