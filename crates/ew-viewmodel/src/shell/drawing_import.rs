// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! File ▸ Import… of a drawing (doc 14 §13): a DXF or a DWG becomes a new graphic page or an underlay of the
//! shown page; a PDF page becomes an underlay. The dialog says what the file is, whether its unit had to be assumed,
//! that a DWG needs the user's ODA File Converter, and that an underlay stays outside the project unless embedded (a
//! customer's drawing may be confidential). A DWG asks for the converter's program once and keeps it in the settings
//! once it converted a drawing.
//!
//! **A job**: the file is read, a DXF probed for the dialog and a DWG converted on a job's thread, listed in
//! the Jobs pane with Cancel (which also stops the converter); its end opens the dialog or makes the page or the
//! underlay on the window's thread, since the session never leaves it. A script waits for the job.

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
    /// The extensions File ▸ Import… takes for a drawing, with their kinds.
    pub const EXTENSIONS: [(&'static str, DrawingKind); 3] = [
        ("dxf", DrawingKind::Dxf),
        ("dwg", DrawingKind::Dwg),
        ("pdf", DrawingKind::Pdf),
    ];

    /// The kind of an extension (`dxf`, `dwg`, `pdf`, any case).
    pub fn of(ext: &str) -> Option<DrawingKind> {
        let ext = ext.to_ascii_lowercase();
        Self::EXTENSIONS.iter().find(|(e, _)| *e == ext).map(|(_, k)| *k)
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
        let Some(kind) = file
            .extension()
            .and_then(|e| e.to_str())
            .and_then(DrawingKind::of)
        else {
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
        // A DWG: the converter first (asked for once, then kept in the settings once a conversion succeeded).
        let apply = match target {
            None => None,
            Some(target) => {
                let mut converter = None;
                if kind == DrawingKind::Dwg {
                    let chosen = match string_param(p, "converter") {
                        Err(e) => return e,
                        Ok(c) => c.map(str::to_owned),
                    };
                    let Some(c) = chosen.clone().or_else(|| self.settings.dwg_converter.clone()) else {
                        return self.need_path("file.import_drawing", Prompt::OpenConverter, "converter", p);
                    };
                    converter = Some((c, chosen.is_some()));
                }
                Some(Apply {
                    target,
                    units,
                    embed: flag(p, "embed"),
                    converter,
                })
            }
        };
        self.start_drawing_job(file, name, kind, apply)
    }

    /// Reads (and probes, or converts) the drawing as a job: an entry of the Jobs pane with Cancel, off the
    /// window's thread; its end ([`ShellModel::finish_drawing`]) opens the dialog or makes the page or the underlay. A
    /// script waits for it.
    fn start_drawing_job(
        &mut self,
        file: PathBuf,
        name: String,
        kind: DrawingKind,
        apply: Option<Apply>,
    ) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let (rev, epoch) = (project.session.world().rev.0, project.epoch);
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let options = ew_api::ew_jobs::JobOptions {
            name: crate::jobs::JobKind::Import.word().to_owned(),
            waker: self.jobs.waker.clone(),
            gate: self.jobs.hooks.gate(),
        };
        let hooks = self.jobs.hooks.clone();
        let body = {
            let (file, name, stop) = (file.clone(), name.clone(), std::sync::Arc::clone(&stop));
            let converter = apply.as_ref().and_then(|a| a.converter.clone()).map(|(c, _)| c);
            let probe = apply.is_none();
            // A drawing that becomes a page is converted here too, off the window's thread.
            let page = apply
                .as_ref()
                .filter(|a| a.target == "page" && kind != DrawingKind::Pdf)
                .map(|a| DxfOptions {
                    unit: a.units,
                    ..DxfOptions::default()
                });
            move |ctl: &ew_api::ew_jobs::JobCtl| {
                // A test fault (feature `testkit` only): the body panics, as a reader's panic would end it.
                hooks.fault("drawing");
                ctl.checkpoint("drawing")?;
                Ok(
                    read_drawing(&file, &name, kind, probe, converter.as_deref(), &stop).and_then(
                        |mut drawn| {
                            if let Some(options) = &page {
                                let converted = ew_api::dxf::convert_dxf(&drawn.name, &drawn.bytes, options)
                                    .map_err(|e| DrawingError::Refused(e.to_string()))?;
                                drawn.converted = Some(converted);
                            }
                            Ok(drawn)
                        },
                    ),
                )
            }
        };
        let Ok(job) = ew_api::ew_jobs::Job::spawn(options, body) else {
            return failed(key!("error-import-read"), &[&name]);
        };
        let number = self
            .jobs
            .start_flagged(crate::jobs::JobKind::Import, name.clone(), rev, epoch, stop);
        let after = AfterDrawing {
            number,
            epoch,
            file,
            name,
            kind,
            apply,
        };
        if self.mode == super::Mode::Headless || !self.is_person() {
            return self.finish_drawing(job.wait(), after);
        }
        self.import.drawing = Some(DrawingJob { job, after });
        Outcome::Done
    }

    /// Takes the end of a running drawing import (the window every frame, through `poll_jobs`).
    pub(super) fn poll_drawing(&mut self) {
        let ended = self.import.drawing.as_mut().and_then(|d| d.job.try_end());
        if let Some(end) = ended
            && let Some(pending) = self.import.drawing.take()
        {
            let outcome = self.finish_drawing(end, pending.after);
            self.say_failed_end(outcome);
        }
    }

    /// Waits for a running drawing import and takes its end (the end of a headless run).
    pub(super) fn wait_drawing(&mut self) {
        if let Some(DrawingJob { job, after }) = self.import.drawing.take() {
            let outcome = self.finish_drawing(job.wait(), after);
            self.say_failed_end(outcome);
        }
    }

    /// The end of a drawing import: its entry leaves the Jobs pane; a refusal or a cancel is said in words; else the
    /// dialog opens, or the page or the underlay is made. A job of a project that closed meanwhile imports nothing and
    /// says so.
    fn finish_drawing(
        &mut self,
        end: ew_api::ew_jobs::JobEnd<Result<Drawn, DrawingError>>,
        pending: AfterDrawing,
    ) -> Outcome {
        self.jobs.entries.retain(|e| e.number != pending.number);
        if self.project.as_ref().is_none_or(|p| p.epoch != pending.epoch) {
            return failed(key!("error-import-dropped"), &[&pending.name]);
        }
        let name = pending.name;
        let drawn = match end {
            ew_api::ew_jobs::JobEnd::Done(Ok(d)) => d,
            ew_api::ew_jobs::JobEnd::Done(Err(DrawingError::Cancelled))
            | ew_api::ew_jobs::JobEnd::Cancelled => {
                let message = strings::format(key!("error-import-cancelled"), &[&name]);
                self.notify(message.clone(), false);
                return Outcome::Failed { message };
            }
            ew_api::ew_jobs::JobEnd::Done(Err(DrawingError::Read)) => {
                return failed(key!("error-import-read"), &[&name]);
            }
            ew_api::ew_jobs::JobEnd::Done(Err(DrawingError::Refused(why))) => {
                return self.import_failed(&name, &why);
            }
            ew_api::ew_jobs::JobEnd::Panicked | ew_api::ew_jobs::JobEnd::Failed(_) => {
                return failed(key!("error-import-panicked"), &[&name]);
            }
        };
        let Some(apply) = pending.apply else {
            return self.drawing_dialog(&pending.file, &name, pending.kind, drawn.unitless);
        };
        // A converter chosen now is kept once it converted the drawing.
        if let Some((converter, true)) = apply.converter {
            self.settings.dwg_converter = Some(converter);
            self.save_settings();
        }
        let (name, bytes, notes, kind, converted) = (
            drawn.name,
            drawn.bytes,
            drawn.notes,
            pending.kind,
            drawn.converted,
        );
        let (target, units) = (apply.target, apply.units);
        self.dialog = None;
        let options = DxfOptions {
            unit: units,
            ..DxfOptions::default()
        };
        match target.as_str() {
            "page" if kind != DrawingKind::Pdf => self.drawing_page(&name, bytes, &options, notes, converted),
            "underlay" => {
                let Some(folio) = self.current_folio().map(str::to_owned) else {
                    return Outcome::refused(key!("reason-no-page"));
                };
                let source = match kind {
                    DrawingKind::Pdf => UnderlaySource::Pdf { page: 1 },
                    DrawingKind::Dxf | DrawingKind::Dwg => UnderlaySource::Dxf(options),
                };
                let embed = apply.embed;
                let Some(project) = self.project.as_mut() else {
                    return Outcome::refused(key!("reason-no-project"));
                };
                let outcome = match project
                    .session
                    .place_underlay(&folio, &name, bytes, &source, embed, notes)
                {
                    Ok(_) => {
                        self.notify(
                            strings::format(key!("toast-underlay-placed"), &[&name, &folio]),
                            true,
                        );
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

    pub(super) fn import_failed(&mut self, name: &str, why: &str) -> Outcome {
        // `error-import` ends the sentence: a reason that is a sentence of its own loses its full stop.
        let message = strings::format(key!("error-import"), &[name, strings::without_stop(why)]);
        self.banner = Some(message.clone());
        Outcome::Failed { message }
    }

    /// The drawing as a new graphic page (from its conversion on the job's thread, when it has one), then shown.
    fn drawing_page(
        &mut self,
        name: &str,
        bytes: Vec<u8>,
        options: &DxfOptions,
        notes: Vec<ew_api::import::ReportEntry>,
        converted: Option<ew_api::dxf::Converted>,
    ) -> Outcome {
        let Some(project) = self.project.as_mut() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let batch = match converted {
            Some(converted) => project
                .session
                .dxf_page_batch_converted(name, bytes, options, converted),
            None => project.session.dxf_page_batch(name, bytes, options),
        };
        let imported = batch.and_then(|mut batch| {
            batch.report.entries.extend(notes);
            project.session.apply_page_batch(batch)
        });
        match imported {
            Ok(page) => {
                self.refresh_rows();
                let mut go = Map::new();
                go.insert("folio".to_owned(), Value::String(page.folio.clone()));
                let _ = self.act_page_goto(&go);
                self.notify(
                    strings::format(key!("toast-drawing-imported"), &[name, &page.folio]),
                    true,
                );
                Outcome::Done
            }
            Err(e) => self.import_failed(name, &e.to_string()),
        }
    }

    /// The dialog of a drawing: what it is (`unitless`: a DXF whose header names no unit), and its buttons.
    fn drawing_dialog(&mut self, file: &Path, name: &str, kind: DrawingKind, unitless: bool) -> Outcome {
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

/// What a drawing import does after its job: the target (`page`, `underlay`), the unit of a drawing without one, an
/// underlay's `embed`, and a DWG's converter with whether it was chosen now (kept in the settings once it converted).
pub(crate) struct Apply {
    target: String,
    units: Option<Unit>,
    embed: bool,
    converter: Option<(String, bool)>,
}

/// A drawing import running as a job: the job, and what its end needs.
pub(crate) struct DrawingJob {
    job: ew_api::ew_jobs::Job<Result<Drawn, DrawingError>>,
    after: AfterDrawing,
}

/// What a drawing import's end needs: its entry in the Jobs pane, the project session it is for, the file, and what it
/// does (`apply`: none opens the dialog).
pub(crate) struct AfterDrawing {
    number: u64,
    epoch: u64,
    file: PathBuf,
    name: String,
    kind: DrawingKind,
    apply: Option<Apply>,
}

impl std::fmt::Debug for DrawingJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DrawingJob({})", self.after.number)
    }
}

/// What a drawing import's job read: the name and bytes the import takes (a DWG's converted DXF), its report notes,
/// whether a DXF's header names no unit, and the drawing converted for a page.
pub(crate) struct Drawn {
    name: String,
    bytes: Vec<u8>,
    notes: Vec<ew_api::import::ReportEntry>,
    unitless: bool,
    converted: Option<ew_api::dxf::Converted>,
}

/// Why a drawing import's job read nothing.
pub(crate) enum DrawingError {
    /// The file cannot be read, or is larger than the input limit.
    Read,
    /// The reader or the converter refused it, in words.
    Refused(String),
    /// Its Cancel was pressed.
    Cancelled,
}

/// The job's body: the drawing at `file` read through the input limit, then probed (`probe`: a DXF's unit, for the
/// dialog) or, for a DWG with its `converter`, converted (until `stop` is set).
fn read_drawing(
    file: &Path,
    name: &str,
    kind: DrawingKind,
    probe: bool,
    converter: Option<&str>,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<Drawn, DrawingError> {
    let stopped = || stop.load(std::sync::atomic::Ordering::Relaxed);
    let bytes = read_limited(file).ok().flatten().ok_or(DrawingError::Read)?;
    if stopped() {
        return Err(DrawingError::Cancelled);
    }
    if probe {
        let unitless = match kind {
            DrawingKind::Dxf => ew_api::dxf::probe_dxf(name, &bytes)
                .map_err(|e| DrawingError::Refused(e.to_string()))?
                .unit
                .is_none(),
            DrawingKind::Dwg | DrawingKind::Pdf => false,
        };
        return Ok(Drawn {
            name: name.to_owned(),
            bytes: Vec::new(),
            notes: Vec::new(),
            unitless,
            converted: None,
        });
    }
    let Some(converter) = converter.filter(|_| kind == DrawingKind::Dwg) else {
        return Ok(Drawn {
            name: name.to_owned(),
            bytes,
            notes: Vec::new(),
            unitless: false,
            converted: None,
        });
    };
    match ew_api::dxf::convert_dwg(Path::new(converter), name, &bytes, ew_api::dxf::DWG_TIMEOUT, stop) {
        Ok(dxf) => Ok(Drawn {
            name: Path::new(name)
                .with_extension("dxf")
                .to_string_lossy()
                .into_owned(),
            bytes: dxf,
            notes: vec![ew_api::dxf::dwg_converted(name)],
            unitless: false,
            converted: None,
        }),
        Err(ew_api::dxf::DwgError::Cancelled) => Err(DrawingError::Cancelled),
        Err(e) => Err(DrawingError::Refused(e.to_string())),
    }
}

/// The bytes of a file an importer reads, `None` when it is larger than the importers' input limit (never more than
/// one byte past the limit is read).
pub(super) fn read_limited(path: &Path) -> std::io::Result<Option<Vec<u8>>> {
    use std::io::Read;
    let limit = ew_api::import::Limits::default().max_input_bytes;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    Ok((bytes.len() as u64 <= limit).then_some(bytes))
}
