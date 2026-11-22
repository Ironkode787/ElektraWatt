// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A finding on a cable opens the page of its cores (FT-1924): the sample's supply cable -101W1 with its length
//! cleared (EW-CORE-033) names a page, and Enter on its Messages row opens it with the cable's core wires selected.

use ew_api::output::{NavKind, NavRole};

#[test]
fn a_cable_finding_opens_the_page_of_its_cores() {
    let s = crate::messages::with_a_cable_without_length();
    let t = s.navigator_table(NavKind::Messages);
    let row = t
        .nodes
        .iter()
        .find(|n| n.role == NavRole::Finding && n.label.contains("-101W1"))
        .expect("the cable's EW-CORE-033 row");
    let open = row.open.as_ref().expect("Enter opens a page");
    assert!(!open.select.is_empty(), "its core wires are selected");
    // The wires selected are the drawables on the page opened.
    for id in &open.select {
        assert!(
            row.drawables
                .iter()
                .any(|d| d.record == *id && d.page == open.page),
            "{id} is drawn on the page opened"
        );
    }
}
