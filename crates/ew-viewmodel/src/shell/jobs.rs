// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shell's part of the jobs and notifications (doc 16 §8.2, §8.3): the PDF export as a job with its entries and actions, the
//! notifications and their log, and the recovery dialog's answers.
//!
//! **Threads.** The `Session` never leaves this thread: `file.export_pdf` takes `Session::export_job` (a snapshot) and
//! hands it to a job of `ew_jobs`; nothing on the job's thread touches the shell. The end of a job is taken by
//! [`ShellModel::poll_jobs`] (the window calls it every frame, the script runner before every step) or, headless, by a
//! waiting `file.export_pdf` or `jobs.wait`. Polling makes no `Session` call.

use std::path::{Path, PathBuf};

#[cfg(debug_assertions)]
use ew_api::ew_jobs::Gate;
use ew_api::ew_jobs::{JobEnd, Waker};
use ew_api::output::ExportOptions;
use ew_api::{Recovery, Session};
use serde_json::{Map, Value};

use super::{
    AccessMode, Dialog, HistoryMarker, Mode, Outcome, Project, Prompt, Record, ShellModel, dir_name, failed,
    open_error_key, string_param,
};
use crate::jobs::{ExportDone, JobKind, JobStatus, JobsState};
use crate::layout::PaneKind;
use crate::notifications::{Note, NoteTone, Notification, NotificationLog, Offer, Run, Validity};
use crate::strings;
use crate::strings::Key;
use crate::{count_key, key};

/// The key of a placement's toast that names the wires it made ([`ShellModel::notify_wired`]).
const WIRED: &str = "wired";

/// A `uint` parameter: `Ok(None)` when absent, `Err` when present with another type.
fn uint_param(p: &Map<String, Value>, name: &str) -> Result<Option<u64>, Outcome> {
    match p.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .map(Some)
            .ok_or_else(|| failed(key!("error-bad-param"), &[name])),
    }
}

/// A `bool` parameter with its default.
pub(super) fn bool_param(p: &Map<String, Value>, name: &str, default: bool) -> Result<bool, Outcome> {
    match p.get(name) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(failed(key!("error-bad-param"), &[name])),
    }
}

/// An export's pages by kind in words ("1 drawing page and 10 report pages"): the drawing pages, the
/// section divider pages and the report pages that it has, each count with its plural form.
fn page_words(done: &ExportDone) -> String {
    let kinds = done.kinds;
    let reports = done.pages.saturating_sub(kinds.drawing + kinds.dividers);
    let parts: Vec<String> = [
        (kinds.drawing, crate::count_key!("export-pages-drawing")),
        (kinds.dividers, crate::count_key!("export-pages-divider")),
        (reports, crate::count_key!("export-pages-report")),
    ]
    .into_iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, key)| strings::format_count(key, n))
    .collect();
    if parts.is_empty() {
        return strings::format_count(crate::count_key!("export-pages-drawing"), 0);
    }
    strings::and_list(&parts)
}

/// The numbers `1, 2 and 3` of a refusal that names the valid ones.
fn number_list(numbers: &[u64]) -> String {
    let texts: Vec<String> = numbers.iter().map(u64::to_string).collect();
    if texts.is_empty() {
        return strings::text(key!("list-none")).to_owned();
    }
    strings::and_list(&texts)
}

/// The `file://` URL of a local file or folder (the privacy file, the log folder): the path's characters outside the
/// unreserved set percent-encoded, so nothing in a folder name can leave the URL.
pub(crate) fn file_url_of(path: &Path) -> String {
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
/// Open PDF's path name the written file even when a script passed a relative path. A path `std::path::absolute` cannot handle (an empty one) stays as it is.
pub(super) fn absolute_target(path: PathBuf) -> PathBuf {
    std::path::absolute(&path).unwrap_or(path)
}

/// Whether `target` lies inside the project directory `dir`: compared on the canonical paths when both exist, else on
/// the absolute ones (component by component, so `p.ew2` is not inside `p.ew`).
pub(super) fn inside(dir: &Path, target: &Path) -> bool {
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
        self.panes.layout.panes().contains(&PaneKind::Jobs)
    }

    /// The notifications: the visible toasts and the log.
    pub fn notifications(&self) -> &NotificationLog {
        &self.notes
    }

    /// The newest visible toast.
    pub fn toast(&self) -> Option<&Notification> {
        self.notes.newest_visible()
    }

    /// Whether `n`'s action is valid now: Undo while the step it reports is the next undo step of the same
    /// project session; Open while the file it names was written in this session. `false` without an action.
    pub fn notification_valid(&self, n: &Notification) -> bool {
        n.action().is_some_and(|a| self.note_action_reason(n, a).is_ok())
    }

    /// Whether the offer `word` of `n` can run, else why not: Undo while its step is the next undo step, Open
    /// always, Show changes while its history entry is still there, a tip's action while its project session is
    /// open, an offer on a page while the page is there (on show, where it acts on the page on show), Back to the
    /// editor while the draft is kept.
    ///
    /// # Errors
    ///
    /// The reason key: the project closed since, a later change, the history entry gone, the draft discarded.
    pub fn note_action_reason(&self, n: &Notification, word: &str) -> Result<(), Key<'static>> {
        let session = |epoch: u64| {
            self.project
                .as_ref()
                .filter(|p| p.epoch == epoch)
                .map(|p| &p.session)
                .ok_or(key!("reason-action-project-closed"))
        };
        let ok = |valid: bool, why: Key<'static>| if valid { Ok(()) } else { Err(why) };
        let Some(offer) = n.offer(word) else {
            return Err(key!("reason-action-expired"));
        };
        match &offer.validity {
            Validity::Always => Ok(()),
            Validity::Page { epoch, page, shown } => {
                session(*epoch)?;
                let there = if *shown {
                    self.focused_page_id() == Some(*page)
                } else {
                    self.page_rows().iter().any(|r| r.id == *page)
                };
                ok(there, key!("reason-action-expired"))
            }
            Validity::History { epoch, entry, rev } => ok(
                session(*epoch)?
                    .history()
                    .entries()
                    .get(*entry)
                    .is_some_and(|e| e.rev().0 == *rev),
                key!("reason-action-history-gone"),
            ),
            Validity::Undo { epoch, cursor, rev } => {
                let h = session(*epoch)?.history();
                ok(
                    h.cursor() == *cursor
                        && cursor
                            .checked_sub(1)
                            .and_then(|i| h.entries().get(i))
                            .is_some_and(|e| e.rev().0 == *rev),
                    key!("reason-action-undo-later"),
                )
            }
            Validity::Session { epoch } => session(*epoch).map(|_| ()),
            Validity::SymbolDraft => ok(
                self.kept_symbol_editor().is_some(),
                key!("reason-action-draft-gone"),
            ),
        }
    }

    /// The history's marker lines of the open project session (exports, recoveries), in the order they were added.
    pub fn markers(&self) -> &[HistoryMarker] {
        &self.markers
    }

    /// The written file the window must hand to the system's file opener (Open PDF), taken once: its path, never a
    /// `file://` URL (the URL handed to Windows' URL handler opened nothing).
    pub fn take_open_request(&mut self) -> Option<PathBuf> {
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

    /// The test gate of the jobs started from now on (a debug build's; the product never sets one).
    #[cfg(debug_assertions)]
    pub fn set_job_gate_for_tests(&mut self, gate: Option<Gate>) {
        self.jobs.hooks.gate = gate;
    }

    /// A test fault of the export jobs started from now on (a debug build's): `Some("sibling")` makes the body panic
    /// right after the sibling file is written and synced, before the rename; `Some("table-read")` makes the table
    /// import's read panic inside its guard.
    #[cfg(debug_assertions)]
    pub fn set_job_panic_for_tests(&mut self, at: Option<&'static str>) {
        self.jobs.hooks.panic_at = at;
    }

    // -----------------------------------------------------------------------------------------------------------
    // Notifications

    /// Shows `text` as a toast of `key` and logs it (the toasts of edits map to key `edit`); `undo` offers Undo of the step
    /// just committed, valid while it is the next undo step.
    pub(super) fn notify_with(&mut self, text: String, key: &'static str, undo: bool) {
        let undo = if undo { self.undo_offer() } else { None };
        self.notes.push(Note::new(key, text).offering([undo]));
    }

    /// Shows `text` as a toast: what a command did that the page does not show by itself (a new strip, the
    /// wires a placement drew); `undo` offers Undo. The toasts of these callers share the key `edit`, so a
    /// newer one replaces an older one on screen; every one enters the notifications log.
    pub(super) fn notify(&mut self, text: String, undo: bool) {
        self.notify_with(text, "edit", undo);
    }

    /// Shows a placement's sentences `made` that name the wires it made, with Undo, as a toast of key `wired`: the next
    /// placement's toast (key `edit`) leaves it up, and the next placement that made wires adds its sentences to it,
    /// so no wire made goes unsaid.
    /// `without` is its last sentence, the way to place without wires, said once at the end.
    pub(super) fn notify_wired(&mut self, made: Vec<String>, without: String) {
        let mut sentences = match self.notes.visible().iter().find(|n| n.key == WIRED) {
            Some(earlier) => {
                let mut earlier = earlier.sentences.clone();
                if earlier.last() == Some(&without) {
                    earlier.pop();
                }
                earlier
            }
            None => Vec::new(),
        };
        sentences.extend(made);
        sentences.push(without);
        let undo = self.undo_offer();
        self.notes.push(Note::of(WIRED, sentences).offering([undo]));
    }

    /// What keeps an action offered on the step just committed valid: while that step is the next undo step.
    pub(super) fn undo_validity(&self) -> Option<Validity> {
        self.project.as_ref().and_then(|p| {
            let h = p.session.history();
            let cursor = h.cursor();
            let rev = h.entries().get(cursor.checked_sub(1)?)?.rev().0;
            Some(Validity::Undo {
                epoch: p.epoch,
                cursor,
                rev,
            })
        })
    }

    /// Undo of the step just committed, valid while that step is the next undo step.
    pub(super) fn undo_offer(&self) -> Option<Offer> {
        self.undo_validity().map(Offer::undo)
    }

    /// What keeps an offer valid while the open project session lasts.
    pub(super) fn session_validity(&self) -> Option<Validity> {
        self.project
            .as_ref()
            .map(|p| Validity::Session { epoch: p.epoch })
    }

    /// What keeps an offer on page `page` valid: while the project session lasts and the page is in it; with `shown`,
    /// while it is the page on show.
    pub(super) fn page_validity(&self, page: ew_api::output::Uuid, shown: bool) -> Option<Validity> {
        self.project.as_ref().map(|p| Validity::Page {
            epoch: p.epoch,
            page,
            shown,
        })
    }

    /// The offer `word` with the label of `label`, the actions `steps` with their parameters, valid while the
    /// project session lasts.
    pub(super) fn session_offer(
        &self,
        word: &'static str,
        label: Key<'static>,
        steps: Vec<(&'static str, Value)>,
    ) -> Option<Offer> {
        self.session_validity()
            .map(|v| Offer::steps(word, label, v, steps))
    }

    /// Rack…: the view of station `station`, offered while the project session lasts.
    pub(super) fn rack_offer(&self, station: ew_api::output::Uuid) -> Option<Offer> {
        self.session_offer(
            "rack",
            key!("action-plc-rack-editor"),
            vec![(
                "plc.rack_editor",
                serde_json::json!({ "station": station.to_string() }),
            )],
        )
    }

    /// Go to page: page `page` (an Undo or a Redo changed it, and it is not the page on show), offered while it is in
    /// the project.
    pub(super) fn go_to_offer(&self, page: ew_api::output::Uuid) -> Option<Offer> {
        let folio = self
            .page_rows()
            .iter()
            .find(|r| r.id == page)
            .map(|r| r.folio.clone());
        folio.zip(self.page_validity(page, false)).map(|(folio, v)| {
            Offer::steps(
                "go_to",
                key!("note-action-go-to-page"),
                v,
                vec![("page.goto", serde_json::json!({ "folio": folio }))],
            )
        })
    }

    /// Shows `text` as a toast of key `edit` that offers `offer` (the rack of a station, Route these too… on a page,
    /// Swap two valves, Show item numbers, Fit the layout to the sheet, New station…).
    pub(super) fn notify_offer(&mut self, text: String, offer: Option<Offer>) {
        self.notes.push(Note::new("edit", text).offering([offer]));
    }

    /// Shows `text` as a toast of key `edit` with Undo (the step just made) and Go to page `page`.
    pub(super) fn notify_undo_or_go_to(&mut self, text: String, page: ew_api::output::Uuid) {
        if self.project.is_none() {
            return;
        }
        let undo = self.undo_offer();
        let go_to = self.go_to_offer(page);
        self.notes.push(Note::new("edit", text).offering([undo, go_to]));
    }

    /// Shows `text` as a toast of key `key` with Show in folder, which opens `folder` in the system's file manager.
    pub(super) fn notify_folder(&mut self, text: String, key: &'static str, folder: PathBuf) {
        let offer = Offer::new(
            "show_folder",
            strings::text(key!("toast-show-folder")).to_owned(),
            Validity::Always,
            Run::Open(folder),
        );
        self.notes.push(Note::new(key, text).offering([Some(offer)]));
    }

    /// Shows a result's `text` as a toast of key `edit` whose icon follows `tone` (the check's worst severity).
    pub(super) fn notify_toned(&mut self, text: String, tone: NoteTone) {
        self.notes.push(Note::new("edit", text).toned(tone));
    }

    /// `notifications.show {on?}` (View ▸ Notifications, the status bar's field): the log popup.
    pub(crate) fn act_notifications_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let want = or_return!(bool_param(p, "on", !self.notes.popup));
        self.notes.popup = want;
        Outcome::Done
    }

    /// `notifications.run {entry?, action}`: a log entry's action while it is valid; without `entry`, the newest
    /// entry's that offers `action`.
    pub(crate) fn act_notifications_run(&mut self, p: &Map<String, Value>) -> Outcome {
        let word = match string_param(p, "action") {
            Err(e) => return e,
            Ok(Some(word)) => word.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["action"]),
        };
        // Without `entry`, the newest message that offers `action`: the button a person presses on the toast in
        // front, which a script cannot name by its serial.
        let serial = match uint_param(p, "entry") {
            Err(e) => return e,
            Ok(Some(n)) => n,
            Ok(None) => match self
                .notes
                .serials()
                .into_iter()
                .rev()
                .find(|s| self.notes.get(*s).is_some_and(|n| n.offer(&word).is_some()))
            {
                Some(s) => s,
                None => return failed(key!("error-no-such-action"), &[&word]),
            },
        };
        let Some(entry) = self.notes.get(serial).cloned() else {
            return failed(
                key!("error-unknown-notification"),
                &[&serial.to_string(), &number_list(&self.notes.serials())],
            );
        };
        if entry.offer(&word).is_none() {
            return failed(key!("error-no-such-action"), &[&word]);
        }
        self.run_action_of(&entry, &word)
    }

    /// Runs `entry`'s first action: refused `reason-action-expired` when it is no longer valid.
    fn run_note_action(&mut self, entry: &Notification) -> Outcome {
        match entry.action() {
            Some(action) => self.run_action_of(entry, action),
            None => Outcome::refused(key!("reason-no-toast")),
        }
    }

    /// Runs `entry`'s offer `word`: refused with its reason when it can no longer run. Undo undoes the step (the toast
    /// goes); Open hands the file to the system's opener; the generic offer's actions run in their order (the toast
    /// goes; a `once` offer is spent).
    fn run_action_of(&mut self, entry: &Notification, word: &str) -> Outcome {
        if let Err(reason_key) = self.note_action_reason(entry, word) {
            return Outcome::Refused {
                reason: reason_key.into(),
            };
        }
        // An offer that its first run spent is gone (`entry` is the log's copy; the log knows it is spent).
        let spent = self
            .notes
            .get(entry.serial)
            .and_then(|n| n.offer(word))
            .is_some_and(|o| o.spent);
        let Some(offer) = entry.offer(word).cloned().filter(|_| !spent) else {
            return Outcome::refused(key!("reason-action-expired"));
        };
        match offer.run {
            Run::Undo => {
                // The toast's Undo is spent: it goes from the screen.
                self.notes.dismiss(entry.serial);
                self.history.unsaid = true;
                let out = self.dispatch("edit.undo", &Value::Null);
                self.history.unsaid = false;
                out
            }
            Run::Open(path) => match self.mode {
                Mode::Headless => Outcome::refused(key!("reason-no-opener")),
                Mode::Gui => {
                    self.jobs.open_request = Some(path);
                    Outcome::Done
                }
            },
            // Route these too…: on the page it names, while that page is on show (its validity).
            Run::RouteHis => {
                self.notes.dismiss(entry.serial);
                self.route_his_card()
            }
            // Two valves swap their positions: the first takes the second's, the second the first's.
            Run::Swap { device, holder } => {
                self.notes.dismiss(entry.serial);
                let seat = self.project_session().and_then(|s| {
                    let h = s.query().devices().get(holder.to_string().parse().ok()?)?.clone();
                    Some((h.parent?.as_uuid(), h.position?))
                });
                let Some((parent, position)) = seat else {
                    return Outcome::refused(key!("reason-action-expired"));
                };
                self.run_logged(
                    "assembly.attach".to_owned(),
                    serde_json::json!({ "parent": parent.to_string(), "swap": true,
                        "children": [{ "device": device.to_string(), "position": position }] }),
                )
            }
            Run::Steps { steps, once, keeps } => {
                if !keeps {
                    self.notes.dismiss(entry.serial);
                }
                if once {
                    self.notes.spend(entry.serial, word);
                }
                let mut out = Outcome::Done;
                for (action, params) in &steps {
                    out = self.dispatch(action, params);
                    if out != Outcome::Done {
                        break;
                    }
                }
                out
            }
        }
    }

    /// Shows a paste's `text` with its Undo and Keep, which freezes each tag of `kept` (a device and its tag before the
    /// paste) as it read, both valid while the paste is the next undo step. Keep is left out when no tag can be read
    /// back as typed; its label names the tag when it keeps one.
    pub(super) fn notify_keep_tags(&mut self, text: String, kept: &[(ew_api::output::Uuid, String)]) {
        let edits: Vec<Value> = self.project_session().map_or_else(Vec::new, |s| {
            kept.iter()
                .filter_map(|(d, tag)| {
                    let parsed = s.parse_field_input("device", "tag.frozen", tag).ok()?;
                    (!parsed.value.is_null()).then(|| {
                        serde_json::json!({"kind": "device", "targets": [d.to_string()], "path": "tag.frozen",
                            "value": parsed.value})
                    })
                })
                .collect()
        });
        if edits.is_empty() {
            self.notify(text, true);
            return;
        }
        // Its label names the one tag it keeps ("Keep -X1021").
        let label = match kept {
            [(_, tag)] => strings::format(key!("note-action-keep-tag"), &[tag]),
            _ => strings::text(key!("note-action-keep-tags")).to_owned(),
        };
        let step =
            crate::notifications::command_step("fields.set_many", serde_json::json!({ "edits": edits }));
        let keep = self.undo_validity().map(|v| {
            Offer::steps("keep_tag", key!("note-action-keep-tags"), v, vec![step])
                .once()
                .labelled(label)
        });
        self.notes
            .push(Note::new("edit", text).offering([self.undo_offer(), keep]));
    }

    /// The `edit.delete` of the terminals `terminals` and of each strip that would keep none of its terminals and draws
    /// nothing itself (a strip's segment), in one step.
    pub(super) fn removal_command(&self, terminals: &[ew_api::output::Uuid]) -> Option<Value> {
        let session = self.project_session()?;
        let q = session.query();
        let idx = q.idx();
        let mut targets: Vec<Value> = terminals
            .iter()
            .map(|t| serde_json::json!({"kind": "device", "id": t.to_string()}))
            .collect();
        let strips: std::collections::BTreeSet<_> = terminals
            .iter()
            .filter_map(|t| {
                q.devices()
                    .get(t.to_string().parse().ok()?)?
                    .aspects
                    .terminal
                    .as_ref()
                    .map(|a| a.strip)
            })
            .collect();
        for strip in strips {
            let emptied = idx
                .terminals_of_strip(strip)
                .all(|t| terminals.contains(&t.as_uuid()));
            let drawn = idx
                .functions_of_device(strip)
                .any(|f| idx.reps_of_function(f).next().is_some());
            if emptied && !drawn {
                targets.push(serde_json::json!({"kind": "device", "id": strip.as_uuid().to_string()}));
            }
        }
        (!targets.is_empty()).then(|| serde_json::json!({ "targets": targets }))
    }

    /// Shows the toast that the parts list leaves out the devices without a part, when there are any:
    /// how many, with Select them and Type not known yet….
    #[inline(never)]
    pub(super) fn notify_unlisted(&mut self) {
        let Some(devices) = self
            .project_session()
            .map(|s| ew_api::output::unlisted_devices(&s.query()))
        else {
            return;
        };
        let text = match devices.len() {
            0 => return,
            n => strings::format_count(crate::count_key!("toast-parts-unlisted"), n),
        };
        let select: Vec<ew_api::output::Uuid> = devices.iter().map(|d| d.as_uuid()).collect();
        // Its own key, so it stands beside the toast of the step that added the list (with its Undo). Beside Select
        // them, Type not known yet… lists them by a description.
        let offers = [
            self.session_offer(
                "select_unlisted",
                key!("note-action-select-unlisted"),
                vec![
                    crate::notifications::select_step(&select),
                    ("props.focus", Value::Null),
                ],
            ),
            self.session_offer(
                "list_unlisted",
                key!("action-part-placeholder"),
                vec![
                    crate::notifications::select_step(&select),
                    ("part.placeholder", serde_json::json!({})),
                ],
            ),
        ];
        self.notes.push(Note::new("unlisted", text).offering(offers));
    }

    /// `toast.undo`: the newest visible toast's Undo (the registry's undo); the toast goes.
    pub(crate) fn act_toast_undo(&mut self) -> Outcome {
        let Some(toast) = self.notes.newest_visible().cloned() else {
            return Outcome::refused(key!("reason-no-toast"));
        };
        if toast.action() != Some("undo") {
            return Outcome::refused(key!("reason-no-toast-undo"));
        }
        self.run_note_action(&toast)
    }

    /// `toast.dismiss`: the newest visible toast goes (the log keeps it).
    pub(crate) fn act_toast_dismiss(&mut self) -> Outcome {
        if let Some(serial) = self.notes.newest_visible().map(|n| n.serial) {
            self.notes.dismiss(serial);
        }
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // The PDF export

    /// `file.export_pdf {path?, replace?, wait?}`: the export as a job writing `path` through its hidden sibling.
    pub(crate) fn act_export_pdf(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => absolute_target(PathBuf::from(path)),
            // The native save dialog asks about an existing file itself, so its answer carries `replace`.
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
        let project = or_return!(crate::shell::gated(&self.project));
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return failed(key!("error-bad-param"), &["path"]);
        };
        // A print being spooled and a drawing import write no file and run beside the export.
        if self
            .jobs
            .entries()
            .any(|e| e.is_running() && matches!(e.kind, JobKind::Pdf | JobKind::Table))
        {
            return Outcome::refused(key!("reason-export-running"));
        }
        // A PDF renamed over a file of the open project would damage it.
        if inside(&project.dir, &path) {
            return Outcome::refused(key!("reason-export-into-project"));
        }
        if path.exists() && !replace {
            return Outcome::refused(key!("reason-file-exists"));
        }
        let rev = project.session.world().rev.0;
        let epoch = project.epoch;
        let job = project.session.export_job(&ExportOptions::default());
        self.count(|c| c.export_job += 1);
        let number = match self.jobs.start_export(job, path, name, rev, epoch) {
            Ok(n) => n,
            Err(_) => {
                let message =
                    strings::format(key!("error-export"), &[strings::text(key!("error-no-thread"))]);
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

    /// `file.export_dxf {dir?, confirm?, wait?}` (Output ▸ Export DXF…, doc 09 §4.9): the DXF set as a job writing
    /// every page and the pictures they name into the folder `dir` as a whole. A folder that holds files no earlier
    /// DXF export wrote is refused before the job starts, so no file of the user's is ever removed; one that holds an
    /// earlier set is replaced only with `confirm` (the window asks first).
    pub(crate) fn act_export_dxf(&mut self, p: &Map<String, Value>) -> Outcome {
        let dir = match string_param(p, "dir") {
            Err(e) => return e,
            Ok(Some(dir)) => absolute_target(PathBuf::from(dir)),
            Ok(None) => return self.need_path("file.export_dxf", Prompt::SaveDxf, "dir", p),
        };
        let (wait, confirm) = match (bool_param(p, "wait", true), bool_param(p, "confirm", false)) {
            (Ok(w), Ok(c)) => (w, c),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let project = or_return!(crate::shell::gated(&self.project));
        let Some(name) = dir.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return failed(key!("error-bad-param"), &["dir"]);
        };
        if self
            .jobs
            .entries()
            .any(|e| e.is_running() && matches!(e.kind, JobKind::Pdf | JobKind::Table | JobKind::Dxf))
        {
            return Outcome::refused(key!("reason-export-running"));
        }
        // A set renamed over a folder of the open project would damage it; the project's own folder (or one that holds
        // it) holds other files, which the next check refuses.
        if inside(&project.dir, &dir) {
            return Outcome::refused(key!("reason-export-into-project"));
        }
        if !matches!(
            ew_api::file::foreign_entry(&dir, &ew_api::output::DXF_SET_EXTENSIONS),
            Ok(None)
        ) {
            return Outcome::refused(key!("reason-export-dxf-folder"));
        }
        // An earlier set's files may hold a person's later edits in a CAD program: the window asks before they go
        // (doc 16a §6.12), a script says `confirm`.
        let files = std::fs::read_dir(&dir).map_or(0, |d| {
            d.flatten()
                .filter(|e| e.file_name() != ew_api::file::FILE_SET_MARK)
                .count()
        });
        if files > 0 && !confirm {
            return match self.mode {
                Mode::Headless => failed(key!("error-export-dxf-confirm"), &[]),
                Mode::Gui => self.open_dialog(Dialog::ConfirmReplaceDxf {
                    dir: dir.to_string_lossy().into_owned(),
                    folder: name,
                    files,
                }),
            };
        }
        // The confirmation gives way.
        if matches!(&self.dialog, Some(Dialog::ConfirmReplaceDxf { .. })) {
            self.dialog = None;
        }
        let project = or_return!(crate::shell::gated(&self.project));
        let rev = project.session.world().rev.0;
        let epoch = project.epoch;
        let job = project.session.export_dxf_job(&ExportOptions::default());
        self.count(|c| c.export_job += 1);
        let number = match self.jobs.start_dxf(job, dir, name, rev, epoch) {
            Ok(n) => n,
            Err(_) => {
                let message =
                    strings::format(key!("error-export"), &[strings::text(key!("error-no-thread"))]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        if self.mode == Mode::Headless && wait {
            return self.wait_job(number);
        }
        Outcome::Done
    }

    /// Takes the ends of the jobs that have finished (the window every frame, the script runner before every step).
    /// Makes no `Session` call.
    pub fn poll_jobs(&mut self) {
        // The print pre-dialog's build too.
        self.poll_print();
        // And the import's, and a drawing import's.
        self.poll_import();
        self.poll_drawing();
        self.poll_reads();
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
    /// every run.
    ///
    /// Returns, in job order, the notification texts of the failed, cancelled or panicked ends that no step waited
    /// for, whether their end was taken now or by an earlier poll: the runner prints each as
    /// `error: <text>` and exits 1. Which poll took an end depends on timing; this list does not.
    pub fn wait_jobs(&mut self) -> Vec<String> {
        // A running drawing import ends in its dialog, page or underlay, a running read in its import's.
        self.wait_drawing();
        self.wait_reads();
        // A running import job ends in its card.
        if let Some(pending) = self.import.job.take() {
            let end = pending.job.wait();
            let _ = self.finish_import(end, pending.name, pending.dir, pending.library_note);
        }
        // So does a running re-import job.
        if let Some(pending) = self.import.reimport_job.take() {
            let end = pending.job.wait();
            let _ = self.finish_reimport(end, pending.name, pending.path, pending.epoch);
        }
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

    /// Waits for job `number` and takes its end; the step's outcome by the end. The waiting step sees
    /// the end, so it is not an unseen failure, also when a poll took the end before this step; the
    /// entry is marked as waited for, whatever its end.
    pub(super) fn wait_job(&mut self, number: u64) -> Outcome {
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

    /// The end of job `number`: its entry's status, the notification, the history marker. Returns the
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
                // The file's name, the page count, the size and the folder it is in;
                // a DXF set's folder, which holds one file per page. A file this session's log
                // already names as exported says "again", so a second export visibly changes the toast it replaces.
                let again = self.notes.entries().any(|n| {
                    n.key == "export"
                        && n.offers
                            .first()
                            .is_some_and(|o| matches!(&o.run, Run::Open(path) if *path == entry.path))
                });
                // A table has no pages; its end says the size only.
                let table = entry.kind == JobKind::Table;
                // A DXF set's end says that its folder holds DXF files.
                let (pdf, pdf_again) = if entry.kind == JobKind::Dxf {
                    (key!("toast-export-dxf"), key!("toast-export-dxf-again"))
                } else {
                    (key!("toast-export-pdf"), key!("toast-export-pdf-again"))
                };
                // The pages by kind ("1 drawing page and 10 report pages"), each count with its
                // plural form.
                let pages = page_words(&done);
                let dxf = entry.kind == JobKind::Dxf;
                let folder = if dxf {
                    entry.path.clone()
                } else {
                    entry.path.parent().map(Path::to_path_buf).unwrap_or_default()
                };
                let shown = if dxf {
                    entry.path.display().to_string()
                } else {
                    entry.name.clone()
                };
                let place = folder.display().to_string();
                let text = match (entry.kind, table, again) {
                    // A package names its file and how many files it holds, a draft's said so.
                    (JobKind::Package { draft }, _, _) => strings::format_counted(
                        if draft {
                            crate::count_key!("toast-package-draft")
                        } else {
                            crate::count_key!("toast-package")
                        },
                        done.pages,
                        &[&entry.name],
                    ),
                    (_, false, false) => strings::format(pdf, &[&shown, &pages, &kib.to_string(), &place]),
                    (_, false, true) => {
                        strings::format(pdf_again, &[&shown, &pages, &kib.to_string(), &place])
                    }
                    (_, true, false) => strings::format(
                        key!("toast-export-table"),
                        &[&entry.name, &kib.to_string(), &place],
                    ),
                    (_, true, true) => strings::format(
                        key!("toast-export-table-again"),
                        &[&entry.name, &kib.to_string(), &place],
                    ),
                };
                let open = Offer::new(
                    "open",
                    strings::text(key!("toast-open")).to_owned(),
                    Validity::Always,
                    Run::Open(entry.path.clone()),
                );
                // A file's toast shows its folder too; a DXF set's Open opens its folder already.
                let show_folder = (!dxf).then(|| {
                    Offer::new(
                        "show_folder",
                        strings::text(key!("toast-show-folder")).to_owned(),
                        Validity::Always,
                        Run::Open(folder),
                    )
                });
                (
                    JobStatus::Done {
                        pages: (!table).then_some(done.pages),
                        kib,
                        now,
                    },
                    text,
                    Some((open, show_folder)),
                    Outcome::Done,
                )
            }
            JobEnd::Cancelled => {
                let text = strings::text(key!("toast-export-cancelled")).to_owned();
                (
                    JobStatus::Cancelled,
                    text.clone(),
                    None,
                    Outcome::Failed { message: text },
                )
            }
            JobEnd::Failed(cause) => {
                let text = strings::format(key!("toast-export-failed"), &[&cause]);
                (
                    JobStatus::Failed(cause),
                    text.clone(),
                    None,
                    Outcome::Failed { message: text },
                )
            }
            JobEnd::Panicked => {
                let cause = strings::text(key!("job-cause-internal")).to_owned();
                let text = strings::text(key!("toast-export-internal")).to_owned();
                (
                    JobStatus::Failed(cause),
                    text.clone(),
                    None,
                    Outcome::Failed { message: text },
                )
            }
        };
        let marker_key = match entry.kind {
            JobKind::Table => key!("marker-export-table"),
            JobKind::Dxf => key!("marker-export-dxf"),
            _ => key!("marker-export"),
        };
        let marker = matches!(status, JobStatus::Done { .. })
            .then(|| strings::format(marker_key, &[&entry.name]))
            .filter(|_| same_session)
            .map(|text| HistoryMarker { rev: entry.rev, text });
        entry.status = status;
        if matches!(outcome, Outcome::Failed { .. }) {
            entry.unseen_failure = Some(text.clone());
        }
        let offers = match action {
            Some((open, second)) => [Some(open), second],
            None => [None, None],
        };
        self.notes.push(Note::new("export", text).offering(offers));
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
        none: Key<'static>,
    ) -> Result<u64, Outcome> {
        match uint_param(p, "job")? {
            Some(n) if self.jobs.get(n).is_some() => Ok(n),
            Some(n) => Err(failed(
                key!("error-unknown-job"),
                &[&n.to_string(), &number_list(&self.jobs.numbers())],
            )),
            None => default(&self.jobs).ok_or(Outcome::Refused { reason: none.into() }),
        }
    }

    /// `jobs.show {on?}` (View ▸ Jobs, the status bar's job field): the Jobs pane in the bottom dock.
    pub(crate) fn act_jobs_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let shown = self.jobs_shown();
        let want = or_return!(bool_param(p, "on", !shown));
        if want != shown {
            let layout = if want {
                self.panes.layout.with_bottom(PaneKind::Jobs)
            } else {
                self.panes.layout.without(PaneKind::Jobs)
            };
            self.replace_layout(layout);
        }
        Outcome::Done
    }

    /// `jobs.cancel {job?}`: sets the cancel flag of a running job (the newest by default); its end arrives later.
    pub(crate) fn act_jobs_cancel(&mut self, p: &Map<String, Value>) -> Outcome {
        let number = match self.job_number(
            p,
            |j| j.newest_running().map(|e| e.number),
            key!("reason-no-running-job"),
        ) {
            Ok(n) => n,
            Err(e) => return e,
        };
        let Some(entry) = self.jobs.get(number) else {
            return Outcome::refused(key!("reason-job-finished"));
        };
        match (&entry.handle, &entry.cancel) {
            (Some(h), _) => {
                h.cancel();
                Outcome::Done
            }
            // A print the platform runs ends, aborted, at its next band.
            (None, Some(cancel)) if entry.is_running() => {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                Outcome::Done
            }
            _ => Outcome::refused(key!("reason-job-finished")),
        }
    }

    /// `jobs.wait {job?}` (headless only): waits for a job and takes its end; by default the newest job no step has
    /// waited for, whether it still runs or a poll already took its end, so neither the choice nor the outcome depends
    /// on which poll took an end. Done at once when there is none.
    pub(crate) fn act_jobs_wait(&mut self, p: &Map<String, Value>) -> Outcome {
        let unseen = |j: &JobsState| j.entries().filter(|e| !e.waited).last().map(|e| e.number);
        let number = match self.job_number(p, unseen, key!("reason-no-running-job")) {
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
    pub(crate) fn act_jobs_open_result(&mut self, p: &Map<String, Value>) -> Outcome {
        let newest_done = |j: &JobsState| {
            j.entries()
                .filter(|e| matches!(e.status, JobStatus::Done { .. }))
                .last()
                .map(|e| e.number)
        };
        let number = or_return!(self.job_number(p, newest_done, key!("reason-no-result")));
        let Some(entry) = self.jobs.get(number) else {
            return Outcome::refused(key!("reason-no-result"));
        };
        if !matches!(entry.status, JobStatus::Done { .. }) {
            return Outcome::refused(key!("reason-no-result"));
        }
        if self.mode == Mode::Headless {
            return Outcome::refused(key!("reason-no-opener"));
        }
        self.jobs.open_request = Some(entry.path.clone());
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // The recovery dialog

    /// `journal.keep` (Open read-only, Escape): the dialog closes; the project stays read-only and the journal stays.
    pub(crate) fn act_journal_keep(&mut self) -> Outcome {
        if matches!(self.dialog, Some(Dialog::Recovery { .. })) {
            self.dialog = None;
        }
        Outcome::Done
    }

    /// `journal.recover` (Recover): the read-only session closes and the project re-opens with `Recovery::Apply`. When
    /// the opened store still reports the leftover journal (the replay failed) or the open is not an editing one, that
    /// session closes again (the leftover stays), the project re-opens read-only and the dialog shows the reason with
    /// Open read-only and Discard… only. Otherwise the project is open for editing and dirty, with a notification
    /// and a history marker.
    pub(crate) fn act_journal_recover(&mut self) -> Outcome {
        let Some(Dialog::Recovery {
            name,
            transactions,
            base_matches,
            last_change_ms,
            failed: None,
        }) = self.dialog.clone()
        else {
            return Outcome::refused(key!("reason-recovery-failed"));
        };
        let Some(dir) = self.project.as_ref().map(|p| p.dir.clone()) else {
            return Outcome::refused(key!("reason-no-project"));
        };
        self.dialog = None;
        // The read-only session holds no lock and owns no journal.
        self.close_project();
        let opened = self.open_for_editing_with(&dir, Recovery::Apply);
        let (session, report) = match opened {
            Ok(o) => o,
            Err(e) => {
                let message =
                    strings::format(key!("error-open"), &[&name, strings::text(open_error_key(&e))]);
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
                strings::text(key!("recovery-failed-replay"))
            } else {
                strings::text(key!("recovery-failed-moved"))
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
                    let message =
                        strings::format(key!("error-open"), &[&name, strings::text(open_error_key(&e))]);
                    self.banner = Some(message.clone());
                    return Outcome::Failed { message };
                }
            };
            self.install_project(
                &dir,
                again,
                AccessMode::ReadOnly {
                    reason_key: key!("readonly-recovery"),
                },
            );
            self.banner = Some(strings::text(key!("banner-recovery")).to_owned());
            self.dialog_gen += 1;
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
        self.notify_with(
            strings::format_count(count_key!("toast-recovered"), count),
            "recovery",
            false,
        );
        if let Some(rev) = self.revision() {
            self.markers.push(HistoryMarker {
                rev,
                text: strings::format_count(count_key!("marker-recovered"), count),
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
            sections: Vec::new(),
        });
        self.install_view();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A relative target is made absolute when the job starts, so its `file://` URL is
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
