// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page output: the export job of the session (FT-59; doc 09 §4; ARCH §6.4, §8.4; decision log ID-125, ID-144,
//! ID-344, ID-346).
//!
//! **The view.** `view.rs` owns the glue rules that fill `ew_scene::page::PageView` from `Q` (the rules that were
//! test glue until cycle 14), one query family per part of the view. **The layers.** `layers.rs` builds one
//! memoised `page_display_list` family per layer over the parts that layer reads, so a wire edit never rebuilds
//! the title block (the memo oracle and `tests/page_output_memo.rs` hold it).
//!
//! **The job.** [`export_pages`] (and the [`Session`] methods [`Session::export_pdf`], [`Session::export_svg`])
//! runs in three phases:
//!
//! 1. on the session's thread, through its memoising `Q`: `page_order`, the page selection, and for every
//!    selected page its memoised layers assembled into a page scene ([`page_scene`]);
//! 2. the page phase on [`ew_export::par_map`] (scoped `std` threads, results by page index, decision log ID-344):
//!    each page's layers mapped onto its sheet with their symbol instances inlined, the one list every writer draws;
//! 3. the writer: [`ew_export::pdf`] (PDF/A-2b, outline, `/PageLabels`, named destinations, links, subset fonts)
//!    or [`ew_export::svg`] (one `<label>.svg` per page), with the bundled fonts only ([`font_bytes`]).
//!
//! Serial and parallel runs give the same bytes (AC-14). The `Session` is not `Send`, so phase 1 stays on its
//! thread: the layers are memoised in the session's store and a second export after an edit rebuilds only the
//! layers the edit touched. The job is synchronous: progress, cancellation and the job pool of ARCH §2 (`ew-jobs`)
//! are not built (the CLI is its only caller so far).
//!
//! **Metadata** ([`pdf_options`], doc 09 §4.3). Title: the project description (neutral). Creator: `ElektraWatt
//! <version>`. Date (FR-OUT-026): the caller's (`SOURCE_DATE_EPOCH` in the CLI), else the model's edited date, else
//! its created date, else 1980-01-01; never the wall clock. Document ID: krilla's hash of the text
//! `ew-pdf/1 <project id> <derivation profile> <date> <pages>[ mono] ew <version> sha256:<digest>` (doc 09 §4.3
//! rule 2: a hash of the export input), `<pages>` being `all` or the selected folio labels joined by `,` and
//! `<digest>` the [`content_digest`] of the pages and the bundled fonts.

mod layers;
mod navigation;
mod view;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

use ew_derive::{PageEntry, PageOrder, PageOrderV, PageTitle, Policy, Q, Query};
use ew_dl::FontKey;
use ew_export::ew_render_cpu::FontBytes;
use ew_export::{PdfError, PdfOptions, PdfPage, SvgError, SvgFace, SvgOptions, par_map};
use ew_scene::page::PageFinding;
use sha2::{Digest, Sha256};

/// The display-list IR the pages of an export are made of ([`ExportPages`]' `content`), re-exported so that the
/// shell names its types (`DisplayList`, `EntityRef`, `Rect`) without a workspace edge to `ew-dl` (WP-S2.02 §4.2;
/// architecture §3 rule 7).
pub use ew_dl;
pub use ew_export::{Parallelism, PdfDate, PdfReport, SvgFile, SvgReport, svg_file_name};
pub use layers::{
    FrameLayer, LabelsLayer, PageLayer, SymbolsLayer, TablesLayer, WiresLayer, page_kind, page_layer,
    page_scene, page_view,
};
pub use navigation::{CellLabel, GotoError, GotoTarget, LinkTarget, PageCells, PageRow};
pub use view::{
    CpTargetsByPage, FrameForms, GENERIC_SYMBOL_SEALS, GENERIC_SYMBOLS_PROFILE, GenericSymbols, NOGRID_FRAME,
    PageBindingsOf, PageConnectionsOf, PageCpTargetsOf, PageEmbeddedOf, PageFrameOf, PageGridOf,
    PageJunctionsOf, PageRepsOf, PageSlotsOf, PageTableOf, generic_symbols, page_destination, page_uuid,
};

use crate::Session;

/// Every query family of the page output with its policy, in name order (the memoised ones are swept by
/// `ew verify-derived`, [`crate::verify::SWEPT`]).
pub const FAMILIES: &[(&str, Policy)] = &[
    (CpTargetsByPage::FAMILY, CpTargetsByPage::POLICY),
    (PageBindingsOf::FAMILY, PageBindingsOf::POLICY),
    (PageConnectionsOf::FAMILY, PageConnectionsOf::POLICY),
    (PageCpTargetsOf::FAMILY, PageCpTargetsOf::POLICY),
    (FrameLayer::FAMILY, FrameLayer::POLICY),
    (LabelsLayer::FAMILY, LabelsLayer::POLICY),
    (SymbolsLayer::FAMILY, SymbolsLayer::POLICY),
    (TablesLayer::FAMILY, TablesLayer::POLICY),
    (WiresLayer::FAMILY, WiresLayer::POLICY),
    (PageEmbeddedOf::FAMILY, PageEmbeddedOf::POLICY),
    (PageFrameOf::FAMILY, PageFrameOf::POLICY),
    (PageGridOf::FAMILY, PageGridOf::POLICY),
    (PageJunctionsOf::FAMILY, PageJunctionsOf::POLICY),
    (PageRepsOf::FAMILY, PageRepsOf::POLICY),
    (PageSlotsOf::FAMILY, PageSlotsOf::POLICY),
    (PageTableOf::FAMILY, PageTableOf::POLICY),
];

/// Why an export did not complete.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum OutputError {
    /// The page selection names a folio that is not in the page order, a range that runs backwards, or nothing.
    Selection(String),
    /// The PDF writer refused the pages.
    Pdf(PdfError),
    /// The SVG writer refused the pages.
    Svg(SvgError),
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::Selection(m) => write!(f, "page selection: {m}"),
            OutputError::Pdf(e) => write!(f, "PDF: {e}"),
            OutputError::Svg(e) => write!(f, "SVG: {e}"),
        }
    }
}

impl std::error::Error for OutputError {}

/// Which pages of the page order an export writes (`--pages`).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum PageSelection {
    /// Every page of the page order.
    #[default]
    All,
    /// The listed items in page order, each page once: an item is a folio label ("101", "2.1") or a range
    /// `<first>-<last>` of two folio labels, every page from the first to the last in page order.
    Folios(Vec<String>),
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

    /// The positions in `order` the selection names, ascending and unique.
    ///
    /// # Errors
    ///
    /// [`OutputError::Selection`] when an item names no folio, a range runs backwards or is ambiguous.
    pub fn resolve(&self, order: &PageOrderV) -> Result<Vec<usize>, OutputError> {
        let PageSelection::Folios(items) = self else {
            return Ok((0..order.entries.len()).collect());
        };
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

/// A page finding with the folio it was found on.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageFindingAt {
    /// The folio label.
    pub folio: String,
    /// The finding.
    pub finding: PageFinding,
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
}

/// A written SVG set.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SvgExport {
    /// One file per page, in page order.
    pub files: Vec<SvgFile>,
    /// The writer's report.
    pub report: SvgReport,
    /// The findings of the page scenes.
    pub findings: Vec<PageFindingAt>,
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

/// The SVG options of an export: the bundled faces by family, weight and style.
pub fn svg_options(monochrome: bool) -> SvgOptions {
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
/// pages, the PDF options and the page findings.
///
/// # Errors
///
/// [`OutputError::Selection`] when the selection names a folio the page order does not have.
pub fn export_pages(q: &Q, options: &ExportOptions) -> Result<ExportPages, OutputError> {
    let order = q.get::<PageOrder>(());
    let picked = options.pages.resolve(&order)?;
    if picked.is_empty() {
        return Err(OutputError::Selection("the export has no pages".to_owned()));
    }
    // Phase 1, on this thread through `q`: the memoised layers of every selected page.
    let phase1: Vec<Phase1> = picked
        .iter()
        .map(|&i| {
            let entry = order.entries[i].clone();
            Phase1 {
                title: outline_title(q, &entry),
                section: section_title(q, &entry),
                scene: page_scene(q, &entry),
                entry,
            }
        })
        .collect();
    let findings = phase1
        .iter()
        .flat_map(|p| {
            p.scene.findings.iter().map(|f| PageFindingAt {
                folio: p.entry.folio.clone(),
                finding: f.clone(),
            })
        })
        .collect();
    // Phase 2, the page phase: every sheet list on its own, collected by page index.
    let pages = par_map(&phase1, options.parallelism, |_, p| PdfPage {
        destination: page_destination(p.entry.page),
        label: p.entry.folio.clone(),
        title: p.title.clone(),
        section: p.section.clone(),
        width: p.scene.sheet_width,
        height: p.scene.sheet_height,
        content: p.scene.sheet_list(),
    });
    let labels: Option<Vec<String>> = match options.pages {
        PageSelection::All => None,
        PageSelection::Folios(_) => Some(pages.iter().map(|p| p.label.clone()).collect()),
    };
    Ok(ExportPages {
        options: pdf_options(q, options, labels.as_deref(), &pages),
        pages,
        findings,
    })
}

/// The PDF of an export over `q` (phase 3: [`ew_export::pdf`] with the bundled fonts).
///
/// # Errors
///
/// [`OutputError`] for a selection the page order does not have or pages the writer refuses.
pub fn export_pdf(q: &Q, options: &ExportOptions) -> Result<PdfExport, OutputError> {
    let pages = export_pages(q, options)?;
    let out = ew_export::pdf(&pages.pages, &pages.options, font_bytes(), None).map_err(OutputError::Pdf)?;
    Ok(PdfExport {
        bytes: out.bytes,
        report: out.report,
        pages: pages.pages.len(),
        findings: pages.findings,
    })
}

/// The SVG set of an export over `q` (phase 3: [`ew_export::svg`], its page phase on the same parallelism).
///
/// # Errors
///
/// [`OutputError`] for a selection the page order does not have or pages the writer refuses.
pub fn export_svg(q: &Q, options: &ExportOptions) -> Result<SvgExport, OutputError> {
    let pages = export_pages(q, options)?;
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
    /// ([`export_pages`]).
    ///
    /// # Errors
    ///
    /// [`OutputError::Selection`] when the selection names a folio the page order does not have.
    pub fn export_pages(&self, options: &ExportOptions) -> Result<ExportPages, OutputError> {
        export_pages(&self.query(), options)
    }

    /// The PDF of the current snapshot (`ew export pdf`, doc 09 §4.2): the export job of this module through the
    /// session's memo store.
    ///
    /// # Errors
    ///
    /// [`OutputError`] for a selection the page order does not have or pages the writer refuses.
    pub fn export_pdf(&self, options: &ExportOptions) -> Result<PdfExport, OutputError> {
        export_pdf(&self.query(), options)
    }

    /// The SVG set of the current snapshot (`ew export svg`, doc 09 §4.4), one file per page.
    ///
    /// # Errors
    ///
    /// [`OutputError`] for a selection the page order does not have or pages the writer refuses.
    pub fn export_svg(&self, options: &ExportOptions) -> Result<SvgExport, OutputError> {
        export_svg(&self.query(), options)
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
