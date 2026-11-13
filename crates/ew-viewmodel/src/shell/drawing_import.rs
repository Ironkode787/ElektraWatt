// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! File ▸ Import… of a drawing (WP-S3.19, doc 14 §13): a DXF or a DWG becomes a new graphic page or an underlay of the
//! shown page; a PDF page becomes an underlay. The dialog says what the file is, whether its unit had to be assumed,
//! that a DWG needs the user's ODA File Converter, and that an underlay stays outside the project unless embedded (a
//! customer's drawing may be confidential). A DWG asks for the converter's program once and keeps it in the settings.

use std::path::{Path, PathBuf};

use ew_api::dxf::{DxfOptions, Unit};
use ew_api::output::UnderlaySource;
use serde_json::{Map, Value};

use super::{Dialog, Outcome, Prompt, ShellModel, failed, flag, string_param};
use crate::key;
use crate::strings;

/// The kind of drawing File ▸ Import… takes, by the file's extension.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawingKind {
    /// A DXF drawing.
    Dxf,
    /// A DWG drawing, read through the ODA File Converter.
    Dwg,
    /// A page of a PDF, as an underlay only.
    Pdf,
}

impl DrawingKind {
    /// The kind of an extension (`dxf`, `dwg`, `pdf`, any case).
    pub fn of(ext: &str) -> Option<DrawingKind> {
        match ext.to_ascii_lowercase().as_str() {
            "dxf" => Some(DrawingKind::Dxf),
            "dwg" => Some(DrawingKind::Dwg),
            "pdf" => Some(DrawingKind::Pdf),
            _ => None,
        }
    }
}

impl ShellModel {
    /// `file.import_drawing`: without `as`, the dialog; with `as` (`page`, or `underlay` on the shown page, `embed` for
    /// an embedded source), the import, one undo step. `units` reads a drawing whose header names none; `converter`
    /// is the ODA File Converter's program, kept in the settings.
    pub(crate) fn act_import_drawing(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.project.is_none() {
            return Outcome::refused(key!("reason-no-project"));
        }
        let file = match string_param(p, "file") {
            Err(e) => return e,
            Ok(Some(file)) => PathBuf::from(file),
            Ok(None) => return self.need_path("file.import_drawing", Prompt::OpenImport, "file", p),
        };
        let name = file
            .file_name()
            .map_or_else(|| "drawing".to_owned(), |n| n.to_string_lossy().into_owned());
        let Some(kind) = file.extension().and_then(|e| e.to_str()).and_then(DrawingKind::of) else {
            return Outcome::refused(key!("reason-import-kind"));
        };
        let target = match string_param(p, "as") {
            Err(e) => return e,
            Ok(t) => t.map(str::to_owned),
        };
        let units = match string_param(p, "units") {
            Err(e) => return e,
            Ok(None) => None,
            Ok(Some(u)) => match Unit::parse(u) {
                Some(u) => Some(u),
                None => return failed(key!("error-bad-param"), &["units"]),
            },
        };
        let Some(bytes) = read(&file) else {
            return failed(key!("error-import-read"), &[&name]);
        };
        let Some(target) = target else {
            return self.drawing_dialog(&file, &name, kind, &bytes);
        };
        // A DWG: the converter first (asked for once, then kept in the settings).
        let (name, bytes, notes) = if kind == DrawingKind::Dwg {
            let chosen = match string_param(p, "converter") {
                Err(e) => return e,
                Ok(c) => c.map(str::to_owned),
            };
            if let Some(c) = &chosen {
                self.settings.dwg_converter = Some(c.clone());
                self.save_settings();
            }
            let Some(converter) = chosen.or_else(|| self.settings.dwg_converter.clone()) else {
                return self.need_path("file.import_drawing", Prompt::OpenConverter, "converter", p);
            };
            match ew_api::dxf::convert_dwg(Path::new(&converter), &name, &bytes, ew_api::dxf::DWG_TIMEOUT) {
                Ok(dxf) => {
                    let converted = Path::new(&name).with_extension("dxf").to_string_lossy().into_owned();
                    (converted, dxf, vec![ew_api::dxf::dwg_converted(&name)])
                }
                Err(e) => return self.import_failed(&name, &e.to_string()),
            }
        } else {
            (name, bytes, Vec::new())
        };
        self.dialog = None;
        let options = DxfOptions {
            unit: units,
            ..DxfOptions::default()
        };
        match target.as_str() {
            "page" if kind != DrawingKind::Pdf => self.drawing_page(&name, bytes, &options, notes),
            "underlay" => {
                let Some(folio) = self.current_folio().map(str::to_owned) else {
                    return Outcome::refused(key!("reason-no-page"));
                };
                let source = match kind {
                    DrawingKind::Pdf => UnderlaySource::Pdf { page: 1 },
                    DrawingKind::Dxf | DrawingKind::Dwg => UnderlaySource::Dxf(options),
                };
                let embed = flag(p, "embed");
                let Some(project) = self.project.as_mut() else {
                    return Outcome::refused(key!("reason-no-project"));
                };
                let outcome = match project.session.place_underlay(&folio, &name, bytes, &source, embed, notes) {
                    Ok(_) => {
                        self.notify(strings::format(key!("toast-underlay-placed"), &[&name, &folio]), true);
                        Outcome::Done
                    }
                    Err(e) => self.import_failed(&name, &e.to_string()),
                };
                self.refresh_rows();
                outcome
            }
            _ => failed(key!("error-bad-param"), &["as"]),
        }
    }

    fn import_failed(&mut self, name: &str, why: &str) -> Outcome {
        let message = strings::format(key!("error-import"), &[name, why]);
        self.banner = Some(message.clone());
        Outcome::Failed { message }
    }

    /// The drawing as a new graphic page, then shown.
    fn drawing_page(
        &mut self,
        name: &str,
        bytes: Vec<u8>,
        options: &DxfOptions,
        notes: Vec<ew_api::import::ReportEntry>,
    ) -> Outcome {
        let Some(project) = self.project.as_mut() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let imported = project.session.dxf_page_batch(name, bytes, options).and_then(|mut batch| {
            batch.report.entries.extend(notes);
            project.session.apply_page_batch(batch)
        });
        match imported {
            Ok(page) => {
                self.refresh_rows();
                let mut go = Map::new();
                go.insert("folio".to_owned(), Value::String(page.folio.clone()));
                let _ = self.act_page_goto(&go);
                self.notify(strings::format(key!("toast-drawing-imported"), &[name, &page.folio]), true);
                Outcome::Done
            }
            Err(e) => self.import_failed(name, &e.to_string()),
        }
    }

    /// The dialog of a drawing: what it is, and its buttons.
    fn drawing_dialog(&mut self, file: &Path, name: &str, kind: DrawingKind, bytes: &[u8]) -> Outcome {
        let unitless = match kind {
            DrawingKind::Dxf => match ew_api::dxf::probe_dxf(name, bytes) {
                Ok(probe) => probe.unit.is_none(),
                Err(e) => return self.import_failed(name, &e.to_string()),
            },
            DrawingKind::Dwg | DrawingKind::Pdf => false,
        };
        let dialog = Dialog::ImportDrawing {
            path: file.to_string_lossy().into_owned(),
            name: name.to_owned(),
            kind,
            folio: self.current_folio().map(str::to_owned),
            unitless,
            converter: kind == DrawingKind::Dwg && self.settings.dwg_converter.is_none(),
        };
        self.open_dialog(dialog)
    }
}

/// The bytes of a drawing, at most the importers' input limit.
fn read(path: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let limit = ew_api::import::Limits::default().max_input_bytes;
    let mut bytes = Vec::new();
    std::fs::File::open(path).ok()?.take(limit + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}
