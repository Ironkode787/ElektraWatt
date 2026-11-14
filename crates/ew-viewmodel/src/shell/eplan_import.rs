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
    /// The kind of an extension (any case).
    pub fn of(ext: &str) -> Option<EplanKind> {
        match ext.to_ascii_lowercase().as_str() {
            "edz" | "7z" | "zip" => Some(EplanKind::Edz),
            "xml" => Some(EplanKind::PartsXml),
            "xlsx" | "csv" => Some(EplanKind::Report),
            _ => None,
        }
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
    /// `file.import_eplan {file?, how?, preset?, saved?, terms?, dir?}`: without `how`, the dialog of the file; with it,
    /// the import (`update`, `library` or `report`).
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
        let limits = ew_api::catalog::table_limits();
        let input = match ew_api::catalog::read_input(&file, &limits) {
            Ok(i) => i,
            Err(_) => return failed(key!("error-import-read"), &[&name]),
        };
        if kind == EplanKind::Report {
            return self.eplan_report(p, &file, &name, &input);
        }
        self.eplan_parts(p, &file, &name, kind, &input)
    }

    fn eplan_parts(
        &mut self,
        p: &Map<String, Value>,
        file: &std::path::Path,
        name: &str,
        kind: EplanKind,
        input: &ew_api::catalog::ImportInput,
    ) -> Outcome {
        if self.project.is_none() {
            return Outcome::refused(key!("reason-no-project"));
        }
        let how = match string_param(p, "how") {
            Err(e) => return e,
            Ok(h) => h.map(str::to_owned),
        };
        let edz = kind == EplanKind::Edz;
        if how.is_none() && edz && !self.import.edz_terms && !flag(p, "terms") {
            return self.open_dialog(Dialog::ImportEplan {
                path: file.to_string_lossy().into_owned(),
                name: name.to_owned(),
                step: EplanStep::Terms,
            });
        }
        if edz && flag(p, "terms") {
            self.import.edz_terms = true;
        }
        let fndefs = match eplan::fndef_map(self.config.config_dir.as_deref()) {
            Ok(m) => m,
            Err(why) => return self.import_failed(name, &why),
        };
        let Some(project) = self.project.as_mut() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let imported = match project.session.eplan_parts(input, &fndefs, None) {
            Ok(i) => i,
            Err(e) => return self.import_failed(name, &e.to_string()),
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
        let Some(how) = how else {
            let mut maps: Vec<(String, ReportMap)> = Vec::new();
            if let Some(m) = &saved {
                maps.push((saved_key.clone(), m.clone()));
            }
            maps.extend(eplan::report_presets());
            let fits = eplan::fitting_maps(input, &maps);
            let had_saved = saved.is_some() && fits.first().is_some_and(|f| *f == saved_key);
            let fits = fits
                .into_iter()
                .filter_map(|n| {
                    maps.iter()
                        .find(|(m, _)| *m == n)
                        .map(|(n, m)| (n.clone(), m.target))
                })
                .collect();
            return self.open_dialog(Dialog::ImportEplan {
                path: file.to_string_lossy().into_owned(),
                name: name.to_owned(),
                step: EplanStep::Report {
                    fits,
                    saved: had_saved,
                },
            });
        };
        if how != "report" {
            return failed(key!("error-bad-param"), &["how"]);
        }
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
        if self.has_project()
            && let Some(outcome) = self.guard_unsaved("file.import_eplan", p)
        {
            return outcome;
        }
        let dir = match string_param(p, "dir") {
            Err(e) => return e,
            Ok(Some(dir)) => PathBuf::from(dir),
            Ok(None) => return self.need_path("file.import_eplan", Prompt::NewFolder, "dir", p),
        };
        if let Some(outcome) = self.require_free_dir(&dir) {
            return outcome;
        }
        let batch = match eplan::report_batch(input, &map, &key_name) {
            Ok(b) => b,
            Err(e) => return self.import_failed(name, &e.to_string()),
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
        let outcome = self.create_and_install(session, &dir, key!("error-import-write"), None);
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
