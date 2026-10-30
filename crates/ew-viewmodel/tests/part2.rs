// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shell model of WP-S2.01 part 2 on temporary directories: New project (FT-307), the sample project, the
//! QElectroTech import and the tutorials copy (FT-312), the journal discard (FT-313's first part), first run, the
//! company-standard wizard with its preview (FT-308), workspaces (FT-309), F1 and the quick switches (FT-311).

use ew_testdir as fresh_dir;

use std::path::{Path, PathBuf};

use ew_viewmodel::ew_api::Session;
use ew_viewmodel::shell::{AccessMode, Dialog, Mode, Outcome, Resources, ShellConfig, ShellModel};
use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
const VERSION: &str = "0.0.0-test";

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        Tmp(fresh_dir::fresh_dir("ew-vm2-test"))
    }

    fn resources() -> Resources {
        Resources {
            sample_dir: Some(PathBuf::from(GOLDEN_A)),
            content_dir: Some(PathBuf::from(CONTENT)),
            docs_dir: None,
            notices_file: None,
        }
    }

    fn config(&self) -> ShellConfig {
        ShellConfig::in_dir(&self.0.join("config"), VERSION).with_resources(Tmp::resources())
    }

    fn shell(&self) -> ShellModel {
        ShellModel::new(Mode::Headless, self.config())
    }

    fn gui(&self) -> ShellModel {
        ShellModel::new(Mode::Gui, self.config())
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn run(shell: &mut ShellModel, action: &str, params: Value) -> Outcome {
    shell.dispatch(action, &params)
}

fn done(shell: &mut ShellModel, action: &str, params: Value) {
    let out = run(shell, action, params);
    assert_eq!(out, Outcome::Done, "{action}: {out:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// FT-307, FT-312: new, sample, import, tutorials

#[test]
fn a_new_project_starts_from_the_builtin_basic_project_and_is_saved_at_once() {
    let tmp = Tmp::new();
    let dir = tmp.0.join("first.ew");
    let mut shell = tmp.shell();
    done(&mut shell, "file.new", json!({ "dir": path(&dir) }));
    assert_eq!(shell.project_name(), Some("first.ew"));
    assert_eq!(shell.access(), Some(AccessMode::Edit));
    assert!(!shell.is_dirty());
    assert_eq!(shell.page_labels(), ["1"]);
    assert!(!shell.can_undo());
    assert!(dir.join("manifest.json").is_file(), "written at once");
    assert_eq!(shell.recent_names(), ["first.ew"]);
    // The new project edits and saves like any other.
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "page.create", "params": {"number": 2}}),
    );
    assert!(shell.is_dirty());
    done(&mut shell, "file.save", json!({}));
    assert_eq!(shell.page_labels(), ["1", "2"]);
    done(&mut shell, "file.close", json!({}));
    let (reopened, _) = Session::open_read_only(&dir, VERSION).unwrap();
    assert_eq!(reopened.page_labels(), ["1", "2"]);
}

#[test]
fn new_project_asks_for_a_directory_in_the_gui_and_guards_unsaved_changes() {
    let tmp = Tmp::new();
    let mut shell = tmp.gui();
    done(&mut shell, "file.new", json!({}));
    let prompt = shell.take_prompt().expect("a directory prompt");
    assert_eq!(prompt.action, "file.new");
    assert_eq!(prompt.param, "dir");
    // A dirty project asks first.
    let first = tmp.0.join("a.ew");
    done(&mut shell, "file.new", json!({ "dir": path(&first) }));
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "page.create", "params": {"number": 2}}),
    );
    assert!(shell.is_dirty());
    done(
        &mut shell,
        "file.new",
        json!({ "dir": path(&tmp.0.join("b.ew")) }),
    );
    assert!(matches!(shell.dialog(), Some(Dialog::ConfirmDiscard { .. })));
    assert_eq!(shell.project_name(), Some("a.ew"), "nothing happened yet");
    shell.shutdown();
}

#[test]
fn a_new_project_in_an_occupied_directory_fails_without_a_path() {
    let tmp = Tmp::new();
    let dir = tmp.0.join("p.ew");
    let mut shell = tmp.shell();
    done(&mut shell, "file.new", json!({ "dir": path(&dir) }));
    done(&mut shell, "file.close", json!({}));
    match run(&mut shell, "file.new", json!({ "dir": path(&dir) })) {
        Outcome::Failed { message } => {
            assert!(message.contains("not empty"), "{message}");
            assert!(!message.contains(&path(&dir)), "{message}");
        }
        other => panic!("{other:?}"),
    }
    assert!(!shell.has_project());
}

#[test]
fn a_directory_with_anything_in_it_is_refused_before_the_open_project_closes() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    done(
        &mut shell,
        "file.new",
        json!({ "dir": path(&tmp.0.join("open.ew")) }),
    );
    let taken = tmp.0.join("taken");
    std::fs::create_dir_all(&taken).unwrap();
    std::fs::write(taken.join("notes.txt"), "mine").unwrap();
    let qet = tmp.0.join("example.qet");
    std::fs::write(&qet, QET).unwrap();
    let not_empty = |out: Outcome| match out {
        Outcome::Failed { message } => {
            assert!(message.contains("not empty"), "{message}");
            assert!(!message.contains(&path(&taken)), "{message}");
        }
        other => panic!("{other:?}"),
    };
    not_empty(run(&mut shell, "file.new", json!({ "dir": path(&taken) })));
    not_empty(run(&mut shell, "help.tutorials", json!({ "dir": path(&taken) })));
    not_empty(run(
        &mut shell,
        "file.import_qet",
        json!({ "file": path(&qet), "dir": path(&taken) }),
    ));
    assert_eq!(
        shell.project_name(),
        Some("open.ew"),
        "the open project was not closed"
    );
    done(&mut shell, "tools.company_standard", json!({}));
    not_empty(run(&mut shell, "wizard.finish", json!({ "dir": path(&taken) })));
    assert!(shell.wizard().is_some(), "the wizard stays open");
    assert!(
        shell
            .wizard()
            .unwrap()
            .error()
            .is_some_and(|e| e.contains("not empty"))
    );
    assert_eq!(shell.project_name(), Some("open.ew"));
    // Nothing was written: the directory holds what it held.
    let names: Vec<_> = std::fs::read_dir(&taken)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["notes.txt"]);
    // An empty directory and an absent one are fine.
    let empty = tmp.0.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    done(&mut shell, "wizard.cancel", json!({}));
    done(&mut shell, "file.new", json!({ "dir": path(&empty) }));
    assert_eq!(shell.project_name(), Some("empty"));
    shell.shutdown();
}

#[test]
fn the_sample_project_opens_read_only_and_save_as_makes_a_copy() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    done(&mut shell, "file.open_sample", json!({}));
    assert_eq!(shell.project_name(), Some("J-E-2416.ew"));
    assert!(matches!(
        shell.access(),
        Some(AccessMode::ReadOnly {
            reason_key: "readonly-sample"
        })
    ));
    assert!(shell.banner().unwrap_or_default().contains("sample project"));
    assert!(shell.recent_names().is_empty(), "the sample is no recent project");
    assert_eq!(
        run(&mut shell, "file.save", json!({})),
        Outcome::Refused {
            reason_key: "reason-read-only"
        }
    );
    assert!(
        !Path::new(GOLDEN_A).join(".~ew.lock").exists(),
        "no lock in the installation"
    );
    let copy = tmp.0.join("mine.ew");
    done(&mut shell, "file.save_as", json!({ "dir": path(&copy) }));
    assert_eq!(shell.access(), Some(AccessMode::Edit));
    assert_eq!(shell.project_name(), Some("mine.ew"));
    shell.shutdown();
}

#[test]
fn without_a_sample_the_action_fails_and_the_tutorials_page_still_opens() {
    let tmp = Tmp::new();
    let mut config = tmp.config();
    config.resources.sample_dir = None;
    let mut shell = ShellModel::new(Mode::Headless, config);
    assert!(!shell.has_sample());
    match run(&mut shell, "file.open_sample", json!({})) {
        Outcome::Failed { message } => assert_eq!(message, "The sample project is not installed."),
        other => panic!("{other:?}"),
    }
    done(&mut shell, "help.tutorials", json!({}));
    assert_eq!(shell.last_help(), Some("tutorials/index.html"));
}

#[test]
fn the_tutorials_open_a_fresh_copy_of_the_sample_project() {
    let tmp = Tmp::new();
    let dir = tmp.0.join("tutorial.ew");
    let mut shell = tmp.shell();
    done(&mut shell, "help.tutorials", json!({ "dir": path(&dir) }));
    assert_eq!(shell.access(), Some(AccessMode::Edit));
    assert_eq!(shell.project_name(), Some("tutorial.ew"));
    let (sample, _) = Session::open_read_only(Path::new(GOLDEN_A), VERSION).unwrap();
    assert_eq!(shell.page_labels(), sample.page_labels().as_slice());
    assert_eq!(shell.last_help(), Some("tutorials/index.html"));
    shell.shutdown();
}

const QET: &str = r#"<project version="0.100" title="Example">
<diagram title="First page" folio="1" cols="17" colsize="60" rows="8" rowsize="80" height="640"/>
</project>"#;

#[test]
fn a_qelectrotech_project_is_imported_into_a_new_directory() {
    let tmp = Tmp::new();
    let qet = tmp.0.join("example.qet");
    std::fs::write(&qet, QET).unwrap();
    let dir = tmp.0.join("imported.ew");
    let mut shell = tmp.shell();
    done(
        &mut shell,
        "file.import_qet",
        json!({ "file": path(&qet), "dir": path(&dir) }),
    );
    assert_eq!(shell.project_name(), Some("imported.ew"));
    assert_eq!(shell.access(), Some(AccessMode::Edit));
    assert!(!shell.is_dirty());
    assert_eq!(shell.page_count(), 1);
    let banner = shell.banner().unwrap_or_default().to_owned();
    assert!(banner.contains("Imported example.qet: 1 pages"), "{banner}");
    assert!(dir.join("manifest.json").is_file());
    shell.shutdown();
}

#[test]
fn an_import_that_cannot_be_read_fails_without_a_path() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    let missing = tmp.0.join("nope.qet");
    match run(
        &mut shell,
        "file.import_qet",
        json!({ "file": path(&missing), "dir": path(&tmp.0.join("x.ew")) }),
    ) {
        Outcome::Failed { message } => {
            assert_eq!(message, "Cannot read nope.qet.");
        }
        other => panic!("{other:?}"),
    }
    let bad = tmp.0.join("bad.qet");
    std::fs::write(&bad, "<notaproject/>").unwrap();
    match run(
        &mut shell,
        "file.import_qet",
        json!({ "file": path(&bad), "dir": path(&tmp.0.join("y.ew")) }),
    ) {
        Outcome::Failed { message } => {
            assert!(message.starts_with("Cannot import bad.qet"), "{message}");
            assert!(!message.contains(&path(&tmp.0)), "{message}");
        }
        other => panic!("{other:?}"),
    }
    assert!(!shell.has_project());
    // The GUI asks for the file first, then for the directory.
    let mut gui = tmp.gui();
    done(&mut gui, "file.import_qet", json!({}));
    let first = gui.take_prompt().unwrap();
    assert_eq!(first.param, "file");
    done(&mut gui, "file.import_qet", json!({ "file": path(&bad) }));
    assert_eq!(gui.take_prompt().unwrap().param, "dir");
}

// ---------------------------------------------------------------------------------------------------------------
// FT-313's first part: discarding the journal of an earlier session

fn crash_with_an_edit(tmp: &Tmp, dir: &Path) {
    let mut crashed = tmp.shell();
    done(&mut crashed, "file.open", json!({ "dir": path(dir) }));
    done(
        &mut crashed,
        "command.run",
        json!({"cmd": "page.create", "params": {"number": 996}}),
    );
    // Dropped without `shutdown`: the journal stays, like after a crash.
}

fn copy_golden(to: &Path) {
    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }
    copy_dir(Path::new(GOLDEN_A), to);
}

#[test]
fn the_journal_of_an_earlier_session_is_discarded_after_a_confirmation() {
    let tmp = Tmp::new();
    let dir = tmp.0.join("g.ew");
    copy_golden(&dir);
    crash_with_an_edit(&tmp, &dir);

    // The GUI asks.
    let mut gui = tmp.gui();
    done(&mut gui, "file.open", json!({ "dir": path(&dir) }));
    assert!(gui.can_discard_journal());
    done(&mut gui, "file.discard_journal", json!({}));
    assert!(matches!(gui.dialog(), Some(Dialog::ConfirmJournalDiscard)));
    assert!(gui.can_discard_journal(), "asking changes nothing");
    gui.shutdown();

    // A script must say that it confirms.
    let mut shell = tmp.shell();
    done(&mut shell, "file.open", json!({ "dir": path(&dir) }));
    match run(&mut shell, "file.discard_journal", json!({})) {
        Outcome::Failed { message } => assert!(message.contains("\"confirm\": true"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert!(shell.can_discard_journal());
    done(&mut shell, "file.discard_journal", json!({ "confirm": true }));
    assert_eq!(shell.access(), Some(AccessMode::Edit));
    assert!(!shell.can_discard_journal());
    assert!(shell.banner().unwrap_or_default().contains("discarded"));
    assert!(
        !shell.page_labels().iter().any(|l| l == "996"),
        "the edit is gone"
    );
    // The project edits now, and the next open finds no journal.
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "page.create", "params": {"number": 997}}),
    );
    done(&mut shell, "file.save", json!({}));
    done(&mut shell, "file.close", json!({}));
    done(&mut shell, "file.open", json!({ "dir": path(&dir) }));
    assert_eq!(shell.access(), Some(AccessMode::Edit));
    assert!(shell.page_labels().iter().any(|l| l == "997"));
    shell.shutdown();
}

#[test]
fn the_discard_is_refused_without_a_leftover_journal() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    assert_eq!(
        run(&mut shell, "file.discard_journal", json!({"confirm": true})),
        Outcome::Refused {
            reason_key: "reason-no-journal"
        }
    );
    done(
        &mut shell,
        "file.new",
        json!({ "dir": path(&tmp.0.join("p.ew")) }),
    );
    assert_eq!(
        run(&mut shell, "file.discard_journal", json!({"confirm": true})),
        Outcome::Refused {
            reason_key: "reason-no-journal"
        }
    );
    shell.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// FT-308: first run

#[test]
fn the_first_run_flow_records_its_answers_in_the_settings() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    assert!(
        shell.first_run().is_none(),
        "the headless mode never starts it by itself"
    );
    shell.start_first_run_if_needed();
    assert!(shell.first_run().is_some());
    done(
        &mut shell,
        "first_run.set",
        json!({"field": "language", "value": "en"}),
    );
    done(&mut shell, "first_run.next", json!({}));
    done(
        &mut shell,
        "first_run.set",
        json!({"field": "comes_from", "value": "kicad"}),
    );
    assert_eq!(shell.settings().comes_from, "kicad");
    assert_eq!(shell.settings().keymap, "kicad", "step 2 sets the keymap preset");
    done(&mut shell, "first_run.next", json!({}));
    assert!(
        !shell.settings().update_check,
        "the update check is off by default"
    );
    done(
        &mut shell,
        "first_run.set",
        json!({"field": "update_check", "value": true}),
    );
    done(&mut shell, "first_run.next", json!({}));
    done(
        &mut shell,
        "first_run.set",
        json!({"field": "theme", "value": "dark"}),
    );
    done(
        &mut shell,
        "first_run.set",
        json!({"field": "ui_scale", "value": 1.5}),
    );
    done(&mut shell, "first_run.next", json!({}));
    done(&mut shell, "first_run.next", json!({}));
    assert_eq!(
        shell.first_run().unwrap().step,
        ew_viewmodel::first_run::FirstRunStep::Start,
        "clamped"
    );
    done(&mut shell, "first_run.back", json!({}));
    done(&mut shell, "first_run.next", json!({}));
    done(&mut shell, "first_run.finish", json!({"start": "none"}));
    assert!(shell.first_run().is_none());
    assert!(shell.settings().first_run_done);
    assert_eq!(shell.settings().theme, "dark");
    assert_eq!(shell.settings().ui_scale, 1.5);
    // A second shell reads the answers, and the flow does not start again.
    let mut again = tmp.shell();
    assert_eq!(again.settings().comes_from, "kicad");
    assert!(again.settings().update_check);
    again.start_first_run_if_needed();
    assert!(again.first_run().is_none());
    // Help, First-run setup runs it again, at a step of the caller's choice.
    done(&mut again, "help.first_run", json!({"step": "privacy"}));
    assert_eq!(
        again.first_run().unwrap().step,
        ew_viewmodel::first_run::FirstRunStep::Privacy
    );
}

#[test]
fn bad_first_run_answers_are_refused_and_the_flow_ends_where_it_is_told() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    assert_eq!(
        run(&mut shell, "first_run.next", json!({})),
        Outcome::Refused {
            reason_key: "reason-no-first-run"
        }
    );
    done(&mut shell, "help.first_run", json!({}));
    for (field, value) in [
        ("language", json!("xx")),
        ("comes_from", json!("vi")),
        ("theme", json!("blue")),
        ("update_check", json!(3)),
        ("nope", json!(1)),
    ] {
        assert!(
            matches!(
                run(
                    &mut shell,
                    "first_run.set",
                    json!({"field": field, "value": value})
                ),
                Outcome::Failed { .. }
            ),
            "{field}"
        );
    }
    assert!(matches!(
        run(&mut shell, "first_run.finish", json!({"start": "elsewhere"})),
        Outcome::Failed { .. }
    ));
    // Finishing with a start choice runs that action: the sample opens.
    done(&mut shell, "first_run.finish", json!({"start": "sample"}));
    assert_eq!(shell.project_name(), Some("J-E-2416.ew"));
    assert!(shell.settings().first_run_done);
    shell.shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// FT-308: the wizard

#[test]
fn the_wizard_keeps_a_draft_until_it_finishes() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    done(&mut shell, "tools.company_standard", json!({}));
    assert!(shell.wizard().is_some());
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "company_name", "value": "Example Panels"}),
    );
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "csa_mains", "value": "2.5"}),
    );
    done(&mut shell, "wizard.goto", json!({"step": 4}));
    match run(
        &mut shell,
        "wizard.set",
        json!({"field": "csa_mains", "value": "many"}),
    ) {
        Outcome::Failed { message } => assert!(message.contains("number above zero"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert!(shell.wizard().unwrap().error().is_some());
    done(&mut shell, "wizard.cancel", json!({}));
    assert!(shell.wizard().is_none());
    assert!(
        tmp.0.join("config/wizard-draft.json").is_file(),
        "the draft stays"
    );
    // The next time it resumes where it was, with the answers.
    done(&mut shell, "tools.company_standard", json!({}));
    let w = shell.wizard().unwrap();
    assert_eq!(w.draft().get("company_name"), "Example Panels");
    assert_eq!(w.draft().get("csa_mains"), "2.5");
    assert_eq!(w.step(), ew_viewmodel::wizard::Step::Numbering);
    // Start over deletes it.
    done(&mut shell, "wizard.reset", json!({}));
    assert!(!tmp.0.join("config/wizard-draft.json").exists());
    assert_eq!(shell.wizard().unwrap().draft().get("company_name"), "");
    done(&mut shell, "wizard.cancel", json!({}));
}

#[test]
fn the_wizard_writes_a_basic_project_that_new_projects_start_from() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    done(&mut shell, "tools.company_standard", json!({}));
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "company_name", "value": "Example Panels"}),
    );
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "device_format", "value": "{page}-{letter}{counter}"}),
    );
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "csa_mains", "value": "2.5"}),
    );
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "location_node", "value": "J-1"}),
    );
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "first_revision", "value": "V0.1"}),
    );
    let basic = tmp.0.join("company.ew");
    done(&mut shell, "wizard.finish", json!({ "dir": path(&basic) }));
    // FT-336: the finished wizard stays on the review step until it is closed (the check scheme is applied after
    // finishing, by a person); its fields are no longer edited.
    assert!(shell.banner().unwrap_or_default().contains("company.ew"));
    let w = shell.wizard().expect("the finished wizard stays open");
    assert_eq!(w.finished_dir(), Some(basic.as_path()));
    assert!(w.can_apply_check_scheme());
    assert!(matches!(
        run(
            &mut shell,
            "wizard.set",
            json!({"field": "company_name", "value": "Other"})
        ),
        Outcome::Refused {
            reason_key: "reason-wizard-finished"
        }
    ));
    assert!(matches!(
        run(&mut shell, "wizard.finish", json!({ "dir": path(&basic) })),
        Outcome::Refused { .. }
    ));
    done(&mut shell, "wizard.cancel", json!({}));
    assert!(shell.wizard().is_none());
    assert!(
        !tmp.0.join("config/wizard-draft.json").exists(),
        "a finished wizard leaves no draft"
    );
    assert_eq!(
        shell.settings().basic_project.as_deref(),
        Some(path(&basic).as_str())
    );
    // The basic project is a project of kind basic with the settings, the nodes, the sections and the revision.
    let text = std::fs::read_to_string(basic.join("project.jsonl")).unwrap();
    assert!(text.contains("\"kind\":\"basic\""), "{text}");
    assert!(text.contains("company.dbu-wh-24v"), "the sample colour profile");
    assert!(
        text.contains("company.0.75-2.5"),
        "the size profile with the chosen cross section"
    );
    assert!(text.contains("{page}-{letter}{counter}"), "the numbering format");
    let structure = std::fs::read_to_string(basic.join("structure.jsonl")).unwrap();
    assert!(structure.contains("\"code\":\"P01\"") && structure.contains("\"code\":\"J-1\""));
    assert_eq!(structure.matches("\"$k\":\"section\"").count(), 3);
    let revisions = std::fs::read_to_string(basic.join("revisions.jsonl")).unwrap();
    assert!(revisions.contains("V0.1"), "{revisions}");

    // New project: the default basic project is the template, so the copy has its settings and its sections.
    assert_eq!(shell.default_template_name().as_deref(), Some("company.ew"));
    let new = tmp.0.join("job.ew");
    done(&mut shell, "file.new", json!({ "dir": path(&new) }));
    let text = std::fs::read_to_string(new.join("project.jsonl")).unwrap();
    assert!(!text.contains("\"kind\""), "a schematic project: {text}");
    assert!(text.contains("company.0.75-2.5"));
    let structure = std::fs::read_to_string(new.join("structure.jsonl")).unwrap();
    assert_eq!(structure.matches("\"$k\":\"section\"").count(), 3);
    // `template: builtin` overrides the default.
    done(&mut shell, "file.close", json!({}));
    let plain = tmp.0.join("plain.ew");
    done(
        &mut shell,
        "file.new",
        json!({ "dir": path(&plain), "template": "builtin" }),
    );
    assert!(
        !std::fs::read_to_string(plain.join("project.jsonl"))
            .unwrap()
            .contains("company.0.75")
    );
    shell.shutdown();
}

#[test]
fn the_wizard_needs_a_directory_parameter_in_a_script_and_a_prompt_in_the_gui() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    done(&mut shell, "tools.company_standard", json!({}));
    assert!(matches!(
        run(&mut shell, "wizard.finish", json!({})),
        Outcome::Failed { .. }
    ));
    let mut gui = tmp.gui();
    done(&mut gui, "tools.company_standard", json!({}));
    done(&mut gui, "wizard.finish", json!({}));
    assert_eq!(gui.take_prompt().unwrap().action, "wizard.finish");
    // A refused command leaves the wizard open with the reason.
    let mut open = tmp.shell();
    done(&mut open, "tools.company_standard", json!({}));
    done(
        &mut open,
        "wizard.set",
        json!({"field": "sections", "value": "1 100-199 A\n2 200-299 B"}),
    );
    let blocked = tmp.0.join("blocked.ew");
    std::fs::create_dir_all(&blocked).unwrap();
    std::fs::write(blocked.join("project.jsonl"), "").unwrap();
    // A directory that already holds a project file is refused.
    let out = run(&mut open, "wizard.finish", json!({ "dir": path(&blocked) }));
    let Outcome::Failed { message } = &out else {
        panic!("{out:?}");
    };
    assert!(!message.contains(&path(&tmp.0)), "{message}");
    assert!(open.wizard().is_some(), "the wizard stays open");
}

#[test]
fn the_preview_follows_the_draft_and_the_default_draft_reproduces_the_sample() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    done(&mut shell, "tools.company_standard", json!({}));
    let first = shell.wizard().unwrap().preview_rev();
    assert!(first > 0, "the preview was built when the wizard opened");
    assert!(shell.wizard_preview_content().is_ok());
    // A field that is only kept in the draft (the check scheme) does not rebuild the preview.
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "check_scheme", "value": "full"}),
    );
    assert_eq!(shell.wizard().unwrap().preview_rev(), first);
    // A numbering change does.
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "device_format", "value": "{page}-{letter}{counter}"}),
    );
    assert!(shell.wizard().unwrap().preview_rev() > first);
    // The cover and the sample page are two folios of the same preview.
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "preview", "value": "cover"}),
    );
    assert_eq!(shell.wizard().unwrap().preview_folio(), "1");
    let cover = shell.wizard_preview_content();
    assert!(cover.is_ok(), "{:?}", cover.err());
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "preview", "value": "page"}),
    );
    assert_eq!(shell.wizard().unwrap().preview_folio(), "102");
    assert!(
        matches!(
            run(
                &mut shell,
                "wizard.set",
                json!({"field": "preview", "value": "x"})
            ),
            Outcome::Failed { .. }
        ),
        "bad preview value"
    );
    done(&mut shell, "wizard.cancel", json!({}));
}

#[test]
fn every_choice_of_the_wizard_is_accepted_by_the_commands() {
    // The preview applies the settings commands to the sample project, so a refused value shows at once. Every
    // pair of the choices that reach the commands builds, and the review step can create the project: every
    // colour with every size, the xref format alternating so that each colour and each size also meets both
    // xref formats (8 of the 16 combinations, a pairwise cover).
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    done(&mut shell, "tools.company_standard", json!({}));
    let mut n = 0;
    for (ci, colour) in ["sample", "en60204-machine", "iec60445-installation", "default"]
        .into_iter()
        .enumerate()
    {
        for (si, size) in ["sample", "default"].into_iter().enumerate() {
            let xref = ["qet", "eplan"][(ci + si) % 2];
            for (field, value) in [("colour", colour), ("size", size), ("xref", xref)] {
                done(&mut shell, "wizard.set", json!({"field": field, "value": value}));
            }
            let content = shell.wizard_preview_content();
            assert!(content.is_ok(), "{colour} {size} {xref}: {:?}", content.err());
            let dir = tmp.0.join(format!("basic-{n}.ew"));
            n += 1;
            let out = run(&mut shell, "wizard.finish", json!({ "dir": path(&dir) }));
            assert_eq!(out, Outcome::Done, "{colour} {size} {xref}: {out:?}");
            done(&mut shell, "tools.company_standard", json!({}));
        }
    }
    assert_eq!(n, 8);
    shell.shutdown();
}

#[test]
fn the_default_draft_reproduces_the_sample_projects_folio_102() {
    // NFR-UX-08 in miniature: the defaults of the wizard are the sample project's conventions, so the settings
    // commands of the default draft change nothing, and the preview page is the sample's own page.
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    done(&mut shell, "file.open_sample", json!({}));
    let folio = "102".to_owned();
    let sample = shell.page_content(&folio).expect("the sample's folio 102");
    done(&mut shell, "tools.company_standard", json!({}));
    let preview = shell.wizard_preview_content().expect("the preview");
    assert_eq!(
        format!(
            "{:?}",
            preview.pages.iter().map(|p| &p.content).collect::<Vec<_>>()
        ),
        format!(
            "{:?}",
            sample.pages.iter().map(|p| &p.content).collect::<Vec<_>>()
        ),
        "the default draft leaves folio 102 as the sample has it"
    );
    // A different numbering format changes the page: the preview shows the current choices.
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "device_format", "value": "{page}-{letter}{counter}"}),
    );
    let changed = shell.wizard_preview_content().expect("the preview");
    assert_ne!(
        format!(
            "{:?}",
            changed.pages.iter().map(|p| &p.content).collect::<Vec<_>>()
        ),
        format!(
            "{:?}",
            sample.pages.iter().map(|p| &p.content).collect::<Vec<_>>()
        ),
        "the numbering format reaches the device tags on the page"
    );
    shell.shutdown();
}

/// The source texts of the shaped runs of a page output, in display-list order (the `text` member of each run).
fn page_texts(pages: &ew_viewmodel::ew_api::output::ExportPages) -> Vec<String> {
    use ew_viewmodel::ew_api::output::ew_dl::{Item, ItemKind};
    fn walk(items: &[Item], out: &mut Vec<String>) {
        for item in items {
            match &item.kind {
                ItemKind::Glyphs(run) => out.push(run.text.clone()),
                ItemKind::Group(group) => walk(&group.items, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    for page in &pages.pages {
        walk(&page.content.items, &mut out);
    }
    out
}

/// Folio `folio` of the project in `dir`, rendered through the page output.
fn render_folio(dir: &Path, folio: &str) -> ew_viewmodel::ew_api::output::ExportPages {
    let (session, _) = Session::open_read_only(dir, VERSION).expect("the project opens");
    session
        .export_pages(&ew_viewmodel::ew_api::output::ExportOptions {
            pages: ew_viewmodel::ew_api::output::PageSelection::Folios(vec![folio.to_owned()]),
            ..ew_viewmodel::ew_api::output::ExportOptions::default()
        })
        .expect("the folio renders")
}

/// FT-336: the wizard writes the company, the frame and the formats through their commands. The preview (the
/// sample project's cover and folio 102 with the choices) prints the notice and the dates in the chosen format; the
/// finished basic project holds the panel-builder party, the notice, the frame defaults with the embedded forms and
/// the languages and formats, and its page prints the notice in the frame the wizard chose. The check scheme stays
/// in the draft (its command is `HumanOnly`).
#[test]
fn the_wizard_writes_the_company_the_frame_and_the_formats() {
    const NOTICE: &str = "Property of Example Panels";
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    done(&mut shell, "tools.company_standard", json!({}));
    // The sample's own dates print in the sample's format, dd-mm-yyyy.
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "preview", "value": "cover"}),
    );
    let cover = page_texts(&shell.wizard_preview_content().expect("the cover"));
    assert!(cover.iter().any(|t| t.contains("14-04-2025")), "{cover:?}");
    for (field, value) in [
        ("company_name", "Example Panels"),
        ("company_notice", NOTICE),
        ("date_format", "iso8601"),
        ("check_scheme", "full"),
    ] {
        done(&mut shell, "wizard.set", json!({"field": field, "value": value}));
    }
    let cover = page_texts(&shell.wizard_preview_content().expect("the cover"));
    assert!(
        cover.iter().any(|t| t.contains("2025-04-14")),
        "the ISO date on the cover: {cover:?}"
    );
    assert!(!cover.iter().any(|t| t.contains("14-04-2025")), "{cover:?}");
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "preview", "value": "page"}),
    );
    let page = page_texts(&shell.wizard_preview_content().expect("folio 102"));
    assert!(
        page.iter().any(|t| t.contains(NOTICE)),
        "the notice on folio 102: {page:?}"
    );
    assert!(
        page.iter().any(|t| t.contains("2025-04-14")),
        "the ISO date on folio 102: {page:?}"
    );
    // The review says what is written.
    let review = shell.wizard().unwrap().draft().review();
    let kept: Vec<&str> = review
        .iter()
        .filter(|l| !l.written)
        .map(|l| l.label.as_str())
        .collect();
    assert_eq!(kept, ["Check scheme"], "{review:?}");
    let describe = shell.describe();
    assert!(describe.contains("kept"), "{describe}");

    let grid = tmp.0.join("grid.ew");
    done(&mut shell, "wizard.finish", json!({ "dir": path(&grid) }));
    let text = std::fs::read_to_string(grid.join("project.jsonl")).unwrap();
    for needle in [
        "{\"role\":\"panel_builder\",\"name\":\"Example Panels\",\"personal_data\":true}",
        "\"ew.company\":{\"notice\":\"Property of Example Panels\"}",
        "\"languages\":{\"source\":\"en\"}",
        "\"formatting\":{\"date\":\"iso8601\",\"decimal_separator\":\"point\"}",
        "\"frames\":{\"default\":",
    ] {
        assert!(text.contains(needle), "{needle} in {text}");
    }
    assert!(
        !text.contains("\"checks\""),
        "the check scheme is not written: {text}"
    );
    assert!(
        std::fs::read_dir(grid.join("lib/forms")).unwrap().count() == 4,
        "the shipped forms are embedded"
    );
    let folio_1 = render_folio(&grid, "1");
    assert!(
        page_texts(&folio_1).iter().any(|t| t.contains(NOTICE)),
        "the notice in the title block"
    );

    // The frame choice reaches the page output: the same draft with the no-grid frame draws another frame.
    done(&mut shell, "tools.company_standard", json!({}));
    for (field, value) in [
        ("company_name", "Example Panels"),
        ("company_notice", NOTICE),
        ("date_format", "iso8601"),
        ("frame", "qet-nogrid"),
    ] {
        done(&mut shell, "wizard.set", json!({"field": field, "value": value}));
    }
    let nogrid = tmp.0.join("nogrid.ew");
    done(&mut shell, "wizard.finish", json!({ "dir": path(&nogrid) }));
    let other = render_folio(&nogrid, "1");
    assert!(page_texts(&other).iter().any(|t| t.contains(NOTICE)));
    assert_ne!(
        format!("{:?}", folio_1.pages[0].content),
        format!("{:?}", other.pages[0].content),
        "the grid frame and the no-grid frame differ"
    );
    // The grid frame labels its 17 columns (0 to 16) and 8 rows (A to H); the no-grid frame does not.
    let labels: std::collections::BTreeSet<String> = (0..=16)
        .map(|c| c.to_string())
        .chain(('A'..='H').map(String::from))
        .collect();
    let printed = |p: &ew_viewmodel::ew_api::output::ExportPages| -> std::collections::BTreeSet<String> {
        page_texts(p).into_iter().filter(|t| labels.contains(t)).collect()
    };
    assert_eq!(
        printed(&folio_1),
        labels,
        "the grid frame's column and row labels"
    );
    assert_ne!(printed(&other), labels, "the no-grid frame has no full label set");
    shell.shutdown();
}

/// The vector logos of a page output (FT-384): the groups that an SVG image became, clipped to the image's rectangle,
/// whose paths are filled; the placeholder logo is one filled rectangle over its whole view box, so its path bounds
/// equal the clip.
fn vector_logos(pages: &ew_viewmodel::ew_api::output::ExportPages) -> usize {
    use ew_viewmodel::ew_api::output::ew_dl::{Item, ItemKind};
    fn walk(items: &[Item], out: &mut usize) {
        for item in items {
            if let ItemKind::Group(group) = &item.kind {
                let filled_to_clip = group.clip.is_some()
                    && !group.items.is_empty()
                    && group.items.iter().all(|i| match &i.kind {
                        ItemKind::Path(p) => p.fill.is_some() && p.path.bounds() == group.clip,
                        _ => false,
                    });
                if filled_to_clip {
                    *out += 1;
                } else {
                    walk(&group.items, out);
                }
            }
        }
    }
    let mut out = 0;
    for page in &pages.pages {
        walk(&page.content.items, &mut out);
    }
    out
}

/// The images of a page output, by content hash and format, in display-list order.
fn page_images(
    pages: &ew_viewmodel::ew_api::output::ExportPages,
) -> Vec<([u8; 32], ew_viewmodel::ew_api::output::ew_dl::ImageFormat)> {
    use ew_viewmodel::ew_api::output::ew_dl::{Item, ItemKind};
    fn walk(items: &[Item], out: &mut Vec<([u8; 32], ew_viewmodel::ew_api::output::ew_dl::ImageFormat)>) {
        for item in items {
            match &item.kind {
                ItemKind::Image(image) => out.push((image.image.sha256, image.image.format)),
                ItemKind::Group(group) => walk(&group.items, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    for page in &pages.pages {
        walk(&page.content.items, &mut out);
    }
    out
}

/// The company logo's reference in a project directory's `project.jsonl`: its hash (raw) and the member as written.
fn logo_of(dir: &Path) -> Option<([u8; 32], Value)> {
    let text = std::fs::read_to_string(dir.join("project.jsonl")).ok()?;
    let project: Value = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["$k"] == "project")?;
    let logo = project.get("ew.company")?.get("logo")?.clone();
    let hex = logo["sha256"].as_str()?.strip_prefix("sha256:")?;
    let mut sha = [0u8; 32];
    for (i, b) in sha.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()?;
    }
    Some((sha, logo))
}

/// FT-337 and FT-341: the wizard writes the logo as a project asset that the title block draws, the shipped EN 60204-1
/// colour profile, and the report blocks of the golden layout, whose folios render through the page output; the
/// preview shows the logo on the sample's cover; a project made from the basic project carries the asset; the review
/// says `written` for every field but the check scheme.
#[test]
fn the_wizard_writes_the_logo_the_shipped_profile_and_the_report_blocks() {
    use ew_viewmodel::ew_api::output::ew_dl::ImageFormat;
    // A placeholder logo, no company's artwork.
    const LOGO: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 10"><rect width="40" height="10"/></svg>"#;
    let tmp = Tmp::new();
    let logo_file = tmp.0.join("logo.svg");
    std::fs::write(&logo_file, LOGO).unwrap();
    let mut shell = tmp.shell();
    done(&mut shell, "tools.company_standard", json!({}));
    for (field, value) in [
        ("company_name", "Example Panels".to_owned()),
        ("company_logo", path(&logo_file)),
        ("colour", "en60204-machine".to_owned()),
        ("preview", "cover".to_owned()),
    ] {
        done(&mut shell, "wizard.set", json!({"field": field, "value": value}));
    }
    // The preview: the sample's cover with the logo in its title block, drawn as vector paths (FT-384).
    let cover = shell.wizard_preview_content().expect("the cover");
    assert_eq!(vector_logos(&cover), 1, "the logo on the preview's cover");
    assert!(
        !page_images(&cover).iter().any(|(_, f)| *f == ImageFormat::Svg),
        "no SVG image item is left for a renderer to skip"
    );
    // The review: every field is written but the check scheme.
    let review = shell.wizard().unwrap().review();
    let kept: Vec<&str> = review.iter().filter(|l| !l.written).map(|l| l.key).collect();
    assert_eq!(kept, ["check_scheme"], "{review:?}");
    let logo_line = review.iter().find(|l| l.key == "company_logo").unwrap();
    assert_eq!(logo_line.value, "logo.svg");

    let basic = tmp.0.join("basic.ew");
    done(&mut shell, "wizard.finish", json!({ "dir": path(&basic) }));
    // The logo: an asset file named by its hash, the company member naming it with its format and size.
    let (sha, member) = logo_of(&basic).expect("the company logo is written");
    assert_eq!(member["format"], "svg");
    assert_eq!(member["size"], json!([40, 10]));
    let hex: String = sha.iter().map(|b| format!("{b:02x}")).collect();
    let asset = basic.join("assets").join(format!("{hex}.svg"));
    assert_eq!(std::fs::read(&asset).unwrap(), LOGO, "the asset holds the file");
    let manifest = std::fs::read_to_string(basic.join("manifest.json")).unwrap();
    assert!(manifest.contains(&format!("assets/{hex}.svg")), "{manifest}");
    // The title block of folio 1 draws it, as vector paths.
    let folio_1 = render_folio(&basic, "1");
    assert_eq!(
        vector_logos(&folio_1),
        1,
        "the logo in the title block of folio 1"
    );
    let (session, _) = Session::open_read_only(&basic, VERSION).unwrap();
    let held = session
        .asset(&ew_viewmodel::ew_api::Sha256(sha))
        .expect("the project holds the asset");
    assert_eq!(held.0.format, ew_viewmodel::ew_api::AssetFormat::Svg);
    assert_eq!(&*held.1, LOGO);
    drop(session);
    // The shipped profile reaches the records.
    let project = std::fs::read_to_string(basic.join("project.jsonl")).unwrap();
    assert!(project.contains("\"key\":\"ew.en60204-1-machine\""), "{project}");
    assert!(project.contains("\"verified\":false"), "{project}");
    // The report blocks of the golden layout, on the embedded no-grid frame.
    let blocks = std::fs::read_to_string(basic.join("report-blocks.jsonl")).unwrap();
    assert_eq!(blocks.lines().count(), 9, "{blocks}");
    for kind in ["toc", "parts_summary", "terminal_diagram", "revision_overview"] {
        assert!(
            blocks.contains(&format!("{{\"kind\":\"{kind}\"}}")),
            "{kind} in {blocks}"
        );
    }
    assert_eq!(std::fs::read_dir(basic.join("lib/forms")).unwrap().count(), 4);
    // A report folio renders through the page output: the revision overview lists the first revision.
    let revisions = render_folio(&basic, "911");
    assert_eq!(revisions.pages.len(), 1);
    let texts = page_texts(&revisions);
    assert!(texts.iter().any(|t| t.contains("V1.0")), "{texts:?}");
    let toc = render_folio(&basic, "2");
    assert!(!toc.pages.is_empty(), "the table of contents renders");
    assert!(
        page_texts(&toc).iter().any(|t| t.contains("911")),
        "the TOC lists folio 911"
    );

    // A project made from the basic project carries the asset.
    done(&mut shell, "wizard.cancel", json!({}));
    let job = tmp.0.join("job.ew");
    done(&mut shell, "file.new", json!({ "dir": path(&job) }));
    assert_eq!(logo_of(&job).map(|(s, _)| s), Some(sha));
    assert_eq!(
        std::fs::read(job.join("assets").join(format!("{hex}.svg"))).unwrap(),
        LOGO
    );
    assert_eq!(vector_logos(&render_folio(&job, "1")), 1);
    shell.shutdown();
}

/// FT-336 ruling: the check scheme is applied after finishing by `wizard.apply_check_scheme`, a confirmed action that
/// submits the `HumanOnly` command `project.checks.set`; the headless mode refuses it with the human-only reason.
#[test]
fn the_check_scheme_is_applied_after_finishing_by_a_person_only() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    done(&mut shell, "tools.company_standard", json!({}));
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "check_scheme", "value": "full"}),
    );
    assert_eq!(
        run(&mut shell, "wizard.apply_check_scheme", json!({})),
        Outcome::Refused {
            reason_key: "reason-wizard-not-finished"
        }
    );
    let basic = tmp.0.join("company.ew");
    done(&mut shell, "wizard.finish", json!({ "dir": path(&basic) }));
    let review = shell.describe();
    assert!(
        review.contains("review \"Check scheme\": \"full\" kept"),
        "{review}"
    );
    assert!(review.contains("button \"Apply the check scheme\""), "{review}");
    for params in [json!({}), json!({"confirm": true})] {
        assert_eq!(
            run(&mut shell, "wizard.apply_check_scheme", params),
            Outcome::Refused {
                reason_key: "reason-human-only"
            },
            "the headless mode never applies it"
        );
    }
    let text = std::fs::read_to_string(basic.join("project.jsonl")).unwrap();
    assert!(!text.contains("\"checks\""), "{text}");
    done(&mut shell, "wizard.cancel", json!({}));

    // In the window: a confirmation that names the scheme, then the command as the person's.
    let mut gui = tmp.gui();
    done(&mut gui, "tools.company_standard", json!({}));
    done(
        &mut gui,
        "wizard.set",
        json!({"field": "check_scheme", "value": "full"}),
    );
    let window = tmp.0.join("window.ew");
    done(&mut gui, "wizard.finish", json!({ "dir": path(&window) }));
    done(&mut gui, "wizard.apply_check_scheme", json!({}));
    assert_eq!(
        gui.dialog(),
        Some(&Dialog::ConfirmCheckScheme {
            scheme: "full".into()
        })
    );
    let dialog = gui.describe();
    assert!(
        dialog.contains("dialog \"Apply the check scheme?\"") && dialog.contains("\"full\""),
        "{dialog}"
    );
    done(&mut gui, "wizard.apply_check_scheme", json!({"confirm": true}));
    assert_eq!(gui.dialog(), None);
    let text = std::fs::read_to_string(window.join("project.jsonl")).unwrap();
    assert!(
        text.contains("\"checks\":{\"release_scheme\":\"full\"}"),
        "{text}"
    );
    let review = gui.describe();
    assert!(
        review.contains("review \"Check scheme\": \"full\" written"),
        "{review}"
    );
    assert!(!review.contains("button \"Apply the check scheme\""), "{review}");
    assert_eq!(
        run(&mut gui, "wizard.apply_check_scheme", json!({"confirm": true})),
        Outcome::Refused {
            reason_key: "reason-check-scheme-written"
        }
    );
    done(&mut gui, "wizard.cancel", json!({}));
    gui.shutdown();
    shell.shutdown();
}

#[test]
fn without_the_sample_the_wizard_works_and_the_preview_says_why_it_is_empty() {
    let tmp = Tmp::new();
    let mut config = tmp.config();
    config.resources.sample_dir = None;
    let mut shell = ShellModel::new(Mode::Headless, config);
    done(&mut shell, "tools.company_standard", json!({}));
    let err = shell.wizard_preview_content().unwrap_err();
    assert!(err.contains("sample project is not installed"), "{err}");
    let describe = shell.describe();
    assert!(
        describe.contains("preview \"Folio 102\": unavailable \"The sample project is not installed"),
        "{describe}"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// FT-309: workspaces

#[test]
fn workspaces_are_files_in_the_configuration_directory() {
    let tmp = Tmp::new();
    let ws_dir = tmp.0.join("config/workspaces");
    let mut shell = tmp.shell();
    assert_eq!(shell.workspace(), "Schematic");
    assert_eq!(shell.workspace_names(), ["Schematic"]);
    let before = shell.layout().clone();
    // Save the layout as a named workspace; the file is the layout's text.
    done(&mut shell, "view.workspace_save", json!({"name": "Review"}));
    assert_eq!(shell.workspace(), "Review");
    let text = std::fs::read_to_string(ws_dir.join("Review.json")).unwrap();
    assert_eq!(text, before.to_json("Review"));
    assert_eq!(shell.workspace_names(), ["Schematic", "Review"]);
    assert_eq!(shell.settings().workspace, "Review");
    // Another layout, as a user drag would make it, is written one second after its last change.
    let tabbed = ew_viewmodel::layout::Layout {
        root: ew_viewmodel::layout::LayoutNode::Tabs {
            children: vec![
                ew_viewmodel::layout::LayoutNode::Pane(ew_viewmodel::layout::PaneKind::PageView),
                ew_viewmodel::layout::LayoutNode::Pane(ew_viewmodel::layout::PaneKind::Pages),
            ],
            active: 0,
        },
    };
    shell.set_layout(tabbed.clone(), 10.0);
    assert!(shell.layout_pending());
    assert!(!shell.persist_layout_if_due(10.5), "not yet");
    assert_eq!(std::fs::read_to_string(ws_dir.join("Review.json")).unwrap(), text);
    assert!(shell.persist_layout_if_due(11.0));
    assert!(!shell.layout_pending());
    assert_eq!(
        std::fs::read_to_string(ws_dir.join("Review.json")).unwrap(),
        tabbed.to_json("Review")
    );
    // Loading Schematic restores the built-in layout and bumps the version, so that the frame rebuilds its tree.
    let v = shell.layout_version();
    done(&mut shell, "view.workspace_load", json!({"name": "Schematic"}));
    assert_eq!(shell.layout(), &before);
    assert!(shell.layout_version() > v);
    // A new shell starts in the active workspace and finds its layout.
    done(&mut shell, "view.workspace_load", json!({"name": "Review"}));
    assert_eq!(shell.layout(), &tabbed);
    shell.shutdown();
    let again = tmp.shell();
    assert_eq!(again.workspace(), "Review");
    assert_eq!(again.layout(), &tabbed);
    // Reset goes back to the built-in layout and writes it for the active workspace.
    let mut again = again;
    done(&mut again, "view.workspace_reset", json!({}));
    assert_eq!(again.layout(), &before);
    again.shutdown();
    assert_eq!(
        std::fs::read_to_string(ws_dir.join("Review.json")).unwrap(),
        before.to_json("Review")
    );
}

#[test]
fn workspace_names_and_files_are_validated() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    for bad in ["", " x", "x ", "a/b", "..", "a\\b", "x.json"] {
        assert!(
            matches!(
                run(&mut shell, "view.workspace_save", json!({ "name": bad })),
                Outcome::Failed { .. }
            ),
            "save {bad:?}"
        );
        assert!(
            matches!(
                run(&mut shell, "view.workspace_load", json!({ "name": bad })),
                Outcome::Failed { .. }
            ),
            "load {bad:?}"
        );
    }
    match run(&mut shell, "view.workspace_load", json!({"name": "Nope"})) {
        Outcome::Failed { message } => assert_eq!(message, "There is no workspace named Nope."),
        other => panic!("{other:?}"),
    }
    // A damaged file is refused with the reason and left as it is; at start it gives the built-in layout.
    let ws_dir = tmp.0.join("config/workspaces");
    std::fs::create_dir_all(&ws_dir).unwrap();
    std::fs::write(ws_dir.join("Broken.json"), "{\"$schema\":\"ew.workspace/1\"}").unwrap();
    match run(&mut shell, "view.workspace_load", json!({"name": "Broken"})) {
        Outcome::Failed { message } => assert!(message.contains("cannot be used"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(shell.workspace(), "Schematic");
    shell.settings(); // the active workspace did not change
    assert!(ws_dir.join("Broken.json").is_file());
    // The GUI asks for the name; a script must give it.
    assert!(matches!(
        run(&mut shell, "view.workspace_save", json!({})),
        Outcome::Failed { .. }
    ));
    let mut gui = tmp.gui();
    done(&mut gui, "view.workspace_save", json!({}));
    assert!(matches!(gui.dialog(), Some(Dialog::SaveWorkspace { .. })));
    gui.set_workspace_text("Mine");
    match gui.dialog() {
        Some(Dialog::SaveWorkspace { text }) => assert_eq!(text, "Mine"),
        other => panic!("{other:?}"),
    }
    done(&mut gui, "view.workspace_save", json!({"name": "Mine"}));
    assert!(gui.dialog().is_none());
    assert_eq!(gui.workspace(), "Mine");
}

#[test]
fn a_headless_run_without_a_configuration_directory_writes_no_workspace() {
    let config = ShellConfig {
        config_dir: None,
        state_dir: None,
        app_version: VERSION.to_owned(),
        resources: Resources::default(),
    };
    let mut shell = ShellModel::new(Mode::Headless, config);
    done(&mut shell, "view.workspace_save", json!({"name": "Nowhere"}));
    done(&mut shell, "view.workspace_reset", json!({}));
    shell.shutdown();
    assert_eq!(shell.workspace(), "Nowhere");
}

// ---------------------------------------------------------------------------------------------------------------
// FT-311, FT-312: F1, the settings quick switches, the describe text

#[test]
fn f1_resolves_the_context_in_order() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    let topic_of = |shell: &mut ShellModel| -> String {
        done(shell, "help.documentation", json!({}));
        shell.last_help().unwrap().to_owned()
    };
    assert_eq!(topic_of(&mut shell), "how-to/application-window.html#start-page");
    done(
        &mut shell,
        "file.new",
        json!({ "dir": path(&tmp.0.join("p.ew")) }),
    );
    assert_eq!(
        topic_of(&mut shell),
        "how-to/application-window.html#project-view"
    );
    done(&mut shell, "help.shortcuts", json!({}));
    assert_eq!(
        topic_of(&mut shell),
        "how-to/application-window.html#keyboard-shortcuts"
    );
    done(&mut shell, "help.about", json!({}));
    assert_eq!(topic_of(&mut shell), "how-to/application-window.html#about");
    done(&mut shell, "tools.palette", json!({}));
    assert_eq!(
        topic_of(&mut shell),
        "how-to/application-window.html#command-palette"
    );
    done(&mut shell, "tools.company_standard", json!({}));
    assert_eq!(
        topic_of(&mut shell),
        "how-to/application-window.html#company-standard-wizard"
    );
    done(&mut shell, "wizard.cancel", json!({}));
    done(&mut shell, "help.first_run", json!({}));
    assert_eq!(
        topic_of(&mut shell),
        "how-to/application-window.html#first-run-setup"
    );
    // A hovered item names its own topic; an unknown topic is refused; the request carries no URL without a book.
    done(&mut shell, "help.documentation", json!({"topic": "workspaces"}));
    assert_eq!(
        shell.last_help(),
        Some("how-to/application-window.html#workspaces")
    );
    let request = shell.take_doc_request().unwrap();
    assert_eq!(request.url, None);
    assert!(shell.take_doc_request().is_none(), "taken once");
    assert!(matches!(
        run(&mut shell, "help.documentation", json!({"topic": "nope"})),
        Outcome::Failed { .. }
    ));
    shell.shutdown();
}

#[test]
fn the_documentation_url_points_into_the_installed_book() {
    let tmp = Tmp::new();
    let docs = tmp.0.join("book");
    std::fs::create_dir_all(&docs).unwrap();
    let mut config = tmp.config();
    config.resources.docs_dir = Some(docs.clone());
    let mut shell = ShellModel::new(Mode::Headless, config);
    done(&mut shell, "help.documentation", json!({"topic": "tutorials"}));
    let request = shell.take_doc_request().unwrap();
    assert_eq!(request.target, "tutorials/index.html");
    let url = request.url.unwrap();
    assert!(
        url.starts_with("file://") && url.ends_with("/book/tutorials/index.html"),
        "{url}"
    );
}

#[test]
fn quick_switches_validate_and_persist() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    done(
        &mut shell,
        "settings.set",
        json!({"key": "keymap", "value": "kicad"}),
    );
    done(
        &mut shell,
        "settings.set",
        json!({"key": "theme", "value": "light"}),
    );
    done(
        &mut shell,
        "settings.set",
        json!({"key": "language", "value": "en"}),
    );
    done(
        &mut shell,
        "settings.set",
        json!({"key": "update_check", "value": "on"}),
    );
    for (key, value) in [
        ("keymap", json!("emacs")),
        ("theme", json!(1)),
        ("language", json!("xx")),
        ("update_check", json!("maybe")),
        ("bogus", json!(1)),
    ] {
        assert!(
            matches!(
                run(&mut shell, "settings.set", json!({"key": key, "value": value})),
                Outcome::Failed { .. }
            ),
            "{key}"
        );
    }
    assert!(matches!(
        run(&mut shell, "settings.set", json!({"key": "theme"})),
        Outcome::Failed { .. }
    ));
    let again = tmp.shell();
    assert_eq!(again.settings().keymap, "kicad");
    assert_eq!(again.settings().theme, "light");
    assert!(again.settings().update_check);
    // The describe text shows the quick switches and the privacy line of the start page.
    let text = again.describe();
    assert!(text.contains("quick \"Keymap\": \"KiCad-like\""), "{text}");
    assert!(
        text.contains("label \"Update check: on (this version has none)\""),
        "{text}"
    );
}

#[test]
fn workspace_names_that_windows_reserves_or_that_differ_only_in_case_are_refused() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    for bad in ["CON", "nul", "Aux", "COM1", "lpt9"] {
        assert!(
            matches!(
                run(&mut shell, "view.workspace_save", json!({ "name": bad })),
                Outcome::Failed { .. }
            ),
            "{bad}"
        );
    }
    done(&mut shell, "view.workspace_save", json!({"name": "Review"}));
    match run(&mut shell, "view.workspace_save", json!({"name": "review"})) {
        Outcome::Failed { message } => assert!(message.contains("letter case"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        run(&mut shell, "view.workspace_load", json!({"name": "REVIEW"})),
        Outcome::Failed { .. }
    ));
    // The name as it is saved still works, and so does Schematic against "schematic".
    done(&mut shell, "view.workspace_save", json!({"name": "Review"}));
    assert!(matches!(
        run(&mut shell, "view.workspace_save", json!({"name": "schematic"})),
        Outcome::Failed { .. }
    ));
}

/// The words of a describe line that are structure: the quoted texts, numbers, action IDs (they have a dot),
/// shortcuts and punctuation are taken out, and everything after a free-text marker is user text.
fn structural_words(line: &str, shortcut_keys: &[String]) -> Vec<String> {
    let mut outside = String::new();
    let mut quoted = false;
    for c in line.chars() {
        if c == '"' {
            quoted = !quoted;
            outside.push(' ');
        } else if !quoted {
            outside.push(c);
        }
    }
    let outside = outside.replace(['(', ')', '[', ']', ',', '|', '%'], " ");
    let mut words = Vec::new();
    for token in outside.split_whitespace() {
        if ew_viewmodel::describe::FREE_TEXT_MARKERS.contains(&token) {
            words.push(token.to_owned());
            break;
        }
        let skip = token.chars().all(|c| c.is_ascii_digit())
            // An ID alias `@n` is a value token like a number (ID-630).
            || ew_viewmodel::alias::is_alias(token)
            || token.starts_with('#')
            || token.contains('.')
            || token.contains('+')
            || token.chars().all(|c| !c.is_alphanumeric())
            || shortcut_keys.iter().any(|k| k == token);
        if !skip {
            words.push(token.to_owned());
        }
    }
    words
}

#[test]
fn every_structural_word_of_the_describe_text_is_in_the_vocabulary() {
    let shortcut_keys: Vec<String> = ew_viewmodel::actions::ACTIONS
        .iter()
        .flat_map(|a| a.shortcut.iter())
        .flat_map(|s| s.split(['+', ' ']))
        .map(str::to_owned)
        .collect();
    let tmp = Tmp::new();
    let mut texts: Vec<String> = Vec::new();
    // The start page, a first run through all its steps, the wizard through all its steps, and a project with its
    // dialogs, popups and banners.
    let mut shell = tmp.gui();
    texts.push(shell.describe());
    done(&mut shell, "help.first_run", json!({}));
    for _ in 0..5 {
        texts.push(shell.describe());
        let _ = run(&mut shell, "first_run.next", json!({}));
    }
    // The privacy step with the opt-in on.
    done(&mut shell, "help.first_run", json!({"step": "privacy"}));
    done(
        &mut shell,
        "first_run.set",
        json!({"field": "update_check", "value": true}),
    );
    texts.push(shell.describe());
    done(
        &mut shell,
        "first_run.set",
        json!({"field": "comes_from", "value": "qet"}),
    );
    texts.push(shell.describe());
    done(&mut shell, "first_run.finish", json!({}));
    done(&mut shell, "tools.company_standard", json!({}));
    for step in 0..10 {
        done(&mut shell, "wizard.goto", json!({ "step": step }));
        texts.push(shell.describe());
    }
    // A refused value shows its reason on the screen (an `error:` line).
    let refused = run(
        &mut shell,
        "wizard.set",
        json!({"field": "csa_mains", "value": "x"}),
    );
    assert!(matches!(refused, Outcome::Failed { .. }));
    texts.push(shell.describe());
    done(&mut shell, "wizard.cancel", json!({}));
    let dir = tmp.0.join("p.ew");
    done(&mut shell, "file.new", json!({ "dir": path(&dir) }));
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "page.create", "params": {"number": 2}}),
    );
    done(&mut shell, "file.save", json!({}));
    done(&mut shell, "tools.palette", json!({"query": "page"}));
    texts.push(shell.describe());
    done(&mut shell, "tools.palette", json!({"query": "zzzz"}));
    texts.push(shell.describe());
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "view.workspace_save", json!({}));
    texts.push(shell.describe());
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "page.goto", json!({}));
    texts.push(shell.describe());
    done(&mut shell, "ui.dismiss", json!({}));
    // FT-408 and FT-409: the creation dialogs, the first with a refusal under its fields.
    done(&mut shell, "page.new", json!({}));
    texts.push(shell.describe());
    // WP-S2.06 part 1 (FT-461): the proposed folio is cleared, so Enter is refused.
    shell.set_form_field("folio", "");
    assert!(matches!(shell.submit_form(), Outcome::Failed { .. }));
    texts.push(shell.describe());
    for action in ["structure.new", "external_ref.new"] {
        done(&mut shell, action, json!({}));
        texts.push(shell.describe());
    }
    done(&mut shell, "ui.dismiss", json!({}));
    done(&mut shell, "file.close", json!({}));
    texts.push(shell.describe());
    // A refused open leaves a banner on the start page.
    let _ = run(
        &mut shell,
        "file.open",
        json!({ "dir": path(&tmp.0.join("missing.ew")) }),
    );
    texts.push(shell.describe());
    done(&mut shell, "file.open", json!({ "dir": path(&dir) }));
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "page.create", "params": {"number": 3}}),
    );
    done(&mut shell, "file.close", json!({}));
    texts.push(shell.describe());
    done(&mut shell, "ui.dismiss", json!({}));
    for dialog in ["help.about", "help.notices", "help.shortcuts"] {
        done(&mut shell, dialog, json!({}));
        texts.push(shell.describe());
    }
    done(&mut shell, "help.documentation", json!({}));
    texts.push(shell.describe());
    // A tab layout.
    let tabbed = r#"{"$schema":"ew.workspace/1","name":"T","root":{"tabs":[{"pane":"page-view"},{"pane":"pages"}],"active":1}}"#;
    std::fs::create_dir_all(tmp.0.join("config/workspaces")).unwrap();
    std::fs::write(tmp.0.join("config/workspaces/T.json"), tabbed).unwrap();
    done(&mut shell, "view.workspace_load", json!({"name": "T"}));
    texts.push(shell.describe());
    let vertical = r#"{"$schema":"ew.workspace/1","name":"V","root":{"split":"vertical","children":[{"share":1,"node":{"pane":"page-view"}},{"share":1,"node":{"pane":"pages"}}]}}"#;
    std::fs::write(tmp.0.join("config/workspaces/V.json"), vertical).unwrap();
    done(&mut shell, "view.workspace_load", json!({"name": "V"}));
    texts.push(shell.describe());
    shell.shutdown();
    // The canvas on the sample project (WP-S2.02): the statistics shown, the pointer on an entity (its hover line
    // with the box), the grid off, the pointer gone.
    let mut sample = tmp.gui();
    done(&mut sample, "file.open_sample", json!({}));
    done(&mut sample, "page.goto", json!({"folio": "103"}));
    done(&mut sample, "view.frame_stats", json!({}));
    let (x, y) = sample
        .canvas()
        .and_then(|c| {
            c.page()
                .entities
                .last()
                .map(|(_, r)| ew_viewmodel::canvas::rect_centre(*r))
        })
        .expect("folio 103 has entities");
    done(
        &mut sample,
        "canvas.pointer",
        json!({"x_mm": x as f64 / 1e6, "y_mm": y as f64 / 1e6}),
    );
    texts.push(sample.describe());
    done(&mut sample, "view.grid", json!({}));
    done(&mut sample, "canvas.pointer", json!({}));
    texts.push(sample.describe());
    // The CPU renderer setting (WP-S2.02 part 2).
    done(&mut sample, "view.cpu_canvas", json!({}));
    texts.push(sample.describe());
    // The tool framework (WP-S2.03 part 2): Select pressed and its rubber band, Measure armed on a connection point
    // and measuring. (The preview words `stale`, `failed`, `tags` and `xrefs` need a tool that previews, which only
    // the test-only probe is: `tests/probe.rs` checks them; `junction`, a snap marker kind, comes with WP-S2.04's
    // Wire tool and is checked above.)
    // An entity whose centre is no link (a press on a link follows it rather than selecting) and that is no
    // representation (a press on a representation drags it since WP-S2.04; on another entity it starts a band).
    let (x, y) = {
        let c = sample.canvas().unwrap();
        let page = c.page();
        page.tables
            .connections
            .iter()
            .filter_map(|w| {
                let (a, b) = (w.polyline.first()?, w.polyline.get(1)?);
                Some(page.snapper.page_to_sheet(((a.0 + b.0) / 2, (a.1 + b.1) / 2)))
            })
            .find(|p| {
                c.link_at(*p).is_none()
                    && ew_viewmodel::canvas::pick(&page.entities, Some(*p), 1.0)
                        .is_none_or(|(e, _)| !page.tables.reps.iter().any(|r| r.id == e.0))
            })
            .expect("a wire off the links and the symbols")
    };
    let at = json!({"x_mm": x as f64 / 1e6, "y_mm": y as f64 / 1e6});
    done(&mut sample, "canvas.press", at.clone());
    texts.push(sample.describe());
    done(
        &mut sample,
        "canvas.pointer",
        json!({"x_mm": x as f64 / 1e6 + 30.0, "y_mm": y as f64 / 1e6 + 20.0}),
    );
    texts.push(sample.describe());
    done(
        &mut sample,
        "canvas.release",
        json!({"x_mm": x as f64 / 1e6 + 30.0, "y_mm": y as f64 / 1e6 + 20.0}),
    );
    texts.push(sample.describe());
    done(&mut sample, "tool.measure", json!({}));
    done(&mut sample, "cp.next", json!({}));
    texts.push(sample.describe());
    done(&mut sample, "canvas.activate", json!({}));
    done(&mut sample, "cp.next", json!({}));
    texts.push(sample.describe());
    done(&mut sample, "ui.dismiss", json!({}));
    done(&mut sample, "ui.dismiss", json!({}));
    // The page navigator's views, sorts and a collapsed node; side by side and the focus (WP-S2.03 part 1).
    done(&mut sample, "pages.mode", json!({"mode": "flat"}));
    texts.push(sample.describe());
    done(&mut sample, "pages.mode", json!({"mode": "structure"}));
    done(&mut sample, "pages.sort", json!({"by": "type"}));
    let first = sample.navigator_rows()[0].key.clone();
    done(&mut sample, "pages.toggle", json!({"key": first}));
    done(&mut sample, "view.side_by_side", json!({}));
    done(&mut sample, "ui.focus", json!({"area": "navigator"}));
    done(&mut sample, "cursor.right", json!({}));
    texts.push(sample.describe());
    // The property grid (WP-S2.05 part 1): a read-only sheet, then on a writable copy a device with derived, inherited
    // and library rows, a frozen tag, a wire with its chips, an edit with its buffer, an error, choices, the popups, a
    // collapsed category, the card, the toast, a page with an overridden description and its finding, a gone record.
    done(&mut sample, "select.set", json!({"refs": ["tag:-101F1"]}));
    texts.push(sample.describe());
    done(
        &mut sample,
        "file.save_as",
        json!({ "dir": path(&tmp.0.join("copy.ew")) }),
    );
    done(&mut sample, "props.focus", json!({}));
    texts.push(sample.describe());
    done(&mut sample, "select.set", json!({"refs": ["tag:S51"]}));
    texts.push(sample.describe());
    done(
        &mut sample,
        "select.set",
        json!({"ids": ["9eb2d865-fca4-59b1-87bf-c073d7190ceb"]}),
    );
    texts.push(sample.describe());
    done(
        &mut sample,
        "props.action",
        json!({"row": "connection/derived.csa", "action": "override"}),
    );
    done(&mut sample, "props.input", json!({"text": "1,500"}));
    texts.push(sample.describe());
    done(&mut sample, "props.commit", json!({}));
    texts.push(sample.describe());
    done(&mut sample, "props.choose", json!({"index": 1}));
    done(
        &mut sample,
        "props.action",
        json!({"row": "connection/derived.colour", "action": "override"}),
    );
    done(&mut sample, "props.input", json!({"text": "BR"}));
    let _ = run(&mut sample, "props.commit", json!({}));
    texts.push(sample.describe());
    done(&mut sample, "props.cancel", json!({}));
    done(&mut sample, "props.row", json!({"to": "connection/derived.csa"}));
    done(&mut sample, "props.row_menu", json!({}));
    texts.push(sample.describe());
    done(&mut sample, "ui.dismiss", json!({}));
    done(
        &mut sample,
        "select.set",
        json!({"refs": ["tag:-101F1", "tag:-102F1"]}),
    );
    done(
        &mut sample,
        "props.mltext",
        json!({"row": "device/function_text"}),
    );
    texts.push(sample.describe());
    done(&mut sample, "ui.dismiss", json!({}));
    done(
        &mut sample,
        "props.category",
        json!({"category": "part", "expanded": false}),
    );
    done(
        &mut sample,
        "props.set",
        json!({"row": "device/function_text", "value": "Spare"}),
    );
    texts.push(sample.describe());
    done(&mut sample, "select.set", json!({"refs": ["tag:-102F1"]}));
    texts.push(sample.describe());
    done(
        &mut sample,
        "select.set",
        json!({"refs": ["tag:-101F1", "tag:-102F1"]}),
    );
    done(
        &mut sample,
        "props.set",
        json!({"row": "device/tag.counter", "value": "9"}),
    );
    texts.push(sample.describe());
    done(
        &mut sample,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-102F2"}],
            "path": "function_text", "value": "x"}}),
    );
    texts.push(sample.describe());
    done(&mut sample, "preview.cancel", json!({}));
    done(&mut sample, "select.set", json!({"ids": []}));
    done(&mut sample, "page.goto", json!({"folio": "101"}));
    done(
        &mut sample,
        "props.action",
        json!({"row": "page/derived.description", "action": "override"}),
    );
    done(
        &mut sample,
        "props.input",
        json!({"text": "Control Cabinet Expansion"}),
    );
    done(&mut sample, "props.commit", json!({}));
    texts.push(sample.describe());
    done(&mut sample, "props.project", json!({}));
    texts.push(sample.describe());
    done(&mut sample, "select.set", json!({"refs": ["tag:-101F1"]}));
    done(
        &mut sample,
        "command.run",
        json!({"cmd": "edit.delete", "params": {"targets": [{"kind": "device", "id": {"ref": "tag:-101F1"}}]}}),
    );
    texts.push(sample.describe());
    sample.shutdown();
    // A wizard whose preview is not available says why.
    let mut config = tmp.config();
    config.resources.sample_dir = None;
    let mut no_sample = ShellModel::new(Mode::Headless, config);
    done(&mut no_sample, "tools.company_standard", json!({}));
    texts.push(no_sample.describe());

    let allowed = ew_viewmodel::describe::VOCABULARY;
    // Every word the tool lines print for a marker, a snap or a preview status is in the vocabulary, reached or not.
    use ew_viewmodel::tools::{MarkerKind, PreviewStatus, SnapKind};
    for word in [
        MarkerKind::Grid.word(),
        MarkerKind::Cp.word(),
        MarkerKind::Junction.word(),
        SnapKind::Grid.word(),
        SnapKind::Cp.word(),
        PreviewStatus::Ready.word(),
        PreviewStatus::Stale.word(),
        PreviewStatus::Failed.word(),
    ] {
        assert!(allowed.contains(&word), "{word}");
    }
    let mut seen = std::collections::BTreeSet::new();
    for text in &texts {
        for line in text.lines() {
            for word in structural_words(line, &shortcut_keys) {
                assert!(
                    allowed.contains(&word.as_str()),
                    "`{word}` is in no word list of describe.rs; line: {line}"
                );
                seen.insert(word);
            }
        }
    }
    // No UUID reaches the describe text: the hovered entity and the selection print as aliases `@n`, value tokens
    // that the word check above skips (FT-422, ID-630).
    for text in &texts {
        assert_eq!(ew_viewmodel::alias::find_uuid(text), None, "{text}");
    }
    assert!(
        texts
            .iter()
            .flat_map(|t| t.lines())
            .any(|l| l.starts_with("canvas selection ")
                && l.split_whitespace().any(ew_viewmodel::alias::is_alias)),
        "a selection line with an alias"
    );
    // The states above reach most of the vocabulary: a word that nothing emits is a stale entry or a missed state.
    let unused: Vec<&&str> = allowed.iter().filter(|w| !seen.contains(**w)).collect();
    // (`disabled` without a reason cannot be reached through the registry today; `external`, a link to another
    // project, needs a cross-reference whose target is outside the page order, which Golden A does not draw.)
    // The 28 words of WP-S2.04 part 1 need a writable project: `tests/place.rs` reaches each of them.
    let part1 = ew_viewmodel::describe::PLACE_WORDS;
    // The 21 words of WP-S2.04 part 2 need wiring on a writable project: `tests/wiring.rs` reaches each of them.
    let wiring = ew_viewmodel::describe::WIRE_WORDS;
    // (The property grid's kind words of kinds this sequence does not select — a representation and its functions are
    // selected by a canvas click in `tests/properties.rs` and the kittest flows — and a warning or error finding.)
    assert!(
        unused.iter().all(|w| matches!(
            **w,
            "disabled"
                // WP-S2.17 part 4: the palette lists no model command (doc 16a §6.14).
                | "command"
                | "external"
                | "stale"
                | "failed"
                | "tags"
                | "xrefs"
                | "junction"
                | "warning"
                | "error"
                | "plc_config_project"
                | "import_lineage"
                | "potential_def"
                | "structure_node"
                | "external_ref"
                | "conductor"
                | "channel"
                | "bond"
                | "device_link"
                | "report_block"
                | "revision"
                // WP-S1.16 part 5: no state of this sequence selects a waiver record.
                | "waiver"
                | "page_item"
                | "structure_box"
                // WP-S1.19 part 1: no command creates a macro instance yet, so no state selects one.
                | "macro_instance"
                | "inherited"
                | "frozen"
                | "library"
                | "derived"
                | "missing"
                | "representation"
                | "function"
                // WP-S1.21 part 2: a CP with a printed PLC address needs a bound channel; `tests/plc_hover.rs` reaches it.
                | "address"
        ) || part1.contains(*w)
            // WP-S2.05 part 2: the find words need the find pane: `tests/find.rs` reaches them.
            || ew_viewmodel::describe::FIND_WORDS.contains(*w)
            // WP-S2.05 part 3: the clipboard's words need a copy: `tests/clipboard.rs` reaches them.
            || ew_viewmodel::describe::CLIP_WORDS.contains(*w)
            // FT-410: the parts import card's words: `tests/parts_import.rs` reaches them.
            || ew_viewmodel::describe::IMPORT_WORDS.contains(*w)
            || wiring.contains(*w)
            // WP-S6.13 part 1: the page text's words are `page.describe`'s: `tests/page_describe.rs` reaches them.
            || ew_viewmodel::describe::PAGE_WORDS.contains(*w)
            // FT-660: a generated page's `note` line: `tests/page_describe.rs` reaches the table lines.
            || ew_viewmodel::describe::LIST_WORDS.contains(*w)
            // WP-S2.04 part 3: the IP and black box words need the tools on a writable project: `tests/part3.rs`.
            || ew_viewmodel::describe::IP_WORDS.contains(*w)
            // WP-S2.04 part 4b: the item and box words need a writable project: `tests/part4b.rs` reaches `graphic`,
            // `marker` and `boxes`; the drawing tools' words and `unknown` come with the later units of part 4b.
            || ew_viewmodel::describe::ITEM_WORDS.contains(*w)
            // WP-S2.04 part 5: `image` comes with the Image tool: `tests/part5_image.rs` reaches it.
            || ew_viewmodel::describe::IMAGE_WORDS.contains(*w)
            // WP-S2.06 part 1: the navigator words need open navigators: `tests/navigators.rs` reaches them.
            || ew_viewmodel::describe::NAV_WORDS.contains(*w)
            // WP-S2.06 part 2: the further navigators' and the highlights' words: `tests/navigators_part2.rs`.
            || ew_viewmodel::describe::NAV2_WORDS.contains(*w)
            // WP-S2.09 part 1: the job, log and recovery words need a job, the log popup or a leftover journal:
            // `tests/jobs.rs`, `tests/notifications.rs` and `tests/recovery.rs` reach them.
            || ew_viewmodel::describe::JOB_WORDS.contains(*w)
            // WP-S2.09 part 2: the message panel's words need its pane: `tests/messages.rs` reaches them.
            || ew_viewmodel::describe::MSG_WORDS.contains(*w)
            // WP-S2.09 part 3: the preview and history words need a card or the history panel: `tests/preview_history.rs`.
            || ew_viewmodel::describe::PREVIEW_WORDS.contains(*w)
            // WP-S2.06 part 3: the table words need a navigator in table mode: `tests/navigators_table.rs` (TB8).
            || ew_viewmodel::describe::TABLE_WORDS.contains(*w)
            // WP-S2.07 part 1: `accessory` needs an accessory row in the strip editor: `tests/strip_editor.rs` (SE14).
            || ew_viewmodel::describe::STRIP_EDITOR_WORDS.contains(*w)
            // WP-S2.18 part 1: `chooser` needs the Join chooser open on a writable project: `tests/join.rs`.
            || **w == "chooser"),
        "words of the vocabulary no state used: {unused:?}"
    );
}

#[test]
fn describe_lists_the_start_page_the_wizard_and_the_project_view() {
    let tmp = Tmp::new();
    let mut shell = tmp.shell();
    let start = shell.describe();
    for needle in [
        "button \"New project\"",
        "label \"Template: the built-in basic project\"",
        "button \"Open sample project\"",
        "button \"Import QElectroTech project\"",
        "button \"Create company standard\"",
        "button \"Tutorials\"",
        "quick \"Language\": \"English\"",
        "item file.new \"New project…\" [Ctrl+N] enabled",
        "item file.discard_journal \"Discard unsaved changes of the earlier session…\" disabled: The project has no journal of an earlier session",
        "item help.documentation \"Documentation\" [F1] enabled",
        "item tools.company_standard \"Company standard wizard…\" enabled",
    ] {
        assert!(start.contains(needle), "{needle}\n{start}");
    }
    done(&mut shell, "tools.company_standard", json!({}));
    let wizard = shell.describe();
    assert!(
        wizard.contains("wizard \"Company standard\": step 1 of 10 \"Company\" (#1)"),
        "{wizard}"
    );
    assert!(wizard.contains("field \"Company name\": \"-\""), "{wizard}");
    assert!(wizard.contains("preview \"Folio 102\": ready"), "{wizard}");
    done(&mut shell, "wizard.goto", json!({"step": 9}));
    let review = shell.describe();
    assert!(
        review.contains("wizard \"Company standard\": step 10 of 10 \"Review and create\" (#11)"),
        "{review}"
    );
    assert!(
        review.contains("review \"Cross-reference style\": \"QET style (102-A1)\" written"),
        "{review}"
    );
    assert!(
        review.contains("review \"Company name\": \"-\" written"),
        "{review}"
    );
    assert!(
        review.contains("review \"Check scheme\": \"pre-delivery\" kept"),
        "{review}"
    );
    assert!(
        review.contains("button \"Create the basic project…\""),
        "{review}"
    );
    done(&mut shell, "wizard.cancel", json!({}));
    done(
        &mut shell,
        "file.new",
        json!({ "dir": path(&tmp.0.join("p.ew")) }),
    );
    let view = shell.describe();
    assert!(view.contains("workspace \"Schematic\""), "{view}");
    assert!(
        view.contains("layout: split horizontal [split vertical [\"Pages\" 50%, \"Properties\" 50%] 23%, \"Page view\" 77%]"),
        "{view}"
    );
    done(&mut shell, "help.about", json!({}));
    let about = shell.describe();
    for needle in [
        "Support: pre-release build; only the latest 0.x release is supported.",
        "ElektraWatt's checks do not replace engineering verification by a competent person.",
        "PRIVACY.md lists every data flow",
        "button \"Privacy (PRIVACY.md)\"",
        "button \"Third-party notices\"",
    ] {
        assert!(about.contains(needle), "{needle}\n{about}");
    }
    done(&mut shell, "help.notices", json!({}));
    let notices = shell.describe();
    assert!(notices.contains("dialog \"Third-party notices\""), "{notices}");
    shell.shutdown();
}
