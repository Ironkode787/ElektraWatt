// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The canvas overlays (WP-S2.02 §6.4): the surround, the page bounds, the grid dots and the hover and selection
//! outlines, as rectangles of whole physical pixels of the pane.
//!
//! One code path for every backend: the egui host paints the rectangles as shapes over the tiles (the dots as one
//! mesh), and the headless snapshot composites them into its image with integer pixel fills ([`composite`]), so
//! the window and the snapshot agree pixel for pixel. No meaning is in colour alone: the hover is dashed, a
//! selection solid, the keyboard grid cursor a crosshair with a gap at its centre. The paint order is the surround,
//! the page bounds, the grid dots, the hover, the selection and last the cursor.
//!
//! The theme tokens are constants here for the light and the dark theme until WP-S2.08's theme system replaces
//! them; the sheet itself is white with its authored colours in every theme (FR-UI-043, doc 16 §11.2).

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
    /// The keyboard grid cursor's crosshair (provisional token `cursor`, #D55E00, 3.87:1 on white; the crosshair's
    /// shape carries the meaning, doc 16 §10.3).
    pub cursor: Color,
    /// The sheet (white in every theme).
    pub paper: Color,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

/// The light theme's canvas tokens (provisional until WP-S2.08).
pub const LIGHT: Theme = Theme {
    surround: rgb(0xD4, 0xD4, 0xD4),
    border: rgb(0x8C, 0x8C, 0x8C),
    grid_dot: rgb(0x9E, 0x9E, 0x9E),
    hover: rgb(0x00, 0x72, 0xB2),
    selection: rgb(0x00, 0x72, 0xB2),
    cursor: rgb(0xD5, 0x5E, 0x00),
    paper: rgb(0xFF, 0xFF, 0xFF),
};

/// The dark theme's canvas tokens (provisional until WP-S2.08); the sheet stays white.
pub const DARK: Theme = Theme {
    surround: rgb(0x2B, 0x2B, 0x2B),
    border: rgb(0x6E, 0x6E, 0x6E),
    grid_dot: rgb(0x9E, 0x9E, 0x9E),
    hover: rgb(0x00, 0x72, 0xB2),
    selection: rgb(0x00, 0x72, 0xB2),
    cursor: rgb(0xD5, 0x5E, 0x00),
    paper: rgb(0xFF, 0xFF, 0xFF),
};

/// The smallest spacing of grid dots on screen, in logical points: below it the drawn pitch doubles.
pub const MIN_DOT_SPACING_PT: f64 = 8.0;

/// The hover and selection outlines stand this many logical points outside the entity's rectangle.
pub const OUTLINE_OUTSET_PT: f64 = 2.0;

/// The dash of the hover outline: this many pixels on, as many off.
pub const HOVER_DASH_PX: i64 = 4;

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
    /// The bands of the selection outlines.
    pub selected: Vec<PxRect>,
    /// The four arms of the grid cursor's crosshair.
    pub cursor: Vec<PxRect>,
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

/// The 1-pixel dashed outline of `r` (`HOVER_DASH_PX` on, as many off, each edge from its first corner).
fn dashed(r: PxRect, pane: &PxRect, out: &mut Vec<PxRect>) {
    let period = 2 * HOVER_DASH_PX;
    let mut x = r.x0;
    while x < r.x1 {
        let end = (x + HOVER_DASH_PX).min(r.x1);
        push_clipped(out, PxRect::new(x, r.y0, end, r.y0 + 1), pane);
        push_clipped(out, PxRect::new(x, r.y1 - 1, end, r.y1), pane);
        x += period;
    }
    let mut y = r.y0;
    while y < r.y1 {
        let end = (y + HOVER_DASH_PX).min(r.y1);
        push_clipped(out, PxRect::new(r.x0, y, r.x0 + 1, end), pane);
        push_clipped(out, PxRect::new(r.x1 - 1, y, r.x1, end), pane);
        y += period;
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
    let mut hover = Vec::new();
    if let Some(r) = input.hover {
        dashed(outline_rect(&input.transform, input.ppp, r), &pane, &mut hover);
    }
    let mut selected = Vec::new();
    for &r in input.selected {
        solid(
            outline_rect(&input.transform, input.ppp, r),
            2,
            &pane,
            &mut selected,
        );
    }
    let mut cursor = Vec::new();
    if let Some((cx, cy)) = input.cursor {
        let (sx, sy) = input.transform.to_screen((cx as f64, cy as f64));
        let (px, py) = (sx.floor() as i64, sy.floor() as i64);
        for arm in [
            PxRect::new(px - 8, py, px - 2, py + 1),
            PxRect::new(px + 3, py, px + 9, py + 1),
            PxRect::new(px, py - 8, px + 1, py - 2),
            PxRect::new(px, py + 3, px + 1, py + 9),
        ] {
            push_clipped(&mut cursor, arm, &pane);
        }
    }
    Overlay {
        sheet,
        surround,
        border,
        grid_step,
        dots,
        hover,
        selected,
        cursor,
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
/// selection, and last the grid cursor's crosshair.
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
    for &r in &overlay.selected {
        fill(image, r, theme.selection);
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
        assert_eq!(at(s.x0 + 3, s.y0), [0x00, 0x72, 0xB2]);
        assert_eq!(at(600, 300), [255, 255, 255]);
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
        composite(&mut img, &o, &LIGHT);
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
        assert_eq!(LIGHT.cursor, rgb(0xD5, 0x5E, 0x00));
        assert_eq!(DARK.cursor, rgb(0xD5, 0x5E, 0x00));
    }

    #[test]
    fn the_dark_theme_changes_the_surround_not_the_paper() {
        assert_ne!(LIGHT.surround, DARK.surround);
        assert_eq!(LIGHT.paper, DARK.paper);
        assert_eq!(LIGHT.hover, rgb(0x00, 0x72, 0xB2));
    }
}
