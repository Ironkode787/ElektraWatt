// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! New project, the templates and the template wizard by the window's own path, as a persona round met them: a
//! template renamed in its list, Browse… that comes back with nothing, Create's reason beside it, the wizard's folder
//! and its Close.

use crate::support::{Scripted, Tmp, harness_with, settle};
use egui::{Key, Modifiers};
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ew_ui::ew_viewmodel::shell::{Dialog, Mode, ShellModel};
use serde_json::json;

fn gui_shell(tmp: &Tmp) -> ShellModel {
    ShellModel::new(Mode::Gui, tmp.config_with_resources())
}

/// A project folder `name` in the test's folder, made and closed, for the templates' list.
fn template_folder(shell: &mut ShellModel, tmp: &Tmp, name: &str) -> String {
    let dir = tmp.0.join(name).to_string_lossy().into_owned();
    crate::support::done(shell, "file.new", json!({"dir": dir, "template": "builtin"}));
    crate::support::done(shell, "file.close", json!({}));
    dir
}

/// A name typed into the selected template's name field and saved with Save name shows in the list, in New project's
/// Start from and after the program starts again; a name another template has is refused in words.
#[test]
fn a_template_renamed_in_its_list_keeps_the_name_everywhere() {
    let tmp = Tmp::new();
    let mut shell = gui_shell(&tmp);
    let first = template_folder(&mut shell, &tmp, "First job");
    let second = template_folder(&mut shell, &tmp, "Second job");
    crate::support::done(&mut shell, "templates.add", json!({"dir": first}));
    crate::support::done(&mut shell, "templates.add", json!({"dir": second}));
    let mut h = harness_with(shell, Scripted::default(), None);
    settle(&mut h);
    h.get_by_label("Templates…").click();
    settle(&mut h);
    // The second row selected, its Rename, a new name typed, Save name.
    h.get_by_role_and_label(egui::Role::ListItem, "Second job").click();
    settle(&mut h);
    h.get_by_label("Rename").click();
    settle(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run_ok();
    h.get_by_role(egui::Role::TextInput).type_text("Spare starter");
    h.run_ok();
    h.get_by_label("Save name").click();
    settle(&mut h);
    assert!(
        h.query_by_role_and_label(egui::Role::ListItem, "Spare starter").is_some(),
        "the list shows the new name"
    );
    assert!(h.query_all_by_label("Second job").next().is_none());
    // A name the other template has is refused in words, and the list keeps both names.
    h.get_by_label("Rename").click();
    settle(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run_ok();
    h.get_by_role(egui::Role::TextInput).type_text("First job");
    h.run_ok();
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(
        h.query_by_label_contains("Another template is called \"First job\" already")
            .is_some()
    );
    let names: Vec<String> = h
        .state()
        .shell()
        .settings()
        .templates
        .iter()
        .map(|t| t.name.clone())
        .collect();
    assert_eq!(names, ["First job", "Spare starter"]);
    // The program started again: the settings keep the name, and New project lists it.
    let mut again = gui_shell(&tmp);
    crate::support::done(&mut again, "file.new", json!({}));
    let Some(Dialog::NewProject(d)) = again.dialog() else {
        panic!("{:?}", again.dialog())
    };
    assert!(d.starts.iter().any(|l| l.name == "Spare starter"), "{:?}", d.starts);
}

/// The wizard's step of wire sizes is named by what it sets, and an installation without example pages shows no
/// Example column that only says it has none.
#[test]
fn the_wizard_names_its_wire_step_and_shows_no_empty_example() {
    let tmp = Tmp::new();
    for (shell, example) in [
        (ShellModel::new(Mode::Gui, tmp.config()), false),
        (gui_shell(&tmp), true),
    ] {
        let mut h = harness_with(shell, Scripted::default(), None);
        settle(&mut h);
        crate::support::dispatch(&mut h, "tools.company_standard", json!({}));
        settle(&mut h);
        assert!(h.query_by_label("4. Wire cross-sections").is_some());
        assert_eq!(h.query_by_label("Cover page").is_some(), example);
        assert!(
            h.query_by_label_contains("do not come with this installation")
                .is_none()
        );
        crate::support::dispatch(&mut h, "wizard.cancel", json!({}));
    }
}
