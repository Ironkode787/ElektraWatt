// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool HUD and the page's horizontal scroll bar (FT-2096, doc 16a §5.5): on a page zoomed in so the bar shows,
//! the HUD stands above it, so the bar's thumb can be seen and dragged along its whole track.

use crate::support;

use egui_kittest::kittest::{NodeT as _, Queryable};
use serde_json::json;
use support::{Tmp, dispatch, on_sample, settle};

#[test]
fn the_hud_stands_above_the_horizontal_scroll_bar() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "101");
    for _ in 0..3 {
        dispatch(&mut h, "view.zoom_in", json!({}));
    }
    settle(&mut h);
    let hud = h
        .get_all_by_role(egui::Role::Status)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Tool"))
        .expect("the HUD")
        .rect();
    let bar = h.get_by_label("Scroll the page sideways").rect();
    assert!(
        hud.max.y <= bar.min.y,
        "the HUD {hud:?} over the scroll bar {bar:?}"
    );
    h.state_mut().shell_mut().shutdown();
}
