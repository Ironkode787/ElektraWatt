// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S4.02 part 4 in the window, on a writable copy of Golden A, folio 902 (PS-7's kittest flows): Draw ▸ Layout's
//! Duct button arming Draw duct with its options card; the Create enclosure dialog with its two choice lists and its
//! refusal under the fields, and its Create disabled without a part; the Auto-fill rail dialog's line and its Fill
//! button; Rail 1's Properties with its free length. The drawing, the commits and the poses are the viewmodel's
//! `tests/it/layout_setup.rs`.

use crate::support;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::{Dialog, Outcome};
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
fn the_duct_button_arms_draw_duct_with_its_options_card() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    dispatch(&mut h, "view.command_tab", json!({ "tab": "draw" }));
    settle(&mut h);
    assert!(
        h.query_all_by_label("Grid step").next().is_none(),
        "no card before the tool runs"
    );
    h.get_by_role_and_label(egui::Role::Button, "Duct").click();
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "duct_draw");
    assert!(
        h.query_all_by_label("Grid step").next().is_some(),
        "the grid's tab list"
    );
    assert!(h.query_all_by_label("Part").next().is_some(), "the part's label");
    h.get_by_role_and_label(egui::Role::Tab, "1 mm").click();
    settle(&mut h);
    assert_eq!(h.state().shell().layout_options().grid_mm, 1);
    dispatch(&mut h, "ui.dismiss", json!({}));
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "select");
    assert!(
        h.query_all_by_label("Grid step").next().is_none(),
        "the card goes with the tool"
    );
}

#[test]
fn the_create_enclosure_dialog_keeps_its_refusal_under_the_fields() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    dispatch(&mut h, "layout.enclosure_new", json!({}));
    settle(&mut h);
    assert!(
        h.query_all_by_label("Create enclosure").next().is_some(),
        "the title"
    );
    assert!(h.query_all_by_label("Location").next().is_some());
    assert!(h.query_all_by_label("Enclosure part").next().is_some());
    assert!(
        h.query_all_by_label_contains("+J-E-2416").next().is_some(),
        "the page's location chosen"
    );
    h.get_by_label("Create").click();
    settle(&mut h);
    let message = match h.state().shell().dialog() {
        Some(Dialog::Form(f)) => f.error.clone().expect("the refusal"),
        other => panic!("{other:?}"),
    };
    assert!(
        h.query_all_by_label(&message).next().is_some(),
        "under the fields: {message}"
    );
}

/// W44.2 F1: with its part emptied, the dialog's Create is disabled; a click on it sends nothing and leaves no
/// refusal under the fields.
#[test]
fn create_is_disabled_without_a_part() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    dispatch(&mut h, "layout.enclosure_new", json!({}));
    settle(&mut h);
    h.state_mut().shell_mut().set_form_field("part", "");
    settle(&mut h);
    let rev = h.state().shell().revision();
    h.get_by_label("Create").click();
    settle(&mut h);
    match h.state().shell().dialog() {
        Some(Dialog::Form(f)) => assert_eq!(f.error, None, "nothing sent"),
        other => panic!("{other:?}"),
    }
    assert_eq!(h.state().shell().revision(), rev);
}

#[test]
fn the_auto_fill_dialog_says_which_devices_it_fills_with() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    dispatch(&mut h, "layout.auto_fill_rail", json!({}));
    settle(&mut h);
    assert!(
        h.query_all_by_label("Auto-fill rail").next().is_some(),
        "the title"
    );
    assert!(
        h.query_all_by_label("Fills with every unplaced device that fits the rail.")
            .next()
            .is_some(),
        "the devices' line"
    );
    assert!(
        h.query_all_by_label("Fill").next().is_some(),
        "the primary button"
    );
    assert!(h.query_all_by_label("Order").next().is_some());
}

#[test]
fn rail_1s_properties_show_its_free_length() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    let r1 = h
        .state()
        .shell()
        .panel_tables()
        .unwrap()
        .carriers
        .iter()
        .find(|c| c.label == "Rail 1")
        .unwrap()
        .id;
    dispatch(&mut h, "select.set", json!({ "ids": [r1.to_string()] }));
    settle(&mut h);
    assert!(h.query_all_by_label("Free length").next().is_some());
    assert!(h.query_all_by_label("341 mm").next().is_some());
}
