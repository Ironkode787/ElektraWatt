// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.18 part 5 (RU-3): the revision dialogs of Check ▸ Revision on copies of `golden/fixtures/release-min` (one open
//! revision V1.0 with its author; no finding but EW-REL-001 before the sign-off). The dialogs open, fill, validate and
//! cancel in the headless shell through their own actions; a scripted confirm is refused with its whole text; a
//! person at the window approves, releases and withdraws. FT-842: the release's housekeeping warnings in Messages.
//! Cycle 41, ruling 4: a scripted purge is refused.

use crate::common;

use std::path::{Path, PathBuf};

use common::{Tmp, copy_dir, done, refused};
use ew_viewmodel::dialogs::{DialogAnswer, spec};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/fixtures/release-min");
/// The date the window would pass (the shell never reads a clock).
const DATE: &str = "2026-10-30";
/// The refusal of every confirm that no person at the window made (cycle 41, ruling 1), as a whole text.
const PERSON: &str = "This needs a person at the window: a revision is approved, released, withdrawn or given its \
                      author only by a confirmation made there, never by a script, a replay or a plugin.";

/// A copy of the release fixture in `tmp`.
fn fixture_copy(tmp: &Tmp) -> PathBuf {
    let dir = tmp.0.join("release-min");
    copy_dir(Path::new(FIXTURE), &dir);
    dir
}

fn config(tmp: &Tmp) -> ShellConfig {
    ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test")
}

/// `shell` with the copy `dir` open for editing.
fn opened(mut shell: ShellModel, dir: &Path) -> ShellModel {
    done(&mut shell, "file.open", json!({ "dir": dir.to_string_lossy() }));
    shell
}

fn failed(shell: &mut ShellModel, action: &str, params: Value) -> String {
    match shell.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action} {params}: {other:?}"),
    }
}

/// The open form's action, its fields as `(name, value)` and its error.
fn form(shell: &ShellModel) -> (&'static str, Vec<(String, String)>, Option<String>) {
    match shell.dialog() {
        Some(Dialog::Form(f)) => (
            f.action,
            f.fields
                .iter()
                .map(|x| (x.name.to_owned(), x.value.clone()))
                .collect(),
            f.error.clone(),
        ),
        other => panic!("no form: {other:?}"),
    }
}

/// The lines of `ui.describe` from the dialog line on.
fn dialog_text(shell: &mut ShellModel) -> String {
    let d = shell.describe();
    let at = d.find("dialog \"").unwrap_or_else(|| panic!("no dialog in {d}"));
    d[at..]
        .lines()
        .take_while(|l| l.starts_with("dialog") || l.starts_with("  "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The archive a release of V1.0 writes beside the copy `dir` (the default root).
fn archive_of(dir: &Path) -> PathBuf {
    dir.parent()
        .unwrap()
        .join("release-min.ew-archive")
        .join("V1.0.ewarchive")
}

/// RU-3, the first step (cycle 41, ruling 1: "the dialogs open, fill, validate and cancel in the headless shell, through
/// their own actions with parameters"): Approve, Release and Set author open with the gate's result in words, take
/// their fields from parameters, refuse a wrong typed label (whole text, kept in the dialog) and close with `cancel`.
#[test]
fn ru3_the_revision_dialogs_open_fill_validate_and_cancel_headless() {
    let tmp = Tmp::new("revision-headless");
    let dir = fixture_copy(&tmp);
    let mut s = opened(ShellModel::new(Mode::Headless, config(&tmp)), &dir);

    // Approve: the gate passes but for EW-REL-001; the checker's role and the typed label.
    done(&mut s, "revision.approve", json!({}));
    assert_eq!(
        dialog_text(&mut s),
        "dialog \"Approve revision V1.0\"\n  label \"The checker signs off the saved content of V1.0, written by the \
         author. The release gate runs on the saved project first; the sign-off is saved.\"\n  field \"Checker's role\": \
         \"checker\"\n  field \"Type the label to confirm\": \"\"\n  button \"Approve\"\n  button \"Cancel\""
    );
    done(&mut s, "revision.approve", json!({ "typed": "V0.9" }));
    assert_eq!(
        form(&s).1,
        vec![
            ("role".to_owned(), "checker".to_owned()),
            ("typed".to_owned(), "V0.9".to_owned())
        ]
    );
    assert_eq!(
        failed(
            &mut s,
            "revision.approve",
            json!({ "confirm": true, "date": DATE })
        ),
        "Type the revision label V1.0 to confirm."
    );
    assert_eq!(
        form(&s).2.as_deref(),
        Some("Type the revision label V1.0 to confirm.")
    );
    assert_eq!(
        failed(
            &mut s,
            "revision.approve",
            json!({ "role": "nobody", "typed": "V1.0", "confirm": true })
        ),
        "Choose a role."
    );
    done(&mut s, "revision.approve", json!({ "cancel": true }));
    assert!(s.dialog().is_none(), "cancel closes the dialog");

    // Release: refused by the gate's EW-REL-001 before the sign-off, so it shows the blocker and Close only.
    done(&mut s, "revision.release", json!({}));
    let text = dialog_text(&mut s);
    assert!(
        text.starts_with("dialog \"Release revision V1.0?\"\n  label \"The release is blocked by 1 findings of the release gate:\"\n  item \""),
        "{text}"
    );
    assert!(text.ends_with("\n  button \"Close\""), "{text}");
    assert!(!text.contains("EW-REL-001"), "words, not rule IDs: {text}");
    done(&mut s, "revision.release", json!({ "cancel": true }));
    assert!(s.dialog().is_none());

    // Set author: the role field, the current author preselected.
    done(&mut s, "revision.set_author", json!({ "role": "engineer" }));
    assert_eq!(
        dialog_text(&mut s),
        "dialog \"Set the author of V1.0\"\n  label \"The party with this role is the author of V1.0; the checker who \
         approves it must have another role.\"\n  field \"Author's role\": \"engineer\"\n  button \"Set author\"\n  \
         button \"Cancel\""
    );
    done(&mut s, "revision.set_author", json!({ "cancel": true }));

    // Withdraw needs a released revision.
    assert_eq!(
        refused(&mut s, "revision.withdraw", json!({})),
        "reason-no-released-revision"
    );
    assert_eq!(
        refused(&mut s, "revision.incomplete", json!({})),
        "reason-no-incomplete-release"
    );
    s.shutdown();
}

/// RU-3, the second step (cycle 41, ruling 1: "the scripted confirm is refused, asserted on its whole text"): every
/// confirm of a headless shell and of a window a script drives (the visual driver's `ShellModel::new_scripted`) is
/// refused with the person text and changes nothing.
#[test]
fn ru3_a_scripted_confirm_is_refused_with_the_whole_text() {
    let tmp = Tmp::new("revision-scripted");
    let dir = fixture_copy(&tmp);
    for scripted in [
        ShellModel::new(Mode::Headless, config(&tmp)),
        ShellModel::new_scripted(config(&tmp)),
    ] {
        assert!(!scripted.is_person());
        let mut s = opened(scripted, &dir);
        let rev = s.revision();
        let approve = json!({ "role": "checker", "typed": "V1.0", "date": DATE, "confirm": true });
        assert_eq!(failed(&mut s, "revision.approve", approve), PERSON);
        assert_eq!(
            form(&s).2.as_deref(),
            Some(PERSON),
            "the refusal stays in the dialog"
        );
        done(&mut s, "revision.approve", json!({ "cancel": true }));
        assert_eq!(
            failed(
                &mut s,
                "revision.set_author",
                json!({ "role": "engineer", "confirm": true })
            ),
            PERSON
        );
        done(&mut s, "ui.dismiss", Value::Null);
        assert_eq!(s.revision(), rev, "nothing was submitted");
        assert!(!s.is_dirty());
        s.shutdown();
    }
    // The sign-off is in place (a person approved it): the scripted release confirm is refused all the same, and no
    // archive is written.
    let mut person = opened(ShellModel::new(Mode::Gui, config(&tmp)), &dir);
    done(
        &mut person,
        "revision.approve",
        json!({ "role": "checker", "typed": "V1.0", "date": DATE, "confirm": true }),
    );
    person.shutdown();
    for scripted in [
        ShellModel::new(Mode::Headless, config(&tmp)),
        ShellModel::new_scripted(config(&tmp)),
    ] {
        let mut s = opened(scripted, &dir);
        done(&mut s, "revision.release", json!({}));
        assert_eq!(form(&s).1, vec![("typed".to_owned(), String::new())]);
        let release = json!({ "typed": "V1.0", "date": DATE, "confirm": true });
        assert_eq!(failed(&mut s, "revision.release", release), PERSON);
        assert!(!archive_of(&dir).exists(), "nothing was released");
        assert_eq!(s.open_revision_label().as_deref(), Some("V1.0"));
        s.shutdown();
    }
}

/// RU-3, the completing half (cycle 41, ruling 1: "an ew-viewmodel test proves the completing half (approve, release,
/// withdraw on `golden/fixtures/release-min`) by confirming the dialog through the same window-input path a click
/// takes"): the window's shell (`ShellModel::new(Mode::Gui, …)`) dispatches each dialog's confirm button, as the window
/// does on a click (with the date it reads).
#[test]
fn ru3_a_person_at_the_window_approves_releases_and_withdraws() {
    let tmp = Tmp::new("revision-person");
    let dir = fixture_copy(&tmp);
    let mut s = opened(ShellModel::new(Mode::Gui, config(&tmp)), &dir);
    assert!(s.is_person());

    // The confirm the window sends is the dialog's own button: Approve is the Primary.
    done(&mut s, "revision.approve", json!({}));
    let d = s.dialog().cloned().unwrap();
    let approve = spec(&d, &s).primary.answer;
    assert_eq!(
        approve,
        DialogAnswer::Action("revision.approve", json!({ "confirm": true }))
    );
    done(&mut s, "revision.approve", json!({ "typed": "V1.0" }));
    done(
        &mut s,
        "revision.approve",
        json!({ "confirm": true, "date": DATE }),
    );
    assert!(s.dialog().is_none());
    assert!(!s.is_dirty(), "the approval is saved");
    assert!(
        s.notifications()
            .visible()
            .iter()
            .any(|n| n.text == "Approved revision V1.0; saved"),
        "the toast"
    );

    // Release: the six steps; Cancel is the Primary, Release the destructive button.
    done(&mut s, "revision.release", json!({}));
    let d = s.dialog().cloned().unwrap();
    let r = spec(&d, &s);
    assert!(r.confirms_destruction());
    assert_eq!(
        r.danger.as_ref().map(|b| b.answer.clone()),
        Some(DialogAnswer::Action(
            "revision.release",
            json!({ "confirm": true })
        ))
    );
    let text = dialog_text(&mut s);
    assert!(
        text.contains(
            "  item \"4. Write and verify the archive V1.0.ewarchive in the folder release-min.ew-archive\""
        ),
        "{text}"
    );
    assert!(
        text.contains("  item \"3. No git commit: the project folder is not in a git repository\""),
        "{text}"
    );
    done(&mut s, "revision.release", json!({ "typed": "V1.0" }));
    done(
        &mut s,
        "revision.release",
        json!({ "confirm": true, "date": DATE }),
    );
    assert!(s.dialog().is_none());
    assert!(archive_of(&dir).exists(), "the archive is written");
    assert_eq!(s.open_revision_label(), None, "V1.0 is released");
    assert_eq!(s.released_labels(), vec!["V1.0".to_owned()]);
    assert!(
        s.notifications()
            .visible()
            .iter()
            .any(|n| n.text == "Released revision V1.0 as V1.0.ewarchive"),
        "the toast"
    );

    // Withdraw: the last released revision, a reason required.
    done(&mut s, "revision.withdraw", json!({}));
    assert_eq!(
        form(&s).1,
        vec![
            ("revision".to_owned(), "V1.0".to_owned()),
            ("reason".to_owned(), String::new())
        ]
    );
    assert_eq!(
        failed(&mut s, "revision.withdraw", json!({ "confirm": true })),
        "Give the reason for the withdrawal."
    );
    assert_eq!(
        failed(
            &mut s,
            "revision.withdraw",
            json!({ "revision": "V9.9", "reason": "wrong", "confirm": true })
        ),
        "V9.9 is not a released revision of this project."
    );
    done(
        &mut s,
        "revision.withdraw",
        json!({ "revision": "V1.0", "reason": "issued in error", "confirm": true }),
    );
    assert!(s.dialog().is_none());
    assert!(s.released_labels().is_empty(), "V1.0 is withdrawn");
    s.shutdown();
}

/// The incomplete-release offer at open (§9): an archive of the open revision's label in the archive root (a release
/// that stopped before its stamp, here the archive of a released copy) is offered when the project opens: Not now (the
/// Primary) and Complete release; no Discard archive, since no run record of this folder proves it its own. A scripted
/// shell's Complete release is refused with the person text (cycle 41, ruling 1); the window's completes the release.
#[test]
fn the_incomplete_release_is_offered_at_open_and_completed_by_a_person() {
    let tmp = Tmp::new("revision-incomplete");
    let dir = fixture_copy(&tmp);
    let mut s = opened(ShellModel::new(Mode::Gui, config(&tmp)), &dir);
    done(
        &mut s,
        "revision.approve",
        json!({ "role": "checker", "typed": "V1.0", "date": DATE, "confirm": true }),
    );
    s.shutdown();
    let other = tmp.0.join("other");
    copy_dir(&dir, &other);
    let mut s = opened(ShellModel::new(Mode::Gui, config(&tmp)), &other);
    done(
        &mut s,
        "revision.release",
        json!({ "typed": "V1.0", "date": DATE, "confirm": true }),
    );
    s.shutdown();
    std::fs::create_dir_all(archive_of(&dir).parent().unwrap()).unwrap();
    std::fs::copy(
        tmp.0.join("other.ew-archive").join("V1.0.ewarchive"),
        archive_of(&dir),
    )
    .unwrap();

    let mut h = opened(ShellModel::new_scripted(config(&tmp)), &dir);
    assert_eq!(form(&h).0, "revision.incomplete", "offered at open");
    assert!(h.has_incomplete_release());
    let d = h.dialog().cloned().unwrap();
    let offer = spec(&d, &h);
    assert_eq!(offer.title, "Settle the incomplete release of V1.0?");
    assert_eq!(offer.primary.answer, DialogAnswer::Dismiss);
    assert_eq!(
        offer
            .secondary
            .iter()
            .map(|b| (b.label.as_str(), b.answer.clone()))
            .collect::<Vec<_>>(),
        vec![(
            "Complete release",
            DialogAnswer::Action(
                "revision.incomplete",
                json!({ "how": "complete", "confirm": true })
            )
        )]
    );
    assert!(offer.danger.is_none(), "not provably this folder's: no discard");
    assert_eq!(
        failed(
            &mut h,
            "revision.incomplete",
            json!({ "how": "complete", "confirm": true })
        ),
        PERSON
    );
    assert_eq!(h.open_revision_label().as_deref(), Some("V1.0"));
    h.shutdown();

    let mut s = opened(ShellModel::new(Mode::Gui, config(&tmp)), &dir);
    assert_eq!(form(&s).0, "revision.incomplete");
    done(
        &mut s,
        "revision.incomplete",
        json!({ "how": "complete", "confirm": true }),
    );
    assert!(s.dialog().is_none());
    assert!(!s.has_incomplete_release());
    assert_eq!(s.open_revision_label(), None, "V1.0 is released with the archive");
    assert_eq!(s.released_labels(), vec!["V1.0".to_owned()]);
    s.shutdown();
}

/// FT-842 (cycle 41, ruling 2: "After a release, the window drains them and shows each once as a Messages entry in
/// words, with the file name only. The entry has severity Warning, and its rule column names the source in a word
/// ("Release"), not an ID. One toast points to Messages in its text"): a state directory whose release records
/// cannot be read gives one warning under Warnings ▸ Release and one toast; an open without trouble shows none.
#[test]
fn ft842_the_release_warnings_show_once_in_messages() {
    let tmp = Tmp::new("revision-ft842");
    let dir = fixture_copy(&tmp);
    // No trouble: nothing in Messages, no toast.
    let mut s = opened(ShellModel::new(Mode::Gui, config(&tmp)), &dir);
    done(&mut s, "messages.show", json!({ "on": true }));
    let t = s.nav(NavKind::Messages).and_then(|n| n.table()).unwrap();
    assert!(t.nodes.iter().all(|n| !n.key.contains("rule:release")));
    assert!(
        s.notifications()
            .visible()
            .iter()
            .all(|n| !n.text.contains("see Messages"))
    );
    s.shutdown();
    // The folder of the release records is a file: the records cannot be read when the project opens.
    let releases = tmp.0.join("config").join("state").join("releases");
    let _ = std::fs::remove_dir_all(&releases);
    std::fs::create_dir_all(releases.parent().unwrap()).unwrap();
    std::fs::write(&releases, b"not a folder").unwrap();
    let error = std::fs::read_dir(&releases).unwrap_err();
    let mut s = opened(ShellModel::new(Mode::Gui, config(&tmp)), &dir);
    done(&mut s, "messages.show", json!({ "on": true }));
    let t = s.nav(NavKind::Messages).and_then(|n| n.table()).unwrap();
    let rows: Vec<_> = t
        .nodes
        .iter()
        .filter(|n| n.key.starts_with("severity:warning/rule:release/"))
        .collect();
    assert_eq!(rows.len(), 1, "one entry");
    let row = rows[0];
    assert_eq!(row.role, NavRole::Finding);
    assert_eq!(
        row.cells,
        vec![
            "warning".to_owned(),
            "Release".to_owned(),
            format!("The release records in the folder releases could not be read: {error}"),
            String::new(),
            String::new(),
            String::new(),
            "open".to_owned(),
        ]
    );
    let group = &t.nodes[row.parent.unwrap() as usize];
    assert_eq!((group.label.as_str(), group.role), ("Release", NavRole::Rule));
    assert_eq!(t.nodes[group.parent.unwrap() as usize].key, "severity:warning");
    let toasts: Vec<_> = s
        .notifications()
        .visible()
        .iter()
        .filter(|n| n.text.contains("see Messages"))
        .map(|n| n.text.clone())
        .collect();
    assert_eq!(
        toasts,
        vec!["The release left housekeeping warnings (1): see Messages".to_owned()]
    );
    // Shown once: another fetch of the table keeps the one entry and adds no toast.
    done(&mut s, "messages.show", json!({ "on": false }));
    done(&mut s, "messages.show", json!({ "on": true }));
    let t = s.nav(NavKind::Messages).and_then(|n| n.table()).unwrap();
    assert_eq!(
        t.nodes
            .iter()
            .filter(|n| n.key.starts_with("severity:warning/rule:release/"))
            .count(),
        1
    );
    assert_eq!(
        s.notifications()
            .visible()
            .iter()
            .filter(|n| n.text.contains("see Messages"))
            .count(),
        1
    );
    s.shutdown();
}

/// Cycle 41, ruling 4 ("the purge's origin": a scripted purge is refused): the scripted window opens the purge
/// confirmation but its confirm is refused and deletes nothing; the headless shell refuses the action.
#[test]
fn ruling4_a_scripted_purge_is_refused() {
    let tmp = Tmp::new("revision-purge");
    let dir = tmp.0.join("small.ew");
    let logo = |bytes: &[u8]| json!({"cmd": "project.company.set", "params": {"logo": ew_viewmodel::ew_api::base64_encode(bytes)}});
    {
        let mut s = ShellModel::new(Mode::Gui, config(&tmp));
        done(
            &mut s,
            "file.new",
            json!({"dir": dir.to_string_lossy(), "template": "builtin"}),
        );
        done(
            &mut s,
            "command.run",
            logo(br#"<svg viewBox="0 0 8 2"><rect width="8" height="2"/></svg>"#),
        );
        done(&mut s, "file.save", json!({}));
        done(
            &mut s,
            "command.run",
            logo(br#"<svg viewBox="0 0 3 3"><circle r="1"/></svg>"#),
        );
        done(&mut s, "file.save", json!({}));
        s.shutdown();
    }
    let files = || -> usize { std::fs::read_dir(dir.join("assets")).unwrap().count() };
    let before = files();
    let mut s = opened(ShellModel::new_scripted(config(&tmp)), &dir);
    done(&mut s, "project.purge_assets", json!({}));
    assert!(matches!(s.dialog(), Some(Dialog::ConfirmPurge { .. })));
    assert_eq!(
        refused(&mut s, "project.purge_assets", json!({ "confirm": true })),
        "reason-human-only"
    );
    assert_eq!(files(), before, "nothing was purged");
    s.shutdown();
    let mut h = opened(ShellModel::new(Mode::Headless, config(&tmp)), &dir);
    assert_eq!(
        refused(&mut h, "project.purge_assets", json!({ "confirm": true })),
        "reason-human-only"
    );
    assert_eq!(files(), before);
    h.shutdown();
}
