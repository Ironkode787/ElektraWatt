// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page rasters (WP-S1.22 part 2, D7; FR-EXT-004, FT-254): `ew render --page <folio> --png` and `ew export png`.
//!
//! One function draws a page: [`page_png`] renders a page scene's sheet list through `ew-render-cpu` with the bundled
//! fonts only ([`font_bytes`]) and encodes it as PNG with the resolution in its `pHYs` chunk. The golden PNGs
//! (`golden/J-E-2416/expected/png/<folio>.png`, 150 dpi) are this function's output, so the CLI writes their bytes.
//! [`export_png_with`] runs phases 1 and 2 of the export job ([`export_pages_with`]: the selection, the memoised
//! layers, the images resolved from the project's assets) and rasterises the sheets on the same parallelism, so the
//! threads change the speed, never the bytes.
//!
//! Memory is bounded twice (cycle-40 review): a page whose raster would exceed [`MAX_PAGE_PIXELS`] is refused before
//! any page is drawn ([`check_page_pixels`]), and `export png` runs only as many rasters at once as keep the pixels in
//! flight under [`MAX_PIXELS_IN_FLIGHT`] ([`raster_workers`]).

use std::ops::RangeInclusive;

use ew_derive::{PageOrder, Q};
use ew_dl::DisplayList;
use ew_export::ew_render_cpu;
use ew_export::{Parallelism, par_map};
use ew_scene::page::PageScene;
use ew_units::Nm;

use super::{
    ExportAssets, ExportOptions, OutputError, PageFindingAt, PageSelection, export_pages_with, font_bytes,
};
use crate::Session;

/// The resolution of a page raster when none is given: the golden PNGs' 150 dpi.
pub const DEFAULT_DPI: u32 = 150;

/// The resolutions a page raster may take (`--dpi`).
pub const DPI_RANGE: RangeInclusive<u32> = 72..=600;

/// The largest raster of one page, in pixels (about 600 MB of RGBA): A2 at 600 dpi fits, A1 at 600 dpi does not.
pub const MAX_PAGE_PIXELS: u64 = 150_000_000;

/// The most raster pixels `export png` holds at once over all its threads (at least one page at a time). It counts
/// the rasters only: each page's PNG encoder buffers and encoded bytes come on top.
pub const MAX_PIXELS_IN_FLIGHT: u64 = 600_000_000;

/// The pixel size of a sheet at `dpi`, and whether it may be rasterised (at most [`MAX_PAGE_PIXELS`] and no side
/// above `u16::MAX`).
fn raster_pixels(width: Nm, height: Nm, dpi: u32) -> (u64, u64, bool) {
    let (w, h) = ew_render_cpu::pixel_size(ew_render_cpu::SheetSize { width, height }, dpi);
    let side = u64::from(u16::MAX);
    let fits = w.saturating_mul(h) <= MAX_PAGE_PIXELS && w <= side && h <= side;
    (w, h, fits)
}

/// Refuses a page whose raster at `dpi` would exceed [`MAX_PAGE_PIXELS`] (or `u16::MAX` pixels on a side), before
/// anything is allocated. `who` names the page at the start of a sentence (`Folio 103`).
///
/// # Errors
///
/// [`OutputError::Raster`] naming the page, its size in pixels at `dpi` and the highest whole dpi that fits:
/// "Folio 103 would be 23386 × 16535 pixels at 600 dpi; the limit is 150 million pixels. Choose 400 dpi or less."
/// When not even the lowest resolution of [`DPI_RANGE`] fits, the last sentence says so instead.
pub fn check_page_pixels(who: &str, width: Nm, height: Nm, dpi: u32) -> Result<(), OutputError> {
    let (w, h, fits) = raster_pixels(width, height, dpi);
    if fits {
        return Ok(());
    }
    let lowest = *DPI_RANGE.start();
    let advice = match (lowest..dpi).rev().find(|&d| raster_pixels(width, height, d).2) {
        Some(d) => format!("Choose {d} dpi or less."),
        None => format!("The page is too large at every resolution from {lowest} dpi."),
    };
    Err(OutputError::Raster(format!(
        "{who} would be {w} × {h} pixels at {dpi} dpi; the limit is {} million pixels. {advice}",
        MAX_PAGE_PIXELS / 1_000_000
    )))
}

/// How many rasters `export png` draws at once: the most threads, up to `requested`, such that the largest pages
/// together stay within [`MAX_PIXELS_IN_FLIGHT`]; never fewer than 1. `pixels` holds each page's pixel count.
///
/// Any page may meet any other on the threads, so the bound sums the `n` largest pages, whatever their order.
pub fn raster_workers(pixels: &[u64], requested: usize) -> usize {
    let mut sorted = pixels.to_vec();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    let mut total = 0u64;
    let mut n = 0usize;
    for p in sorted.into_iter().take(requested) {
        total = total.saturating_add(p);
        if total > MAX_PIXELS_IN_FLIGHT {
            break;
        }
        n += 1;
    }
    n.max(1)
}

/// Refuses a file set in which two folio labels give the same file name under `name` (`1/2` and `1_2` both give
/// `1_2.png`), so that no page of a PNG or SVG set overwrites another (cycle-40 review, part 3, finding 4).
///
/// # Errors
///
/// [`OutputError::Selection`] naming the first two such folios in page order and the file name.
pub(super) fn refuse_same_file_names<'a>(
    labels: impl IntoIterator<Item = &'a str>,
    name: impl Fn(&str) -> String,
) -> Result<(), OutputError> {
    // Case folded: `A1.png` and `a1.png` are one file on Windows and macOS, and the refusal is the same everywhere.
    let mut seen: std::collections::BTreeMap<String, (&str, String)> = std::collections::BTreeMap::new();
    for label in labels {
        let file = name(label);
        if let Some((first, file)) = seen.get(&file.to_ascii_lowercase()) {
            return Err(OutputError::Selection(format!(
                "folios `{first}` and `{label}` would both be written to `{file}`; export them in separate runs \
                 (`--pages`)"
            )));
        }
        seen.insert(file.to_ascii_lowercase(), (label, file));
    }
    Ok(())
}

/// One page raster.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PngFile {
    /// The file name: `<folio>.png`, the folio label with every character outside `[A-Za-z0-9._-]` as `_`.
    pub name: String,
    /// The folio label.
    pub folio: String,
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
    /// The PNG bytes.
    pub bytes: Vec<u8>,
}

/// The rasters of an export, one per selected page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PngExport {
    /// One file per page, in page order.
    pub files: Vec<PngFile>,
    /// The findings of the page scenes.
    pub findings: Vec<PageFindingAt>,
}

/// The file name of a page raster: the folio label with every character outside `[A-Za-z0-9._-]` as `_`, then
/// `.png` (the rule of [`super::svg_file_name`]).
pub fn png_file_name(label: &str) -> String {
    let svg = super::svg_file_name(label);
    format!("{}.png", svg.strip_suffix(".svg").unwrap_or(&svg))
}

/// The PNG of a page scene at `dpi` (D7): its sheet list rendered with the bundled fonts, encoded with the resolution.
///
/// # Errors
///
/// [`OutputError::Raster`] when the sheet has no positive size at `dpi`, is larger than [`MAX_PAGE_PIXELS`]
/// ([`check_page_pixels`]), or a text names a font the bundled set does not hold.
pub fn page_png(scene: &PageScene, dpi: u32) -> Result<Vec<u8>, OutputError> {
    check_page_pixels("The page", scene.sheet_width, scene.sheet_height, dpi)?;
    sheet_png(&scene.sheet_list(), scene.sheet_width, scene.sheet_height, dpi).map(|(bytes, _)| bytes)
}

/// The PNG of one sheet display list and its pixel size.
fn sheet_png(
    list: &DisplayList,
    width: Nm,
    height: Nm,
    dpi: u32,
) -> Result<(Vec<u8>, (u32, u32)), OutputError> {
    let options = ew_render_cpu::RenderOptions {
        dpi,
        ..Default::default()
    };
    let (img, report) = ew_render_cpu::render(
        list,
        ew_render_cpu::SheetSize { width, height },
        &options,
        font_bytes(),
        None,
    )
    .map_err(|e| OutputError::Raster(e.to_string()))?;
    if !report.missing_fonts.is_empty() {
        return Err(OutputError::Raster(format!(
            "{} fonts of the page texts are not bundled",
            report.missing_fonts.len()
        )));
    }
    Ok((
        ew_render_cpu::encode_png(&img, Some(dpi)),
        (img.width, img.height),
    ))
}

/// The rasters of an export over `q` at `dpi` (`ew export png`): phases 1 and 2 of the export job, then every sheet
/// rendered by [`page_png`]'s renderer on `options.parallelism`, but never more rasters at once than
/// [`raster_workers`] allows. Every page's size is checked ([`check_page_pixels`]) before the first is drawn.
///
/// # Errors
///
/// [`OutputError::Selection`] for a selection the page order does not have; [`OutputError::Raster`] as [`page_png`],
/// naming the first page in page order that is too large by its folio.
pub fn export_png_with(
    q: &Q,
    options: &ExportOptions,
    dpi: u32,
    assets: &ExportAssets,
) -> Result<PngExport, OutputError> {
    let pages = export_pages_with(q, options, assets)?;
    refuse_same_file_names(pages.pages.iter().map(|p| p.label.as_str()), png_file_name)?;
    let mut pixels = Vec::with_capacity(pages.pages.len());
    for page in &pages.pages {
        check_page_pixels(&format!("Folio {}", page.label), page.width, page.height, dpi)?;
        let (w, h, _) = raster_pixels(page.width, page.height, dpi);
        pixels.push(w * h);
    }
    let requested = options.parallelism.workers(pages.pages.len());
    let parallelism = Parallelism::threads(raster_workers(&pixels, requested));
    let files = par_map(&pages.pages, parallelism, |_, page| {
        sheet_png(&page.content, page.width, page.height, dpi).map(|(bytes, (width, height))| PngFile {
            name: png_file_name(&page.label),
            folio: page.label.clone(),
            width,
            height,
            bytes,
        })
    })
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    Ok(PngExport {
        files,
        findings: pages.findings,
    })
}

impl Session {
    /// The rasters of the current snapshot at `dpi`, one per selected page (`ew export png`, FT-254).
    ///
    /// # Errors
    ///
    /// As [`export_png_with`].
    pub fn export_png(&self, options: &ExportOptions, dpi: u32) -> Result<PngExport, OutputError> {
        export_png_with(&self.query(), options, dpi, &self.export_assets())
    }

    /// The raster of the page with the folio label `folio` at `dpi` (`ew render --page <folio> --png`, FR-EXT-004):
    /// the label exactly, never a range.
    ///
    /// # Errors
    ///
    /// [`OutputError::Selection`] when no page has the label; [`OutputError::Raster`] as [`page_png`].
    pub fn render_png(&self, folio: &str, dpi: u32) -> Result<PngExport, OutputError> {
        let q = self.query();
        if !q.get::<PageOrder>(()).entries.iter().any(|e| e.folio == folio) {
            return Err(OutputError::Selection(format!(
                "the page order has no folio `{folio}`"
            )));
        }
        let options = ExportOptions {
            pages: PageSelection::Folios(vec![folio.to_owned()]),
            ..ExportOptions::default()
        };
        export_png_with(&q, &options, dpi, &self.export_assets())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MM: i64 = 1_000_000;

    #[test]
    fn a_page_above_the_pixel_limit_is_refused_with_its_size_and_the_dpi_that_fits() {
        // A0 portrait (841 × 1189 mm) at 600 dpi: about 558 million pixels.
        let e = check_page_pixels("Folio 103", Nm(841 * MM), Nm(1189 * MM), 600).unwrap_err();
        assert_eq!(
            e,
            OutputError::Raster(
                "Folio 103 would be 19866 × 28087 pixels at 600 dpi; the limit is 150 million pixels. \
                 Choose 311 dpi or less."
                    .to_owned()
            )
        );
        // The dpi named fits; one more does not.
        assert!(check_page_pixels("Folio 103", Nm(841 * MM), Nm(1189 * MM), 311).is_ok());
        assert!(check_page_pixels("Folio 103", Nm(841 * MM), Nm(1189 * MM), 312).is_err());
        // A2 (420 × 594 mm) at 600 dpi fits.
        assert!(check_page_pixels("Folio 1", Nm(420 * MM), Nm(594 * MM), 600).is_ok());
    }

    #[test]
    fn a_page_too_large_at_every_resolution_says_so() {
        // 20 m × 1 m at 72 dpi.
        let e = check_page_pixels("Folio 7", Nm(20_000 * MM), Nm(1_000 * MM), 72).unwrap_err();
        assert_eq!(
            e.to_string(),
            "PNG: Folio 7 would be 56693 × 2835 pixels at 72 dpi; the limit is 150 million pixels. \
             The page is too large at every resolution from 72 dpi."
        );
        // A side above u16::MAX is refused even when the pixel count is small.
        let e = check_page_pixels("Folio 8", Nm(30_000 * MM), Nm(10 * MM), 72).unwrap_err();
        assert_eq!(
            e.to_string(),
            "PNG: Folio 8 would be 85039 × 28 pixels at 72 dpi; the limit is 150 million pixels. \
             The page is too large at every resolution from 72 dpi."
        );
    }

    /// Cycle-40 review, part 3, finding 4: two folios whose file names are the same (after `_` for other characters,
    /// and ignoring case) are refused naming both folios and the file; distinct names pass.
    #[test]
    fn two_folios_with_one_file_name_are_refused() {
        assert_eq!(
            refuse_same_file_names(["1", "1/2", "2", "1_2"], png_file_name)
                .unwrap_err()
                .to_string(),
            "page selection: folios `1/2` and `1_2` would both be written to `1_2.png`; export them in separate \
             runs (`--pages`)"
        );
        assert_eq!(
            refuse_same_file_names(["A1", "a1"], super::super::svg_file_name)
                .unwrap_err()
                .to_string(),
            "page selection: folios `A1` and `a1` would both be written to `A1.svg`; export them in separate runs \
             (`--pages`)"
        );
        assert!(refuse_same_file_names(["101", "101.1", "908.2"], png_file_name).is_ok());
    }

    #[test]
    fn raster_workers_keep_the_largest_pages_within_the_pixels_in_flight() {
        let a3_600 = 9921 * 7016; // about 70 million
        let a2_600 = 9921 * 14031; // about 139 million
        // Small pages: the requested threads.
        assert_eq!(raster_workers(&[1_000_000; 30], 24), 24);
        // Never more threads than requested, never fewer than one.
        assert_eq!(raster_workers(&[1_000_000; 30], 1), 1);
        assert_eq!(raster_workers(&[], 4), 1);
        // A3 at 600 dpi: 8 pages fit in 600 million, 9 do not.
        assert_eq!(raster_workers(&[a3_600; 40], 24), 8);
        // Two A2 pages and six of 50 million fit (578 million), a seventh does not, wherever the large pages stand.
        let mut mixed = vec![50_000_000; 20];
        mixed.insert(3, a2_600);
        mixed.push(a2_600);
        assert_eq!(raster_workers(&mixed, 24), 8);
        // Pages at the per-page limit still run four at a time.
        assert_eq!(raster_workers(&[MAX_PAGE_PIXELS; 10], 24), 4);
    }
}
