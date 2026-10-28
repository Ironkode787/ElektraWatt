// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! JB7 (WP-S2.09 part 1, D7, §6.6; FR-UI-050's recovery half, FT-313's rest): a dirty session ended without a clean
//! close (`shutdown`) leaves its journal; the next editing open shows the project read-only with the recovery dialog
//! and writes nothing before a choice. Recover opens it editable and dirty with the edits, a notification and a
//! marker; Open read-only keeps the journal and the read-only access; Discard... asks, then opens it editable without
//! the edits; with the project saved by another session meanwhile so that a field collides, Recover fails, re-opens
//! read-only and says why.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::{GOLDEN_A, Tmp, copy_dir, done, refused};
use ew_viewmodel::ew_api::{CmdLog, Recovery, Session, SessionOptions, StoreOptions};
use ew_viewmodel::shell::{AccessMode, ConfirmAnswer, Dialog, Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;

fn config(tmp: &Tmp) -> ShellConfig {
    ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test")
}

fn shell(tmp: &Tmp, mode: Mode) -> ShellModel {
    ShellModel::new(mode, config(tmp))
}

fn project(tmp: &Tmp) -> PathBuf {
    tmp.0.join("small.ew")
}

fn open(s: &mut ShellModel, dir: &Path) {
    done(s, "file.open", json!({ "dir": dir.to_string_lossy() }));
}

/// The title of folio 101 set by a command-log line (one undo step).
fn set_title(s: &mut ShellModel, title: &str) {
    done(
        s,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "page", "targets": [{"ref": "folio:101"}],
            "path": "title", "value": title}}),
    );
}

/// A saved one-page project, then a session that edits its title and ends dirty without a clean close.
fn crashed(tmp: &Tmp) -> PathBuf {
    let dir = project(tmp);
    let mut s = shell(tmp, Mode::Headless);
    done(
        &mut s,
        "file.new",
        json!({"dir": dir.to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Saved"}),
    );
    done(&mut s, "file.save", json!({}));
    done(&mut s, "file.close", json!({}));
    open(&mut s, &dir);
    assert_eq!(s.access(), Some(AccessMode::Edit));
    set_title(&mut s, "Unsaved");
    assert!(s.is_dirty());
    // The end of a run with unsaved work: the journal stays (crash-recovery semantics).
    s.shutdown();
    dir
}

/// Every file below `dirs` with its bytes (the project and the journals).
fn files(dirs: &[PathBuf]) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.insert(p.clone(), std::fs::read(&p).unwrap_or_default());
            }
        }
    }
    let mut out = BTreeMap::new();
    for d in dirs {
        walk(d, &mut out);
    }
    out
}

/// The Pages navigator's line of folio 101 (with its title).
fn title(s: &ShellModel) -> String {
    s.describe()
        .lines()
        .find(|l| l.contains("page \"101\" \""))
        .unwrap_or_default()
        .to_owned()
}

fn dialog_line(s: &ShellModel) -> String {
    s.describe()
        .lines()
        .find(|l| l.starts_with("  recovery "))
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn jb7_a_leftover_journal_opens_read_only_with_the_dialog_and_nothing_is_written_before_a_choice() {
    let tmp = Tmp::new("jb7-dialog");
    let dir = crashed(&tmp);
    let watched = [dir.clone(), tmp.0.join("config").join("state")];
    let before = files(&watched);
    let mut s = shell(&tmp, Mode::Headless);
    open(&mut s, &dir);
    assert!(matches!(
        s.access(),
        Some(AccessMode::ReadOnly {
            reason_key: "readonly-recovery"
        })
    ));
    let d = s.describe();
    assert!(
        d.contains(
            "dialog \"Recover unsaved changes\"\n  label \"small.ew has unsaved changes from a session that ended \
             unexpectedly.\"\n  recovery transactions 1 base matches\n  button \"Recover\"\n  button \"Open \
             read-only\"\n  button \"Discard...\""
        ),
        "{d}"
    );
    assert!(!d.contains("Last change"), "no time in the describe text");
    assert_eq!(files(&watched), before, "nothing is written before a choice");
    // Anything but the dialog's answers is refused while it is open.
    assert_eq!(refused(&mut s, "help.about", json!({})), "reason-dialog-open");
    assert_eq!(
        refused(&mut s, "file.open", json!({"dir": dir.to_string_lossy()})),
        "reason-dialog-open"
    );
    assert_eq!(files(&watched), before);
    s.shutdown();
}

#[test]
fn jb7_recover_opens_the_project_editable_and_dirty_with_the_edits() {
    let tmp = Tmp::new("jb7-recover");
    let dir = crashed(&tmp);
    let mut s = shell(&tmp, Mode::Headless);
    open(&mut s, &dir);
    assert!(title(&s).contains("Saved"), "the disk state: {}", title(&s));
    done(&mut s, "journal.recover", json!({}));
    assert_eq!(s.dialog(), None);
    assert_eq!(s.access(), Some(AccessMode::Edit));
    assert!(s.is_dirty(), "recovered changes are unsaved");
    assert!(title(&s).contains("Unsaved"), "the edit is back: {}", title(&s));
    assert_eq!(s.toast().unwrap().text, "Recovered 1 change; save to keep it");
    assert_eq!(s.markers().len(), 1);
    assert_eq!(s.markers()[0].text, "Recovered 1 change from the journal");
    assert_eq!(s.banner(), None);
    done(&mut s, "file.save", json!({}));
    done(&mut s, "file.close", json!({}));
    // The next open finds no journal and the saved edit.
    open(&mut s, &dir);
    assert_eq!(s.dialog(), None);
    assert_eq!(s.access(), Some(AccessMode::Edit));
    assert!(title(&s).contains("Unsaved"));
    s.shutdown();
}

#[test]
fn jb7_open_read_only_keeps_the_journal_and_the_read_only_access() {
    let tmp = Tmp::new("jb7-keep");
    let dir = crashed(&tmp);
    let mut s = shell(&tmp, Mode::Headless);
    open(&mut s, &dir);
    let watched = [dir.clone(), tmp.0.join("config").join("state")];
    let before = files(&watched);
    done(&mut s, "journal.keep", json!({}));
    assert_eq!(s.dialog(), None);
    assert!(s.can_discard_journal(), "read-only for the journal, as built");
    assert!(
        s.banner()
            .unwrap_or_default()
            .contains("unsaved changes from a session that ended"),
        "the banner stays"
    );
    let banner = s.banner().unwrap_or_default();
    assert!(!banner.contains("later version"), "{banner}");
    assert!(banner.contains("Recover in the recovery dialog"), "{banner}");
    assert_eq!(
        refused(&mut s, "journal.recover", json!({})),
        "reason-no-recovery"
    );
    // Editing is refused (read-only); the journal is still there for the next open.
    assert_eq!(files(&watched), before);
    done(&mut s, "file.close", json!({}));
    open(&mut s, &dir);
    assert!(matches!(s.dialog(), Some(Dialog::Recovery { .. })));
    // Escape is Open read-only too.
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.dialog(), None);
    assert!(s.can_discard_journal());
    s.shutdown();
}

#[test]
fn jb7_discard_asks_then_opens_editable_without_the_edits() {
    let tmp = Tmp::new("jb7-discard");
    let dir = crashed(&tmp);
    // The window: Discard... asks; Cancel returns to the recovery dialog; Discard deletes the journal.
    let mut gui = shell(&tmp, Mode::Gui);
    open(&mut gui, &dir);
    done(&mut gui, "file.discard_journal", json!({}));
    assert!(matches!(gui.dialog(), Some(Dialog::ConfirmJournalDiscard)));
    assert_eq!(gui.answer_confirm(ConfirmAnswer::Cancel), Outcome::Done);
    assert!(matches!(gui.dialog(), Some(Dialog::Recovery { .. })));
    done(&mut gui, "file.discard_journal", json!({}));
    assert_eq!(gui.answer_confirm(ConfirmAnswer::Discard), Outcome::Done);
    assert_eq!(gui.dialog(), None);
    assert_eq!(gui.access(), Some(AccessMode::Edit));
    assert!(!gui.is_dirty());
    assert!(title(&gui).contains("Saved"), "the edit is gone");
    gui.shutdown();
    // The journal is gone: the next open is editable at once.
    let mut s = shell(&tmp, Mode::Headless);
    open(&mut s, &dir);
    assert_eq!(s.dialog(), None);
    assert_eq!(s.access(), Some(AccessMode::Edit));
    s.shutdown();
}

#[test]
fn jb7_a_script_discards_with_confirm() {
    let tmp = Tmp::new("jb7-script");
    let dir = crashed(&tmp);
    let mut s = shell(&tmp, Mode::Headless);
    open(&mut s, &dir);
    match s.dispatch("file.discard_journal", &json!({})) {
        Outcome::Failed { message } => assert!(message.contains("\"confirm\": true"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert!(
        matches!(s.dialog(), Some(Dialog::Recovery { .. })),
        "asking changes nothing"
    );
    done(&mut s, "file.discard_journal", json!({"confirm": true}));
    assert_eq!(s.dialog(), None);
    assert_eq!(s.access(), Some(AccessMode::Edit));
    assert!(title(&s).contains("Saved"));
    s.shutdown();
}

#[test]
fn jb7_a_collision_with_a_save_of_another_session_fails_the_recovery_and_reopens_read_only() {
    let tmp = Tmp::new("jb7-moved");
    let dir = crashed(&tmp);
    // Another session saves the same field meanwhile (it opens with the journal kept, as `ew` would).
    let mut store = StoreOptions::new("0.0.0-test");
    store.state_dir = Some(tmp.0.join("config").join("state"));
    let (mut other, report) = Session::open(&dir, store, SessionOptions::default(), Recovery::Keep).unwrap();
    assert!(report.recovery.is_some());
    let log = CmdLog::parse(
        "{\"$schema\":\"ew.cmdlog/1\",\"label\":\"other\"}\n{\"cmd\":\"fields.set\",\"params\":{\"kind\":\"page\",\
         \"targets\":[{\"ref\":\"folio:101\"}],\"path\":\"title\",\"value\":\"Elsewhere\"}}\n",
    )
    .unwrap();
    other.run_script(&log, "other").unwrap();
    other.save().unwrap();
    other.close();

    let mut s = shell(&tmp, Mode::Headless);
    open(&mut s, &dir);
    assert_eq!(dialog_line(&s), "  recovery transactions 1 base moved");
    let watched = [tmp.0.join("config").join("state")];
    done(&mut s, "journal.recover", json!({}));
    assert_eq!(
        dialog_line(&s),
        "  recovery transactions 1 base moved failed \"the project changed on disk in the same places; recovery \
         needs the merge panel\""
    );
    let d = s.describe();
    assert!(!d.contains("button \"Recover\""), "{d}");
    assert!(
        d.contains("  button \"Open read-only\"\n  button \"Discard...\""),
        "{d}"
    );
    assert!(matches!(
        s.access(),
        Some(AccessMode::ReadOnly {
            reason_key: "readonly-recovery"
        })
    ));
    assert!(title(&s).contains("Elsewhere"), "the disk state");
    assert!(!files(&watched).is_empty(), "the leftover journal stays");
    assert_eq!(
        refused(&mut s, "journal.recover", json!({})),
        "reason-recovery-failed"
    );
    // Discard still answers it.
    done(&mut s, "file.discard_journal", json!({"confirm": true}));
    assert_eq!(s.access(), Some(AccessMode::Edit));
    assert!(title(&s).contains("Elsewhere"));
    s.shutdown();
}

/// A copy of Golden A, the original of a Save As.
fn original(tmp: &Tmp) -> PathBuf {
    let original = tmp.golden_copy();
    copy_dir(Path::new(GOLDEN_A), &original);
    original
}

/// Opens `original` for editing and makes one unsaved edit (page 995).
fn edit(s: &mut ShellModel, original: &Path) {
    open(s, original);
    assert_eq!(s.access(), Some(AccessMode::Edit));
    done(
        s,
        "command.run",
        json!({"cmd": "page.create", "params": {"number": 995}}),
    );
    assert!(s.is_dirty());
}

fn has_995(s: &ShellModel) -> bool {
    s.page_labels().iter().any(|l| l == "995")
}

/// FT-499 (fixed in the cycle-30 review round): Save As closes the original cleanly once the copy exists, so the
/// edits that went into the copy leave no journal behind and the original reopens editable, without the recovery
/// dialog and unchanged on disk.
#[test]
fn save_as_closes_the_original_so_it_reopens_editable_without_a_journal() {
    let tmp = Tmp::new("ft499-save-as");
    let mut s = shell(&tmp, Mode::Headless);
    let original = original(&tmp);
    let on_disk = files(std::slice::from_ref(&original));
    let state = tmp.0.join("config").join("state");
    let copy = tmp.0.join("copy.ew");
    edit(&mut s, &original);
    // The original's journal: the only one so far.
    let journal: Vec<PathBuf> = files(std::slice::from_ref(&state)).into_keys().collect();
    assert_eq!(journal.len(), 1, "{journal:?}");
    let journal_dir = journal[0].parent().unwrap().to_path_buf();
    done(&mut s, "file.save_as", json!({ "dir": copy.to_string_lossy() }));
    assert!(!s.is_dirty());
    assert!(!journal_dir.exists(), "the original's journal is gone");
    done(&mut s, "file.close", json!({}));
    assert!(
        files(std::slice::from_ref(&state)).is_empty(),
        "no journal is left"
    );
    assert_eq!(
        files(std::slice::from_ref(&original)),
        on_disk,
        "the original's records are as before the edit"
    );
    open(&mut s, &original);
    assert_eq!(s.dialog(), None);
    assert_eq!(s.access(), Some(AccessMode::Edit));
    assert!(!s.can_discard_journal());
    assert!(!has_995(&s), "the original has no edit");
    done(&mut s, "file.close", json!({}));
    open(&mut s, &copy);
    assert!(has_995(&s), "the copy has the edit");
    s.shutdown();
}

/// FT-499: a Save As that cannot create its folder leaves the original and its journal as they were, so the dirty
/// original is still recoverable after the session ends without a clean close.
#[test]
fn a_failed_save_as_keeps_the_original_journal() {
    let tmp = Tmp::new("ft499-failed");
    let mut s = shell(&tmp, Mode::Headless);
    let original = original(&tmp);
    edit(&mut s, &original);
    let blocker = tmp.0.join("a-file");
    std::fs::write(&blocker, b"x").unwrap();
    let target = blocker.join("copy.ew");
    match s.dispatch("file.save_as", &json!({ "dir": target.to_string_lossy() })) {
        Outcome::Failed { .. } => {}
        other => panic!("{other:?}"),
    }
    assert!(s.is_dirty());
    s.shutdown();
    let mut again = shell(&tmp, Mode::Headless);
    open(&mut again, &original);
    assert!(matches!(again.dialog(), Some(Dialog::Recovery { .. })));
    done(&mut again, "journal.recover", json!({}));
    assert!(has_995(&again), "the edit is recovered");
    again.shutdown();
}
