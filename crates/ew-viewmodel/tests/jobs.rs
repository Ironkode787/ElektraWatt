// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.09 part 1 (§10.1): the PDF export as a job (JB3 to JB5), the notifications and their log (JB6), and the exact
//! counts of §9 item 5. The flows that need a held gate or a fault run with `ShellModel::set_job_gate_for_tests` and
//! `set_job_panic_for_tests` (feature `testkit`). The byte checks against Golden A's digest are `ew-api`'s
//! `tests/export_job.rs` (JB2) and `bin/elektrawatt`'s headless test; here a one-page project keeps the jobs short.

mod common;

use std::path::Path;

use common::{Tmp, done, refused, state};
use ew_viewmodel::ew_api::ew_jobs::Gate;
use ew_viewmodel::jobs::JobStatus;
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};

/// A headless shell with a new one-page project (the builtin template, folio 101).
fn small(tmp: &Tmp, mode: Mode) -> ShellModel {
    let mut s = ShellModel::new(mode, ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"));
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("small.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Power"}),
    );
    s
}

fn path_text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn sibling(target: &Path) -> std::path::PathBuf {
    let name = target.file_name().unwrap().to_string_lossy();
    target.with_file_name(format!(".{name}.ew-part"))
}

fn failed(shell: &mut ShellModel, action: &str, params: Value) -> String {
    match shell.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action} {params}: {other:?}"),
    }
}

fn last_status(shell: &ShellModel) -> String {
    shell.status_fields(false).last().cloned().unwrap_or_default()
}

#[test]
fn jb3_a_held_export_shows_running_and_the_job_field_and_a_cancel_leaves_no_file() {
    let tmp = Tmp::new("jb3");
    let mut s = small(&tmp, Mode::Headless);
    let gate = Gate::new();
    gate.hold("page");
    s.set_job_gate_for_tests(Some(gate.clone()));
    done(&mut s, "jobs.show", json!({}));
    let target = tmp.0.join("out.pdf");
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&target), "wait": false}),
    );
    gate.wait_reached("page");
    let d = s.describe();
    assert!(
        d.contains("jobs running 1\n  job 1 pdf \"out.pdf\" running"),
        "{d}"
    );
    assert_eq!(last_status(&s), "PDF export", "the job field, no number headless");
    assert_eq!(
        state(&s)["status"].as_array().unwrap().last().unwrap(),
        "PDF export"
    );
    // The UI thread is not blocked: other actions run while the job waits at its gate.
    done(&mut s, "ui.describe", json!({}));
    done(&mut s, "jobs.cancel", json!({}));
    gate.release();
    done(&mut s, "jobs.wait", json!({}));
    let d = s.describe();
    assert!(
        d.contains("jobs running 0\n  job 1 pdf \"out.pdf\" cancelled"),
        "{d}"
    );
    assert!(!target.exists(), "no file");
    assert!(!sibling(&target).exists(), "no sibling");
    assert_eq!(s.toast().unwrap().text, "Export cancelled; no file written");
    assert!(d.contains("toast \"Export cancelled; no file written\""), "{d}");
    assert_ne!(last_status(&s), "PDF export", "no job field once it ended");
    assert!(s.markers().is_empty(), "a cancelled export adds no marker");
    s.shutdown();
}

#[test]
fn jb3_a_cancel_after_the_sibling_is_written_removes_it() {
    let tmp = Tmp::new("jb3-rename");
    let mut s = small(&tmp, Mode::Headless);
    let gate = Gate::new();
    gate.hold("rename");
    s.set_job_gate_for_tests(Some(gate.clone()));
    let target = tmp.0.join("out.pdf");
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&target), "wait": false}),
    );
    gate.wait_reached("rename");
    assert!(sibling(&target).exists(), "the bytes are in the hidden sibling");
    assert!(!target.exists(), "the target only comes with the rename");
    done(&mut s, "jobs.cancel", json!({"job": 1}));
    gate.release();
    done(&mut s, "jobs.wait", json!({"job": 1}));
    assert!(!target.exists());
    assert!(!sibling(&target).exists(), "the cancel removed the sibling");
    s.shutdown();
}

#[test]
fn jb3_a_panic_after_the_sibling_is_written_is_reported_and_leaves_no_file() {
    let tmp = Tmp::new("jb3-panic");
    let mut s = small(&tmp, Mode::Headless);
    s.set_job_panic_for_tests(Some("sibling"));
    let target = tmp.0.join("out.pdf");
    let rev = s.revision();
    let message = failed(&mut s, "file.export_pdf", json!({"path": path_text(&target)}));
    assert_eq!(message, "Export failed: an internal error");
    assert!(!target.exists());
    assert!(!sibling(&target).exists(), "the unwinding removed the sibling");
    // The session is untouched and the shell goes on.
    assert_eq!(s.revision(), rev);
    done(&mut s, "jobs.show", json!({}));
    let d = s.describe();
    assert!(
        d.contains("job 1 pdf \"out.pdf\" failed \"an internal error\""),
        "{d}"
    );
    s.set_job_panic_for_tests(None);
    done(&mut s, "file.export_pdf", json!({"path": path_text(&target)}));
    assert!(target.exists());
    s.shutdown();
}

#[test]
fn jb4_the_replace_rule_and_the_marker() {
    let tmp = Tmp::new("jb4");
    let mut s = small(&tmp, Mode::Headless);
    let target = tmp.0.join("out.pdf");
    // A new file: no question.
    done(&mut s, "file.export_pdf", json!({"path": path_text(&target)}));
    let bytes = std::fs::read(&target).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    let rev = s.revision().unwrap();
    assert_eq!(s.markers().len(), 1);
    assert_eq!(s.markers()[0].text, "Exported PDF to out.pdf");
    assert_eq!(s.markers()[0].rev, rev);
    let kib = bytes.len().div_ceil(1024);
    assert_eq!(
        s.toast().unwrap().text,
        format!("Exported out.pdf: {} pages, {kib} KiB", s.page_count())
    );
    // An existing file without `replace`: refused, unchanged.
    std::fs::write(&target, b"not a pdf").unwrap();
    assert_eq!(
        refused(&mut s, "file.export_pdf", json!({"path": path_text(&target)})),
        "reason-file-exists"
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"not a pdf");
    // With `replace: true` it is replaced.
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&target), "replace": true}),
    );
    assert_eq!(std::fs::read(&target).unwrap(), bytes, "the same export again");
    assert!(!sibling(&target).exists());
    assert_eq!(s.markers().len(), 2);
    s.shutdown();
}

#[test]
fn jb4_the_window_path_asks_the_save_dialog_whose_answer_carries_replace() {
    let tmp = Tmp::new("jb4-window");
    let mut s = small(&tmp, Mode::Gui);
    done(&mut s, "file.export_pdf", json!({}));
    let prompt = s.take_prompt().expect("the save dialog");
    assert_eq!(prompt.action, "file.export_pdf");
    assert_eq!(prompt.param, "path");
    assert_eq!(prompt.params["replace"], true, "{:?}", prompt.params);
    s.shutdown();
}

#[test]
fn jb4_a_second_export_while_one_runs_is_refused() {
    let tmp = Tmp::new("jb4-second");
    let mut s = small(&tmp, Mode::Headless);
    let gate = Gate::new();
    gate.hold("page");
    s.set_job_gate_for_tests(Some(gate.clone()));
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&tmp.0.join("a.pdf")), "wait": false}),
    );
    gate.wait_reached("page");
    assert_eq!(
        refused(
            &mut s,
            "file.export_pdf",
            json!({"path": path_text(&tmp.0.join("b.pdf"))})
        ),
        "reason-export-running"
    );
    assert_eq!(
        failed(&mut s, "jobs.cancel", json!({"job": 7})),
        "There is no job 7; the jobs are 1."
    );
    done(&mut s, "jobs.cancel", json!({}));
    gate.release();
    done(&mut s, "jobs.wait", json!({}));
    assert_eq!(refused(&mut s, "jobs.cancel", json!({})), "reason-no-running-job");
    assert_eq!(
        refused(&mut s, "jobs.cancel", json!({"job": 1})),
        "reason-job-finished"
    );
    s.shutdown();
}

#[test]
fn jb5_a_target_in_a_missing_folder_fails_with_the_cause_and_the_remedy() {
    let tmp = Tmp::new("jb5-missing");
    let mut s = small(&tmp, Mode::Headless);
    let target = tmp.0.join("missing").join("out.pdf");
    let message = failed(&mut s, "file.export_pdf", json!({"path": path_text(&target)}));
    assert_eq!(
        message,
        "Export failed: the folder does not exist; check that the folder is writable and the file is not open in \
         another program"
    );
    assert!(!message.contains(&*path_text(&tmp.0)), "no path");
    done(&mut s, "jobs.show", json!({}));
    let d = s.describe();
    assert!(
        d.contains("job 1 pdf \"out.pdf\" failed \"the folder does not exist\""),
        "{d}"
    );
    assert!(!sibling(&target).exists());
    s.shutdown();
}

#[test]
fn jb5_a_failed_rename_removes_the_sibling() {
    let tmp = Tmp::new("jb5-rename");
    let mut s = small(&tmp, Mode::Headless);
    // The target is a folder: the sibling is written, the rename over the folder fails.
    let target = tmp.0.join("folder.pdf");
    std::fs::create_dir_all(target.join("inside")).unwrap();
    let message = failed(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&target), "replace": true}),
    );
    assert!(message.starts_with("Export failed: "), "{message}");
    assert!(!message.contains(&*path_text(&tmp.0)), "no path: {message}");
    assert!(!sibling(&target).exists(), "the failure removed the sibling");
    assert!(target.join("inside").is_dir(), "the folder is untouched");
    s.shutdown();
}

#[test]
fn jb5_a_result_after_an_edit_during_the_job_names_both_revisions() {
    let tmp = Tmp::new("jb5-rev");
    let mut s = small(&tmp, Mode::Headless);
    let gate = Gate::new();
    gate.hold("written");
    s.set_job_gate_for_tests(Some(gate.clone()));
    done(&mut s, "jobs.show", json!({}));
    let target = tmp.0.join("out.pdf");
    let rev = s.revision().unwrap();
    let pages = s.page_count();
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&target), "wait": false}),
    );
    gate.wait_reached("written");
    // An edit while the job writes: the job keeps its snapshot.
    done(
        &mut s,
        "page.new",
        json!({"folio": "102", "type": "multi_line", "title": "Control"}),
    );
    let now = s.revision().unwrap();
    assert_ne!(now, rev);
    gate.release();
    done(&mut s, "jobs.wait", json!({}));
    let kib = std::fs::metadata(&target).unwrap().len().div_ceil(1024);
    let d = s.describe();
    assert!(
        d.contains(&format!(
            "job 1 pdf \"out.pdf\" done pages {pages} kib {kib} rev {rev} now {now}"
        )),
        "{d}"
    );
    // The marker sits at the revision the export ran on.
    assert_eq!(s.markers()[0].rev, rev);
    s.shutdown();
}

#[test]
fn jb5_polling_takes_the_end_of_a_job_that_was_not_waited_for() {
    let tmp = Tmp::new("jb5-poll");
    let mut s = small(&tmp, Mode::Headless);
    let target = tmp.0.join("out.pdf");
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&target), "wait": false}),
    );
    for _ in 0..2000 {
        s.poll_jobs();
        if s.jobs().running() == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(s.jobs().running(), 0, "the job ended");
    assert!(target.exists());
    let pages = s.page_count();
    assert!(
        s.toast()
            .unwrap()
            .text
            .starts_with(&format!("Exported out.pdf: {pages} pages"))
    );
    s.shutdown();
}

/// §9 item 5: an export makes one `export_job` and no other `Session` call; polling, the log and the Jobs pane make
/// none.
#[test]
fn jb9_the_exact_counts_of_an_export() {
    let tmp = Tmp::new("jb9-counts");
    let mut s = small(&tmp, Mode::Headless);
    let before = s.session_calls();
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&tmp.0.join("out.pdf"))}),
    );
    let mut expected = before;
    expected.export_job += 1;
    assert_eq!(s.session_calls(), expected);
    s.poll_jobs();
    done(&mut s, "notifications.show", json!({}));
    done(&mut s, "jobs.show", json!({}));
    done(&mut s, "notifications.show", json!({"on": false}));
    done(&mut s, "toast.dismiss", json!({}));
    assert_eq!(s.session_calls(), expected, "no Session call");
    s.shutdown();
}

/// The notifications of a one-page project: an export's Open is refused headless, unknown entries and actions are
/// named, and the log popup closes with Escape.
#[test]
fn jb6_the_log_popup_and_its_refusals() {
    let tmp = Tmp::new("jb6-log");
    let mut s = small(&tmp, Mode::Headless);
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&tmp.0.join("out.pdf"))}),
    );
    done(&mut s, "notifications.show", json!({}));
    let d = s.describe();
    let kib = std::fs::metadata(tmp.0.join("out.pdf"))
        .unwrap()
        .len()
        .div_ceil(1024);
    assert!(
        d.contains(&format!(
            "notifications 1\n  notification 1 \"Exported out.pdf: {} pages, {kib} KiB\" open",
            s.page_count()
        )),
        "{d}"
    );
    assert!(d.contains("| Notifications 1"), "the status bar's field: {d}");
    assert_eq!(
        refused(&mut s, "notifications.run", json!({"entry": 1, "action": "open"})),
        "reason-no-opener"
    );
    assert_eq!(refused(&mut s, "jobs.open_result", json!({})), "reason-no-opener");
    assert_eq!(
        failed(&mut s, "notifications.run", json!({"entry": 9, "action": "open"})),
        "There is no notification 9; the log holds 1."
    );
    assert_eq!(
        failed(&mut s, "notifications.run", json!({"entry": 1, "action": "undo"})),
        "The notification offers no undo action."
    );
    done(&mut s, "ui.dismiss", json!({}));
    assert!(!s.notifications().popup_open(), "Escape closes the log");
    // A second export's toast replaces the first (key `export`); the log keeps both.
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&tmp.0.join("two.pdf"))}),
    );
    assert_eq!(s.notifications().visible().len(), 1);
    assert_eq!(s.notifications().len(), 2);
    s.shutdown();
}

/// A window shell hands Open PDF to the window as a `file://` URL of the written file.
#[test]
fn jb6_open_pdf_in_the_window_is_a_file_url_request() {
    let tmp = Tmp::new("jb6-open");
    let mut s = small(&tmp, Mode::Gui);
    let target = tmp.0.join("a b.pdf");
    done(&mut s, "file.export_pdf", json!({"path": path_text(&target)}));
    // The window never waits: the end arrives by polling.
    for _ in 0..2000 {
        s.poll_jobs();
        if s.jobs().running() == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    done(&mut s, "jobs.open_result", json!({}));
    let url = s.take_open_request().expect("an open request");
    assert!(url.starts_with("file:///") || url.starts_with("file://"), "{url}");
    assert!(url.ends_with("/a%20b.pdf"), "{url}");
    done(&mut s, "notifications.run", json!({"entry": 1, "action": "open"}));
    assert!(s.take_open_request().is_some());
    s.shutdown();
}

/// The texts of the notifications log, oldest first.
fn log_texts(s: &ShellModel) -> Vec<String> {
    s.notifications().entries().map(|n| n.text.clone()).collect()
}

/// ID-719: Quit (`shutdown`) cancels a running export, waits for it and records its real end like any other: a job
/// that met its cancel is cancelled, its entry and the notifications log say so, and no file is left.
#[test]
fn quit_records_a_cancelled_export_in_its_entry_and_the_log() {
    let tmp = Tmp::new("quit-cancel");
    let mut s = small(&tmp, Mode::Gui);
    let gate = Gate::new();
    gate.hold("page");
    s.set_job_gate_for_tests(Some(gate.clone()));
    let target = tmp.0.join("out.pdf");
    done(&mut s, "file.export_pdf", json!({"path": path_text(&target)}));
    gate.wait_reached("page");
    // The cancel flag is set before the job goes on, so its end is the cancel; nobody takes it before the quit.
    done(&mut s, "jobs.cancel", json!({}));
    gate.release();
    s.shutdown();
    assert_eq!(
        s.jobs().get(1).map(|e| e.status.clone()),
        Some(JobStatus::Cancelled)
    );
    assert_eq!(log_texts(&s), ["Export cancelled; no file written"]);
    assert!(!target.exists(), "no file");
    assert!(!sibling(&target).exists(), "no sibling");
}

/// ID-719: a job that had already written its file when the quit came keeps its real end: done, not cancelled, with
/// its notification in the log.
#[test]
fn quit_keeps_the_end_of_an_export_that_wrote_its_file() {
    let tmp = Tmp::new("quit-done");
    let mut s = small(&tmp, Mode::Gui);
    let target = tmp.0.join("out.pdf");
    done(&mut s, "file.export_pdf", json!({"path": path_text(&target)}));
    // The window never waits and nothing polls: the end is still in the job when the shell ends. After the rename the
    // body has no checkpoint left, so the cancel cannot change its end.
    for _ in 0..2000 {
        if target.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(target.exists(), "the file is written");
    assert_eq!(s.jobs().running(), 1, "its end is not taken yet");
    s.shutdown();
    assert!(matches!(
        s.jobs().get(1).map(|e| e.status.clone()),
        Some(JobStatus::Done { .. })
    ));
    let texts = log_texts(&s);
    assert_eq!(texts.len(), 1, "{texts:?}");
    assert!(texts[0].starts_with("Exported out.pdf: "), "{texts:?}");
}

/// Review finding 5 of cycle 30: an export target inside the open project's folder is refused, with or without
/// `replace`, and nothing there changes; a folder whose name only starts with the project's is not inside it.
#[test]
fn an_export_into_the_open_projects_folder_is_refused() {
    let tmp = Tmp::new("into-project");
    let mut s = small(&tmp, Mode::Headless);
    let project = tmp.0.join("small.ew");
    let manifest = project.join("manifest.json");
    let before = std::fs::read(&manifest).unwrap();
    assert_eq!(
        refused(
            &mut s,
            "file.export_pdf",
            json!({"path": path_text(&project.join("out.pdf"))})
        ),
        "reason-export-into-project"
    );
    assert!(!project.join("out.pdf").exists());
    assert_eq!(
        refused(
            &mut s,
            "file.export_pdf",
            json!({"path": path_text(&manifest), "replace": true})
        ),
        "reason-export-into-project"
    );
    assert_eq!(
        std::fs::read(&manifest).unwrap(),
        before,
        "the project is unchanged"
    );
    // A relative spelling of the same folder is inside it too.
    let dotted = project.join("..").join("small.ew").join("x.pdf");
    assert_eq!(
        refused(&mut s, "file.export_pdf", json!({"path": path_text(&dotted)})),
        "reason-export-into-project"
    );
    assert!(s.jobs().entries().next().is_none(), "no job started");
    let beside = tmp.0.join("small.ew2");
    std::fs::create_dir_all(&beside).unwrap();
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&beside.join("out.pdf"))}),
    );
    assert!(beside.join("out.pdf").exists());
    s.shutdown();
}

/// FT-513: the end of a `wait: false` export that failed, was cancelled or panicked and that no step waited for is
/// returned by `wait_jobs` (the headless runner prints it and exits 1), one text per end, in job order; an end a step
/// waited for (`jobs.wait`, a waiting export) is not, and neither is a done one.
#[test]
fn ft513_wait_jobs_returns_the_unseen_failed_cancelled_and_panicked_ends() {
    let tmp = Tmp::new("ft513");
    let mut s = small(&tmp, Mode::Headless);
    // Failed: a target in a missing folder, its end taken at the end of the run.
    let missing = tmp.0.join("missing").join("out.pdf");
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&missing), "wait": false}),
    );
    let failed_text = "Export failed: the folder does not exist; check that the folder is writable and the file is \
                       not open in another program";
    assert_eq!(s.wait_jobs(), vec![failed_text.to_owned()]);
    // Cancelled: held, cancelled, released; the end is taken by `wait_jobs` again.
    let gate = Gate::new();
    gate.hold("page");
    s.set_job_gate_for_tests(Some(gate.clone()));
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&tmp.0.join("b.pdf")), "wait": false}),
    );
    gate.wait_reached("page");
    done(&mut s, "jobs.cancel", json!({}));
    gate.release();
    s.set_job_gate_for_tests(None);
    let cancelled_text = "Export cancelled; no file written";
    assert_eq!(
        s.wait_jobs(),
        vec![failed_text.to_owned(), cancelled_text.to_owned()],
        "the earlier unseen end stays in the list"
    );
    // Panicked, its end taken by a poll before a later step: still unseen.
    s.set_job_panic_for_tests(Some("sibling"));
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&tmp.0.join("c.pdf")), "wait": false}),
    );
    while s.jobs().running() > 0 {
        s.poll_jobs();
        std::thread::yield_now();
    }
    s.set_job_panic_for_tests(None);
    let internal_text = "Export failed: an internal error";
    assert_eq!(
        s.wait_jobs(),
        vec![
            failed_text.to_owned(),
            cancelled_text.to_owned(),
            internal_text.to_owned()
        ]
    );
    s.shutdown();
}

/// FT-513: a failed end that a step waited for is the step's outcome (or, for `jobs.wait`, its entry's), never an
/// unseen failure; a done export is none either.
#[test]
fn ft513_a_waited_failure_and_a_done_export_are_not_unseen() {
    let tmp = Tmp::new("ft513-seen");
    let mut s = small(&tmp, Mode::Headless);
    let missing = tmp.0.join("missing").join("out.pdf");
    let _ = failed(&mut s, "file.export_pdf", json!({"path": path_text(&missing)}));
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&missing), "wait": false}),
    );
    done(&mut s, "jobs.wait", json!({}));
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&tmp.0.join("ok.pdf")), "wait": false}),
    );
    assert!(s.wait_jobs().is_empty());
    assert!(tmp.0.join("ok.pdf").exists());
    s.shutdown();
}

/// Polls until no job runs (the end is taken by a poll, as the script runner's poll before a step may take it).
fn poll_to_the_end(s: &mut ShellModel) {
    while s.jobs().running() > 0 {
        s.poll_jobs();
        std::thread::yield_now();
    }
}

/// R1-2 of cycle 31: a `jobs.wait` after a poll already took the failed or cancelled end still sees it, with and
/// without `job`, so whether the runner's poll or the wait took the end does not change the run's outcome.
#[test]
fn ft513_a_wait_after_a_poll_took_the_end_still_sees_it() {
    let tmp = Tmp::new("ft513-polled");
    let mut s = small(&tmp, Mode::Headless);
    let missing = tmp.0.join("missing").join("out.pdf");
    // Failed, `jobs.wait {}`.
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&missing), "wait": false}),
    );
    poll_to_the_end(&mut s);
    done(&mut s, "jobs.wait", json!({}));
    assert!(s.wait_jobs().is_empty(), "jobs.wait {{}} saw the failure");
    // Failed, `jobs.wait {job: 2}`.
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&missing), "wait": false}),
    );
    poll_to_the_end(&mut s);
    done(&mut s, "jobs.wait", json!({"job": 2}));
    assert!(s.wait_jobs().is_empty(), "jobs.wait {{job: 2}} saw the failure");
    // Held, cancelled, released, polled to its end, then `jobs.wait {}`.
    let gate = Gate::new();
    gate.hold("page");
    s.set_job_gate_for_tests(Some(gate.clone()));
    done(
        &mut s,
        "file.export_pdf",
        json!({"path": path_text(&tmp.0.join("b.pdf")), "wait": false}),
    );
    gate.wait_reached("page");
    done(&mut s, "jobs.cancel", json!({}));
    gate.release();
    poll_to_the_end(&mut s);
    s.set_job_gate_for_tests(None);
    done(&mut s, "jobs.wait", json!({}));
    assert!(s.wait_jobs().is_empty(), "jobs.wait {{}} saw the cancel");
    // Nothing left unseen: `jobs.wait {}` is done at once.
    done(&mut s, "jobs.wait", json!({}));
    s.shutdown();
}

/// R2-1 of cycle 31: with two `wait: false` exports, A failed and B done, `jobs.wait {}` chooses B whether a poll
/// already took B's end (a) or B still runs (b): A's failure stays unseen in both, so the run reports the same text
/// and the same exit status. A second `jobs.wait {}` then chooses A and sees its failure.
#[test]
fn ft513_jobs_wait_chooses_the_newest_job_no_step_waited_for_whatever_the_polls_took() {
    let failed_text = "Export failed: the folder does not exist; check that the folder is writable and the file is \
                       not open in another program";
    for polled in [true, false] {
        let tmp = Tmp::new(if polled { "r2-1-polled" } else { "r2-1-running" });
        let mut s = small(&tmp, Mode::Headless);
        // A: fails; the runner's poll takes its end before the next export may start.
        done(
            &mut s,
            "file.export_pdf",
            json!({"path": path_text(&tmp.0.join("missing").join("a.pdf")), "wait": false}),
        );
        poll_to_the_end(&mut s);
        // B: succeeds, held at its first page by the test hook until released.
        let gate = Gate::new();
        gate.hold("page");
        s.set_job_gate_for_tests(Some(gate.clone()));
        let b = tmp.0.join("b.pdf");
        done(
            &mut s,
            "file.export_pdf",
            json!({"path": path_text(&b), "wait": false}),
        );
        gate.wait_reached("page");
        // The hold stays for every later checkpoint `page`: a helper releases until the step is over.
        let stop = std::sync::atomic::AtomicBool::new(false);
        // Sets `stop` on every exit of the scope closure, unwinding included, so a failed assertion fails the
        // test instead of leaving the helper looping (R3-1).
        struct Stop<'a>(&'a std::sync::atomic::AtomicBool);
        impl Drop for Stop<'_> {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        std::thread::scope(|scope| {
            let _stop = Stop(&stop);
            scope.spawn(|| {
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    gate.release();
                    std::thread::yield_now();
                }
            });
            if polled {
                // (a) A poll takes B's end before the step.
                poll_to_the_end(&mut s);
                assert_eq!(s.jobs().running(), 0);
            } else {
                // (b) B still runs for the shell when the step starts (no poll has taken its end).
                assert_eq!(s.jobs().running(), 1);
            }
            done(&mut s, "jobs.wait", json!({}));
        });
        s.set_job_gate_for_tests(None);
        assert!(b.exists(), "B wrote its file");
        assert!(
            matches!(s.jobs().get(2).map(|e| &e.status), Some(JobStatus::Done { .. })),
            "B ended done"
        );
        assert_eq!(
            s.wait_jobs(),
            vec![failed_text.to_owned()],
            "jobs.wait {{}} chose B, so A's failure stays unseen (polled: {polled})"
        );
        // A second `jobs.wait {}` chooses A, the newest job no step waited for, and sees its failure.
        done(&mut s, "jobs.wait", json!({}));
        assert!(s.wait_jobs().is_empty(), "polled: {polled}");
        s.shutdown();
    }
}
