// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rasteriser: a display list in sheet nanometres onto an RGBA8 image (doc 09 §4.4).

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};

use ew_dl::ew_units::{Nm, Rect};
use ew_dl::{
    Brush, Cap, Color, DisplayList, FontKey, GeometryResolver, ItemKind, Join, MAX_INSTANCE_DEPTH,
    MITER_LIMIT, PathItem, ShapedRun, Stroke, Transform,
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
/// and images (not drawn yet: the IR's image bytes are placeholders until assets reach the scene).
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
        let zero_if = |extent: Nm, v: u64| if extent.0 <= 0 || options.dpi == 0 { 0 } else { v };
        return Err(RenderError::Size {
            width: zero_if(sheet.width, w),
            height: zero_if(sheet.height, h),
        });
    };

    let mut ctx = RenderContext::new_with(
        pw,
        ph,
        RenderSettings {
            level: Level::baseline(),
            num_threads: 0,
        },
    );
    let resources = Resources::new();
    let scale = f64::from(options.dpi) / NM_PER_INCH as f64;

    // The background is a full-size rectangle under everything.
    ctx.set_transform(Affine::IDENTITY);
    ctx.set_paint(paint_color(options.background));
    ctx.fill_rect(&kurbo::Rect::new(0.0, 0.0, f64::from(pw), f64::from(ph)));

    let mut painter = Painter {
        ctx,
        resources,
        scale,
        fonts,
        geometry,
        font_cache: BTreeMap::new(),
        missing: BTreeSet::new(),
        unresolved_instances: 0,
        skipped_images: 0,
    };
    painter.items(&list.items, Affine::IDENTITY, 0, 0);

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
    Ok((image, report))
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
    /// Pixels per nm.
    scale: f64,
    fonts: &'a dyn FontResolver,
    geometry: Option<&'a dyn GeometryResolver>,
    /// The font data per key for this call (`None`: the resolver did not have it).
    font_cache: BTreeMap<FontKey, Option<FontData>>,
    missing: BTreeSet<FontKey>,
    unresolved_instances: usize,
    skipped_images: usize,
}

impl Painter<'_> {
    /// Draws `items` under `cur`, the accumulated nm-space transform of the enclosing groups and
    /// instances; `depth` counts groups, `inst_depth` instances.
    fn items(&mut self, items: &[ew_dl::Item], cur: Affine, depth: usize, inst_depth: usize) {
        for item in items {
            match &item.kind {
                ItemKind::Path(p) => self.path(p, cur),
                ItemKind::Glyphs(run) => self.glyphs(run, cur),
                ItemKind::Image(_) => self.skipped_images += 1,
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
        Affine::scale(self.scale) * cur
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

    /// The font for `key`, built once per render call; `None` and recorded as missing if the resolver
    /// does not have it or its bytes do not open as a font face (validated here, once per key, with
    /// skrifa, the parser the glyph machinery uses).
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
            .set_transform(Affine::scale(k) * placed * Affine::scale(1.0 / k));
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
