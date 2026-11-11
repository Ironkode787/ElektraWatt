// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Properties explains itself (the owner's hands-on test of 0.1.0, FT-1648 to FT-1651), through the window: a
//! category folded by a click on its header and by Enter on it.

use crate::support;

use egui::Key;
use egui_kittest::kittest::{NodeT as _, Queryable};
use serde_json::json;
use support::{Tmp, dispatch, settle};

/// -101F1's symbol on folio 101.
const F1_SYMBOL: &str = "7b76f7a3-8966-50ed-a194-36d38ac1c0a1";

/// FT-1650: a selected symbol shows its device's, its function's and its representation's sections. A click on the
/// representation's Placement header folds that category (its rows go, its chevron turns), a second click unfolds it,
/// and Enter on the focused header folds it again; the fold is kept when the selection changes and comes back.
#[test]
fn a_click_on_a_category_header_folds_it() {
    let tmp = Tmp::new();
    let mut h = support::on_golden(&tmp, "101");
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    dispatch(&mut h, "props.show", json!({"on": true}));
    dispatch(&mut h, "select.set", json!({"ids": [F1_SYMBOL]}));
    settle(&mut h);
    let collapsed = |h: &egui_kittest::Harness<'_, ew_ui::app::App>| {
        h.state()
            .shell()
            .properties_collapsed()
            .into_iter()
            .collect::<Vec<_>>()
    };
    let headers = |h: &egui_kittest::Harness<'_, ew_ui::app::App>| {
        h.get_all_by_role(egui::Role::Button)
            .filter(|n| n.accesskit_node().label().as_deref() == Some("Placement"))
            .count()
    };
    // The device's section has a Placement category too, above the representation's.
    assert_eq!(headers(&h), 2);
    assert!(collapsed(&h).is_empty());
    last_placement(&h).scroll_to_me();
    settle(&mut h);
    last_placement(&h).click();
    settle(&mut h);
    assert_eq!(collapsed(&h), ["representation/placement"]);
    assert!(!drawn(&h, "Pos"), "the folded rows go");
    last_placement(&h).click();
    settle(&mut h);
    assert!(collapsed(&h).is_empty());
    assert!(drawn(&h, "Pos"));
    // From the keyboard: Enter and Space on the focused header.
    last_placement(&h).focus();
    settle(&mut h);
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(collapsed(&h), ["representation/placement"]);
    h.key_press(Key::Space);
    settle(&mut h);
    assert!(collapsed(&h).is_empty());
    h.key_press(Key::Space);
    settle(&mut h);
    // Another selection and back: the fold stays.
    dispatch(&mut h, "select.set", json!({"ids": []}));
    dispatch(&mut h, "select.set", json!({"ids": [F1_SYMBOL]}));
    settle(&mut h);
    assert_eq!(collapsed(&h), ["representation/placement"]);
    assert!(!drawn(&h, "Pos"));
    h.state_mut().shell_mut().shutdown();
}

/// Whether the grid drew the row labelled `label` (its named `Group` node).
fn drawn(h: &egui_kittest::Harness<'_, ew_ui::app::App>, label: &str) -> bool {
    h.get_all_by_role(egui::Role::Group)
        .any(|n| n.accesskit_node().label().as_deref() == Some(label))
}

/// The last Placement header of the grid in layout order: the representation's.
fn last_placement<'h>(h: &'h egui_kittest::Harness<'_, ew_ui::app::App>) -> egui_kittest::Node<'h> {
    let mut all: Vec<_> = h
        .get_all_by_role(egui::Role::Button)
        .filter(|n| n.accesskit_node().label().as_deref() == Some("Placement"))
        .collect();
    all.sort_by(|a, b| a.rect().top().total_cmp(&b.rect().top()));
    all.pop().expect("a Placement header")
}
