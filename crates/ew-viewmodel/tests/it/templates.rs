// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! New project, the templates and the check level in the window (G1, G2 of the owner's test of 0.3.0-rc.2): a new
//! project takes a template's pages only when asked, the templates are a list the person manages, and the check level
//! is set by a person in the window.

use std::path::Path;

use ew_viewmodel::dialogs::{DialogAnswer, spec};
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellModel};
use serde_json::json;

use crate::common::{GOLDEN_A, Tmp, copy_dir, done};

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Presses the open dialog's Primary, as the window does.
fn press_primary(shell: &mut ShellModel) -> Outcome {
    let d = shell.dialog().expect("a dialog").clone();
    let s = spec(&d, shell);
    assert_eq!(s.primary.disabled, None, "{} can be pressed", s.primary.label);
    match s.primary.answer {
        DialogAnswer::Action(action, params) => shell.dispatch(action, &params),
        other => panic!("{other:?}"),
    }
}

/// New project in the window: the dialog, its name and folder, the start line `start` (1-based), the pages box, and
/// Create.
fn new_project(shell: &mut ShellModel, folder: &Path, name: &str, start: usize, pages: bool) {
    done(shell, "file.new", json!({}));
    assert!(matches!(shell.dialog(), Some(Dialog::NewProject(_))));
    done(
        shell,
        "new_project.set",
        json!({"name": name, "folder": path(folder), "start": start, "pages": pages}),
    );
    assert_eq!(press_primary(shell), Outcome::Done);
    assert_eq!(shell.dialog(), None, "the dialog closes once the project is made");
}

/// The owner made a template of an old project and found its pages in every new project: now a new project takes a
/// template's pages, and the devices on them, only when Also copy its pages is ticked; Empty project is empty; the
/// template used for new projects is the start the dialog opens on, and every start says what it brings.
#[test]
fn a_template_with_pages_gives_a_project_without_them_unless_ticked() {
    let tmp = Tmp::new("templates-pages");
    let old = tmp.0.join("old.ew");
    copy_dir(Path::new(GOLDEN_A), &old);
    let mut shell = tmp.empty_shell(Mode::Gui);
    done(&mut shell, "templates.add", json!({"dir": path(&old)}));
    done(&mut shell, "templates.use", json!({"index": 1}));
    done(&mut shell, "ui.dismiss", json!({}));

    done(&mut shell, "file.new", json!({}));
    let text = shell.describe();
    assert!(
        text.contains("start 2 (chosen) (for new projects) \"old.ew\""),
        "the template for new projects is chosen: {text}"
    );
    assert!(
        text.contains("start 1 \"Empty project\": \"One empty page"),
        "{text}"
    );
    assert!(text.contains("pages"), "the template says it has pages: {text}");
    assert!(
        text.lines()
            .any(|l| l.contains("check \"Also copy its") && l.ends_with(": off")),
        "the pages box is offered and off: {text}"
    );
    done(&mut shell, "ui.dismiss", json!({}));

    let drawn = |shell: &ShellModel| shell.page_labels().iter().any(|l| l == "102");
    new_project(&mut shell, &tmp.0, "without", 2, false);
    assert!(
        !drawn(&shell),
        "no page of the template: {:?}",
        shell.page_labels()
    );
    assert_eq!(shell.page_labels().first().map(String::as_str), Some("1"));
    done(&mut shell, "file.close", json!({}));

    new_project(&mut shell, &tmp.0, "with", 2, true);
    assert!(drawn(&shell), "the pages, ticked: {:?}", shell.page_labels());
    done(&mut shell, "file.close", json!({}));

    new_project(&mut shell, &tmp.0, "empty", 1, true);
    assert_eq!(shell.page_labels(), ["1"], "Empty project is one empty page");
    shell.shutdown();
}

/// Create says why it cannot run: no name, a name no folder can have, a folder that is taken.
#[test]
fn create_says_why_it_cannot_run() {
    let tmp = Tmp::new("templates-create");
    std::fs::create_dir_all(tmp.0.join("taken")).unwrap();
    std::fs::write(tmp.0.join("taken").join("notes.txt"), "mine").unwrap();
    let mut shell = tmp.empty_shell(Mode::Gui);
    done(&mut shell, "file.new", json!({}));
    let reason = |shell: &mut ShellModel, name: &str| {
        done(
            shell,
            "new_project.set",
            json!({"name": name, "folder": path(&tmp.0)}),
        );
        let d = shell.dialog().unwrap().clone();
        spec(&d, shell).primary.disabled
    };
    for (name, words) in [
        ("", "Type a name"),
        ("a:b", "cannot hold"),
        ("taken", "is there already"),
    ] {
        let why = reason(&mut shell, name).unwrap_or_default();
        assert!(why.contains(words), "{name:?}: {why}");
    }
    assert_eq!(reason(&mut shell, "free"), None);
    shell.shutdown();
}

/// P2 to P6: the wizard's template joins the list and is used for new projects when its last step says so; Templates…
/// uses it or not, renames it and removes it from the list (its folder stays); Save as template… makes one of the open
/// project; an open template says so in the title bar, and it is no recent project.
#[test]
fn templates_are_a_list_the_person_manages() {
    let tmp = Tmp::new("templates-list");
    let mut shell = tmp.empty_shell(Mode::Gui);
    let company = tmp.0.join("company.ew");
    done(&mut shell, "tools.company_standard", json!({}));
    done(&mut shell, "wizard.goto", json!({"step": 9}));
    assert!(
        shell
            .describe()
            .contains("check \"Use this template for new projects\": on"),
        "the last step's box is on"
    );
    done(&mut shell, "wizard.finish", json!({"dir": path(&company)}));
    done(&mut shell, "wizard.cancel", json!({}));
    assert_eq!(
        shell.settings().basic_project.as_deref(),
        Some(path(&company).as_str())
    );

    done(&mut shell, "templates.open", json!({}));
    let text = shell.describe();
    assert!(
        text.contains(
            "template 1 (selected) (for new projects) \"company.ew\": \"Its own frame and title block"
        ),
        "{text}"
    );
    assert!(
        text.contains("no pages"),
        "the wizard's template has no pages: {text}"
    );
    done(&mut shell, "templates.use", json!({"index": 1, "on": false}));
    assert_eq!(shell.settings().basic_project, None);
    done(
        &mut shell,
        "templates.rename",
        json!({"index": 1, "name": "Panels"}),
    );
    assert!(shell.describe().contains("template 1 (selected) \"Panels\""));
    done(&mut shell, "ui.dismiss", json!({}));

    // Not used for new projects: the dialog opens on Empty project.
    done(&mut shell, "file.new", json!({}));
    assert!(shell.describe().contains("start 1 (chosen) \"Empty project\""));
    done(&mut shell, "ui.dismiss", json!({}));

    // P4: Save as template… of a project set up the way he likes.
    new_project(&mut shell, &tmp.0, "cabinet", 1, false);
    done(&mut shell, "project.save_as_template", json!({}));
    assert!(matches!(shell.dialog(), Some(Dialog::Form(_))), "its form");
    done(&mut shell, "ui.dismiss", json!({}));
    done(
        &mut shell,
        "project.save_as_template",
        json!({"name": "Cabinet standard", "folder": path(&tmp.0)}),
    );
    let cabinet = tmp.0.join("Cabinet standard");
    assert_eq!(
        shell.settings().basic_project.as_deref(),
        Some(path(&cabinet).as_str()),
        "used for new projects unless the box is off"
    );
    done(&mut shell, "file.close", json!({}));

    // P6: editing a template, known as one.
    done(&mut shell, "templates.open", json!({}));
    done(&mut shell, "templates.edit", json!({"index": 2}));
    assert_eq!(shell.editing_template().as_deref(), Some("Cabinet standard"));
    assert!(
        shell.title().contains("Template: Cabinet standard"),
        "{}",
        shell.title()
    );
    assert!(
        !shell.settings().recent_projects.contains(&path(&cabinet)),
        "a template is no recent project"
    );
    done(&mut shell, "file.close", json!({}));

    // P5: removed from the list, the folder stays.
    done(&mut shell, "templates.open", json!({}));
    done(&mut shell, "templates.remove", json!({"index": 1}));
    assert!(
        company.join("manifest.json").is_file(),
        "the folder stays on the disk"
    );
    let text = shell.describe();
    assert!(text.contains("Its folder stays on the disk"), "{text}");
    assert!(
        text.contains("template 1 (selected) (for new projects) \"Cabinet standard\"")
            && !text.contains("template 2"),
        "{text}"
    );
    shell.shutdown();
}

/// G2: Check level… says what the level is for and what each one stops, and a person sets it in the window.
#[test]
fn the_check_level_is_set_in_the_window_in_words() {
    let tmp = Tmp::new("templates-level");
    let mut shell = tmp.empty_shell(Mode::Gui);
    new_project(&mut shell, &tmp.0, "job", 1, false);
    done(&mut shell, "project.check_level", json!({}));
    let text = shell.describe();
    assert!(
        text.contains("Before drawings go out, the program checks them."),
        "the purpose: {text}"
    );
    assert!(text.contains("field \"Check level\": \"Pre-delivery\""), "{text}");
    let Some(Dialog::Form(form)) = shell.dialog() else {
        panic!("the form")
    };
    assert!(
        shell
            .form_note(form, "level")
            .is_some_and(|n| n.contains("interruption point")),
        "what the chosen level stops"
    );
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "project.check_level", json!({"level": "full"}));
    done(&mut shell, "file.save", json!({}));
    let text = std::fs::read_to_string(tmp.0.join("job").join("project.jsonl")).unwrap();
    assert!(text.contains("\"release_scheme\":\"full\""), "{text}");
    assert_eq!(
        shell.dispatch("project.check_level", &json!({"level": "strict"})),
        Outcome::Failed {
            message: shell_bad_param("level")
        }
    );
    shell.shutdown();
}

/// The words of a parameter that has no such value.
fn shell_bad_param(name: &str) -> String {
    ew_viewmodel::strings::format(ew_viewmodel::key!("error-bad-param"), &[name])
}
