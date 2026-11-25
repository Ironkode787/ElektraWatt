// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The parts import's Map columns card stood at the top of the window, under the command bar, where it read
//! as a strip left behind. It stands in the window's centre, where a dialog stands, at 1920 × 1080 and 2560 × 1440.

use crate::support;

use serde_json::json;
use support::{Scripted, Tmp, dispatch, harness, settle, wait_for_page};

#[test]
fn the_mapping_card_stands_in_the_windows_centre() {
    let tables = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tables/parts.csv");
    for size in [egui::vec2(1920.0, 1080.0), egui::vec2(2560.0, 1440.0)] {
        let tmp = Tmp::new();
        let dir = tmp.sample("s.ew");
        let mut h = harness(&tmp, Scripted::default(), Some(dir));
        h.set_size(size);
        settle(&mut h);
        wait_for_page(&mut h);
        let path = tables.to_string_lossy().into_owned();
        dispatch(&mut h, "parts.import", json!({ "path": path }));
        settle(&mut h);
        settle(&mut h);
        let card = h
            .ctx
            .memory(|m| m.area_rect(egui::Id::unique("parts-map-card")))
            .expect("the mapping card");
        let window = h.ctx.content_rect();
        assert!(
            (card.center().y - window.center().y).abs() < 2.0
                && (card.center().x - window.center().x).abs() < 2.0,
            "{size:?}: the card {card:?} in the window {window:?}"
        );
        h.state_mut().shell_mut().shutdown();
    }
}
