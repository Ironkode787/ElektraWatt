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

use ew_api::import::{
    Detected, Refusal, ReimportClass, ReimportError, Severity, StarterLibrary, import_qet, migration_counts,
};
use ew_api::{NewProjectKind, ProjectTemplate, Recovery, Session};
use serde_json::{Map, Value};

use super::{
    AccessMode, Dialog, Mode, Outcome, Project, Prompt, Record, ShellModel, dir_name, failed, flag,
    open_error_key, string_param,
};
use crate::key;
use crate::strings;
use crate::strings::Key;

/// An imported project the import card shows, not written yet (WP-S3.09 D8).
#[derive(Debug)]
pub(crate) struct PendingImport {
    /// The session of the committed import.
    session: Session,
    /// The folder the user chose.
    dir: PathBuf,
    /// The banner of the opened project.
    banner: String,
}

/// The banner text of a refused import (T1, T2 through their keys; an importer's refusal in its English words).
pub(crate) fn refusal_text(file: &str, refusal: &Refusal) -> String {
    match refusal {
        Refusal::Panicked { .. } => strings::format(key!("error-import-panicked"), &[file]),
        Refusal::Contract { breach, .. } => {
            let reason = match breach {
                ew_api::import::ContractBreach::SharedIdentity => key!("reason-import-shared-identity"),
                ew_api::import::ContractBreach::IdentityNotFromKey => key!("reason-import-identity-key"),
                ew_api::import::ContractBreach::OtherLineage => key!("reason-import-other-lineage"),
                ew_api::import::ContractBreach::OutOfOrder => key!("reason-import-order"),
            };
            strings::format(key!("error-import-refused"), &[file, strings::text(reason)])
        }
        Refusal::Import(e) => strings::format(key!("error-import"), &[file, &e.to_string()]),
    }
}

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

    /// A new project goes into an empty or absent directory (doc 16: "a new, empty directory"): `Some` with the
    /// failed outcome (and its banner) when `dir` holds anything. Checked before anything is closed or written.
    pub(super) fn require_free_dir(&mut self, dir: &Path) -> Option<Outcome> {
        let free = match std::fs::read_dir(dir) {
            Ok(mut entries) => entries.next().is_none(),
            Err(e) => e.kind() == std::io::ErrorKind::NotFound,
        };
        if free {
            return None;
        }
        let message = strings::text(key!("error-dir-not-empty")).to_owned();
        self.banner = Some(message.clone());
        Some(Outcome::Failed { message })
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
        self.epochs += 1;
        self.project = Some(Project {
            session,
            epoch: self.epochs,
            dir: dir.to_path_buf(),
            name: dir_name(dir),
            access: AccessMode::Edit,
            labels,
            rows: Vec::new(),
        });
        self.install_view();
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
        failure_key: Key<'_>,
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

    pub(crate) fn act_new(&mut self, p: &Map<String, Value>) -> Outcome {
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
        if let Some(outcome) = self.require_free_dir(&dir) {
            return outcome;
        }
        self.close_project();
        let from = match &template {
            Some(d) => ProjectTemplate::Directory(d),
            None => ProjectTemplate::Builtin,
        };
        match Session::new_project(from, NewProjectKind::Schematic, self.session_options()) {
            Ok(session) => self.create_and_install(session, &dir, key!("error-new"), None),
            Err(e) => {
                let message = strings::format(key!("error-new"), &[strings::text(open_error_key(&e))]);
                self.banner = Some(message.clone());
                Outcome::Failed { message }
            }
        }
    }

    pub(crate) fn act_open_sample(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(sample) = self.config.resources.sample_dir.clone().filter(|d| d.is_dir()) else {
            return failed(key!("error-no-sample"), &[]);
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
                    reason_key: key!("readonly-sample"),
                };
            }
            self.banner = Some(strings::text(key!("banner-sample")).to_owned());
        }
        outcome
    }

    pub(crate) fn act_tutorials(&mut self, p: &Map<String, Value>) -> Outcome {
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
        if let Some(outcome) = self.require_free_dir(&dir) {
            return outcome;
        }
        self.close_project();
        // A fresh copy of the sample project: its records under new identities, in a directory of the user's.
        let session = match Session::new_project(
            ProjectTemplate::Directory(&sample),
            NewProjectKind::Schematic,
            self.session_options(),
        ) {
            Ok(s) => s,
            Err(e) => {
                let message = strings::format(key!("error-new"), &[strings::text(open_error_key(&e))]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        let outcome = self.create_and_install(
            session,
            &dir,
            key!("error-new"),
            Some(strings::text(key!("banner-tutorial-copy")).to_owned()),
        );
        if outcome == Outcome::Done {
            self.request_doc("tutorials");
        }
        outcome
    }

    pub(crate) fn act_import_qet(&mut self, p: &Map<String, Value>) -> Outcome {
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
        if let Some(outcome) = self.require_free_dir(&dir) {
            return outcome;
        }
        let name = file
            .file_name()
            .map_or_else(|| "project.qet".to_owned(), |n| n.to_string_lossy().into_owned());
        let Ok(bytes) = std::fs::read(&file) else {
            return failed(key!("error-import-read"), &[&name]);
        };
        let (library, library_note) = self.import_library();
        let imported = match import_qet(&name, bytes, &library) {
            Ok(i) => i,
            Err(e) => {
                let message = refusal_text(&name, &e);
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
                let message = strings::format(key!("error-import"), &[&name, &reason]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        let mut banner = strings::format(
            key!("banner-imported"),
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
        // WP-S3.09 D8: the import card first (its summary and pages); Apply asks the checkpoint confirmation, then
        // writes and opens the project. Cancel writes nothing.
        let (errors, warnings, infos) = migration_counts(&session);
        let summary = strings::format(
            key!("import-card-summary"),
            &[
                &counts.pages.to_string(),
                &counts.devices.to_string(),
                &counts.connections.to_string(),
                &errors.to_string(),
                &warnings.to_string(),
                &infos.to_string(),
            ],
        );
        let pages = session.page_labels();
        self.palette = None;
        self.dialog_gen += 1;
        self.dialog = Some(Dialog::ImportQet {
            file: name.clone(),
            folder: dir_name(&dir),
            summary,
            pages,
            devices: counts.devices,
            confirm: false,
        });
        self.import_pending = Some(PendingImport { session, dir, banner });
        Outcome::Done
    }

    /// `file.import_qet_apply {confirm?}` (the import card's Apply, WP-S3.09 D8): the checkpoint confirmation T4 (in the
    /// window a question, headless the `confirm` parameter), then the project written into the chosen folder and
    /// opened. Without an import card it is refused.
    pub(crate) fn act_import_qet_apply(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(Dialog::ImportQet { file, confirm, .. }) = self.dialog.as_mut() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-import"),
            };
        };
        if !flag(p, "confirm") {
            return match self.mode {
                Mode::Headless => failed(key!("error-import-confirm"), &[file]),
                Mode::Gui => {
                    *confirm = true;
                    self.dialog_gen += 1;
                    Outcome::Done
                }
            };
        }
        self.dialog = None;
        let Some(pending) = self.import_pending.take() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-import"),
            };
        };
        self.close_project();
        self.create_and_install(
            pending.session,
            &pending.dir,
            key!("error-import-write"),
            Some(pending.banner),
        )
    }

    /// The starter library an import resolves its built-in keys in, and the banner's note when there is none.
    fn import_library(&self) -> (StarterLibrary, Option<&'static str>) {
        match self.config.resources.content_dir.as_deref() {
            Some(content) => match StarterLibrary::from_dir(content) {
                Ok(l) => (l, None),
                Err(_) => (
                    StarterLibrary::empty(),
                    Some(strings::text(key!("banner-import-no-content"))),
                ),
            },
            None => (
                StarterLibrary::empty(),
                Some(strings::text(key!("banner-import-no-content"))),
            ),
        }
    }

    /// `file.reimport_qet {file?, accept?}` (WP-S3.09 D10 to D12): the QElectroTech file re-imported into its earlier
    /// import in the open project. A file matched by its items asks T6 first (`accept: true` answers it); a file of
    /// another project is refused with T7; else the re-import card shows T8 and its rows. Nothing changes until Apply.
    pub(crate) fn act_reimport_qet(&mut self, p: &Map<String, Value>) -> Outcome {
        let file = match string_param(p, "file") {
            Err(e) => return e,
            Ok(Some(file)) => PathBuf::from(file),
            Ok(None) => return self.need_path("file.reimport_qet", Prompt::OpenQet, "file", p),
        };
        if !self.is_writable() {
            return Outcome::Refused {
                reason_key: key!("reason-read-only"),
            };
        }
        let name = file
            .file_name()
            .map_or_else(|| "project.qet".to_owned(), |n| n.to_string_lossy().into_owned());
        let Ok(bytes) = std::fs::read(&file) else {
            return failed(key!("error-import-read"), &[&name]);
        };
        let (library, _) = self.import_library();
        let accept = flag(p, "accept");
        let Some(project) = self.project.as_mut() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        let refused = |e: &ReimportError| match e {
            ReimportError::Refused(r) => refusal_text(&name, r),
            ReimportError::NotAnImport { .. } => strings::format(key!("error-reimport-foreign"), &[&name]),
            ReimportError::Apply(e) => strings::format(key!("error-import"), &[&name, &e.to_string()]),
        };
        let detected = match project.session.detect_qet_lineage(&name, bytes.clone(), &library) {
            Ok(d) => d,
            Err(e) => {
                let message = refused(&e);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        if let Detected::Proposed {
            source_name, percent, ..
        } = &detected
            && !accept
        {
            self.palette = None;
            return self.open_dialog(Dialog::ReimportMatch {
                path: file.to_string_lossy().into_owned(),
                file: name,
                source: source_name.clone(),
                percent: *percent,
            });
        }
        let reimport = match project
            .session
            .reimport_qet(&name, bytes, &library, detected.lineage())
        {
            Ok(r) => r,
            Err(e) => {
                let message = refused(&e);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        let n = |c: ReimportClass| reimport.plan.count(c).to_string();
        let summary = strings::format(
            key!("reimport-card-summary"),
            &[
                &name,
                &n(ReimportClass::Added),
                &n(ReimportClass::SourceChanged),
                &n(ReimportClass::EwChanged),
                &n(ReimportClass::Conflict),
                &n(ReimportClass::Removed),
                &n(ReimportClass::Unchanged),
            ],
        );
        let value = |v: Option<&Value>| match v {
            None => "-".to_owned(),
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
        };
        let mut rows = Vec::new();
        for row in &reimport.plan.rows {
            let key = match row.class {
                ReimportClass::Added => key!("reimport-row-added"),
                ReimportClass::SourceChanged => key!("reimport-row-source"),
                ReimportClass::Conflict => key!("reimport-row-both"),
                ReimportClass::Removed => key!("reimport-row-removed"),
                ReimportClass::EwChanged | ReimportClass::Unchanged => continue,
            };
            rows.push(strings::format(key, &[&row.record]));
            for m in &row.members {
                rows.push(strings::format(
                    key!("reimport-row-member"),
                    &[
                        &strings::field_label(row.key.0.name(), &m.path),
                        &value(m.ours.as_ref()),
                        &name,
                        &value(m.theirs.as_ref()),
                    ],
                ));
            }
        }
        self.palette = None;
        self.reimport_pending = Some(reimport);
        self.open_dialog(Dialog::Reimport {
            file: name,
            summary,
            rows,
        })
    }

    /// `file.reimport_qet_apply` (the re-import card's Apply, WP-S3.09 D12): the previewed re-import committed as one
    /// undo step (nothing when it changes nothing). Without a re-import card it is refused.
    pub(crate) fn act_reimport_qet_apply(&mut self) -> Outcome {
        let Some(Dialog::Reimport { file, .. }) = self.dialog.take() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-reimport"),
            };
        };
        let (Some(pending), Some(project)) = (self.reimport_pending.take(), self.project.as_mut()) else {
            return Outcome::Refused {
                reason_key: key!("reason-no-reimport"),
            };
        };
        let outcome = match project.session.apply_reimport(pending) {
            Ok(_) => {
                self.banner = Some(strings::format(key!("banner-reimported"), &[&file]));
                Outcome::Done
            }
            Err(e) => {
                let message = strings::format(key!("error-import"), &[&file, &e.to_string()]);
                self.banner = Some(message.clone());
                Outcome::Failed { message }
            }
        };
        self.refresh();
        outcome
    }

    pub(crate) fn act_discard_journal(&mut self, p: &Map<String, Value>) -> Outcome {
        if !flag(p, "confirm") {
            return match self.mode {
                Mode::Headless => failed(key!("error-confirm-needed"), &[]),
                Mode::Gui => {
                    // WP-S2.09 §6.6: Discard… of the recovery dialog; Cancel returns to it.
                    if matches!(self.dialog, Some(Dialog::Recovery { .. })) {
                        self.recovery_stash = self.dialog.take();
                    }
                    self.open_dialog(Dialog::ConfirmJournalDiscard)
                }
            };
        }
        // The confirmed discard answers the recovery dialog too.
        if matches!(self.dialog, Some(Dialog::Recovery { .. })) {
            self.dialog = None;
        }
        self.recovery_stash = None;
        let Some(dir) = self.project.as_ref().map(|p| p.dir.clone()) else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
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
                self.epochs += 1;
                self.project = Some(Project {
                    session,
                    epoch: self.epochs,
                    dir,
                    name,
                    access,
                    labels,
                    rows: Vec::new(),
                });
                self.install_view();
                self.banner = Some(match banner {
                    Some(b) => b,
                    None => strings::text(key!("banner-journal-discarded")).to_owned(),
                });
                Outcome::Done
            }
            Err(e) => {
                let message =
                    strings::format(key!("error-open"), &[&name, strings::text(open_error_key(&e))]);
                self.banner = Some(message.clone());
                Outcome::Failed { message }
            }
        }
    }
}
