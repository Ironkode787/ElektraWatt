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

use std::ops::RangeInclusive;

use ew_derive::{PageOrder, Q};
use ew_dl::DisplayList;
use ew_export::ew_render_cpu;
use ew_export::par_map;
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
/// [`OutputError::Raster`] when the sheet has no positive size at `dpi` or is too large for the rasteriser, or a text
/// names a font the bundled set does not hold.
pub fn page_png(scene: &PageScene, dpi: u32) -> Result<Vec<u8>, OutputError> {
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
/// rendered by [`page_png`]'s renderer on `options.parallelism`.
///
/// # Errors
///
/// [`OutputError::Selection`] for a selection the page order does not have; [`OutputError::Raster`] as [`page_png`].
pub fn export_png_with(
    q: &Q,
    options: &ExportOptions,
    dpi: u32,
    assets: &ExportAssets,
) -> Result<PngExport, OutputError> {
    let pages = export_pages_with(q, options, assets)?;
    let files = par_map(&pages.pages, options.parallelism, |_, page| {
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
