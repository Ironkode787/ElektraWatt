// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.17 part 7 (D24, D25, the driver status; decision log ID-1284): the window's half of collaboration.
//!
//! - **External change** (doc 05 §9, §16): [`ShellModel::poll_disk`] polls the project on disk (the window every
//!   [`ew_api::external::POLL_EVERY`] and when it regains the focus, never during a gesture; `project.check_disk` at
//!   once). An applied change is a toast "External change applied: …" with Undo and Show changes; a collision opens the
//!   collision dialog (mine or theirs per field, All mine, All theirs, Decide later) and a banner says saves wait; an
//!   unreadable change is a banner (D21); a read-only window reloads and keeps its page, zoom and selection where the
//!   records exist.
//! - **Cloud folders** (D25): a project in a cloud-synced folder shows a banner at open, and its session reads an
//!   unreadable change again before saves wait. Online-only placeholders are not detected (de-scoped, §14.4).
//! - **`.ewz`** (D24): File ▸ Export .ewz… (`file.export_ewz`) writes the saved generation; File ▸ Open .ewz…
//!   (`file.open_ewz`) asks Extract and open or Open read-only.
//! - **The upgrade** (D11, D24): the first save of a project in an older format asks once (`file.upgrade`).
//! - **The merge driver** (doc 05 §11.1, §16): the status bar's "Merge driver not configured" for a project in a git
//!   worktree without `merge.elektrawatt.driver`; "Set up merge driver" (`git.setup`) asks, then writes the
//!   configuration and the managed blocks as `ew git setup` does.

use std::path::{Path, PathBuf};

use ew_api::external::{QUIESCENCE, Take};
use ew_api::{DiskPoll, ExternalApplied, ExternalError};
use serde_json::{Map, Value, json};

use super::jobs::bool_param;
use super::{Dialog, Outcome, Prompt, RecordedCommand, ShellModel, failed, string_param};
use crate::key;
use crate::notifications::{NoteAction, Validity};
use crate::strings;

/// One colliding field in the collision dialog (D24).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CollisionRow {
    /// The record in words: its kind and its label in the property grid's words (`connection 102.1`).
    pub record: String,
    /// The field in the property grid's words (`Colour override`), or the whole record when one side deleted it.
    pub field: String,
    /// The value at the baseline, as text.
    pub base: String,
    /// The value in this window.
    pub mine: String,
    /// The value on disk.
    pub theirs: String,
    /// Whether the row is the whole record (one side deleted it), not one field.
    pub whole: bool,
    /// The side chosen.
    pub take: Take,
}

/// The rows of the collision dialog counted as they read (W49.3-F4): `2 fields and 1 record`, a whole-record row
/// counted as a record.
pub(crate) fn collision_count(rows: &[CollisionRow]) -> String {
    let records = rows.iter().filter(|r| r.whole).count();
    let fields = rows.len() - records;
    let mut parts = Vec::new();
    if fields > 0 {
        parts.push(strings::format_count(
            crate::count_key!("external-fields"),
            fields,
        ));
    }
    if records > 0 {
        parts.push(strings::format_count(
            crate::count_key!("external-records"),
            records,
        ));
    }
    and_list(&parts)
}

/// The collaboration state of the shell.
#[derive(Default)]
pub(crate) struct CollabState {
    /// The environment the cloud detection reads, set by a test (feature `testkit`); `None`: the process's.
    pub(crate) cloud_env: Option<Vec<(String, std::ffi::OsString)>>,
    /// The `ew` executable `git.setup` configures, set by a test; `None`: the one beside this program.
    pub(crate) ew_exe: Option<PathBuf>,
    /// Whether the open project is in a git worktree whose git configuration has no merge driver.
    pub(crate) driver_missing: bool,
    /// The banner of a state that holds ([`ShellModel::refresh_state_banner`]): saves held by a change on disk, or the
    /// cloud-synced folder. It shows under a message banner and stays while its state holds (W49.3-F2).
    pub(crate) state_banner: Option<String>,
    /// Whether the user dismissed the cloud folder's banner (until the next open).
    pub(crate) cloud_dismissed: bool,
    /// The damaged library copy that made the project of this epoch read-only (FT-1383), kept from its open: its banner
    /// is a state banner and stays while the project is open (ID-1427, W52 F7).
    pub(crate) damaged: Option<(u64, ew_api::DamagedCopy)>,
    /// Whether the collision dialog of the held change was shown: a poll opens it once, never over another dialog and
    /// never again after Decide later (a refused save opens it again).
    pub(crate) collisions_shown: bool,
    /// The migration chain the shell's stores open with, set by a test (feature `testkit`); `None`: the shipped one.
    pub(crate) migrations: Option<&'static ew_api::Migrations>,
    /// The released formats the shell's stores save with, set by a test (feature `testkit`); `None`: the shipped list.
    pub(crate) released: Option<&'static [&'static str]>,
}

/// The most polls `project.check_disk` makes while the change settles.
const CHECK_POLLS: usize = 5;

/// A side of a collision as the dialog shows it: for a field its value with record IDs named ([`value_text`]); for the
/// whole record (one side deleted it) whether that side holds it, changed or deleted.
fn side_text(session: &ew_api::Session, whole: bool, base: bool, v: Option<&Value>) -> String {
    match (whole, v) {
        (false, v) => value_text(v.map(|v| session.named_value(v)).as_ref()),
        (true, None) => strings::text(key!("external-deleted")).to_owned(),
        (true, Some(_)) if base => strings::text(key!("external-present")).to_owned(),
        (true, Some(_)) => strings::text(key!("external-changed")).to_owned(),
    }
}

/// A value as the collision dialog shows it: a text as it is, a list of texts joined with commas, any other value as
/// its JSON text, or the word for an absent one.
fn value_text(v: Option<&Value>) -> String {
    let text = |v: &Value| v.as_str().map_or_else(|| v.to_string(), str::to_owned);
    match v {
        None => strings::text(key!("external-absent")).to_owned(),
        Some(Value::Array(items)) => items.iter().map(text).collect::<Vec<_>>().join(", "),
        Some(v) => text(v),
    }
}

impl ShellModel {
    /// Polls the open project for an external change (D20, D22): the window calls it every
    /// [`ew_api::external::POLL_EVERY`], when it regains the focus and, while it answers `true`, again after
    /// [`QUIESCENCE`]; never during a gesture. `true` while a change is settling.
    pub fn poll_disk(&mut self) -> bool {
        let Some(project) = self.project.as_mut() else {
            return false;
        };
        let poll = project.session.poll_disk();
        let settling = matches!(poll, DiskPoll::Settling | DiskPoll::Unreadable { retrying: true });
        self.disk_changed(poll);
        self.refresh_state_banner();
        settling
    }

    /// `project.check_disk` (File ▸ Check for changes on disk): polls now, and while the change settles again after
    /// [`QUIESCENCE`], as the window's timer would.
    pub(crate) fn act_check_disk(&mut self) -> Outcome {
        if self.project.is_none() {
            return Outcome::refused(key!("reason-no-project"));
        }
        for _ in 0..CHECK_POLLS {
            if !self.poll_disk() {
                break;
            }
            std::thread::sleep(QUIESCENCE);
        }
        Outcome::Done
    }

    /// What a poll found, in the window.
    fn disk_changed(&mut self, poll: DiskPoll) {
        match poll {
            DiskPoll::Applied(applied) => self.external_applied(&applied),
            DiskPoll::Collisions => {
                // A new change: its dialog replaces one of an earlier change, never another dialog.
                self.collab.collisions_shown = false;
                if self.dialog.is_none() || matches!(self.dialog, Some(Dialog::ExternalChange { .. })) {
                    self.open_collisions();
                }
            }
            // The held change is unchanged: its dialog opens once another dialog has closed, never after Decide later.
            DiskPoll::Held => {
                if !self.collab.collisions_shown && self.dialog.is_none() {
                    self.open_collisions();
                }
            }
            DiskPoll::Reloaded => self.reloaded(),
            // The disk is back to the baseline: nothing is held, so the collision dialog has nothing to apply.
            DiskPoll::Unchanged => {
                self.release_banner();
                if matches!(self.dialog, Some(Dialog::ExternalChange { .. })) {
                    self.dialog = None;
                }
            }
            DiskPoll::Settling | DiskPoll::GitBusy | DiskPoll::Unreadable { .. } => {}
        }
    }

    /// The banner of the state that holds, in the UI language (doc 16a §6.11; W49.3-F2): saves held by a change on disk
    /// that collides (D24) or cannot be read (D21), else the cloud-synced folder (D25) until the user dismisses it. It
    /// stays while its state holds, whatever action runs; only resolving the state (or Dismiss, for the cloud folder)
    /// takes it away.
    pub(crate) fn refresh_state_banner(&mut self) {
        let text = self.project.as_ref().and_then(|p| {
            if p.session.external_collisions().is_some() {
                return Some(strings::text(key!("banner-external-collisions")).to_owned());
            }
            if p.session.external_unreadable() {
                return Some(strings::text(key!("banner-external-unreadable")).to_owned());
            }
            if let Some(text) = self.damaged_state(p.epoch).map(super::damaged_banner) {
                return Some(text);
            }
            if self.collab.cloud_dismissed {
                return None;
            }
            p.session.cloud_service().map(|service| match service.name() {
                Some(name) => strings::format(key!("banner-cloud"), &[name]),
                None => strings::text(key!("banner-cloud-other")).to_owned(),
            })
        });
        self.collab.state_banner = text;
    }

    /// Whether the collisions of a change on disk wait for a choice: the held-saves banner's Primary reopens their dialog
    /// (`external.review`, doc 16a §6.11).
    pub fn external_choice_waiting(&self) -> bool {
        self.project
            .as_ref()
            .is_some_and(|p| p.session.external_collisions().is_some())
    }

    /// Whether the damaged-copy banner offers Update from library (`library.update_damaged`, FT-1499, ID-1486): the
    /// open project is read-only for a damaged library copy and an installed library holds one the open dropped.
    pub fn damaged_update_offered(&self) -> bool {
        self.project
            .as_ref()
            .and_then(|p| self.damaged_state(p.epoch))
            .is_some_and(|d| d.in_library)
    }

    /// Whether the banner shown may be dismissed: a message, or the cloud folder's; never saves held by a change on disk,
    /// which only a choice (or the disk) resolves.
    pub fn banner_dismissable(&self) -> bool {
        self.banner.is_some()
            || !self.project.as_ref().is_some_and(|p| {
                p.session.external_collisions().is_some()
                    || p.session.external_unreadable()
                    || self.damaged_state(p.epoch).is_some()
            })
    }

    /// The damaged library copy that made the open project of `epoch` read-only (ID-1427, W52 F7).
    fn damaged_state(&self, epoch: u64) -> Option<&ew_api::DamagedCopy> {
        self.collab
            .damaged
            .as_ref()
            .filter(|(e, _)| *e == epoch)
            .map(|(_, d)| d)
    }

    /// Dismisses the state banner where that is allowed (the cloud folder's).
    pub(super) fn dismiss_state_banner(&mut self) {
        if self.banner_dismissable() {
            self.collab.cloud_dismissed = true;
        }
        self.refresh_state_banner();
    }

    /// `external.review` (the held-saves banner's Choose…): the collision dialog again, mine chosen.
    pub(crate) fn act_external_review(&mut self) -> Outcome {
        if !self.external_choice_waiting() {
            return Outcome::refused(key!("reason-no-external-change"));
        }
        // D24: never over another dialog (C49-p10b#6).
        if self.dialog.is_some() {
            return Outcome::refused(key!("reason-dialog-open"));
        }
        self.open_collisions();
        Outcome::Done
    }

    /// Takes a refused save's banner that named a held change away once it is no longer held.
    fn release_banner(&mut self) {
        let refusals = [
            key!("saveerr-external-collisions"),
            key!("saveerr-external-unreadable"),
        ]
        .map(|k| strings::format(key!("error-save"), &[strings::text(k)]));
        if self
            .banner
            .as_deref()
            .is_some_and(|b| refusals.iter().any(|r| r == b))
        {
            self.banner = None;
        }
    }

    /// An applied external change: the views follow, the step is recorded, the toast offers Undo and Show changes.
    fn external_applied(&mut self, a: &ExternalApplied) {
        self.release_banner();
        if matches!(self.dialog, Some(Dialog::ExternalChange { .. })) {
            self.dialog = None;
        }
        self.refresh();
        let Some(p) = self.project.as_ref() else {
            return;
        };
        let epoch = p.epoch;
        let entry = a
            .entry
            .and_then(|i| Some((i, p.session.history().entries().get(i)?.rev().0)));
        if entry.is_some() {
            self.record.steps.push(RecordedCommand {
                cmd: "external.apply".to_owned(),
                params: json!({ "generation": a.generation }),
            });
        }
        let records = strings::format_count(crate::count_key!("external-changes"), a.records);
        // W49.3-F5: nothing of theirs was taken (All mine): the window says it kept the user's version.
        // C50-p1#4: nothing taken and nothing kept (the same edit made on both sides) has words of its own.
        let mut text = if a.records == 0 && a.kept == 0 {
            strings::text(key!("toast-external-same")).to_owned()
        } else if a.records == 0 && a.kept > 0 {
            let kept = strings::format_count(crate::count_key!("external-changes"), a.kept);
            strings::format(key!("toast-external-kept"), &[&kept])
        } else if a.folios.is_empty() {
            strings::format(key!("toast-external"), &[&records])
        } else if let [folio] = a.folios.as_slice() {
            strings::format(key!("toast-external-folio"), &[&records, folio])
        } else {
            strings::format(key!("toast-external-folios"), &[&records, &and_list(&a.folios)])
        };
        if let Some(hash) = &a.commit {
            // FT-1505: the commit goes inside the sentence, before its full stop.
            text = format!(
                "{}{}.",
                strings::without_stop(&text),
                strings::format(key!("toast-external-git"), &[hash])
            );
        }
        let (undo, show) = match entry {
            Some((index, rev)) => (
                Some((
                    NoteAction::Undo,
                    Validity::Undo {
                        epoch,
                        cursor: index + 1,
                        rev,
                    },
                )),
                Some((
                    NoteAction::ShowChanges,
                    Validity::History {
                        epoch,
                        entry: index,
                        rev,
                    },
                )),
            ),
            None => (None, None),
        };
        self.notes.push_two(text, "external", undo, show);
    }

    /// Opens the collision dialog with every colliding field, mine chosen (D24).
    pub(super) fn open_collisions(&mut self) {
        let Some(p) = self.project.as_ref() else {
            return;
        };
        let Some(collisions) = p.session.external_collisions() else {
            return;
        };
        let rows = collisions
            .iter()
            .map(|c| {
                let (kind, id) = c.key;
                let words = strings::kind_heading(kind.name());
                let label = p
                    .session
                    .record_label(kind, id.as_uuid())
                    .map_or_else(|| words.clone(), |l| format!("{words} {l}"));
                let whole = c.path.is_empty();
                let side = |base: bool, v: Option<&Value>| side_text(&p.session, whole, base, v);
                CollisionRow {
                    record: label,
                    field: if whole {
                        strings::text(key!("external-whole-record")).to_owned()
                    } else {
                        strings::field_label(kind.name(), &c.path)
                    },
                    base: side(true, c.base.as_ref()),
                    mine: side(false, c.mine.as_ref()),
                    theirs: side(false, c.theirs.as_ref()),
                    whole,
                    take: Take::Mine,
                }
            })
            .collect();
        self.palette = None;
        self.collab.collisions_shown = true;
        self.dialog_gen += 1;
        self.dialog = Some(Dialog::ExternalChange { rows });
    }

    /// `external.choose {row, take}`: the side of one row of the collision dialog (from 1).
    pub(crate) fn act_external_choose(&mut self, p: &Map<String, Value>) -> Outcome {
        let take = match string_param(p, "take") {
            Err(e) => return e,
            Ok(Some("mine")) => Take::Mine,
            Ok(Some("theirs")) => Take::Theirs,
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["take"]),
            Ok(None) => return failed(key!("error-missing-param"), &["take"]),
        };
        let row = p
            .get("row")
            .and_then(Value::as_u64)
            .and_then(|r| usize::try_from(r).ok());
        let Some(Dialog::ExternalChange { rows }) = &mut self.dialog else {
            return Outcome::refused(key!("reason-no-external-change"));
        };
        match row.and_then(|r| r.checked_sub(1)).and_then(|r| rows.get_mut(r)) {
            Some(r) => {
                r.take = take;
                Outcome::Done
            }
            None => failed(key!("error-bad-param"), &["row"]),
        }
    }

    /// `external.apply {take?}`: applies the held change with the dialog's choices, or with `mine` or `theirs` for
    /// every field (All mine, All theirs), as one step.
    pub(crate) fn act_external_apply(&mut self, p: &Map<String, Value>) -> Outcome {
        let all = match string_param(p, "take") {
            Err(e) => return e,
            Ok(Some("mine")) => Some(Take::Mine),
            Ok(Some("theirs")) => Some(Take::Theirs),
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["take"]),
            Ok(None) => None,
        };
        let choices: Vec<Take> = match (&self.dialog, all) {
            (Some(Dialog::ExternalChange { rows }), all) => {
                rows.iter().map(|r| all.unwrap_or(r.take)).collect()
            }
            (_, Some(all)) => {
                let n = self
                    .project
                    .as_ref()
                    .and_then(|p| p.session.external_collisions())
                    .map_or(0, <[_]>::len);
                vec![all; n]
            }
            (_, None) => Vec::new(),
        };
        let Some(project) = self.project.as_mut() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        if project.session.external_collisions().is_none() {
            return Outcome::refused(key!("reason-no-external-change"));
        }
        match project.session.apply_external(&choices) {
            Ok(applied) => {
                self.external_applied(&applied);
                Outcome::Done
            }
            Err(ExternalError::Nothing) => Outcome::refused(key!("reason-no-external-change")),
            // The collisions changed since they were shown: the dialog shows the new ones for a new choice.
            Err(ExternalError::Changed) => {
                self.open_collisions();
                failed(key!("error-external-changed"), &[])
            }
            Err(ExternalError::Choices(_)) => failed(key!("error-external-choices"), &[]),
            Err(ExternalError::Command(e)) => super::tools::command_failed(&e, &Value::Null),
        }
    }

    /// A read-only window after its session reloaded (D23, D24): the page, the zoom and the selection stay where their
    /// records exist; the views follow the new generation.
    fn reloaded(&mut self) {
        // The session's model moved on by one revision: the canvases replace their pages and keep their views.
        if let Some(p) = self.project.as_mut() {
            p.labels = p.session.page_labels();
        }
        let exists = |s: &ew_api::Session, e: &ew_api::output::ew_dl::EntityRef| s.record_exists(e.0);
        if let Some(p) = self.project.as_ref() {
            self.tools.selection.items.retain(|e| exists(&p.session, e));
            if self
                .tools
                .selection
                .primary
                .is_some_and(|e| !exists(&p.session, &e))
            {
                self.tools.selection.primary = None;
            }
        }
        self.tools.selection.generation += 1;
        self.refresh();
    }

    // -------------------------------------------------------------------------------------------------------------
    // Opening: the cloud banner, the upgrade banner, the driver status

    /// What the window learns when a project opened in `dir`: the cloud-synced folder (D25) and the merge driver's status
    /// (doc 05 §16); the banner for a project of an older format (D11). Returns the banner's text to add, if any.
    pub(super) fn opened_extras(&mut self, dir: &Path, migrated: Option<&str>) -> Option<String> {
        let cloud = self.cloud_of(dir);
        if let Some(p) = self.project.as_mut() {
            p.session.set_cloud_service(cloud);
        }
        // Only a window that edits can commit a merge: a read-only one shows no driver status.
        let edits = self
            .project
            .as_ref()
            .is_some_and(|p| matches!(p.access, super::AccessMode::Edit));
        self.collab.driver_missing = edits && driver_missing(dir);
        // ID-1427 (W52 F7): a damaged library copy's banner is a state banner too, while the project stays open.
        self.collab.damaged = self.project.as_ref().and_then(|p| {
            let integrity = matches!(p.access, super::AccessMode::ReadOnly { reason_key }
                if reason_key == key!("readonly-integrity"));
            integrity
                .then(|| p.session.damaged_copy())
                .flatten()
                .map(|d| (p.epoch, d))
        });
        // The cloud folder's banner is a state banner (W49.3-F2).
        self.collab.cloud_dismissed = false;
        self.refresh_state_banner();
        migrated.map(|_| strings::text(key!("banner-older-format")).to_owned())
    }

    /// An environment variable as the window reads it (the cloud detection, `$APPIMAGE`): a test's environment when one
    /// is set (feature `testkit`), else the process's; `None` when unset or empty.
    fn env_var(&self, name: &str) -> Option<std::ffi::OsString> {
        match &self.collab.cloud_env {
            Some(vars) => vars
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.clone())
                .filter(|v| !v.is_empty()),
            None => crate::settings::env_path(name).map(PathBuf::into_os_string),
        }
    }

    /// The cloud-synced folder the project folder `dir` lies in, if any (D25).
    pub(super) fn cloud_of(&self, dir: &Path) -> Option<ew_api::cloud::CloudService> {
        ew_api::cloud::detect(&ew_api::OsVfs, &|name| self.env_var(name), dir)
    }

    /// Whether the status bar shows "Merge driver not configured" (doc 05 §16): the open project is in a git worktree
    /// whose git configuration has no `merge.elektrawatt.driver`.
    pub fn merge_driver_missing(&self) -> bool {
        self.project.is_some() && self.collab.driver_missing
    }

    /// The environment the window reads from now on (the cloud detection, `$APPIMAGE`), instead of the process's
    /// (feature `testkit`).
    #[cfg(feature = "testkit")]
    pub fn set_cloud_env_for_tests(&mut self, vars: Option<Vec<(String, std::ffi::OsString)>>) {
        self.collab.cloud_env = vars;
    }

    /// The migration chain of the stores the shell opens from now on, instead of the shipped one (feature `testkit`): a
    /// test's chain makes a project of an older format.
    #[cfg(feature = "testkit")]
    pub fn set_migrations_for_tests(&mut self, chain: Option<&'static ew_api::Migrations>) {
        self.collab.migrations = chain;
    }

    /// The released formats of the stores the shell opens from now on, instead of the shipped list (feature
    /// `testkit`): a list without this build's format makes every save mark its manifest `unstable` (FT-1154).
    #[cfg(feature = "testkit")]
    pub fn set_released_for_tests(&mut self, released: Option<&'static [&'static str]>) {
        self.collab.released = released;
    }

    /// The `ew` executable `git.setup` configures from now on, instead of the one beside this program (feature
    /// `testkit`).
    #[cfg(feature = "testkit")]
    pub fn set_ew_exe_for_tests(&mut self, path: Option<PathBuf>) {
        self.collab.ew_exe = path;
    }

    /// `git.setup {confirm?}` (the status bar's "Set up merge driver"): without `confirm` the dialog that asks; with it
    /// the four keys in the repository's git configuration and the managed blocks in the project folder, as
    /// `ew git setup` writes them (D28).
    pub(crate) fn act_git_setup(&mut self, p: &Map<String, Value>) -> Outcome {
        let confirm = or_return!(bool_param(p, "confirm", false));
        let Some(project) = self.project.as_ref() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let dir = project.dir.clone();
        let Ok(Some(root)) = ew_api::git::worktree_root(&ew_api::OsVfs, &dir) else {
            return Outcome::refused(key!("reason-not-in-git"));
        };
        if !confirm {
            self.palette = None;
            self.dialog_gen += 1;
            self.dialog = Some(Dialog::ConfirmGitSetup {
                folder: super::dir_name(&dir),
            });
            return Outcome::Done;
        }
        let Some(ew) = self.collab.ew_exe.clone().or_else(ew_beside_program) else {
            return Outcome::refused(key!("reason-no-ew"));
        };
        if matches!(self.dialog, Some(Dialog::ConfirmGitSetup { .. })) {
            self.dialog = None;
        }
        // From an AppImage, git runs the AppImage with `ew` first, never its transient mount (C49-p9#5, C49-p10#2).
        let [image, mount] = ["APPIMAGE", "APPDIR"].map(|name| self.env_var(name).map(PathBuf::from));
        let ew = ew_api::git::ew_command(&ew, image.as_deref(), mount.as_deref());
        let git = ew_api::git::GitExe::default();
        for (k, v) in ew_api::git::driver_config(&ew) {
            if let Err(e) = git.config_set(&root, k, &v, ew_api::git::ConfigScope::Repository) {
                let message = strings::format(key!("error-git-setup"), &[&e.to_string()]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        }
        if let Err(e) = ew_api::git::write_blocks(&ew_api::OsVfs, &dir) {
            let message = strings::format(key!("error-git-setup"), &[&e.to_string()]);
            self.banner = Some(message.clone());
            return Outcome::Failed { message };
        }
        self.collab.driver_missing = driver_missing(&dir);
        self.notes
            .push(strings::text(key!("toast-git-setup")).to_owned(), "edit", None);
        Outcome::Done
    }

    // -------------------------------------------------------------------------------------------------------------
    // The upgrade (D11, D24)

    /// The first save of a project of an older format: the dialog that asks once (a save refused for the consent).
    pub(super) fn ask_upgrade(&mut self, from: &str) -> Outcome {
        self.palette = None;
        self.dialog_gen += 1;
        self.dialog = Some(Dialog::ConfirmUpgrade {
            from: from.to_owned(),
        });
        Outcome::Done
    }

    /// `file.upgrade` (the upgrade dialog's Upgrade and save): the consent, then the save, which writes the backup
    /// beside the project folder first.
    pub(crate) fn act_upgrade(&mut self) -> Outcome {
        let Some(p) = self.project.as_mut() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        p.session.confirm_upgrade();
        if matches!(self.dialog, Some(Dialog::ConfirmUpgrade { .. })) {
            self.dialog = None;
        }
        let saved = self.act_save();
        if saved == Outcome::Done
            && self.banner.as_deref() == Some(strings::text(key!("banner-older-format")))
        {
            self.banner = None;
        }
        saved
    }

    // -------------------------------------------------------------------------------------------------------------
    // `.ewz` (D24)

    /// `file.export_ewz {path?, replace?}` (File ▸ Export .ewz…): the project's saved generation as an `.ewz`, written
    /// through a temporary file renamed over the target; a project with unsaved changes exports what is saved and says
    /// so.
    pub(crate) fn act_export_ewz(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => super::jobs::absolute_target(PathBuf::from(path)),
            Ok(None) => {
                let mut rest = p.clone();
                rest.insert("replace".to_owned(), Value::Bool(true));
                return self.need_path("file.export_ewz", Prompt::SaveEwz, "path", &rest);
            }
        };
        let replace = or_return!(bool_param(p, "replace", false));
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        if super::jobs::inside(&project.dir, &path) {
            return Outcome::refused(key!("reason-export-into-project"));
        }
        if path.exists() && !replace {
            return Outcome::refused(key!("reason-file-exists"));
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let export = match project.session.export_ewz() {
            Ok(e) => e,
            Err(e) => {
                let message = strings::format(key!("error-export-ewz"), &[&e.to_string()]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        if let Err(e) = write_replacing(&path, &export.bytes) {
            let message = strings::format(key!("error-export-ewz"), &[&e.to_string()]);
            self.banner = Some(message.clone());
            return Outcome::Failed { message };
        }
        let text = if export.unsaved {
            strings::format(key!("toast-ewz-unsaved"), &[&name])
        } else {
            strings::format(key!("toast-ewz"), &[&name])
        };
        self.notes.push(text, "export", None);
        Outcome::Done
    }

    /// `file.open_ewz {path?, how?}` (File ▸ Open .ewz…): without `how` the dialog that asks; `extract` unpacks it into
    /// a new folder beside it (`<name>.ew`, refused when that exists) and opens that; `read_only` opens it from memory.
    pub(crate) fn act_open_ewz(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.has_project()
            && let Some(outcome) = self.guard_unsaved("file.open_ewz", p)
        {
            return outcome;
        }
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => PathBuf::from(path),
            Ok(None) => return self.need_path("file.open_ewz", Prompt::OpenEwz, "path", p),
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match string_param(p, "how") {
            Err(e) => e,
            Ok(None) => {
                self.palette = None;
                self.dialog_gen += 1;
                self.dialog = Some(Dialog::OpenEwz {
                    path: path.to_string_lossy().into_owned(),
                    name,
                });
                Outcome::Done
            }
            Ok(Some("read_only")) => {
                self.dialog = None;
                self.open_dir(&path, true)
            }
            Ok(Some("extract")) => {
                self.dialog = None;
                let target = path.with_extension("ew");
                let folder = super::dir_name(&target);
                // A folder of that name refuses the extract before anything is written (D9); the messages name no path.
                let refused = if target.exists() {
                    Some(strings::format(key!("error-extract-exists"), &[&name, &folder]))
                } else {
                    ew_api::ewz::read_file(&ew_api::OsVfs, &path)
                        .ok()
                        .and_then(|archive| ew_api::ewz::extract(&ew_api::OsVfs, &archive, &target).ok())
                        .is_none()
                        .then(|| strings::format(key!("error-extract-ewz"), &[&name]))
                };
                match refused {
                    None => self.open_dir(&target, false),
                    Some(message) => {
                        self.banner = Some(message.clone());
                        Outcome::Failed { message }
                    }
                }
            }
            Ok(Some(_)) => failed(key!("error-bad-param"), &["how"]),
        }
    }
}

/// `a, b and c`: the folios of a toast.
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} {} {last}", rest.join(", "), strings::text(key!("list-and"))),
    }
}

/// Whether the project folder `dir` is in a git worktree whose git configuration has no merge driver (exit status
/// only); `false` outside a worktree or without git.
fn driver_missing(dir: &Path) -> bool {
    if !matches!(ew_api::git::worktree_root(&ew_api::OsVfs, dir), Ok(Some(_))) {
        return false;
    }
    ew_api::git::GitExe::default()
        .config_has(
            dir,
            "merge.elektrawatt.driver",
            ew_api::git::ConfigScope::Repository,
        )
        .is_ok_and(|set| !set)
}

/// The `ew` executable beside this program, if it is there.
fn ew_beside_program() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let ew = exe.with_file_name(format!("ew{}", std::env::consts::EXE_SUFFIX));
    ew.is_file().then_some(ew)
}

/// Writes `bytes` to `path` all or nothing: a temporary file beside it, synced, renamed over it; removed on a failure.
fn write_replacing(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp-ew");
    let temp = PathBuf::from(temp);
    let written = (|| {
        let mut f = std::fs::File::create(&temp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&temp, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written
}
