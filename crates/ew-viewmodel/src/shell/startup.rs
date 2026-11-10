// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The start-up offer (WP-S6.05 D10, D11; NFR-REL-13's "offer recovery at the next start", FR-UI-050's 0.1.0 part as
//! ID-1247 reads it, doc 05 §8.3's start-up half).
//!
//! At start (the window and the headless mode, not the smoke run) [`ShellModel::start_up`] makes the configuration
//! directory, the state directory, its `journal/` and its `crashes/` user-only, prunes the crash logs (older than 30
//! days, or beyond the 20 newest) and every project's leftover journals older than 30 days, and lists (a) the projects
//! whose leftover journals hold unsaved transactions, skipping a project whose lock is live, and (b) the newest crash
//! log that ends with the end line of a panic that ended its session ([`ENDED_LINE`]) and was not offered before. When
//! either is there the offer is pending: the window shows it as a dialog; headless it blocks no other action and
//! `ui.describe` lists it. `startup.recover {project}` opens a project as Open does (WP-S2.09's recovery dialog
//! follows), `startup.open_log_folder` and `startup.delete_log` act on the log, `startup.dismiss` closes the offer.
//! When the offer closes, its log is renamed `<stem>.offered.log` and never offered again; the journals stay until
//! they are recovered, discarded or pruned.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::{Map, Value, json};

use super::{Mode, Outcome, ShellModel, failed};
use crate::dialogs::{DialogAnswer, DialogButton, DialogSpec};
use crate::key;
use crate::strings;

/// The directory of the crash logs under the state directory.
pub const CRASH_DIR: &str = "crashes";

/// The line a crash log ends with when its last panic ended the session (§5 L8; English, read by support).
pub const ENDED_LINE: &str = "Ended: this panic ended the session";

/// The crash logs kept: the newest 20.
pub const KEEP_LOGS: usize = 20;

/// The age after which a crash log is deleted at start (30 days, as a leftover journal).
pub const KEEP_LOG_AGE: Duration = Duration::from_secs(30 * 24 * 3600);

/// The end of the name of a crash log that was offered.
const OFFERED: &str = ".offered.log";

/// A project of the offer: one whose leftover journal holds unsaved transactions.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OfferedProject {
    /// The project directory.
    pub path: PathBuf,
    /// Its directory name.
    pub name: String,
    /// The unsaved transactions.
    pub transactions: usize,
    /// The time of the last change, epoch milliseconds (0: unknown).
    pub last_change_ms: u64,
}

/// The crash log of the offer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OfferedLog {
    /// The log file.
    pub path: PathBuf,
    /// When the session ended, `yyyy-mm-dd hh:mm` in UTC (the time of its last entry).
    pub when: String,
}

/// The pending start-up offer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StartupOffer {
    /// The projects with unsaved work, newest last change first.
    pub projects: Vec<OfferedProject>,
    /// The crash log, until it is deleted.
    pub log: Option<OfferedLog>,
    /// The refusal of the last answer (S7), shown in the offer.
    pub refusal: Option<String>,
    /// The older logs that also ended their session and were not offered (a crash loop at start): marked offered with
    /// the offered one, so that no later start shows an older crash.
    pub earlier_logs: Vec<PathBuf>,
}

impl StartupOffer {
    /// The folder of the crash logs, as the offer names it.
    pub fn log_folder(&self) -> Option<&Path> {
        self.log.as_ref().and_then(|l| l.path.parent())
    }

    /// The crash line (S3 with a project, S4 for a log alone), `None` without a log.
    pub fn crash_line(&self) -> Option<String> {
        let log = self.log.as_ref()?;
        let folder = self
            .log_folder()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        let key = if self.projects.is_empty() {
            key!("startup-crash-alone")
        } else {
            key!("startup-crash-project")
        };
        Some(strings::format(key, &[&log.when, &folder]))
    }

    /// The row of the `n`th project (S5).
    pub fn project_row(project: &OfferedProject) -> String {
        let changes = strings::format_count(crate::count_key!("startup-unsaved"), project.transactions);
        strings::format(
            key!("startup-project-row"),
            &[&project.name, &changes, &utc_minutes(project.last_change_ms)],
        )
    }

    /// The texts and buttons of the offer (doc 16a §6.12): the title (S1 with a project, S2 for a log alone), the crash
    /// line, then Recover (the newest project) as the Primary with a project, Not now, and with a log Open log folder
    /// and Delete log. Not now is the Primary of a log alone.
    pub fn spec(&self) -> DialogSpec {
        let button = |key: strings::Key<'_>, action: &'static str, params: Value| DialogButton {
            label: strings::text(key).to_owned(),
            answer: DialogAnswer::Action(action, params),
            disabled: None,
        };
        let not_now = button(key!("startup-not-now"), "startup.dismiss", Value::Null);
        let (title, primary, mut secondary) = if self.projects.is_empty() {
            (key!("startup-title-crash"), not_now, Vec::new())
        } else {
            (
                key!("startup-title-project"),
                button(
                    key!("startup-recover"),
                    "startup.recover",
                    json!({ "project": 1 }),
                ),
                vec![not_now],
            )
        };
        if self.log.is_some() {
            secondary.push(button(
                key!("startup-open-log-folder"),
                "startup.open_log_folder",
                Value::Null,
            ));
            secondary.push(button(
                key!("startup-delete-log"),
                "startup.delete_log",
                Value::Null,
            ));
        }
        DialogSpec {
            title: strings::text(title).to_owned(),
            body: self.crash_line().into_iter().collect(),
            primary,
            secondary,
            danger: None,
            asks: false,
            // Four buttons and the project rows: the form's width (640 px, growing with the window), so the Dutch
            // footer fits at the 1280 × 800 floor.
            form: true,
        }
    }
}

/// Epoch milliseconds as `yyyy-mm-dd hh:mm` (UTC: the standard library has no local time zone).
fn utc_minutes(ms: u64) -> String {
    i64::try_from(ms / 1000)
        .ok()
        .and_then(ew_api::output::date_from_epoch)
        .map_or_else(String::new, |d| {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                d.year, d.month, d.day, d.hour, d.minute
            )
        })
}

/// The time of a crash log's last entry (`Time: yyyy-mm-ddThh:mm:ssZ`, L3) as `yyyy-mm-dd hh:mm`.
fn last_entry_time(text: &str) -> Option<String> {
    let time = text.lines().rev().find_map(|l| l.strip_prefix("Time: "))?;
    let (date, clock) = time.split_once('T')?;
    Some(format!("{date} {}", clock.get(..5)?))
}

/// The crash logs of `dir` after the start-up pruning (logs older than [`KEEP_LOG_AGE`] at `now`, then all but the
/// [`KEEP_LOGS`] newest, are deleted): the newest one that ended its session and was not offered, and the older ones
/// that did too.
fn prune_logs(dir: &Path, now: SystemTime) -> (Option<OfferedLog>, Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (None, Vec::new());
    };
    let mut logs: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if !name.ends_with(".log") || !path.is_file() {
            continue;
        }
        let modified = entry.metadata().and_then(|m| m.modified()).ok();
        if modified.is_some_and(|m| now.duration_since(m).is_ok_and(|age| age > KEEP_LOG_AGE)) {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        logs.push((name, path));
    }
    // The names start with the session's start in UTC, so the newest sorts last.
    logs.sort();
    logs.reverse();
    for (_, path) in logs.iter().skip(KEEP_LOGS) {
        let _ = std::fs::remove_file(path);
    }
    logs.truncate(KEEP_LOGS);
    let mut ended = logs
        .into_iter()
        .filter(|(name, _)| !name.ends_with(OFFERED))
        .filter_map(|(_, path)| {
            let text = std::fs::read_to_string(&path).ok()?;
            text.trim_end()
                .ends_with(ENDED_LINE)
                .then(|| (path, last_entry_time(&text).unwrap_or_default()))
        });
    let newest = ended.next().map(|(path, when)| OfferedLog { path, when });
    (newest, ended.map(|(path, _)| path).collect())
}

/// Renames the log `path` to `<stem>.offered.log`.
fn mark_offered(path: &Path) {
    if let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) {
        let stem = name.strip_suffix(".log").unwrap_or(&name);
        let _ = std::fs::rename(path, path.with_file_name(format!("{stem}{OFFERED}")));
    }
}

/// `path` as the system resolves it, for comparing two names of one project.
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

impl ShellModel {
    /// The start of the window and the headless mode (module documentation): the directories made user-only, the
    /// pruning, and the offer when there is something to offer.
    pub fn start_up(&mut self) {
        if let Some(dir) = &self.config.config_dir {
            crate::settings::user_only_dir(dir);
        }
        let store = self.store_options();
        let found = store.start_up_scan();
        let (log, earlier_logs) = match store.state_dir.as_ref() {
            Some(state) => {
                let dir = state.join(CRASH_DIR);
                crate::settings::user_only_dir(&dir);
                // The store's clock, the one its journal pruning reads.
                prune_logs(&dir, store.clock.now())
            }
            None => (None, Vec::new()),
        };
        let projects: Vec<OfferedProject> = found
            .into_iter()
            .map(|p| OfferedProject {
                name: super::dir_name(&p.path),
                path: p.path,
                transactions: p.transactions,
                last_change_ms: p.last_change_ms,
            })
            .collect();
        if !projects.is_empty() || log.is_some() {
            self.startup = Some(StartupOffer {
                projects,
                log,
                refusal: None,
                earlier_logs,
            });
        }
    }

    /// The pending start-up offer.
    pub fn startup_offer(&self) -> Option<&StartupOffer> {
        self.startup.as_ref()
    }

    /// The folder the window is asked to open (`startup.open_log_folder`), as a `file://` URL, and the folder.
    pub fn take_log_folder_request(&mut self) -> Option<(String, PathBuf)> {
        self.startup_folder_request.take()
    }

    /// The window could not open the log folder: the offer says where it is (S7).
    pub fn log_folder_failed(&mut self, folder: &Path) {
        if let Some(offer) = &mut self.startup {
            offer.refusal = Some(strings::format(
                key!("startup-log-folder-failed"),
                &[&folder.to_string_lossy()],
            ));
        }
    }

    /// Closes the offer; its log and the earlier ones are renamed `<stem>.offered.log`, so none is offered again.
    fn close_startup(&mut self) {
        let Some(offer) = self.startup.take() else { return };
        for path in offer.log.iter().map(|l| &l.path).chain(&offer.earlier_logs) {
            mark_offered(path);
        }
    }

    /// A project that opened is no longer offered (`elektrawatt <dir>` recovers it at open); an offer left
    /// with neither a project nor a log closes.
    pub(super) fn opened_offered_project(&mut self, dir: &Path) {
        let Some(offer) = &mut self.startup else { return };
        let opened = canonical(dir);
        offer.projects.retain(|p| canonical(&p.path) != opened);
        if offer.projects.is_empty() && offer.log.is_none() {
            self.close_startup();
        }
    }

    /// `startup.recover {project}` (1-based): the offer closes and the project opens as Open does; a project that is no
    /// longer there is refused in the offer (S7), which stays.
    pub(crate) fn act_startup_recover(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(offer) = &mut self.startup else {
            return Outcome::refused(key!("reason-no-startup"));
        };
        let n = match p.get("project") {
            None => return failed(key!("error-missing-param"), &["project"]),
            Some(v) => v.as_u64().and_then(|n| usize::try_from(n).ok()),
        };
        let Some(project) = n
            .and_then(|n| n.checked_sub(1))
            .and_then(|i| offer.projects.get(i))
        else {
            return failed(key!("error-bad-param"), &["project"]);
        };
        let path = project.path.clone();
        if !path.is_dir() {
            let message = strings::format(key!("startup-project-missing"), &[&path.to_string_lossy()]);
            offer.refusal = Some(message.clone());
            return Outcome::Failed { message };
        }
        self.close_startup();
        self.dispatch("file.open", &json!({ "dir": path.to_string_lossy() }))
    }

    /// `startup.open_log_folder`: the window opens the folder of the crash logs; the offer stays. The headless mode
    /// opens nothing.
    pub(crate) fn act_startup_open_log_folder(&mut self) -> Outcome {
        let Some(folder) = self
            .startup
            .as_ref()
            .and_then(StartupOffer::log_folder)
            .map(Path::to_path_buf)
        else {
            return Outcome::refused(key!("reason-no-crash-log"));
        };
        if self.mode == Mode::Headless {
            return Outcome::refused(key!("reason-no-opener"));
        }
        self.startup_folder_request = Some((super::file_url_of(&folder), folder));
        Outcome::Done
    }

    /// `startup.delete_log`: the crash log is deleted; the offer closes when no project is left in it.
    pub(crate) fn act_startup_delete_log(&mut self) -> Outcome {
        let Some(offer) = &mut self.startup else {
            return Outcome::refused(key!("reason-no-startup"));
        };
        let Some(log) = offer.log.take() else {
            return Outcome::refused(key!("reason-no-crash-log"));
        };
        let _ = std::fs::remove_file(&log.path);
        for path in std::mem::take(&mut offer.earlier_logs) {
            mark_offered(&path);
        }
        offer.refusal = None;
        if offer.projects.is_empty() {
            self.startup = None;
        }
        Outcome::Done
    }

    /// `startup.dismiss` (Not now): the offer closes.
    pub(crate) fn act_startup_dismiss(&mut self) -> Outcome {
        if self.startup.is_none() {
            return Outcome::refused(key!("reason-no-startup"));
        }
        self.close_startup();
        Outcome::Done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_time_of_the_last_entry_is_read_to_the_minute() {
        let text = "ElektraWatt 0.1.0 crash log\nTime: 2026-11-04T09:15:42Z\n\nElektraWatt 0.1.0 crash log\n\
                    Time: 2026-11-04T10:07:59Z\nBacktrace:\n\nEnded: this panic ended the session\n";
        assert_eq!(last_entry_time(text).as_deref(), Some("2026-11-04 10:07"));
        assert_eq!(last_entry_time("no time"), None);
        assert_eq!(utc_minutes(1_792_222_930_000), "2026-10-17 07:42");
    }
}
