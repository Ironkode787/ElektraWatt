// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S4.02 part 3 in the window, on a writable copy of Golden A, folio 902: the Unplaced devices navigator's rows on
//! show; the tool options card while F55 floats, its two segmented controls (tab lists "Placing mode" and "Grid step")
//! sending `layout.options`; the card gone again when the tool ends. The placing flows themselves (the ghost against
//! the commit) are the viewmodel's `tests/it/layout_place.rs`.

use crate::support;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ew_ui::app::App;
use ew_ui::ew_viewmodel::ew_api::output::SnapMode;
use ew_ui::ew_viewmodel::shell::Outcome;
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn on_902(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({ "folio": "902" }));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

#[test]
fn the_unplaced_navigator_and_the_layout_options_card() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    dispatch(&mut h, "navigator.show", json!({ "kind": "unplaced" }));
    settle(&mut h);
    for label in ["-201U9", "E55", "E56", "F55", "S56"] {
        assert!(h.query_by_label_contains(label).is_some(), "the row {label}");
    }
    assert!(
        h.query_by_label("Placing mode").is_none(),
        "no card before the tool runs"
    );

    let f55 = h
        .state()
        .shell()
        .panel_tables()
        .unwrap()
        .unplaced
        .iter()
        .find(|u| u.label == "F55")
        .unwrap()
        .device;
    dispatch(&mut h, "tool.place_layout", json!({ "device": f55.to_string() }));
    settle(&mut h);
    assert!(h.query_by_label("Placing mode").is_some(), "the mode's tab list");
    assert!(h.query_by_label("Grid step").is_some(), "the grid's tab list");
    h.get_by_role_and_label(egui::Role::Tab, "Push").click();
    settle(&mut h);
    assert_eq!(h.state().shell().layout_options().mode, SnapMode::Push);
    h.get_by_role_and_label(egui::Role::Tab, "1 mm").click();
    settle(&mut h);
    assert_eq!(h.state().shell().layout_options().grid_mm, 1);
    // Escape lets the device go; the tool ends with the next Escape and the card with it.
    dispatch(&mut h, "ui.dismiss", json!({}));
    dispatch(&mut h, "ui.dismiss", json!({}));
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "select");
    assert!(
        h.query_by_label("Placing mode").is_none(),
        "the card goes with the tool"
    );
    // The sticky choice stays for the next placing.
    assert_eq!(h.state().shell().layout_options().mode, SnapMode::Push);
}
