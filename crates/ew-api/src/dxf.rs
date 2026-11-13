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
}

impl std::fmt::Display for DxfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DxfError::NoUnit => f.write_str("The drawing names no unit. Choose the unit it is drawn in."),
            DxfError::Refused(r) => write!(f, "{r}"),
            DxfError::Apply(e) => write!(f, "{e}"),
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
