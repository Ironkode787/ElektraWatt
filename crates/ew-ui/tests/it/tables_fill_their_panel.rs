// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! At 2560 × 1440 the Bus, Devices and Medium tables kept their columns at their start width and left the
//! rest of their panel empty. A table without a filling column shares the room its columns leave among them, so its
//! columns reach the grid's right edge (doc 16a §6.21).

use crate::support;

use egui_kittest::kittest::Queryable;
use serde_json::json;
use support::{Tmp, dispatch, on_sample, settle};

/// The right edge of the rightmost cell of the grid named `grid`, and the grid's own right edge.
fn edges(h: &egui_kittest::Harness<'_, ew_ui::app::App>, grid: &str) -> (f32, f32) {
    let rect = h.get_by_label(grid).rect();
    let right = h
        .get_all_by_role(egui::Role::Cell)
        .map(|n| n.rect())
        .filter(|r| rect.intersects(*r))
        .map(|r| r.right())
        .fold(f32::MIN, f32::max);
    (right, rect.right())
}

#[test]
fn the_devices_table_fills_its_panel_at_2560() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "101");
    h.set_size(egui::vec2(2560.0, 1440.0));
    settle(&mut h);
    dispatch(&mut h, "navigator.show", json!({"kind": "devices", "on": true}));
    dispatch(&mut h, "navigator.table", json!({"kind": "devices", "on": true}));
    settle(&mut h);
    settle(&mut h);
    let (cells, grid) = edges(&h, "Devices table");
    // The room left is at most a scroll bar's and a gap's.
    assert!(
        grid - cells < 40.0,
        "the columns end at {cells}, the grid at {grid}"
    );
    h.state_mut().shell_mut().shutdown();
}
