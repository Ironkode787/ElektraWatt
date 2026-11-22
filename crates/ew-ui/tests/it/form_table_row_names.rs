// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A form table that adds rows names them by its own row word for a screen reader (FT-1965): Copy pages…'s rename
//! table, a row added, names its cells and its remove button "text to change 1", never "value set 1".

use crate::support::{Tmp, dispatch, on_sample, settle};
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use serde_json::json;

#[test]
fn the_rename_table_names_its_rows_by_its_own_word() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "102");
    dispatch(&mut h, "pages.copy", json!({}));
    settle(&mut h);
    h.get_by_label("Add a text to change").click();
    settle(&mut h);
    let labels: Vec<String> = h
        .root()
        .children_recursive()
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    for want in [
        "Old text, text to change 1",
        "New text, text to change 1",
        "Remove text to change 1",
    ] {
        assert!(labels.iter().any(|l| l == want), "{want} in {labels:?}");
    }
    assert!(!labels.iter().any(|l| l.contains("value set")), "{labels:?}");
}
