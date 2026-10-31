// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.18 part 5 (RU-3): the revision dialogs of Check ▸ Revision on copies of `golden/fixtures/release-min` (one open
//! revision V1.0 with its author; no finding but EW-REL-001 before the sign-off). The dialogs open, fill, validate and
//! cancel in the headless shell through their own actions; a scripted confirm is refused with its whole text; a
//! person at the window approves, releases and withdraws. FT-842: the release's housekeeping warnings in Messages.
//! Cycle 41, ruling 4: a scripted purge is refused. The fix round FIX41b: every refusal in words (ruling 1), the
//! incomplete release's confirmations with the git consent (ruling 2), counts in their noun's number (ruling 4), the
//! check messages without a command (ruling 5), the release's warnings with their cause in words (ruling 6).

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

    // Release: refused by the gate's EW-REL-001 before the sign-off, so it shows the blocker and Close only. FIX41b
    // ruling 4 ("counts take their noun's number": one finding) and ruling 5 ("the check messages name no command").
    done(&mut s, "revision.release", json!({}));
    assert_eq!(
        dialog_text(&mut s),
        "dialog \"Release revision V1.0?\"\n  label \"The release is blocked by 1 finding of the release gate:\"\n  \
         item \"revision V1.0: checker not set; the checker approves the revision\"\n  button \"Close\""
    );
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

/// A dialog's specification as plain data: its title, paragraphs, Primary, other buttons and danger button, each
/// button as `(label, answer)`.
type Buttons = Vec<(String, DialogAnswer)>;
fn whole_spec(shell: &ShellModel) -> (String, Vec<String>, Buttons, Buttons, Buttons) {
    let d = shell.dialog().cloned().expect("a dialog");
    let s = spec(&d, shell);
    let button = |b: &ew_viewmodel::dialogs::DialogButton| (b.label.clone(), b.answer.clone());
    (
        s.title.clone(),
        s.body.clone(),
        vec![button(&s.primary)],
        s.secondary.iter().map(button).collect(),
        s.danger.iter().map(button).collect(),
    )
}

/// The approved fixture copy `dir` with an incomplete release of V1.0 beside it: the archive of a released copy of it
/// (`<other>/release-min`, outside `dir`'s folder), copied into `dir`'s archive root, as a release that stopped before
/// its stamp leaves it. No run record of `dir` proves it `dir`'s own.
fn incomplete_beside(tmp: &Tmp, dir: &Path, other: &Path) {
    let mut s = opened(ShellModel::new(Mode::Gui, config(tmp)), dir);
    done(
        &mut s,
        "revision.approve",
        json!({ "role": "checker", "typed": "V1.0", "date": DATE, "confirm": true }),
    );
    s.shutdown();
    copy_dir(dir, other);
    let mut s = opened(ShellModel::new(Mode::Gui, config(tmp)), other);
    done(
        &mut s,
        "revision.release",
        json!({ "typed": "V1.0", "date": DATE, "confirm": true }),
    );
    s.shutdown();
    std::fs::create_dir_all(archive_of(dir).parent().unwrap()).unwrap();
    std::fs::copy(archive_of(other), archive_of(dir)).unwrap();
}

/// The incomplete-release offer at open (§9): an archive of the open revision's label in the archive root (a release
/// that stopped before its stamp, here the archive of a released copy) is offered when the project opens: Not now (the
/// Primary) and Complete release; no Discard archive, since no run record of this folder proves it its own.
///
/// Cycle 41, FIX41b ruling 2 ("completing an incomplete release asks, like a release"): Complete release opens its
/// confirmation, Cancel the Primary and Complete release the danger button; outside a git repository it shows no
/// consent toggle. A scripted shell's confirm is refused with the person text (cycle 41, ruling 1); the window's
/// completes the release.
#[test]
fn the_incomplete_release_is_offered_at_open_and_completed_by_a_person() {
    let tmp = Tmp::new("revision-incomplete");
    let dir = fixture_copy(&tmp);
    let other = tmp.0.join("other").join("release-min");
    incomplete_beside(&tmp, &dir, &other);

    let mut h = opened(ShellModel::new_scripted(config(&tmp)), &dir);
    assert_eq!(form(&h).0, "revision.incomplete", "offered at open");
    assert!(h.has_incomplete_release());
    assert_eq!(
        whole_spec(&h),
        (
            "Settle the incomplete release of V1.0?".to_owned(),
            vec![
                "A release of V1.0 stopped before its stamp and left the archive V1.0.ewarchive.".to_owned(),
                "Complete release stamps the revision released with it, committing and tagging it when the project \
                 folder is in a git repository."
                    .to_owned(),
                "It is not provably this folder's own leftover, so it is not offered for discarding; remove the file \
                 by hand if you know it is one."
                    .to_owned(),
            ],
            vec![("Not now".to_owned(), DialogAnswer::Dismiss)],
            vec![(
                "Complete release".to_owned(),
                DialogAnswer::Action("revision.incomplete", json!({ "how": "complete" }))
            )],
            vec![],
        )
    );
    // Not this folder's own: no discard confirmation either.
    assert_eq!(
        failed(&mut h, "revision.incomplete", json!({ "how": "discard" })),
        "It is not provably this folder's own leftover, so it is not offered for discarding; remove the file by hand \
         if you know it is one."
    );
    done(&mut h, "revision.incomplete", json!({ "how": "complete" }));
    let confirmation = (
        "Complete the release of V1.0?".to_owned(),
        vec![
            "Completing stamps V1.0 released with the archive V1.0.ewarchive and saves the project. It cannot be \
             undone."
                .to_owned(),
        ],
        vec![("Cancel".to_owned(), DialogAnswer::Dismiss)],
        vec![],
        vec![(
            "Complete release".to_owned(),
            DialogAnswer::Action(
                "revision.incomplete",
                json!({ "how": "complete", "confirm": true }),
            ),
        )],
    );
    assert_eq!(whole_spec(&h), confirmation);
    assert!(form(&h).1.is_empty(), "no git consent outside a repository");
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
    done(&mut s, "revision.incomplete", json!({ "how": "complete" }));
    assert_eq!(whole_spec(&s), confirmation);
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

/// `git` in `dir` with `args`; its standard output on success.
fn git_in(dir: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Cycle 41, FIX41b ruling 2 ("the consent passed is the toggle's value, never a constant `true`"): in a git repository
/// the confirmation of Complete release shows the consent toggle, off; confirming with it off is refused with the
/// whole text (the release would commit and tag), and nothing is committed or tagged; with it on the release completes
/// and the tag is created. Skipped without a git executable on the path.
#[test]
fn ruling2_completing_in_a_git_repository_passes_the_toggle_as_consent() {
    if git_in(Path::new("."), &["--version"]).is_none() {
        eprintln!("skipped: no git executable on the path");
        return;
    }
    let tmp = Tmp::new("revision-complete-git");
    let repo = tmp.0.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    for args in [
        &["init", "--quiet"][..],
        &["config", "user.name", "ElektraWatt test"],
        &["config", "user.email", "test@example.invalid"],
        &["config", "commit.gpgsign", "false"],
        &["config", "tag.gpgsign", "false"],
        &["config", "core.autocrlf", "false"],
    ] {
        git_in(&repo, args).unwrap_or_else(|| panic!("git {args:?}"));
    }
    std::fs::write(repo.join("README"), "a repository of the revision test\n").unwrap();
    git_in(&repo, &["add", "README"]).unwrap();
    git_in(&repo, &["commit", "--quiet", "-m", "init"]).unwrap();
    let dir = repo.join("release-min");
    copy_dir(Path::new(FIXTURE), &dir);
    let other = tmp.0.join("other").join("release-min");
    incomplete_beside(&tmp, &dir, &other);
    let head = git_in(&repo, &["rev-parse", "HEAD"]);

    let mut s = opened(ShellModel::new(Mode::Gui, config(&tmp)), &dir);
    done(&mut s, "revision.incomplete", json!({ "how": "complete" }));
    let (title, body, ..) = whole_spec(&s);
    assert_eq!(title, "Complete the release of V1.0?");
    assert_eq!(
        body,
        vec![
            "Completing stamps V1.0 released with the archive V1.0.ewarchive and saves the project. It cannot be \
             undone."
                .to_owned(),
            "The project folder is in a git repository: completing commits it and creates the tag ew/rev/V1.0."
                .to_owned(),
        ]
    );
    assert_eq!(
        form(&s).1,
        vec![("git".to_owned(), "false".to_owned())],
        "the toggle, off"
    );
    assert_eq!(
        failed(
            &mut s,
            "revision.incomplete",
            json!({ "how": "complete", "confirm": true })
        ),
        "Tick \u{201c}Commit and tag in git\u{201d}: the project folder is in a git repository, and the release \
         commits it and creates the tag ew/rev/V1.0."
    );
    assert_eq!(git_in(&repo, &["rev-parse", "HEAD"]), head, "nothing committed");
    assert_eq!(git_in(&repo, &["tag", "--list"]).as_deref(), Some(""), "no tag");
    assert_eq!(s.open_revision_label().as_deref(), Some("V1.0"));

    done(
        &mut s,
        "revision.incomplete",
        json!({ "how": "complete", "git": true, "confirm": true }),
    );
    assert!(s.dialog().is_none());
    assert_eq!(s.open_revision_label(), None, "V1.0 is released");
    assert_eq!(
        git_in(&repo, &["tag", "--list"]).as_deref(),
        Some("ew/rev/V1.0\n"),
        "the tag of the consented completion"
    );
    assert_ne!(git_in(&repo, &["rev-parse", "HEAD"]), head, "committed");
    s.shutdown();
}

/// Writes the run record that proves the incomplete release of `dir` its own (as the release that wrote the archive
/// would have kept it before a crash), through the release's own record format (`ew_api::archive`), in the shell's
/// state directory.
fn record_as_own(tmp: &Tmp, dir: &Path) {
    use ew_viewmodel::ew_api::{Recovery, Session, SessionOptions, StoreOptions, Vfs as _, archive};
    let mut options = StoreOptions::new("revision-test");
    options.state_dir = Some(tmp.0.join("config").join("state"));
    let (session, _) = Session::open(dir, options, SessionOptions::default(), Recovery::Keep).expect("opens");
    let found = session
        .incomplete_release(None)
        .expect("looks")
        .expect("the incomplete release");
    let store = session.store().expect("a store");
    let vfs = &**store.vfs();
    let state = store.state_dir().expect("a state directory");
    let (_, generation, _) = found.facts.clone().expect("its facts");
    let record = archive::RunRecord {
        project_dir: archive::path_text(vfs, store.root()),
        archive: archive::path_text(vfs, &found.archive),
        project: session.query().project().id.to_string(),
        revision: found.revision.to_string(),
        label: found.label.clone(),
        generation,
        sha256: found.sha256.to_string(),
        modified: vfs.stat(&found.archive).ok().and_then(|s| s.modified),
        run: "revision-test-run".to_owned(),
    };
    archive::write_run_record(vfs, state, &record).expect("the record");
    drop(session);
}

/// Cycle 41, FIX41b ruling 2 ("Discard archive also gets a danger confirmation with Cancel as the primary button"): a
/// provably own incomplete release offers Discard archive (the danger button of the offer), which opens its
/// confirmation; cancelled there, the archive stays and the release is still incomplete.
#[test]
fn ruling2_a_discard_cancelled_at_its_confirmation_keeps_the_archive() {
    let tmp = Tmp::new("revision-discard");
    let dir = fixture_copy(&tmp);
    let other = tmp.0.join("other").join("release-min");
    incomplete_beside(&tmp, &dir, &other);
    record_as_own(&tmp, &dir);
    let bytes = std::fs::read(archive_of(&dir)).unwrap();

    let mut s = opened(ShellModel::new(Mode::Gui, config(&tmp)), &dir);
    let (title, _, primary, secondary, danger) = whole_spec(&s);
    assert_eq!(title, "Settle the incomplete release of V1.0?");
    assert_eq!(primary, vec![("Not now".to_owned(), DialogAnswer::Dismiss)]);
    assert_eq!(
        secondary,
        vec![(
            "Complete release".to_owned(),
            DialogAnswer::Action("revision.incomplete", json!({ "how": "complete" }))
        )]
    );
    assert_eq!(
        danger,
        vec![(
            "Discard archive".to_owned(),
            DialogAnswer::Action("revision.incomplete", json!({ "how": "discard" }))
        )]
    );
    done(&mut s, "revision.incomplete", json!({ "how": "discard" }));
    assert_eq!(
        whole_spec(&s),
        (
            "Discard the archive of V1.0?".to_owned(),
            vec![
                "Discarding deletes the archive V1.0.ewarchive, and the git tag this release created, if any; V1.0 \
                 stays open. It cannot be undone."
                    .to_owned()
            ],
            vec![("Cancel".to_owned(), DialogAnswer::Dismiss)],
            vec![],
            vec![(
                "Discard archive".to_owned(),
                DialogAnswer::Action("revision.incomplete", json!({ "how": "discard", "confirm": true }))
            )],
        )
    );
    // Cancel (the Primary) closes it: nothing is discarded.
    done(&mut s, "ui.dismiss", Value::Null);
    assert!(s.dialog().is_none());
    assert_eq!(
        std::fs::read(archive_of(&dir)).unwrap(),
        bytes,
        "the archive stays"
    );
    assert!(s.has_incomplete_release());
    assert_eq!(s.open_revision_label().as_deref(), Some("V1.0"));
    // The same through `cancel`: the confirmation closes, the archive stays.
    done(&mut s, "revision.incomplete", json!({ "how": "discard" }));
    done(&mut s, "revision.incomplete", json!({ "cancel": true }));
    assert!(s.dialog().is_none());
    assert_eq!(std::fs::read(archive_of(&dir)).unwrap(), bytes);
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
            // FIX41b ruling 6: the cause in words from the error's kind, never the OS text or its code.
            "The release records in the folder releases could not be read: it is not a folder.".to_owned(),
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

/// Cycle 41, FIX41b ruling 1 ("words, not IDs, in every revision refusal"): the gate that cannot run on unsaved
/// changes, the checker of the author's party and a refused author each read as a whole text in words; the refused
/// author goes through `command_failed` (ID-939's rule: the command by its title, the parameter by its field's words)
/// and stays in the dialog.
#[test]
fn ruling1_the_revision_refusals_read_in_words() {
    let tmp = Tmp::new("revision-words");
    let dir = fixture_copy(&tmp);
    let mut s = opened(ShellModel::new(Mode::Gui, config(&tmp)), &dir);
    done(
        &mut s,
        "revision.set_author",
        json!({ "role": "engineer", "confirm": true }),
    );
    // The gate runs on the saved project: with the new author unsaved it cannot run.
    done(&mut s, "revision.approve", json!({}));
    assert_eq!(
        dialog_text(&mut s),
        "dialog \"Approve revision V1.0\"\n  label \"The release gate cannot run: the project has unsaved changes; \
         save it first.\"\n  button \"Close\""
    );
    done(&mut s, "revision.approve", json!({ "cancel": true }));
    done(&mut s, "file.save", json!({}));
    // The checker of the author's party.
    assert_eq!(
        failed(
            &mut s,
            "revision.approve",
            json!({ "role": "engineer", "typed": "V1.0", "date": DATE, "confirm": true })
        ),
        "The approval was refused: the engineer is the author's party; the four-eyes rule needs a checker of another \
         party."
    );
    done(&mut s, "revision.approve", json!({ "cancel": true }));
    // Approved by the checker; the author may not then take the checker's role.
    done(
        &mut s,
        "revision.approve",
        json!({ "role": "checker", "typed": "V1.0", "date": DATE, "confirm": true }),
    );
    let refusal = failed(
        &mut s,
        "revision.set_author",
        json!({ "role": "checker", "confirm": true }),
    );
    assert_eq!(
        refusal,
        "Set revision author: Role: the author is the checker's party; four eyes need another party (D-34, \
         FR-REV-005)"
    );
    assert_eq!(
        form(&s).2.as_deref(),
        Some(refusal.as_str()),
        "kept in the dialog"
    );
    s.shutdown();
}

/// The number of findings a revision dialog's list stands for: its lines, or the listed ones and "and <n> more".
fn listed_count(list: &[String]) -> usize {
    match list.last().and_then(|l| {
        l.strip_prefix("and ")?
            .strip_suffix(" more")?
            .parse::<usize>()
            .ok()
    }) {
        Some(more) => list.len() - 1 + more,
        None => list.len(),
    }
}

/// Cycle 41, FIX41b rulings 1 and 4: on a copy of Golden A (V2.0 open, its author set, many errors) the blocked Approve
/// names the four-eyes rule by its title and the count in the plural, and the blocked Release counts its findings in
/// the plural (the approval's errors and the four-eyes finding). Golden A's totals are not pinned: the count is the
/// dialog's own list.
#[test]
fn ruling1_ruling4_golden_a_counts_its_blockers_in_words() {
    let tmp = Tmp::new("revision-golden");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut s, "revision.approve", json!({}));
    let errors = listed_count(s.revision_list());
    assert!(errors >= 2, "several errors: {:?}", s.revision_list());
    let (body, label, confirmable) = s.revision_dialog().expect("the Approve form");
    assert_eq!(
        (body.to_vec(), label, confirmable),
        (
            vec![format!(
                "The release gate has {errors} errors besides the four-eyes rule; correct or waive them first:"
            )],
            "V2.0",
            false
        )
    );
    done(&mut s, "revision.approve", json!({ "cancel": true }));
    done(&mut s, "revision.release", json!({}));
    let blockers = listed_count(s.revision_list());
    let (body, _, confirmable) = s.revision_dialog().expect("the Release form");
    assert_eq!(
        (body.to_vec(), confirmable),
        (
            vec![format!(
                "The release is blocked by {blockers} findings of the release gate:"
            )],
            false
        )
    );
    assert_eq!(
        blockers,
        errors + 1,
        "the approval's errors and the four-eyes finding"
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
