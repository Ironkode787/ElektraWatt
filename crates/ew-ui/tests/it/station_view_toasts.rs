// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1824: in a station's view, at 1920 × 1080, the toasts never cover a To do button: the To do stands under the
//! rack, clear of the toasts at the bottom right of the lists (doc 16a §6.10, doc 10 §6).

use crate::support::{Tmp, dispatch, on_sample, settle};

use egui_kittest::kittest::Queryable;
use ew_viewmodel::shell::station::TodoFix;
use serde_json::json;

/// The sample's valve terminal in its view, its To do with two steps (its electrical side, its overview page), and a
/// toast of three lines: no To do button lies under it.
#[test]
fn the_toasts_leave_the_todo_buttons_free() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "201");
    dispatch(&mut h, "plc.rack_editor", json!({"station": "tag:-201K1"}));
    settle(&mut h);
    // A toast of three lines: a new external reference's, which says how to use it.
    dispatch(
        &mut h,
        "external_ref.new",
        json!({"location": "+FLD", "device": "-K7 in the far control cabinet"}),
    );
    settle(&mut h);
    let labels: Vec<String> = h
        .state()
        .shell()
        .station_todo()
        .into_iter()
        .filter_map(|step| match step.fix? {
            TodoFix::Action { label, .. } | TodoFix::OnPage { label, .. } => Some(label),
            TodoFix::ChoosePart { .. } => None,
        })
        .collect();
    assert!(!labels.is_empty(), "the To do offers its steps");
    let stack = h
        .ctx
        .memory(|m| m.area_rect(egui::Id::unique("ew-toasts")))
        .expect("the toasts");
    for label in &labels {
        let button = h.get_by_label(label).rect();
        assert!(
            !button.intersects(stack),
            "{label} at {button:?} under the toasts at {stack:?}"
        );
    }
    h.state_mut().shell_mut().shutdown();
}
