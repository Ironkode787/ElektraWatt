// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Output ▸ Workshop ▸ Drilling… (doc 13 §9): the form "Export the drawings for drilling", one ticked row per surface
//! with holes (its words, its page, its holes, the sheets its template takes), Template to print at full size (PDF), DXF
//! for a machine, the paper and the folder (Drilling, beside the project); Export writes the files, each whole, and a
//! toast names the folder with Show in folder.

use std::path::PathBuf;

use ew_api::output::{DrillingAsk, Paper};
use serde_json::{Map, Value};

use super::{Dialog, FormDialog, FormField, FormRow, FormTable, Outcome, ShellModel, TICK, TICKED, failed};
use crate::{key, strings};

const ACTION: &str = "drilling.export";

/// A yes-or-no parameter as a form or a script gives it; absent: yes.
fn yes(p: &Map<String, Value>, name: &str) -> bool {
    match p.get(name) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s != "false",
        _ => true,
    }
}

impl ShellModel {
    /// Whether a surface of the project has holes: Drilling… is greyed without one.
    pub(crate) fn has_holes(&self) -> bool {
        self.project.as_ref().is_some_and(|p| p.session.has_holes())
    }

    /// The folder the drawings for drilling go to by default: Drilling, beside the project.
    fn drilling_folder(&self) -> Option<PathBuf> {
        let dir = &self.project.as_ref()?.dir;
        Some(
            dir.parent()
                .unwrap_or(dir)
                .join(strings::text(key!("drilling-folder"))),
        )
    }

    /// `drilling.export {surfaces?, template?, dxf?, paper?, dir?}` (Output ▸ Workshop ▸ Drilling…, the module
    /// documentation): in the window without parameters the form opens; else the ticked surfaces' files (every surface
    /// with holes without a table), into `dir` (absent: Drilling, beside the project). Refused in words: no surface
    /// with holes, nothing to make, no surface ticked.
    pub(crate) fn act_drilling_export(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let session = &project.session;
        let paper = match p.get("paper").and_then(Value::as_str) {
            None => Paper::A4,
            Some(w) => match Paper::of_word(w) {
                Some(paper) => paper,
                None => return failed(key!("error-bad-param"), &["paper"]),
            },
        };
        let offered = session.drilling_surfaces(paper);
        if offered.is_empty() {
            return Outcome::refused(key!("reason-no-holes"));
        }
        if p.is_empty() && self.mode == super::Mode::Gui && !self.in_form_of(ACTION) {
            let folder = self.drilling_folder().unwrap_or_default();
            let rows = offered
                .iter()
                .map(|o| FormRow {
                    key: strings::format(
                        key!("drilling-row"),
                        &[
                            &strings::render(&o.words),
                            o.folio.as_deref().unwrap_or("—"),
                            &strings::format_count(crate::count_key!("drilling-holes"), o.holes),
                            &strings::format_count(crate::count_key!("drilling-sheets"), o.sheets),
                        ],
                    ),
                    cells: vec![TICKED.to_owned()],
                })
                .collect();
            let field = |name: &'static str, label: strings::Key<'static>, choices, value: &str| FormField {
                label: None,
                name: name.into(),
                label_key: label,
                choices,
                options: Vec::new(),
                value: value.to_owned(),
            };
            return self.open_dialog(Dialog::Form(FormDialog {
                action: ACTION,
                title_key: key!("drilling-title"),
                ok_key: key!("drilling-export"),
                fields: vec![
                    field(
                        "template",
                        key!("drilling-template"),
                        &super::choices::BOOL,
                        "true",
                    ),
                    field("dxf", key!("drilling-dxf"), &super::choices::BOOL, "true"),
                    field("paper", key!("drilling-paper"), &super::choices::PAPERS, "a4"),
                    field(
                        "dir",
                        key!("drilling-dir"),
                        &super::choices::NONE,
                        &folder.to_string_lossy(),
                    ),
                ],
                error: None,
                table: Some(FormTable {
                    name: "surfaces",
                    heading_key: key!("drilling-surfaces"),
                    fixed: true,
                    key_label: key!("drilling-surface"),
                    add_key: key!("form-value-set-add"),
                    columns: vec![(strings::text(key!("drilling-tick")).to_owned(), TICK.to_owned())],
                    rows,
                }),
            }));
        }
        let (template, dxf) = (yes(p, "template"), yes(p, "dxf"));
        if !template && !dxf {
            return self.form_failed(ACTION, strings::text(key!("drilling-nothing")).to_owned());
        }
        let tick = strings::text(key!("drilling-tick"));
        let surfaces: Vec<_> = match p.get("surfaces").and_then(Value::as_array) {
            Some(rows) => offered
                .iter()
                .zip(rows)
                .filter(|(_, r)| {
                    r.get("values")
                        .and_then(|v| v.get(tick))
                        .and_then(Value::as_str)
                        .is_some_and(|v| v == TICKED)
                })
                .map(|(o, _)| o.surface)
                .collect(),
            None => offered.iter().map(|o| o.surface).collect(),
        };
        if surfaces.is_empty() {
            return self.form_failed(ACTION, strings::text(key!("drilling-none-ticked")).to_owned());
        }
        let dir = match p
            .get("dir")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|d| !d.is_empty())
        {
            Some(d) => PathBuf::from(d),
            None => self.drilling_folder().unwrap_or_default(),
        };
        let ask = DrillingAsk {
            surfaces,
            paper,
            template,
            dxf,
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let files = match project.session.drilling_files(&ask) {
            Ok(f) => f,
            Err(e) => return failed(key!("error-export"), &[&e]),
        };
        if let Err(e) = ew_api::Session::write_drilling_files(&dir, &files) {
            return failed(key!("error-export"), &[&crate::jobs::io_cause(&e)]);
        }
        let folder = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let text = strings::format_counted(
            crate::count_key!("toast-drilling-written"),
            files.len(),
            &[&folder],
        );
        if self.in_form_of(ACTION) {
            self.dialog = None;
        }
        self.notify_folder(text, "edit", dir);
        Outcome::Done
    }
}
