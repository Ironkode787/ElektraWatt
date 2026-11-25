// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A medium's Colour stood in Properties under Electrical. A medium's colour marks its lines on the page and
//! stands under Identity; a cable core's colour stays an electrical value.

use ew_api::output::{Category, field_catalogue};

fn category(kind: &str, path: &str) -> Category {
    field_catalogue(kind)
        .unwrap_or_else(|| panic!("the {kind} catalogue"))
        .iter()
        .find(|f| f.path == path)
        .unwrap_or_else(|| panic!("{kind} {path}"))
        .category
}

#[test]
fn a_mediums_colour_is_no_electrical_value() {
    assert_eq!(category("medium", "colour"), Category::Identity);
    assert_eq!(category("conductor", "colour"), Category::Electrical);
}
