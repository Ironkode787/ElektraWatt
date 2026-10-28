// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shell's part of WP-S2.09 part 1: the PDF export as a job (§6.3) with its entries and actions (§6.4), the
//! notifications and their log (§6.5), and the recovery dialog's answers (§6.6).
//!
//! **Threads.** The `Session` never leaves this thread: `file.export_pdf` takes `Session::export_job` (a snapshot) and
//! hands it to a job of `ew_jobs`; nothing on the job's thread touches the shell. The end of a job is taken by
//! [`ShellModel::poll_jobs`] (the window calls it every frame, the script runner before every step) or, headless, by a
//! waiting `file.export_pdf` or `jobs.wait`. Polling makes no `Session` call.

use std::path::{Path, PathBuf};

#[cfg(feature = "testkit")]
use ew_api::ew_jobs::Gate;
use ew_api::ew_jobs::{JobEnd, Waker};
use ew_api::output::ExportOptions;
use ew_api::{Recovery, Session};
use serde_json::{Map, Value};

use super::{
    AccessMode, Dialog, HistoryMarker, Mode, Outcome, Project, Prompt, Record, ShellModel, dir_name, failed,
    open_error_key, string_param,
};
use crate::jobs::{ExportDone, JobStatus, JobsState};
use crate::layout::PaneKind;
use crate::notifications::{NoteAction, Notification, NotificationLog, Validity};
use crate::strings;

/// A `uint` parameter: `Ok(None)` when absent, `Err` when present with another type.
fn uint_param(p: &Map<String, Value>, name: &str) -> Result<Option<u64>, Outcome> {
    match p.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .map(Some)
            .ok_or_else(|| failed("error-bad-param", &[name])),
    }
}

/// A `bool` parameter with its default.
pub(super) fn bool_param(p: &Map<String, Value>, name: &str, default: bool) -> Result<bool, Outcome> {
    match p.get(name) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(failed("error-bad-param", &[name])),
    }
}

/// The numbers `1, 2 and 3` of a refusal that names the valid ones.
fn number_list(numbers: &[u64]) -> String {
    let texts: Vec<String> = numbers.iter().map(u64::to_string).collect();
    match texts.as_slice() {
        [] => strings::text("list-none").to_owned(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} {} {last}", rest.join(", "), strings::text("list-and")),
    }
}

/// The `file://` URL of a written file (Open PDF): the path's characters outside the unreserved set percent-encoded,
/// so nothing in a folder name can leave the URL.
fn file_url_of(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let path = path.strip_prefix("//?/").unwrap_or(&path);
    let encoded = super::onboarding::encode_path(path);
    if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

/// The export target made absolute when the job starts (against the working directory), so the entry's path and
/// Open PDF's `file://` URL name the written file even when a script passed a relative path (review finding 6 of
/// cycle 30). A path `std::path::absolute` cannot handle (an empty one) stays as it is.
fn absolute_target(path: PathBuf) -> PathBuf {
    std::path::absolute(&path).unwrap_or(path)
}

/// Whether `target` lies inside the project directory `dir`: compared on the canonical paths when both exist, else on
/// the absolute ones (component by component, so `p.ew2` is not inside `p.ew`).
fn inside(dir: &Path, target: &Path) -> bool {
    let folder = target.parent().unwrap_or(target);
    match (std::fs::canonicalize(dir), std::fs::canonicalize(folder)) {
        (Ok(d), Ok(f)) => f.starts_with(d),
        _ => match (std::path::absolute(dir), std::path::absolute(folder)) {
            (Ok(d), Ok(f)) => f.starts_with(d),
            _ => false,
        },
    }
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The jobs and the Jobs pane's entries.
    pub fn jobs(&self) -> &JobsState {
        &self.jobs
    }

    /// Whether the Jobs pane is in the layout.
    pub fn jobs_shown(&self) -> bool {
        self.layout.panes().contains(&PaneKind::Jobs)
    }

    /// The notifications: the visible toasts and the log.
    pub fn notifications(&self) -> &NotificationLog {
        &self.notes
    }

    /// The newest visible toast.
    pub fn toast(&self) -> Option<&Notification> {
        self.notes.newest_visible()
    }

    /// Whether `n`'s action is valid now (§6.5): Undo while the step it reports is the next undo step of the same
    /// project session; Open while the file it names was written in this session. `false` without an action.
    pub fn notification_valid(&self, n: &Notification) -> bool {
        match &n.action {
            None => false,
            Some((_, Validity::Open { .. })) => true,
            Some((_, Validity::Undo { epoch, cursor, rev })) => self.project.as_ref().is_some_and(|p| {
                let h = p.session.history();
                p.epoch == *epoch
                    && h.cursor() == *cursor
                    && cursor
                        .checked_sub(1)
                        .and_then(|i| h.entries().get(i))
                        .is_some_and(|e| e.rev().0 == *rev)
            }),
        }
    }

    /// The history's marker lines of the open project session (exports, recoveries), in the order they were added.
    pub fn markers(&self) -> &[HistoryMarker] {
        &self.markers
    }

    /// The `file://` URL the window must hand to the system opener (Open PDF), taken once.
    pub fn take_open_request(&mut self) -> Option<String> {
        self.jobs.open_request.take()
    }

    /// Takes the toast `serial` off the screen (the window's 8 seconds, or its Close button); the log keeps it.
    pub fn dismiss_toast(&mut self, serial: u64) {
        self.notes.dismiss(serial);
    }

    /// The window's repaint request, called by a job after each progress report and at its end.
    pub fn set_job_waker(&mut self, waker: Option<Waker>) {
        self.jobs.waker = waker;
    }

    /// The test gate of the jobs started from now on (feature `testkit`; the product never sets one).
    #[cfg(feature = "testkit")]
    pub fn set_job_gate_for_tests(&mut self, gate: Option<Gate>) {
        self.jobs.hooks.gate = gate;
    }

    /// A test fault of the export jobs started from now on (feature `testkit`): `Some("sibling")` makes the body panic
    /// right after the sibling file is written and synced, before the rename.
    #[cfg(feature = "testkit")]
    pub fn set_job_panic_for_tests(&mut self, at: Option<&'static str>) {
        self.jobs.hooks.panic_at = at;
    }

    // -----------------------------------------------------------------------------------------------------------
    // Notifications (§6.5)

    /// Shows `text` as a toast of `key` and logs it (FT-485's toasts map to key `edit`); `undo` offers Undo of the step
    /// just committed, valid while it is the next undo step.
    pub(super) fn notify_with(&mut self, text: String, key: &'static str, undo: bool) {
        let action = if undo {
            self.project.as_ref().and_then(|p| {
                let h = p.session.history();
                let cursor = h.cursor();
                let rev = h.entries().get(cursor.checked_sub(1)?)?.rev().0;
                Some((
                    NoteAction::Undo,
                    Validity::Undo {
                        epoch: p.epoch,
                        cursor,
                        rev,
                    },
                ))
            })
        } else {
            None
        };
        self.notes.push(text, key, action);
    }

    /// `notifications.show {on?}` (View ▸ Notifications, the status bar's field): the log popup.
    pub(super) fn act_notifications_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let want = match bool_param(p, "on", !self.notes.popup) {
            Ok(w) => w,
            Err(e) => return e,
        };
        self.notes.popup = want;
        Outcome::Done
    }

    /// `notifications.run {entry, action}`: a log entry's action while it is valid.
    pub(super) fn act_notifications_run(&mut self, p: &Map<String, Value>) -> Outcome {
        let serial = match uint_param(p, "entry") {
            Err(e) => return e,
            Ok(Some(n)) => n,
            Ok(None) => return failed("error-missing-param", &["entry"]),
        };
        let action = match string_param(p, "action") {
            Err(e) => return e,
            Ok(Some(word)) => match NoteAction::from_word(word) {
                Some(a) => a,
                None => return failed("error-bad-param", &["action"]),
            },
            Ok(None) => return failed("error-missing-param", &["action"]),
        };
        let Some(entry) = self.notes.get(serial).cloned() else {
            return failed(
                "error-unknown-notification",
                &[&serial.to_string(), &number_list(&self.notes.serials())],
            );
        };
        if entry.action() != Some(action) {
            return failed("error-no-such-action", &[action.word()]);
        }
        self.run_note_action(&entry)
    }

    /// Runs `entry`'s action: refused `reason-action-expired` when it is no longer valid.
    fn run_note_action(&mut self, entry: &Notification) -> Outcome {
        if !self.notification_valid(entry) {
            return Outcome::Refused {
                reason_key: "reason-action-expired",
            };
        }
        match entry.action() {
            Some(NoteAction::Undo) => {
                // The toast's Undo is spent: it goes from the screen.
                self.notes.dismiss(entry.serial);
                self.dispatch("edit.undo", &Value::Null)
            }
            Some(NoteAction::Open) => match (self.mode, entry.open_path()) {
                (Mode::Headless, _) => Outcome::Refused {
                    reason_key: "reason-no-opener",
                },
                (Mode::Gui, Some(path)) => {
                    self.jobs.open_request = Some(file_url_of(path));
                    Outcome::Done
                }
                (Mode::Gui, None) => Outcome::Refused {
                    reason_key: "reason-action-expired",
                },
            },
            None => Outcome::Refused {
                reason_key: "reason-no-toast",
            },
        }
    }

    /// `toast.undo`: the newest visible toast's Undo (the registry's undo); the toast goes.
    pub(super) fn act_toast_undo(&mut self) -> Outcome {
        let Some(toast) = self.notes.newest_visible().cloned() else {
            return Outcome::Refused {
                reason_key: "reason-no-toast",
            };
        };
        if toast.action() != Some(NoteAction::Undo) {
            return Outcome::Refused {
                reason_key: "reason-no-toast-undo",
            };
        }
        self.run_note_action(&toast)
    }

    /// `toast.dismiss`: the newest visible toast goes (the log keeps it).
    pub(super) fn act_toast_dismiss(&mut self) -> Outcome {
        if let Some(serial) = self.notes.newest_visible().map(|n| n.serial) {
            self.notes.dismiss(serial);
        }
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // The PDF export (§6.3)

    /// `file.export_pdf {path?, replace?, wait?}`: the export as a job writing `path` through its hidden sibling.
    pub(super) fn act_export_pdf(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => absolute_target(PathBuf::from(path)),
            // The native save dialog asks about an existing file itself, so its answer carries `replace` (D5).
            Ok(None) => {
                let mut rest = p.clone();
                rest.insert("replace".to_owned(), Value::Bool(true));
                return self.need_path("file.export_pdf", Prompt::SavePdf, "path", &rest);
            }
        };
        let (replace, wait) = match (bool_param(p, "replace", false), bool_param(p, "wait", true)) {
            (Ok(r), Ok(w)) => (r, w),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return failed("error-bad-param", &["path"]);
        };
        if self.jobs.running() > 0 {
            return Outcome::Refused {
                reason_key: "reason-export-running",
            };
        }
        // A PDF renamed over a file of the open project would damage it (review finding 5 of cycle 30).
        if inside(&project.dir, &path) {
            return Outcome::Refused {
                reason_key: "reason-export-into-project",
            };
        }
        if path.exists() && !replace {
            return Outcome::Refused {
                reason_key: "reason-file-exists",
            };
        }
        let rev = project.session.world().rev.0;
        let epoch = project.epoch;
        let job = project.session.export_job(&ExportOptions::default());
        self.count(|c| c.export_job += 1);
        let number = match self.jobs.start_export(job, path, name, rev, epoch) {
            Ok(n) => n,
            Err(_) => {
                let message = strings::format("error-export", &[strings::text("error-no-thread")]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        // Headless the step waits for its job (deterministic transcripts) unless `wait: false`; the window never waits.
        if self.mode == Mode::Headless && wait {
            return self.wait_job(number);
        }
        Outcome::Done
    }

    /// Takes the ends of the jobs that have finished (the window every frame, the script runner before every step).
    /// Makes no `Session` call.
    pub fn poll_jobs(&mut self) {
        let ended: Vec<(u64, JobEnd<ExportDone>)> = self
            .jobs
            .entries
            .iter_mut()
            .filter_map(|e| {
                let end = e.handle.as_mut()?.try_end()?;
                e.handle = None;
                Some((e.number, end))
            })
            .collect();
        for (number, end) in ended {
            self.finish_job(number, end);
        }
    }

    /// Waits for every running job and takes its end (its entry, notification and marker), as `jobs.wait` does for
    /// one. The headless runner calls it at the end of a run, before its unsaved-changes check, so a script whose
    /// export did not wait (`wait: false`) still writes its file, and its transcript, file and log are the same on
    /// every run (§9 items 1 and 2; review finding 3 of cycle 30).
    ///
    /// Returns, in job order, the notification texts of the failed, cancelled or panicked ends that no step waited
    /// for, whether their end was taken now or by an earlier poll (FT-513): the runner prints each as
    /// `error: <text>` and exits 1. Which poll took an end depends on timing; this list does not.
    pub fn wait_jobs(&mut self) -> Vec<String> {
        let running: Vec<u64> = self
            .jobs
            .entries()
            .filter(|e| e.handle.is_some())
            .map(|e| e.number)
            .collect();
        for number in running {
            if let Some(handle) = self.jobs.get_mut(number).and_then(|e| e.handle.take()) {
                let end = handle.wait();
                let _ = self.finish_job(number, end);
            }
        }
        self.jobs
            .entries()
            .filter_map(|e| e.unseen_failure.clone())
            .collect()
    }

    /// Waits for job `number` and takes its end; the step's outcome by the end (§6.3 item 5). The waiting step sees
    /// the end, so it is not an unseen failure, also when a poll took the end before this step (R1-2 of cycle 31); the
    /// entry is marked as waited for, whatever its end (R2-1).
    fn wait_job(&mut self, number: u64) -> Outcome {
        let Some(handle) = self.jobs.get_mut(number).and_then(|e| e.handle.take()) else {
            if let Some(entry) = self.jobs.get_mut(number) {
                entry.unseen_failure = None;
                entry.waited = true;
            }
            return Outcome::Done;
        };
        let end = handle.wait();
        let outcome = self.finish_job(number, end);
        if let Some(entry) = self.jobs.get_mut(number) {
            entry.unseen_failure = None;
            entry.waited = true;
        }
        outcome
    }

    /// The end of job `number` (§6.3 item 5): its entry's status, the notification, the history marker. Returns the
    /// outcome a waiting step reports.
    pub(super) fn finish_job(&mut self, number: u64, end: JobEnd<ExportDone>) -> Outcome {
        let current = self.project.as_ref().map(|p| (p.epoch, p.session.world().rev.0));
        let Some(entry) = self.jobs.get_mut(number) else {
            return Outcome::Done;
        };
        entry.handle = None;
        let same_session = current.is_some_and(|(epoch, _)| epoch == entry.epoch);
        let (status, text, action, outcome) = match end {
            JobEnd::Done(done) => {
                let kib = done.bytes.div_ceil(1024);
                let now = current
                    .filter(|(epoch, rev)| *epoch == entry.epoch && *rev != entry.rev)
                    .map(|(_, rev)| rev);
                // FT-485 item 11: the file's name (never its folder), the page count and the size.
                let text = strings::format(
                    "toast-export-pdf",
                    &[&entry.name, &done.pages.to_string(), &kib.to_string()],
                );
                let open = (
                    NoteAction::Open,
                    Validity::Open {
                        path: entry.path.clone(),
                    },
                );
                (
                    JobStatus::Done {
                        pages: done.pages,
                        kib,
                        now,
                    },
                    text,
                    Some(open),
                    Outcome::Done,
                )
            }
            JobEnd::Cancelled => {
                let text = strings::text("toast-export-cancelled").to_owned();
                (
                    JobStatus::Cancelled,
                    text.clone(),
                    None,
                    Outcome::Failed { message: text },
                )
            }
            JobEnd::Failed(cause) => {
                let text = strings::format("toast-export-failed", &[&cause]);
                (
                    JobStatus::Failed(cause),
                    text.clone(),
                    None,
                    Outcome::Failed { message: text },
                )
            }
            JobEnd::Panicked => {
                let cause = strings::text("job-cause-internal").to_owned();
                let text = strings::text("toast-export-internal").to_owned();
                (
                    JobStatus::Failed(cause),
                    text.clone(),
                    None,
                    Outcome::Failed { message: text },
                )
            }
        };
        let marker = matches!(status, JobStatus::Done { .. })
            .then(|| strings::format("marker-export", &[&entry.name]))
            .filter(|_| same_session)
            .map(|text| HistoryMarker { rev: entry.rev, text });
        entry.status = status;
        if matches!(outcome, Outcome::Failed { .. }) {
            entry.unseen_failure = Some(text.clone());
        }
        self.notes.push(text, "export", action);
        if let Some(m) = marker {
            self.markers.push(m);
        }
        outcome
    }

    /// The job a `jobs.*` action names: `job` when given (refused with the valid numbers when the pane has no such
    /// entry), else `default`'s pick.
    fn job_number(
        &self,
        p: &Map<String, Value>,
        default: impl Fn(&JobsState) -> Option<u64>,
        none: &'static str,
    ) -> Result<u64, Outcome> {
        match uint_param(p, "job")? {
            Some(n) if self.jobs.get(n).is_some() => Ok(n),
            Some(n) => Err(failed(
                "error-unknown-job",
                &[&n.to_string(), &number_list(&self.jobs.numbers())],
            )),
            None => default(&self.jobs).ok_or(Outcome::Refused { reason_key: none }),
        }
    }

    /// `jobs.show {on?}` (View ▸ Jobs, the status bar's job field): the Jobs pane in the bottom dock (D14).
    pub(super) fn act_jobs_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let shown = self.jobs_shown();
        let want = match bool_param(p, "on", !shown) {
            Ok(w) => w,
            Err(e) => return e,
        };
        if want != shown {
            let layout = if want {
                self.layout.with_bottom(PaneKind::Jobs)
            } else {
                self.layout.without(PaneKind::Jobs)
            };
            self.replace_layout(layout);
        }
        Outcome::Done
    }

    /// `jobs.cancel {job?}`: sets the cancel flag of a running job (the newest by default); its end arrives later.
    pub(super) fn act_jobs_cancel(&mut self, p: &Map<String, Value>) -> Outcome {
        let number = match self.job_number(
            p,
            |j| j.newest_running().map(|e| e.number),
            "reason-no-running-job",
        ) {
            Ok(n) => n,
            Err(e) => return e,
        };
        match self.jobs.get(number).and_then(|e| e.handle.as_ref()) {
            Some(h) => {
                h.cancel();
                Outcome::Done
            }
            None => Outcome::Refused {
                reason_key: "reason-job-finished",
            },
        }
    }

    /// `jobs.wait {job?}` (headless only): waits for a job and takes its end; by default the newest job no step has
    /// waited for, whether it still runs or a poll already took its end, so neither the choice nor the outcome depends
    /// on which poll took an end (R1-2 and R2-1 of cycle 31). Done at once when there is none.
    pub(super) fn act_jobs_wait(&mut self, p: &Map<String, Value>) -> Outcome {
        let unseen = |j: &JobsState| j.entries().filter(|e| !e.waited).last().map(|e| e.number);
        let number = match self.job_number(p, unseen, "reason-no-running-job") {
            Ok(n) => n,
            // Every job was waited for: there is nothing to wait for.
            Err(Outcome::Refused { .. }) => return Outcome::Done,
            Err(e) => return e,
        };
        // The job's own end (cancelled, failed) is its entry's and its notification's; the wait itself is done.
        let _ = self.wait_job(number);
        Outcome::Done
    }

    /// `jobs.open_result {job?}`: the written file of a finished export (the newest by default) in the system's viewer;
    /// headless there is no opener.
    pub(super) fn act_jobs_open_result(&mut self, p: &Map<String, Value>) -> Outcome {
        let newest_done = |j: &JobsState| {
            j.entries()
                .filter(|e| matches!(e.status, JobStatus::Done { .. }))
                .last()
                .map(|e| e.number)
        };
        let number = match self.job_number(p, newest_done, "reason-no-result") {
            Ok(n) => n,
            Err(e) => return e,
        };
        let Some(entry) = self.jobs.get(number) else {
            return Outcome::Refused {
                reason_key: "reason-no-result",
            };
        };
        if !matches!(entry.status, JobStatus::Done { .. }) {
            return Outcome::Refused {
                reason_key: "reason-no-result",
            };
        }
        if self.mode == Mode::Headless {
            return Outcome::Refused {
                reason_key: "reason-no-opener",
            };
        }
        self.jobs.open_request = Some(file_url_of(&entry.path));
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // The recovery dialog (§6.6)

    /// `journal.keep` (Open read-only, Escape): the dialog closes; the project stays read-only and the journal stays.
    pub(super) fn act_journal_keep(&mut self) -> Outcome {
        if matches!(self.dialog, Some(Dialog::Recovery { .. })) {
            self.dialog = None;
        }
        Outcome::Done
    }

    /// `journal.recover` (Recover): the read-only session closes and the project re-opens with `Recovery::Apply`. When
    /// the opened store still reports the leftover journal (the replay failed) or the open is not an editing one, that
    /// session closes again (the leftover stays), the project re-opens read-only and the dialog shows the reason with
    /// Open read-only and Discard... only. Otherwise the project is open for editing and dirty, with a notification
    /// and a history marker.
    pub(super) fn act_journal_recover(&mut self) -> Outcome {
        let Some(Dialog::Recovery {
            name,
            transactions,
            base_matches,
            last_change_ms,
            failed: None,
        }) = self.dialog.clone()
        else {
            return Outcome::Refused {
                reason_key: "reason-recovery-failed",
            };
        };
        let Some(dir) = self.project.as_ref().map(|p| p.dir.clone()) else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        self.dialog = None;
        // The read-only session holds no lock and owns no journal.
        self.close_project();
        let opened = self.open_for_editing_with(&dir, Recovery::Apply);
        let (session, report) = match opened {
            Ok(o) => o,
            Err(e) => {
                let message = strings::format("error-open", &[&name, strings::text(open_error_key(&e))]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        let replayed = session.store().is_some_and(|s| s.recovery().is_none())
            && matches!(report.access, ew_api::Access::Edit);
        if !replayed {
            // Its own (empty) journal goes, the leftover stays.
            session.close();
            let reason = if base_matches {
                strings::text("recovery-failed-replay")
            } else {
                strings::text("recovery-failed-moved")
            };
            let fresh = report
                .recovery
                .as_ref()
                .map(|r| (r.transactions, r.base_matches, r.last_change_ms));
            let (transactions, base_matches, last_change_ms) =
                fresh.unwrap_or((transactions, base_matches, last_change_ms));
            let again = match Session::open_read_only(&dir, &self.config.app_version) {
                Ok((session, _)) => session,
                Err(e) => {
                    let message = strings::format("error-open", &[&name, strings::text(open_error_key(&e))]);
                    self.banner = Some(message.clone());
                    return Outcome::Failed { message };
                }
            };
            self.install_project(
                &dir,
                again,
                AccessMode::ReadOnly {
                    reason_key: "readonly-recovery",
                },
            );
            self.banner = Some(strings::text("banner-recovery").to_owned());
            self.dialog = Some(Dialog::Recovery {
                name,
                transactions,
                base_matches,
                last_change_ms,
                failed: Some(reason.to_owned()),
            });
            return Outcome::Done;
        }
        self.install_project(&dir, session, AccessMode::Edit);
        self.banner = None;
        let count = transactions;
        self.notify_with(strings::format_count("toast-recovered", count), "recovery", false);
        if let Some(rev) = self.revision() {
            self.markers.push(HistoryMarker {
                rev,
                text: strings::format_count("marker-recovered", count),
            });
        }
        Outcome::Done
    }

    /// Holds `session` as the open project of `dir` with `access` (the recovery's re-opens): a new project session, the
    /// record of `dir` kept.
    fn install_project(&mut self, dir: &Path, session: Session, access: AccessMode) {
        let labels = session.page_labels();
        if self.record.dir.as_deref() != Some(dir) {
            self.record = Record {
                dir: Some(dir.to_path_buf()),
                ..Record::default()
            };
        }
        self.epochs += 1;
        self.project = Some(Project {
            session,
            epoch: self.epochs,
            dir: dir.to_path_buf(),
            name: dir_name(dir),
            access,
            labels,
            rows: Vec::new(),
        });
        self.install_view();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review finding 6 of cycle 30: a relative target is made absolute when the job starts, so its `file://` URL is
    /// the working directory's file, never `file:///out.pdf`.
    #[test]
    fn a_relative_target_gets_the_url_of_the_absolute_file() {
        let target = absolute_target(PathBuf::from("out.pdf"));
        assert!(target.is_absolute(), "{target:?}");
        assert_eq!(target, std::path::absolute("out.pdf").unwrap());
        let url = file_url_of(&target);
        assert_ne!(url, "file:///out.pdf");
        assert!(url.starts_with("file:///") || url.starts_with("file://"), "{url}");
        assert!(url.ends_with("/out.pdf"), "{url}");
        assert!(url.len() > "file:///out.pdf".len(), "{url}");
        // An absolute target is kept as it is.
        assert_eq!(absolute_target(target.clone()), target);
    }
}
