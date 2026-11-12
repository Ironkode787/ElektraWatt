// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1686: the title block's fields of an imported QElectroTech project, edited in the window. The guide's sample
//! `lamp-circuit.qet` is imported through File ▸ Import…; Properties of its page links Title block fields…, whose
//! form lists the fields the page's title block prints from the project's data; typed values are written in one
//! step and the page prints them at once.

use crate::support;

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::{App, Pick};
use ew_ui::ew_viewmodel::shell::Dialog;
use support::{Scripted, Tmp, harness, settle};

/// The text field labelled `name`.
fn field<'a, 'h>(h: &'h Harness<'a, App>, name: &str) -> egui_kittest::Node<'h> {
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no field {name}"))
}

/// The text the focused page's title block prints for `path` now, as the form shows it when it opens.
fn printed(h: &mut Harness<'_, App>, path: &str) -> String {
    support::dispatch(h, "page.title_block", serde_json::json!({}));
    let text = match h.state().shell().dialog() {
        Some(Dialog::Form(f)) => f
            .fields
            .iter()
            .find(|f| f.name == path)
            .map(|f| f.value.clone())
            .unwrap_or_else(|| panic!("the title block prints no {path}")),
        other => panic!("the form: {other:?}"),
    };
    support::dispatch(h, "ui.dismiss", serde_json::json!({}));
    text
}

#[test]
fn the_title_block_fields_of_an_imported_page_are_edited_in_one_form() {
    let tmp = Tmp::new();
    let qet = tmp.0.join("lamp-circuit.qet");
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/book/src/tutorials/files/lamp-circuit.qet"
        ),
        &qet,
    )
    .unwrap();
    let platform = Scripted::default();
    platform
        .answers
        .borrow_mut()
        .extend([Pick::Path(qet), Pick::Path(tmp.0.join("lamp.ew"))]);
    let mut h = harness(&tmp, platform, None);
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    support::open_menu(&mut h, &["File"]);
    support::menu_node(&h, "Import…").click();
    settle(&mut h);
    assert!(h.state_mut().shell_mut().wait_jobs().is_empty());
    settle(&mut h);
    h.get_by_label("Create project").click();
    settle(&mut h);
    support::wait_for_page(&mut h);
    settle(&mut h);
    assert_eq!(printed(&mut h, "project.drawing_id"), "");
    // Properties of the page links the form; it lists the fields the title block prints, the description first.
    h.get_by_label("Title block fields…").click();
    settle(&mut h);
    let Some(Dialog::Form(form)) = h.state().shell().dialog() else {
        panic!("the form: {:?}", h.state().shell().dialog());
    };
    let names: Vec<&str> = form.fields.iter().map(|f| f.name).collect();
    for path in [
        "page.title",
        "project.reference",
        "page.description",
        "project.drawing_id",
        "revision.current.label",
        "party.end_user.name",
        "project.object",
    ] {
        assert!(names.contains(&path), "{path} in {names:?}");
    }
    for (label, text) in [
        ("Drawing ID", "D-0001"),
        ("End user", "Plant operator"),
        ("Revision", "V1.0"),
    ] {
        field(&h, label).click();
        settle(&mut h);
        field(&h, label).type_text(text);
        settle(&mut h);
    }
    let Some(Dialog::Form(form)) = h.state().shell().dialog() else {
        panic!("the form");
    };
    let typed: Vec<(&str, &str)> = form
        .fields
        .iter()
        .filter(|f| ["project.drawing_id", "party.end_user.name", "revision.current.label"].contains(&f.name))
        .map(|f| (f.name, f.value.as_str()))
        .collect();
    assert_eq!(
        typed,
        [
            ("project.drawing_id", "D-0001"),
            ("revision.current.label", "V1.0"),
            ("party.end_user.name", "Plant operator"),
        ]
    );
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none(), "{:?}", h.state().shell().dialog());
    // The page prints the new values at once.
    assert_eq!(printed(&mut h, "project.drawing_id"), "D-0001");
    assert_eq!(printed(&mut h, "party.end_user.name"), "Plant operator");
    assert_eq!(printed(&mut h, "revision.current.label"), "V1.0");
    // The fields' step is one undo step (the revision its own).
    let shell = h.state_mut().shell_mut();
    assert_eq!(
        shell.recorded_commands().iter().rev().take(2).map(|c| c.cmd.as_str()).collect::<Vec<_>>(),
        ["revision.create", "fields.set_many"]
    );
    shell.shutdown();
}
