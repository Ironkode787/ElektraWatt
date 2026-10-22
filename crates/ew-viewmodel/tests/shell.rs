// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shell-model transitions on a temporary copy of Golden A (WP-S2.01 part 1, acceptance checks A2, A3, A9).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_viewmodel::ew_api::Session;
use ew_viewmodel::shell::{AccessMode, Dialog, Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const VERSION: &str = "0.0.0-test";

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-vm-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    fn config(&self) -> ShellConfig {
        ShellConfig::in_dir(&self.0.join("config"), VERSION)
    }

    /// A copy of Golden A inside this directory.
    fn golden(&self, name: &str) -> PathBuf {
        let target = self.0.join(name);
        copy_dir(Path::new(GOLDEN_A), &target);
        target
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

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

fn open(shell: &mut ShellModel, dir: &Path) -> Outcome {
    shell.dispatch("file.open", &json!({ "dir": dir.to_string_lossy() }))
}

fn refused(shell: &mut ShellModel, action: &str, params: Value) -> &'static str {
    match shell.dispatch(action, &params) {
        Outcome::Refused { reason_key } => reason_key,
        other => panic!("{action} should be refused, got {other:?}"),
    }
}

#[test]
fn the_shell_starts_on_the_start_page_and_opens_a_project() {
    let tmp = Tmp::new();
    let dir = tmp.golden("J-E-2416.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    assert!(!shell.has_project());
    assert_eq!(shell.title(), "ElektraWatt");
    assert_eq!(shell.status_fields(false), ["No project open", "CPU", "EN"]);
    // Nothing that needs a project is enabled on the start page.
    assert_eq!(refused(&mut shell, "file.save", json!({})), "reason-no-project");
    assert_eq!(refused(&mut shell, "page.next", json!({})), "reason-no-project");
    assert_eq!(refused(&mut shell, "edit.undo", json!({})), "reason-no-project");
    assert_eq!(refused(&mut shell, "file.close", json!({})), "reason-no-project");

    assert_eq!(open(&mut shell, &dir), Outcome::Done);
    assert_eq!(shell.project_name(), Some("J-E-2416.ew"));
    assert_eq!(shell.title(), "J-E-2416.ew - ElektraWatt");
    assert_eq!(shell.access(), Some(AccessMode::Edit));
    assert!(!shell.is_dirty());
    // The page list equals the session's own labels.
    let (session, _) = Session::open_read_only(&dir, VERSION).unwrap();
    assert_eq!(shell.page_labels(), session.page_labels().as_slice());
    assert!(shell.page_count() > 3);
    assert_eq!(shell.page_index(), 0);
    let status = shell.status_fields(false);
    assert_eq!(status[0], "Read-only view");
    assert_eq!(
        status[1],
        format!("{} (1 of {})", shell.page_labels()[0], shell.page_count())
    );
    // The canvas's zoom and grid fields stand between the page and the save state (WP-S2.02 section 6.5).
    assert!(status[2].ends_with(" %"), "{status:?}");
    assert_eq!(status[3], "QET 10 px");
    assert_eq!(status[4], "saved");
    // The opened project is the first recent one.
    assert_eq!(shell.recent_names(), ["J-E-2416.ew"]);
    shell.shutdown();
}

#[test]
fn page_navigation_has_bounds_and_reasons() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut shell, &dir);
    let labels: Vec<String> = shell.page_labels().to_vec();
    let last = labels.len() - 1;

    assert_eq!(
        refused(&mut shell, "page.previous", json!({})),
        "reason-no-previous-page"
    );
    assert_eq!(shell.dispatch("page.next", &Value::Null), Outcome::Done);
    assert_eq!(shell.page_index(), 1);
    assert_eq!(shell.dispatch("page.previous", &Value::Null), Outcome::Done);
    assert_eq!(shell.page_index(), 0);
    assert_eq!(shell.dispatch("page.last", &Value::Null), Outcome::Done);
    assert_eq!(shell.page_index(), last);
    assert_eq!(refused(&mut shell, "page.next", json!({})), "reason-no-next-page");
    assert_eq!(shell.dispatch("page.first", &Value::Null), Outcome::Done);
    assert_eq!(shell.page_index(), 0);

    let folio = labels[3].clone();
    assert_eq!(
        shell.dispatch("page.goto", &json!({ "folio": folio })),
        Outcome::Done
    );
    assert_eq!(shell.page_index(), 3);
    assert_eq!(shell.current_folio(), Some(labels[3].as_str()));
    match shell.dispatch("page.goto", &json!({ "folio": "no-such-folio" })) {
        Outcome::Failed { message } => assert!(message.contains("no-such-folio"), "{message}"),
        other => panic!("{other:?}"),
    }
    // A script must name the folio: there is no dialog to ask in the headless mode.
    assert!(matches!(
        shell.dispatch("page.goto", &json!({})),
        Outcome::Failed { .. }
    ));
    assert_eq!(shell.page_index(), 3);
    shell.shutdown();
}

#[test]
fn edits_undo_redo_save_and_close() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut shell, &dir);
    let before = shell.page_labels().to_vec();

    assert_eq!(
        refused(&mut shell, "edit.undo", json!({})),
        "reason-nothing-to-undo"
    );
    assert_eq!(
        refused(&mut shell, "edit.redo", json!({})),
        "reason-nothing-to-redo"
    );

    let run = json!({"cmd": "page.create", "params": {"number": 999, "title": "Smoke"}});
    assert_eq!(shell.dispatch("command.run", &run), Outcome::Done);
    assert!(shell.is_dirty());
    assert!(shell.can_undo());
    assert!(shell.title().starts_with("*g.ew - "), "{}", shell.title());
    assert_eq!(shell.status_fields(false)[4], "*");
    assert_eq!(shell.page_count(), before.len() + 1);
    assert!(shell.page_labels().iter().any(|l| l == "999"));
    assert_eq!(shell.recorded_commands().len(), 1);

    assert_eq!(shell.dispatch("edit.undo", &Value::Null), Outcome::Done);
    assert_eq!(shell.page_labels(), before.as_slice());
    assert!(!shell.is_dirty());
    assert!(shell.can_redo());
    assert!(shell.recorded_commands().is_empty());
    assert_eq!(shell.dispatch("edit.redo", &Value::Null), Outcome::Done);
    assert!(shell.page_labels().iter().any(|l| l == "999"));
    assert_eq!(shell.recorded_commands().len(), 1);
    assert!(shell.is_dirty());

    // Close with unsaved changes fails in the headless mode unless the step says `discard`.
    match shell.dispatch("file.close", &json!({})) {
        Outcome::Failed { message } => assert!(message.contains("discard"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert!(shell.has_project());

    assert_eq!(shell.dispatch("file.save", &Value::Null), Outcome::Done);
    assert!(!shell.is_dirty());
    assert_eq!(shell.status_fields(false)[4], "saved");
    assert_eq!(shell.dispatch("file.close", &json!({})), Outcome::Done);
    assert!(!shell.has_project());
    assert_eq!(shell.title(), "ElektraWatt");

    // The saved page is there after a reopen.
    open(&mut shell, &dir);
    assert!(shell.page_labels().iter().any(|l| l == "999"));
    shell.shutdown();
}

#[test]
fn closing_a_dirty_project_with_discard_loses_the_edit() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut shell, &dir);
    let run = json!({"cmd": "page.create", "params": {"number": 998, "title": "Gone"}});
    assert_eq!(shell.dispatch("command.run", &run), Outcome::Done);
    assert_eq!(
        shell.dispatch("file.close", &json!({"discard": true})),
        Outcome::Done
    );
    open(&mut shell, &dir);
    assert!(!shell.page_labels().iter().any(|l| l == "998"));
    assert!(
        shell.recorded_commands().is_empty(),
        "a discarded step is not recorded"
    );
    shell.shutdown();
}

#[test]
fn the_gui_asks_instead_of_failing() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    // Open without a directory is a prompt for the window.
    assert_eq!(shell.dispatch("file.open", &Value::Null), Outcome::Done);
    let prompt = shell.take_prompt().expect("a prompt");
    assert_eq!(prompt.action, "file.open");
    assert_eq!(prompt.param, "dir");
    assert!(shell.take_prompt().is_none());
    open(&mut shell, &dir);

    let run = json!({"cmd": "page.create", "params": {"number": 997}});
    assert_eq!(shell.dispatch("command.run", &run), Outcome::Done);
    // Close with unsaved changes is a dialog.
    assert_eq!(shell.dispatch("file.close", &Value::Null), Outcome::Done);
    assert!(matches!(shell.dialog(), Some(Dialog::ConfirmDiscard { .. })));
    assert!(shell.has_project());
    // Escape (ui.dismiss) is Cancel.
    assert_eq!(shell.dispatch("ui.dismiss", &Value::Null), Outcome::Done);
    assert!(shell.dialog().is_none());
    assert!(shell.has_project());
    // Discard closes it.
    shell.dispatch("file.close", &Value::Null);
    let answer = shell.answer_confirm(ew_viewmodel::shell::ConfirmAnswer::Discard);
    assert_eq!(answer, Outcome::Done);
    assert!(!shell.has_project());
    // Save, then close.
    open(&mut shell, &dir);
    shell.dispatch("command.run", &run);
    shell.dispatch("file.close", &Value::Null);
    assert_eq!(
        shell.answer_confirm(ew_viewmodel::shell::ConfirmAnswer::Save),
        Outcome::Done
    );
    assert!(!shell.has_project());
    open(&mut shell, &dir);
    assert!(shell.page_labels().iter().any(|l| l == "997"));
    // Go to page opens a dialog in the window.
    assert_eq!(shell.dispatch("page.goto", &Value::Null), Outcome::Done);
    assert!(matches!(shell.dialog(), Some(Dialog::GotoPage { .. })));
    shell.shutdown();
}

#[test]
fn a_second_shell_opens_a_held_project_read_only() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut first = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut first, &dir);
    assert_eq!(first.access(), Some(AccessMode::Edit));

    let other = Tmp::new();
    let mut second = ShellModel::new(Mode::Headless, other.config());
    assert_eq!(open(&mut second, &dir), Outcome::Done);
    assert!(matches!(second.access(), Some(AccessMode::ReadOnly { .. })));
    assert!(
        second.banner().is_some_and(|b| b.contains("read-only")),
        "{:?}",
        second.banner()
    );
    assert!(second.status_fields(false)[4].starts_with("read-only: "));
    assert_eq!(refused(&mut second, "file.save", json!({})), "reason-read-only");
    assert_eq!(refused(&mut second, "edit.undo", json!({})), "reason-read-only");
    assert_eq!(
        refused(&mut second, "command.run", json!({"cmd": "page.create"})),
        "reason-read-only"
    );
    // Reading works.
    assert_eq!(second.dispatch("page.next", &Value::Null), Outcome::Done);
    second.shutdown();
    first.shutdown();
}

#[test]
fn a_leftover_journal_opens_the_project_read_only_with_a_banner() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    {
        let mut crashed = ShellModel::new(Mode::Headless, tmp.config());
        open(&mut crashed, &dir);
        let run = json!({"cmd": "page.create", "params": {"number": 996}});
        assert_eq!(crashed.dispatch("command.run", &run), Outcome::Done);
        // Dropped without `shutdown`: the journal stays, like after a crash.
    }
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    assert_eq!(open(&mut shell, &dir), Outcome::Done);
    assert!(matches!(shell.access(), Some(AccessMode::ReadOnly { .. })));
    let banner = shell.banner().unwrap_or_default().to_owned();
    assert!(banner.contains("unsaved changes"), "{banner}");
    assert!(banner.contains("read-only"), "{banner}");
    assert_eq!(refused(&mut shell, "file.save", json!({})), "reason-read-only");
    assert!(!shell.page_labels().iter().any(|l| l == "996"));
    shell.shutdown();
}

#[test]
fn save_as_writes_a_new_project_directory() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let target = tmp.0.join("copy.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut shell, &dir);
    let run = json!({"cmd": "page.create", "params": {"number": 995}});
    shell.dispatch("command.run", &run);
    let out = shell.dispatch("file.save_as", &json!({ "dir": target.to_string_lossy() }));
    assert_eq!(out, Outcome::Done, "{out:?}");
    assert_eq!(shell.project_name(), Some("copy.ew"));
    assert!(!shell.is_dirty());
    assert!(target.join("manifest.json").is_file());
    shell.shutdown();
    // A second Save As into the same, now occupied directory fails without a path in the message.
    let mut again = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut again, &dir);
    match again.dispatch("file.save_as", &json!({ "dir": target.to_string_lossy() })) {
        Outcome::Failed { message } => {
            assert!(!message.contains(&*tmp.0.to_string_lossy()), "{message}");
        }
        other => panic!("{other:?}"),
    }
    again.shutdown();
    // The copy opens and has the page.
    let mut third = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut third, &target);
    assert!(third.page_labels().iter().any(|l| l == "995"));
    third.shutdown();
}

#[test]
fn a_failed_open_leaves_no_path_in_the_message_and_drops_a_recent() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut shell, &dir);
    shell.dispatch("file.close", &json!({}));
    assert_eq!(shell.recent_names(), ["g.ew"]);
    // The directory goes away; the recent entry stays until it is opened.
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(shell.recent_names(), ["g.ew"]);
    match shell.dispatch("file.open_recent", &json!({"index": 0})) {
        Outcome::Failed { message } => {
            assert!(message.contains("g.ew"), "{message}");
            assert!(!message.contains(&*tmp.0.to_string_lossy()), "{message}");
        }
        other => panic!("{other:?}"),
    }
    assert!(shell.recent_names().is_empty());
    assert!(
        shell
            .banner()
            .is_some_and(|b| b.contains("removed from the recent projects"))
    );
    // And a missing directory that was never recent fails the same way.
    match shell.dispatch(
        "file.open",
        &json!({"dir": tmp.0.join("nothing").to_string_lossy()}),
    ) {
        Outcome::Failed { message } => assert!(!message.contains(&*tmp.0.to_string_lossy()), "{message}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_ui_scale_is_validated_and_persists() {
    let tmp = Tmp::new();
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    assert_eq!(shell.settings().ui_scale, 1.0);
    assert_eq!(
        shell.dispatch("view.ui_scale", &json!({"scale": 1.5})),
        Outcome::Done
    );
    assert_eq!(shell.settings().ui_scale, 1.5);
    assert!(matches!(
        shell.dispatch("view.ui_scale", &json!({"scale": 1.1})),
        Outcome::Failed { .. }
    ));
    let again = ShellModel::new(Mode::Headless, tmp.config());
    assert_eq!(again.settings().ui_scale, 1.5);
}

#[test]
fn a_corrupted_settings_file_raises_a_banner_and_keeps_a_copy() {
    let tmp = Tmp::new();
    let config = tmp.config();
    let dir = config.config_dir.clone().unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("settings.toml"), "{{ not toml").unwrap();
    let shell = ShellModel::new(Mode::Headless, config);
    assert!(
        shell.banner().is_some_and(|b| b.contains("settings.toml.bad-1")),
        "{:?}",
        shell.banner()
    );
    assert!(dir.join("settings.toml.bad-1").is_file());
}

#[test]
fn palette_lists_disabled_entries_last_with_their_reason() {
    let tmp = Tmp::new();
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    assert_eq!(
        shell.dispatch("tools.palette", &json!({"query": "save"})),
        Outcome::Done
    );
    let entries = shell.palette_entries();
    let save = entries
        .iter()
        .find(|e| e.id == "file.save")
        .expect("Save is listed");
    assert!(!save.enabled);
    assert_eq!(save.reason.as_deref(), Some("No project is open"));
    // Disabled entries come after the enabled ones.
    let first_disabled = entries.iter().position(|e| !e.enabled).unwrap();
    assert!(entries[first_disabled..].iter().all(|e| !e.enabled));
    // Matching is by label or ID, case-insensitively.
    shell.set_palette_query("FILE.OPEN");
    assert!(
        shell
            .palette_entries()
            .iter()
            .any(|e| e.id == "file.open" && e.enabled)
    );
    // Headless-only actions are not offered.
    shell.set_palette_query("describe");
    assert!(shell.palette_entries().iter().all(|e| e.id != "ui.describe"));
    // Toggle closes, Escape closes.
    assert_eq!(shell.dispatch("tools.palette", &Value::Null), Outcome::Done);
    assert!(shell.palette().is_none());
    shell.dispatch("tools.palette", &Value::Null);
    assert!(shell.palette().is_some());
    shell.dispatch("ui.dismiss", &Value::Null);
    assert!(shell.palette().is_none());
}

/// A9: a script cannot run a command that the exposure class refuses, and a bad command changes nothing.
#[test]
fn command_run_refuses_what_a_script_may_not_run() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut shell, &dir);
    let labels = shell.page_labels().to_vec();

    // Every registered command whose exposure class is not `Edit` (HumanOnly, Import, Internal).
    let (session, _) = Session::open_read_only(&dir, VERSION).unwrap();
    let refused_by_exposure: Vec<String> = session
        .registry()
        .commands()
        .filter(|c| format!("{:?}", c.meta().exposure) != "Edit")
        .map(|c| c.meta().name.to_owned())
        .collect();
    for name in &refused_by_exposure {
        match shell.dispatch("command.run", &json!({"cmd": name, "params": {}})) {
            Outcome::Failed { message } => assert!(
                message.contains("Exposure") || message.contains("exposure"),
                "{name}: {message}"
            ),
            other => panic!("{name} must fail, got {other:?}"),
        }
    }
    // An unknown command, bad parameters and a bad shape fail too.
    for params in [
        json!({"cmd": "no.such.command", "params": {}}),
        json!({"cmd": "page.create", "params": {"no_such_member": 1}}),
        json!({"cmd": "page.create", "params": 3}),
        json!({"params": {}}),
    ] {
        assert!(
            matches!(shell.dispatch("command.run", &params), Outcome::Failed { .. }),
            "{params}"
        );
    }
    assert_eq!(shell.page_labels(), labels.as_slice());
    assert!(!shell.is_dirty());
    assert!(!shell.can_undo());
    assert!(shell.recorded_commands().is_empty());
    shell.shutdown();
}

fn recorded(shell: &ShellModel) -> Vec<(String, Value)> {
    shell
        .recorded_commands()
        .iter()
        .map(|c| (c.cmd.clone(), c.params.clone()))
        .collect()
}

/// Review ruling 1: a step discarded by Open (or Open recent) leaves the record, so a reopen of the same
/// directory does not put it into `--record` (spec section 6.4).
#[test]
fn a_discarded_step_leaves_the_record_when_the_directory_is_reopened() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut shell, &dir);
    let run = json!({"cmd": "page.create", "params": {"number": 990}});
    assert_eq!(shell.dispatch("command.run", &run), Outcome::Done);
    assert_eq!(recorded(&shell).len(), 1);
    // Open over the dirty project needs `discard`, in a script.
    assert!(matches!(
        shell.dispatch("file.open", &json!({"dir": dir.to_string_lossy()})),
        Outcome::Failed { .. }
    ));
    assert_eq!(
        shell.dispatch(
            "file.open",
            &json!({"dir": dir.to_string_lossy(), "discard": true})
        ),
        Outcome::Done
    );
    assert!(recorded(&shell).is_empty(), "the discarded step is not recorded");
    assert!(!shell.page_labels().iter().any(|l| l == "990"));

    // The same through Open recent.
    shell.dispatch("command.run", &run);
    assert_eq!(
        shell.dispatch("file.open_recent", &json!({"index": 0, "discard": true})),
        Outcome::Done
    );
    assert!(recorded(&shell).is_empty());
    shell.shutdown();
}

/// Review ruling 1: what is undone past a save is not what the disk holds; the record keeps the steps as saved.
#[test]
fn undo_past_a_save_does_not_lose_the_saved_steps_from_the_record() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut shell, &dir);
    let a = json!({"cmd": "page.create", "params": {"number": 991}});
    let b = json!({"cmd": "page.create", "params": {"number": 992}});
    shell.dispatch("command.run", &a);
    shell.dispatch("file.save", &Value::Null);
    shell.dispatch("command.run", &b);
    assert_eq!(recorded(&shell).len(), 2);
    // Undo both: one is past the save.
    shell.dispatch("edit.undo", &Value::Null);
    shell.dispatch("edit.undo", &Value::Null);
    assert!(recorded(&shell).is_empty());
    // Discarding the close and reopening: the disk holds step a, and the record says so.
    assert_eq!(
        shell.dispatch("file.close", &json!({"discard": true})),
        Outcome::Done
    );
    assert_eq!(
        recorded(&shell),
        [("page.create".to_owned(), json!({"number": 991}))]
    );
    open(&mut shell, &dir);
    assert!(shell.page_labels().iter().any(|l| l == "991"));
    assert!(!shell.page_labels().iter().any(|l| l == "992"));
    assert_eq!(recorded(&shell).len(), 1);
    shell.shutdown();
}

/// Review ruling 4: a recent entry goes only when its path is gone.
#[test]
fn a_recent_entry_stays_when_the_open_fails_for_another_reason() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut shell, &dir);
    shell.dispatch("file.close", &json!({}));
    // The directory is there but no longer holds a project.
    std::fs::remove_dir_all(&dir).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    assert!(matches!(
        shell.dispatch("file.open_recent", &json!({"index": 0})),
        Outcome::Failed { .. }
    ));
    assert_eq!(shell.recent_names(), ["g.ew"], "the entry stays");
    assert!(
        shell.banner().is_some_and(|b| b.contains("Cannot open g.ew")),
        "{:?}",
        shell.banner()
    );
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(matches!(
        shell.dispatch("file.open_recent", &json!({"index": 0})),
        Outcome::Failed { .. }
    ));
    assert!(
        shell.recent_names().is_empty(),
        "the path is gone: the entry goes"
    );
}

/// Review ruling 6 (data loss): any exit while the project is dirty keeps the journal; a clean exit and an
/// explicit Discard remove it.
#[test]
fn the_journal_survives_a_shutdown_while_the_project_is_dirty() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let run = json!({"cmd": "page.create", "params": {"number": 993}});

    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut shell, &dir);
    shell.dispatch("command.run", &run);
    shell.shutdown();
    // The next open finds the journal: read-only with the recovery banner.
    let mut again = ShellModel::new(Mode::Headless, tmp.config());
    assert_eq!(open(&mut again, &dir), Outcome::Done);
    assert!(matches!(again.access(), Some(AccessMode::ReadOnly { .. })));
    assert!(
        again.banner().is_some_and(|b| b.contains("unsaved changes")),
        "{:?}",
        again.banner()
    );
    again.shutdown();
    // Opening read-only left the journal alone: it is still offered.
    let mut third = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut third, &dir);
    assert!(matches!(third.access(), Some(AccessMode::ReadOnly { .. })));
    third.shutdown();

    // An explicit Discard is the user's choice: the journal goes (a fresh copy, so nothing is left over).
    let dir2 = tmp.golden("h.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut shell, &dir2);
    shell.dispatch("command.run", &run);
    assert_eq!(
        shell.dispatch("file.close", &json!({"discard": true})),
        Outcome::Done
    );
    let mut after = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut after, &dir2);
    assert_eq!(after.access(), Some(AccessMode::Edit), "no journal after Discard");
    // A clean shutdown removes the journal too: nothing is left to recover.
    after.shutdown();
    let mut clean = ShellModel::new(Mode::Headless, tmp.config());
    open(&mut clean, &dir2);
    assert_eq!(clean.access(), Some(AccessMode::Edit));
    clean.shutdown();
}

/// Review ruling 9: Escape closes a popup or dialog and otherwise does nothing; the banner has a button.
#[test]
fn escape_leaves_the_banner_alone() {
    let tmp = Tmp::new();
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    shell.show_banner("note");
    assert_eq!(shell.dispatch("ui.dismiss", &Value::Null), Outcome::Done);
    assert_eq!(shell.banner(), Some("note"));
    shell.dismiss_banner();
    assert_eq!(shell.banner(), None);
}
