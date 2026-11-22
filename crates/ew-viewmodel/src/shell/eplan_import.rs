// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! File ▸ Import… of EPLAN data (WP-S3.18, doc 14 §5, §13): an EDZ or a parts XML, and a report spreadsheet.
//!
//! **Parts** (`.edz`, `.7z`, `.zip`, `.xml`): the Data Portal terms notice first, once per session, for an EDZ (it is
//! logged with the import, EW-IMP-201); then a dialog that says how many parts the file holds, how many of them the
//! project has a copy of and how many of those copies would change. *Update* previews the update of those copies on
//! the bulk card, which lists the changed fields and the devices (FR-PRT-014, AC-38); *Add to my parts library* writes
//! the parts into the user's own library (changed parts as their next patch version). An import never creates a part in
//! the project.
//!
//! **A report** (`.xlsx`, `.csv`): the dialog offers the mappings that fit the file's headers (the user's saved one and
//! the shipped German and English ones) and makes a new project without drawings from the one chosen.

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
    /// The parts of a file and what they mean for the project.
    Parts {
        /// The parts the file holds (rejected ones not counted).
        read: usize,
        /// Those the project has a copy of.
        matched: usize,
        /// The copies that would change.
        changed: usize,
    },
    /// A report's mappings that fit the file: `(name, target)`, the user's saved mapping first (`saved`).
    Report {
        /// The mappings that fit, by name.
        fits: Vec<(String, ReportTarget)>,
        /// Whether the first of them is the user's saved mapping.
        saved: bool,
    },
}

impl ShellModel {
    /// `file.import_eplan {file?, how?, preset?, terms?, dir?, discard?}`: without `how`, the dialog of the file; with it,
    /// the import (`update`, `library` or `report`), the report's column mapper (`map`) or the parts import (`parts`).
    pub(crate) fn act_import_eplan(&mut self, p: &Map<String, Value>) -> Outcome {
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
            return self.eplan_parts(p, file, name);
        }
        let limits = ew_api::catalog::table_limits();
        let input = match ew_api::catalog::read_input(&file, &limits) {
            Ok(i) => i,
            Err(_) => return failed(key!("error-import-read"), &[&name]),
        };
        self.eplan_report(p, &file, &name, &input)
    }

    /// An EDZ or a parts XML: read and parsed as a job (FT-1763, [`super::read_jobs`]), then the terms notice, the
    /// dialog or the step `how` at its end ([`ShellModel::eplan_parts_read`]).
    fn eplan_parts(&mut self, p: &Map<String, Value>, file: PathBuf, name: String) -> Outcome {
        let Some(project) = self.project.as_ref() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let how = match string_param(p, "how") {
            Err(e) => return e,
            Ok(h) => h.map(str::to_owned),
        };
        let (terms, terms_seen) = (flag(p, "terms"), self.import.edz_terms);
        let config_dir = self.config.config_dir.clone();
        let reader = project.session.eplan_parts_read(None);
        let job_name = name.clone();
        self.start_read(&job_name, move |_| {
            let Ok(input) = ew_api::catalog::read_input(&file, &ew_api::catalog::table_limits()) else {
                return Ok(
                    Box::new(move |_: &mut ShellModel| failed(key!("error-import-read"), &[&name])) as Finish,
                );
            };
            // The notice belongs to an EDZ whatever the file is called and whatever step the call is for.
            let edz = eplan::is_edz(input.bytes());
            let parsed =
                if edz && !terms_seen && !terms {
                    None
                } else {
                    Some(eplan::fndef_map(config_dir.as_deref()).and_then(|fndefs| {
                        Ok((reader.run(&input, &fndefs).map_err(|e| e.to_string())?, fndefs))
                    }))
                };
            let read = PartsRead {
                file,
                name,
                how,
                input,
                edz,
                terms,
                parsed,
            };
            Ok(Box::new(move |s: &mut ShellModel| s.eplan_parts_read(read)) as Finish)
        })
    }

    /// The end of an EDZ's or a parts XML's read: the terms notice first, then the dialog of its parts, or the
    /// update's preview, or the parts written into the user's library.
    fn eplan_parts_read(&mut self, read: PartsRead) -> Outcome {
        let PartsRead {
            file,
            name,
            how,
            input,
            edz,
            terms,
            parsed,
        } = read;
        let (name, input) = (name.as_str(), &input);
        let Some(parsed) = parsed else {
            return self.open_dialog(Dialog::ImportEplan {
                path: file.to_string_lossy().into_owned(),
                name: name.to_owned(),
                step: EplanStep::Terms,
            });
        };
        if edz && terms {
            self.import.edz_terms = true;
        }
        let (imported, fndefs) = match parsed {
            Ok(done) => done,
            Err(why) => return self.import_failed(name, &why),
        };
        let Some(project) = self.project.as_mut() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let updates = match project.session.update_parts_from_import(&imported) {
            Ok(u) => u,
            Err(e) => return self.import_failed(name, &e.to_string()),
        };
        let changed = updates.plan.as_ref().map_or(0, |plan| plan.copies.len());
        match how.as_deref() {
            None => {
                let counts = imported.counts();
                self.open_dialog(Dialog::ImportEplan {
                    path: file.to_string_lossy().into_owned(),
                    name: name.to_owned(),
                    step: EplanStep::Parts {
                        read: counts.new + counts.updated + counts.unchanged + counts.duplicate,
                        matched: updates.matched.len(),
                        changed,
                    },
                })
            }
            Some("update") => {
                let Some(plan) = updates.plan else {
                    return Outcome::refused(key!("reason-eplan-no-change"));
                };
                self.dialog = None;
                self.preview_host_card(
                    &plan.command.name,
                    plan.command.params,
                    key!("reason-library-update"),
                )
            }
            Some("library") => {
                let reg = project.session.libraries_mut();
                let written = eplan::user_parts_library(reg).and_then(|id| {
                    eplan::import_into_library(reg, id, input, &fndefs, (None, true, false), edz)
                });
                match written {
                    Ok(done) => {
                        let c = done.import.counts();
                        self.dialog = None;
                        self.notify(
                            strings::format(
                                key!("toast-eplan-parts-added"),
                                &[
                                    name,
                                    &c.new.to_string(),
                                    &c.updated.to_string(),
                                    &c.rejected.to_string(),
                                ],
                            ),
                            false,
                        );
                        Outcome::Done
                    }
                    Err(e) => self.import_failed(name, &e.to_string()),
                }
            }
            Some(_) => failed(key!("error-bad-param"), &["how"]),
        }
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
                // A table that no report mapping fits and that reads as a parts table goes on to the parts import.
                if fits.is_empty() && self.reads_as_parts_table(file, input) {
                    if !self.has_project() {
                        return Outcome::refused(key!("reason-no-project"));
                    }
                    return self.act_parts_import(&path_params(file));
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
                self.act_parts_import(&path_params(file))
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
        // The spreadsheet read into the project's batch as a job (FT-1763), the project made at its end. The window
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

/// An EDZ's or a parts XML's read, for its end ([`ShellModel::eplan_parts_read`]).
struct PartsRead {
    /// The file.
    file: PathBuf,
    /// Its name.
    name: String,
    /// The step asked for (`update`, `library`), else the dialog.
    how: Option<String>,
    /// The file's bytes.
    input: ew_api::catalog::ImportInput,
    /// Whether it is an EDZ.
    edz: bool,
    /// Whether the call accepts the Data Portal terms.
    terms: bool,
    /// The parts read with the function definitions' map, or why not; `None` for an EDZ whose terms notice comes
    /// first.
    parsed: Option<Result<(ew_api::catalog::PartsImport, eplan::FndefMap), String>>,
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
