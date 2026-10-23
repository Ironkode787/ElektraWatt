// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rasteriser: a display list in sheet nanometres onto an RGBA8 image (doc 09 §4.4).

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};

use ew_dl::ew_units::{Nm, Rect};
use ew_dl::{
    Brush, Cap, Color, DisplayList, FontKey, GeometryResolver, ImageFormat, ImageItem, ItemKind, Join,
    MAX_INSTANCE_DEPTH, MITER_LIMIT, PathItem, ShapedRun, Stroke, Transform,
};
use vello_cpu::kurbo::{self, Affine, Shape};
use vello_cpu::peniko::{Blob, Fill, FontData};
use vello_cpu::{Level, Pixmap, RenderContext, RenderSettings, Resources};

use crate::fonts::FontResolver;

/// Nanometres per inch.
const NM_PER_INCH: i128 = 25_400_000;

/// How deep groups may nest before the renderer skips the rest of a branch. A display list from the
/// scene nests a handful of levels; the bound keeps hostile input from exhausting the stack or the
/// rasteriser's layer stack.
const MAX_GROUP_DEPTH: usize = 128;

/// Sheet size in nm (paper), e.g. A4 landscape 297 mm × 210 mm.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SheetSize {
    /// Paper width.
    pub width: Nm,
    /// Paper height.
    pub height: Nm,
}

/// What to render at.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct RenderOptions {
    /// Dots per inch; the pixel size is `round_half_up(mm / 25.4 × dpi)` per axis, at least 1 (see
    /// [`pixel_size`]).
    pub dpi: u32,
    /// Background painted under everything (opaque white by default).
    pub background: Color,
}

impl Default for RenderOptions {
    /// 150 dpi on opaque white.
    fn default() -> Self {
        RenderOptions {
            dpi: 150,
            background: Color::WHITE,
        }
    }
}

/// Straight-alpha (NOT premultiplied) RGBA8, row-major, top row first.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct RgbaImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` bytes: r, g, b, a per pixel.
    pub pixels: Vec<u8>,
}

/// Why a sheet could not be rendered.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RenderError {
    /// A pixel dimension is 0 (the sheet has no positive extent, or `dpi` is 0) or exceeds `u16::MAX`
    /// (the rasteriser's limit). The fields are the offending [`pixel_size`], with 0 for an axis that
    /// has no positive extent.
    Size {
        /// Pixel width.
        width: u64,
        /// Pixel height.
        height: u64,
    },
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Size { width, height } => write!(
                f,
                "a {width} × {height} pixel image is outside 1..={} per side",
                u16::MAX
            ),
        }
    }
}

impl std::error::Error for RenderError {}

/// What the renderer skipped (never a panic): fonts the resolver did not have, unresolved instances,
/// and images it does not draw: a JPEG (no JPEG decoder in this renderer's dependency graph), an image without its
/// bytes (the export job resolves them from the asset store, FT-384), and bytes that do not decode as a PNG within
/// [`MAX_IMAGE_PIXELS`]. A PNG image is decoded with `png` and drawn stretched over its rectangle.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RenderReport {
    /// The fonts of skipped glyph runs, sorted and deduplicated.
    pub missing_fonts: Vec<FontKey>,
    /// Instances whose geometry the resolver did not have (or that nest deeper than
    /// [`MAX_INSTANCE_DEPTH`]), counted per drawn occurrence.
    pub unresolved_instances: usize,
    /// Image items not drawn, counted per drawn occurrence.
    pub skipped_images: usize,
}

/// The most pixels a decoded image may have (16 Mi pixels, 64 MiB as RGBA): a larger PNG is skipped, so that a small
/// file cannot decompress into an unbounded buffer.
pub const MAX_IMAGE_PIXELS: u64 = 16 * 1024 * 1024;

/// The pixmap of PNG `bytes` (premultiplied RGBA8), or `None` when they do not decode, a side exceeds the pixmap's
/// `u16` range or the image has more than [`MAX_IMAGE_PIXELS`].
fn decode_png(bytes: &[u8]) -> Option<Pixmap> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
    decoder.set_limits(png::Limits {
        bytes: usize::try_from(MAX_IMAGE_PIXELS * 4).unwrap_or(usize::MAX),
    });
    let mut reader = decoder.read_info().ok()?;
    let (w, h) = (reader.info().width, reader.info().height);
    if u64::from(w) * u64::from(h) > MAX_IMAGE_PIXELS {
        return None;
    }
    let (w16, h16) = (u16::try_from(w).ok()?, u16::try_from(h).ok()?);
    let mut buf = vec![0u8; reader.output_buffer_size()?];
    reader.next_frame(&mut buf).ok()?;
    let (color, depth) = reader.output_color_type();
    if depth != png::BitDepth::Eight {
        return None;
    }
    let rgba: Vec<u8> = match color {
        png::ColorType::Rgba => buf,
        png::ColorType::GrayscaleAlpha => buf
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[g, a]| [g, g, g, a])
            .collect(),
        _ => return None,
    };
    if rgba.len() != usize::from(w16) * usize::from(h16) * 4 {
        return None;
    }
    Some(Pixmap::from_parts(
        rgba,
        w16,
        h16,
        vello_cpu::PixelMetadata {
            may_have_transparency: true,
            alpha_type: vello_cpu::peniko::ImageAlphaType::Alpha,
        },
    ))
}

/// The pixel size of `sheet` at `dpi`: `(w_nm × dpi + 12_700_000) / 25_400_000` in integer arithmetic
/// (`i128`), each at least 1.
pub fn pixel_size(sheet: SheetSize, dpi: u32) -> (u64, u64) {
    (axis_pixels(sheet.width, dpi), axis_pixels(sheet.height, dpi))
}

/// One axis of [`pixel_size`], unclamped below (can be 0 or negative).
fn raw_axis_pixels(extent: Nm, dpi: u32) -> i128 {
    (i128::from(extent.0) * i128::from(dpi) + NM_PER_INCH / 2).div_euclid(NM_PER_INCH)
}

fn axis_pixels(extent: Nm, dpi: u32) -> u64 {
    u64::try_from(raw_axis_pixels(extent, dpi).max(1)).unwrap_or(u64::MAX)
}

/// Renders `list` (sheet coordinates) onto a [`pixel_size`] image. Paint order is item order.
/// Instances are drawn through `geometry` (their geometry under the instance transform, recursively
/// up to [`MAX_INSTANCE_DEPTH`]); without a resolver or when it returns `None` the instance is skipped
/// and counted.
///
/// Paths fill (non-zero) and then stroke; a stroke thinner than a device pixel keeps its true width
/// and is anti-aliased. Glyph outlines come from the bytes `fonts` returns, unhinted, never from
/// system fonts. The scalar-width (`Level::baseline`) u8 pipeline runs on one thread, so equal inputs
/// give equal bytes on every run of one build target (doc 09 §4.4).
///
/// A sheet with no positive extent, `dpi == 0`, or more than `u16::MAX` pixels on a side is a
/// [`RenderError::Size`]. Groups nested deeper than 128 levels are dropped.
pub fn render(
    list: &DisplayList,
    sheet: SheetSize,
    options: &RenderOptions,
    fonts: &dyn FontResolver,
    geometry: Option<&dyn GeometryResolver>,
) -> Result<(RgbaImage, RenderReport), RenderError> {
    let (w, h) = pixel_size(sheet, options.dpi);
    let degenerate = sheet.width.0 <= 0 || sheet.height.0 <= 0 || options.dpi == 0;
    let side = |v: u64| u16::try_from(v).ok().filter(|&v| v > 0);
    let (Some(pw), Some(ph), false) = (side(w), side(h), degenerate) else {
        return Err(degenerate_error(sheet, options.dpi, w, h));
    };
    let scale = f64::from(options.dpi) / NM_PER_INCH as f64;
    Ok(paint(
        list,
        None,
        Canvas {
            width: pw,
            height: ph,
            device: Affine::scale(scale),
            scale,
            background: options.background,
        },
        fonts,
        geometry,
    ))
}

/// The [`RenderError::Size`] of a sheet without extent or of `dpi == 0` (0 for such an axis), else of the pixel
/// size `(w, h)`.
fn degenerate_error(sheet: SheetSize, dpi: u32, w: u64, h: u64) -> RenderError {
    let zero_if = |extent: Nm, v: u64| if extent.0 <= 0 || dpi == 0 { 0 } else { v };
    RenderError::Size {
        width: zero_if(sheet.width, w),
        height: zero_if(sheet.height, h),
    }
}

/// The largest side of a [`render_region`] image.
const MAX_REGION_SIDE: u32 = 4096;

/// What [`render_region`] renders: a rectangle of the full-sheet raster at `dpi`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct RegionOptions {
    /// Dots per inch of the full-sheet raster the region is cut from (as [`RenderOptions::dpi`]).
    pub dpi: u32,
    /// The column of the full-sheet raster that becomes the region's first column; may be negative or beyond
    /// the sheet.
    pub x: i64,
    /// The row of the full-sheet raster that becomes the region's first row; may be negative or beyond the sheet.
    pub y: i64,
    /// The region's width in pixels, `1..=4096`.
    pub width: u32,
    /// The region's height in pixels, `1..=4096`.
    pub height: u32,
    /// Painted under everything inside the sheet (as [`RenderOptions::background`]).
    pub background: Color,
    /// Every pixel outside the sheet's [`pixel_size`] rectangle.
    pub surround: Color,
}

/// Renders a rectangle of the full-sheet raster: output pixel `(i, j)` is pixel `(x + i, y + j)` of
/// [`render`]'s image at `options.dpi` where that pixel lies inside [`pixel_size`]`(sheet, dpi)`, and
/// `options.surround` elsewhere; inside the sheet the background is `options.background`. The sheet may be larger
/// than [`render`]'s `u16::MAX` limit: only the region is rasterised.
///
/// The full rectangle (`x = y = 0` and the sheet's pixel size) gives the bytes of [`render`] exactly: the same
/// single-threaded `Level::baseline()` pipeline runs with the same transforms. Another rectangle rounds its
/// coordinates at another magnitude, so its pixels match the full raster's within anti-aliasing noise, not
/// necessarily byte for byte (WP-S2.02 §8).
///
/// With `bounds` (from [`ItemBounds::of`] on the same list and resolver), a top-level item whose inflated bounds
/// miss the region is skipped; culling never changes a pixel. Bounds made for a list with another number of items
/// are ignored.
///
/// # Errors
///
/// [`RenderError::Size`] when `width` or `height` is outside `1..=4096` (the fields are the region's size), or
/// for a sheet without extent or `dpi == 0` (as [`render`]).
pub fn render_region(
    list: &DisplayList,
    bounds: Option<&ItemBounds>,
    sheet: SheetSize,
    options: &RegionOptions,
    fonts: &dyn FontResolver,
    geometry: Option<&dyn GeometryResolver>,
) -> Result<(RgbaImage, RenderReport), RenderError> {
    let region_side = |v: u32| {
        u16::try_from(v)
            .ok()
            .filter(|&v| v > 0 && u32::from(v) <= MAX_REGION_SIDE)
    };
    let (Some(rw), Some(rh)) = (region_side(options.width), region_side(options.height)) else {
        return Err(RenderError::Size {
            width: u64::from(options.width),
            height: u64::from(options.height),
        });
    };
    let (sw, sh) = pixel_size(sheet, options.dpi);
    if sheet.width.0 <= 0 || sheet.height.0 <= 0 || options.dpi == 0 {
        return Err(degenerate_error(sheet, options.dpi, sw, sh));
    }
    let scale = f64::from(options.dpi) / NM_PER_INCH as f64;
    let device = Affine::translate((-(options.x as f64), -(options.y as f64))) * Affine::scale(scale);
    // Culling: every top-level item's reach, inflated by 2 device pixels, against the region in device pixels.
    let visible = bounds.filter(|b| b.items.len() == list.items.len()).map(|b| {
        let margin = 2.0 / scale;
        b.items
            .iter()
            .map(|reach| {
                reach.is_some_and(|r| {
                    let grow = r.pad + margin;
                    let on_device = device.transform_rect_bbox(r.rect.inflate(grow, grow));
                    on_device.x0 < f64::from(rw)
                        && on_device.x1 > 0.0
                        && on_device.y0 < f64::from(rh)
                        && on_device.y1 > 0.0
                })
            })
            .collect::<Vec<bool>>()
    });
    let (mut image, report) = paint(
        list,
        visible.as_deref(),
        Canvas {
            width: rw,
            height: rh,
            device,
            scale,
            background: options.background,
        },
        fonts,
        geometry,
    );
    // Pixels outside the sheet's raster are the surround, whatever was drawn there.
    let inside = |v: i64, n: u64| u64::try_from(v).is_ok_and(|v| v < n);
    let surround = [
        options.surround.r,
        options.surround.g,
        options.surround.b,
        options.surround.a,
    ];
    for (j, row) in image.pixels.chunks_exact_mut(usize::from(rw) * 4).enumerate() {
        let row_inside = inside(options.y.saturating_add(j as i64), sh);
        for (i, px) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if !(row_inside && inside(options.x.saturating_add(i as i64), sw)) {
                px.copy_from_slice(&surround);
            }
        }
    }
    Ok((image, report))
}

/// The bounds of the top-level items of a display list, for [`render_region`]'s culling.
///
/// Per top-level item: the box (in sheet nanometres) of everything the renderer draws for it (path control boxes,
/// glyph-run layout boxes and glyph origins, resolved instances, groups under their transforms and cut to their
/// clips; images are not drawn and add nothing) and how far ink can reach beyond that box: half the widest stroke
/// times the miter limit (a miter join reaches that far) and twice the em size of a glyph run (outlines reach
/// beyond their layout box). [`render_region`] adds 2 device pixels for anti-aliasing.
#[derive(Clone, PartialEq, Debug)]
pub struct ItemBounds {
    items: Vec<Option<Reach>>,
}

/// A box and how far beyond it ink reaches, in nm.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Reach {
    rect: kurbo::Rect,
    pad: f64,
}

impl ItemBounds {
    /// The bounds of every top-level item of `list`, resolving instances through `geometry` as [`render`] does.
    pub fn of(list: &DisplayList, geometry: Option<&dyn GeometryResolver>) -> ItemBounds {
        ItemBounds {
            items: list
                .items
                .iter()
                .map(|item| {
                    let mut acc = Acc::default();
                    reach_of(
                        std::slice::from_ref(item),
                        Affine::IDENTITY,
                        0,
                        0,
                        geometry,
                        &mut acc,
                    );
                    acc.rect.map(|rect| Reach { rect, pad: acc.pad })
                })
                .collect(),
        }
    }
}

/// The running box and reach of one top-level item.
#[derive(Default)]
struct Acc {
    rect: Option<kurbo::Rect>,
    pad: f64,
}

impl Acc {
    fn add(&mut self, r: kurbo::Rect) {
        self.rect = Some(self.rect.map_or(r, |a| a.union(r)));
    }
}

/// The walk of [`ItemBounds::of`]; it follows [`Painter::items`], and what that skips adds nothing.
fn reach_of(
    items: &[ew_dl::Item],
    cur: Affine,
    depth: usize,
    inst_depth: usize,
    geometry: Option<&dyn GeometryResolver>,
    acc: &mut Acc,
) {
    for item in items {
        match &item.kind {
            ItemKind::Path(p) => {
                if p.path.is_empty() {
                    continue;
                }
                acc.add(cur.transform_rect_bbox(p.path.to_kurbo().control_box()));
                if let Some(stroke) = &p.stroke
                    && stroke.width.0 > 0
                {
                    acc.pad = acc.pad.max(stroke.width.0 as f64 / 2.0 * MITER_LIMIT.max(2.0));
                }
            }
            ItemKind::Glyphs(run) => {
                if run.size.0 <= 0 || run.glyphs.is_empty() {
                    continue;
                }
                for q in run.bound_points() {
                    let p = cur * kurbo::Point::new(q.x.0 as f64, q.y.0 as f64);
                    acc.add(kurbo::Rect::from_points(p, p));
                }
                acc.pad = acc.pad.max(2.0 * run.size.0 as f64);
            }
            ItemKind::Image(_) => {}
            ItemKind::Instance(inst) => {
                let geom = geometry
                    .filter(|_| inst_depth < MAX_INSTANCE_DEPTH)
                    .and_then(|g| g.geometry(inst.symbol));
                if let Some(dl) = geom {
                    let t = cur * inst.transform.to_affine();
                    reach_of(&dl.items, t, depth, inst_depth + 1, geometry, acc);
                }
            }
            ItemKind::Group(g) => {
                if depth >= MAX_GROUP_DEPTH {
                    continue;
                }
                let t = cur * g.transform.unwrap_or(Transform::IDENTITY).to_affine();
                match g.clip {
                    None => reach_of(&g.items, t, depth + 1, inst_depth, geometry, acc),
                    Some(clip) => {
                        // Nothing inside reaches beyond the clip: the content's inflated box, cut to it.
                        let mut inner = Acc::default();
                        reach_of(&g.items, t, depth + 1, inst_depth, geometry, &mut inner);
                        if let Some(r) = inner.rect {
                            let r = r.inflate(inner.pad, inner.pad);
                            let cut = r.intersect(t.transform_rect_bbox(nm_rect(clip)));
                            if cut.width() >= 0.0 && cut.height() >= 0.0 {
                                acc.add(cut);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The pixel buffer one [`paint`] call draws into.
struct Canvas {
    width: u16,
    height: u16,
    /// Sheet nm to device pixels.
    device: Affine,
    /// Pixels per nm (the scale part of `device`).
    scale: f64,
    background: Color,
}

/// Draws the top-level items of `list` that `visible` lets through (all of them without it) onto `canvas`: the
/// one pipeline of [`render`] and [`render_region`].
fn paint(
    list: &DisplayList,
    visible: Option<&[bool]>,
    canvas: Canvas,
    fonts: &dyn FontResolver,
    geometry: Option<&dyn GeometryResolver>,
) -> (RgbaImage, RenderReport) {
    let Canvas {
        width: pw,
        height: ph,
        device,
        scale,
        background,
    } = canvas;
    let mut ctx = RenderContext::new_with(
        pw,
        ph,
        RenderSettings {
            level: Level::baseline(),
            num_threads: 0,
        },
    );
    let resources = Resources::new();

    // The background is a full-size rectangle under everything.
    ctx.set_transform(Affine::IDENTITY);
    ctx.set_paint(paint_color(background));
    ctx.fill_rect(&kurbo::Rect::new(0.0, 0.0, f64::from(pw), f64::from(ph)));

    let mut painter = Painter {
        ctx,
        resources,
        device,
        scale,
        fonts,
        geometry,
        font_cache: BTreeMap::new(),
        missing: BTreeSet::new(),
        unresolved_instances: 0,
        skipped_images: 0,
        images: BTreeMap::new(),
    };
    match visible {
        None => painter.items(&list.items, Affine::IDENTITY, 0, 0),
        Some(visible) => {
            for (item, _) in list.items.iter().zip(visible).filter(|(_, v)| **v) {
                painter.items(std::slice::from_ref(item), Affine::IDENTITY, 0, 0);
            }
        }
    }

    let Painter {
        mut ctx,
        mut resources,
        missing,
        unresolved_instances,
        skipped_images,
        ..
    } = painter;
    ctx.flush();
    let mut pixmap = Pixmap::new(pw, ph);
    ctx.render(&mut pixmap, &mut resources);

    let image = RgbaImage {
        width: u32::from(pw),
        height: u32::from(ph),
        pixels: unpremultiply(pixmap.data_as_u8_slice()),
    };
    let report = RenderReport {
        missing_fonts: missing.into_iter().collect(),
        unresolved_instances,
        skipped_images,
    };
    (image, report)
}

/// Premultiplied RGBA8 to straight alpha with exact integer rounding.
fn unpremultiply(premul: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(premul.len());
    for px in premul.as_chunks::<4>().0 {
        let a = u32::from(px[3]);
        // Fully transparent pixels become (0, 0, 0, 0): the division yields nothing for a == 0.
        for &c in &px[..3] {
            let v = (u32::from(c) * 255 + a / 2).checked_div(a).unwrap_or(0);
            out.push(v.min(255) as u8);
        }
        out.push(px[3]);
    }
    out
}

fn paint_color(c: Color) -> vello_cpu::color::AlphaColor<vello_cpu::color::Srgb> {
    vello_cpu::color::AlphaColor::from_rgba8(c.r, c.g, c.b, c.a)
}

fn brush_color(b: Brush) -> Color {
    match b {
        Brush::Solid(c) => c,
    }
}

fn nm_rect(r: Rect) -> kurbo::Rect {
    let (a, b) = (r.min(), r.max());
    kurbo::Rect::new(a.x.0 as f64, a.y.0 as f64, b.x.0 as f64, b.y.0 as f64)
}

fn kurbo_stroke(s: &Stroke) -> kurbo::Stroke {
    let cap = match s.cap {
        Cap::Butt => kurbo::Cap::Butt,
        Cap::Round => kurbo::Cap::Round,
        Cap::Square => kurbo::Cap::Square,
    };
    let join = match s.join {
        Join::Miter => kurbo::Join::Miter,
        Join::Round => kurbo::Join::Round,
        Join::Bevel => kurbo::Join::Bevel,
    };
    let mut k = kurbo::Stroke::new(s.width.0 as f64)
        .with_caps(cap)
        .with_join(join)
        .with_miter_limit(MITER_LIMIT);
    // A dash array that is empty, has a non-positive entry or sums to zero would not advance along
    // the path; such a stroke is drawn solid.
    if let Some(d) = &s.dash
        && !d.segments.is_empty()
        && d.segments.iter().all(|n| n.0 > 0)
    {
        k = k.with_dashes(d.offset.0 as f64, d.segments.iter().map(|n| n.0 as f64));
    }
    k
}

/// The drawing state of one `render` call.
struct Painter<'a> {
    ctx: RenderContext,
    resources: Resources,
    /// Sheet nm to device pixels: the scale, and for a region the shift to its first pixel.
    device: Affine,
    /// Pixels per nm.
    scale: f64,
    fonts: &'a dyn FontResolver,
    geometry: Option<&'a dyn GeometryResolver>,
    /// The font data per key for this call (`None`: the resolver did not have it).
    font_cache: BTreeMap<FontKey, Option<FontData>>,
    missing: BTreeSet<FontKey>,
    unresolved_instances: usize,
    skipped_images: usize,
    /// The decoded PNG images of this call, by content hash (`None`: they do not decode).
    images: BTreeMap<[u8; 32], Option<std::sync::Arc<Pixmap>>>,
}

impl Painter<'_> {
    /// Draws `items` under `cur`, the accumulated nm-space transform of the enclosing groups and
    /// instances; `depth` counts groups, `inst_depth` instances.
    fn items(&mut self, items: &[ew_dl::Item], cur: Affine, depth: usize, inst_depth: usize) {
        for item in items {
            match &item.kind {
                ItemKind::Path(p) => self.path(p, cur),
                ItemKind::Glyphs(run) => self.glyphs(run, cur),
                ItemKind::Image(img) => self.image(img, cur),
                ItemKind::Instance(inst) => {
                    let geom = self
                        .geometry
                        .filter(|_| inst_depth < MAX_INSTANCE_DEPTH)
                        .and_then(|g| g.geometry(inst.symbol));
                    match geom {
                        Some(dl) => {
                            let t = cur * inst.transform.to_affine();
                            self.items(&dl.items, t, depth, inst_depth + 1);
                        }
                        None => self.unresolved_instances += 1,
                    }
                }
                ItemKind::Group(g) => {
                    if depth >= MAX_GROUP_DEPTH {
                        continue;
                    }
                    let t = cur * g.transform.unwrap_or(Transform::IDENTITY).to_affine();
                    if let Some(clip) = g.clip {
                        self.ctx.set_transform(self.pixel_transform(t));
                        self.ctx.push_clip_layer(&nm_rect(clip).to_path(0.1));
                    }
                    self.items(&g.items, t, depth + 1, inst_depth);
                    if g.clip.is_some() {
                        self.ctx.pop_layer();
                    }
                }
            }
        }
    }

    /// Nm space under `cur` to device pixels.
    fn pixel_transform(&self, cur: Affine) -> Affine {
        self.device * cur
    }

    fn path(&mut self, item: &PathItem, cur: Affine) {
        if item.path.is_empty() {
            return;
        }
        let bez = item.path.to_kurbo();
        self.ctx.set_transform(self.pixel_transform(cur));
        if let Some(brush) = item.fill {
            self.ctx.set_fill_rule(Fill::NonZero);
            self.ctx.set_paint(paint_color(brush_color(brush)));
            self.ctx.fill_path(&bez);
        }
        if let Some(stroke) = &item.stroke
            && stroke.width.0 > 0
        {
            self.ctx.set_stroke(kurbo_stroke(stroke));
            self.ctx.set_paint(paint_color(brush_color(stroke.brush)));
            self.ctx.stroke_path(&bez);
        }
    }

    /// A PNG image stretched over its rectangle (the pixel grid mapped onto it); anything else is counted as skipped
    /// ([`RenderReport::skipped_images`]).
    fn image(&mut self, img: &ImageItem, cur: Affine) {
        let pixmap = match img.image.format {
            ImageFormat::Png if !img.image.bytes.is_empty() => self
                .images
                .entry(img.image.sha256)
                .or_insert_with(|| decode_png(&img.image.bytes).map(std::sync::Arc::new))
                .clone(),
            _ => None,
        };
        let Some(pixmap) = pixmap else {
            self.skipped_images += 1;
            return;
        };
        let r = nm_rect(img.rect);
        if r.width() <= 0.0 || r.height() <= 0.0 {
            return;
        }
        let (w, h) = (f64::from(pixmap.width()), f64::from(pixmap.height()));
        self.ctx.set_transform(self.pixel_transform(cur));
        self.ctx.set_paint_transform(
            Affine::translate((r.x0, r.y0)) * Affine::scale_non_uniform(r.width() / w, r.height() / h),
        );
        self.ctx.set_paint(vello_cpu::Image {
            image: vello_cpu::ImageSource::Pixmap(pixmap),
            sampler: vello_cpu::peniko::ImageSampler::default(),
        });
        self.ctx.fill_rect(&r);
        self.ctx.reset_paint_transform();
    }

    /// The font for `key`, built once per render call; `None` and recorded as missing if the resolver
    /// does not have it or its bytes do not open as a font face (validated here, once per key, with the
    /// crate's own skrifa; glifo parses with its own, older skrifa, so a face that opens here but not there is
    /// still caught by the `catch_unwind` backstop below).
    fn font(&mut self, key: FontKey) -> Option<FontData> {
        if let Some(f) = self.font_cache.get(&key) {
            return f.clone();
        }
        let font = self
            .fonts
            .font(key)
            .filter(|b| skrifa::FontRef::from_index(&b.data, b.index).is_ok())
            .map(|b| FontData::new(Blob::new(std::sync::Arc::new(b.data)), b.index));
        if font.is_none() {
            self.missing.insert(key);
        }
        self.font_cache.insert(key, font.clone());
        font
    }

    /// Glyph outlines are drawn at a font size in device pixels (`size × scale`) with glyph positions
    /// scaled the same way; the context transform keeps only the run's placement (translation and
    /// the orthonormal turn/mirror), so the outlines are never scaled by a nm-sized factor.
    fn glyphs(&mut self, run: &ShapedRun, cur: Affine) {
        if run.size.0 <= 0 || run.glyphs.is_empty() {
            return;
        }
        let Some(font) = self.font(run.font) else {
            return;
        };
        let placed = cur
            * Affine::translate((run.origin.x.0 as f64, run.origin.y.0 as f64))
            * run.transform.to_affine();
        let k = self.scale;
        self.ctx
            .set_transform(self.device * placed * Affine::scale(1.0 / k));
        self.ctx.set_paint(paint_color(brush_color(run.brush)));
        let glyphs = run.glyphs.iter().map(|g| vello_cpu::Glyph {
            id: g.id,
            x: (g.x.0 as f64 * k) as f32,
            y: (g.y.0 as f64 * k) as f32,
        });
        // Glyphs the font cannot draw come back as an error and stay blank; nothing else to do. The font
        // file itself was validated in `font()`; the `catch_unwind` below is a backstop for failures
        // inside the glyph machinery at glyph level, which then count the font as missing. It only
        // works with `panic = "unwind"` (the workspace profiles do not set `panic = "abort"`).
        let ctx = &mut self.ctx;
        let resources = &mut self.resources;
        let drawn = catch_unwind(AssertUnwindSafe(|| {
            let _ = ctx
                .glyph_run(resources, &font)
                .font_size((run.size.0 as f64 * k) as f32)
                .hint(false)
                .fill_glyphs(glyphs);
        }));
        if drawn.is_err() {
            self.missing.insert(run.font);
            self.font_cache.insert(run.font, None);
        }
    }
}
