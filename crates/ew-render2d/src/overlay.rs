// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The canvas overlays (WP-S2.02 §6.4, WP-S2.03 §7.4): the surround, the page bounds, the grid dots, the hover and
//! selection outlines, the tool overlay (lines, polylines, rectangles, the ghost of a drag and the snap markers) and
//! the keyboard grid cursor, as rectangles of whole physical pixels of the pane.
//!
//! One code path for every backend: the egui host paints the rectangles as shapes over the tiles (the dots as one
//! mesh), and the headless snapshot composites them into its image with integer pixel fills ([`composite`]), so
//! the window and the snapshot agree pixel for pixel. No meaning is in colour alone: the hover is dashed, a
//! selection solid, the keyboard grid cursor a crosshair with a gap at its centre, a snap marker a cross (grid), a
//! ring (connection point) or a filled square (junction), a free connection point a small hollow square (FT-705), a
//! rubber band solid while it encloses and dashed while it touches. The paint order is the surround, the page bounds,
//! the grid dots, the hover, the selection, the open markers of the free connection points, the tool's lines and
//! ghosts, the snap markers and last the cursor. The tool overlay's text labels are not drawn here: the
//! window draws them as chips in its own font, and the headless snapshot leaves them out (WP-S2.03 §7.4).
//!
//! **FT-391's scale rule** (WP-S2.17 part 5): every mark scales by the integer factor `s = max(1, ⌊ppp⌋)`
//! ([`mark_scale`]): lines are `s` pixels wide with dashes of `4s` on and `4s` off, the grid cross and the junction
//! square are `6s + 1` pixels, the CP ring `8s + 1`, the hover outline `s`, the selection outline `2s` with an
//! `s`-pixel white casing on both sides and `8s`-pixel corner handles (white with an accent rim), and the grid cursor's
//! arms `s` wide from `2s` to `8s` off its centre. At ppp 1 every mark keeps its size as built before part 5.
//!
//! The colours come from the caller's [`Theme`]: the window and the snapshot build it from the design language's
//! canvas tokens (doc 16a §4.6); [`LIGHT`] and [`DARK`] hold the same values for callers without the token module.
//! The sheet itself is white with its authored colours in every theme (FR-UI-043, doc 16 §11.2).

use ew_dl::Color;
use ew_dl::ew_units::Rect;
use ew_render_cpu::{RgbaImage, SheetSize, pixel_size};

use crate::view::{DrawTransform, PxRect, round_px};

/// The colours of the canvas around and over the sheet.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Theme {
    /// Outside the sheet.
    pub surround: Color,
    /// The 1-pixel outline of the sheet.
    pub border: Color,
    /// The grid dots.
    pub grid_dot: Color,
    /// The dashed hover outline (doc 16 §10.3).
    pub hover: Color,
    /// The solid selection outline (doc 16 §10.3).
    pub selection: Color,
    /// The keyboard grid cursor's crosshair (doc 16a §7.2: the accent; the crosshair's shape carries the meaning).
    pub cursor: Color,
    /// The sheet (white in every theme).
    pub paper: Color,
    /// The geometric ghost of a drag (`canvas.ghost`, the accent at 60 %).
    pub ghost: Color,
    /// The snap markers (`canvas.snap`; the marker's shape carries the meaning).
    pub snap: Color,
    /// The Measure tool's line (`canvas.measure`).
    pub measure: Color,
    /// The rubber band of a selection (the accent).
    pub rubber_band: Color,
    /// The selection's casing and the faces of its handles (white, doc 16a §7.1).
    pub casing: Color,
    /// The open marker of a connection point without a connection (`canvas.open_cp`, FT-705).
    pub open_cp: Color,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

/// The accent `volt-600` #0B63CE (5.69:1 on white).
const ACCENT: Color = rgb(0x0B, 0x63, 0xCE);

/// The light theme's canvas tokens (doc 16a §4.6: the values of `ew-ui`'s token module).
pub const LIGHT: Theme = Theme {
    surround: rgb(0xE3, 0xE6, 0xEA),
    border: rgb(0xD5, 0xD9, 0xDF),
    grid_dot: rgb(0xB8, 0xBE, 0xC7),
    hover: ACCENT,
    selection: ACCENT,
    cursor: ACCENT,
    paper: rgb(0xFF, 0xFF, 0xFF),
    ghost: Color {
        r: 0x0B,
        g: 0x63,
        b: 0xCE,
        a: 153,
    },
    snap: ACCENT,
    measure: rgb(0x1A, 0x1D, 0x21),
    rubber_band: ACCENT,
    casing: rgb(0xFF, 0xFF, 0xFF),
    open_cp: ACCENT,
};

/// The dark theme's canvas tokens (doc 16a §4.6); the sheet and the marks on it stay as in [`LIGHT`].
pub const DARK: Theme = Theme {
    surround: rgb(0x2E, 0x32, 0x38),
    border: rgb(0x3F, 0x45, 0x4D),
    ..LIGHT
};

/// FT-391's integer scale factor of the overlay marks at `ppp` physical pixels per point: `max(1, ⌊ppp⌋)`, at most
/// 16 (a value that is not finite counts as 1).
pub fn mark_scale(ppp: f64) -> i64 {
    if ppp.is_finite() && ppp >= 1.0 {
        (ppp.floor() as i64).min(16)
    } else {
        1
    }
}

/// The smallest spacing of grid dots on screen, in logical points: below it the drawn pitch doubles.
pub const MIN_DOT_SPACING_PT: f64 = 8.0;

/// The hover and selection outlines stand this many logical points outside the entity's rectangle.
pub const OUTLINE_OUTSET_PT: f64 = 2.0;

/// The dash of the hover outline at scale 1: this many pixels on, as many off (`4s` at scale `s`, FT-391).
pub const HOVER_DASH_PX: i64 = 4;

/// The dash of a tool line at scale 1: this many pixels on, as many off (WP-S2.03 §7.4; `4s` at scale `s`).
pub const TOOL_DASH_PX: i64 = 4;

/// The side of a selection handle at scale 1 (doc 16a §7.1: `8s`).
pub const HANDLE_PX: i64 = 8;

/// The smallest step of the connection grid on screen, in logical points, at which the open markers of the free
/// connection points are drawn (FT-705): 4 points, so that they show at the fit zoom of an A3 or A4 page in a usual
/// window (QET's 2.6 mm step is about 6.8 points at 69 %) and not on a page shrunk to a thumbnail.
pub const OPEN_CP_MIN_STEP_PT: f64 = 4.0;

/// The side of the open marker of a free connection point at scale 1 (FT-705: a `4s + 1`-pixel hollow square with an
/// `s`-pixel rim; 5 pixels at scale 1).
pub const OPEN_CP_PX: i64 = 5;

/// The colour token of a tool line (WP-S2.03 §7.4; provisional until WP-S2.08's theme).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolToken {
    /// The ghost of a drag.
    Ghost,
    /// A snap indication.
    Snap,
    /// The Measure tool.
    Measure,
    /// A rubber band.
    RubberBand,
}

impl Theme {
    /// The colour of a tool token.
    pub fn token(&self, t: ToolToken) -> Color {
        match t {
            ToolToken::Ghost => self.ghost,
            ToolToken::Snap => self.snap,
            ToolToken::Measure => self.measure,
            ToolToken::RubberBand => self.rubber_band,
        }
    }
}

/// Whether a tool line is solid or dashed ([`TOOL_DASH_PX`] on, as many off).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dash {
    /// Solid.
    Solid,
    /// Dashed.
    Dashed,
}

/// The shape of a snap marker: a `6s + 1`-pixel cross on a grid point, an `8s + 1`-pixel ring on a connection point,
/// a `6s + 1`-pixel filled square on a junction (7, 9 and 7 pixels at scale 1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MarkerKind {
    /// A grid point.
    Grid,
    /// A connection point.
    Cp,
    /// A junction.
    Junction,
}

/// One mark of a tool's overlay in sheet nm: the toolkit-neutral form of `ew_viewmodel::tools::OverlayItem`, text
/// labels left out.
#[derive(Clone, PartialEq, Debug)]
pub enum ToolMark {
    /// A line from `a` to `b`.
    Line {
        /// The start.
        a: (i64, i64),
        /// The end.
        b: (i64, i64),
        /// Solid or dashed.
        dash: Dash,
        /// The colour token.
        token: ToolToken,
    },
    /// A polyline through `points`.
    Polyline {
        /// The vertices.
        points: Vec<(i64, i64)>,
        /// Solid or dashed.
        dash: Dash,
        /// The colour token.
        token: ToolToken,
    },
    /// The outline of a rectangle.
    Rect {
        /// The rectangle.
        r: Rect,
        /// Solid or dashed.
        dash: Dash,
        /// The colour token.
        token: ToolToken,
    },
    /// The geometric ghost of a drag: the rectangles moved by `offset`, dashed in the ghost token.
    Ghost {
        /// The rectangles (entity rectangles).
        rects: Vec<Rect>,
        /// The displacement.
        offset: (i64, i64),
    },
    /// A floating symbol's outline (WP-S2.04 §6.3): polylines in sheet nm, drawn as dashed lines in the ghost token
    /// (`s` pixels wide, FT-391).
    Paths {
        /// The polylines.
        paths: Vec<Vec<(i64, i64)>>,
    },
    /// A snap marker, in the snap token.
    Marker {
        /// Where.
        at: (i64, i64),
        /// Its shape.
        kind: MarkerKind,
    },
}

/// A page's connection grid on the sheet: grid point `(i, j)` of the page lies at
/// `offset + scale × (i, j) × num_nm / den` (`scale = sheet_scale_ppb / 10⁹`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridSpec {
    /// The pitch's numerator in nm on the page.
    pub num_nm: i64,
    /// The pitch's denominator.
    pub den: i64,
    /// The page-to-sheet scale in parts per 10⁹.
    pub sheet_scale_ppb: u32,
    /// Where the page origin lands on the sheet (nm).
    pub sheet_offset_nm: (i64, i64),
}

impl GridSpec {
    /// The pitch on the sheet in nm (`None` for a pitch that is not positive).
    pub fn sheet_pitch_nm(&self) -> Option<f64> {
        let p = self.num_nm as f64 / self.den as f64 * f64::from(self.sheet_scale_ppb) / 1e9;
        (p.is_finite() && p > 0.0).then_some(p)
    }
}

/// What the overlays are built from.
#[derive(Clone, Copy, Debug)]
pub struct OverlayInput<'a> {
    /// The frame's transform (the same the tiles are drawn with).
    pub transform: DrawTransform,
    /// The pane in whole physical pixels.
    pub pane_px: (u32, u32),
    /// Physical pixels per logical point.
    pub ppp: f64,
    /// The sheet.
    pub sheet: SheetSize,
    /// The grid to draw, `None` when it is off or the page has none.
    pub grid: Option<GridSpec>,
    /// The hovered entity's rectangle (sheet nm).
    pub hover: Option<Rect>,
    /// The selected entities' rectangles (sheet nm); nothing passes any in WP-S2.02.
    pub selected: &'a [Rect],
    /// The keyboard grid cursor (sheet nm), drawn above every other overlay.
    pub cursor: Option<(i64, i64)>,
    /// The active tool's overlay (WP-S2.03 §7.4): above the selection, below the cursor; markers above lines.
    pub tool: &'a [ToolMark],
    /// The connection points without a connection, each marked open (FT-705), and the pitch that gates them.
    pub open_cps: OpenCps<'a>,
}

/// The connection points of the page that have no connection on it (FT-705, the cycle-38 ruling): each is drawn as a
/// small hollow square, so that a wire drawn across one is visibly not connected to it. Screen only: a session overlay
/// like the CP rings (doc 16a §7), never in the display list, so never in print or PDF. Drawn only while one step of
/// the page's connection grid is at least [`OPEN_CP_MIN_STEP_PT`] on screen, so that the marks of CPs one step apart
/// stay apart (a mark is 5 pixels at scale 1); zoomed further out a dense page shows none.
#[derive(Clone, Copy, Debug, Default)]
pub struct OpenCps<'a> {
    /// The connection points (sheet nm).
    pub points: &'a [(i64, i64)],
    /// One step of the page's connection grid on the sheet (nm, [`GridSpec::sheet_pitch_nm`]); `None` (a page without
    /// a grid) draws none.
    pub pitch_nm: Option<f64>,
}

/// The overlays of one frame, each a list of pixel rectangles clipped to the pane.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Overlay {
    /// The sheet on screen (not clipped).
    pub sheet: PxRect,
    /// The pane outside the sheet.
    pub surround: Vec<PxRect>,
    /// The 1-pixel outline just outside the sheet.
    pub border: Vec<PxRect>,
    /// The multiple of the grid pitch the dots are drawn at (1, 2, 4, …); `None` without dots.
    pub grid_step: Option<u64>,
    /// The grid dots.
    pub dots: Vec<PxRect>,
    /// The dashes of the hover outline.
    pub hover: Vec<PxRect>,
    /// The bands of the selection outlines (`2s` pixels).
    pub selected: Vec<PxRect>,
    /// The white casing of the selection outlines: an `s`-pixel band on each side of the outline (doc 16a §7.1).
    pub casing: Vec<PxRect>,
    /// The selection handles' rims: an `8s`-pixel square on each corner of each outline, in the selection colour.
    pub handles: Vec<PxRect>,
    /// The handles' white faces, inside their rims.
    pub handle_faces: Vec<PxRect>,
    /// The four arms of the grid cursor's crosshair.
    pub cursor: Vec<PxRect>,
    /// The tool's lines, rectangles and ghosts, each pixel run with its colour token, in the tool's order.
    pub tool: Vec<(PxRect, ToolToken)>,
    /// The tool's snap markers.
    pub markers: Vec<PxRect>,
    /// The rims of the open markers of the free connection points (FT-705), under the tool's lines.
    pub open_cps: Vec<PxRect>,
}

/// The sheet's rectangle on screen: with a settled transform exactly the pixels of the full-sheet raster at its
/// resolution, otherwise the mapped corners rounded.
pub fn sheet_rect(t: &DrawTransform, sheet: SheetSize) -> PxRect {
    match t.raster_dpi {
        Some(r) => {
            let (w, h) = pixel_size(sheet, r);
            let (x0, y0) = (round_px(t.origin.0), round_px(t.origin.1));
            let w = i64::try_from(w).unwrap_or(i64::MAX);
            let h = i64::try_from(h).unwrap_or(i64::MAX);
            PxRect::new(x0, y0, x0.saturating_add(w), y0.saturating_add(h))
        }
        None => {
            let (x1, y1) = t.to_screen((sheet.width.0 as f64, sheet.height.0 as f64));
            PxRect::new(
                round_px(t.origin.0),
                round_px(t.origin.1),
                round_px(x1),
                round_px(y1),
            )
        }
    }
}

fn push_clipped(out: &mut Vec<PxRect>, r: PxRect, pane: &PxRect) {
    let c = r.intersect(pane);
    if !c.is_empty() {
        out.push(c);
    }
}

/// The `s`-pixel dashed outline of `r` (`4s` on, as many off, each edge from its first corner; FT-391).
fn dashed(r: PxRect, s: i64, pane: &PxRect, out: &mut Vec<PxRect>) {
    let dash = HOVER_DASH_PX * s;
    let period = 2 * dash;
    let mut x = r.x0;
    while x < r.x1 {
        let end = (x + dash).min(r.x1);
        push_clipped(out, PxRect::new(x, r.y0, end, r.y0 + s), pane);
        push_clipped(out, PxRect::new(x, r.y1 - s, end, r.y1), pane);
        x += period;
    }
    let mut y = r.y0;
    while y < r.y1 {
        let end = (y + dash).min(r.y1);
        push_clipped(out, PxRect::new(r.x0, y, r.x0 + s, end), pane);
        push_clipped(out, PxRect::new(r.x1 - s, y, r.x1, end), pane);
        y += period;
    }
}

/// The selection of the outline rectangle `r` at scale `s` (doc 16a §7.1 and the mockup, FT-391): the `2s` band just
/// inside its edges, an `s`-pixel white casing outside and inside that band, and an `8s`-pixel handle centred on each
/// corner of the band's centre line, a rim in the selection colour (`⌈1.5 s⌉` pixels) around a white face.
fn selection(r: PxRect, s: i64, pane: &PxRect, o: &mut Overlay) {
    solid(r, 2 * s, pane, &mut o.selected);
    solid(
        PxRect::new(r.x0 - s, r.y0 - s, r.x1 + s, r.y1 + s),
        s,
        pane,
        &mut o.casing,
    );
    let inner = PxRect::new(r.x0 + 2 * s, r.y0 + 2 * s, r.x1 - 2 * s, r.y1 - 2 * s);
    if inner.x1 - inner.x0 > 2 * s && inner.y1 - inner.y0 > 2 * s {
        solid(inner, s, pane, &mut o.casing);
    }
    let half = HANDLE_PX * s / 2;
    let rim = (3 * s + 1) / 2;
    for (cx, cy) in [
        (r.x0 + s, r.y0 + s),
        (r.x1 - s, r.y0 + s),
        (r.x0 + s, r.y1 - s),
        (r.x1 - s, r.y1 - s),
    ] {
        let h = PxRect::new(cx - half, cy - half, cx + half, cy + half);
        push_clipped(&mut o.handles, h, pane);
        push_clipped(
            &mut o.handle_faces,
            PxRect::new(h.x0 + rim, h.y0 + rim, h.x1 - rim, h.y1 - rim),
            pane,
        );
    }
}

/// The `width`-pixel solid band just inside the edges of `r`.
fn solid(r: PxRect, width: i64, pane: &PxRect, out: &mut Vec<PxRect>) {
    push_clipped(out, PxRect::new(r.x0, r.y0, r.x1, r.y0 + width), pane);
    push_clipped(out, PxRect::new(r.x0, r.y1 - width, r.x1, r.y1), pane);
    push_clipped(
        out,
        PxRect::new(r.x0, r.y0 + width, r.x0 + width, r.y1 - width),
        pane,
    );
    push_clipped(
        out,
        PxRect::new(r.x1 - width, r.y0 + width, r.x1, r.y1 - width),
        pane,
    );
}

/// An entity rectangle on screen, outset by [`OUTLINE_OUTSET_PT`].
fn outline_rect(t: &DrawTransform, ppp: f64, r: Rect) -> PxRect {
    let (a, b) = (r.min(), r.max());
    let (x0, y0) = t.to_screen((a.x.0 as f64, a.y.0 as f64));
    let (x1, y1) = t.to_screen((b.x.0 as f64, b.y.0 as f64));
    let o = OUTLINE_OUTSET_PT * ppp;
    PxRect::new(
        round_px(x0 - o),
        round_px(y0 - o),
        round_px(x1 + o),
        round_px(y1 + o),
    )
}

/// The grid dots inside the sheet and the pane, and the pitch multiple they are drawn at.
fn grid_dots(input: &OverlayInput<'_>, pane: &PxRect) -> Option<(u64, Vec<PxRect>)> {
    let grid = input.grid?;
    let pitch = grid.sheet_pitch_nm()?;
    let t = &input.transform;
    let k = t.px_per_nm();
    let ppp = if input.ppp > 0.0 { input.ppp } else { 1.0 };
    // Double the drawn pitch until the dots are at least 8 points apart: every dot stays on a grid point.
    let mut step: u64 = 1;
    while pitch * step as f64 * k / ppp < MIN_DOT_SPACING_PT && step < (1 << 40) {
        step *= 2;
    }
    let spacing = pitch * step as f64;
    let size: i64 = if ppp >= 2.0 { 2 } else { 1 };
    // The part of the sheet the pane shows, in nm.
    let (vx0, vy0) = t.to_sheet((pane.x0 as f64, pane.y0 as f64));
    let (vx1, vy1) = t.to_sheet((pane.x1 as f64, pane.y1 as f64));
    let range = |lo: f64, hi: f64, extent: f64, offset: f64| -> Option<(i64, i64)> {
        let (lo, hi) = (lo.max(0.0), hi.min(extent));
        if hi < lo {
            return None;
        }
        let first = ((lo - offset) / spacing).ceil();
        let last = ((hi - offset) / spacing).floor();
        (first <= last && first.is_finite() && last.is_finite()).then_some((first as i64, last as i64))
    };
    let (ox, oy) = (grid.sheet_offset_nm.0 as f64, grid.sheet_offset_nm.1 as f64);
    let (i0, i1) = range(vx0, vx1, input.sheet.width.0 as f64, ox)?;
    let (j0, j1) = range(vy0, vy1, input.sheet.height.0 as f64, oy)?;
    let half = size as f64 / 2.0;
    let mut dots = Vec::with_capacity(((i1 - i0 + 1) * (j1 - j0 + 1)).clamp(0, 1 << 20) as usize);
    for j in j0..=j1 {
        let y = round_px(t.origin.1 + (oy + j as f64 * spacing) * k - half);
        for i in i0..=i1 {
            let x = round_px(t.origin.0 + (ox + i as f64 * spacing) * k - half);
            push_clipped(&mut dots, PxRect::new(x, y, x + size, y + size), pane);
        }
    }
    Some((step, dots))
}

/// The overlays of a frame.
pub fn build(input: &OverlayInput<'_>) -> Overlay {
    let pane = PxRect::new(0, 0, i64::from(input.pane_px.0), i64::from(input.pane_px.1));
    let sheet = sheet_rect(&input.transform, input.sheet);
    let mut surround = Vec::new();
    // Above, below, then left and right of the sheet.
    push_clipped(
        &mut surround,
        PxRect::new(pane.x0, pane.y0, pane.x1, sheet.y0),
        &pane,
    );
    push_clipped(
        &mut surround,
        PxRect::new(pane.x0, sheet.y1, pane.x1, pane.y1),
        &pane,
    );
    let (band_y0, band_y1) = (sheet.y0.max(pane.y0), sheet.y1.min(pane.y1));
    push_clipped(
        &mut surround,
        PxRect::new(pane.x0, band_y0, sheet.x0, band_y1),
        &pane,
    );
    push_clipped(
        &mut surround,
        PxRect::new(sheet.x1, band_y0, pane.x1, band_y1),
        &pane,
    );
    let mut border = Vec::new();
    let outer = PxRect::new(sheet.x0 - 1, sheet.y0 - 1, sheet.x1 + 1, sheet.y1 + 1);
    solid(outer, 1, &pane, &mut border);
    let (grid_step, dots) = match grid_dots(input, &pane) {
        Some((step, dots)) => (Some(step), dots),
        None => (None, Vec::new()),
    };
    let s = mark_scale(input.ppp);
    let mut hover = Vec::new();
    if let Some(r) = input.hover {
        dashed(outline_rect(&input.transform, input.ppp, r), s, &pane, &mut hover);
    }
    let mut sel = Overlay::default();
    for &r in input.selected {
        selection(outline_rect(&input.transform, input.ppp, r), s, &pane, &mut sel);
    }
    let mut cursor = Vec::new();
    if let Some((cx, cy)) = input.cursor {
        let (sx, sy) = input.transform.to_screen((cx as f64, cy as f64));
        let (px, py) = (sx.floor() as i64, sy.floor() as i64);
        // Arms `s` wide from `2s` to `8s` off the centre pixel (at scale 1: 17 pixels with a 5-pixel gap).
        let o = (s - 1) / 2;
        let (y0, x0) = (py - o, px - o);
        for arm in [
            PxRect::new(px - 8 * s, y0, px - 2 * s, y0 + s),
            PxRect::new(px + 2 * s + 1, y0, px + 8 * s + 1, y0 + s),
            PxRect::new(x0, py - 8 * s, x0 + s, py - 2 * s),
            PxRect::new(x0, py + 2 * s + 1, x0 + s, py + 8 * s + 1),
        ] {
            push_clipped(&mut cursor, arm, &pane);
        }
    }
    let open_cps = open_cp_marks(input, s, &pane);
    let mut tool = Vec::new();
    let mut markers = Vec::new();
    let width = s;
    let t = &input.transform;
    let screen = |p: (i64, i64)| t.to_screen((p.0 as f64, p.1 as f64));
    for mark in input.tool {
        let mut px = Vec::new();
        let token = match mark {
            ToolMark::Line { a, b, dash, token } => {
                line_px(screen(*a), screen(*b), width, s, *dash, &pane, &mut px);
                *token
            }
            ToolMark::Polyline { points, dash, token } => {
                for w in points.windows(2) {
                    line_px(screen(w[0]), screen(w[1]), width, s, *dash, &pane, &mut px);
                }
                *token
            }
            ToolMark::Rect { r, dash, token } => {
                rect_px(t, *r, (0, 0), width, *dash, &pane, &mut px);
                *token
            }
            ToolMark::Ghost { rects, offset } => {
                for r in rects {
                    rect_px(t, *r, *offset, width, Dash::Dashed, &pane, &mut px);
                }
                ToolToken::Ghost
            }
            ToolMark::Paths { paths } => {
                for line in paths {
                    for w in line.windows(2) {
                        line_px(screen(w[0]), screen(w[1]), width, s, Dash::Dashed, &pane, &mut px);
                    }
                }
                ToolToken::Ghost
            }
            ToolMark::Marker { at, kind } => {
                marker_px(screen(*at), *kind, s, &pane, &mut markers);
                continue;
            }
        };
        tool.extend(px.into_iter().map(|r| (r, token)));
    }
    Overlay {
        sheet,
        surround,
        border,
        grid_step,
        dots,
        hover,
        selected: sel.selected,
        casing: sel.casing,
        handles: sel.handles,
        handle_faces: sel.handle_faces,
        cursor,
        tool,
        markers,
        open_cps,
    }
}

/// The rims of the open markers of `input.open_cps` at scale `s` (see [`OpenCps`]): a `4s + 1`-pixel square centred
/// on the CP's pixel with an `s`-pixel rim; none while a grid step is under [`OPEN_CP_MIN_STEP_PT`] on screen.
fn open_cp_marks(input: &OverlayInput<'_>, s: i64, pane: &PxRect) -> Vec<PxRect> {
    let mut out = Vec::new();
    let Some(pitch) = input.open_cps.pitch_nm.filter(|p| p.is_finite() && *p > 0.0) else {
        return out;
    };
    let ppp = if input.ppp > 0.0 { input.ppp } else { 1.0 };
    if pitch * input.transform.px_per_nm() / ppp < OPEN_CP_MIN_STEP_PT {
        return out;
    }
    let half = (OPEN_CP_PX - 1) / 2 * s;
    for p in input.open_cps.points {
        let (x, y) = input.transform.to_screen((p.0 as f64, p.1 as f64));
        if !(x.is_finite() && y.is_finite()) {
            continue;
        }
        let (px, py) = (x.floor() as i64, y.floor() as i64);
        let r = PxRect::new(px - half, py - half, px + half + 1, py + half + 1);
        solid(r, s, pane, &mut out);
    }
    out
}

/// Appends the pixel run `r` to `out`, extending the last rectangle when `r` continues its row or its column.
fn push_run(out: &mut Vec<PxRect>, r: PxRect) {
    if let Some(last) = out.last_mut() {
        let same_rows = last.y0 == r.y0 && last.y1 == r.y1;
        let same_cols = last.x0 == r.x0 && last.x1 == r.x1;
        if same_rows && r.x0 <= last.x1 && r.x1 >= last.x0 {
            last.x0 = last.x0.min(r.x0);
            last.x1 = last.x1.max(r.x1);
            return;
        }
        if same_cols && r.y0 <= last.y1 && r.y1 >= last.y0 {
            last.y0 = last.y0.min(r.y0);
            last.y1 = last.y1.max(r.y1);
            return;
        }
    }
    out.push(r);
}

/// The parameter range `[t0, t1]` of the segment `a → b` inside `[lo, hi]` on both axes (Liang–Barsky), if any.
fn clip_segment(a: (f64, f64), b: (f64, f64), lo: (f64, f64), hi: (f64, f64)) -> Option<(f64, f64)> {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let d = (b.0 - a.0, b.1 - a.1);
    for (p, q) in [
        (-d.0, a.0 - lo.0),
        (d.0, hi.0 - a.0),
        (-d.1, a.1 - lo.1),
        (d.1, hi.1 - a.1),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    (t0 <= t1 && t0.is_finite() && t1.is_finite()).then_some((t0, t1))
}

/// `num / den` rounded to the nearest integer, halves away from zero (`den != 0`).
fn div_round(num: i64, den: i64) -> i64 {
    let (q, r) = (num / den, num % den);
    if 2 * r.abs() >= den.abs() {
        q + num.signum() * den.signum()
    } else {
        q
    }
}

/// The pixels of the line from the screen point `a` to `b` (physical pixels, unrounded): from the pixel of `a` to
/// the pixel of `b`, one square of `width` pixels per step along the major axis, the dash (`4s` steps on, as many
/// off) counted from `a`, only the part within the pane walked and the result clipped to it.
fn line_px(
    a: (f64, f64),
    b: (f64, f64),
    width: i64,
    s: i64,
    dash: Dash,
    pane: &PxRect,
    out: &mut Vec<PxRect>,
) {
    if !(a.0.is_finite() && a.1.is_finite() && b.0.is_finite() && b.1.is_finite()) {
        return;
    }
    let (fx0, fy0) = (a.0.floor(), a.1.floor());
    let (fx1, fy1) = (b.0.floor(), b.1.floor());
    let m = width as f64 + 1.0;
    let lo = (pane.x0 as f64 - m, pane.y0 as f64 - m);
    let hi = (pane.x1 as f64 + m, pane.y1 as f64 + m);
    let Some((t0, t1)) = clip_segment((fx0, fy0), (fx1, fy1), lo, hi) else {
        return;
    };
    let steps = (fx1 - fx0).abs().max((fy1 - fy0).abs());
    if steps >= 1e9 {
        return;
    }
    let n = steps as i64;
    let (first, last) = if n == 0 {
        (0, 0)
    } else {
        (
            ((t0 * steps).floor() as i64).max(0),
            ((t1 * steps).ceil() as i64).min(n),
        )
    };
    let (x0, y0) = (fx0 as i64, fy0 as i64);
    let (dx, dy) = (fx1 as i64 - x0, fy1 as i64 - y0);
    let mut run = Vec::new();
    for i in first..=last {
        if dash == Dash::Dashed && (i / (TOOL_DASH_PX * s)) % 2 == 1 {
            continue;
        }
        let (x, y) = if n == 0 {
            (x0, y0)
        } else {
            (x0 + div_round(dx * i, n), y0 + div_round(dy * i, n))
        };
        push_run(&mut run, PxRect::new(x, y, x + width, y + width));
    }
    for r in run {
        push_clipped(out, r, pane);
    }
}

/// The outline of the sheet rectangle `r` moved by `offset`: its four edges as lines between its corners.
fn rect_px(
    t: &DrawTransform,
    r: Rect,
    offset: (i64, i64),
    width: i64,
    dash: Dash,
    pane: &PxRect,
    out: &mut Vec<PxRect>,
) {
    let (a, b) = (r.min(), r.max());
    let p = |x: i64, y: i64| {
        t.to_screen((
            x.saturating_add(offset.0) as f64,
            y.saturating_add(offset.1) as f64,
        ))
    };
    let (x0, y0, x1, y1) = (a.x.0, a.y.0, b.x.0, b.y.0);
    // The lines are `width` (= `s`) pixels wide and dashed at the same scale.
    let s = width;
    line_px(p(x0, y0), p(x1, y0), width, s, dash, pane, out);
    line_px(p(x1, y0), p(x1, y1), width, s, dash, pane, out);
    line_px(p(x1, y1), p(x0, y1), width, s, dash, pane, out);
    line_px(p(x0, y1), p(x0, y0), width, s, dash, pane, out);
}

/// The pixels of a snap marker centred on the pixel of the screen point `at`, at scale `s` (FT-391).
fn marker_px(at: (f64, f64), kind: MarkerKind, s: i64, pane: &PxRect, out: &mut Vec<PxRect>) {
    if !(at.0.is_finite() && at.1.is_finite()) {
        return;
    }
    let (px, py) = (at.0.floor() as i64, at.1.floor() as i64);
    let h = 3 * s;
    match kind {
        MarkerKind::Grid => {
            // Two `6s + 1`-pixel arms `s` wide crossing at the centre pixel (the vertical arm split around the
            // horizontal one, so that no pixel is listed twice).
            let o = (s - 1) / 2;
            let (y0, x0) = (py - o, px - o);
            push_clipped(out, PxRect::new(px - h, y0, px + h + 1, y0 + s), pane);
            push_clipped(out, PxRect::new(x0, py - h, x0 + s, y0), pane);
            push_clipped(out, PxRect::new(x0, y0 + s, x0 + s, py + h + 1), pane);
        }
        MarkerKind::Cp => {
            // The pixels whose centre lies between `3s + 0.5` and `4s + 0.5` pixels from the centre pixel's (at
            // scale 1: d² in 13 ..= 20), that is `(6s + 1)² ≤ 4d² ≤ (8s + 1)²`.
            let r = 4 * s;
            let (lo, hi) = ((6 * s + 1).pow(2), (8 * s + 1).pow(2));
            for dy in -r..=r {
                let mut run = Vec::new();
                for dx in -r..=r {
                    if (lo..=hi).contains(&(4 * (dx * dx + dy * dy))) {
                        push_run(&mut run, PxRect::new(px + dx, py + dy, px + dx + 1, py + dy + 1));
                    }
                }
                for r in run {
                    push_clipped(out, r, pane);
                }
            }
        }
        MarkerKind::Junction => push_clipped(out, PxRect::new(px - h, py - h, px + h + 1, py + h + 1), pane),
    }
}

/// Fills `r` (clipped to the image) with `c`, straight-alpha over what is there.
pub fn fill(image: &mut RgbaImage, r: PxRect, c: Color) {
    let bounds = PxRect::new(0, 0, i64::from(image.width), i64::from(image.height));
    let r = r.intersect(&bounds);
    if r.is_empty() {
        return;
    }
    let width = image.width as usize;
    for y in r.y0..r.y1 {
        for x in r.x0..r.x1 {
            let i = (y as usize * width + x as usize) * 4;
            let px = &mut image.pixels[i..i + 4];
            if c.a == 255 {
                px.copy_from_slice(&[c.r, c.g, c.b, 255]);
            } else {
                let a = u32::from(c.a);
                let blend =
                    |src: u8, dst: u8| ((u32::from(src) * a + u32::from(dst) * (255 - a) + 127) / 255) as u8;
                px[0] = blend(c.r, px[0]);
                px[1] = blend(c.g, px[1]);
                px[2] = blend(c.b, px[2]);
                px[3] = (a + u32::from(px[3]) * (255 - a) / 255).min(255) as u8;
            }
        }
    }
}

/// Composites the overlays into a snapshot of the pane, in paint order: surround, page bounds, grid dots, hover,
/// the selection (casing, outline, the handles' rims, then their faces), the open markers of the free connection
/// points, the tool's lines and ghosts, its snap markers, and last the grid cursor's crosshair.
pub fn composite(image: &mut RgbaImage, overlay: &Overlay, theme: &Theme) {
    for &r in &overlay.surround {
        fill(image, r, theme.surround);
    }
    for &r in &overlay.border {
        fill(image, r, theme.border);
    }
    for &r in &overlay.dots {
        fill(image, r, theme.grid_dot);
    }
    for &r in &overlay.hover {
        fill(image, r, theme.hover);
    }
    for &r in &overlay.casing {
        fill(image, r, theme.casing);
    }
    for &r in &overlay.selected {
        fill(image, r, theme.selection);
    }
    for &r in &overlay.handles {
        fill(image, r, theme.selection);
    }
    for &r in &overlay.handle_faces {
        fill(image, r, theme.casing);
    }
    for &r in &overlay.open_cps {
        fill(image, r, theme.open_cp);
    }
    for &(r, token) in &overlay.tool {
        fill(image, r, theme.token(token));
    }
    for &r in &overlay.markers {
        fill(image, r, theme.snap);
    }
    for &r in &overlay.cursor {
        fill(image, r, theme.cursor);
    }
}

#[cfg(test)]
mod tests {
    use ew_dl::ew_units::{Nm, Pt2};

    use super::*;
    use crate::view::View;

    fn a4() -> SheetSize {
        SheetSize {
            width: Nm::from_mm(297),
            height: Nm::from_mm(210),
        }
    }

    fn qet() -> GridSpec {
        GridSpec {
            num_nm: 7_937_500,
            den: 3,
            sheet_scale_ppb: 986_282_578,
            sheet_offset_nm: (9_790_000, 9_790_000),
        }
    }

    fn input(view: &View, grid: Option<GridSpec>, hover: Option<Rect>) -> OverlayInput<'static> {
        OverlayInput {
            transform: view.settled_transform(),
            pane_px: view.pane_px_whole(),
            ppp: view.ppp,
            sheet: a4(),
            grid,
            hover,
            selected: &[],
            cursor: None,
            tool: &[],
            open_cps: OpenCps::default(),
        }
    }

    fn view(zoom: f64) -> View {
        View {
            centre: (148_500_000, 105_000_000),
            zoom,
            pane: (1200.0, 800.0),
            ppp: 1.0,
        }
    }

    #[test]
    fn the_surround_and_the_border_frame_the_sheet() {
        let v = view(1.5625); // D = 150: the sheet is 1754 × 1240, wider than the pane.
        let o = build(&input(&v, None, None));
        let (w, h) = pixel_size(a4(), 150);
        assert_eq!(o.sheet.x1 - o.sheet.x0, w as i64);
        assert_eq!(o.sheet.y1 - o.sheet.y0, h as i64);
        assert!(o.surround.is_empty(), "the sheet covers the pane");
        let fit = view(0.3); // The whole sheet inside the pane.
        let o = build(&input(&fit, None, None));
        assert_eq!(o.surround.len(), 4);
        let area: i64 = o.surround.iter().map(|r| (r.x1 - r.x0) * (r.y1 - r.y0)).sum();
        let sheet = (o.sheet.x1 - o.sheet.x0) * (o.sheet.y1 - o.sheet.y0);
        assert_eq!(area + sheet, 1200 * 800, "surround and sheet tile the pane");
        // The border is the ring of pixels just outside the sheet.
        let ring: i64 = o.border.iter().map(|r| (r.x1 - r.x0) * (r.y1 - r.y0)).sum();
        let (sw, sh) = (o.sheet.x1 - o.sheet.x0, o.sheet.y1 - o.sheet.y0);
        assert_eq!(ring, 2 * (sw + 2) + 2 * sh);
    }

    #[test]
    fn grid_dots_sit_on_the_mapped_grid_and_keep_8_points_apart() {
        let v = view(1.5625);
        let o = build(&input(&v, Some(qet()), None));
        assert_eq!(o.grid_step, Some(1), "2.6 mm at 150 dpi is 15 px");
        assert!(!o.dots.is_empty());
        let t = v.settled_transform();
        let pitch = qet().sheet_pitch_nm().unwrap();
        for d in o.dots.iter().take(50) {
            // The dot's pixel holds the screen position of a grid point.
            let (sx, sy) = t.to_sheet((d.x0 as f64 + 0.5, d.y0 as f64 + 0.5));
            let i = (sx - 9_790_000.0) / pitch;
            let j = (sy - 9_790_000.0) / pitch;
            let px_nm = 1.0 / t.px_per_nm();
            assert!((i - i.round()).abs() * pitch <= px_nm, "{d:?}: {i}");
            assert!((j - j.round()).abs() * pitch <= px_nm, "{d:?}: {j}");
            assert_eq!((d.x1 - d.x0, d.y1 - d.y0), (1, 1));
        }
        // At 25 % the 2.6 mm pitch is 2.4 points: drawn every fourth point (9.8 points).
        let far = view(0.25);
        let o = build(&input(&far, Some(qet()), None));
        assert_eq!(o.grid_step, Some(4));
        // At ppp 2 the dots are 2 pixels.
        let hidpi = View { ppp: 2.0, ..v };
        let o = build(&input(&hidpi, Some(qet()), None));
        assert!(o.dots.iter().all(|d| d.x1 - d.x0 == 2 && d.y1 - d.y0 == 2));
        // No grid, no dots.
        assert!(build(&input(&v, None, None)).dots.is_empty());
    }

    #[test]
    fn the_hover_is_dashed_and_a_selection_solid() {
        let v = view(1.0);
        let r = Rect::from_points(
            Pt2::new(Nm::from_mm(140), Nm::from_mm(100)),
            Pt2::new(Nm::from_mm(160), Nm::from_mm(110)),
        );
        let mut i = input(&v, None, Some(r));
        let sel = [r];
        i.selected = &sel;
        let o = build(&i);
        assert!(!o.hover.is_empty());
        assert!(
            o.hover
                .iter()
                .all(|d| (d.x1 - d.x0).max(d.y1 - d.y0) <= HOVER_DASH_PX)
        );
        // The top edge: dashes start every 8 pixels.
        let top: Vec<&PxRect> = o.hover.iter().filter(|d| d.y1 - d.y0 == 1).collect();
        assert!(top.windows(2).any(|w| w[1].x0 - w[0].x0 == 2 * HOVER_DASH_PX));
        assert_eq!(o.selected.len(), 4);
        // Composited into an image, the selection band is selection-blue and the paper stays white elsewhere.
        let mut img = RgbaImage {
            width: 1200,
            height: 800,
            pixels: vec![255; 1200 * 800 * 4],
        };
        composite(&mut img, &o, &LIGHT);
        let s = o.selected[0];
        let at = |x: i64, y: i64| {
            let k = (y as usize * 1200 + x as usize) * 4;
            [img.pixels[k], img.pixels[k + 1], img.pixels[k + 2]]
        };
        assert_eq!(at((s.x0 + s.x1) / 2, s.y0), [0x0B, 0x63, 0xCE]);
        assert_eq!(at(600, 300), [255, 255, 255]);
    }

    /// A theme whose every token has its own colour, so that a composite tells the layers apart.
    fn distinct() -> Theme {
        Theme {
            surround: rgb(1, 1, 1),
            border: rgb(2, 2, 2),
            grid_dot: rgb(3, 3, 3),
            hover: rgb(4, 4, 4),
            selection: rgb(5, 5, 5),
            cursor: rgb(0xD5, 0x5E, 0x00),
            paper: rgb(255, 255, 255),
            ghost: rgb(7, 7, 7),
            snap: rgb(0x00, 0x9E, 0x73),
            measure: rgb(0xA8, 0x6F, 0x00),
            rubber_band: rgb(9, 9, 9),
            casing: rgb(10, 10, 10),
            open_cp: rgb(11, 11, 11),
        }
    }

    fn white() -> RgbaImage {
        RgbaImage {
            width: 1200,
            height: 800,
            pixels: vec![255; 1200 * 800 * 4],
        }
    }

    #[test]
    fn the_cursor_is_a_17_pixel_crosshair_with_a_5_pixel_gap() {
        let v = view(1.0);
        let mut i = input(&v, None, None);
        i.cursor = Some((148_500_000, 105_000_000));
        let o = build(&i);
        assert_eq!(o.cursor.len(), 4);
        let (sx, sy) = i.transform.to_screen((148_500_000.0, 105_000_000.0));
        let (px, py) = (sx.floor() as i64, sy.floor() as i64);
        let mut horizontal = 0;
        let mut vertical = 0;
        for r in &o.cursor {
            let (w, h) = (r.x1 - r.x0, r.y1 - r.y0);
            assert!((w, h) == (6, 1) || (w, h) == (1, 6), "{r:?}");
            if w == 6 {
                horizontal += 1;
            } else {
                vertical += 1;
            }
        }
        assert_eq!((horizontal, vertical), (2, 2));
        let covered = |x: i64, y: i64| {
            o.cursor
                .iter()
                .any(|r| (r.x0..r.x1).contains(&x) && (r.y0..r.y1).contains(&y))
        };
        for d in -8..=8_i64 {
            let arm = d.abs() >= 3;
            assert_eq!(covered(px + d, py), arm, "row, offset {d}");
            assert_eq!(covered(px, py + d), arm, "column, offset {d}");
        }
        assert!(!covered(px, py), "the centre pixel is empty");
        assert!(!covered(px - 9, py) && !covered(px + 9, py));
        assert!(!covered(px, py - 9) && !covered(px, py + 9));
    }

    #[test]
    fn the_cursor_is_painted_over_the_hover() {
        let v = view(1.0);
        let r = Rect::from_points(
            Pt2::new(Nm::from_mm(140), Nm::from_mm(100)),
            Pt2::new(Nm::from_mm(160), Nm::from_mm(110)),
        );
        let mut i = input(&v, None, Some(r));
        let base = build(&i);
        // Put the cursor's centre on the first pixel of a hover dash: its arms run along the dash.
        let dash = base.hover[0];
        let (nx, ny) = i.transform.to_sheet((dash.x0 as f64 + 0.5, dash.y0 as f64 + 0.5));
        i.cursor = Some((nx as i64, ny as i64));
        let o = build(&i);
        let mut img = white();
        composite(&mut img, &o, &distinct());
        let at = |x: i64, y: i64| {
            let k = (y as usize * 1200 + x as usize) * 4;
            [img.pixels[k], img.pixels[k + 1], img.pixels[k + 2]]
        };
        let mut overlaps = 0;
        for c in &o.cursor {
            for h in &o.hover {
                let x = c.intersect(h);
                if !x.is_empty() {
                    overlaps += 1;
                    assert_eq!(at(x.x0, x.y0), [0xD5, 0x5E, 0x00]);
                }
            }
        }
        assert!(overlaps > 0, "the cursor overlaps a hover dash");
    }

    #[test]
    fn a_cursor_outside_the_pane_has_no_arms() {
        let v = view(1.0);
        let mut i = input(&v, None, None);
        i.cursor = Some((-5_000_000_000, -5_000_000_000));
        assert!(build(&i).cursor.is_empty());
    }

    #[test]
    fn the_cursor_token_is_the_same_in_both_themes() {
        assert_eq!(LIGHT.cursor, rgb(0x0B, 0x63, 0xCE));
        assert_eq!(DARK.cursor, rgb(0x0B, 0x63, 0xCE));
    }

    #[test]
    fn the_dark_theme_changes_the_surround_not_the_paper() {
        assert_ne!(LIGHT.surround, DARK.surround);
        assert_eq!(LIGHT.paper, DARK.paper);
        assert_eq!(LIGHT.hover, rgb(0x0B, 0x63, 0xCE));
        assert_eq!(LIGHT.surround, rgb(0xE3, 0xE6, 0xEA));
        assert_eq!(DARK.surround, rgb(0x2E, 0x32, 0x38));
        assert_eq!(LIGHT.casing, rgb(0xFF, 0xFF, 0xFF));
        assert_eq!(LIGHT.ghost.a, 153, "the accent at 60 %");
    }

    // ---------------------------------------------------------------------------------------------------------------
    // The tool overlay (WP-S2.03 §7.4, acceptance check T7)

    /// The set of pixels the rectangles cover.
    fn pixels<'a>(rects: impl IntoIterator<Item = &'a PxRect>) -> std::collections::BTreeSet<(i64, i64)> {
        let mut out = std::collections::BTreeSet::new();
        for r in rects {
            for y in r.y0..r.y1 {
                for x in r.x0..r.x1 {
                    out.insert((x, y));
                }
            }
        }
        out
    }

    // FT-705
    #[test]
    fn a_free_cp_is_a_hollow_square_drawn_only_at_a_zoom_where_grid_steps_are_4_points_apart() {
        let points = [(100_000_000, 100_000_000), (120_000_000, 100_000_000)];
        let pitch = qet().sheet_pitch_nm();
        let marks = |v: &View, pitch: Option<f64>| {
            build(&OverlayInput {
                open_cps: OpenCps {
                    points: &points,
                    pitch_nm: pitch,
                },
                ..input(v, None, None)
            })
        };
        // At 100 % (96 dpi) a 2.6 mm step is 9.8 points: drawn.
        let v = view(1.0);
        let t = v.settled_transform();
        let o = marks(&v, pitch);
        let px = pixels(o.open_cps.iter());
        for p in points {
            let (x, y) = px_of(&t, p);
            // A 5-pixel square's rim: its corners and edge midpoints, not its centre (it is open).
            for d in [(-2, -2), (2, -2), (-2, 2), (2, 2), (0, -2), (2, 0)] {
                assert!(px.contains(&(x + d.0, y + d.1)), "{p:?} {d:?}");
            }
            assert!(!px.contains(&(x, y)), "the centre is open");
            assert!(!px.contains(&(x + 3, y)), "5 pixels wide");
        }
        assert_eq!(px.len(), 2 * 16, "16 rim pixels each");
        // None of the other overlays carries them.
        assert!(o.markers.is_empty() && o.tool.is_empty());
        // At 69 % the step is 6.8 points: drawn; at 25 % it is 2.4 points: none; nor on a page without a grid.
        assert!(!marks(&view(0.69), pitch).open_cps.is_empty());
        assert!(marks(&view(0.25), pitch).open_cps.is_empty());
        assert!(marks(&v, None).open_cps.is_empty());
        // At ppp 2 the square is 9 pixels with a 2-pixel rim.
        let hidpi = View { ppp: 2.0, ..v };
        let o = marks(&hidpi, pitch);
        let t = hidpi.settled_transform();
        let (x, y) = px_of(&t, points[0]);
        let px = pixels(o.open_cps.iter());
        assert!(px.contains(&(x - 4, y - 4)) && px.contains(&(x + 4, y + 4)) && px.contains(&(x - 3, y)));
        assert!(!px.contains(&(x - 2, y)) && !px.contains(&(x + 5, y)));
        // The snapshot composites them in `open_cp`.
        let mut image = RgbaImage {
            width: 1200,
            height: 800,
            pixels: vec![255; 1200 * 800 * 4],
        };
        let o = marks(&v, pitch);
        composite(&mut image, &o, &LIGHT);
        let (x, y) = px_of(&v.settled_transform(), points[0]);
        let at = |x: i64, y: i64| {
            let i = (y as usize * 1200 + x as usize) * 4;
            image.pixels[i..i + 4].to_vec()
        };
        let c = LIGHT.open_cp;
        assert_eq!(at(x - 2, y - 2), vec![c.r, c.g, c.b, 255]);
    }

    fn with_tool<'a>(view: &View, tool: &'a [ToolMark]) -> OverlayInput<'a> {
        OverlayInput {
            tool,
            ..input(view, None, None)
        }
    }

    fn px_of(t: &DrawTransform, p: (i64, i64)) -> (i64, i64) {
        let (x, y) = t.to_screen((p.0 as f64, p.1 as f64));
        (x.floor() as i64, y.floor() as i64)
    }

    #[test]
    fn t7_a_line_runs_from_the_pixel_of_its_start_to_the_pixel_of_its_end() {
        let v = view(1.0);
        let t = v.settled_transform();
        let (a, b) = ((100_000_000, 100_000_000), (150_000_000, 100_000_000));
        let solid = [ToolMark::Line {
            a,
            b,
            dash: Dash::Solid,
            token: ToolToken::Measure,
        }];
        let o = build(&with_tool(&v, &solid));
        assert_eq!(o.tool.len(), 1, "a horizontal line is one run");
        let (pa, pb) = (px_of(&t, a), px_of(&t, b));
        assert_eq!(o.tool[0].0, PxRect::new(pa.0, pa.1, pb.0 + 1, pa.1 + 1));
        assert_eq!(o.tool[0].1, ToolToken::Measure);
        // Dashed: 4 on, 4 off, from the start.
        let dashed = [ToolMark::Line {
            a,
            b,
            dash: Dash::Dashed,
            token: ToolToken::RubberBand,
        }];
        let o = build(&with_tool(&v, &dashed));
        assert!(o.tool.len() > 2);
        assert_eq!(o.tool[0].0.x0, pa.0);
        for w in o.tool.windows(2) {
            assert_eq!(w[1].0.x0 - w[0].0.x0, 2 * TOOL_DASH_PX);
            assert!(w[0].0.x1 - w[0].0.x0 <= TOOL_DASH_PX);
        }
        // A diagonal: one pixel per step of the major axis, both ends included.
        let c = (130_000_000, 120_000_000);
        let diagonal = [ToolMark::Line {
            a,
            b: c,
            dash: Dash::Solid,
            token: ToolToken::Measure,
        }];
        let o = build(&with_tool(&v, &diagonal));
        let px = pixels(o.tool.iter().map(|(r, _)| r));
        let pc = px_of(&t, c);
        let steps = (pc.0 - pa.0).abs().max((pc.1 - pa.1).abs());
        assert_eq!(px.len() as i64, steps + 1);
        assert!(px.contains(&pa) && px.contains(&pc));
        // At ppp 2 the lines are 2 pixels wide.
        let hidpi = View { ppp: 2.0, ..v };
        let o = build(&with_tool(&hidpi, &solid));
        assert!(o.tool.iter().all(|(r, _)| r.y1 - r.y0 == 2));
    }

    #[test]
    fn t7_rectangles_polylines_and_ghosts_map_to_the_screen() {
        let v = view(1.0);
        let t = v.settled_transform();
        let r = Rect::from_points(
            Pt2::new(Nm::from_mm(140), Nm::from_mm(100)),
            Pt2::new(Nm::from_mm(160), Nm::from_mm(110)),
        );
        let rect = [ToolMark::Rect {
            r,
            dash: Dash::Solid,
            token: ToolToken::RubberBand,
        }];
        let o = build(&with_tool(&v, &rect));
        let px = pixels(o.tool.iter().map(|(r, _)| r));
        let (p0, p1) = (
            px_of(&t, (r.min().x.0, r.min().y.0)),
            px_of(&t, (r.max().x.0, r.max().y.0)),
        );
        // The outline: every pixel of the four edges, nothing inside.
        let ring = 2 * (p1.0 - p0.0 + 1) + 2 * (p1.1 - p0.1 - 1);
        assert_eq!(px.len() as i64, ring);
        assert!(px.contains(&p0) && px.contains(&p1));
        assert!(!px.contains(&((p0.0 + p1.0) / 2, (p0.1 + p1.1) / 2)));
        // A ghost is the rectangle moved by its offset, dashed in the ghost token.
        let offset = (10_000_000, -5_000_000);
        let ghost = [ToolMark::Ghost {
            rects: vec![r],
            offset,
        }];
        let o = build(&with_tool(&v, &ghost));
        assert!(o.tool.iter().all(|(_, k)| *k == ToolToken::Ghost));
        let g = pixels(o.tool.iter().map(|(r, _)| r));
        let moved = px_of(&t, (r.min().x.0 + offset.0, r.min().y.0 + offset.1));
        assert!(g.contains(&moved));
        assert!(g.len() < ring as usize, "dashed");
        // A polyline is its segments.
        let points = vec![
            (100_000_000, 100_000_000),
            (120_000_000, 100_000_000),
            (120_000_000, 90_000_000),
        ];
        let poly = [ToolMark::Polyline {
            points: points.clone(),
            dash: Dash::Solid,
            token: ToolToken::Snap,
        }];
        let o = build(&with_tool(&v, &poly));
        let px = pixels(o.tool.iter().map(|(r, _)| r));
        for p in points.clone() {
            assert!(px.contains(&px_of(&t, p)));
        }
        // A floating outline (WP-S2.04 §6.3) is its polylines, dashed in the ghost token: fewer pixels than solid.
        let paths = [ToolMark::Paths {
            paths: vec![
                points.clone(),
                vec![(90_000_000, 80_000_000), (90_000_000, 120_000_000)],
            ],
        }];
        let o = build(&with_tool(&v, &paths));
        assert!(o.tool.iter().all(|(_, k)| *k == ToolToken::Ghost));
        let dashed = pixels(o.tool.iter().map(|(r, _)| r));
        assert!(
            dashed.contains(&px_of(&t, points[0])),
            "a dash starts at the first vertex"
        );
        assert!(dashed.len() < px.len() + 200, "{}", dashed.len());
        let solid_vertical = 40_000_000.0 * t.px_per_nm();
        let vertical: Vec<_> = dashed
            .iter()
            .filter(|p| p.0 == px_of(&t, (90_000_000, 0)).0)
            .collect();
        assert!(
            (vertical.len() as f64) < solid_vertical * 0.75,
            "the vertical line is dashed"
        );
    }

    #[test]
    fn t7_markers_are_a_cross_a_ring_and_a_square() {
        let v = view(1.0);
        let t = v.settled_transform();
        let at = (148_500_000, 105_000_000);
        let c = px_of(&t, at);
        let mark = |kind| [ToolMark::Marker { at, kind }];
        let grid = pixels(&build(&with_tool(&v, &mark(MarkerKind::Grid))).markers);
        assert_eq!(grid.len(), 13, "two 7-pixel arms crossing");
        assert!(grid.contains(&c) && grid.contains(&(c.0 - 3, c.1)) && grid.contains(&(c.0, c.1 + 3)));
        let ring = pixels(&build(&with_tool(&v, &mark(MarkerKind::Cp))).markers);
        assert_eq!(ring.len(), 32);
        assert!(!ring.contains(&c), "a ring is open at its centre");
        assert!(ring.contains(&(c.0 + 4, c.1)) && ring.contains(&(c.0, c.1 - 4)));
        assert!(
            ring.iter()
                .all(|p| (p.0 - c.0).abs() <= 4 && (p.1 - c.1).abs() <= 4),
            "9 pixels wide"
        );
        let square = pixels(&build(&with_tool(&v, &mark(MarkerKind::Junction))).markers);
        assert_eq!(square.len(), 49);
        assert!(square.contains(&c));
        // Markers go to their own layer, never into the lines'.
        assert!(build(&with_tool(&v, &mark(MarkerKind::Cp))).tool.is_empty());
    }

    #[test]
    fn t7_the_tool_overlay_composites_between_the_selection_and_the_cursor() {
        let v = view(1.0);
        let at = (148_500_000, 105_000_000);
        let tool = [
            ToolMark::Line {
                a: (140_000_000, 105_000_000),
                b: (160_000_000, 105_000_000),
                dash: Dash::Solid,
                token: ToolToken::Measure,
            },
            ToolMark::Marker {
                at,
                kind: MarkerKind::Junction,
            },
        ];
        let mut i = with_tool(&v, &tool);
        let o = build(&i);
        let mut img = white();
        composite(&mut img, &o, &distinct());
        let rgb_at = |img: &RgbaImage, p: (i64, i64)| {
            let k = (p.1 as usize * 1200 + p.0 as usize) * 4;
            [img.pixels[k], img.pixels[k + 1], img.pixels[k + 2]]
        };
        let t = v.settled_transform();
        let c = px_of(&t, at);
        // The marker is drawn over the measure line, in the snap token.
        assert_eq!(rgb_at(&img, c), [0x00, 0x9E, 0x73]);
        assert_eq!(rgb_at(&img, (c.0 + 6, c.1)), [0xA8, 0x6F, 0x00]);
        // The cursor is drawn over the marker.
        i.cursor = Some(at);
        let o = build(&i);
        let mut img = white();
        composite(&mut img, &o, &distinct());
        assert_eq!(rgb_at(&img, (c.0 + 3, c.1)), [0xD5, 0x5E, 0x00]);
        // Twice the same input, the same pixels.
        let mut again = white();
        composite(&mut again, &build(&i), &distinct());
        assert_eq!(img.pixels, again.pixels);
    }

    #[test]
    fn t7_a_line_far_outside_the_pane_draws_only_what_shows() {
        let v = view(1.0);
        let far = [ToolMark::Line {
            a: (-9_000_000_000_000, 105_000_000),
            b: (9_000_000_000_000, 105_000_000),
            dash: Dash::Dashed,
            token: ToolToken::Measure,
        }];
        let o = build(&with_tool(&v, &far));
        assert!(
            !o.tool.is_empty() && o.tool.len() <= 1200 / 8 + 2,
            "{}",
            o.tool.len()
        );
        assert!(o.tool.iter().all(|(r, _)| r.x0 >= 0 && r.x1 <= 1200));
        let outside = [ToolMark::Line {
            a: (-9_000_000_000, -9_000_000_000),
            b: (-8_000_000_000, -9_000_000_000),
            dash: Dash::Solid,
            token: ToolToken::Measure,
        }];
        assert!(build(&with_tool(&v, &outside)).tool.is_empty());
        assert_eq!(LIGHT.token(ToolToken::Ghost), LIGHT.ghost);
        assert_eq!(DARK.token(ToolToken::Snap), rgb(0x0B, 0x63, 0xCE));
    }

    // ---------------------------------------------------------------------------------------------------------------
    // FT-391's scale rule and the selection of doc 16a §7.1 (WP-S2.17 part 5, acceptance checks OV3 and OV6)

    fn view_at(ppp: f64) -> View {
        View { ppp, ..view(1.0) }
    }

    /// The width and height of the bounding box of a set of pixels.
    fn extent(p: &std::collections::BTreeSet<(i64, i64)>) -> (i64, i64) {
        let x0 = p.iter().map(|q| q.0).min().unwrap();
        let x1 = p.iter().map(|q| q.0).max().unwrap();
        let y0 = p.iter().map(|q| q.1).min().unwrap();
        let y1 = p.iter().map(|q| q.1).max().unwrap();
        (x1 - x0 + 1, y1 - y0 + 1)
    }

    #[test]
    fn ov3_the_scale_factor_is_the_whole_part_of_ppp() {
        assert_eq!(mark_scale(1.0), 1);
        assert_eq!(mark_scale(1.5), 1);
        assert_eq!(mark_scale(2.0), 2);
        assert_eq!(mark_scale(2.99), 2);
        assert_eq!(mark_scale(3.0), 3);
        assert_eq!(mark_scale(0.5), 1);
        assert_eq!(mark_scale(f64::NAN), 1);
        assert_eq!(mark_scale(f64::INFINITY), 1);
        assert_eq!(mark_scale(100.0), 16);
    }

    /// OV3: at ppp 1, 1.5, 2 and 3 the markers are `6s + 1` (cross, square) and `8s + 1` (ring) pixels, lines `s`
    /// pixels wide with `4s` dashes, the hover `s` with `4s` dashes, the cursor's arms `s` wide and `6s` long.
    #[test]
    fn ov3_every_mark_follows_the_scale_rule() {
        for (ppp, s) in [(1.0, 1_i64), (1.5, 1), (2.0, 2), (3.0, 3)] {
            let v = view_at(ppp);
            let t = v.settled_transform();
            let at = (148_500_000, 105_000_000);
            let c = px_of(&t, at);
            let mark = |kind| [ToolMark::Marker { at, kind }];
            let grid = pixels(&build(&with_tool(&v, &mark(MarkerKind::Grid))).markers);
            assert_eq!(extent(&grid), (6 * s + 1, 6 * s + 1), "grid cross at ppp {ppp}");
            assert_eq!(
                grid.len() as i64,
                2 * s * (6 * s + 1) - s * s,
                "two arms {s} wide"
            );
            assert!(grid.contains(&c));
            let ring = pixels(&build(&with_tool(&v, &mark(MarkerKind::Cp))).markers);
            assert_eq!(extent(&ring), (8 * s + 1, 8 * s + 1), "CP ring at ppp {ppp}");
            assert!(!ring.contains(&c), "open at its centre");
            assert!(ring.contains(&(c.0 + 4 * s, c.1)) && ring.contains(&(c.0, c.1 - 4 * s)));
            // The ring is `s` thick on the axes.
            let on_axis = (1..=4 * s).filter(|d| ring.contains(&(c.0 + d, c.1))).count() as i64;
            assert_eq!(on_axis, s, "ring thickness at ppp {ppp}");
            let square = pixels(&build(&with_tool(&v, &mark(MarkerKind::Junction))).markers);
            assert_eq!(extent(&square), (6 * s + 1, 6 * s + 1));
            assert_eq!(square.len() as i64, (6 * s + 1) * (6 * s + 1));

            // A horizontal line: `s` rows; dashed, `4s` on and `4s` off.
            let (a, b) = ((100_000_000, 100_000_000), (150_000_000, 100_000_000));
            let line = |dash| {
                [ToolMark::Line {
                    a,
                    b,
                    dash,
                    token: ToolToken::Measure,
                }]
            };
            let solid = build(&with_tool(&v, &line(Dash::Solid)));
            assert_eq!(solid.tool.len(), 1);
            assert_eq!(
                solid.tool[0].0.y1 - solid.tool[0].0.y0,
                s,
                "line width at ppp {ppp}"
            );
            let dashed = build(&with_tool(&v, &line(Dash::Dashed)));
            let runs: Vec<PxRect> = dashed.tool.iter().map(|(r, _)| *r).collect();
            assert!(runs.len() > 2);
            assert_eq!(
                runs[0].x1 - runs[0].x0,
                4 * s + s - 1,
                "a dash of {s}-pixel squares"
            );
            assert_eq!(runs[1].x0 - runs[0].x0, 8 * s, "the dash period at ppp {ppp}");

            // The hover: `s` thick, `4s` dashes from each edge's first corner.
            let r = Rect::from_points(
                Pt2::new(Nm::from_mm(140), Nm::from_mm(100)),
                Pt2::new(Nm::from_mm(160), Nm::from_mm(110)),
            );
            let o = build(&input(&v, None, Some(r)));
            let top: Vec<&PxRect> = {
                let y = o.hover.iter().map(|d| d.y0).min().unwrap();
                let mut t: Vec<&PxRect> = o.hover.iter().filter(|d| d.y0 == y && d.x1 - d.x0 > s).collect();
                t.sort_by_key(|d| d.x0);
                t
            };
            assert!(
                top.iter().all(|d| d.y1 - d.y0 == s),
                "hover thickness at ppp {ppp}"
            );
            assert_eq!(top[0].x1 - top[0].x0, 4 * s);
            assert_eq!(top[1].x0 - top[0].x0, 8 * s);

            // The cursor: four arms `s` wide, `6s` long, the centre pixel empty.
            let mut i = input(&v, None, None);
            i.cursor = Some(at);
            let o = build(&i);
            assert_eq!(o.cursor.len(), 4);
            for arm in &o.cursor {
                let (w, h) = (arm.x1 - arm.x0, arm.y1 - arm.y0);
                assert!(
                    (w, h) == (6 * s, s) || (w, h) == (s, 6 * s),
                    "{arm:?} at ppp {ppp}"
                );
            }
            assert!(pixels(&o.cursor).iter().all(|p| *p != c));
        }
    }

    /// OV3: at ppp 1 the marks are exactly those built before the scale rule (the snapshots change only in colour).
    #[test]
    fn ov3_at_ppp_1_the_marks_keep_their_sizes() {
        let v = view(1.0);
        let at = (148_500_000, 105_000_000);
        let mark = |kind| [ToolMark::Marker { at, kind }];
        assert_eq!(
            pixels(&build(&with_tool(&v, &mark(MarkerKind::Grid))).markers).len(),
            13
        );
        assert_eq!(
            pixels(&build(&with_tool(&v, &mark(MarkerKind::Cp))).markers).len(),
            32
        );
        assert_eq!(
            pixels(&build(&with_tool(&v, &mark(MarkerKind::Junction))).markers).len(),
            49
        );
        let r = Rect::from_points(
            Pt2::new(Nm::from_mm(140), Nm::from_mm(100)),
            Pt2::new(Nm::from_mm(160), Nm::from_mm(110)),
        );
        let sel = [r];
        let mut i = input(&v, None, None);
        i.selected = &sel;
        let o = build(&i);
        assert_eq!(o.selected.len(), 4);
        let band = o.selected[0];
        assert_eq!(band.y1 - band.y0, 2, "a 2-pixel outline");
    }

    /// OV6 (model and pixels): at ppp 1 the selection is a 2-pixel outline in the selection colour with a 1-pixel
    /// white casing on both sides and four 8-pixel handles on its corners, white with a selection-coloured rim; at ppp
    /// 2 everything doubles.
    #[test]
    fn ov6_the_selection_has_a_casing_and_four_8_pixel_handles() {
        let r = Rect::from_points(
            Pt2::new(Nm::from_mm(140), Nm::from_mm(100)),
            Pt2::new(Nm::from_mm(160), Nm::from_mm(110)),
        );
        let sel = [r];
        for (ppp, s) in [(1.0, 1_i64), (2.0, 2)] {
            let v = view_at(ppp);
            let mut i = input(&v, None, None);
            i.selected = &sel;
            let o = build(&i);
            let outline = pixels(&o.selected);
            let (x0, y0) = (
                outline.iter().map(|p| p.0).min().unwrap(),
                outline.iter().map(|p| p.1).min().unwrap(),
            );
            let (x1, y1) = (
                outline.iter().map(|p| p.0).max().unwrap() + 1,
                outline.iter().map(|p| p.1).max().unwrap() + 1,
            );
            assert_eq!(o.handles.len(), 4);
            assert_eq!(o.handle_faces.len(), 4);
            for h in &o.handles {
                assert_eq!(
                    (h.x1 - h.x0, h.y1 - h.y0),
                    (8 * s, 8 * s),
                    "an 8s handle at ppp {ppp}"
                );
                // Centred on a corner of the outline's centre line.
                let (cx, cy) = ((h.x0 + h.x1) / 2, (h.y0 + h.y1) / 2);
                assert!(cx == x0 + s || cx == x1 - s, "{h:?}");
                assert!(cy == y0 + s || cy == y1 - s, "{h:?}");
            }
            let casing = pixels(&o.casing);
            let mid = (x0 + x1) / 2;
            // Above the outline, the outline (2s), below it (inside).
            for k in 0..s {
                assert!(casing.contains(&(mid, y0 - 1 - k)), "outer casing at ppp {ppp}");
                assert!(
                    casing.contains(&(mid, y0 + 2 * s + k)),
                    "inner casing at ppp {ppp}"
                );
            }
            assert!(!casing.contains(&(mid, y0 - 1 - s)));
            let mut img = RgbaImage {
                width: (1200.0 * ppp) as u32,
                height: (800.0 * ppp) as u32,
                pixels: vec![0; (1200.0 * ppp * 800.0 * ppp) as usize * 4],
            };
            composite(&mut img, &o, &LIGHT);
            let w = img.width as usize;
            let at = |x: i64, y: i64| {
                let k = (y as usize * w + x as usize) * 4;
                [img.pixels[k], img.pixels[k + 1], img.pixels[k + 2]]
            };
            let blue = [0x0B, 0x63, 0xCE];
            let white = [0xFF, 0xFF, 0xFF];
            assert_eq!(at(mid, y0), blue);
            assert_eq!(at(mid, y0 + 2 * s - 1), blue);
            assert_eq!(at(mid, y0 - 1), white, "outer casing");
            assert_eq!(at(mid, y0 + 2 * s), white, "inner casing");
            let h = o.handles[0];
            assert_eq!(at(h.x0, h.y0), blue, "the handle's rim");
            assert_eq!(
                at((h.x0 + h.x1) / 2, (h.y0 + h.y1) / 2),
                white,
                "the handle's face"
            );
        }
    }
}
