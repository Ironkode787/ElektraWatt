// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shell frame without rendering (WP-S2.01 part 1, acceptance checks A10 and A11): the start page, the
//! command palette, page keys, the About dialog and the unsaved-changes dialog, each followed by the
//! accessible-name walker.

mod support;

use egui::{Key, Modifiers};
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::ew_viewmodel::shell::{Dialog, Outcome, Prompt};
use serde_json::json;
use support::{Scripted, Tmp, harness, settle, wait_for_page};

#[test]
fn the_start_page_offers_open_and_names_no_project() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    assert!(h.query_by_label("Open...").is_some());
    assert!(h.query_by_label("Update check: off").is_some());
    assert!(h.query_by_label("No project open").is_some());
    // The menu bar is there, in order, on every operating system.
    for menu in ["File", "Edit", "View", "Page", "Tools", "Help"] {
        assert!(h.query_by_label(menu).is_some(), "menu {menu}");
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
    let total = h.state().shell().page_count();
    let first = format!("1 (1 of {total})");
    assert!(h.query_by_label_contains(&first).is_some(), "{first}");
    assert!(h.query_by_label("Pages").is_some());
    assert!(h.query_by_label("saved").is_some());

    h.key_press(Key::PageDown);
    settle(&mut h);
    assert_eq!(h.state().shell().page_index(), 1);
    let second = format!("2 (2 of {total})");
    assert!(h.query_by_label_contains(&second).is_some(), "{second}");
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
fn a_click_in_the_page_list_goes_to_that_page() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    let label = h.state().shell().page_labels()[3].clone();
    h.get_by_label(&label).click();
    settle(&mut h);
    assert_eq!(h.state().shell().page_index(), 3);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn help_about_opens_and_closes() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    h.get_by_label("Help").click();
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
    platform.answers.borrow_mut().push_back(Some(dir));
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
        h.query_by_label_contains("*").is_some(),
        "the save state shows unsaved"
    );
    h.key_press_modifiers(Modifiers::COMMAND, Key::W);
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
    h.key_press_modifiers(Modifiers::COMMAND, Key::W);
    settle(&mut h);
    h.get_by_label("Discard").click();
    settle(&mut h);
    assert!(!h.state().shell().has_project());
    assert!(h.query_by_label("Open...").is_some(), "back on the start page");
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
    h.get_by_role(egui::Role::TextInput).type_text(&folio);
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
    h.get_by_label("File").click();
    settle(&mut h);
    // A menu entry is named by its label and its accelerator.
    let open = format!("Open... {}", ew_ui::keys::display("Ctrl+O"));
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
    h.get_by_label("View").click();
    settle(&mut h);
    h.get_by_label("UI scale").click();
    settle(&mut h);
    h.get_by_label("150 %").click();
    settle(&mut h);
    assert_eq!(h.state().shell().settings().ui_scale, 1.5);
    assert_eq!(h.ctx.zoom_factor(), 1.5);
}
