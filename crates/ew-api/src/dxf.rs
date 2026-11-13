// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! DXF import into the open project (doc 14 §6.2): a drawing through the front door as a new graphic page, committed
//! by `import.apply` as one undo step with its import run.
//!
//! A file the project already holds an import of is imported again as a copy (doc 14 §2.3 `Copy{of, n}`), so the
//! same drawing can become a second page. The page takes the folio after the last page of the page order, in its
//! section and on its grid (a project without pages: folio 1 on the QET 10 px grid, `page.create`'s default).

use ew_derive::ew_ids::Id;
use ew_import_dxf::ew_interop_api::{ImportContext, ImportInput, ImportMode};
use ew_schema::{ImportLineage, LineageAnchor, Page, Record};
use ew_units::Grid;

pub use ew_import_dxf::{DxfOptions, NO_UNIT, Probe, Space, Unit};

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
    pub(crate) fn import_copy(&self, importer: &str, sha: [u8; 32], first: ew_derive::ew_ids::Uuid) -> Option<u32> {
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

    /// The folio, section and grid of a page added after the last page of the page order.
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
        let number = q
            .pages()
            .iter()
            .filter(|(_, p)| p.folio.section == last.section)
            .map(|(_, p)| p.folio.number)
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
    pub fn dxf_page_batch(&self, name: &str, bytes: Vec<u8>, options: &DxfOptions) -> Result<ImportBatch, DxfError> {
        let input = ImportInput::new(name, bytes).map_err(|e| {
            DxfError::Refused(Refusal::Import(ImportError::Unsupported { message: e.to_string() }))
        })?;
        let sha = input.sha256();
        let copy = self.import_copy(ew_import_dxf::IMPORTER_ID, sha, ew_import_dxf::first_namespace(sha));
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
        Ok(front_door(name, &[input.bytes().len() as u64], &limits, None, || {
            ew_import_dxf::import_page(&input, &ctx, options, &page, copy)
        })?)
    }

    /// Imports the DXF drawing `bytes` named `name` as a new graphic page (doc 14 §6.2): one undo step.
    ///
    /// # Errors
    ///
    /// As [`Session::dxf_page_batch`], and [`DxfError::Apply`] when the transaction is refused.
    pub fn import_dxf_page(&mut self, name: &str, bytes: Vec<u8>, options: &DxfOptions) -> Result<DxfPage, DxfError> {
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
    part.version = ew_import_catalog::bump_patch(&doc.version)
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
        }
    }
}

impl std::error::Error for DwgError {}

/// A fresh, empty working folder under the system's temporary folder.
fn work_dir() -> Result<std::path::PathBuf, DwgError> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let base = std::env::temp_dir();
    for _ in 0..64 {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = base.join(format!("ew-dwg-{}-{n}", std::process::id()));
        if std::fs::create_dir(&dir).is_ok() {
            return Ok(dir);
        }
    }
    Err(DwgError::Failed("no working folder could be made".to_owned()))
}

/// The DXF of the DWG `bytes` named `name`, made by the user's ODA File Converter `converter` (doc 14 §6.5): the
/// converter runs as an external process on a copy in a fresh temporary folder (`<converter> <in> <out> ACAD2018 DXF 0
/// 1 *.DWG`), at most `timeout`; the DXF it writes is read under the DXF input limit, and the folder is removed. The
/// converter is never bundled or downloaded.
///
/// # Errors
///
/// [`DwgError`]: the converter fails, times out or writes nothing.
pub fn convert_dwg(
    converter: &std::path::Path,
    name: &str,
    bytes: &[u8],
    timeout: std::time::Duration,
) -> Result<Vec<u8>, DwgError> {
    let work = work_dir()?;
    let result = convert_in(&work, converter, name, bytes, timeout);
    let _ = std::fs::remove_dir_all(&work);
    result
}

fn convert_in(
    work: &std::path::Path,
    converter: &std::path::Path,
    name: &str,
    bytes: &[u8],
    timeout: std::time::Duration,
) -> Result<Vec<u8>, DwgError> {
    let failed = |e: std::io::Error| DwgError::Failed(e.kind().to_string());
    let (input, output) = (work.join("in"), work.join("out"));
    std::fs::create_dir(&input).map_err(failed)?;
    std::fs::create_dir(&output).map_err(failed)?;
    // The copy keeps only a plain stem of the file's name, so the converter's file filter finds it.
    let stem: String = std::path::Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().chars().filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_')).collect())
        .filter(|s: &String| !s.is_empty())
        .unwrap_or_else(|| "drawing".to_owned());
    std::fs::write(input.join(format!("{stem}.dwg")), bytes).map_err(failed)?;
    let mut child = std::process::Command::new(converter)
        .arg(&input)
        .arg(&output)
        .args(["ACAD2018", "DXF", "0", "1", "*.DWG"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(failed)?;
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait().map_err(failed)? {
            Some(status) => break status,
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(DwgError::TimedOut);
            }
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
        return Err(DwgError::Failed(format!("its DXF is larger than the {limit} bytes an import reads")));
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
