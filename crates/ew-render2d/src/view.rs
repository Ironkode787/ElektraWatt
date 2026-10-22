// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The view transform (WP-S2.02 §6.1, §6.3): sheet nanometres (origin top-left, y down) to physical pixels of the
//! pane (origin at the pane's top-left corner).
//!
//! A [`View`] is what the canvas model holds: the sheet point at the pane's centre, the zoom as a factor of paper
//! size (100 % shows 1 mm of paper as 96/25.4 logical points), the pane in logical points and the pixels per point.
//! Its display resolution is `D = zoom × 96 × ppp` dpi. While the view moves it is drawn with the exact transform
//! ([`View::transform`]); once it has settled it is drawn at the whole resolution `round(D)` with the sheet origin
//! on a whole pixel ([`View::settled_transform`]), so an idle canvas shows the CPU raster pixel for pixel.

/// Nanometres per inch.
pub const NM_PER_INCH: f64 = 25_400_000.0;

/// Logical points per inch of paper at 100 % (the CSS convention: the page keeps its physical size).
pub const POINTS_PER_INCH: f64 = 96.0;

/// What the canvas shows: the canvas model's view state.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct View {
    /// The sheet point (nm) at the pane's centre.
    pub centre: (i64, i64),
    /// The zoom as a factor of paper size (1.0 = 100 %).
    pub zoom: f64,
    /// The pane's size in logical points.
    pub pane: (f64, f64),
    /// Physical pixels per logical point.
    pub ppp: f64,
}

impl View {
    /// The display resolution `D = zoom × 96 × ppp` in dpi.
    pub fn display_dpi(&self) -> f64 {
        self.zoom * POINTS_PER_INCH * self.ppp
    }

    /// The pane in physical pixels, unrounded.
    pub fn pane_px(&self) -> (f64, f64) {
        (self.pane.0 * self.ppp, self.pane.1 * self.ppp)
    }

    /// The pane in whole physical pixels (each side at least 1): the size of the headless snapshot.
    pub fn pane_px_whole(&self) -> (u32, u32) {
        let side = |v: f64| {
            if v.is_finite() {
                v.round().clamp(1.0, f64::from(u32::MAX)) as u32
            } else {
                1
            }
        };
        let (w, h) = self.pane_px();
        (side(w), side(h))
    }

    /// The exact transform at `D`: the pane's centre shows [`View::centre`].
    pub fn transform(&self) -> DrawTransform {
        let dpi = self.display_dpi();
        let k = dpi / NM_PER_INCH;
        let (w, h) = self.pane_px();
        DrawTransform {
            dpi,
            origin: (
                w / 2.0 - self.centre.0 as f64 * k,
                h / 2.0 - self.centre.1 as f64 * k,
            ),
            raster_dpi: None,
        }
    }

    /// The whole display resolution `round(D)` (at least 1).
    pub fn settled_dpi(&self) -> u32 {
        let d = self.display_dpi().round();
        if d.is_finite() {
            d.clamp(1.0, f64::from(u32::MAX)) as u32
        } else {
            1
        }
    }

    /// The settled transform: resolution `round(D)` and the sheet origin rounded to a whole pixel, so one raster
    /// pixel at that resolution is one screen pixel.
    pub fn settled_transform(&self) -> DrawTransform {
        let r = self.settled_dpi();
        let k = f64::from(r) / NM_PER_INCH;
        let (w, h) = self.pane_px();
        DrawTransform {
            dpi: f64::from(r),
            origin: (
                (w / 2.0 - self.centre.0 as f64 * k).round(),
                (h / 2.0 - self.centre.1 as f64 * k).round(),
            ),
            raster_dpi: Some(r),
        }
    }
}

/// Sheet nanometres to screen pixels: `screen = origin + nm × dpi / NM_PER_INCH`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DrawTransform {
    /// The resolution the sheet is drawn at, in dpi.
    pub dpi: f64,
    /// Where the sheet's origin lands, in physical pixels of the pane.
    pub origin: (f64, f64),
    /// `Some(dpi)` for a settled transform: the origin is a whole pixel and one raster pixel at this resolution is
    /// one screen pixel.
    pub raster_dpi: Option<u32>,
}

impl DrawTransform {
    /// Pixels per nanometre.
    pub fn px_per_nm(&self) -> f64 {
        self.dpi / NM_PER_INCH
    }

    /// The screen pixel position of a sheet point (nm).
    pub fn to_screen(&self, nm: (f64, f64)) -> (f64, f64) {
        let k = self.px_per_nm();
        (self.origin.0 + nm.0 * k, self.origin.1 + nm.1 * k)
    }

    /// The sheet point (nm) at a screen pixel position.
    pub fn to_sheet(&self, px: (f64, f64)) -> (f64, f64) {
        let k = self.px_per_nm();
        ((px.0 - self.origin.0) / k, (px.1 - self.origin.1) / k)
    }

    /// The screen position of raster coordinate `v` (pixels) of the full-sheet raster at `raster_dpi`, along one
    /// axis whose origin is `origin`.
    pub fn raster_to_screen(&self, origin: f64, raster_dpi: u32, v: f64) -> f64 {
        origin + v * self.dpi / f64::from(raster_dpi)
    }
}

/// A rectangle of whole physical pixels of the pane, `[x0, x1) × [y0, y1)`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct PxRect {
    /// The first column.
    pub x0: i64,
    /// The first row.
    pub y0: i64,
    /// One past the last column.
    pub x1: i64,
    /// One past the last row.
    pub y1: i64,
}

impl PxRect {
    /// The rectangle `[x0, x1) × [y0, y1)`.
    pub const fn new(x0: i64, y0: i64, x1: i64, y1: i64) -> PxRect {
        PxRect { x0, y0, x1, y1 }
    }

    /// Whether it covers no pixel.
    pub fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }

    /// The pixels in both rectangles (possibly empty).
    pub fn intersect(&self, other: &PxRect) -> PxRect {
        PxRect {
            x0: self.x0.max(other.x0),
            y0: self.y0.max(other.y0),
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
        }
    }
}

/// `v` rounded to the nearest whole pixel, saturating at the `i64` range (NaN is 0).
pub fn round_px(v: f64) -> i64 {
    // `as` saturates and maps NaN to 0.
    v.round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View {
        View {
            centre: (148_500_000, 105_000_000),
            zoom: 1.0,
            pane: (1200.0, 800.0),
            ppp: 1.0,
        }
    }

    #[test]
    fn the_centre_is_at_the_pane_centre() {
        let v = view();
        let t = v.transform();
        assert_eq!(v.display_dpi(), 96.0);
        let c = t.to_screen((148_500_000.0, 105_000_000.0));
        assert!((c.0 - 600.0).abs() < 1e-9 && (c.1 - 400.0).abs() < 1e-9, "{c:?}");
        // 1 inch of paper is 96 pixels at 100 % and ppp 1.
        let one_inch = t.to_screen((148_500_000.0 + NM_PER_INCH, 105_000_000.0));
        assert!((one_inch.0 - 696.0).abs() < 1e-9);
        let back = t.to_sheet(one_inch);
        assert!((back.0 - (148_500_000.0 + NM_PER_INCH)).abs() < 1e-3);
    }

    #[test]
    fn ppp_scales_pixels_not_paper() {
        let mut v = view();
        v.ppp = 2.0;
        assert_eq!(v.display_dpi(), 192.0);
        assert_eq!(v.pane_px(), (2400.0, 1600.0));
        assert_eq!(v.pane_px_whole(), (2400, 1600));
    }

    #[test]
    fn the_settled_transform_has_a_whole_origin_and_resolution() {
        let mut v = view();
        v.zoom = 1.5625;
        v.centre = (148_500_001, 104_999_999);
        let s = v.settled_transform();
        assert_eq!(s.raster_dpi, Some(150));
        assert_eq!(s.dpi, 150.0);
        assert_eq!(s.origin.0.fract(), 0.0);
        assert_eq!(s.origin.1.fract(), 0.0);
        let exact = v.transform();
        assert!((s.origin.0 - exact.origin.0).abs() <= 0.5 + 1e-9);
        // A raster pixel at the settled resolution is one screen pixel.
        assert_eq!(s.raster_to_screen(s.origin.0, 150, 10.0) - s.origin.0, 10.0);
    }
}
