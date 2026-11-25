// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! DXF import into the open project (doc 14 §6.2): a drawing through the front door as a new graphic page, committed
//! by `import.apply` as one undo step with its import run.
//!
//! A file the project already holds an import of is imported again as a copy (doc 14 §2.3 `Copy{of, n}`), so the
//! same drawing can become a second page. The page takes the folio after the last page of the page order, in its
//! section and on its grid (a project without pages: folio 1 on the QET 10 px grid, `page.create`'s default).

use ew_ids::Id;
use ew_import_dxf::ew_interop_api::{ImportContext, ImportInput, ImportMode};
use ew_schema::{GraphicShape, ImportLineage, LineageAnchor, Page, PageItemKind, Record};
use ew_units::{Grid, Nm, Pt2};

pub use ew_import_dxf::{Converted, DxfOptions, NO_UNIT, Probe, Space, Unit};

use crate::Session;
use crate::import::{ApplyError, ImportBatch, ImportError, Limits, Refusal, front_door};

/// Why a DXF import did nothing; the project is unchanged in every case.
#[derive(Debug)]
pub enum DxfError {
    /// The drawing names no unit and none was given (the caller asks, EW-IMP-251).
    NoUnit,
    /// The front door refused the file.
    Refused(Refusal),
    /// The import's transaction was refused.
    Apply(ApplyError),
    /// The project has no drawn page of this folio (an underlay's page).
    NoPage(String),
    /// A source the user did not embed could not be kept in the underlay cache (the words).
    Cache(String),
}

impl std::fmt::Display for DxfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DxfError::NoUnit => f.write_str("The drawing names no unit. Choose the unit it is drawn in."),
            DxfError::Refused(r) => write!(f, "{r}"),
            DxfError::Apply(e) => write!(f, "{e}"),
            DxfError::NoPage(folio) => write!(f, "The project has no drawn page {folio}."),
            DxfError::Cache(words) => f.write_str(words),
        }
    }
}

impl std::error::Error for DxfError {}

impl From<Refusal> for DxfError {
    fn from(r: Refusal) -> DxfError {
        match r {
            Refusal::Import(ImportError::Unsupported { message }) if message == NO_UNIT => DxfError::NoUnit,
            other => DxfError::Refused(other),
        }
    }
}

/// A DXF drawing converted through the front door (its limits, a reader's panic caught), for
/// [`Session::dxf_page_batch_converted`]: what can run off the window's thread.
///
/// # Errors
///
/// [`DxfError::NoUnit`] when the drawing names no unit and `options` none; [`DxfError::Refused`] otherwise.
pub fn convert_dxf(name: &str, bytes: &[u8], options: &DxfOptions) -> Result<Converted, DxfError> {
    let limits = Limits::default();
    let converted = front_door(name, &[bytes.len() as u64], &limits, None, || {
        ew_import_dxf::convert(bytes, &limits, options.unit, &options.space, &options.show, name).map(Convert)
    })?;
    Ok(converted.0)
}

/// What a conversion returns through the front door.
struct Convert(Converted);

impl crate::import::FrontDoorOutput for Convert {}

/// What a probe returns through the front door.
struct Probed(Probe);

impl crate::import::FrontDoorOutput for Probed {}

/// What the import dialog shows of a DXF file before it is imported (doc 14 §13): its version, unit, layouts, layers,
/// blocks and extents, read through the front door.
///
/// # Errors
///
/// [`Refusal`] for a file that is no DXF, exceeds a limit, or stops the reader.
pub fn probe_dxf(name: &str, bytes: &[u8]) -> Result<Probe, Refusal> {
    let limits = Limits::default();
    front_door(name, &[bytes.len() as u64], &limits, None, || {
        ew_import_dxf::probe(bytes, &limits).map(Probed)
    })
    .map(|p| p.0)
}

/// The DXF import of a page: the page made and the import's report.
#[derive(Debug)]
pub struct DxfPage {
    /// The new page.
    pub page: Id<Page>,
    /// Its printed folio.
    pub folio: String,
    /// The importer's report (the findings of the import run).
    pub report: crate::import::ImportReport,
}

impl Session {
    /// The copy number of a new import of a file of SHA-256 `sha` (doc 14 §2.3): `None` for the first import into
    /// this project, else one more than the copies it holds.
    pub(crate) fn import_copy(&self, importer: &str, sha: [u8; 32], first: ew_ids::Uuid) -> Option<u32> {
        let q = self.query();
        let mut held = false;
        let mut copies = 0u32;
        for (_, l) in q.table::<ImportLineage>().iter() {
            if l.importer.as_str() != importer {
                continue;
            }
            match &l.anchor {
                LineageAnchor::FirstImportSha256(s) if s.0 == sha => held = true,
                LineageAnchor::Copy(c) if c.of.as_uuid() == first => copies = copies.max(c.n),
                _ => {}
            }
        }
        held.then_some(copies + 1)
    }

    /// The folio, section and grid of a page added after the last page of the page order: the section and grid of the
    /// last drawn page, and the folio after every page of that section (a generated report page too), so no page has it.
    pub(crate) fn next_folio(&self) -> (Option<Id<ew_schema::Section>>, u32, Grid) {
        let q = self.query();
        let order = q.get::<ew_derive::PageOrder>(());
        let last = order
            .entries
            .iter()
            .rev()
            .find(|e| matches!(e.page, ew_derive::PageRef::Drawn(_)));
        let Some(last) = last else {
            return (None, 1, Grid::QET_10PX);
        };
        let grid = match last.page {
            ew_derive::PageRef::Drawn(p) => q.pages().get(p).map_or(Grid::QET_10PX, |p| p.grid),
            _ => Grid::QET_10PX,
        };
        // A folio no page has: after every page of the section, the generated report pages included.
        let number = order
            .entries
            .iter()
            .filter(|e| e.section == last.section)
            .map(|e| e.number)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        (last.section, number, grid)
    }

    /// The batch of the DXF drawing `bytes` named `name` as a new graphic page, through the front door.
    ///
    /// # Errors
    ///
    /// [`DxfError::NoUnit`] when the drawing names no unit and `options` none; [`DxfError::Refused`] otherwise.
    pub fn dxf_page_batch(
        &self,
        name: &str,
        bytes: Vec<u8>,
        options: &DxfOptions,
    ) -> Result<ImportBatch, DxfError> {
        self.page_batch_of(name, bytes, options, None)
    }

    /// [`Session::dxf_page_batch`] of a drawing [`convert_dxf`] converted already (with the same `options`), so that
    /// the window converts it on a job's thread and makes the page on its own.
    ///
    /// # Errors
    ///
    /// [`DxfError::Refused`] for bytes that are no import input.
    pub fn dxf_page_batch_converted(
        &self,
        name: &str,
        bytes: Vec<u8>,
        options: &DxfOptions,
        converted: Converted,
    ) -> Result<ImportBatch, DxfError> {
        self.page_batch_of(name, bytes, options, Some(converted))
    }

    fn page_batch_of(
        &self,
        name: &str,
        bytes: Vec<u8>,
        options: &DxfOptions,
        converted: Option<Converted>,
    ) -> Result<ImportBatch, DxfError> {
        let input = ImportInput::new(name, bytes).map_err(|e| {
            DxfError::Refused(Refusal::Import(ImportError::Unsupported {
                message: e.to_string(),
            }))
        })?;
        let sha = input.sha256();
        let copy = self.import_copy(
            ew_import_dxf::IMPORTER_ID,
            sha,
            ew_import_dxf::first_namespace(sha),
        );
        let (section, number, grid) = self.next_folio();
        let title = std::path::Path::new(name)
            .file_stem()
            .map_or_else(|| name.to_owned(), |s| s.to_string_lossy().into_owned());
        let page = ew_import_dxf::PageSpec {
            section,
            number,
            grid,
            title,
        };
        let mut ctx = ImportContext::new(self.query().project().id);
        ctx.mode = ImportMode::Lenient;
        let limits = ctx.limits;
        let mut batch = match converted {
            Some(converted) => ew_import_dxf::page_batch(&input, &ctx, options, &page, copy, converted),
            None => front_door(name, &[input.bytes().len() as u64], &limits, None, || {
                ew_import_dxf::import_page(&input, &ctx, options, &page, copy)
            })?,
        };
        // The drawing is in sheet millimetres; the new page's frame maps page to sheet, so its items go through the
        // inverse of that map and print where they were drawn.
        let map = crate::output::new_page_map(&self.query(), &ew_schema::PageType::Graphic);
        if !map.is_identity() {
            for r in &mut batch.records {
                if let Record::PageItem(i) = r {
                    map_item(&mut i.kind, &|p| map.unapply(p), &|n| map.unscale_len(n));
                }
            }
        }
        Ok(batch)
    }

    /// Imports the DXF drawing `bytes` named `name` as a new graphic page (doc 14 §6.2): one undo step.
    ///
    /// # Errors
    ///
    /// As [`Session::dxf_page_batch`], and [`DxfError::Apply`] when the transaction is refused.
    pub fn import_dxf_page(
        &mut self,
        name: &str,
        bytes: Vec<u8>,
        options: &DxfOptions,
    ) -> Result<DxfPage, DxfError> {
        let batch = self.dxf_page_batch(name, bytes, options)?;
        self.apply_page_batch(batch)
    }

    /// Commits the batch of a DXF page ([`Session::dxf_page_batch`]): one undo step.
    ///
    /// # Errors
    ///
    /// [`DxfError::Apply`] when the transaction is refused.
    pub fn apply_page_batch(&mut self, batch: ImportBatch) -> Result<DxfPage, DxfError> {
        let page = batch
            .records
            .iter()
            .find_map(|r| match r {
                Record::Page(p) => Some(p.id),
                _ => None,
            })
            .ok_or(DxfError::Apply(ApplyError::NoLineage))?;
        let command = crate::import::apply_command(&batch, None, ImportMode::Lenient, None, false)
            .map_err(DxfError::Apply)?;
        let preview = self.import_preview(&command).map_err(DxfError::Apply)?;
        self.commit_import(preview).map_err(DxfError::Apply)?;
        let folio = self
            .query()
            .get::<ew_derive::PageOrder>(())
            .entries
            .iter()
            .find(|e| e.page == ew_derive::PageRef::Drawn(page))
            .map(|e| e.folio.clone())
            .unwrap_or_default();
        Ok(DxfPage {
            page,
            folio,
            report: batch.report,
        })
    }
}

/// A drawing's page item `kind` with its points through `point` and its lengths (a radius, a stroke width) through
/// `len`; a text keeps its style, its own height through `len`.
pub(crate) fn map_item(kind: &mut PageItemKind, point: &dyn Fn(Pt2) -> Pt2, len: &dyn Fn(Nm) -> Nm) {
    match kind {
        PageItemKind::Graphic(g) => {
            match &mut g.shape {
                GraphicShape::Line(l) => {
                    l.a = point(l.a);
                    l.b = point(l.b);
                }
                GraphicShape::Polyline(p) | GraphicShape::Polygon(p) => {
                    p.points.iter_mut().for_each(|q| *q = point(*q));
                }
                GraphicShape::Rect(r) => {
                    r.min = point(r.min);
                    r.max = point(r.max);
                }
                GraphicShape::Circle(c) => {
                    c.centre = point(c.centre);
                    c.r = len(c.r);
                }
                GraphicShape::Arc(a) => {
                    a.centre = point(a.centre);
                    a.r = len(a.r);
                }
                GraphicShape::Unknown(_) => {}
            }
            if let Some(s) = g.stroke.as_mut() {
                s.width = len(s.width);
            }
        }
        PageItemKind::FreeText(t) => {
            t.at = point(t.at);
            t.height = t.height.map(len);
        }
        _ => {}
    }
}

/// What a part outline import wrote.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PartOutline {
    /// The part's new version.
    pub version: String,
    /// The outline's polylines.
    pub lines: usize,
}

/// The front outline of a vendor drawing as the part's `mech.front_2d` (doc 14 §6.2, FR-IMP-035): the DXF `bytes`
/// named `name` (read in `unit` when its header names none) through the front door, written into the part of part
/// number `part_number` of the writable library at `library` as its next patch version, under the library's lock.
/// The drilling pattern of the same drawing waits for the `FeaturePattern` record (doc 12 §10).
///
/// # Errors
///
/// In words: the front door's refusal, a missing unit, no such library or part, a library that cannot be written.
pub fn import_part_outline(
    library: &std::path::Path,
    part_number: &str,
    name: &str,
    bytes: &[u8],
    unit: Option<Unit>,
) -> Result<PartOutline, String> {
    let limits = Limits::default();
    let lines = front_door(name, &[bytes.len() as u64], &limits, None, || {
        ew_import_dxf::outline(bytes, &limits, unit).map(Outline)
    })
    .map_err(|r| DxfError::from(r).to_string())?
    .0;
    if lines.is_empty() {
        return Err(format!("{name} holds no line to make an outline of."));
    }
    let mut tiers = crate::library::LibraryTiers::for_user(vec![], vec![])
        .ok_or("No user library directory is configured.")?;
    tiers.company.push(library.to_path_buf());
    let mut registry = crate::library::Registry::open(&tiers).map_err(|e| e.to_string())?;
    let lib = registry
        .libraries()
        .into_iter()
        .find(|l| l.path.as_deref() == Some(library))
        .ok_or("The folder is no library.")?;
    let doc = registry
        .documents(lib.id, ew_schema::Kind::Part)
        .into_iter()
        .find_map(|d| match d {
            ew_schema::LibDocument::Part(p) if p.item.part_number.as_str() == part_number => Some(p),
            _ => None,
        })
        .ok_or_else(|| format!("The library holds no part {part_number}."))?;
    let mut part = doc.clone();
    part.version = ew_schema::next_patch(&doc.version)
        .ok_or_else(|| format!("The part {part_number} has no version that can be raised."))?;
    part.item.mech.get_or_insert_with(Default::default).front_2d = lines.clone();
    registry
        .write(lib.id, &[ew_schema::LibDocument::Part(part.clone())])
        .map_err(|e| e.to_string())?;
    Ok(PartOutline {
        version: part.version,
        lines: lines.len(),
    })
}

/// An outline through the front door.
struct Outline(Vec<Vec<ew_units::Pt2>>);

impl crate::import::FrontDoorOutput for Outline {}

// -------------------------------------------------------------------------------------------------------------------
// DWG through the ODA File Converter (doc 14 §6.5, FR-IMP-036)

/// How long the converter may run: 120 s (doc 14 §11).
pub const DWG_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// The words of EW-IMP-257 when no converter is configured: what it is and how to give it.
pub const DWG_HOW_TO: &str = "DWG files are read through the ODA File Converter, which ElektraWatt does not include. \
     Install it from the Open Design Alliance, then give its program: with --converter, or in the window when it asks \
     for it.";

/// Why a DWG was not converted; nothing was imported.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DwgError {
    /// No converter is configured (EW-IMP-257's explanation, [`DWG_HOW_TO`]).
    NoConverter,
    /// The converter could not be started or ended with an error, in words.
    Failed(String),
    /// The converter ran longer than [`DWG_TIMEOUT`] and was stopped.
    TimedOut,
    /// The converter ended without writing a DXF.
    NoOutput,
    /// The conversion was cancelled (its stop signal was set) and the converter stopped.
    Cancelled,
}

impl std::fmt::Display for DwgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DwgError::NoConverter => f.write_str(DWG_HOW_TO),
            DwgError::Failed(why) => write!(f, "The ODA File Converter failed: {why}."),
            DwgError::TimedOut => write!(
                f,
                "The ODA File Converter did not finish within {} seconds and was stopped.",
                DWG_TIMEOUT.as_secs()
            ),
            DwgError::NoOutput => f.write_str("The ODA File Converter wrote no DXF for the drawing."),
            DwgError::Cancelled => f.write_str("The conversion was cancelled."),
        }
    }
}

impl std::error::Error for DwgError {}

/// A fresh, empty working folder under the system's temporary folder, removed with its guard: named with a random
/// suffix (the standard library's own hash seeds, from the operating system) and readable by its user only, since
/// the drawing in it may be confidential.
struct WorkDir(std::path::PathBuf);

impl WorkDir {
    fn new() -> Result<WorkDir, DwgError> {
        use std::hash::{BuildHasher, Hasher};
        let base = std::env::temp_dir();
        for _ in 0..64 {
            #[allow(
                clippy::disallowed_methods,
                clippy::disallowed_types,
                reason = "an unpredictable folder name for the converter's copy; it never reaches the model or an output"
            )]
            let suffix = std::hash::RandomState::new().build_hasher().finish();
            let dir = base.join(format!("ew-dwg-{}-{suffix:016x}", std::process::id()));
            #[allow(unused_mut, reason = "only a Unix builder is given a mode")]
            let mut made = std::fs::DirBuilder::new();
            #[cfg(unix)]
            std::os::unix::fs::DirBuilderExt::mode(&mut made, 0o700);
            if made.create(&dir).is_ok() {
                return Ok(WorkDir(dir));
            }
        }
        Err(DwgError::Failed("no working folder could be made".to_owned()))
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A running converter, stopped with the processes it started when it is dropped before it ended (a timeout, an
/// error reading its state).
struct Running {
    child: std::process::Child,
    ended: bool,
}

impl Drop for Running {
    fn drop(&mut self) {
        if self.ended {
            return;
        }
        // The converter may have started helpers of its own: its tree goes with it where the platform has a command
        // for that (a process group on Unix, `taskkill /T` on Windows); the direct child is stopped in any case.
        let pid = self.child.id().to_string();
        #[cfg(windows)]
        let tree = std::process::Command::new("taskkill")
            .args(["/T", "/F", "/PID", &pid])
            .output();
        #[cfg(unix)]
        let tree = std::process::Command::new("kill")
            .args(["-KILL", "--", &format!("-{pid}")])
            .output();
        #[cfg(not(any(windows, unix)))]
        let tree: Result<(), ()> = Ok(());
        let _ = tree;
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The DXF of the DWG `bytes` named `name`, made by the user's ODA File Converter `converter` (doc 14 §6.5): the
/// converter runs as an external process on a copy in a fresh temporary folder (`<converter> <in> <out> ACAD2018 DXF 0
/// 1 *.DWG`), at most `timeout` and until `stop` is set; the DXF it writes is read under the DXF input limit, and the
/// folder is removed. The converter is never bundled or downloaded.
///
/// # Errors
///
/// [`DwgError`]: the converter fails, times out, is cancelled or writes nothing.
pub fn convert_dwg(
    converter: &std::path::Path,
    name: &str,
    bytes: &[u8],
    timeout: std::time::Duration,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<Vec<u8>, DwgError> {
    // The guard removes the folder on every way out, a panic included.
    let work = WorkDir::new()?;
    convert_in(&work.0, converter, name, bytes, timeout, stop)
}

fn convert_in(
    work: &std::path::Path,
    converter: &std::path::Path,
    name: &str,
    bytes: &[u8],
    timeout: std::time::Duration,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<Vec<u8>, DwgError> {
    let failed = |e: std::io::Error| DwgError::Failed(e.kind().to_string());
    let (input, output) = (work.join("in"), work.join("out"));
    std::fs::create_dir(&input).map_err(failed)?;
    std::fs::create_dir(&output).map_err(failed)?;
    // The copy keeps only a plain stem of the file's name, so the converter's file filter finds it.
    let stem: String = std::path::Path::new(name)
        .file_stem()
        .map(|s| {
            s.to_string_lossy()
                .chars()
                .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
                .collect()
        })
        .filter(|s: &String| !s.is_empty())
        .unwrap_or_else(|| "drawing".to_owned());
    std::fs::write(input.join(format!("{stem}.dwg")), bytes).map_err(failed)?;
    let mut command = std::process::Command::new(converter);
    command
        .arg(&input)
        .arg(&output)
        .args(["ACAD2018", "DXF", "0", "1", "*.DWG"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // Its own process group on Unix, so that stopping it can stop what it started.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut run = Running {
        child: command.spawn().map_err(failed)?,
        ended: false,
    };
    #[allow(
        clippy::disallowed_methods,
        reason = "the converter's timeout is wall time; the instant never reaches the model or an output"
    )]
    let started = std::time::Instant::now();
    let status = loop {
        match run.child.try_wait().map_err(failed)? {
            Some(status) => {
                run.ended = true;
                break status;
            }
            // The guard stops it.
            None if started.elapsed() >= timeout => return Err(DwgError::TimedOut),
            None if stop.load(std::sync::atomic::Ordering::Relaxed) => return Err(DwgError::Cancelled),
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    };
    if !status.success() {
        return Err(DwgError::Failed(format!("it ended with {status}")));
    }
    let dxf = std::fs::read_dir(&output)
        .map_err(failed)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dxf")))
        .ok_or(DwgError::NoOutput)?;
    let limit = Limits::default().max_input_bytes;
    let mut out = Vec::new();
    use std::io::Read;
    std::fs::File::open(&dxf)
        .and_then(|f| f.take(limit + 1).read_to_end(&mut out))
        .map_err(failed)?;
    if out.len() as u64 > limit {
        return Err(DwgError::Failed(format!(
            "its DXF is larger than the {limit} bytes an import reads"
        )));
    }
    Ok(out)
}

/// The report entry of a converted DWG (EW-IMP-257).
pub fn dwg_converted(name: &str) -> crate::import::ReportEntry {
    crate::import::ReportEntry::new(
        crate::import::Code::imp(257),
        crate::import::Severity::Info,
        format!("{name} was converted to DXF by the ODA File Converter before the import."),
    )
}
