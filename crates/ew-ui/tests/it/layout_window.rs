// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! In the window, on a writable copy of Golden A, folio 902: the Unplaced devices navigator's rows on
//! show; the tool options card while F55 floats, its two segmented controls (tab lists "Placing mode" and "Grid step")
//! sending `layout.options`; the card gone again when the tool ends. The placing flows themselves (the ghost against
//! the commit) are the viewmodel's `tests/it/layout_place.rs`.

use crate::support;

use egui_kittest::kittest::Queryable;
use ew_ui::ew_viewmodel::ew_api::output::SnapMode;
use serde_json::json;
use support::{Tmp, dispatch, on_golden, settle};

#[test]
fn the_unplaced_navigator_and_the_layout_options_card() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "902");
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
    // Escape drops the ghost and the tool gives way to Select, the card with it.
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

/// Doc 12 §12.1: Lock shows `lock` at the locked placement's top-right corner, standing on the
/// footprint's top edge, `icon.sm` square and fixed on screen at another zoom; Unlock takes it away.
#[test]
fn a_locked_placement_shows_the_lock_icon() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "902");
    assert!(h.state_mut().canvas_host().lock_marks(0).is_empty());
    let panel = h.state().shell().panel_tables().unwrap();
    let row = panel
        .placements
        .iter()
        .find(|p| p.tag == "-102F1")
        .unwrap()
        .clone();
    dispatch(&mut h, "select.set", json!({ "ids": [row.id.to_string()] }));
    dispatch(&mut h, "layout.lock_selection", json!({}));
    settle(&mut h);
    let size = ew_ui::theme::current(&h.ctx).icon_sm;
    for zoom in [false, true] {
        if zoom {
            dispatch(&mut h, "view.zoom_in", json!({}));
            settle(&mut h);
            settle(&mut h);
        }
        // The footprint's top-right corner on screen, as the last frame drew the page.
        let corner = {
            let shell = h.state().shell();
            let canvas = shell.canvas().unwrap();
            let page = canvas.page();
            let panel = page.panel.clone().unwrap();
            let (lo, hi) = (row.footprint.min(), row.footprint.max());
            let sheet: Vec<(i64, i64)> = [(lo.x.0, lo.y.0), (hi.x.0, hi.y.0)]
                .iter()
                .map(|&(u, v)| {
                    page.snapper
                        .page_to_sheet(panel.to_page((i128::from(u), i128::from(v))))
                })
                .collect();
            let host = h.state_mut().canvas_host();
            let pts: Vec<egui::Pos2> = sheet
                .iter()
                .map(|p| host.drawn_screen_of(0, *p).expect("a drawn canvas"))
                .collect();
            egui::pos2(pts[0].x.max(pts[1].x), pts[0].y.min(pts[1].y))
        };
        let marks = h.state_mut().canvas_host().lock_marks(0).to_vec();
        assert_eq!(marks.len(), 1, "one locked placement, one lock");
        let m = marks[0];
        assert_eq!(
            (m.width(), m.height()),
            (size, size),
            "icon.sm at zoom in: {zoom}"
        );
        assert!(
            (m.right() - corner.x).abs() < 0.5 && (m.bottom() - corner.y).abs() < 0.5,
            "{m:?} at {corner:?}"
        );
    }
    dispatch(&mut h, "layout.unlock_selection", json!({}));
    settle(&mut h);
    assert!(h.state_mut().canvas_host().lock_marks(0).is_empty());
}
