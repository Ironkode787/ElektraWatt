// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The EPLAN report's Map columns card stood at the top of the window, under the command bar, where it read as a strip
//! left behind. It stands in the window's centre, where a dialog stands, at 1920 × 1080 and 2560 × 1440.

use crate::support;

use serde_json::json;
use support::{Scripted, Tmp, dispatch, harness, settle, wait_for_page};

#[test]
fn the_mapping_card_stands_in_the_windows_centre() {
    for size in [egui::vec2(1920.0, 1080.0), egui::vec2(2560.0, 1440.0)] {
        let tmp = Tmp::new();
        let report = tmp.0.join("klemmen.csv");
        std::fs::write(
            &report,
            "Terminal strip,Terminal,Level,Part number\n-X1,1,1,\n-X1,2,1,\n",
        )
        .unwrap();
        let dir = tmp.sample("s.ew");
        let mut h = harness(&tmp, Scripted::default(), Some(dir));
        h.set_size(size);
        settle(&mut h);
        wait_for_page(&mut h);
        let file = report.to_string_lossy().into_owned();
        dispatch(&mut h, "file.import_eplan", json!({ "file": file, "how": "map" }));
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
