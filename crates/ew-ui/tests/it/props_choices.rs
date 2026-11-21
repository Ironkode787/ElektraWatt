// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The owner's test of 0.3.0-rc.1: "If a property has a drop down menu, it should communicate that". In the property
//! grid a row whose value is chosen from a list is a combo box (with a chevron at its value's right end); a row typed
//! as text stays a button.

use crate::support;

use egui_kittest::kittest::Queryable;
use ew_viewmodel::properties::{ValueChooser, find_row, value_chooser};
use serde_json::json;
use support::{Tmp, dispatch, on_sample, settle};

#[test]
fn a_row_chosen_from_a_list_is_a_combo_box() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "101");
    dispatch(&mut h, "select.set", json!({"refs": ["tag:-101F1"]}));
    dispatch(&mut h, "props.show", json!({"on": true}));
    settle(&mut h);
    // The grid holds both kinds of row.
    let sheet = h.state().shell().properties().sheet().unwrap().clone();
    let keys: Vec<(String, Option<ValueChooser>)> = sheet
        .sections
        .iter()
        .flat_map(|s| s.rows.iter())
        .map(|r| (r.key.clone(), value_chooser(r)))
        .collect();
    assert!(
        keys.iter().any(|(_, c)| *c == Some(ValueChooser::List)),
        "{keys:?}"
    );
    let text_row = find_row(&sheet, "device/function_text").map(|(_, r)| value_chooser(r));
    assert_eq!(text_row, Some(None), "function text is typed");
    // In the window: combo boxes in the property grid, below its filter field.
    let filter = h.get_by_label("Filter properties").rect();
    let combos = h
        .get_all_by_role(egui::Role::ComboBox)
        .filter(|n| n.rect().top() > filter.bottom() && n.rect().right() <= filter.right() + 20.0)
        .count();
    assert!(combos > 0, "a list row is a combo box");
    h.state_mut().shell_mut().shutdown();
}
