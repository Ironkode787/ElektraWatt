// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Potential panel of a supply mark names its fields for a screen reader (FT-1910): on the sample's page 102, an
//! incoming mark selected and its panel open, each row's Conductor list and Voltage field read "<column>, <mark>".

use crate::support::{Tmp, dispatch, on_sample, settle};
use egui_kittest::kittest::NodeT as _;
use ew_viewmodel::shell::Outcome;
use serde_json::json;

#[test]
fn the_supply_panel_names_its_conductor_and_voltage_fields() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "102");
    let marks: Vec<String> = h
        .state()
        .shell()
        .canvas()
        .expect("page 102")
        .page()
        .tables
        .reps
        .iter()
        .filter(|r| r.ip.is_some())
        .map(|r| r.id.to_string())
        .collect();
    let opened = marks.iter().any(|mark| {
        let shell = h.state_mut().shell_mut();
        shell.dispatch("select.set", &json!({ "ids": [mark] })) == Outcome::Done
            && shell.dispatch("ip.supply_potential", &json!({})) == Outcome::Done
            && shell.supply_panel().is_some()
    });
    assert!(opened, "an incoming mark of 102 opens its Potential panel");
    settle(&mut h);
    let rows: Vec<String> = h
        .state()
        .shell()
        .supply_panel()
        .expect("the panel")
        .rows
        .iter()
        .map(|r| r.name.clone())
        .collect();
    let labels: Vec<String> = h
        .root()
        .children_recursive()
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    for name in rows {
        for column in ["Conductor", "Voltage"] {
            let want = format!("{column}, {name}");
            assert!(labels.contains(&want), "{want} in {labels:?}");
        }
    }
    dispatch(&mut h, "ui.dismiss", json!({}));
}
