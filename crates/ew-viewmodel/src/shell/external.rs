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
use ew_api::{DiskPoll, ExternalApplied};
use serde_json::{Map, Value, json};

use super::jobs::bool_param;
use super::{Dialog, Outcome, Prompt, RecordedCommand, ShellModel, failed, string_param};
use crate::key;
use crate::notifications::{NoteAction, Validity};
use crate::strings;

/// One colliding field in the collision dialog (D24).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CollisionRow {
    /// The record in words, with its derived tags (`connection -102G1:+ → -102F1:1`).
    pub record: String,
    /// The member's path (`overrides.colour`), or the record's absence on one side in words.
    pub field: String,
    /// The value at the baseline, as text.
    pub base: String,
    /// The value in this window.
    pub mine: String,
    /// The value on disk.
    pub theirs: String,
    /// The side chosen.
    pub take: Take,
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
    /// The banner a held external change set (it goes when the change is applied).
    pub(crate) held_banner: Option<String>,
    /// The migration chain the shell's stores open with, set by a test (feature `testkit`); `None`: the shipped one.
    pub(crate) migrations: Option<&'static ew_api::Migrations>,
}

/// The most polls `project.check_disk` makes while the change settles.
const CHECK_POLLS: usize = 5;

/// A value as the collision dialog shows it: compact JSON, or the word for an absent one.
fn value_text(v: Option<&Value>) -> String {
    v.map_or_else(
        || strings::text(key!("external-absent")).to_owned(),
        Value::to_string,
    )
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
        settling
    }

    /// `project.check_disk` (File ▸ Check for changes on disk): polls now, and while the change settles again after
    /// [`QUIESCENCE`], as the window's timer would.
    pub(crate) fn act_check_disk(&mut self) -> Outcome {
        if self.project.is_none() {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
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
                self.open_collisions();
                self.hold_banner(strings::text(key!("banner-external-collisions")).to_owned());
            }
            DiskPoll::Unreadable { retrying: false } => {
                self.hold_banner(strings::text(key!("banner-external-unreadable")).to_owned());
            }
            DiskPoll::Reloaded => self.reloaded(),
            DiskPoll::Unchanged => self.release_banner(),
            DiskPoll::Settling | DiskPoll::GitBusy | DiskPoll::Unreadable { retrying: true } => {}
        }
    }

    /// Shows the banner of a held change.
    fn hold_banner(&mut self, text: String) {
        self.banner = Some(text.clone());
        self.collab.held_banner = Some(text);
    }

    /// Takes the banner of a held change away once it is no longer held, and a refused save's banner that named it.
    fn release_banner(&mut self) {
        let held = self.collab.held_banner.take();
        let refusals = [key!("saveerr-external-collisions"), key!("saveerr-external-unreadable")]
            .map(|k| strings::format(key!("error-save"), &[strings::text(k)]));
        if let Some(b) = self.banner.as_deref()
            && (held.as_deref() == Some(b) || refusals.iter().any(|r| r == b))
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
        let records = strings::format_count(crate::count_key!("external-records"), a.records);
        let mut text = if a.folios.is_empty() {
            strings::format(key!("toast-external"), &[&records])
        } else {
            strings::format(key!("toast-external-folios"), &[&records, &and_list(&a.folios)])
        };
        if let Some(hash) = &a.commit {
            text.push_str(&strings::format(key!("toast-external-git"), &[hash]));
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
    fn open_collisions(&mut self) {
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
                let label = p
                    .session
                    .record_label(kind, id.as_uuid())
                    .map_or_else(|| kind.name().to_owned(), |l| format!("{} {l}", kind.name()));
                CollisionRow {
                    record: label,
                    field: if c.path.is_empty() {
                        strings::text(key!("external-whole-record")).to_owned()
                    } else {
                        c.path.clone()
                    },
                    base: value_text(c.base.as_ref()),
                    mine: value_text(c.mine.as_ref()),
                    theirs: value_text(c.theirs.as_ref()),
                    take: Take::Mine,
                }
            })
            .collect();
        self.palette = None;
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
            return Outcome::Refused {
                reason_key: key!("reason-no-external-change"),
            };
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
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        if project.session.external_collisions().is_none() {
            return Outcome::Refused {
                reason_key: key!("reason-no-external-change"),
            };
        }
        match project.session.apply_external(&choices) {
            Ok(applied) => {
                self.external_applied(&applied);
                Outcome::Done
            }
            Err(e) => {
                let message = strings::format(key!("error-external-apply"), &[&e.to_string()]);
                Outcome::Failed { message }
            }
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
        let env = self.collab.cloud_env.clone();
        let lookup = move |name: &str| match &env {
            Some(vars) => vars.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone()),
            None => crate::settings::env_path(name).map(PathBuf::into_os_string),
        };
        let cloud = ew_api::cloud::detect(&ew_api::OsVfs, &lookup, dir);
        if let Some(p) = self.project.as_mut() {
            p.session.set_cloud_service(cloud);
        }
        self.collab.driver_missing = driver_missing(dir);
        let mut banners = Vec::new();
        if migrated.is_some() {
            banners.push(strings::text(key!("banner-older-format")).to_owned());
        }
        match cloud {
            Some(service) => banners.push(match service.name() {
                Some(name) => strings::format(key!("banner-cloud"), &[name]),
                None => strings::text(key!("banner-cloud-other")).to_owned(),
            }),
            None => {}
        }
        (!banners.is_empty()).then(|| banners.join(" "))
    }

    /// Whether the status bar shows "Merge driver not configured" (doc 05 §16): the open project is in a git worktree
    /// whose git configuration has no `merge.elektrawatt.driver`.
    pub fn merge_driver_missing(&self) -> bool {
        self.project.is_some() && self.collab.driver_missing
    }

    /// The environment the cloud detection reads from now on, instead of the process's (feature `testkit`).
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
        let confirm = match bool_param(p, "confirm", false) {
            Ok(c) => c,
            Err(e) => return e,
        };
        let Some(project) = self.project.as_ref() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        let dir = project.dir.clone();
        let Ok(Some(root)) = ew_api::git::worktree_root(&ew_api::OsVfs, &dir) else {
            return Outcome::Refused {
                reason_key: key!("reason-not-in-git"),
            };
        };
        if !confirm {
            self.palette = None;
            self.dialog = Some(Dialog::ConfirmGitSetup);
            return Outcome::Done;
        }
        let Some(ew) = self.collab.ew_exe.clone().or_else(ew_beside_program) else {
            return Outcome::Refused {
                reason_key: key!("reason-no-ew"),
            };
        };
        if matches!(self.dialog, Some(Dialog::ConfirmGitSetup)) {
            self.dialog = None;
        }
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
        self.dialog = Some(Dialog::ConfirmUpgrade {
            from: from.to_owned(),
        });
        Outcome::Done
    }

    /// `file.upgrade` (the upgrade dialog's Upgrade and save): the consent, then the save, which writes the backup
    /// beside the project folder first.
    pub(crate) fn act_upgrade(&mut self) -> Outcome {
        let Some(p) = self.project.as_mut() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
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
        let replace = match bool_param(p, "replace", false) {
            Ok(r) => r,
            Err(e) => return e,
        };
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        if super::jobs::inside(&project.dir, &path) {
            return Outcome::Refused {
                reason_key: key!("reason-export-into-project"),
            };
        }
        if path.exists() && !replace {
            return Outcome::Refused {
                reason_key: key!("reason-file-exists"),
            };
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
