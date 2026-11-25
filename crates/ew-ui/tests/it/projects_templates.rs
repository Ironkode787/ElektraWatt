// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! New project, the templates and the template wizard by the window's own path: a
//! template renamed in its list, Browse… that comes back with nothing, Create's reason beside it, the wizard's folder
//! and its Close.

use crate::support::{Scripted, Tmp, harness_with, settle};
use egui::{Key, Modifiers};
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ew_viewmodel::shell::{Dialog, Mode, ShellModel};
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
    h.get_by_role_and_label(egui::Role::ListItem, "Second job")
        .click();
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
        h.query_by_role_and_label(egui::Role::ListItem, "Spare starter")
            .is_some(),
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
    assert!(
        d.starts.iter().any(|l| l.name == "Spare starter"),
        "{:?}",
        d.starts
    );
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

/// New project: Browse… that comes back with nothing leaves what was typed; Create's reason
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
    // A folder typed without its whole path is read from the projects folder. One that is not there yet is made by
    // Create, and the line over Create says so; a file of that name in
    // its way is refused in words.
    assert!(reason(&h, "This folder is not there yet: Create makes it."));
    assert!(!h.get_by_label("Create").accesskit_node().is_disabled());
    let projects = tmp.0.join("config").join("projects");
    std::fs::create_dir_all(&projects).unwrap();
    std::fs::write(projects.join("pumpstation"), b"").unwrap();
    set(&mut h, json!({"name": "Pump station"}));
    assert!(reason(&h, "A file of that name is in the folder's way"));
    assert!(h.get_by_label("Create").accesskit_node().is_disabled());
    std::fs::remove_file(projects.join("pumpstation")).unwrap();
    let folder = format!("pumpstation{}stage 1", std::path::MAIN_SEPARATOR);
    set(&mut h, json!({"folder": folder}));
    assert!(reason(&h, "This folder is not there yet: Create makes it."));
    let target = projects.join("pumpstation").join("stage 1").join("Pump station");
    assert!(reason(&h, &target.to_string_lossy()), "the line says where");
    assert!(!h.get_by_label("Create").accesskit_node().is_disabled());
    // No "the rows stay put" here: this line names the folder, and whether it takes one line or two follows from the
    // length of the temporary folder's path, which holds the process's number. The line without a path, above, is
    // the one that shows the room is kept.
    h.get_by_label("Create").click();
    settle(&mut h);
    assert!(
        target.join("manifest.json").is_file(),
        "Create made the folders on the way"
    );
    // A project of that name there already.
    crate::support::dispatch(&mut h, "file.new", json!({"discard": true}));
    settle(&mut h);
    set(&mut h, json!({"folder": folder, "name": "Pump station"}));
    assert!(reason(&h, "A folder of that name is there already"));
    h.state_mut().shell_mut().shutdown();
}

/// Close with answers in the wizard asks before it goes:
/// Back to the wizard stays, Close keeps the answers for the next time, and Close and drop my answers drops them.
#[test]
fn closing_the_wizard_with_answers_asks_and_keeps_them() {
    let tmp = Tmp::new();
    let draft = tmp.0.join("config").join("wizard-draft.json");
    let mut h = harness_with(gui_shell(&tmp), Scripted::default(), None);
    settle(&mut h);
    let open_with_answer = |h: &mut egui_kittest::Harness<'_, ew_ui::app::App>| {
        crate::support::dispatch(h, "tools.company_standard", json!({}));
        crate::support::dispatch(
            h,
            "wizard.set",
            json!({"field": "company_name", "value": "Example Panels"}),
        );
        settle(h);
    };
    open_with_answer(&mut h);
    h.get_by_label("Close").click();
    settle(&mut h);
    assert!(h.state().shell().wizard().is_some(), "Close asks first");
    assert!(h.query_by_label_contains("Your answers are kept").is_some());
    h.get_by_label("Back to the wizard").click();
    settle(&mut h);
    assert!(h.state().shell().wizard().is_some_and(|w| !w.closing()));
    h.get_by_label("Close").click();
    settle(&mut h);
    h.get_by_label("Close").click();
    settle(&mut h);
    assert!(h.state().shell().wizard().is_none());
    assert!(draft.is_file(), "the answers are kept");
    crate::support::dispatch(&mut h, "tools.company_standard", json!({}));
    settle(&mut h);
    assert_eq!(
        h.state().shell().wizard().unwrap().draft().get("company_name"),
        "Example Panels"
    );
    h.get_by_label("Close").click();
    settle(&mut h);
    h.get_by_label("Close and drop my answers").click();
    settle(&mut h);
    assert!(h.state().shell().wizard().is_none());
    assert!(!draft.exists(), "the answers are dropped");
}

/// Sections of a running project: a page put in a section that is not there is refused
/// with a sentence that says how to make it; New section… makes it; its row in the Pages list and its link in Project
/// properties open Edit section…, which renames it and changes its folio range in one step.
#[test]
fn a_running_projects_sections_are_made_renamed_and_ranged() {
    let tmp = Tmp::new();
    let mut shell = gui_shell(&tmp);
    let dir = tmp.0.join("job").to_string_lossy().into_owned();
    crate::support::done(&mut shell, "file.new", json!({"dir": dir, "template": "builtin"}));
    let refused = shell.dispatch(
        "page.new",
        &json!({"folio": "301", "type": "multi_line", "section": "3"}),
    );
    let ew_viewmodel::shell::Outcome::Failed { message } = refused else {
        panic!("{refused:?}")
    };
    assert!(
        message.starts_with("The project has no section 3 yet: make it with New section…"),
        "{message}"
    );
    assert!(!message.contains("section:"), "{message}");
    crate::support::done(
        &mut shell,
        "section.new",
        json!({"number": "3", "title": "Cabinet", "first": "300", "last": "399"}),
    );
    let mut h = harness_with(shell, Scripted::default(), None);
    settle(&mut h);
    // The Pages list's section row: its menu edits it.
    h.get_by_label("Section 3 - Cabinet, section").click_secondary();
    settle(&mut h);
    h.get_by_label("Edit section…").click();
    settle(&mut h);
    let fields = |h: &egui_kittest::Harness<'_, ew_ui::app::App>| match h.state().shell().dialog() {
        Some(Dialog::Form(f)) => {
            assert_eq!(f.action, "section.edit");
            f.fields
                .iter()
                .map(|x| (x.name, x.value.clone()))
                .collect::<Vec<_>>()
        }
        other => panic!("{other:?}"),
    };
    assert_eq!(
        fields(&h),
        [
            ("title", "Cabinet".to_owned()),
            ("first", "300".to_owned()),
            ("last", "399".to_owned()),
            ("number", "3".to_owned())
        ]
    );
    // A range that runs backwards is refused in words; a good one is applied with the new title.
    let apply = |h: &mut egui_kittest::Harness<'_, ew_ui::app::App>, title: &str, first: &str, last: &str| {
        h.state_mut().shell_mut().dispatch(
            "section.edit",
            &json!({"number": "3", "title": title, "first": first, "last": last}),
        )
    };
    let _ = apply(&mut h, "Cabinet layout", "350", "320");
    settle(&mut h);
    assert!(h.query_by_label_contains("comes after the last").is_some());
    assert_eq!(
        apply(&mut h, "Cabinet layout", "300", "349"),
        ew_viewmodel::shell::Outcome::Done
    );
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none());
    assert_eq!(
        h.state().shell().project_sections(),
        [(3, "Cabinet layout".to_owned())]
    );
    // Project properties link the section, and its Edit section… shows the new range.
    crate::support::dispatch(&mut h, "props.show", json!({"on": true}));
    crate::support::dispatch(&mut h, "props.project", json!({}));
    settle(&mut h);
    h.get_by_label("3 Cabinet layout").click();
    settle(&mut h);
    assert!(fields(&h).contains(&("last", "349".to_owned())));
    h.state_mut().shell_mut().shutdown();
}
