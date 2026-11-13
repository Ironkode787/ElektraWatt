// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The project-creating and project-recovering actions of the shell: New project, Open
//! sample project and Tutorials, Import QElectroTech project, and discarding the journal of an
//! earlier session.
//!
//! All of them end in a `Session` from `ew-api` that the shell holds like an opened project: a new project is
//! `Session::new_project` followed by `Session::create` into the directory the user chose, an import is
//! `ew_api::import::import_qet` followed by `Session::create`, the discard is an open with `Recovery::Discard`.

use std::path::{Path, PathBuf};

use ew_api::ew_jobs::{Job, JobEnd, JobOptions};
use ew_api::import::{
    Detected, ImportBatch, LineageInfo, QetLineages, Refusal, ReimportClass, ReimportError, StarterLibrary,
    detect_qet_reimport, import_qet_batch, imported_of, migration_counts, reimport_qet_batch,
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

/// The import and re-import cards of the shell and their jobs.
#[derive(Default)]
pub(super) struct ImportState {
    /// The imported project the import card shows.
    pub(super) pending: Option<PendingImport>,
    /// The import job running for the import card.
    pub(super) job: Option<ImportJob>,
    /// The previewed re-import the re-import card shows.
    pub(super) reimport_pending: Option<ew_api::import::Reimport>,
    /// The re-import job running for the re-import card.
    pub(super) reimport_job: Option<ReimportJob>,
    /// The project (its epoch), file and lineage of the re-import's proposal (T6) shown last: its Re-import imports
    /// into that lineage, in that project only.
    pub(super) reimport_proposal: Option<(u64, PathBuf, ew_api::import::LineageInfo)>,
    /// The re-import's proposal (T6) or card that waits behind a dialog open when its job ended.
    pub(super) reimport_waiting: Option<Dialog>,
}

/// An imported project the import card shows, not written yet.
#[derive(Debug)]
pub(crate) struct PendingImport {
    /// The session of the committed import.
    session: Session,
    /// The folder the user chose.
    dir: PathBuf,
    /// The banner of the opened project.
    banner: String,
    /// The import card while another dialog is open: it opens when that dialog closes.
    waiting: Option<Dialog>,
}

/// A running import job of the window: the importer on its own thread, with what its end needs.
#[derive(Debug)]
pub(crate) struct ImportJob {
    /// The job: the batch, or why the front door refused the file.
    pub(super) job: Job<Result<ew_api::import::ImportBatch, Refusal>>,
    /// The source's file name.
    pub(super) name: String,
    /// The folder the user chose.
    pub(super) dir: PathBuf,
    /// The banner's note when there is no starter content.
    pub(super) library_note: Option<&'static str>,
}

/// A running re-import job of the window: the importer on its own thread, with what its end needs.
#[derive(Debug)]
pub(crate) struct ReimportJob {
    /// The job: what it found, or why the re-import was refused.
    pub(super) job: Job<Result<ReimportFound, ReimportError>>,
    /// The source's file name.
    pub(super) name: String,
    /// The file's path, as the proposal's Re-import names it.
    pub(super) path: PathBuf,
    /// The epoch of the project the job reads.
    pub(super) epoch: u64,
}

/// What a re-import job found (D10): a proposed lineage still to be accepted (T6), or the batch imported into the
/// lineage it re-imports into.
#[derive(Debug)]
pub(crate) enum ReimportFound {
    /// A lineage matched by its items, not accepted yet.
    Proposed {
        /// The lineage.
        lineage: LineageInfo,
        /// The source name of its earlier import.
        source: String,
        /// The share of the file's source keys it holds, in per cent.
        percent: u32,
    },
    /// The file imported into the lineage.
    Batch {
        /// The lineage.
        lineage: LineageInfo,
        /// The batch.
        batch: Box<ImportBatch>,
    },
}

/// The body of a re-import job: the importer once, into the accepted proposal's lineage (`proposed`), else with the
/// file's own lineage, which is the batch when the project holds that lineage. Only a file of a proposed lineage that
/// `accept` takes without an earlier proposal is imported a second time, into that lineage.
fn find_reimport(
    lineages: &QetLineages,
    name: &str,
    bytes: Vec<u8>,
    library: &StarterLibrary,
    proposed: Option<LineageInfo>,
    accept: bool,
) -> Result<ReimportFound, ReimportError> {
    let lineage = match proposed {
        Some(l) => l,
        None => match detect_qet_reimport(lineages, name, bytes.clone(), library)? {
            (Detected::Lineage(lineage), Some(batch)) => {
                return Ok(ReimportFound::Batch {
                    lineage,
                    batch: Box::new(batch),
                });
            }
            (
                Detected::Proposed {
                    lineage,
                    source_name,
                    percent,
                },
                _,
            ) if !accept => {
                return Ok(ReimportFound::Proposed {
                    lineage,
                    source: source_name,
                    percent,
                });
            }
            (detected, _) => *detected.lineage(),
        },
    };
    let batch = reimport_qet_batch(lineages, name, bytes, library, &lineage)?;
    Ok(ReimportFound::Batch {
        lineage,
        batch: Box::new(batch),
    })
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
            sections: Vec::new(),
        });
        self.install_view();
        self.settings.push_recent(&dir.to_string_lossy());
        self.save_settings();
        self.banner = banner;
        // A template's image over the pixel limit is named as an open names it.
        for extra in self.undrawn_image_notes() {
            self.banner = Some(match self.banner.take() {
                Some(b) => format!("{b} {extra}"),
                None => extra,
            });
        }
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
                let unstable = session.store().is_some_and(|s| s.writes_unstable());
                self.install_new(session, dir, banner);
                self.warn_unreleased(unstable);
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
        let template = or_return!(self.template_dir(p));
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

    /// `help.tutorials {tutorial?, dir?}` (Help ▸ Tutorials): each tutorial its own start with its
    /// page: `first-page` a new project from the basic project, `reports-and-pdf` (and no `tutorial`, the tutorials
    /// page) a copy of the sample project, the others their page alone (they start from the first tutorial's project,
    /// or a QElectroTech file).
    pub(crate) fn act_tutorials(&mut self, p: &Map<String, Value>) -> Outcome {
        let tutorial = match string_param(p, "tutorial") {
            Err(e) => return e,
            Ok(Some(t)) if !crate::actions::tutorials().any(|(name, _)| name == t) => {
                return failed(key!("error-bad-param"), &["tutorial"]);
            }
            Ok(t) => t.map(str::to_owned),
        };
        let page = tutorial.as_deref().map(|name| format!("tutorials/{name}.html"));
        match tutorial.as_deref() {
            Some("first-page") => return self.tutorial_from_basic(p, page.unwrap_or_default()),
            None | Some("reports-and-pdf") => {}
            Some(_) => {
                self.request_doc_target(page.unwrap_or_default());
                return Outcome::Done;
            }
        }
        let open_page = |s: &mut ShellModel| match &page {
            Some(page) => s.request_doc_target(page.clone()),
            None => s.request_doc("tutorials"),
        };
        let Some(sample) = self.config.resources.sample_dir.clone().filter(|d| d.is_dir()) else {
            // Without the sample the tutorials page still opens.
            open_page(self);
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
            open_page(self);
            self.note_missing_guide();
        }
        outcome
    }

    /// The first tutorial's start, a new project from the basic project in the folder `dir` (asked for when
    /// absent), then its page `page`.
    fn tutorial_from_basic(&mut self, p: &Map<String, Value>, page: String) -> Outcome {
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
        let outcome = match Session::new_project(
            ProjectTemplate::Builtin,
            NewProjectKind::Schematic,
            self.session_options(),
        ) {
            Ok(session) => self.create_and_install(session, &dir, key!("error-new"), None),
            Err(e) => {
                let message = strings::format(key!("error-new"), &[strings::text(open_error_key(&e))]);
                self.banner = Some(message.clone());
                Outcome::Failed { message }
            }
        };
        if outcome == Outcome::Done {
            self.request_doc_target(page);
            self.note_missing_guide();
        }
        outcome
    }

    /// Without an installed guide a tutorial's project opens with no banner over it, and the status
    /// bar says once that the guide is not installed.
    fn note_missing_guide(&mut self) {
        if self.doc_request.as_ref().is_some_and(|r| r.url.is_none()) {
            self.doc_request = None;
            let epoch = self.project.as_ref().map(|p| p.epoch);
            self.status_note = Some((
                strings::text(key!("status-guide-missing")).to_owned(),
                epoch,
                self.revision(),
            ));
        }
    }

    /// `file.import {file?}` (File ▸ Import…): one file dialog for a QElectroTech project (`.qet`), whose import it
    /// starts (`file.import_qet`), and a project archive (`.ewz`), whose Open .ewz dialog it opens (`file.open_ewz`);
    /// any other file is refused in words.
    pub(crate) fn act_import(&mut self, p: &Map<String, Value>) -> Outcome {
        let file = match string_param(p, "file") {
            Err(e) => return e,
            Ok(Some(file)) => file.to_owned(),
            Ok(None) => return self.need_path("file.import", Prompt::OpenImport, "file", p),
        };
        let extension = std::path::Path::new(&file)
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());
        let mut params = p.clone();
        match extension.as_deref() {
            Some("qet") => self.act_import_qet(&params),
            Some("ewz") => {
                params.remove("file");
                params.insert("path".to_owned(), Value::String(file));
                self.act_open_ewz(&params)
            }
            Some("dxf" | "dwg" | "pdf") => self.act_import_drawing(&params),
            _ => Outcome::refused(key!("reason-import-kind")),
        }
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
        // The importer runs as a job (its own thread and `catch_unwind`), so the window stays responsive;
        // the import's transaction follows on this thread, since a session is not `Send`.
        let options = JobOptions {
            name: "import".to_owned(),
            waker: self.jobs.waker.clone(),
            gate: self.jobs.hooks.gate(),
        };
        let job_name = name.clone();
        let hooks = self.jobs.hooks.clone();
        let job = Job::spawn(options, move |_| {
            // A test fault (feature `testkit` only): the job's body panics, as an importer's panic would end it.
            hooks.fault("import");
            Ok(import_qet_batch(&job_name, bytes, &library))
        });
        let Ok(job) = job else {
            return failed(key!("error-import-read"), &[&name]);
        };
        let pending = ImportJob {
            job,
            name,
            dir,
            library_note,
        };
        // A script (headless or driving the window) waits for the job, so its next step finds the card.
        if self.mode == Mode::Headless || !self.is_person() {
            let ImportJob {
                job,
                name,
                dir,
                library_note,
            } = pending;
            return self.finish_import(job.wait(), name, dir, library_note);
        }
        self.banner = Some(strings::format(key!("banner-importing"), &[&pending.name]));
        self.import.job = Some(pending);
        Outcome::Done
    }

    /// Takes the end of a running import job (the window every frame, through `poll_jobs`) and goes on with the import
    /// card.
    pub(super) fn poll_import(&mut self) {
        if self.dialog.is_none()
            && let Some(card) = self.import.pending.as_mut().and_then(|p| p.waiting.take())
        {
            let _ = self.open_dialog(card);
        }
        // And the re-import's proposal or card.
        if self.dialog.is_none()
            && let Some(dialog) = self.import.reimport_waiting.take()
        {
            let _ = self.open_dialog(dialog);
        }
        let ended = self.import.job.as_mut().and_then(|pending| pending.job.try_end());
        if let Some(end) = ended
            && let Some(pending) = self.import.job.take()
        {
            let _ = self.finish_import(end, pending.name, pending.dir, pending.library_note);
        }
        // And the re-import's.
        let ended = self
            .import
            .reimport_job
            .as_mut()
            .and_then(|pending| pending.job.try_end());
        if let Some(end) = ended
            && let Some(pending) = self.import.reimport_job.take()
        {
            let _ = self.finish_reimport(end, pending.name, pending.path, pending.epoch);
        }
    }

    /// The end of an import job: a refusal (T1 for a panic, T2, the importer's words) in the banner, else the batch
    /// committed into a new project by `import.apply` and shown on the import card.
    pub(super) fn finish_import(
        &mut self,
        end: JobEnd<Result<ew_api::import::ImportBatch, Refusal>>,
        name: String,
        dir: PathBuf,
        library_note: Option<&'static str>,
    ) -> Outcome {
        let batch = match end {
            JobEnd::Done(Ok(batch)) => Ok(batch),
            JobEnd::Done(Err(refusal)) => Err(refusal),
            JobEnd::Panicked | JobEnd::Failed(_) => Err(Refusal::Panicked { file: name.clone() }),
            // A cancelled job is no internal error.
            JobEnd::Cancelled => {
                let message = strings::format(key!("error-import-cancelled"), &[&name]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        let imported = match batch {
            Ok(batch) => imported_of(batch, ew_api::SessionOptions::default()),
            Err(e) => {
                let message = refusal_text(&name, &e);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        let counts = imported.counts();
        let session = match imported.session {
            Ok(s) => s,
            Err(reason) => {
                let message = strings::format(key!("error-import"), &[&name, &reason]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        // The import card first (its summary and pages); Apply asks the checkpoint confirmation, then
        // writes and opens the project. Cancel writes nothing.
        let (errors, warnings, infos) = migration_counts(&session);
        // The banner counts the migration report's findings, as the card does, each count
        // with its plural form; it replaces the "Importing…" banner.
        let mut banner = strings::format(
            key!("banner-imported"),
            &[
                &name,
                &strings::format_count(crate::count_key!("import-card-pages"), counts.pages),
                &strings::format_count(crate::count_key!("import-card-devices"), counts.devices),
                &strings::format_count(
                    crate::count_key!("import-card-findings"),
                    errors + warnings + infos,
                ),
            ],
        );
        if let Some(note) = library_note {
            banner.push(' ');
            banner.push_str(note);
        }
        let importing = strings::format(key!("banner-importing"), &[&name]);
        if self.banner.as_deref() == Some(importing.as_str()) {
            self.banner = None;
        }
        // What Create project makes: the folder and each count with its plural form ("1 page").
        let summary = strings::format(
            key!("import-card-does"),
            &[
                &dir_name(&dir),
                &strings::format_count(crate::count_key!("import-card-pages"), counts.pages),
                &strings::format_count(crate::count_key!("import-card-devices"), counts.devices),
                &strings::format_count(crate::count_key!("import-card-connections"), counts.connections),
            ],
        );
        // The verdict: what the findings as a whole ask of the reader.
        let points = |n: usize| strings::format_count(crate::count_key!("import-card-points"), n);
        let verdict = if errors > 0 {
            strings::format(key!("import-card-verdict-errors"), &[&points(errors)])
        } else if warnings > 0 {
            strings::format(key!("import-card-verdict-warnings"), &[&points(warnings)])
        } else {
            strings::text(key!("import-card-verdict-clean")).to_owned()
        };
        // The kinds of findings under what they ask: one row per rule, its count and its title in the window's
        // language, the most frequent first.
        let kinds = ew_api::import::migration_kinds(&session);
        let groups = [
            (ew_api::checks::Severity::Error, key!("import-card-group-errors")),
            (
                ew_api::checks::Severity::Warning,
                key!("import-card-group-warnings"),
            ),
            (ew_api::checks::Severity::Info, key!("import-card-group-infos")),
        ]
        .into_iter()
        .filter_map(|(severity, heading)| {
            let rows: Vec<String> = kinds
                .iter()
                .filter(|k| k.severity == severity)
                .map(|k| {
                    let title = ew_api::checks::messages::title(&k.rule, strings::rule_language())
                        .or_else(|| ew_api::checks::rule(&k.rule).map(|r| r.meta().title.to_owned()))
                        .unwrap_or_else(|| k.rule.clone());
                    strings::format(key!("import-card-row"), &[&k.count.to_string(), &title])
                })
                .collect();
            (!rows.is_empty()).then(|| super::ImportGroup {
                heading: strings::text(heading).to_owned(),
                rows,
            })
        })
        .collect();
        let card = Dialog::ImportQet {
            file: name.clone(),
            summary,
            verdict,
            note: library_note.map(str::to_owned),
            groups,
        };
        // A dialog open when the job ends stays; the card waits until it closes (`poll_import`).
        let waiting = if self.dialog.is_some() {
            Some(card)
        } else {
            let _ = self.open_dialog(card);
            None
        };
        self.import.pending = Some(PendingImport {
            session,
            dir,
            banner,
            waiting,
        });
        Outcome::Done
    }

    /// `file.import_qet_apply {confirm}` (the import card's Create project): the project written into the chosen
    /// folder and opened. The card says what is made, so its button is the confirmation and sends `confirm`; a
    /// script names it too. Without an import card it is refused.
    pub(crate) fn act_import_qet_apply(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(Dialog::ImportQet { file, .. }) = self.dialog.as_ref() else {
            return Outcome::refused(key!("reason-no-import"));
        };
        if !flag(p, "confirm") {
            return failed(key!("error-import-confirm"), &[file]);
        }
        // The project open now (it stayed editable while the importer ran) asks again before it closes; the
        // question replaces the card, and Cancel returns to it.
        if self.has_project() {
            let card = self.dialog.clone();
            if let Some(outcome) = self.guard_unsaved("file.import_qet_apply", p) {
                if matches!(self.dialog, Some(Dialog::ConfirmDiscard { .. })) {
                    self.recovery_stash = card;
                }
                return outcome;
            }
        }
        self.dialog = None;
        let Some(pending) = self.import.pending.take() else {
            return Outcome::refused(key!("reason-no-import"));
        };
        self.close_project();
        let outcome = self.create_and_install(
            pending.session,
            &pending.dir,
            key!("error-import-write"),
            Some(pending.banner),
        );
        // D8: the new project opens on its migration report in Messages.
        if outcome == Outcome::Done {
            let mut on = Map::new();
            on.insert("on".into(), Value::from(true));
            let _ = self.act_messages_show(&on);
            let _ = self.act_messages_migration(&on);
        }
        outcome
    }

    /// The starter library an import resolves its built-in keys in, and the banner's note when there is none.
    fn import_library(&self) -> (StarterLibrary, Option<&'static str>) {
        match self.config.resources.content_dir.as_deref() {
            Some(content) => match StarterLibrary::from_dir(content) {
                Ok(l) => (l, None),
                Err(_) => (
                    StarterLibrary::compiled(),
                    Some(strings::text(key!("banner-import-no-content"))),
                ),
            },
            None => (
                StarterLibrary::compiled(),
                Some(strings::text(key!("banner-import-no-content"))),
            ),
        }
    }

    /// `file.reimport_qet {file?, accept?}`: the QElectroTech file re-imported into its earlier
    /// import in the open project. A file matched by its items asks T6 first (`accept: true` answers it); a file of
    /// another project is refused with T7; else the re-import card shows T8 and its rows. Nothing changes until Apply.
    /// As the import, the importer runs as a job, once (D4); the plan and its preview follow on this thread.
    pub(crate) fn act_reimport_qet(&mut self, p: &Map<String, Value>) -> Outcome {
        let file = match string_param(p, "file") {
            Err(e) => return e,
            Ok(Some(file)) => PathBuf::from(file),
            Ok(None) => return self.need_path("file.reimport_qet", Prompt::OpenQet, "file", p),
        };
        if !self.is_writable() {
            return Outcome::refused(key!("reason-read-only"));
        }
        let name = file
            .file_name()
            .map_or_else(|| "project.qet".to_owned(), |n| n.to_string_lossy().into_owned());
        let Ok(bytes) = std::fs::read(&file) else {
            return failed(key!("error-import-read"), &[&name]);
        };
        let (library, _) = self.import_library();
        let accept = flag(p, "accept");
        let Some(project) = self.project.as_ref() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let epoch = project.epoch;
        let lineages = match project.session.qet_lineages() {
            Ok(l) => l,
            Err(e) => return self.reimport_refused(&name, &e),
        };
        // The proposal this file was shown names its lineage: accepted, the job imports the file into it at once.
        let proposed = self
            .import
            .reimport_proposal
            .take()
            .filter(|(at, path, _)| accept && *at == epoch && *path == file)
            .map(|(_, _, lineage)| lineage);
        let options = JobOptions {
            name: "reimport".to_owned(),
            waker: self.jobs.waker.clone(),
            gate: self.jobs.hooks.gate(),
        };
        let job_name = name.clone();
        let hooks = self.jobs.hooks.clone();
        let job = Job::spawn(options, move |_| {
            // A test fault (feature `testkit` only), as the import job has.
            hooks.fault("reimport");
            Ok(find_reimport(
                &lineages, &job_name, bytes, &library, proposed, accept,
            ))
        });
        let Ok(job) = job else {
            return failed(key!("error-import-read"), &[&name]);
        };
        let pending = ReimportJob {
            job,
            name,
            path: file,
            epoch,
        };
        // A script (headless or driving the window) waits for the job, so its next step finds the card.
        if self.mode == Mode::Headless || !self.is_person() {
            let ReimportJob {
                job,
                name,
                path,
                epoch,
            } = pending;
            return self.finish_reimport(job.wait(), name, path, epoch);
        }
        self.banner = Some(strings::format(key!("banner-importing"), &[&pending.name]));
        self.import.reimport_job = Some(pending);
        Outcome::Done
    }

    /// The banner and outcome of a refused re-import.
    fn reimport_refused(&mut self, name: &str, e: &ReimportError) -> Outcome {
        let message = match e {
            ReimportError::Refused(r) => refusal_text(name, r),
            ReimportError::NotAnImport { .. } => strings::format(key!("error-reimport-foreign"), &[name]),
            ReimportError::Apply(e) => strings::format(key!("error-import"), &[name, &e.to_string()]),
        };
        self.banner = Some(message.clone());
        Outcome::Failed { message }
    }

    /// The end of a re-import job: a refusal in the banner, T6 for a proposed lineage, else the plan and its preview
    /// on this thread and the re-import card; T6 and the card wait behind a dialog open now. A job of a
    /// project that closed meanwhile shows nothing, whatever its end, and takes only its own "Importing…" banner away.
    pub(super) fn finish_reimport(
        &mut self,
        end: JobEnd<Result<ReimportFound, ReimportError>>,
        name: String,
        path: PathBuf,
        epoch: u64,
    ) -> Outcome {
        // The job's end takes its own "Importing…" banner away, whatever follows.
        let importing = strings::format(key!("banner-importing"), &[&name]);
        if self.banner.as_deref() == Some(importing.as_str()) {
            self.banner = None;
        }
        if self.project.as_ref().is_none_or(|p| p.epoch != epoch) {
            return Outcome::Done;
        }
        let found = match end {
            JobEnd::Done(Ok(found)) => found,
            JobEnd::Done(Err(e)) => return self.reimport_refused(&name, &e),
            JobEnd::Panicked | JobEnd::Failed(_) => {
                return self.reimport_refused(
                    &name,
                    &ReimportError::Refused(Refusal::Panicked { file: name.clone() }),
                );
            }
            JobEnd::Cancelled => {
                let message = strings::format(key!("error-import-cancelled"), &[&name]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        let Some(project) = self.project.as_mut() else {
            return Outcome::Done;
        };
        let (lineage, batch) = match found {
            ReimportFound::Proposed {
                lineage,
                source,
                percent,
            } => {
                self.palette = None;
                let dialog = Dialog::ReimportMatch {
                    path: path.to_string_lossy().into_owned(),
                    file: name,
                    source,
                    percent,
                };
                self.import.reimport_proposal = Some((epoch, path, lineage));
                return self.open_or_wait(dialog);
            }
            ReimportFound::Batch { lineage, batch } => (lineage, *batch),
        };
        let reimport = match project.session.plan_qet_reimport(&name, batch, &lineage) {
            Ok(r) => r,
            Err(e) => return self.reimport_refused(&name, &e),
        };
        let n = |c: ReimportClass| reimport.plan.count(c).to_string();
        // A file that writes nothing says so, as a table import does, and its Apply is disabled.
        let nothing = reimport.preview.is_none();
        // The file has not changed since its last import (records the user edited are kept, and
        // counted).
        let kept = reimport.plan.count(ReimportClass::EwChanged);
        let summary = if nothing && kept > 0 {
            strings::format(key!("reimport-card-nothing-kept"), &[&kept.to_string()])
        } else if nothing {
            strings::text(key!("reimport-card-nothing")).to_owned()
        } else {
            strings::format(
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
            )
        };
        // Each member's values in words, as the property grid shows them.
        let session = &project.session;
        let value = |m: &str, v: Option<&Value>, kind: &str| session.member_words(kind, m, v);
        let mut rows = Vec::new();
        for row in &reimport.plan.rows {
            let key = match row.class {
                ReimportClass::Added => key!("reimport-row-added"),
                ReimportClass::SourceChanged => key!("reimport-row-source"),
                ReimportClass::Conflict => key!("reimport-row-both"),
                ReimportClass::Removed => key!("reimport-row-removed"),
                ReimportClass::EwChanged | ReimportClass::Unchanged => continue,
            };
            // The record's words in the window's language.
            let record = match &row.record {
                ew_api::TextArg::Plain(s) => s.clone(),
                ew_api::TextArg::Text(t) => super::tools::rejected_words("", Some(t)),
            };
            rows.push(strings::format(key, &[&record]));
            for m in &row.members {
                rows.push(strings::format(
                    key!("reimport-row-member"),
                    &[
                        &strings::field_label(row.key.0.name(), &m.path),
                        &value(&m.path, m.ours.as_ref(), row.key.0.name()),
                        &name,
                        &value(&m.path, m.theirs.as_ref(), row.key.0.name()),
                    ],
                ));
            }
        }
        self.palette = None;
        self.import.reimport_pending = Some(reimport);
        self.open_or_wait(Dialog::Reimport {
            file: name,
            summary,
            rows,
            nothing,
        })
    }

    /// Opens the re-import's `dialog` (over its own proposal or card), or, while another dialog is open, keeps it until
    /// that one closes (`poll_import`).
    fn open_or_wait(&mut self, dialog: Dialog) -> Outcome {
        if self
            .dialog
            .as_ref()
            .is_some_and(|d| !matches!(d, Dialog::ReimportMatch { .. } | Dialog::Reimport { .. }))
        {
            self.import.reimport_waiting = Some(dialog);
            return Outcome::Done;
        }
        self.open_dialog(dialog)
    }

    /// `file.reimport_qet_apply` (the re-import card's Apply): the previewed re-import committed as one
    /// undo step (nothing when it changes nothing). Without a re-import card it is refused.
    pub(crate) fn act_reimport_qet_apply(&mut self) -> Outcome {
        // The card is checked before it is taken, so a refusal closes no other dialog.
        if !matches!(self.dialog, Some(Dialog::Reimport { .. })) {
            return Outcome::refused(key!("reason-no-reimport"));
        }
        // A re-import that writes nothing has no Apply; the card stays.
        if matches!(self.dialog, Some(Dialog::Reimport { nothing: true, .. })) {
            return Outcome::refused(key!("reason-table-import-nothing"));
        }
        let Some(Dialog::Reimport { file, .. }) = self.dialog.take() else {
            return Outcome::refused(key!("reason-no-reimport"));
        };
        let (Some(pending), Some(project)) = (self.import.reimport_pending.take(), self.project.as_mut())
        else {
            return Outcome::refused(key!("reason-no-reimport"));
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
        self.refresh_rows();
        outcome
    }

    pub(crate) fn act_discard_journal(&mut self, p: &Map<String, Value>) -> Outcome {
        if !flag(p, "confirm") {
            return match self.mode {
                Mode::Headless => failed(key!("error-confirm-needed"), &[]),
                Mode::Gui => {
                    // Discard… of the recovery dialog; Cancel returns to it.
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
            return Outcome::refused(key!("reason-no-project"));
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
                    sections: Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::ShellConfig;

    /// A cancelled import job shows its own text, not T1's internal error.
    #[test]
    fn a_cancelled_import_job_shows_the_cancel_text() {
        let dir = super::super::fresh_test_dir("ew-vm-import-cancelled");
        let mut shell = ShellModel::new(Mode::Gui, ShellConfig::in_dir(&dir, "0.0.0-test"));
        let text = "The import of example.qet was cancelled; nothing was imported.";
        assert_eq!(
            shell.finish_import(
                JobEnd::Cancelled,
                "example.qet".to_owned(),
                dir.join("p.ew"),
                None
            ),
            Outcome::Failed {
                message: text.to_owned()
            }
        );
        assert_eq!(shell.banner(), Some(text));
        assert!(shell.dialog().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
