// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The creation dialogs in the window (kittest, no rendering): Page ▸ New page and the page
//! navigator's New page with their fields (Enter creates, Escape cancels, a refusal stays in the dialog), the Insert
//! entries for a structure node and an external reference, and the entries disabled without a project. Each walk
//! ends with the accessible-name walker (`settle`).

use crate::support;

use egui::Key;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_viewmodel::shell::Dialog;
use support::{Scripted, Tmp, harness, settle, wait_until};

/// The text field labelled `name` (the form fields have no placeholder; their label names them).
fn field<'a, 'h>(h: &'h egui_kittest::Harness<'a, ew_ui::app::App>, name: &str) -> egui_kittest::Node<'h> {
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no field {name}"))
}

/// Whether the open form shows a refusal.
fn form_refused(shell: &ew_viewmodel::shell::ShellModel) -> bool {
    matches!(shell.dialog(), Some(Dialog::Form(f)) if f.error.is_some())
}

fn form_values(h: &egui_kittest::Harness<'_, ew_ui::app::App>) -> Vec<(String, String)> {
    match h.state().shell().dialog() {
        Some(Dialog::Form(f)) => f
            .fields
            .iter()
            .map(|f| (f.name.to_owned(), f.value.clone()))
            .collect(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn page_new_asks_for_the_fields_enter_creates_and_escape_cancels() {
    let tmp = Tmp::new();
    let dir = tmp.sample("s.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    let before = h.state().shell().page_count();
    support::open_menu(&mut h, &["Page"]);
    settle(&mut h);
    h.get_by_label("New page…").click();
    settle(&mut h);
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::Form(f)) if f.action == "page.new"));
    // The navigator's button and the dialog's title.
    assert_eq!(h.query_all_by_label("New page").count(), 2);
    // The folio field holds the proposal for a multi-line page: the first free folio of the section that holds the
    // multi-line pages (the sample's section 1 holds 101 to 105), never one in the front matter that page 1 stands in.
    assert_eq!(form_values(&h)[0], ("folio".into(), "106".into()));
    // Folio 999 stands in no section: the section the proposal chose is cleared.
    h.state_mut().shell_mut().set_form_field("section", "");
    h.state_mut().shell_mut().set_form_field("folio", "");
    settle(&mut h);
    // The folio field has the focus when the dialog opens: type the folio; Tab moves on (to the type's list, then the
    // title), where the rest is typed.
    field(&h, "Folio").type_text("999");
    settle(&mut h);
    h.key_press(Key::Tab);
    h.key_press(Key::Tab);
    settle(&mut h);
    field(&h, "Title").type_text("Typed in the window");
    settle(&mut h);
    let values = form_values(&h);
    assert_eq!(values[0], ("folio".into(), "999".into()));
    assert_eq!(values[1], ("type".into(), "multi_line".into()));
    assert_eq!(values[2], ("title".into(), "Typed in the window".into()));
    h.key_press(Key::Enter);
    wait_until(&mut h, "the dialog's close", |s| s.dialog().is_none());
    assert_eq!(h.state().shell().page_count(), before + 1);
    assert_eq!(h.state().shell().current_folio(), Some("999"));
    // A second time Escape cancels and creates nothing.
    support::open_menu(&mut h, &["Page"]);
    settle(&mut h);
    h.get_by_label("New page…").click();
    settle(&mut h);
    h.state_mut().shell_mut().set_form_field("folio", "");
    settle(&mut h);
    field(&h, "Folio").type_text("998");
    settle(&mut h);
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none());
    assert_eq!(h.state().shell().page_count(), before + 1);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn a_refusal_stays_in_the_dialog_and_the_type_is_a_choice() {
    let tmp = Tmp::new();
    let dir = tmp.sample("s.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    let before = h.state().shell().page_count();
    // The page navigator's own New page opens the same dialog.
    h.get_by_label("New page").click();
    settle(&mut h);
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::Form(f)) if f.action == "page.new"));
    // The folio of an existing page: the command's refusal is shown under the fields, the dialog stays open.
    let existing = h.state().shell().page_labels()[0].clone();
    // Page 1 is in no section: the section the proposal chose is cleared.
    h.state_mut().shell_mut().set_form_field("section", "");
    h.state_mut().shell_mut().set_form_field("folio", "");
    settle(&mut h);
    field(&h, "Folio").type_text(&existing);
    settle(&mut h);
    h.get_by_label("Create").click();
    wait_until(&mut h, "the refusal", form_refused);
    assert_eq!(h.state().shell().page_count(), before);
    match h.state().shell().dialog() {
        Some(Dialog::Form(f)) => assert!(
            f.error
                .as_deref()
                .is_some_and(|e| e.contains("already; choose a free folio")),
            "{f:?}"
        ),
        other => panic!("{other:?}"),
    }
    assert!(
        h.query_by_label_contains("already; choose a free folio")
            .is_some()
    );
    assert!(
        h.state().shell().banner().is_none(),
        "the refusal is not made a banner as well"
    );
    // Choosing the type from the list.
    // The first list is Type (Connection grid follows it).
    crate::support::combos(&h)
        .into_iter()
        .next()
        .expect("the Type list")
        .click();
    settle(&mut h);
    h.get_by_label("Single-line").click();
    settle(&mut h);
    assert_eq!(form_values(&h)[1], ("type".into(), "single_line".into()));
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn the_insert_entries_open_the_structure_and_external_reference_dialogs() {
    let tmp = Tmp::new();
    let dir = tmp.sample("s.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    // Enclosure… makes the Project group four entries, a submenu (doc 16a §9.2).
    support::open_menu(&mut h, &["Project", "Setup"]);
    settle(&mut h);
    h.get_by_label("Structure node…").click();
    settle(&mut h);
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::Form(f)) if f.action == "structure.new"));
    field(&h, "Designation").type_text("J-E-999");
    settle(&mut h);
    h.key_press(Key::Enter);
    wait_until(&mut h, "the dialog's close", |s| s.dialog().is_none());
    support::open_menu(&mut h, &["Project", "Setup"]);
    settle(&mut h);
    h.get_by_label("External reference…").click();
    settle(&mut h);
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::Form(f)) if f.action == "external_ref.new"));
    field(&h, "Location (designation)").type_text("J-E-999");
    field(&h, "Device").click();
    field(&h, "Device").type_text("JE-N1000-99");
    settle(&mut h);
    h.key_press(Key::Enter);
    wait_until(&mut h, "the dialog's close", |s| s.dialog().is_none());
    assert!(h.state().shell().is_dirty());
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn the_new_entries_are_disabled_without_a_project() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    support::open_menu(&mut h, &["Page"]);
    settle(&mut h);
    assert!(
        h.get_by_label("New page…").accesskit_node().is_disabled(),
        "New page is disabled without a project"
    );
    h.key_press(Key::Escape);
    settle(&mut h);
    support::open_menu(&mut h, &["Project", "Setup"]);
    settle(&mut h);
    for entry in ["Structure node…", "External reference…"] {
        assert!(h.get_by_label(entry).accesskit_node().is_disabled(), "{entry}");
    }
}
