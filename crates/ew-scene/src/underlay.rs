// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Underlays drawn (doc 14 §5.5, FR-IMP-030): a PDF page rasterised with hayro into a PNG stretched over the
//! underlay's rectangle, and a DXF drawing as the page items the DXF importer makes of it, both at the underlay's
//! opacity, in grey. Nothing here carries an entity: an underlay is locked, no pick lands on it.
//!
//! The PDF is read under [`MAX_PDF_PAGES`] pages; the raster is [`RASTER_DPI`] dots per inch of the rectangle, each
//! side at most [`MAX_RASTER_SIDE`] pixels. The callers run these inside the import front door or cache their result
//! by the source's hash (a Class III cache): rendering is the expensive part.

use std::sync::Arc;

use ew_dl::{Brush, Color, DisplayList, EntityRef, ImageData, ImageFormat, Item, ItemKind, Stroke};
use ew_schema::{FreeText, GraphicShape, LineDash, PageItemKind};
use ew_text::{DEFAULT_STYLE_KEY, FontSet, TextStyleTable};
use ew_units::{Nm, Orient2, Pt2, Rect};

use crate::items::{shape_path, text_align};
use crate::page::{Builder, TextSpec};
use crate::symbol;

/// The most pages a PDF underlay's source may have (doc 14 §11).
pub const MAX_PDF_PAGES: usize = 10_000;

/// The resolution of a PDF underlay's raster.
pub const RASTER_DPI: f64 = 150.0;

/// The largest side of a PDF underlay's raster, in pixels.
pub const MAX_RASTER_SIDE: u32 = 4096;

/// Why a PDF underlay's source cannot be drawn.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PdfError {
    /// The bytes are no PDF hayro reads (damaged, encrypted).
    Unreadable,
    /// More pages than [`MAX_PDF_PAGES`].
    TooManyPages(usize),
    /// The page asked for is not in the file (1-based).
    NoPage {
        /// The page asked for.
        page: u32,
        /// The file's pages.
        pages: usize,
    },
}

impl std::fmt::Display for PdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfError::Unreadable => f.write_str("the file is no PDF that can be read (damaged or encrypted)"),
            PdfError::TooManyPages(n) => {
                write!(f, "the PDF has {n} pages, more than the {MAX_PDF_PAGES} read")
            }
            PdfError::NoPage { page, pages } => write!(f, "the PDF has no page {page}; it has {pages}"),
        }
    }
}

impl std::error::Error for PdfError {}

/// Shared source bytes as hayro takes them (without a copy).
struct Source(Arc<[u8]>);

impl AsRef<[u8]> for Source {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

pub(crate) fn open(bytes: Arc<[u8]>) -> Result<hayro::hayro_syntax::Pdf, PdfError> {
    let pdf = hayro::hayro_syntax::Pdf::new(Arc::new(Source(bytes))).map_err(|_| PdfError::Unreadable)?;
    let pages = pdf.pages().len();
    if pages > MAX_PDF_PAGES {
        return Err(PdfError::TooManyPages(pages));
    }
    Ok(pdf)
}

/// The page count of a PDF and the size of its page `page` (1-based) in points, as it renders (rotation applied).
///
/// # Errors
///
/// [`PdfError`] for a file hayro does not read, too many pages or a page it does not have.
pub fn pdf_page_size(bytes: Arc<[u8]>, page: u32) -> Result<(usize, f32, f32), PdfError> {
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
    Ok((pages.len(), w, h))
}

/// The pixel size of the raster of a rectangle of `width` × `height` at [`RASTER_DPI`], each side within
/// 1 … [`MAX_RASTER_SIDE`], the aspect kept.
pub fn raster_size(width: Nm, height: Nm) -> (u32, u32) {
    let px = |n: Nm| n.0.max(0) as f64 / 25_400_000.0 * RASTER_DPI;
    let (w, h) = (px(width).max(1.0), px(height).max(1.0));
    let k = (f64::from(MAX_RASTER_SIDE) / w.max(h)).min(1.0);
    (((w * k).round() as u32).max(1), ((h * k).round() as u32).max(1))
}

/// Page `page` (1-based) of a PDF rasterised in `px_w` × `px_h` pixels on white: premultiplied RGBA.
///
/// # Errors
///
/// As [`pdf_page_size`].
pub fn pdf_pixmap(
    bytes: Arc<[u8]>,
    page: u32,
    (px_w, px_h): (u32, u32),
) -> Result<hayro::vello_cpu::Pixmap, PdfError> {
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
    let settings = hayro::PixmapSettings {
        x_scale: px_w.min(MAX_RASTER_SIDE) as f32 / w.max(1.0),
        y_scale: px_h.min(MAX_RASTER_SIDE) as f32 / h.max(1.0),
        bg_color: hayro::vello_cpu::color::palette::css::WHITE,
    };
    let cache = hayro::RenderCache::new();
    Ok(hayro::render(
        p,
        &cache,
        &hayro::hayro_interpret::InterpreterSettings::default(),
        &hayro::RenderSettings::default(),
        &settings,
    ))
}

/// A PNG of page `page` of a PDF in `px_w` × `px_h` pixels, on white, faded to `opacity` per cent.
///
/// # Errors
///
/// As [`pdf_page_size`].
pub fn pdf_png(bytes: Arc<[u8]>, page: u32, px: (u32, u32), opacity: u8) -> Result<Vec<u8>, PdfError> {
    let mut pixmap = pdf_pixmap(bytes, page, px)?;
    // Premultiplied RGBA: every channel scaled fades the page into the sheet.
    let a = u16::from(opacity.clamp(1, 100));
    for b in pixmap.data_as_u8_slice_mut() {
        *b = ((u16::from(*b) * a + 50) / 100) as u8;
    }
    pixmap.into_png().map_err(|_| PdfError::Unreadable)
}

/// The image item of a PDF underlay's raster `png` over `rect`: its bytes inline, its hash the raster's.
pub fn pdf_item(png: Arc<[u8]>, sha256: [u8; 32], rect: Rect) -> Item {
    Item::image(
        Arc::new(ImageData {
            sha256,
            format: ImageFormat::Png,
            bytes: png,
        }),
        rect,
    )
}

/// The grey of an underlay at `opacity` per cent.
fn grey(opacity: u8) -> Color {
    let a = (u16::from(opacity.clamp(1, 100)) * 255 / 100) as u8;
    Color::from_rgba8(0x60, 0x60, 0x60, a)
}

/// The display list of a DXF underlay: the page items the importer made of its drawing, moved by `offset`, in grey at
/// `opacity` per cent; texts in their styles of `styles`.
pub fn dxf_list(
    items: &[PageItemKind],
    offset: Pt2,
    opacity: u8,
    fonts: &FontSet,
    styles: &TextStyleTable,
) -> DisplayList {
    let colour = grey(opacity);
    let moved = |p: Pt2| p + offset;
    let mut b = Builder::new(fonts, styles);
    for kind in items {
        match kind {
            PageItemKind::Graphic(g) => {
                let shape = move_shape(&g.shape, &moved);
                let Some((path, closed)) = shape_path(&shape) else {
                    continue;
                };
                let width = g.stroke.as_ref().map_or(Nm(180_000), |s| s.width);
                let base = Stroke::solid(width).with_brush(Brush::Solid(colour));
                let stroke = match g.dash {
                    LineDash::Dashed => base.dashed(ew_dl::DashPattern::Dash42),
                    LineDash::Dotted => base.dashed(ew_dl::DashPattern::Dot12),
                    LineDash::DashDot => base.dashed(ew_dl::DashPattern::DashDot),
                    LineDash::Solid | LineDash::Unknown(_) => base,
                };
                let fill = (closed && g.fill).then_some(Brush::Solid(colour));
                b.push(Item::path(path, Some(stroke), fill));
            }
            PageItemKind::FreeText(t) => text(&mut b, t, &moved),
            _ => {}
        }
    }
    let mut list = b.finish().list;
    for item in &mut list.items {
        unlock(item, colour);
    }
    list
}

fn text(b: &mut Builder<'_>, t: &FreeText, moved: &dyn Fn(Pt2) -> Pt2) {
    let Some(content) = t.text.as_neutral() else {
        return;
    };
    let align = text_align(t.align.as_ref());
    b.text(&TextSpec {
        text: content,
        at: moved(t.at),
        angle: symbol::readable_angle(t.angle, Orient2::R0),
        align: &symbol::placed_align(&align, t.angle, Orient2::R0),
        style_key: t.style.as_ref().map_or(DEFAULT_STYLE_KEY, |s| s.as_str()),
        entity: EntityRef(ew_ids::Uuid::nil()),
        link: None,
        items: &[],
    });
}

/// The item without an entity (an underlay is never picked), its glyphs in the underlay's grey.
fn unlock(item: &mut Item, colour: Color) {
    item.entity = None;
    match &mut item.kind {
        ItemKind::Glyphs(run) => Arc::make_mut(run).brush = Brush::Solid(colour),
        ItemKind::Group(g) => g.items.iter_mut().for_each(|i| unlock(i, colour)),
        _ => {}
    }
}

fn move_shape(shape: &GraphicShape, moved: &dyn Fn(Pt2) -> Pt2) -> GraphicShape {
    let mut s = shape.clone();
    match &mut s {
        GraphicShape::Line(l) => {
            l.a = moved(l.a);
            l.b = moved(l.b);
        }
        GraphicShape::Polyline(p) | GraphicShape::Polygon(p) => {
            p.points.iter_mut().for_each(|q| *q = moved(*q))
        }
        GraphicShape::Rect(r) => {
            r.min = moved(r.min);
            r.max = moved(r.max);
        }
        GraphicShape::Circle(c) => c.centre = moved(c.centre),
        GraphicShape::Arc(a) => a.centre = moved(a.centre),
        GraphicShape::Unknown(_) => {}
    }
    s
}
