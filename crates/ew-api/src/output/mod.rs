// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page output: the export job of the session (doc 09 §4; ARCH §6.4, §8.4).
//!
//! **The view.** `view.rs` owns the glue rules that fill `ew_scene::page::PageView` from `Q`, one query
//! family per part of the view. **The layers.** `layers.rs` builds one
//! memoised `page_display_list` family per layer over the parts that layer reads, so a wire edit never rebuilds
//! the title block (the memo oracle and `tests/page_output_memo.rs` hold it).
//!
//! **The job.** [`export_pages_with`] (and the [`Session`] methods [`Session::export_pdf`], [`Session::export_svg`])
//! runs in three phases:
//!
//! 1. on the session's thread, through its memoising `Q`: `page_order`, the page selection, and for every
//!    selected page its memoised layers assembled into a page scene ([`page_scene`]);
//! 2. the page phase on [`ew_export::par_map`] (scoped `std` threads, results by page index):
//!    each page's layers mapped onto its sheet with their symbol instances inlined, the one list every writer draws,
//!    and its image items resolved from the project's asset bytes ([`ExportAssets`]: an SVG asset becomes
//!    vector paths through `ew_scene::images`, a PNG or JPEG image gets its bytes); this phase is not a memoised
//!    query, so reading which assets the project holds here is outside the read rule;
//! 3. the writer: [`ew_export::pdf`] (PDF/A-2b, outline, `/PageLabels`, named destinations, links, subset fonts)
//!    or [`ew_export::svg`] (one `<label>.svg` per page), with the bundled fonts only ([`font_bytes`]).
//!
//! Serial and parallel runs give the same bytes (AC-14). The `Session` is not `Send`, so phase 1 of
//! [`Session::export_pdf`] stays on its thread: the layers are memoised in the session's store and a second export
//! after an edit rebuilds only the layers the edit touched.
//!
//! **The export as a job** ([`ExportJob`], `job.rs`). [`Session::export_job`] takes a snapshot
//! (the `World`, the report engine, the asset bytes); [`ExportJob::run`] builds a cold `Q` on the job's own thread and
//! runs the same three phases with a progress report before each page, a checkpoint after each page (`page`), a
//! cancel check before each page of the page phase (`draw`), and checkpoints after the page phase (`drawn`) and the
//! writer (`written`). The phases are one function for both ([`export_pages_with`] runs it without the job's controller), so the job's
//! bytes are the synchronous export's on the same revision. No pool and no memo snapshot in the job.
//!
//! **Metadata** ([`pdf_options`], doc 09 §4.3). Title: the project description (neutral). Creator: `ElektraWatt
//! <version>`. Date (FR-OUT-026): the caller's (`SOURCE_DATE_EPOCH` in the CLI), else the model's edited date, else
//! its created date, else 1980-01-01; never the wall clock. Document ID: krilla's hash of the text
//! `ew-pdf/1 <project id> <derivation profile> <date> <pages>[ mono] ew <version> sha256:<digest>` (doc 09 §4.3
//! rule 2: a hash of the export input), `<pages>` being `all` or the selected folio labels joined by `,` and
//! `<digest>` the [`content_digest`] of the pages and the bundled fonts.
//!
//! **The print set** ([`mod@print`]): the same pages placed on a paper ([`ew_export::PagePlacement`]),
//! stamped when scaled, and written by the same writer; [`PageSelection::Sections`] selects pages by section for both.

mod editing;
mod fields;
// The export as a job (D4).
mod job;
// The devices a placed symbol can join (D1, D2).
pub(crate) mod join;
mod layers;
// Properties for layout records and the Surface view rows.
mod layout_edit;
mod navigation;
// A potential reasoned for an unnamed wire (FT-1700).
pub(crate) mod navigators;
mod potential_offer;
// The surface view of a panel-layout page (D9, D10).
pub(crate) mod panel;
// The labelled entity list of a preview (§4.2).
pub(crate) mod preview;
// The print set (D5 to D7).
pub mod print;
// `ew diff` and its textconv (D26, D27)
mod project_diff;
pub(crate) mod properties;
// Page rasters and the row exports: their XLSX and CSV presets.
mod raster;
mod rows;
// Page digests and the page change status (D6, D7).
mod revisions;
// A page's geometry for the router and the layout measures of `ew layout`.
mod route_measures;
// The scale a panel page is fitted at.
mod scale_fit;
// The strip editor's gestures (§4.2).
mod strip_edit;
// Table mode's pure rule, moved down from the view model.
pub mod table_edit;
// The table round trip.
mod table_io;
// What the window's Title block fields form reads of a page (FT-1686).
mod title_block_fields;
mod view;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

use ew_derive::{PageEntry, PageOrder, PageOrderV, PageTitle, Q, TableView};
use ew_dl::FontKey;
use ew_export::ew_render_cpu::FontBytes;
use ew_export::{PdfError, PdfOptions, PdfPage, SvgError, SvgFace, SvgOptions, par_map};
use ew_scene::page::PageFinding;
use ew_schema::Section;
use sha2::{Digest, Sha256};

pub use editing::{
    CatalogueSymbol, JunctionWire, PageTables, PlaceCatalogue, PlacedBy, SymbolShape, TableConnection,
    TableCp, TableEnd, TableItem, TableJunction, TablePotential, TableRep, TableStructureBox, exchange_map,
    page_type_takes,
};
/// The display-list IR the pages of an export are made of ([`ExportPages`]' `content`), re-exported so that the
/// shell names its types (`DisplayList`, `EntityRef`, `Rect`) without a workspace edge to `ew-dl`
/// (architecture §3 rule 7).
pub use ew_dl;
pub use ew_export::{DxfFile, DxfLayers, Parallelism, PdfDate, PdfReport, SvgFile, svg_file_name};
pub use fields::{
    Category, EditRoute, FieldReadOnly, FieldType, InputError, ParsedInput, field_catalogue,
    field_catalogue_for, format_field_value,
};
pub use job::{ExportJob, PrintSetJob};
pub use layers::{
    CarriersLayer, FrameLayer, LabelsLayer, PageLayer, PlacementsLayer, SurfaceLayer, SymbolsLayer,
    TablesLayer, WiresLayer, new_project_text_style_table, page_kind, page_layer, page_scene, page_view,
    text_style_table,
};
pub use layout_edit::{LayoutEdit, LayoutEditError, parse_mm};
pub use navigation::{CellLabel, GotoError, LinkTarget, PageCells, PageCp, PageListTable, PageRow};
pub use scale_fit::fitting_scale;
// The panel tables and the pure items of `ew_derive::layout` the layout tool runs, so that
// `ew-viewmodel` keeps its one dependency.
pub use ew_derive::layout::{
    Fit, Profile, RailExtent, RailItem, Resolution, SnapError, SnapMode, brackets, end_gap, fits_rail,
    min_gap, resolve, start_gap,
};
/// The round trip's reader: a workbook or CSV read back as header and cell texts.
pub use ew_import_catalog::sheet;
/// The table writers of `ew-reports` the row exports name: the CSV presets and their options, the
/// table model, the workbook's metadata and writer.
pub use ew_reports::csv::{CsvOptions, LineEnd, Preset, restyle, table_csv};
pub use ew_reports::table::{Table, TableCell, TableColumn, TableRow};
pub use ew_reports::xlsx::{WorkbookMeta, base_text, xlsx};
pub use navigators::{
    Drawable, HighlightTarget, LibraryCopyDetail, MacroDetail, MacroThumbnail, NaturalKey, NavColumn,
    NavKind, NavNode, NavRole, NavTable, Uuid, names_subject, nav_natural_key, tokens as nav_tokens,
};
pub use panel::{PanelAxis, PanelCarrier, PanelPlacement, PanelTables, UnplacedDevice, surface_of_page};
pub use potential_offer::PotentialOffer;
pub(crate) use potential_offer::said_type;
pub use preview::PreviewEntity;
pub use project_diff::textconv;
pub use properties::{
    CompanyColour, FindingChip, PropRow, PropertySheet, Provenance, RowAction, RowValue, SelectionWords,
    SheetHeader, SheetSection, SourceLink, WireColours, WireEnd,
};
pub use raster::{DEFAULT_DPI, DPI_RANGE, page_png};
pub use revisions::{PageDigest, changed_now, changed_pages, last_changed_in, page_digest, page_digests};
pub use route_measures::{
    LayoutBends, LayoutCrossing, LayoutError, LayoutJunction, LayoutPair, LayoutThrough, LayoutTouch,
    PageLayout,
};
pub use rows::{
    RowExport, RowsCsv, RowsError, RowsFormat, RowsOutput, block_kind, find_block, report_folios,
    report_kinds, row_export, row_output, sql_table, with_base_digest, workbook_dump,
};
// The table round trip.
pub use strip_edit::{
    ACCESSORY_KINDS, BridgeRow, StripCell, StripEdit, StripEditError, accessory_kind_word,
    accessory_part_kinds,
};
pub use table_io::{
    CellOutcome, ImportCell, ImportTableOptions, TABLE_NAVIGATORS, TableApplyError, TableError, TableImport,
    TableImportError, guard_table_read,
};
pub use title_block_fields::TitleBlockFacts;
pub use view::{
    FrameForms, NOGRID_FRAME, PageConnectionsOf, PageEmbeddedOf, PageFrameOf, PageSlotsOf, PageTableOf,
    page_destination, page_items, page_uuid, page_valve_glyphs,
};

pub(crate) use panel::{PageSurfaceView, ShapedText};
pub(crate) use properties::folio_of;
pub(crate) use view::{CpTargetsByPage, PageBindingsOf, PageCpTargetsOf, PageGridOf, PageRepsOf};

use crate::Session;

/// Why an export did not complete.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum OutputError {
    /// The page selection names a folio that is not in the page order, a range that runs backwards, or nothing.
    Selection(String),
    /// The PDF writer refused the pages.
    Pdf(PdfError),
    /// The SVG writer refused the pages.
    Svg(SvgError),
    /// A page could not be rasterised (a sheet without size at the resolution, a font that is not bundled).
    Raster(String),
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::Selection(m) => write!(f, "page selection: {m}"),
            OutputError::Pdf(e) => write!(f, "PDF: {e}"),
            OutputError::Svg(e) => write!(f, "SVG: {e}"),
            OutputError::Raster(m) => write!(f, "PNG: {m}"),
        }
    }
}

impl std::error::Error for OutputError {}

/// Which pages of the page order an export writes (`--pages`, `--sections`).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum PageSelection {
    /// Every page of the page order.
    #[default]
    All,
    /// The listed items in page order, each page once: an item is a folio label ("101", "2.1") or a range
    /// `<first>-<last>` of two folio labels, every page from the first to the last in page order.
    Folios(Vec<String>),
    /// The pages of each named section (by its number, `1`) in page order, its divider included, each page once.
    Sections(Vec<String>),
}

impl PageSelection {
    /// The selection of a `--pages` text: items separated by `,` (see [`PageSelection::Folios`]).
    ///
    /// # Errors
    ///
    /// [`OutputError::Selection`] for an empty text or an empty item.
    pub fn parse(spec: &str) -> Result<PageSelection, OutputError> {
        let items: Vec<String> = spec.split(',').map(|s| s.trim().to_owned()).collect();
        if items.iter().any(String::is_empty) {
            return Err(OutputError::Selection(format!(
                "`{spec}` has an empty item (folio labels and ranges separated by `,`)"
            )));
        }
        Ok(PageSelection::Folios(items))
    }

    /// The selection of a `--sections` text: section numbers separated by `,` (see [`PageSelection::Sections`]).
    ///
    /// # Errors
    ///
    /// [`OutputError::Selection`] for an empty text or an empty item.
    pub fn parse_sections(spec: &str) -> Result<PageSelection, OutputError> {
        let items: Vec<String> = spec.split(',').map(|s| s.trim().to_owned()).collect();
        if items.iter().any(String::is_empty) {
            return Err(OutputError::Selection(format!(
                "`{spec}` has an empty item (section numbers separated by `,`)"
            )));
        }
        Ok(PageSelection::Sections(items))
    }

    /// The positions in `order` the selection names, ascending and unique; `sections` are the project's sections
    /// ([`Q::sections`]), which [`PageSelection::Sections`] names by number.
    ///
    /// # Errors
    ///
    /// [`OutputError::Selection`] when an item names no folio, a range runs backwards or is ambiguous, or a section
    /// number names no section.
    pub fn resolve(
        &self,
        order: &PageOrderV,
        sections: TableView<'_, Section>,
    ) -> Result<Vec<usize>, OutputError> {
        match self {
            PageSelection::All => Ok((0..order.entries.len()).collect()),
            PageSelection::Folios(items) => resolve_folios(order, items),
            PageSelection::Sections(numbers) => resolve_sections(order, sections, numbers),
        }
    }
}

/// The positions of the pages of the named sections, in page order: refused with "no section `<n>` (the sections
/// are <numbers>)" for a number no section has.
fn resolve_sections(
    order: &PageOrderV,
    sections: TableView<'_, Section>,
    numbers: &[String],
) -> Result<Vec<usize>, OutputError> {
    let all: Vec<(ew_derive::ew_ids::Id<Section>, u16)> =
        sections.iter().map(|(id, s)| (id, s.number)).collect();
    let mut wanted = std::collections::BTreeSet::new();
    for n in numbers {
        let ids: Vec<ew_derive::ew_ids::Id<Section>> = all
            .iter()
            .filter(|(_, number)| number.to_string() == *n)
            .map(|(id, _)| *id)
            .collect();
        if ids.is_empty() {
            let mut known: Vec<u16> = all.iter().map(|(_, number)| *number).collect();
            known.sort_unstable();
            known.dedup();
            let known: Vec<String> = known.iter().map(u16::to_string).collect();
            return Err(OutputError::Selection(if known.is_empty() {
                format!("no section `{n}` (the project has no sections)")
            } else {
                format!("no section `{n}` (the sections are {})", known.join(", "))
            }));
        }
        wanted.extend(ids);
    }
    Ok(order
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.section.is_some_and(|s| wanted.contains(&s)))
        .map(|(i, _)| i)
        .collect())
}

/// The positions of [`PageSelection::Folios`]' items, ascending and unique.
fn resolve_folios(order: &PageOrderV, items: &[String]) -> Result<Vec<usize>, OutputError> {
    let at = |label: &str| order.entries.iter().position(|e| e.folio == label);
    let mut picked = std::collections::BTreeSet::new();
    for item in items {
        if let Some(i) = at(item) {
            picked.insert(i);
            continue;
        }
        // A range: the one split at a `-` whose both sides are folio labels (labels may contain `-`).
        let splits: Vec<(usize, usize)> = item
            .match_indices('-')
            .filter_map(|(k, _)| Some((at(&item[..k])?, at(&item[k + 1..])?)))
            .collect();
        match splits.as_slice() {
            [(a, b)] if a <= b => picked.extend(*a..=*b),
            [(_, _)] => {
                return Err(OutputError::Selection(format!(
                    "the range `{item}` runs backwards in the page order"
                )));
            }
            [] => {
                return Err(OutputError::Selection(format!(
                    "`{item}` is no folio label and no range of two folio labels"
                )));
            }
            _ => return Err(OutputError::Selection(format!("the range `{item}` is ambiguous"))),
        }
    }
    Ok(picked.into_iter().collect())
}

/// What an export writes, beyond the pages.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ExportOptions {
    /// The pages.
    pub pages: PageSelection,
    /// The threads of the page phase; changes speed only, never bytes.
    pub parallelism: Parallelism,
    /// Draw in black and grey (FR-OUT-011).
    pub monochrome: bool,
    /// The creation date (the release context, a scheme date or `SOURCE_DATE_EPOCH`); `None`: the model's.
    pub date: Option<PdfDate>,
}

/// The asset bytes an export draws its image items with, by SHA-256: a snapshot of the project's asset
/// store, taken by the [`Session`] export methods. A [`Q`] holds no asset bytes: the free functions
/// [`export_pages_with`], [`export_pdf_with`] and [`export_svg_with`] draw no image with [`ExportAssets::default`].
#[derive(Clone, Default)]
pub struct ExportAssets {
    bytes: BTreeMap<[u8; 32], Arc<[u8]>>,
}

impl ExportAssets {
    /// The bytes of every asset the model holds.
    pub fn of(model: &ew_model::Model) -> ExportAssets {
        ExportAssets {
            bytes: model
                .assets()
                .map(|a| (a.sha256().0, Arc::clone(a.bytes())))
                .collect(),
        }
    }

    /// The bytes of the asset with this hash, if held.
    pub fn get(&self, sha256: &[u8; 32]) -> Option<Arc<[u8]>> {
        self.bytes.get(sha256).cloned()
    }
}

impl fmt::Debug for ExportAssets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExportAssets")
            .field("assets", &self.bytes.len())
            .finish()
    }
}

/// A page finding with the folio it was found on.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageFindingAt {
    /// The folio label.
    pub folio: String,
    /// The finding.
    pub finding: PageFinding,
}

/// What `findings` are, for a command line: "15 page findings: 15 empty fields in a frame or title block", the kinds
/// in the order they first occur; `None` for no findings.
pub fn page_findings_summary(findings: &[PageFindingAt]) -> Option<String> {
    if findings.is_empty() {
        return None;
    }
    use ew_scene::forms::FormFinding as F;
    use ew_scene::table::TableFinding as T;
    let mut kinds: Vec<((&str, &str), usize)> = Vec::new();
    for f in findings {
        let kind = match &f.finding {
            PageFinding::Form(F::Binding {
                finding: ew_scene::forms::BindingFinding::UnsupportedFilter { .. },
                ..
            }) => (
                "field with a filter this version does not apply",
                "fields with a filter this version does not apply",
            ),
            PageFinding::Form(F::Binding { .. }) => (
                "empty field in a frame or title block",
                "empty fields in a frame or title block",
            ),
            PageFinding::Form(F::TitleBlockMissing) => ("missing title block", "missing title blocks"),
            PageFinding::Form(F::UnknownAlign { .. }) => {
                ("unknown text alignment", "unknown text alignments")
            }
            PageFinding::Form(F::UnknownStyle { .. })
            | PageFinding::UnknownStyle { .. }
            | PageFinding::UnknownSlotStyle { .. } => ("unknown text style", "unknown text styles"),
            PageFinding::Form(F::MissingGlyphs { .. })
            | PageFinding::MissingGlyphs { .. }
            | PageFinding::Table(T::MissingGlyphs { .. }) => (
                "text with characters the fonts lack",
                "texts with characters the fonts lack",
            ),
            PageFinding::Form(F::Overflow { .. }) | PageFinding::Table(T::Overflow { .. }) => {
                ("text too long for its cell", "texts too long for their cells")
            }
            PageFinding::Table(T::TooManyRows { .. }) => {
                ("table with more rows than fit", "tables with more rows than fit")
            }
            PageFinding::MissingSymbol { .. } => (
                "symbol not drawn for want of its copy",
                "symbols not drawn for want of their copy",
            ),
            PageFinding::UnresolvedEnd { .. } => ("unresolved wire end", "unresolved wire ends"),
            PageFinding::ConflictingLinks { .. } => {
                ("text with conflicting links", "texts with conflicting links")
            }
            PageFinding::Image(_) => ("image not drawn", "images not drawn"),
        };
        match kinds.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, n)) => *n += 1,
            None => kinds.push((kind, 1)),
        }
    }
    let what: Vec<String> = kinds
        .iter()
        .map(|((one, many), n)| format!("{n} {}", if *n == 1 { one } else { many }))
        .collect();
    let noun = if findings.len() == 1 {
        "finding"
    } else {
        "findings"
    };
    Some(format!("{} page {noun}: {}", findings.len(), what.join(", ")))
}

/// The pages of an export, ready for a writer.
#[derive(Clone, PartialEq, Debug)]
pub struct ExportPages {
    /// One sheet display list per selected page, in page order.
    pub pages: Vec<PdfPage>,
    /// The PDF options of this export ([`pdf_options`]).
    pub options: PdfOptions,
    /// The findings of the page scenes, in page order.
    pub findings: Vec<PageFindingAt>,
    /// The drawing pages and the section divider pages among them; the rest are report pages.
    pub kinds: PageKinds,
}

/// How many pages of an export are drawing pages and section divider pages; the rest are generated report pages
/// (the export toast names its pages by kind).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PageKinds {
    /// The authored pages.
    pub drawing: usize,
    /// The divider pages of sections.
    pub dividers: usize,
}

/// A written PDF.
#[derive(Clone, PartialEq, Debug)]
pub struct PdfExport {
    /// The file.
    pub bytes: Vec<u8>,
    /// The writer's report.
    pub report: PdfReport,
    /// The number of pages.
    pub pages: usize,
    /// The findings of the page scenes.
    pub findings: Vec<PageFindingAt>,
    /// The drawing pages and the section divider pages among them.
    pub kinds: PageKinds,
}

/// A written SVG set.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SvgExport {
    /// One file per page, in page order.
    pub files: Vec<SvgFile>,
    /// The writer's report.
    pub report: ew_export::SvgReport,
    /// The findings of the page scenes.
    pub findings: Vec<PageFindingAt>,
}

/// A written DXF set (doc 09 §4.4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DxfExport {
    /// One file per page, in page order.
    pub files: Vec<ew_export::DxfFile>,
    /// The findings of the page scenes.
    pub findings: Vec<PageFindingAt>,
}

/// The DXF set of an export over `q` (doc 09 §4.4, FR-OUT-015): one AC1015 file per selected page from its exported
/// layers on the sheet, symbol instances kept for the writer's blocks, the layers named by `layers`.
///
/// # Errors
///
/// [`OutputError::Selection`] for a selection the page order does not have, or two folios of one file name.
pub fn export_dxf_with(
    q: &Q,
    options: &ExportOptions,
    layers: &ew_export::DxfLayers,
) -> Result<DxfExport, OutputError> {
    let order = q.get::<PageOrder>(());
    let picked = options.pages.resolve(&order, q.sections())?;
    if picked.is_empty() {
        return Err(OutputError::Selection("the export has no pages".to_owned()));
    }
    raster::refuse_same_file_names(
        picked.iter().map(|&i| order.entries[i].folio.as_str()),
        ew_export::dxf_file_name,
    )?;
    let mut sheets = Vec::with_capacity(picked.len());
    let mut findings = Vec::new();
    for &i in &picked {
        let entry = &order.entries[i];
        let scene = page_scene(q, entry);
        findings.extend(scene.findings.iter().map(|f| PageFindingAt {
            folio: entry.folio.clone(),
            finding: f.clone(),
        }));
        sheets.push(ew_export::DxfSheet {
            label: entry.folio.clone(),
            width: scene.sheet_width,
            height: scene.sheet_height,
            layers: scene
                .layers
                .iter()
                .filter(|(l, _)| l.is_exported())
                .map(|(l, list)| (*l, scene.map.apply_display_list(list, None)))
                .collect(),
            geometry: scene.geometry.clone(),
            scale_ppb: scene.map.scale_ppb,
        });
    }
    let dxf_options = ew_export::DxfOptions {
        layers: layers.clone(),
        faces: svg_options(false)
            .faces
            .into_iter()
            .map(|(k, f)| (k, (f.family, f.weight >= 700)))
            .collect(),
    };
    Ok(DxfExport {
        files: ew_export::dxf(&sheets, &dxf_options),
        findings,
    })
}

/// The bundled font faces as the writers read them (`ew_text::bundled()`): the export embeds no other font.
pub fn font_bytes() -> &'static BTreeMap<FontKey, FontBytes> {
    static FONTS: OnceLock<BTreeMap<FontKey, FontBytes>> = OnceLock::new();
    FONTS.get_or_init(|| {
        let set = ew_text::bundled();
        set.faces()
            .filter_map(|f| {
                let data = set.face_data(f.key)?;
                Some((
                    f.key,
                    FontBytes {
                        data: Arc::from(data),
                        index: u32::from(f.key.face_index),
                    },
                ))
            })
            .collect()
    })
}

/// The SVG options of an export: the bundled faces by family, weight and style, each with its font file
/// ([`font_bytes`]) so that every page embeds the subsets of the faces it draws.
pub fn svg_options(monochrome: bool) -> SvgOptions {
    let bytes = font_bytes();
    SvgOptions {
        monochrome,
        faces: ew_text::bundled()
            .faces()
            .map(|f| {
                (
                    f.key,
                    SvgFace {
                        family: f.family_name.clone(),
                        weight: f.weight.value(),
                        italic: f.italic,
                        // A face of a collection file is named only: the subsetter takes single-face files.
                        data: bytes
                            .get(&f.key)
                            .filter(|b| b.index == 0)
                            .map(|b| Arc::clone(&b.data)),
                    },
                )
            })
            .collect(),
    }
}

/// The date of `SOURCE_DATE_EPOCH` seconds (UTC); `None` before 0001-01-01 or after 9999-12-31.
pub fn date_from_epoch(seconds: i64) -> Option<PdfDate> {
    let days = seconds.div_euclid(86_400);
    let secs = seconds.rem_euclid(86_400);
    // Civil from days (H. Hinnant's algorithm), proleptic Gregorian.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    Some(PdfDate {
        year: u16::try_from(year).ok().filter(|y| (1..=9999).contains(y))?,
        month: u8::try_from(month).ok()?,
        day: u8::try_from(day).ok()?,
        hour: u8::try_from(secs / 3600).ok()?,
        minute: u8::try_from(secs % 3600 / 60).ok()?,
        second: u8::try_from(secs % 60).ok()?,
    })
}

/// The model's date (FR-OUT-026): edited, else created, else 1980-01-01.
fn model_date(q: &Q) -> PdfDate {
    let m = &q.project().master;
    m.edited.or(m.created).map_or(PdfDate::ymd(1980, 1, 1), |d| {
        PdfDate::ymd(d.year(), d.month(), d.day())
    })
}

/// A [`std::hash::Hasher`] that feeds SHA-256: the `Hash` impls of the display-list types write their fields in
/// a fixed order (integers in the target's byte order, lengths as `usize`), so the digest is the same for the same
/// pages on every 64-bit little-endian target, whatever the record order or the thread count.
struct Sha256Hasher(Sha256);

impl std::hash::Hasher for Sha256Hasher {
    fn write(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    fn finish(&self) -> u64 {
        0
    }
}

/// The content digest of an export (doc 09 §4.3 rule 2, the hash of the export input): SHA-256 over every page
/// (label, outline title and section, sheet size, destination, sheet display list) and the pinned SHA-256 of
/// every bundled font. The pages are derived values, so the digest is the same however the model was loaded
/// (bootstrapped tables, the saved project, a shuffled record order).
pub fn content_digest(pages: &[PdfPage]) -> String {
    use std::hash::Hash;
    let mut h = Sha256Hasher(Sha256::new());
    pages.len().hash(&mut h);
    for p in pages {
        p.label.hash(&mut h);
        p.title.hash(&mut h);
        p.section.hash(&mut h);
        p.width.hash(&mut h);
        p.height.hash(&mut h);
        p.destination.hash(&mut h);
        p.content.hash(&mut h);
    }
    for font in [
        ew_text::NOTO_SANS_REGULAR_SHA256,
        ew_text::NOTO_SANS_BOLD_SHA256,
        ew_text::NOTO_SANS_MATH_REGULAR_SHA256,
    ] {
        h.0.update(font);
    }
    format!("{:x}", h.0.finalize())
}

/// The PDF options of an export of `pages` (`labels`: the selected folio labels; `None`: every page) of the model
/// behind `q` (see the module documentation).
pub fn pdf_options(
    q: &Q,
    options: &ExportOptions,
    labels: Option<&[String]>,
    pages: &[PdfPage],
) -> PdfOptions {
    let date = options.date.unwrap_or_else(|| model_date(q));
    let selection = labels.map_or_else(|| "all".to_owned(), |l| l.join(","));
    let mono = if options.monochrome { " mono" } else { "" };
    PdfOptions {
        title: Some(view::neutral(&q.project().master.description)),
        creator: Some(format!("ElektraWatt {}", env!("CARGO_PKG_VERSION"))),
        monochrome: options.monochrome,
        document_id: format!(
            "ew-pdf/1 {} {} {:04}-{:02}-{:02} {selection}{mono} ew {} sha256:{}",
            q.project().id.as_uuid().hyphenated(),
            q.profile().version,
            date.year,
            date.month,
            date.day,
            env!("CARGO_PKG_VERSION"),
            content_digest(pages)
        ),
        ..PdfOptions::new(date)
    }
}

/// The outline title of a page: "101 Power Supply 1 - Main Current".
pub fn outline_title(q: &Q, entry: &PageEntry) -> String {
    format!(
        "{} {}",
        entry.folio,
        view::neutral(&q.get::<PageTitle>(entry.page))
    )
}

/// The outline section of a page: "Section 1 - Wiring Definition"; `None` for front matter.
pub fn section_title(q: &Q, entry: &PageEntry) -> Option<String> {
    let s = q.sections().get(entry.section?)?;
    Some(format!("Section {} - {}", s.number, view::neutral(&s.title)))
}

/// One selected page after phase 1.
struct Phase1 {
    entry: PageEntry,
    title: String,
    section: Option<String>,
    scene: ew_scene::page::PageScene,
}

/// Phases 1 and 2 of the export job (see the module documentation): the sheet display lists of the selected
/// pages, the PDF options and the page findings, with the asset bytes its image items are drawn with
/// ([`ExportAssets`]).
///
/// # Errors
///
/// [`OutputError::Selection`] when the selection names a folio the page order does not have.
pub fn export_pages_with(
    q: &Q,
    options: &ExportOptions,
    assets: &ExportAssets,
) -> Result<ExportPages, OutputError> {
    match export_pages_hooked(q, options, assets, None) {
        Ok(pages) => Ok(pages),
        Err(Halt::Output(e)) => Err(e),
        // Without the job's controller nothing cancels.
        Err(Halt::Cancelled) => Err(OutputError::Selection("the export was cancelled".to_owned())),
    }
}

/// Why a hooked export run stopped.
pub(crate) enum Halt {
    /// The export failed.
    Output(OutputError),
    /// A checkpoint met the cancel flag.
    Cancelled,
}

impl From<OutputError> for Halt {
    fn from(e: OutputError) -> Halt {
        Halt::Output(e)
    }
}

impl From<ew_jobs::Cancelled> for Halt {
    fn from(_: ew_jobs::Cancelled) -> Halt {
        Halt::Cancelled
    }
}

/// Phases 1 and 2 of the export with the job's controller `ctl` (§6.2): before page i of n of the selection
/// `report(i, n, "building")`, after its scene `checkpoint("page")`; in the page phase each page first checks the
/// cancel flag (`checkpoint("draw")`) and is skipped when cancelled; after the phase `checkpoint("drawn")` and
/// `report(n, n, "drawing")`. Without `ctl` (`None`) this is [`export_pages_with`] exactly.
pub(crate) fn export_pages_hooked(
    q: &Q,
    options: &ExportOptions,
    assets: &ExportAssets,
    ctl: Option<&ew_jobs::JobCtl>,
) -> Result<ExportPages, Halt> {
    let order = q.get::<PageOrder>(());
    let picked = options.pages.resolve(&order, q.sections())?;
    if picked.is_empty() {
        return Err(OutputError::Selection("the export has no pages".to_owned()).into());
    }
    let total = u32::try_from(picked.len()).unwrap_or(u32::MAX);
    // Phase 1, on this thread through `q`: the memoised layers of every selected page.
    let mut phase1: Vec<Phase1> = Vec::with_capacity(picked.len());
    for (done, &i) in (0u32..).zip(picked.iter()) {
        if let Some(ctl) = ctl {
            ctl.report(ew_jobs::Progress {
                done,
                total,
                phase: "building",
            });
        }
        let entry = order.entries[i].clone();
        phase1.push(Phase1 {
            title: outline_title(q, &entry),
            section: section_title(q, &entry),
            scene: page_scene(q, &entry),
            entry,
        });
        if let Some(ctl) = ctl {
            ctl.checkpoint("page")?;
        }
    }
    let kinds = PageKinds {
        drawing: (phase1.iter())
            .filter(|p| matches!(p.entry.page, ew_derive::PageRef::Drawn(_)))
            .count(),
        dividers: (phase1.iter())
            .filter(|p| matches!(p.entry.page, ew_derive::PageRef::Divider(_)))
            .count(),
    };
    let mut findings: Vec<PageFindingAt> = phase1
        .iter()
        .flat_map(|p| {
            p.scene.findings.iter().map(|f| PageFindingAt {
                folio: p.entry.folio.clone(),
                finding: f.clone(),
            })
        })
        .collect();
    // Phase 2, the page phase: every sheet list on its own, collected by page index; a page whose turn comes after a
    // cancel is skipped.
    let resolved = par_map(&phase1, options.parallelism, |_, p| {
        if let Some(ctl) = ctl {
            ctl.checkpoint("draw").ok()?;
        }
        let (content, report) =
            ew_scene::images::resolve_images(&p.scene.sheet_list(), &|sha| assets.get(sha));
        let page = PdfPage {
            destination: page_destination(p.entry.page),
            label: p.entry.folio.clone(),
            title: p.title.clone(),
            section: p.section.clone(),
            width: p.scene.sheet_width,
            height: p.scene.sheet_height,
            content,
            placement: None,
        };
        Some((page, report))
    });
    if let Some(ctl) = ctl {
        ctl.checkpoint("drawn")?;
        ctl.report(ew_jobs::Progress {
            done: total,
            total,
            phase: "drawing",
        });
    }
    // A skipped page means a cancel, which the checkpoint above has seen already; this keeps the types honest.
    let resolved = resolved
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or(Halt::Cancelled)?;
    // The images that could not be drawn (bytes not held, a refused SVG, PNG or JPEG bytes that do not decode) are page
    // findings, after the scene's: no silent loss. The raster check uses the renderers' own
    // decoder, once per asset of the export.
    let mut decodes: BTreeMap<[u8; 32], bool> = BTreeMap::new();
    let mut pages = Vec::with_capacity(resolved.len());
    for (mut page, report) in resolved {
        let mut issues = report.issues;
        undecodable_rasters(&mut page.content.items, &mut decodes, &mut issues);
        findings.extend(issues.into_iter().map(|issue| PageFindingAt {
            folio: page.label.clone(),
            finding: PageFinding::Image(issue),
        }));
        pages.push(page);
    }
    let labels: Option<Vec<String>> = match options.pages {
        PageSelection::All => None,
        PageSelection::Folios(_) | PageSelection::Sections(_) => {
            Some(pages.iter().map(|p| p.label.clone()).collect())
        }
    };
    Ok(ExportPages {
        options: pdf_options(q, options, labels.as_deref(), &pages),
        pages,
        findings,
        kinds,
    })
}

/// Adds an [`ImageProblem::Undecodable`](ew_scene::images::ImageProblem::Undecodable) issue for every PNG or JPEG item
/// of `items` (groups included, in list order) whose bytes are held but do not decode with the renderers' decoder
/// ([`ew_export::ew_render_cpu::decode_image`]), and withdraws that item's bytes, so that no writer embeds them (the
/// SVG writer would otherwise copy them unchanged) and none decodes them again; `decodes` caches the verdict per asset
/// hash, once per export.
fn undecodable_rasters(
    items: &mut [ew_dl::Item],
    decodes: &mut BTreeMap<[u8; 32], bool>,
    issues: &mut Vec<ew_scene::images::ImageIssue>,
) {
    for item in items {
        match &mut item.kind {
            ew_dl::ItemKind::Group(g) => undecodable_rasters(&mut g.items, decodes, issues),
            ew_dl::ItemKind::Image(img)
                if !img.image.bytes.is_empty() && img.image.format != ew_dl::ImageFormat::Svg =>
            {
                let ok = *decodes.entry(img.image.sha256).or_insert_with(|| {
                    ew_export::ew_render_cpu::decode_image(img.image.format, &img.image.bytes).is_some()
                });
                if !ok {
                    issues.push(ew_scene::images::ImageIssue {
                        sha256: img.image.sha256,
                        problem: ew_scene::images::ImageProblem::Undecodable(img.image.format),
                    });
                    img.image = Arc::new(ew_dl::ImageData {
                        sha256: img.image.sha256,
                        format: img.image.format,
                        bytes: Arc::from(&[][..]),
                    });
                }
            }
            _ => {}
        }
    }
}

/// The PDF of an export over `q` (phase 3: [`ew_export::pdf`] with the bundled fonts), its image items drawn with the
/// asset bytes `assets`.
///
/// # Errors
///
/// [`OutputError`] for a selection the page order does not have or pages the writer refuses.
pub fn export_pdf_with(
    q: &Q,
    options: &ExportOptions,
    assets: &ExportAssets,
) -> Result<PdfExport, OutputError> {
    let pages = export_pages_with(q, options, assets)?;
    let out = ew_export::pdf(&pages.pages, &pages.options, font_bytes(), None).map_err(OutputError::Pdf)?;
    Ok(PdfExport {
        bytes: out.bytes,
        report: out.report,
        pages: pages.pages.len(),
        findings: pages.findings,
        kinds: pages.kinds,
    })
}

/// The SVG set of an export over `q` (phase 3: [`ew_export::svg`], its page phase on the same parallelism), its image
/// items drawn with the asset bytes `assets`.
///
/// # Errors
///
/// [`OutputError`] for a selection the page order does not have or pages the writer refuses.
pub fn export_svg_with(
    q: &Q,
    options: &ExportOptions,
    assets: &ExportAssets,
) -> Result<SvgExport, OutputError> {
    let pages = export_pages_with(q, options, assets)?;
    raster::refuse_same_file_names(pages.pages.iter().map(|p| p.label.as_str()), svg_file_name)?;
    let out = ew_export::svg(
        &pages.pages,
        &svg_options(options.monochrome),
        None,
        options.parallelism,
    )
    .map_err(OutputError::Svg)?;
    Ok(SvgExport {
        files: out.files,
        report: out.report,
        findings: pages.findings,
    })
}

/// The connection grid of a page (`Page.grid`, ADR-0005, ADR-0027) as a rational pitch, with the page-to-sheet map
/// of the page's frame form (FR-PAG-029), so that a viewer draws the grid points where the page's connection points
/// print on the sheet: grid point `(i, j)` of the page lies at `offset + scale × (i, j) × num_nm / den` on the
/// sheet, `scale = sheet_scale_ppb / 10⁹`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PageGrid {
    /// The numerator of the pitch in nanometres on the page (`7_937_500` for QElectroTech's 10 px).
    pub num_nm: i64,
    /// The denominator of the pitch (`3` for QElectroTech's 10 px); at least 1.
    pub den: i64,
    /// The scale of the page-to-sheet map in parts per 10⁹ (`1_000_000_000` without a map).
    pub sheet_scale_ppb: u32,
    /// Where the page origin lands on the sheet, in nanometres (`(0, 0)` without a map).
    pub sheet_offset_nm: (i64, i64),
}

impl Session {
    /// The connection grid of the page labelled `folio` in the page order, with its page-to-sheet map
    /// ([`PageGrid`]); `None` when no page has that printed folio label. Reads the memoised `page_order`,
    /// `page_grid` and `page_frame` queries.
    pub fn page_grid(&self, folio: &str) -> Option<PageGrid> {
        let q = self.query();
        let order = q.get::<PageOrder>(());
        let entry = order.entries.iter().find(|e| e.folio == folio)?;
        let grid = *q.get::<PageGridOf>(entry.page);
        let frame = q.get::<PageFrameOf>(entry.page);
        let map = frame
            .as_ref()
            .as_ref()
            .map_or(ew_scene::forms::SheetMap::IDENTITY, |f| {
                ew_scene::forms::SheetMap::of(&f.form)
            });
        Some(PageGrid {
            num_nm: i64::try_from(grid.num()).unwrap_or(i64::MAX),
            den: i64::try_from(grid.den()).unwrap_or(i64::MAX),
            sheet_scale_ppb: map.scale_ppb,
            sheet_offset_nm: (map.offset.x.0, map.offset.y.0),
        })
    }

    /// The sheet display lists of an export of the current snapshot through the session's memo store
    /// ([`export_pages_with`]).
    ///
    /// # Errors
    ///
    /// [`OutputError::Selection`] when the selection names a folio the page order does not have.
    pub fn export_pages(&self, options: &ExportOptions) -> Result<ExportPages, OutputError> {
        export_pages_with(&self.query(), options, &self.export_assets())
    }

    /// The asset bytes of the current snapshot, for the image items of an export ([`ExportAssets`]).
    pub fn export_assets(&self) -> ExportAssets {
        ExportAssets::of(self.engine.model())
    }

    /// The PDF of the current snapshot (`ew export pdf`, doc 09 §4.2): the export job of this module through the
    /// session's memo store.
    ///
    /// # Errors
    ///
    /// [`OutputError`] for a selection the page order does not have or pages the writer refuses.
    pub fn export_pdf(&self, options: &ExportOptions) -> Result<PdfExport, OutputError> {
        export_pdf_with(&self.query(), options, &self.export_assets())
    }

    /// The SVG set of the current snapshot (`ew export svg`, doc 09 §4.4), one file per page.
    ///
    /// # Errors
    ///
    /// [`OutputError`] for a selection the page order does not have or pages the writer refuses.
    pub fn export_svg(&self, options: &ExportOptions) -> Result<SvgExport, OutputError> {
        export_svg_with(&self.query(), options, &self.export_assets())
    }

    /// The DXF set of the current snapshot (`ew export dxf`, doc 09 §4.4), one file per page ([`export_dxf_with`]).
    ///
    /// # Errors
    ///
    /// [`OutputError`] for a selection the page order does not have.
    pub fn export_dxf(
        &self,
        options: &ExportOptions,
        layers: &ew_export::DxfLayers,
    ) -> Result<DxfExport, OutputError> {
        export_dxf_with(&self.query(), options, layers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_dates_are_civil_utc_dates() {
        let d = date_from_epoch(0).unwrap();
        assert_eq!((d.year, d.month, d.day, d.hour), (1970, 1, 1, 0));
        // 2025-04-14T12:34:56Z.
        let d = date_from_epoch(1_744_634_096).unwrap();
        assert_eq!(
            (d.year, d.month, d.day, d.hour, d.minute, d.second),
            (2025, 4, 14, 12, 34, 56)
        );
        // 2024-02-29 (a leap day) and the day before 1970.
        let d = date_from_epoch(1_709_164_800).unwrap();
        assert_eq!((d.year, d.month, d.day), (2024, 2, 29));
        let d = date_from_epoch(-1).unwrap();
        assert_eq!(
            (d.year, d.month, d.day, d.hour, d.minute, d.second),
            (1969, 12, 31, 23, 59, 59)
        );
        assert!(date_from_epoch(i64::MAX / 2).is_none());
    }

    #[test]
    fn a_selection_needs_items() {
        assert_eq!(
            PageSelection::parse("101,103-106").unwrap(),
            PageSelection::Folios(vec!["101".into(), "103-106".into()])
        );
        assert!(PageSelection::parse("").is_err());
        assert!(PageSelection::parse("101,,102").is_err());
    }
}
