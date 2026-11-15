// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The owner's test of 0.3.0-rc.1: Update in the Libraries pane and Delete on a page put a card in the Preview tab that
//! he never saw. A staged card is now said where the action started (the strip with Apply in the Pages pane), on the
//! Preview tab (its dot, while another tab is in front) and in the status bar ("… wait for Apply", which brings the
//! Preview forward).

use crate::support;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::layout::PaneKind;
use ew_ui::ew_viewmodel::status::waiting_text;
use serde_json::json;
use support::{Tmp, dispatch, on_golden, settle};

/// The right edge of the left column: the Pages pane's widgets stand left of it.
fn left_column(h: &Harness<'_, App>) -> f32 {
    h.get_by_label("Filter properties").rect().right() + 1.0
}

#[test]
fn a_staged_delete_is_seen_and_applied_where_it_started() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    let rev = h.state().shell().revision().unwrap();
    dispatch(&mut h, "navigator.delete", json!({ "kind": "pages" }));
    settle(&mut h);
    let waiting = waiting_text(h.state().shell()).expect("the card waits");
    assert!(waiting.ends_with("wait for Apply"), "{waiting}");
    // The Pages pane: the strip with its own Apply.
    let edge = left_column(&h);
    let strip = h
        .root()
        .children_recursive()
        .find(|n| {
            n.accesskit_node()
                .value()
                .is_some_and(|v| v.starts_with("Preview open: Delete"))
        })
        .expect("the strip in the Pages pane");
    assert!(strip.rect().left() < edge, "in the left column");
    h.get_all_by_label("Apply")
        .find(|n| n.rect().right() < edge)
        .expect("Apply in the Pages pane")
        .click();
    settle(&mut h);
    assert!(h.state().shell().properties().card().is_none(), "applied");
    assert_eq!(h.state().shell().revision(), Some(rev + 1), "one commit");

    // A second card, then the History tab put in front: the Preview tab keeps a dot and says why.
    dispatch(&mut h, "navigator.delete", json!({ "kind": "pages" }));
    dispatch(&mut h, "history.show", json!({ "on": true }));
    settle(&mut h);
    assert_ne!(
        h.state().shell().layout().shown_in_place_of(PaneKind::Preview),
        Some(PaneKind::Preview)
    );
    let waiting = waiting_text(h.state().shell()).unwrap();
    let tab = h
        .get_all_by_label("Preview")
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
        .expect("the Preview tab");
    assert_eq!(tab.accesskit_node().description().as_deref(), Some(waiting.as_str()));
    // The status bar's field brings the Preview forward.
    h.get_by_label(&waiting).click();
    settle(&mut h);
    assert_eq!(
        h.state().shell().layout().shown_in_place_of(PaneKind::Preview),
        Some(PaneKind::Preview)
    );
    assert!(h.state().shell().properties().card().is_some(), "still waiting");
    h.state_mut().shell_mut().shutdown();
}
