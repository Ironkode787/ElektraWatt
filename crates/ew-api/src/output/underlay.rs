// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Underlays (doc 14 §5.5, FR-IMP-030; WP-S3.19): a page of a PDF or a DXF drawing placed under a page by an import
//! (`import.apply` of one `underlay` page item), and drawn by the export job when its options ask for them: the
//! window's canvas always, an export only when the user prints underlays.
//!
//! **The source.** An underlay names its source by hash. Embedded (the user's choice, with the confidentiality
//! warning of the dialog), the bytes are a project asset (`assets/<sha256>.pdf` or `.dxf`); otherwise they are kept in
//! the user's underlay cache, `<OS cache directory>/underlays/<sha256>.<ext>` (Class III), and a project opened on
//! another computer draws a frame with the file's name instead.
//!
//! **Drawing.** A PDF page is rasterised with hayro at 150 dpi of its rectangle and stretched over it; a DXF drawing
//! is converted by the DXF importer and drawn as its page items from the rectangle's corner. Both are grey at the
//! underlay's opacity and carry no entity, so no pick lands on them (an underlay is locked; the commands neither move
//! nor copy it). Rasters and conversions are cached by the source's hash for the process (at most
//! [`CACHE_ENTRIES`] each).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ew_derive::ew_ids::Id;
use ew_derive::{PageRef, Q};
use ew_dl::{Brush, Color, DisplayList, Item, Path, Stroke};
use ew_import_dxf::ew_interop_api::{ImportError, ImportInput, ImportReport, SourceInfo};
use ew_schema::{Page, PageItemKind, Underlay, UnderlayFormat};
use ew_units::{Nm, Pt2, Rect};

use super::ExportAssets;
use super::view::PageFrameOf;

/// The most rasters, and DXF conversions, the process keeps.
pub const CACHE_ENTRIES: usize = 8;

/// The importer ID of an underlay's lineage (doc 14 §2.3).
pub const UNDERLAY_IMPORTER: &str = "underlay";

/// Where the user's underlay sources are kept: `<OS cache directory>/underlays`.
pub fn underlay_cache_dir() -> Option<PathBuf> {
    ew_library::default_cache_dir().map(|d| d.join("underlays"))
}

fn hex(sha: &[u8; 32]) -> String {
    sha.iter().map(|b| format!("{b:02x}")).collect()
}

fn ext(format: &UnderlayFormat) -> &'static str {
    match format {
        UnderlayFormat::Dxf => "dxf",
        UnderlayFormat::Pdf | UnderlayFormat::Unknown(_) => "pdf",
    }
}

/// The bytes of an underlay's source: the project's asset, else the user's cache, checked against the hash.
fn source(assets: &ExportAssets, u: &Underlay) -> Option<Arc<[u8]>> {
    if let Some(b) = assets.get(&u.asset.0) {
        return Some(b);
    }
    let path = underlay_cache_dir()?.join(format!("{}.{}", hex(&u.asset.0), ext(&u.format)));
    let bytes = std::fs::read(path).ok()?;
    use sha2::Digest;
    (sha2::Sha256::digest(&bytes).as_slice() == u.asset.0).then(|| Arc::from(bytes))
}

/// A small cache by key, most recent last.
struct Cache<K, V> {
    entries: Mutex<Vec<(K, Arc<V>)>>,
}

impl<K: PartialEq + Clone, V> Cache<K, V> {
    const fn new() -> Cache<K, V> {
        Cache {
            entries: Mutex::new(Vec::new()),
        }
    }

    fn get_or(&self, key: &K, make: impl FnOnce() -> Option<V>) -> Option<Arc<V>> {
        if let Ok(mut e) = self.entries.lock()
            && let Some(i) = e.iter().position(|(k, _)| k == key)
        {
            let hit = e.remove(i);
            let v = Arc::clone(&hit.1);
            e.push(hit);
            return Some(v);
        }
        let v = Arc::new(make()?);
        if let Ok(mut e) = self.entries.lock() {
            if e.len() >= CACHE_ENTRIES {
                e.remove(0);
            }
            e.push((key.clone(), Arc::clone(&v)));
        }
        Some(v)
    }
}

type RasterKey = ([u8; 32], u32, (u32, u32), u8);
static RASTERS: Cache<RasterKey, Vec<u8>> = Cache::new();

type DxfKey = ([u8; 32], Option<String>, Option<String>);
static DRAWINGS: Cache<DxfKey, ew_import_dxf::Converted> = Cache::new();

/// The DXF drawing of an underlay, converted once per source, layout and unit.
fn drawing(u: &Underlay, bytes: &Arc<[u8]>) -> Option<Arc<ew_import_dxf::Converted>> {
    DRAWINGS.get_or(&(u.asset.0, u.layout.clone(), u.unit.clone()), || {
        let space = u
            .layout
            .clone()
            .map_or(ew_import_dxf::Space::Model, ew_import_dxf::Space::Layout);
        let unit = u.unit.as_deref().and_then(ew_import_dxf::Unit::parse);
        let limits = ew_import_dxf::ew_interop_api::Limits::default();
        let run = || ew_import_dxf::convert(bytes, &limits, unit, &space, &Default::default(), &u.name);
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(run))
            .ok()?
            .ok()
    })
}

/// The frame of an underlay whose source is not at hand: its rectangle, dashed, with the file's name.
fn missing(u: &Underlay, out: &mut Vec<Item>) {
    let grey = Color::from_rgba8(0x80, 0x80, 0x80, 0xff);
    let stroke = Stroke::solid(Nm(250_000))
        .with_brush(Brush::Solid(grey))
        .dashed(ew_dl::DashPattern::Dash42);
    out.push(Item::path(Path::rect(u.rect.0), Some(stroke), None));
}

/// The underlays of `page` drawn in page coordinates (see the module documentation).
pub(crate) fn underlay_list(q: &Q, page: Id<Page>, assets: &ExportAssets) -> DisplayList {
    let fonts = ew_text::bundled();
    let styles = super::text_style_table(q);
    let mut items = Vec::new();
    let mut underlays: Vec<(Id<ew_schema::PageItem>, Underlay)> = q
        .idx()
        .page_items_by_page(page)
        .filter_map(|id| Some((id, q.page_items().get(id)?.kind.underlay()?.clone())))
        .collect();
    underlays.sort_by_key(|(id, _)| *id);
    for (_, u) in underlays {
        let Some(bytes) = source(assets, &u) else {
            missing(&u, &mut items);
            continue;
        };
        match u.format {
            UnderlayFormat::Pdf => {
                let size = ew_scene::underlay::raster_size(u.rect.0.width(), u.rect.0.height());
                let key = (u.asset.0, u.page, size, u.opacity);
                let png = RASTERS.get_or(&key, || {
                    let run = || ew_scene::underlay::pdf_png(Arc::clone(&bytes), u.page, size, u.opacity);
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(run))
                        .ok()?
                        .ok()
                });
                match png {
                    Some(png) => {
                        use sha2::Digest;
                        let sha: [u8; 32] = sha2::Sha256::digest(png.as_slice()).into();
                        items.push(ew_scene::underlay::pdf_item(
                            Arc::from(png.as_slice()),
                            sha,
                            u.rect.0,
                        ));
                    }
                    None => missing(&u, &mut items),
                }
            }
            UnderlayFormat::Dxf => match drawing(&u, &bytes) {
                Some(d) => {
                    let from = d.extents.map_or(Pt2::ORIGIN, |e| e.min());
                    let offset = u.rect.0.min() - from;
                    let kinds: Vec<PageItemKind> = d.items.iter().map(|(_, _, k)| k.clone()).collect();
                    items.extend(
                        ew_scene::underlay::dxf_list(&kinds, offset, u.opacity, fonts, &styles).items,
                    );
                }
                None => missing(&u, &mut items),
            },
            UnderlayFormat::Unknown(_) => missing(&u, &mut items),
        }
    }
    DisplayList { items }
}

/// The rectangle a PDF page covers on `page`: its whole sheet, in page coordinates (through the inverse of its frame
/// form's page-to-sheet map).
pub(crate) fn sheet_rect(q: &Q, page: Id<Page>) -> Rect {
    let frame = q.get::<PageFrameOf>(PageRef::Drawn(page));
    let (w, h, map) = match frame.as_ref() {
        Some(f) => (
            f.form.sheet.width,
            f.form.sheet.height,
            ew_scene::forms::SheetMap::of(&f.form),
        ),
        None => (
            ew_scene::page::DEFAULT_SHEET.0,
            ew_scene::page::DEFAULT_SHEET.1,
            ew_scene::forms::SheetMap::IDENTITY,
        ),
    };
    Rect::from_points(map.unapply(Pt2::ORIGIN), map.unapply(Pt2::new(w, h)))
}

/// What an underlay is made of.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum UnderlaySource {
    /// Page `page` (1-based) of a PDF.
    Pdf {
        /// The page.
        page: u32,
    },
    /// A DXF drawing read with these options.
    Dxf(ew_import_dxf::DxfOptions),
}

/// The namespace of the first underlay placed from a source of SHA-256 `sha` (doc 14 §2.3).
pub(crate) fn first_namespace(sha: [u8; 32]) -> ew_derive::ew_ids::Uuid {
    ew_derive::ew_ids::v5::lineage_namespace(
        UNDERLAY_IMPORTER,
        &ew_derive::ew_ids::v5::LineageAnchor::FirstImportSha256(sha),
    )
    .unwrap_or_else(|_| unreachable!("\"underlay\" is a valid importer name"))
}

/// What the import front door returns for a PDF: its page count and the page's size.
struct PdfProbe;

impl crate::import::FrontDoorOutput for PdfProbe {}

/// The batch of an underlay: its lineage (importer `underlay`, anchored on the source's hash, a copy when the project
/// holds that source already), the page item and, when `embed`, the source as an asset; and the report (the DXF
/// conversion's findings).
///
/// # Errors
///
/// [`crate::dxf::DxfError`] when the source is refused (no PDF page, no DXF, no unit) by the front door.
#[allow(clippy::too_many_arguments)]
pub(crate) fn underlay_batch(
    q: &Q,
    page: Id<Page>,
    name: &str,
    bytes: Vec<u8>,
    source: &UnderlaySource,
    embed: bool,
    copy: Option<u32>,
) -> Result<crate::import::ImportBatch, crate::dxf::DxfError> {
    use crate::dxf::DxfError;
    use crate::import::{Limits, Refusal, front_door};
    use ew_derive::ew_ids::v5;
    let input = ImportInput::new(name, bytes).map_err(|e| {
        DxfError::Refused(Refusal::Import(ImportError::Unsupported {
            message: e.to_string(),
        }))
    })?;
    let sha = input.sha256();
    let limits = Limits::default();
    let shared: Arc<[u8]> = Arc::clone(input.bytes());
    let (format, rect, layout, unit, entries) = match source {
        UnderlaySource::Pdf { page: n } => {
            front_door(name, &[shared.len() as u64], &limits, None, || {
                ew_scene::underlay::pdf_page_size(Arc::clone(&shared), *n)
                    .map(|_| PdfProbe)
                    .map_err(|e| ImportError::Malformed {
                        message: e.to_string(),
                        line: None,
                    })
            })?;
            (UnderlayFormat::Pdf, sheet_rect(q, page), None, None, Vec::new())
        }
        UnderlaySource::Dxf(options) => {
            let converted = front_door(name, &[shared.len() as u64], &limits, None, || {
                ew_import_dxf::convert(
                    &shared,
                    &limits,
                    options.unit,
                    &options.space,
                    &options.show,
                    name,
                )
                .map(Probed)
            })?
            .0;
            let rect = converted.extents.unwrap_or_else(|| Rect::from_point(Pt2::ORIGIN));
            let layout = match &options.space {
                ew_import_dxf::Space::Model => None,
                ew_import_dxf::Space::Layout(l) => Some(l.clone()),
            };
            (
                UnderlayFormat::Dxf,
                rect,
                layout,
                Some(converted.unit.name().to_owned()),
                converted.entries,
            )
        }
    };
    let anchor_first = first_namespace(sha);
    let (namespace, anchor) = match copy {
        None => (
            anchor_first,
            ew_schema::LineageAnchor::FirstImportSha256(ew_schema::Sha256(sha)),
        ),
        Some(n) => (
            v5::lineage_namespace(
                UNDERLAY_IMPORTER,
                &v5::LineageAnchor::Copy { of: anchor_first, n },
            )
            .unwrap_or_else(|_| unreachable!("\"underlay\" is a valid importer name")),
            ew_schema::LineageAnchor::Copy(ew_schema::LineageCopy {
                of: Id::from_uuid(anchor_first),
                n,
                extra: Default::default(),
            }),
        ),
    };
    let lineage: Id<ew_schema::ImportLineage> = v5::imported_id(namespace, "underlay:lineage");
    let item = ew_schema::PageItem {
        id: v5::imported_id(namespace, &format!("underlay:{}", page.as_uuid())),
        v: 1,
        page,
        layer: None,
        kind: PageItemKind::Underlay(Underlay {
            asset: ew_schema::Sha256(sha),
            format: format.clone(),
            name: name.to_owned(),
            page: match source {
                UnderlaySource::Pdf { page } => *page,
                UnderlaySource::Dxf(_) => 1,
            },
            layout,
            unit,
            rect: ew_schema::Bounds(rect),
            opacity: 50,
            extra: Default::default(),
        }),
        extra: Default::default(),
    };
    let records = vec![
        ew_schema::Record::ImportLineage(ew_schema::ImportLineage {
            id: lineage,
            v: 1,
            importer: ew_mltext::Ident::from_input(UNDERLAY_IMPORTER)
                .unwrap_or_else(|_| unreachable!("an identifier")),
            source_name: name.to_owned(),
            anchor,
            namespace: Id::from_uuid(namespace),
            last_source_sha256: Some(ew_schema::Sha256(sha)),
            last_run: None,
            extra: Default::default(),
        }),
        ew_schema::Record::PageItem(item),
    ];
    let mut report = ImportReport::new(
        UNDERLAY_IMPORTER,
        "1",
        SourceInfo {
            name: name.to_owned(),
            sha256: sha,
            size: shared.len() as u64,
            format: ext(&format).to_owned(),
            format_version: None,
            detail: None,
        },
    );
    report.entries = entries;
    report.stats.per_kind.insert("import_lineage".to_owned(), 1);
    report.stats.per_kind.insert("page_item".to_owned(), 1);
    Ok(crate::import::ImportBatch {
        records,
        lib: Vec::new(),
        assets: if embed { vec![shared.to_vec()] } else { Vec::new() },
        report,
    })
}

/// A DXF conversion through the front door.
struct Probed(ew_import_dxf::Converted);

impl crate::import::FrontDoorOutput for Probed {}

impl crate::Session {
    /// Places an underlay of `bytes`, the file named `name`, on the drawn page `folio` (doc 14 §5.5): one undo step of
    /// `import.apply` with its import run. With `embed` the source becomes a project asset; otherwise it is kept in
    /// the user's underlay cache. `notes` join the run's findings (a DWG's conversion). Returns the import's report.
    ///
    /// # Errors
    ///
    /// [`crate::dxf::DxfError`]: no such page, a refused source, a cache that cannot be written, a refused transaction.
    pub fn place_underlay(
        &mut self,
        folio: &str,
        name: &str,
        bytes: Vec<u8>,
        source: &UnderlaySource,
        embed: bool,
        notes: Vec<crate::import::ReportEntry>,
    ) -> Result<crate::import::ImportReport, crate::dxf::DxfError> {
        use crate::dxf::DxfError;
        use sha2::Digest;
        let page = {
            let q = self.query();
            let order = q.get::<ew_derive::PageOrder>(());
            order
                .entries
                .iter()
                .find_map(|e| match e.page {
                    PageRef::Drawn(p) if e.folio == folio => Some(p),
                    _ => None,
                })
                .ok_or_else(|| DxfError::NoPage(folio.to_owned()))?
        };
        let sha: [u8; 32] = sha2::Sha256::digest(&bytes).into();
        let copy = self.import_copy(UNDERLAY_IMPORTER, sha, first_namespace(sha));
        let kept = (!embed).then(|| bytes.clone());
        let mut batch = underlay_batch(&self.query(), page, name, bytes, source, embed, copy)?;
        batch.report.entries.extend(notes);
        if let Some(kept) = kept {
            let format = match source {
                UnderlaySource::Pdf { .. } => UnderlayFormat::Pdf,
                UnderlaySource::Dxf(_) => UnderlayFormat::Dxf,
            };
            keep_source(&sha, &format, &kept).map_err(DxfError::Cache)?;
        }
        let mode = ew_import_dxf::ew_interop_api::ImportMode::Lenient;
        let command =
            crate::import::apply_command(&batch, None, mode, None, false).map_err(DxfError::Apply)?;
        let preview = self.import_preview(&command).map_err(DxfError::Apply)?;
        self.commit_import(preview).map_err(DxfError::Apply)?;
        Ok(batch.report)
    }
}

/// Keeps a source the user did not embed in the underlay cache (written whole: a temporary file renamed into place).
///
/// # Errors
///
/// The refusal in words when the cache directory is unknown or cannot be written.
pub(crate) fn keep_source(sha: &[u8; 32], format: &UnderlayFormat, bytes: &[u8]) -> Result<(), String> {
    let refused =
        || "The underlay's file could not be kept in the underlay cache; embed it instead.".to_owned();
    let dir = underlay_cache_dir().ok_or_else(refused)?;
    std::fs::create_dir_all(&dir).map_err(|_| refused())?;
    let path = dir.join(format!("{}.{}", hex(sha), ext(format)));
    if path.is_file() {
        return Ok(());
    }
    let tmp = dir.join(format!("{}.{}.tmp-{}", hex(sha), ext(format), std::process::id()));
    std::fs::write(&tmp, bytes)
        .and_then(|()| std::fs::rename(&tmp, &path))
        .map_err(|_| {
            let _ = std::fs::remove_file(&tmp);
            refused()
        })
}
