// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1761: a row's right-click menu opened at the pointer and covered the row it acts on. It opens under the row,
//! where the pointer was along it.

use crate::support;

use egui_kittest::kittest::Queryable;
use ew_viewmodel::navigators::NavKind;
use serde_json::json;
use support::{Tmp, dispatch, menu_node, on_sample, settle};

#[test]
fn a_rows_menu_opens_under_the_row_it_acts_on() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "101");
    dispatch(&mut h, "navigator.show", json!({ "kind": "strips" }));
    settle(&mut h);
    let name = h
        .state()
        .shell()
        .shown_rows(NavKind::Strips)
        .into_iter()
        .find(|r| r.role == "strip")
        .map(|r| r.name)
        .expect("a strip row");
    let row = h.get_by_label(&name).rect();
    h.get_by_label(&name).click_secondary();
    settle(&mut h);
    settle(&mut h);
    let entry = menu_node(&h, "Add terminal").rect();
    assert!(
        !entry.intersects(row),
        "the menu's entry {entry:?} covers its row {row:?}"
    );
    assert!(entry.top() >= row.bottom() - 0.5, "{entry:?} under {row:?}");
    h.state_mut().shell_mut().shutdown();
}
