// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! File ▸ Import… of EPLAN data (doc 14 §5, §13): an EDZ or a parts XML, and a report spreadsheet.
//!
//! **Parts** (`.edz`, `.7z`, `.zip`, `.xml`): a door into Parts (`parts.import`, doc 07 §3.6): the Data Portal terms
//! notice first, once per session, for an EDZ (it is logged with the import, EW-IMP-201); then the file's parts stand in
//! Parts as a set against My parts, and the copies the open project holds of them are offered their update there. An
//! import never creates a part in the project.
//!
//! **A report** (`.xlsx`, `.csv`): the dialog offers the mappings that fit the file's headers (the user's saved one and
//! the shipped German and English ones) and makes a new project without drawings from the one chosen; a table that no
//! mapping fits and that reads as a parts table goes on to Parts.

use std::path::PathBuf;

use ew_api::eplan::{self, ReportMap, ReportTarget};
use serde_json::{Map, Value};

use super::read_jobs::Finish;
use super::{Dialog, Outcome, Prompt, ShellModel, failed, flag, string_param};
use crate::key;
use crate::strings;

/// What File ▸ Import… takes for EPLAN data, by the file's extension.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EplanKind {
    /// An EDZ part package.
    Edz,
    /// An EPLAN parts XML or a part XML.
    PartsXml,
    /// A report spreadsheet.
    Report,
}

impl EplanKind {
    /// The extensions File ▸ Import… takes for EPLAN data, with their kinds.
    pub const EXTENSIONS: [(&'static str, EplanKind); 6] = [
        ("edz", EplanKind::Edz),
        ("7z", EplanKind::Edz),
        ("zip", EplanKind::Edz),
        ("xml", EplanKind::PartsXml),
        ("xlsx", EplanKind::Report),
        ("csv", EplanKind::Report),
    ];

    /// The kind of an extension (any case).
    pub fn of(ext: &str) -> Option<EplanKind> {
        let ext = ext.to_ascii_lowercase();
        Self::EXTENSIONS.iter().find(|(e, _)| *e == ext).map(|(_, k)| *k)
    }
}

/// The step of the EPLAN import dialog.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EplanStep {
    /// The Data Portal terms notice before an EDZ's first import of the session.
    Terms,
    /// A report's mappings that fit the file: `(name, target)`, the user's saved mapping first (`saved`).
    Report {
        /// The mappings that fit, by name.
        fits: Vec<(String, ReportTarget)>,
        /// Whether the first of them is the user's saved mapping.
        saved: bool,
    },
}

impl ShellModel {
    /// `file.import_eplan {file?, how?, preset?, terms?, dir?, discard?}`: a file of parts goes on to Parts
    /// (`parts.import`, with `terms`); a report without `how` opens the dialog of the file, with it the report's import
    /// (`report`), its column mapper (`map`) or Parts for the table as a parts table (`parts`).
    pub(crate) fn act_import_eplan(&mut self, p: &Map<String, Value>) -> Outcome {
        // The parts file's steps of before (`update`, `library`) are Parts' now: an older script is told the way there.
        if matches!(string_param(p, "how"), Ok(Some("update" | "library"))) {
            return Outcome::refused(key!("reason-eplan-how-parts"));
        }
        let file = match string_param(p, "file") {
            Err(e) => return e,
            Ok(Some(file)) => PathBuf::from(file),
            Ok(None) => return self.need_path("file.import_eplan", Prompt::OpenImport, "file", p),
        };
        let name = file
            .file_name()
            .map_or_else(|| "report".to_owned(), |n| n.to_string_lossy().into_owned());
        let Some(kind) = file.extension().and_then(|e| e.to_str()).and_then(EplanKind::of) else {
            return Outcome::refused(key!("reason-import-kind"));
        };
        if kind != EplanKind::Report {
            let mut params = path_params(&file);
            if flag(p, "terms") {
                params.insert("terms".to_owned(), Value::Bool(true));
            }
            return self.parts_from_file_import_with(&params);
        }
        let limits = ew_api::catalog::table_limits();
        let input = match ew_api::catalog::read_input(&file, &limits) {
            Ok(i) => i,
            Err(_) => return failed(key!("error-import-read"), &[&name]),
        };
        self.eplan_report(p, &file, &name, &input)
    }

    fn eplan_report(
        &mut self,
        p: &Map<String, Value>,
        file: &std::path::Path,
        name: &str,
        input: &ew_api::catalog::ImportInput,
    ) -> Outcome {
        let saved_key = ew_api::catalog::source_key_for(name);
        let saved = self
            .config
            .config_dir
            .as_deref()
            .and_then(|dir| eplan::load_report_map(dir, &saved_key));
        let how = match string_param(p, "how") {
            Err(e) => return e,
            Ok(h) => h.map(str::to_owned),
        };
        match how.as_deref() {
            None => {
                let mut maps: Vec<(String, ReportMap)> = Vec::new();
                if let Some(m) = &saved {
                    maps.push((saved_key.clone(), m.clone()));
                }
                maps.extend(eplan::report_presets());
                let fits = eplan::fitting_maps(input, &maps);
                // A table that no report mapping fits and that reads as a parts table goes on to Parts.
                if fits.is_empty() && self.reads_as_parts_table(file, input) {
                    return self.parts_from_file_import(file);
                }
                let had_saved = saved.is_some() && fits.first().is_some_and(|f| *f == saved_key);
                let fits = fits
                    .into_iter()
                    .filter_map(|n| {
                        maps.iter()
                            .find(|(m, _)| *m == n)
                            .map(|(n, m)| (n.clone(), m.target))
                    })
                    .collect();
                self.open_dialog(Dialog::ImportEplan {
                    path: file.to_string_lossy().into_owned(),
                    name: name.to_owned(),
                    step: EplanStep::Report {
                        fits,
                        saved: had_saved,
                    },
                })
            }
            Some("map") => self.open_report_map(file.to_path_buf(), input),
            Some("parts") => {
                self.dialog = None;
                self.parts_from_file_import(file)
            }
            Some("report") => {
                let (map, key_name) = match string_param(p, "preset") {
                    Err(e) => return e,
                    Ok(Some(preset)) if preset == saved_key && saved.is_some() => {
                        (saved.unwrap_or_else(|| unreachable!("checked")), saved_key)
                    }
                    Ok(Some(preset)) => match eplan::report_presets().remove(preset) {
                        Some(m) => (m, preset.to_owned()),
                        None => return failed(key!("error-bad-param"), &["preset"]),
                    },
                    Ok(None) => return failed(key!("error-missing-param"), &["preset"]),
                };
                self.run_report_import(p, name, input, map, &key_name)
            }
            Some(_) => failed(key!("error-bad-param"), &["how"]),
        }
    }

    /// A report as a new project in `dir` (asked for when absent), the unsaved project asked about first
    /// (`p` carries the action's parameters for those asks): the one end of the preset buttons and of the column
    /// mapper's Import.
    pub(super) fn run_report_import(
        &mut self,
        p: &Map<String, Value>,
        name: &str,
        input: &ew_api::catalog::ImportInput,
        map: ReportMap,
        key_name: &str,
    ) -> Outcome {
        let action = if self.parts_map().is_some() {
            "parts.map_import"
        } else {
            "file.import_eplan"
        };
        if self.has_project()
            && let Some(outcome) = self.guard_unsaved(action, p)
        {
            return outcome;
        }
        let dir = match string_param(p, "dir") {
            Err(e) => return e,
            Ok(Some(dir)) => PathBuf::from(dir),
            Ok(None) => return self.need_path(action, Prompt::NewFolder, "dir", p),
        };
        if let Some(outcome) = self.require_free_dir(&dir) {
            return outcome;
        }
        // The spreadsheet read into the project's batch as a job, the project made at its end. The window
        // answers meanwhile: the revision now tells at the end whether the open project changed since it was asked about.
        let asked = ReportAsked {
            action,
            params: p.clone(),
            rev: self.revision(),
        };
        let (input, owned, key_name) = (input.clone(), name.to_owned(), key_name.to_owned());
        self.start_read(name, move |_| {
            let batch = eplan::report_batch(&input, &map, &key_name).map_err(|e| e.to_string());
            Ok(Box::new(move |s: &mut ShellModel| s.report_read(&owned, &dir, batch, &asked)) as Finish)
        })
    }

    /// The end of a report's read: the new project made from its batch, written to `dir` and opened. When the open
    /// project changed while the file was read (its revision moved), or has unsaved changes it was not asked about, the
    /// unsaved-changes question is asked again first: Discard or Save runs the import again, Cancel keeps the project
    /// and drops what was read, which a toast says.
    fn report_read(
        &mut self,
        name: &str,
        dir: &std::path::Path,
        batch: Result<ew_api::import::ImportBatch, String>,
        asked: &ReportAsked,
    ) -> Outcome {
        if self.has_project()
            && (self.revision() != asked.rev || (self.is_dirty() && !flag(&asked.params, "discard")))
        {
            let mut again = asked.params.clone();
            again.remove("discard");
            if let Some(outcome) = self.guard_unsaved(asked.action, &again) {
                if let Some(Dialog::ConfirmDiscard { pending, .. }) = &mut self.dialog {
                    pending.cancelled = Some(key!("toast-report-read-dropped"));
                }
                return outcome;
            }
        }
        let batch = match batch {
            Ok(b) => b,
            Err(why) => return self.import_failed(name, &why),
        };
        let session = match ew_api::import::session_from_import(
            &batch,
            ew_api::import::ImportMode::Lenient,
            self.session_options(),
        ) {
            Ok(s) => s,
            Err(why) => return self.import_failed(name, &why),
        };
        self.dialog = None;
        self.close_project();
        let outcome = self.create_and_install(session, dir, key!("error-import-write"), None);
        if outcome == Outcome::Done {
            self.notify(
                strings::format(
                    key!("toast-eplan-report-imported"),
                    &[
                        name,
                        &batch
                            .report
                            .stats
                            .per_kind
                            .get("device")
                            .copied()
                            .unwrap_or(0)
                            .to_string(),
                    ],
                ),
                false,
            );
        }
        outcome
    }
}

/// What a report import was started with, for its end ([`ShellModel::report_read`]): the action and its parameters
/// that run it again, and the open project's revision when the unsaved-changes question was last answered.
struct ReportAsked {
    action: &'static str,
    params: Map<String, Value>,
    rev: Option<u64>,
}

impl ShellModel {
    /// File ▸ Import… of a file of parts: Parts on its Parts shelf, its Import… with the file.
    fn parts_from_file_import(&mut self, file: &std::path::Path) -> Outcome {
        self.parts_from_file_import_with(&path_params(file))
    }

    /// [`ShellModel::parts_from_file_import`] with `parts.import`'s parameters `p`.
    fn parts_from_file_import_with(&mut self, p: &Map<String, Value>) -> Outcome {
        self.enter_place();
        if let Some(place) = self.parts.as_deref_mut() {
            place.shelf_symbols = false;
        }
        self.act_parts_import(p)
    }
}

/// The parameters of `parts.import` for `file`.
fn path_params(file: &std::path::Path) -> Map<String, Value> {
    let mut p = Map::new();
    p.insert(
        "path".to_owned(),
        Value::String(file.to_string_lossy().into_owned()),
    );
    p
}
