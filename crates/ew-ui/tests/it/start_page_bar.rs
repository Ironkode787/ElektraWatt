// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! With no project open the command bar showed the Draw tab as if one could draw. On the start page it shows the
//! Project tab; the tabs that need a project are there, greyed with the reason, and a click on one says it.

use crate::support;

use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_viewmodel::command_bar::Tab;
use serde_json::json;
use support::{Scripted, Tmp, dispatch, harness, settle};

#[test]
fn the_start_pages_bar_shows_the_project_tab_and_greys_the_tabs_that_need_a_project() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    assert_eq!(h.state().shell().command_tab(), Tab::Project);
    let tab = |h: &egui_kittest::Harness<'_, ew_ui::app::App>, label: &str| {
        h.get_all_by_role(egui::accesskit::Role::Tab)
            .find(|n| n.accesskit_node().label().as_deref() == Some(label))
            .unwrap_or_else(|| panic!("the {label} tab"))
            .accesskit_node()
            .is_disabled()
    };
    assert!(tab(&h, "Draw"), "Draw is greyed");
    assert!(
        !tab(&h, "Project") && !tab(&h, "View"),
        "Project and View work without a project"
    );
    h.get_all_by_role(egui::accesskit::Role::Tab)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Draw"))
        .unwrap()
        .click();
    settle(&mut h);
    let s = h.state().shell();
    assert_eq!(s.command_tab(), Tab::Project, "a greyed tab is not chosen");
    assert_eq!(
        s.toast().map(|t| t.text.as_str()),
        Some("Start a new project or open one first: New project or Open… on the start page")
    );
    // With a project the tabs work, and Draw is the tab that opens with it.
    dispatch(
        &mut h,
        "file.new",
        json!({"dir": tmp.0.join("p.ew"), "template": "builtin"}),
    );
    settle(&mut h);
    assert_eq!(h.state().shell().command_tab(), Tab::Draw);
    assert!(!tab(&h, "Draw"));
    h.state_mut().shell_mut().shutdown();
}
