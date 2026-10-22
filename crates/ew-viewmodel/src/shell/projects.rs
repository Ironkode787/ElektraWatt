// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The project-creating and project-recovering actions of the shell (WP-S2.01 part 2): New project (FT-307), Open
//! sample project and Tutorials (FT-312), Import QElectroTech project (FT-312), and discarding the journal of an
//! earlier session (FT-313's first part, decision log ID-475).
//!
//! All of them end in a `Session` from `ew-api` that the shell holds like an opened project: a new project is
//! `Session::new_project` followed by `Session::create` into the directory the user chose, an import is
//! `ew_api::import::import_qet` followed by `Session::create`, the discard is an open with `Recovery::Discard`.

use std::path::{Path, PathBuf};

use ew_api::import::{Severity, StarterLibrary, import_qet};
use ew_api::{NewProjectKind, ProjectTemplate, Recovery, Session};
use serde_json::{Map, Value};

use super::{
    AccessMode, Dialog, Mode, Outcome, Project, Prompt, Record, ShellModel, dir_name, failed, flag,
    open_error_key, string_param,
};
use crate::strings;

impl ShellModel {
    /// The template of a new project: the `template` parameter (`builtin` or a directory), else the default basic
    /// project of the settings when it still is a directory, else the built-in one.
    fn template_dir(&self, p: &Map<String, Value>) -> Result<Option<PathBuf>, Outcome> {
        match string_param(p, "template")? {
            Some("builtin") => Ok(None),
            Some(dir) => Ok(Some(PathBuf::from(dir))),
            None => Ok(self
                .settings
                .basic_project
                .as_deref()
                .map(PathBuf::from)
                .filter(|d| d.is_dir())),
        }
    }

    /// The name of the template a new project would start from, for the start page: the default basic project's
    /// directory name, or `None` for the built-in one.
    pub fn default_template_name(&self) -> Option<String> {
        self.settings
            .basic_project
            .as_deref()
            .map(Path::new)
            .filter(|d| d.is_dir())
            .map(dir_name)
    }

    /// Whether the sample project is installed.
    pub fn has_sample(&self) -> bool {
        self.config
            .resources
            .sample_dir
            .as_deref()
            .is_some_and(Path::is_dir)
    }

    /// Makes `session` the open project: it was just written to `dir` (`Session::create`), so it holds the lock and
    /// is editable.
    fn install_new(&mut self, session: Session, dir: &Path, banner: Option<String>) {
        let labels = session.page_labels();
        self.record = Record {
            dir: Some(dir.to_path_buf()),
            ..Record::default()
        };
        self.project = Some(Project {
            session,
            dir: dir.to_path_buf(),
            name: dir_name(dir),
            access: AccessMode::Edit,
            labels,
        });
        self.page_index = 0;
        self.settings.push_recent(&dir.to_string_lossy());
        self.save_settings();
        self.banner = banner;
    }

    /// Writes `session` into `dir` and makes it the open project; the message of a failure carries the reason, never
    /// a path.
    fn create_and_install(
        &mut self,
        mut session: Session,
        dir: &Path,
        failure_key: &str,
        banner: Option<String>,
    ) -> Outcome {
        match session.create(dir, self.store_options()) {
            Ok(()) => {
                self.install_new(session, dir, banner);
                Outcome::Done
            }
            Err(e) => {
                let message = strings::format(failure_key, &[strings::text(open_error_key(&e))]);
                self.banner = Some(message.clone());
                Outcome::Failed { message }
            }
        }
    }

    pub(super) fn act_new(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.has_project()
            && let Some(outcome) = self.guard_unsaved("file.new", p)
        {
            return outcome;
        }
        let template = match self.template_dir(p) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let dir = match string_param(p, "dir") {
            Err(e) => return e,
            Ok(Some(dir)) => PathBuf::from(dir),
            Ok(None) => return self.need_path("file.new", Prompt::NewFolder, "dir", p),
        };
        self.close_project();
        let from = match &template {
            Some(d) => ProjectTemplate::Directory(d),
            None => ProjectTemplate::Builtin,
        };
        match Session::new_project(from, NewProjectKind::Schematic, ShellModel::session_options()) {
            Ok(session) => self.create_and_install(session, &dir, "error-new", None),
            Err(e) => {
                let message = strings::format("error-new", &[strings::text(open_error_key(&e))]);
                self.banner = Some(message.clone());
                Outcome::Failed { message }
            }
        }
    }

    pub(super) fn act_open_sample(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(sample) = self.config.resources.sample_dir.clone().filter(|d| d.is_dir()) else {
            return failed("error-no-sample", &[]);
        };
        if self.has_project()
            && let Some(outcome) = self.guard_unsaved("file.open_sample", p)
        {
            return outcome;
        }
        let outcome = self.open_dir(&sample, true);
        if outcome == Outcome::Done {
            // The sample lives in the installation: it is not a recent project, and a copy is made with Save as.
            self.settings.remove_recent(&sample.to_string_lossy());
            self.save_settings();
            if let Some(project) = &mut self.project {
                project.access = AccessMode::ReadOnly {
                    reason_key: "readonly-sample",
                };
            }
            self.banner = Some(strings::text("banner-sample").to_owned());
        }
        outcome
    }

    pub(super) fn act_tutorials(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(sample) = self.config.resources.sample_dir.clone().filter(|d| d.is_dir()) else {
            // Without the sample the tutorials page still opens.
            self.request_doc("tutorials");
            return Outcome::Done;
        };
        if self.has_project()
            && let Some(outcome) = self.guard_unsaved("help.tutorials", p)
        {
            return outcome;
        }
        let dir = match string_param(p, "dir") {
            Err(e) => return e,
            Ok(Some(dir)) => PathBuf::from(dir),
            Ok(None) => return self.need_path("help.tutorials", Prompt::NewFolder, "dir", p),
        };
        self.close_project();
        // A fresh copy of the sample project: its records under new identities, in a directory of the user's.
        let session = match Session::new_project(
            ProjectTemplate::Directory(&sample),
            NewProjectKind::Schematic,
            ShellModel::session_options(),
        ) {
            Ok(s) => s,
            Err(e) => {
                let message = strings::format("error-new", &[strings::text(open_error_key(&e))]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        let outcome = self.create_and_install(
            session,
            &dir,
            "error-new",
            Some(strings::text("banner-tutorial-copy").to_owned()),
        );
        if outcome == Outcome::Done {
            self.request_doc("tutorials");
        }
        outcome
    }

    pub(super) fn act_import_qet(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.has_project()
            && let Some(outcome) = self.guard_unsaved("file.import_qet", p)
        {
            return outcome;
        }
        let file = match string_param(p, "file") {
            Err(e) => return e,
            Ok(Some(file)) => PathBuf::from(file),
            Ok(None) => return self.need_path("file.import_qet", Prompt::OpenQet, "file", p),
        };
        let dir = match string_param(p, "dir") {
            Err(e) => return e,
            Ok(Some(dir)) => PathBuf::from(dir),
            Ok(None) => return self.need_path("file.import_qet", Prompt::NewFolder, "dir", p),
        };
        let name = file
            .file_name()
            .map_or_else(|| "project.qet".to_owned(), |n| n.to_string_lossy().into_owned());
        let Ok(bytes) = std::fs::read(&file) else {
            return failed("error-import-read", &[&name]);
        };
        let (library, library_note) = match self.config.resources.content_dir.as_deref() {
            Some(content) => match StarterLibrary::from_dir(content) {
                Ok(l) => (l, None),
                Err(_) => (
                    StarterLibrary::empty(),
                    Some(strings::text("banner-import-no-content")),
                ),
            },
            None => (
                StarterLibrary::empty(),
                Some(strings::text("banner-import-no-content")),
            ),
        };
        let imported = match import_qet(&name, bytes, &library) {
            Ok(i) => i,
            Err(e) => {
                let message = strings::format("error-import", &[&name, &e.to_string()]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        let counts = imported.counts();
        let findings = imported
            .batch
            .report
            .entries
            .iter()
            .filter(|e| e.severity != Severity::Info)
            .count();
        let session = match imported.session {
            Ok(s) => s,
            Err(reason) => {
                let message = strings::format("error-import", &[&name, &reason]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        self.close_project();
        let mut banner = strings::format(
            "banner-imported",
            &[
                &name,
                &counts.pages.to_string(),
                &counts.devices.to_string(),
                &findings.to_string(),
            ],
        );
        if let Some(note) = library_note {
            banner.push(' ');
            banner.push_str(note);
        }
        self.create_and_install(session, &dir, "error-import-write", Some(banner))
    }

    pub(super) fn act_discard_journal(&mut self, p: &Map<String, Value>) -> Outcome {
        if !flag(p, "confirm") {
            return match self.mode {
                Mode::Headless => failed("error-confirm-needed", &[]),
                Mode::Gui => self.open_dialog(Dialog::ConfirmJournalDiscard),
            };
        }
        let Some(dir) = self.project.as_ref().map(|p| p.dir.clone()) else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        // The read-only session goes (it holds no lock and owns no journal), then the project opens for editing and
        // deletes the leftover journal on the way (doc 05 section 8.3: "Open without recovering" is the other path).
        self.close_project();
        let name = dir_name(&dir);
        let opened = self
            .open_for_editing_with(&dir, Recovery::Discard)
            .and_then(|(session, mut report)| {
                // The report still names the journal that the open just deleted.
                report.recovery = None;
                self.settle_open(&dir, session, report, false)
            });
        match opened {
            Ok(super::Opened {
                session,
                access,
                banner,
                ..
            }) => {
                let labels = session.page_labels();
                self.record = Record {
                    dir: Some(dir.clone()),
                    ..Record::default()
                };
                self.project = Some(Project {
                    session,
                    dir,
                    name,
                    access,
                    labels,
                });
                self.page_index = 0;
                self.banner = Some(match banner {
                    Some(b) => b,
                    None => strings::text("banner-journal-discarded").to_owned(),
                });
                Outcome::Done
            }
            Err(e) => {
                let message = strings::format("error-open", &[&name, strings::text(open_error_key(&e))]);
                self.banner = Some(message.clone());
                Outcome::Failed { message }
            }
        }
    }
}
