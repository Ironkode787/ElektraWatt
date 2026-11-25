// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A button that is not greyed but whose action is refused said nothing. A refusal is said where the person looks: in
//! a toast and the status line, in an open dialog beside its
//! buttons.

use crate::support;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_viewmodel::actions::ACTIONS;
use ew_viewmodel::shell::{Outcome, ShellModel};
use ew_viewmodel::{key, strings};
use serde_json::json;
use support::{Scripted, Tmp, bar_button, dispatch, harness, settle};

/// The window on a new project from the built-in template.
fn new_project(tmp: &Tmp) -> Harness<'static, App> {
    let mut h = harness(tmp, Scripted::default(), None);
    settle(&mut h);
    dispatch(
        &mut h,
        "file.new",
        json!({"dir": tmp.0.join("p.ew"), "template": "builtin"}),
    );
    settle(&mut h);
    h
}

/// The status bar's state field, at its left end.
fn status_line(shell: &ShellModel) -> String {
    ew_viewmodel::status::bar(shell, false)[0].text.clone()
}

/// The texts of the toasts on screen.
fn toasts(shell: &ShellModel) -> Vec<String> {
    shell
        .notifications()
        .visible()
        .iter()
        .map(|n| n.text.clone())
        .collect()
}

/// Rotate on the Draw tab is not greyed on a schematic page; with nothing selected its action is refused. The click
/// says why in a toast and in the status line.
#[test]
fn a_refused_bar_button_says_why_in_a_toast_and_the_status_line() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    assert!(h.state().shell().selection().is_empty());
    let rotate = bar_button(&h, "Rotate");
    assert!(!rotate.accesskit_node().is_disabled(), "Rotate is not greyed");
    rotate.click();
    settle(&mut h);
    let s = h.state().shell();
    assert_eq!(toasts(s), ["Nothing is selected"]);
    assert_eq!(status_line(s), "Nothing is selected");
    h.state_mut().shell_mut().shutdown();
}

/// In an open dialog a refused action is said beside its buttons, not behind the dialog.
#[test]
fn a_refusal_in_a_dialog_stands_beside_its_buttons() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    dispatch(&mut h, "help.about", json!({}));
    settle(&mut h);
    h.state_mut().dispatch_from_window("tool.rotate_cw", &json!({}));
    settle(&mut h);
    assert!(toasts(h.state().shell()).is_empty(), "no toast behind the dialog");
    assert!(
        h.query_by_label("Nothing is selected").is_some(),
        "the dialog says why"
    );
    h.state_mut().shell_mut().shutdown();
}

/// Every action the command bar and the menus hold: refused, its reason reaches the window. Walked on the start page
/// and on a new project with nothing selected, each action that cannot run there.
#[test]
fn a_refusal_of_every_bar_and_menu_action_reaches_the_window() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    let mut walked = 0;
    for project in [false, true] {
        if project {
            dispatch(
                &mut h,
                "file.new",
                json!({"dir": tmp.0.join("p.ew"), "template": "builtin"}),
            );
            settle(&mut h);
        }
        for a in ACTIONS
            .iter()
            .filter(|a| a.placement.is_some() || a.also.is_some() || a.menu.is_some())
        {
            let Err(reason) = (a.enabled)(h.state().shell()) else {
                continue;
            };
            h.state_mut().dispatch_from_window(a.id, &json!({}));
            let s = h.state().shell();
            assert_eq!(
                s.refusal_shown(),
                Some(strings::render(&reason).as_str()),
                "{} (project open: {project})",
                a.id
            );
            walked += 1;
        }
    }
    assert!(walked > 0);
    assert_eq!(
        h.state_mut().shell_mut().dispatch("tool.rotate_cw", &json!({})),
        Outcome::refused(key!("reason-nothing-selected")),
        "the project is still open with nothing selected"
    );
    h.state_mut().shell_mut().shutdown();
}
