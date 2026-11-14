// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew verify-migration`: an imported project against the drawing it came from (FR-IMP-021, doc 14 §4.15).
//!
//! [`verify_migration`] renders every page of the project at [`DPI`] through the page export (the frame transform
//! included), reads the matching page of the reference PDF with hayro, and compares two things: the text runs
//! (the same string whose anchor lies within [`ANCHOR_TOLERANCE_MM`]) and the raster, cell by cell of the page's frame
//! grid, with the boxes of every text of either side masked so that a substituted font does not swamp the result.
//! Pages pair by folio label when the PDF's page labels name every page of the project, else by order.
//!
//! The result is a list of [`Deviation`]s. A project's expected deltas are declared in a JSON file ([`Expected`]);
//! a deviation one of them matches is reported as expected, and a declared delta that matched nothing is reported
//! as unmet, so the check expects exactly the declared set.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_dl::{Item, ItemKind, Transform};
use ew_export::ew_render_cpu::RgbaImage;
use ew_scene::pdf_text::{TextRun, pdf_page_labels, pdf_page_text};
use ew_scene::underlay::{PdfError, pdf_pixmap};
use ew_units::Pt2;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Session;
use crate::output::{ExportOptions, OutputError, sheet_image};

/// The resolution both sides are rasterised at.
pub const DPI: u32 = 150;

/// How far two anchors of one text may lie apart, in millimetres.
pub const ANCHOR_TOLERANCE_MM: f64 = 0.5;

/// A size of the two pages that differs by more than this many millimetres is a deviation of its own; the page's
/// texts and raster are then not compared.
const SIZE_TOLERANCE_MM: f64 = 1.0;

/// A pixel is ink when its luminance is below this.
const INK_BELOW: u32 = 200;

/// The raster of one cell differs when more pixels than this are ink on one side only: about 10 mm of a hairline at
/// [`DPI`], whatever the cell's size (a page without a frame grid is one cell).
const CELL_PIXELS: usize = 60;

/// The padding of a text box in the mask, in millimetres.
const MASK_PAD_MM: f64 = 1.0;

/// Why a verification could not be made.
#[derive(Debug)]
pub enum MigrationError {
    /// The reference PDF cannot be read.
    Pdf(PdfError),
    /// A page of the project cannot be drawn.
    Output(OutputError),
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::Pdf(e) => write!(f, "the reference PDF: {e}"),
            MigrationError::Output(e) => write!(f, "a page of the project cannot be drawn: {e}"),
        }
    }
}

impl std::error::Error for MigrationError {}

/// What differs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeviationKind {
    /// A page of the project the reference has no match for.
    PageNotInReference,
    /// A page of the reference the project has no match for.
    PageNotInProject,
    /// The two pages differ in size.
    SizeDiffers,
    /// A text of the reference the project does not have.
    TextMissing,
    /// A text of the project the reference does not have.
    TextExtra,
    /// A text both have, anchored further apart than the tolerance.
    TextMoved,
    /// A frame cell whose raster differs.
    RasterCell,
}

impl DeviationKind {
    /// The kind as the words of the JSON result and of an expected-delta file.
    pub const fn word(self) -> &'static str {
        match self {
            DeviationKind::PageNotInReference => "page-not-in-reference",
            DeviationKind::PageNotInProject => "page-not-in-project",
            DeviationKind::SizeDiffers => "size-differs",
            DeviationKind::TextMissing => "text-missing",
            DeviationKind::TextExtra => "text-extra",
            DeviationKind::TextMoved => "text-moved",
            DeviationKind::RasterCell => "raster-cell",
        }
    }
}

/// One difference between the project and the reference.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Deviation {
    /// The folio label of the page (the reference's own label for a page the project lacks).
    pub page: String,
    /// `page-not-in-reference`, `page-not-in-project`, `size-differs`, `text-missing`, `text-extra`, `text-moved` or
    /// `raster-cell` ([`DeviationKind::word`]).
    pub kind: String,
    /// The text, for the text kinds; else `null`.
    pub text: Option<String>,
    /// The frame cell (`C4`), for a raster cell; else `null`.
    pub cell: Option<String>,
    /// Where the difference is: the anchor of the text in the project (the reference for a missing one), in
    /// millimetres from the page's top left; else `null`.
    pub at_mm: Option<[f64; 2]>,
    /// The difference as a sentence.
    pub detail: String,
}

impl Deviation {
    fn new(page: &str, kind: DeviationKind, detail: String) -> Deviation {
        Deviation {
            page: page.to_owned(),
            kind: kind.word().to_owned(),
            text: None,
            cell: None,
            at_mm: None,
            detail,
        }
    }
}

/// One expected delta of a project, declared in a JSON file (an array of these): a deviation of this kind (and of
/// this page, text and cell where given) is expected.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    /// The kind of deviation ([`DeviationKind::word`]).
    pub kind: String,
    /// The folio label; absent: any page.
    #[serde(default)]
    pub page: Option<String>,
    /// The text; absent: any text.
    #[serde(default)]
    pub text: Option<String>,
    /// The frame cell; absent: any cell.
    #[serde(default)]
    pub cell: Option<String>,
}

impl Expected {
    fn matches(&self, d: &Deviation) -> bool {
        self.kind == d.kind
            && self.page.as_ref().is_none_or(|p| *p == d.page)
            && self.text.as_ref().is_none_or(|t| d.text.as_ref() == Some(t))
            && self.cell.as_ref().is_none_or(|c| d.cell.as_ref() == Some(c))
    }
}

/// The expected deltas of a declaration file.
///
/// # Errors
///
/// The reason, in words, when the text is no array of [`Expected`].
pub fn parse_expected(json: &str) -> Result<Vec<Expected>, String> {
    serde_json::from_str(json).map_err(|e| format!("the expected deltas are no list of deltas: {e}"))
}

/// What a verification found.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct MigrationReport {
    /// Page pairs compared.
    pub pages: usize,
    /// Text runs of the reference that found their match in the project.
    pub texts_matched: usize,
    /// Frame cells whose raster was compared.
    pub cells: usize,
    /// Deviations no declared delta matches.
    pub deviations: Vec<Deviation>,
    /// Deviations a declared delta matches.
    pub expected: Vec<Deviation>,
    /// Declared deltas that matched no deviation.
    pub unmet: Vec<Expected>,
}

impl MigrationReport {
    /// Whether the project differs from the reference by exactly the declared deltas.
    pub fn is_clean(&self) -> bool {
        self.deviations.is_empty() && self.unmet.is_empty()
    }
}

/// Compares the project of `session` with the PDF `reference`; `expected` are the declared deltas.
///
/// # Errors
///
/// [`MigrationError`] for a reference hayro does not read and a page that cannot be drawn.
pub fn verify_migration(
    session: &Session,
    reference: Arc<[u8]>,
    expected: &[Expected],
) -> Result<MigrationReport, MigrationError> {
    let labels = pdf_page_labels(reference.clone()).map_err(MigrationError::Pdf)?;
    let exported = session
        .export_pages(&ExportOptions::default())
        .map_err(MigrationError::Output)?;
    let by_label = exported
        .pages
        .iter()
        .all(|p| labels.iter().any(|l| l.as_deref() == Some(p.label.as_str())));
    let mut report = MigrationReport::default();
    let mut found = Vec::new();
    let mut used = vec![false; labels.len()];
    for (i, page) in exported.pages.iter().enumerate() {
        let index = if by_label {
            labels
                .iter()
                .position(|l| l.as_deref() == Some(page.label.as_str()))
        } else {
            (i < labels.len()).then_some(i)
        };
        let Some(index) = index else {
            found.push(Deviation::new(
                &page.label,
                DeviationKind::PageNotInReference,
                "the reference has no page for this page of the project".to_owned(),
            ));
            continue;
        };
        used[index] = true;
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let text = pdf_page_text(reference.clone(), number).map_err(MigrationError::Pdf)?;
        let mm = |n: ew_units::Nm| n.0 as f64 / 1_000_000.0;
        let (width, height) = (mm(page.width), mm(page.height));
        if (width - text.width_mm).abs() > SIZE_TOLERANCE_MM
            || (height - text.height_mm).abs() > SIZE_TOLERANCE_MM
        {
            found.push(Deviation::new(
                &page.label,
                DeviationKind::SizeDiffers,
                format!(
                    "the page is {width:.1} × {height:.1} mm; the reference is {:.1} × {:.1} mm",
                    text.width_mm, text.height_mm
                ),
            ));
            continue;
        }
        report.pages += 1;
        let mine = list_runs(&page.content.items);
        let (matched, mut devs) = compare_texts(&page.label, &mine, &text.runs);
        report.texts_matched += matched;
        found.append(&mut devs);

        let image =
            sheet_image(&page.content, page.width, page.height, DPI).map_err(MigrationError::Output)?;
        let pixmap = pdf_pixmap(reference.clone(), number, (image.width, image.height))
            .map_err(MigrationError::Pdf)?;
        let cells = cells_of(session, &page.label, &image);
        let masks: Vec<[f64; 4]> = mine.iter().chain(&text.runs).map(|r| r.bounds).collect();
        let (compared, mut devs) =
            compare_rasters(&page.label, &image, pixmap.data_as_u8_slice(), &masks, &cells);
        report.cells += compared;
        found.append(&mut devs);
    }
    for (index, _) in used.iter().enumerate().filter(|(_, u)| !**u) {
        let label = labels[index].clone().unwrap_or_else(|| (index + 1).to_string());
        found.push(Deviation::new(
            &label,
            DeviationKind::PageNotInProject,
            "the project has no page for this page of the reference".to_owned(),
        ));
    }
    let mut met = vec![false; expected.len()];
    for d in found {
        let mut hit = false;
        for (e, m) in expected.iter().zip(&mut met) {
            if e.matches(&d) {
                *m = true;
                hit = true;
            }
        }
        if hit {
            report.expected.push(d);
        } else {
            report.deviations.push(d);
        }
    }
    report.unmet = expected
        .iter()
        .zip(&met)
        .filter(|(_, m)| !**m)
        .map(|(e, _)| e.clone())
        .collect();
    Ok(report)
}

/// The text runs of a sheet's display list, in millimetres from the top left of the sheet.
fn list_runs(items: &[Item]) -> Vec<TextRun> {
    fn walk(items: &[Item], stack: &mut Vec<Transform>, out: &mut Vec<TextRun>) {
        for item in items {
            match &item.kind {
                ItemKind::Glyphs(run) => {
                    let map = |p: Pt2| stack.iter().rev().fold(p, |p, t| t.apply(p));
                    let mm = |p: Pt2| (p.x.0 as f64 / 1_000_000.0, p.y.0 as f64 / 1_000_000.0);
                    let mut bounds = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
                    for c in run.layout_quad() {
                        let (x, y) = mm(map(c));
                        bounds = [
                            bounds[0].min(x),
                            bounds[1].min(y),
                            bounds[2].max(x),
                            bounds[3].max(y),
                        ];
                    }
                    out.push(TextRun {
                        text: run.text.clone(),
                        anchor: mm(map(run.origin)),
                        bounds,
                    });
                }
                ItemKind::Group(g) => {
                    stack.extend(g.transform);
                    walk(&g.items, stack, out);
                    if g.transform.is_some() {
                        stack.pop();
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(items, &mut Vec::new(), &mut out);
    out
}

/// A text without its white space: where the words break is the font's business, not the project's.
fn squeezed(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// The text deviations of one page and the number of reference runs that found their match.
///
/// Runs with the same text pair nearest first; a pair within the tolerance matches, the remaining runs of one text
/// pair nearest first as moved, and what is left over is missing (reference) or extra (project).
fn compare_texts(page: &str, mine: &[TextRun], reference: &[TextRun]) -> (usize, Vec<Deviation>) {
    let mut by_text: BTreeMap<String, (Vec<&TextRun>, Vec<&TextRun>)> = BTreeMap::new();
    for r in mine {
        by_text.entry(squeezed(&r.text)).or_default().0.push(r);
    }
    for r in reference {
        by_text.entry(squeezed(&r.text)).or_default().1.push(r);
    }
    let (mut matched, mut out) = (0, Vec::new());
    for (key, (mut mine, mut theirs)) in by_text {
        if key.is_empty() {
            continue;
        }
        let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
        for (i, m) in mine.iter().enumerate() {
            for (j, t) in theirs.iter().enumerate() {
                pairs.push((distance(m.anchor, t.anchor), i, j));
            }
        }
        pairs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let (mut used_m, mut used_t) = (vec![false; mine.len()], vec![false; theirs.len()]);
        for (d, i, j) in pairs {
            if used_m[i] || used_t[j] {
                continue;
            }
            used_m[i] = true;
            used_t[j] = true;
            if d <= ANCHOR_TOLERANCE_MM {
                matched += 1;
            } else {
                let (dx, dy) = (
                    mine[i].anchor.0 - theirs[j].anchor.0,
                    mine[i].anchor.1 - theirs[j].anchor.1,
                );
                let side = |v: f64, plus: &str, minus: &str| {
                    if v >= 0.0 {
                        plus.to_owned()
                    } else {
                        minus.to_owned()
                    }
                };
                let mut dev = Deviation::new(
                    page,
                    DeviationKind::TextMoved,
                    format!(
                        "the text is {:.1} mm {} and {:.1} mm {} of where the reference has it",
                        dx.abs(),
                        side(dx, "right", "left"),
                        dy.abs(),
                        side(dy, "below", "above")
                    ),
                );
                dev.text = Some(mine[i].text.trim().to_owned());
                dev.at_mm = Some([mine[i].anchor.0, mine[i].anchor.1]);
                out.push(dev);
            }
        }
        let mut leftover = |runs: &mut Vec<&TextRun>, used: &[bool], kind, detail: &str| {
            for (run, _) in runs.drain(..).zip(used).filter(|(_, u)| !**u) {
                let mut dev = Deviation::new(page, kind, detail.to_owned());
                dev.text = Some(run.text.trim().to_owned());
                dev.at_mm = Some([run.anchor.0, run.anchor.1]);
                out.push(dev);
            }
        };
        leftover(
            &mut theirs,
            &used_t,
            DeviationKind::TextMissing,
            "the reference has this text; the project does not",
        );
        leftover(
            &mut mine,
            &used_m,
            DeviationKind::TextExtra,
            "the project has this text; the reference does not",
        );
    }
    out.sort_by(|a, b| {
        let key = |d: &Deviation| d.at_mm.map_or((0.0, 0.0), |p| (p[1], p[0]));
        key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal)
    });
    (matched, out)
}

/// The frame cells of a page as pixel rectangles `[x0, y0, x1, y1]` with their label; the whole sheet when the page
/// has no frame grid.
fn cells_of(session: &Session, folio: &str, image: &RgbaImage) -> Vec<(Option<String>, [usize; 4])> {
    let px = |nm: i64, max: u32| {
        ((nm as f64 / 25_400_000.0 * f64::from(DPI)).round().max(0.0) as usize).min(max as usize)
    };
    let cells: Vec<_> = session
        .page_cells(folio)
        .map(|c| c.cells())
        .unwrap_or_default()
        .into_iter()
        .map(|(label, r)| {
            (
                Some(label.to_string()),
                [
                    px(r[0], image.width),
                    px(r[1], image.height),
                    px(r[2], image.width),
                    px(r[3], image.height),
                ],
            )
        })
        .filter(|(_, r)| r[2] > r[0] && r[3] > r[1])
        .collect();
    if cells.is_empty() {
        return vec![(None, [0, 0, image.width as usize, image.height as usize])];
    }
    cells
}

/// Whether each pixel of an RGBA image (premultiplied or straight, on opaque white) is ink.
fn ink_of(pixels: &[u8]) -> Vec<bool> {
    pixels
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| (u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000 < INK_BELOW)
        .collect()
}

/// Every pixel within `radius` (a square) of an ink pixel.
fn dilated(ink: &[bool], width: usize, height: usize, radius: usize) -> Vec<bool> {
    let mut rows = vec![false; ink.len()];
    for y in 0..height {
        let row = &ink[y * width..(y + 1) * width];
        // Running count of ink pixels in the window [x - radius, x + radius].
        let mut count = row[..radius.min(width - 1) + 1].iter().filter(|&&b| b).count();
        for x in 0..width {
            rows[y * width + x] = count > 0;
            if x + radius + 1 < width && row[x + radius + 1] {
                count += 1;
            }
            if x >= radius && row[x - radius] {
                count -= 1;
            }
        }
    }
    let mut out = vec![false; ink.len()];
    for x in 0..width {
        let mut count = (0..=radius.min(height - 1))
            .filter(|&y| rows[y * width + x])
            .count();
        for y in 0..height {
            out[y * width + x] = count > 0;
            if y + radius + 1 < height && rows[(y + radius + 1) * width + x] {
                count += 1;
            }
            if y >= radius && rows[(y - radius) * width + x] {
                count -= 1;
            }
        }
    }
    out
}

/// The raster deviations of one page and the number of cells compared: per cell the pixels that are ink on
/// one side with no ink within the anchor tolerance on the other, with the text boxes `masks` (millimetres) left out.
fn compare_rasters(
    page: &str,
    image: &RgbaImage,
    reference: &[u8],
    masks: &[[f64; 4]],
    cells: &[(Option<String>, [usize; 4])],
) -> (usize, Vec<Deviation>) {
    let (width, height) = (image.width as usize, image.height as usize);
    if reference.len() != image.pixels.len() || width == 0 || height == 0 {
        return (0, Vec::new());
    }
    let (mut mine, mut theirs) = (ink_of(&image.pixels), ink_of(reference));
    let per_mm = f64::from(DPI) / 25.4;
    for m in masks {
        let at = |v: f64, max: usize| ((v * per_mm).round().max(0.0) as usize).min(max);
        let (x0, x1) = (at(m[0] - MASK_PAD_MM, width), at(m[2] + MASK_PAD_MM, width));
        let (y0, y1) = (at(m[1] - MASK_PAD_MM, height), at(m[3] + MASK_PAD_MM, height));
        for y in y0..y1 {
            mine[y * width + x0..y * width + x1].fill(false);
            theirs[y * width + x0..y * width + x1].fill(false);
        }
    }
    let radius = (ANCHOR_TOLERANCE_MM * per_mm).ceil() as usize;
    let (near_mine, near_theirs) = (
        dilated(&mine, width, height, radius),
        dilated(&theirs, width, height, radius),
    );
    let mut out = Vec::new();
    for (label, r) in cells {
        let (mut only_mine, mut only_theirs) = (0usize, 0usize);
        for y in r[1]..r[3] {
            for x in r[0]..r[2] {
                let i = y * width + x;
                only_mine += usize::from(mine[i] && !near_theirs[i]);
                only_theirs += usize::from(theirs[i] && !near_mine[i]);
            }
        }
        if only_mine + only_theirs > CELL_PIXELS {
            let what = label
                .as_ref()
                .map_or_else(|| "the page".to_owned(), |l| format!("cell {l}"));
            let mut dev = Deviation::new(
                page,
                DeviationKind::RasterCell,
                format!(
                    "the drawing in {what} differs from the reference: {only_mine} dark pixels only in the project, \
                     {only_theirs} only in the reference"
                ),
            );
            dev.cell.clone_from(label);
            out.push(dev);
        }
    }
    (cells.len(), out)
}
