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

/// New project as a first-timer met it: Browse… that comes back with nothing leaves what was typed; Create's reason
/// stands over it for each thing missing or wrong; a folder typed without its whole path is read from the projects
/// folder; and the dialog's rows stay where they were drawn while it is filled in.
#[test]
fn new_project_says_why_create_waits_and_keeps_what_was_typed() {
    use ew_ui::app::Pick;
    let tmp = Tmp::new();
    let platform = Scripted::default();
    let mut h = harness_with(gui_shell(&tmp), platform.clone(), None);
    settle(&mut h);
    h.get_by_label("New project").click();
    settle(&mut h);
    let browse = h.get_by_label("Browse…").rect();
    let set = |h: &mut egui_kittest::Harness<'_, ew_ui::app::App>, params| {
        crate::support::dispatch(h, "new_project.set", params);
        settle(h);
    };
    let reason = |h: &egui_kittest::Harness<'_, ew_ui::app::App>, text: &str| {
        h.query_by_label_contains(text).is_some()
    };
    // No folder: Create waits and says why; the rows did not move for the line.
    set(&mut h, json!({"folder": ""}));
    assert!(reason(&h, "Choose the folder the project is made in"));
    assert!(h.get_by_label("Create").accesskit_node().is_disabled());
    assert_eq!(h.get_by_label("Browse…").rect(), browse, "the rows stay put");
    // No name.
    set(&mut h, json!({"folder": "pumpstation", "name": ""}));
    assert!(reason(&h, "Type a name for the project."));
    // Browse… that comes back with nothing keeps the folder typed.
    platform
        .answers
        .borrow_mut()
        .push_back(Pick::Path(std::path::PathBuf::new()));
    set(&mut h, json!({"name": "Pump station"}));
    h.get_by_label("Browse…").click();
    settle(&mut h);
    let Some(Dialog::NewProject(d)) = h.state().shell().dialog() else {
        panic!("the dialog stays open")
    };
    assert_eq!(d.folder, "pumpstation");
    // A folder typed without its whole path is read from the projects folder; one that is not there is refused.
    assert!(reason(&h, "That folder is not there"));
    let projects = tmp.0.join("config").join("projects");
    std::fs::create_dir_all(projects.join("pumpstation")).unwrap();
    set(&mut h, json!({"name": "Pump station"}));
    let target = projects.join("pumpstation").join("Pump station");
    assert!(reason(&h, &target.to_string_lossy()), "the line says where");
    assert!(!h.get_by_label("Create").accesskit_node().is_disabled());
    assert_eq!(h.get_by_label("Browse…").rect(), browse, "the rows stay put");
    h.get_by_label("Create").click();
    settle(&mut h);
    assert!(target.join("manifest.json").is_file());
    // A project of that name there already.
    crate::support::dispatch(&mut h, "file.new", json!({"discard": true}));
    settle(&mut h);
    set(&mut h, json!({"folder": "pumpstation", "name": "Pump station"}));
    assert!(reason(&h, "A folder of that name is there already"));
    h.state_mut().shell_mut().shutdown();
}
