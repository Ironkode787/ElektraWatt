// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The text of a PDF page as the migration verifier reads it (doc 14 §4.15, FR-IMP-021): every run of glyphs a
//! page draws, with its string, the anchor of its first glyph and its box, in millimetres from the top left of the
//! page; and the page labels of the file. hayro interprets the page into a device that keeps only the glyph runs.

use std::sync::Arc;

use hayro::hayro_interpret::font::GlyphRun;
use hayro::hayro_interpret::hayro_cmap::BfString;
use hayro::hayro_interpret::util::TransformExt;
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageDrawProps, InterpreterCache,
    InterpreterSettings, SoftMask, interpret_page,
};
use hayro::hayro_syntax::object::{Array, Dict, Object};
use hayro::kurbo::{Affine, BezPath, Point, Rect};

use crate::underlay::{PdfError, open};

/// Millimetres per PDF point.
const MM_PER_PT: f64 = 25.4 / 72.0;

/// One run of glyphs a page draws: a PDF's, or the project's own text on a sheet.
#[derive(Clone, PartialEq, Debug)]
pub struct TextRun {
    /// The characters the font's character map gives, in drawing order.
    pub text: String,
    /// The anchor: the origin of the first glyph, in millimetres from the page's top left.
    pub anchor: (f64, f64),
    /// The run's box `[x0, y0, x1, y1]` in millimetres from the page's top left.
    pub bounds: [f64; 4],
}

/// The text of one PDF page.
#[derive(Clone, PartialEq, Debug)]
pub struct PdfPageText {
    /// The page's width as it renders, in millimetres.
    pub width_mm: f64,
    /// The page's height as it renders, in millimetres.
    pub height_mm: f64,
    /// The runs in drawing order.
    pub runs: Vec<TextRun>,
}

/// Where the last glyph collected ended, to join the next glyph to its run.
struct Tail {
    /// The pen position after the glyph, in points.
    end: Point,
    /// The unit vector of the baseline.
    along: (f64, f64),
    /// The em in points.
    em: f64,
}

/// Collects the glyphs as runs; draws nothing. A glyph joins the run before it when it follows on the same baseline
/// within [`JOIN_GAP`] ems, however the producer cut its text operators: Qt's PDF engine draws every glyph on its own.
struct Collector {
    runs: Vec<TextRun>,
    tail: Option<Tail>,
}

/// How far, in ems, a glyph may start before or after the pen to continue a run (a word space is about a third).
const JOIN_GAP: (f64, f64) = (-0.15, 0.45);

impl<'a> Device<'a> for Collector {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}

    fn draw_glyph_run(&mut self, run: &GlyphRun<'_, 'a>, props: DrawProps<'a>, _: &DrawMode) {
        for g in run.glyphs() {
            let text = match g.as_unicode() {
                Some(BfString::Char(c)) => c.to_string(),
                Some(BfString::String(s)) => s,
                None => String::new(),
            };
            // Glyph space is 1000 units to the em; the glyph's box runs from its descent to above its cap.
            let m: Affine = props.transform * g.transform();
            let advance = f64::from(match &**g {
                hayro::hayro_interpret::font::Glyph::Outline(o) => o.advance_width().unwrap_or(500.0),
                hayro::hayro_interpret::font::Glyph::Type3(_) => 500.0,
            });
            let origin = m * Point::new(0.0, 0.0);
            let end = m * Point::new(advance, 0.0);
            let em = m.as_coeffs()[0].hypot(m.as_coeffs()[1]) * 1000.0;
            let length = (advance * em / 1000.0).max(1e-9);
            let along = ((end.x - origin.x) / length, (end.y - origin.y) / length);
            let mut bounds = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
            for (x, y) in [(0.0, -250.0), (advance, -250.0), (advance, 950.0), (0.0, 950.0)] {
                let c = m * Point::new(x, y);
                let (x, y) = (c.x * MM_PER_PT, c.y * MM_PER_PT);
                bounds = [
                    bounds[0].min(x),
                    bounds[1].min(y),
                    bounds[2].max(x),
                    bounds[3].max(y),
                ];
            }
            let continues = self.tail.as_ref().is_some_and(|t| {
                let (vx, vy) = (origin.x - t.end.x, origin.y - t.end.y);
                let gap = vx * t.along.0 + vy * t.along.1;
                let off = vx * t.along.1 - vy * t.along.0;
                let turn = along.0 * t.along.1 - along.1 * t.along.0;
                let same_way = along.0 * t.along.0 + along.1 * t.along.1 > 0.0;
                same_way
                    && turn.abs() < 0.01
                    && off.abs() < 0.15 * t.em
                    && (JOIN_GAP.0 * t.em..=JOIN_GAP.1 * t.em).contains(&gap)
            });
            match self.runs.last_mut().filter(|_| continues) {
                Some(last) => {
                    last.text.push_str(&text);
                    last.bounds = [
                        last.bounds[0].min(bounds[0]),
                        last.bounds[1].min(bounds[1]),
                        last.bounds[2].max(bounds[2]),
                        last.bounds[3].max(bounds[3]),
                    ];
                }
                None => self.runs.push(TextRun {
                    text,
                    anchor: (origin.x * MM_PER_PT, origin.y * MM_PER_PT),
                    bounds,
                }),
            }
            self.tail = Some(Tail { end, along, em });
        }
    }
}

/// The text of page `page` (1-based) of a PDF.
///
/// # Errors
///
/// [`PdfError`] for a file hayro does not read, too many pages or a page it does not have.
pub fn pdf_page_text(bytes: Arc<[u8]>, page: u32) -> Result<PdfPageText, PdfError> {
    let pdf = open(bytes)?;
    let pages = pdf.pages();
    let p = usize::try_from(page)
        .ok()
        .and_then(|i| i.checked_sub(1))
        .and_then(|i| pages.get(i))
        .ok_or(PdfError::NoPage {
            page,
            pages: pages.len(),
        })?;
    let (w, h) = p.render_dimensions();
    let cache = InterpreterCache::new();
    let mut context = Context::new(
        p.initial_transform(true).to_kurbo(),
        Rect::new(0.0, 0.0, f64::from(w), f64::from(h)),
        &cache,
        p.xref(),
        InterpreterSettings::default(),
    );
    let mut device = Collector {
        runs: Vec::new(),
        tail: None,
    };
    interpret_page(p, &mut context, &mut device);
    Ok(PdfPageText {
        width_mm: f64::from(w) * MM_PER_PT,
        height_mm: f64::from(h) * MM_PER_PT,
        runs: device.runs,
    })
}

/// The label of each page of a PDF from its `/PageLabels`: the range's prefix, then the page's number within the
/// range when the range numbers (as decimal, whatever the style); `None` for a page without a label. A flat
/// number tree only; a nested one gives no labels.
///
/// # Errors
///
/// [`PdfError`] for a file hayro does not read.
pub fn pdf_page_labels(bytes: Arc<[u8]>) -> Result<Vec<Option<String>>, PdfError> {
    let pdf = open(bytes)?;
    let count = pdf.pages().len();
    let mut labels = vec![None; count];
    let nums = pdf
        .xref()
        .get::<Dict<'_>>(pdf.xref().root_id())
        .and_then(|root| root.get::<Dict<'_>>("PageLabels"))
        .and_then(|tree| tree.get::<Array<'_>>("Nums"));
    let Some(nums) = nums else { return Ok(labels) };
    let mut ranges: Vec<(usize, String, Option<i64>)> = Vec::new();
    let mut items = nums.iter::<Object<'_>>();
    while let (Some(Object::Number(start)), Some(Object::Dict(d))) = (items.next(), items.next()) {
        let prefix = d
            .get::<hayro::hayro_syntax::object::String<'_>>("P")
            .map_or_else(String::new, |s| text_string(&s));
        let numbered = d.contains_key("S").then(|| d.get::<i64>("St").unwrap_or(1));
        ranges.push((
            usize::try_from(start.as_i64()).unwrap_or(usize::MAX),
            prefix,
            numbered,
        ));
    }
    ranges.sort_by_key(|r| r.0);
    for (i, label) in labels.iter_mut().enumerate() {
        if let Some((start, prefix, numbered)) = ranges.iter().rev().find(|r| r.0 <= i) {
            let number = numbered.map_or_else(String::new, |st| (st + (i - start) as i64).to_string());
            *label = Some(format!("{prefix}{number}"));
        }
    }
    Ok(labels)
}

/// A PDF text string: UTF-16BE after a byte order mark, else Latin-1 (PDFDocEncoding's printable part).
fn text_string(s: &[u8]) -> String {
    match s {
        [0xFE, 0xFF, rest @ ..] => {
            char::decode_utf16(rest.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])))
                .map(|r| r.unwrap_or('\u{FFFD}'))
                .collect()
        }
        _ => s.iter().map(|&b| char::from(b)).collect(),
    }
}
