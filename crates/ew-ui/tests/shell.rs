// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shell frame without rendering (WP-S2.01 part 1, acceptance checks A10 and A11): the start page, the
//! command palette, page keys, the About dialog and the unsaved-changes dialog, each followed by the
//! accessible-name walker.

mod support;

use egui::{Key, Modifiers};
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::Pick;
use ew_ui::ew_viewmodel::shell::{Dialog, Outcome, Prompt};
use serde_json::json;
use support::{Scripted, Tmp, harness, settle, wait_for_page};

#[test]
fn the_start_page_offers_open_and_names_no_project() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    assert!(h.query_by_label("Open…").is_some());
    assert!(h.query_by_label("Update check: off").is_some());
    // WP-S2.17 part 3 (D12): the status bar's state field; the start page's hint left the bar.
    assert!(h.query_by_label("Ready").is_some());
    // The app menu is there on every operating system (WP-S2.17 part 2: one button opens the whole tree, in doc
    // 16a §9.2's order).
    support::open_menu(&mut h, &[]);
    let labels: Vec<String> = h
        .get_all_by_role(egui::Role::Button)
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    let menus = [
        "File", "Edit", "Project", "Page", "Draw", "Check", "Output", "View", "Window", "Help",
    ];
    let shown: Vec<&str> = labels
        .iter()
        .map(String::as_str)
        .filter(|l| menus.contains(l))
        .collect();
    for menu in menus {
        assert!(shown.contains(&menu), "menu {menu} in {labels:?}");
    }
    assert!(!h.state().shell().has_project());
}

#[test]
fn the_palette_lists_save_disabled_with_its_reason_and_escape_closes_it() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::K);
    settle(&mut h);
    assert!(h.state().shell().palette().is_some(), "Ctrl+K opens the palette");
    // The field has the keyboard focus: type into it.
    h.get_by_role(egui::Role::TextInput).type_text("save");
    settle(&mut h);
    let palette = h.state().shell().palette().unwrap().clone();
    assert_eq!(palette.query, "save");
    let entries = h.state().shell().palette_entries();
    let save = entries
        .iter()
        .find(|e| e.id == "file.save")
        .expect("Save is listed");
    assert!(!save.enabled);
    assert_eq!(save.reason.as_deref(), Some("No project is open"));
    assert!(h.query_all_by_label_contains("No project is open").count() > 0);
    // Escape closes it.
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(h.state().shell().palette().is_none());
}

#[test]
fn the_palette_runs_the_selected_entry_with_enter() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::K);
    settle(&mut h);
    h.get_by_role(egui::Role::TextInput).type_text("about");
    settle(&mut h);
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(h.state().shell().palette().is_none());
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::About)));
    assert!(h.query_by_label_contains("Version").is_some());
}

#[test]
fn page_down_changes_the_page_in_the_status_bar() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    assert!(
        h.state().shell().has_project(),
        "the initial project opens after the first frame"
    );
    wait_for_page(&mut h);
    settle(&mut h);
    // WP-S2.17 part 3 (D12): the page field left the status bar (the canvas tab and the page list show the page); the
    // save state is "Saved".
    assert!(h.query_all_by_label("Pages").next().is_some());
    assert!(h.query_by_label("Saved").is_some());

    h.key_press(Key::PageDown);
    settle(&mut h);
    assert_eq!(h.state().shell().page_index(), 1);
    h.key_press(Key::PageUp);
    settle(&mut h);
    assert_eq!(h.state().shell().page_index(), 0);
    // Page Up on the first page does nothing (the action is disabled).
    h.key_press(Key::PageUp);
    settle(&mut h);
    assert_eq!(h.state().shell().page_index(), 0);
    // The image of the page arrives from the render worker and is the one of the page on show.
    wait_for_page(&mut h);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn a_click_in_the_page_list_marks_and_enter_goes_to_that_page() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    // The navigator's row of the fourth page, by its accessible name (WP-S2.03 §6.1).
    let shell = h.state().shell();
    let rows = shell.page_rows().to_vec();
    let row = shell
        .navigator_rows()
        .into_iter()
        .find(|r| matches!(r.kind, ew_ui::ew_viewmodel::navigator::RowKind::Page(3)))
        .unwrap();
    let name =
        ew_ui::ew_viewmodel::navigator::PageNavigator::accessible_name(&row, &rows, shell.current_page());
    // WP-S2.06 §6.4: a click marks the row and moves the cursor without switching the page; Enter opens it.
    h.get_by_label(&name).click();
    settle(&mut h);
    assert_eq!(h.state().shell().page_index(), 0);
    assert!(h.state().shell().navigator().is_marked(&row.key));
    // Enter on the cursor row (the click put it there and focused the navigator) opens the page.
    h.key_press(egui::Key::Enter);
    settle(&mut h);
    assert_eq!(h.state().shell().page_index(), 3);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn help_about_opens_and_closes() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    support::open_menu(&mut h, &["Help"]);
    settle(&mut h);
    h.get_by_label("About ElektraWatt").click();
    settle(&mut h);
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::About)));
    assert!(h.query_by_label_contains("Version 0.0.0-test").is_some());
    assert!(h.query_by_label("Page view: CPU renderer").is_some());
    assert!(
        h.query_by_label("Update check: off. Crash reports: none")
            .is_some()
    );
    assert!(h.query_by_label("Licence: GPL-3.0-or-later").is_some());
    // The footer fits the small dialog (doc 16a §6.12): its buttons and the links lie inside the extent of the
    // title and the body, which share one left edge.
    let title = h.get_by_label("About ElektraWatt").rect();
    let body = h.get_by_label("Licence: GPL-3.0-or-later").rect();
    assert!((title.min.x - body.min.x).abs() < 1.0, "{title:?} {body:?}");
    for label in ["Close", "Documentation", "Third-party notices"] {
        let r = h.get_by_label(label).rect();
        assert!(
            r.min.x >= body.min.x - 1.0 && r.max.x <= body.min.x + 440.0,
            "{label} {r:?} outside the body from x {}",
            body.min.x
        );
    }
    h.get_by_label("Close").click();
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none());

    // Escape closes it as well.
    h.state_mut()
        .shell_mut()
        .dispatch("help.about", &serde_json::Value::Null);
    settle(&mut h);
    assert!(h.state().shell().dialog().is_some());
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none());
}

#[test]
fn open_asks_the_file_dialog_and_opens_the_answer() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Path(dir));
    let mut h = harness(&tmp, platform.clone(), None);
    settle(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::O);
    settle(&mut h);
    assert_eq!(platform.asked.borrow().as_slice(), [Prompt::OpenFolder]);
    assert!(h.state().shell().has_project());
    assert_eq!(h.state().shell().project_name(), Some("g.ew"));
    // A cancelled dialog changes nothing.
    h.state_mut()
        .shell_mut()
        .dispatch("file.close", &serde_json::Value::Null);
    settle(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::O);
    settle(&mut h);
    assert!(!h.state().shell().has_project());
    assert_eq!(platform.asked.borrow().len(), 2);
}

#[test]
fn closing_a_dirty_project_asks_and_discard_closes_it() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    let run = json!({"cmd": "page.create", "params": {"number": 999}});
    assert_eq!(
        h.state_mut().shell_mut().dispatch("command.run", &run),
        Outcome::Done
    );
    settle(&mut h);
    assert!(
        h.query_by_label("Unsaved changes").is_some(),
        "the save state shows unsaved"
    );
    // Close project is Ctrl+Shift+W: Ctrl+W closes a canvas tab (WP-S2.03 §6.2).
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::W);
    settle(&mut h);
    assert!(matches!(
        h.state().shell().dialog(),
        Some(Dialog::ConfirmDiscard { .. })
    ));
    assert!(h.state().shell().has_project());
    assert!(h.query_by_label("The project has unsaved changes.").is_some());
    h.get_by_label("Cancel").click();
    settle(&mut h);
    assert!(h.state().shell().has_project());
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::W);
    settle(&mut h);
    h.get_by_label("Don't save").click();
    settle(&mut h);
    assert!(!h.state().shell().has_project());
    assert!(h.query_by_label("Open…").is_some(), "back on the start page");
}

#[test]
fn go_to_page_takes_a_folio() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    let folio = h.state().shell().page_labels()[4].clone();
    h.key_press_modifiers(Modifiers::COMMAND, Key::G);
    settle(&mut h);
    assert!(matches!(
        h.state().shell().dialog(),
        Some(Dialog::GotoPage { .. })
    ));
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().data().placeholder() == Some("Folio or cell"))
        .expect("the go-to field")
        .type_text(&folio);
    settle(&mut h);
    h.get_by_label("Go").click();
    settle(&mut h);
    assert_eq!(h.state().shell().page_index(), 4);
    assert!(h.state().shell().dialog().is_none());
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn keys_do_not_leak_out_of_the_palette_or_a_text_field() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::K);
    settle(&mut h);
    // With the palette open, Page Down does not turn the page.
    h.key_press(Key::PageDown);
    settle(&mut h);
    assert_eq!(h.state().shell().page_index(), 0);
    // Ctrl+K closes the palette again.
    h.key_press_modifiers(Modifiers::COMMAND, Key::K);
    settle(&mut h);
    assert!(h.state().shell().palette().is_none());
    h.key_press(Key::PageDown);
    settle(&mut h);
    assert_eq!(h.state().shell().page_index(), 1);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn the_file_menu_greys_out_what_needs_a_project() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    support::open_menu(&mut h, &["File"]);
    settle(&mut h);
    // A menu entry is named by its label and its accelerator.
    let open = format!("Open… {}", ew_ui::keys::display("Ctrl+O"));
    assert!(!h.get_by_label(&open).accesskit_node().is_disabled());
    let save = format!("Save {}", ew_ui::keys::display("Ctrl+S"));
    assert!(
        h.get_by_label(&save).accesskit_node().is_disabled(),
        "Save is disabled without a project"
    );
}

#[test]
fn the_ui_scale_setting_scales_the_frame() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    assert_eq!(h.ctx.zoom_factor(), 1.0);
    support::open_menu(&mut h, &["View", "UI scale"]);

    settle(&mut h);
    h.get_by_label("150 %").click();
    settle(&mut h);
    assert_eq!(h.state().shell().settings().ui_scale, 1.5);
    assert_eq!(h.ctx.zoom_factor(), 1.5);
}

#[test]
fn escape_does_not_dismiss_the_banner_and_its_button_does() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    h.state_mut().shell_mut().show_banner("Something to read");
    settle(&mut h);
    assert!(h.query_by_label("Something to read").is_some());
    // With nothing else open Escape does nothing (NFR-UX-07).
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(h.state().shell().banner().is_some(), "the banner stays");
    h.get_by_label("Dismiss").click();
    settle(&mut h);
    assert!(h.state().shell().banner().is_none());
}

#[test]
fn a_missing_file_dialog_is_named_in_a_banner() {
    let tmp = Tmp::new();
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Unavailable);
    let mut h = harness(&tmp, platform, None);
    settle(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::O);
    settle(&mut h);
    let banner = h.state().shell().banner().expect("a banner").to_owned();
    assert!(banner.contains("xdg-desktop-portal"), "{banner}");
    assert!(!h.state().shell().has_project());
}

#[test]
fn the_zoom_is_set_when_the_setting_changes_not_every_frame() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    // egui's own zoom keys change the factor; the next frames must not reset it.
    h.ctx.set_zoom_factor(1.2);
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.ctx.zoom_factor(), 1.2);
    // A change of the setting applies once.
    h.state_mut()
        .shell_mut()
        .dispatch("view.ui_scale", &json!({"scale": 1.5}));
    settle(&mut h);
    assert_eq!(h.ctx.zoom_factor(), 1.5);
}

// WP-S2.09 part 1: the one-frame "Exporting PDF…" label went with the export job; `tests/jobs.rs` holds the
// window's export (the save dialog, the job, its progress and its end).
