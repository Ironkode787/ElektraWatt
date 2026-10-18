// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic numerics: rotations, arcs, bulges and the nm → pt conversion.
//!
//! # Why these must be bit-identical
//!
//! PDF and SVG exports are compared byte for byte across Windows, macOS and Linux (FR-OUT-019, NFR-REL-07;
//! doc 09 §4.3). A rotated text anchor or an arc control point that differs in its last bit on one platform
//! rounds to a different nanometre now and then, and the export hash breaks. Doc 09 §4.3 rule 4 therefore
//! requires:
//!
//! - **No platform trigonometry.** `f64::sin`, `cos`, `tan`, `atan2` and `sin_cos` call the platform's
//!   libm, whose last bit differs between C libraries. Everything here uses the pure-Rust [`libm`] crate,
//!   built without its `arch` feature, so every platform runs the same code.
//! - **No fused multiply-add.** Rust never contracts `a * b + c` into an FMA, and this module never calls
//!   `mul_add`. Every expression is evaluated in the written order with IEEE 754 binary64 rounding.
//! - **Exact where exact is possible.** Rotations by multiples of 90° use no trigonometry at all; arc end
//!   points on multiples of 90° are exact.
//! - **kurbo's arc conversion is not used on the export path:** `kurbo::Arc` calls `f64::sin_cos`. kurbo
//!   appears in this crate only in the f64 conversions for renderers and picking
//!   ([`Path::to_kurbo`](crate::Path::to_kurbo), [`Transform::to_affine`](crate::Transform::to_affine)).
//!
//! Supported targets have SSE2 or an equivalent binary64 unit (x86-64, AArch64). An x87-only target (i586)
//! would evaluate in extended precision and is not supported for export.
//!
//! # Angle sense
//!
//! A positive [`Angle2`] turns **counter-clockwise as seen by the viewer on the sheet**, and 0 is the +x
//! direction (product-owner decision of 2026-10-18). Sheet y points down, so a turn by +90° maps
//! `(x, y) → (y, −x)`: the +x axis goes to "up" on the sheet. This is the opposite sense of
//! [`Orient2::R90`](ew_units::Orient2::R90), which is a quarter turn clockwise: `R90` equals a turn by −90°.
//! QET arcs (0° at 3 o'clock, positive span counter-clockwise on screen; doc 14 §4.3) and DXF angles and
//! bulges (counter-clockwise in DXF's y-up view, which the importer keeps visually) use this sense directly.
//!
//! # Rounding
//!
//! Every f64 result is rounded to the nearest nanometre with exact ties **toward zero**, like `ew-units`.
//! The rounded quantity is always an **offset from an integer base point** (the rotation centre, the arc
//! centre, the bulge start), which is then added exactly; so every helper commutes exactly with integer
//! translations. [`nm_to_milli_pt`] is the exception: doc 09 §4.3 rule 4 asks for half to even there.

use std::f64::consts::{FRAC_PI_2, PI};

use ew_units::{Angle2, Nm, Pt2};

use crate::PathEl;

/// Radians per 10⁻³ degree (the unit of [`Angle2`]), `π / 180 000`, evaluated once at compile time.
const RAD_PER_MDEG: f64 = PI / 180_000.0;

/// 10⁻³ degree per quarter turn.
const QUARTER: i64 = 90_000;

/// `v` rounded to the nearest integer, exact ties toward zero; saturating at the `i64` range, NaN → 0.
///
/// `v - v.trunc()` is exact in IEEE 754, so the tie test sees the true fraction.
pub(crate) fn round_tz(v: f64) -> i64 {
    let t = libm::trunc(v);
    let frac = v - t;
    let r = if libm::fabs(frac) > 0.5 {
        t + libm::copysign(1.0, frac)
    } else {
        t
    };
    // `as` saturates and maps NaN to 0.
    r as i64
}

/// `n / d` rounded to the nearest integer, exact ties toward zero. `d` must be non-zero.
pub(crate) fn div_round_tz(n: i128, d: i128) -> i128 {
    let q = n / d;
    let r = n % d;
    let (ar, ad) = (r.unsigned_abs(), d.unsigned_abs());
    if ar > ad - ar {
        if (n < 0) == (d < 0) { q + 1 } else { q - 1 }
    } else {
        q
    }
}

/// `n / d` rounded to the nearest integer, exact ties to the even neighbour. `d` must be non-zero.
fn div_round_even(n: i128, d: i128) -> i128 {
    let q = n / d;
    let r = n % d;
    let (ar, ad) = (r.unsigned_abs(), d.unsigned_abs());
    let away = if (n < 0) == (d < 0) { q + 1 } else { q - 1 };
    match ar.cmp(&(ad - ar)) {
        std::cmp::Ordering::Greater => away,
        std::cmp::Ordering::Less => q,
        std::cmp::Ordering::Equal => {
            if q % 2 == 0 {
                q
            } else {
                away
            }
        }
    }
}

/// `(sin, cos)` of an angle given in 10⁻³ degree as f64. The quadrant is reduced first, so multiples of 90°
/// give exact 0 and ±1, and the libm call only ever sees an argument in `(0, π/2)`.
fn sin_cos_mdeg(mdeg: f64) -> (f64, f64) {
    let q = libm::floor(mdeg / 90_000.0);
    let mut r = mdeg - q * 90_000.0;
    let mut q = q as i64;
    if r < 0.0 {
        r += 90_000.0;
        q -= 1;
    } else if r >= 90_000.0 {
        r -= 90_000.0;
        q += 1;
    }
    let (s, c) = if r == 0.0 {
        (0.0, 1.0)
    } else {
        let t = r * RAD_PER_MDEG;
        (libm::sin(t), libm::cos(t))
    };
    match q.rem_euclid(4) {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

/// `(sin, cos)` of `angle`, deterministic on every platform; exact 0 and ±1 for multiples of 90°.
///
/// ```
/// use ew_dl::num::sin_cos;
/// use ew_units::Angle2;
///
/// assert_eq!(sin_cos(Angle2(90_000)), (1.0, 0.0));
/// assert_eq!(sin_cos(Angle2(-180_000)), (0.0, -1.0));
/// ```
pub fn sin_cos(angle: Angle2) -> (f64, f64) {
    sin_cos_mdeg(f64::from(angle.0))
}

/// Rotates the offset `(x, y)` by `a`, counter-clockwise on the sheet. Exact for multiples of 90°;
/// otherwise f64 via [`libm`], rounded ties toward zero.
fn rotate_offset(x: i64, y: i64, a: Angle2) -> (i64, i64) {
    match a.normalized().0 {
        0 => (x, y),
        90_000 => (y, -x),
        180_000 => (-x, -y),
        270_000 => (-y, x),
        _ => {
            let (s, c) = sin_cos(a);
            let (xf, yf) = (x as f64, y as f64);
            (round_tz(xf * c + yf * s), round_tz(yf * c - xf * s))
        }
    }
}

/// Rotates `p` about `about` by `angle`, counter-clockwise on the sheet for a positive angle.
///
/// Multiples of 90° are exact integer maps with no trigonometry: +90° is `(dx, dy) → (dy, −dx)` about
/// `about`. Other angles compute `(dx·cos + dy·sin, dy·cos − dx·sin)` in f64 with [`libm`] and round the
/// offset to whole nanometres, ties toward zero, before adding `about`. Offsets must stay within ±2⁵³ nm
/// (the persisted range) for the f64 step to be exact on its input; the integer steps follow Rust's
/// overflow semantics.
///
/// ```
/// use ew_dl::num::rotate;
/// use ew_units::{Angle2, Nm, Pt2};
///
/// let p = Pt2::new(Nm(10), Nm(0));
/// // +90° turns +x to "up" on the sheet (negative y).
/// assert_eq!(rotate(p, Angle2(90_000), Pt2::ORIGIN), Pt2::new(Nm(0), Nm(-10)));
/// assert_eq!(rotate(p, Angle2(45_000), Pt2::ORIGIN), Pt2::new(Nm(7), Nm(-7)));
/// ```
pub fn rotate(p: Pt2, angle: Angle2, about: Pt2) -> Pt2 {
    let d = p - about;
    let (x, y) = rotate_offset(d.x.0, d.y.0, angle);
    about + Pt2::new(Nm(x), Nm(y))
}

/// The point at `angle` on the axis-aligned ellipse with centre `center` and radii `rx`, `ry`:
/// `(cx + rx·cos θ, cy − ry·sin θ)`, the QET convention (0° at 3 o'clock, positive angles counter-clockwise
/// on the sheet). The offset is rounded ties toward zero; multiples of 90° are exact.
///
/// ```
/// use ew_dl::num::arc_point;
/// use ew_units::{Angle2, Nm, Pt2};
///
/// let c = Pt2::new(Nm(100), Nm(100));
/// assert_eq!(arc_point(c, Nm(10), Nm(5), Angle2(90_000)), Pt2::new(Nm(100), Nm(95)));
/// ```
pub fn arc_point(center: Pt2, rx: Nm, ry: Nm, angle: Angle2) -> Pt2 {
    let (s, c) = sin_cos(angle);
    center + Pt2::new(Nm(round_tz(rx.0 as f64 * c)), Nm(round_tz(-(ry.0 as f64) * s)))
}

/// An elliptic arc as cubic Béziers: the arc of the axis-aligned ellipse (`center`, `rx`, `ry`) from
/// `start` over `sweep`, in the QET convention (0° at 3 o'clock, a positive sweep runs counter-clockwise on
/// the sheet; a point is `(cx + rx·cos θ, cy − ry·sin θ)`).
///
/// Returns only [`PathEl::CubicTo`] elements; the arc starts at
/// [`arc_point(center, rx, ry, start)`](arc_point), where the caller moves or draws to first. A zero sweep
/// returns no elements. The sweep is split into `n = ⌈|sweep| / 90°⌉` equal segments; segment `i` runs from
/// `θᵢ = start + sweep·i/n` (evaluated in f64 as `start + (sweep·i) / n`, so multiples of 90° stay exact) to
/// `θᵢ₊₁`, with handle length `k = 4/3 · tan(δ/4)`, `δ = sweep/n`:
///
/// - `P₁ = E(θᵢ) + k·E′(θᵢ)`, `P₂ = E(θᵢ₊₁) − k·E′(θᵢ₊₁)`, `P₃ = E(θᵢ₊₁)`,
/// - `E′(θ) = (−rx·sin θ, −ry·cos θ)`.
///
/// Each point's offset from `center` is rounded ties toward zero. Adjacent segments share the rounded end
/// point, so the arc is continuous.
///
/// ```
/// use ew_dl::{PathEl, num::arc_to_cubics};
/// use ew_units::{Angle2, Nm, Pt2};
///
/// let els = arc_to_cubics(Pt2::ORIGIN, Nm(1_000_000), Nm(1_000_000), Angle2(0), Angle2(90_000));
/// assert_eq!(
///     els,
///     [PathEl::CubicTo(
///         Pt2::new(Nm(1_000_000), Nm(-552_285)),
///         Pt2::new(Nm(552_285), Nm(-1_000_000)),
///         Pt2::new(Nm(0), Nm(-1_000_000)),
///     )]
/// );
/// ```
pub fn arc_to_cubics(center: Pt2, rx: Nm, ry: Nm, start: Angle2, sweep: Angle2) -> Vec<PathEl> {
    let total = i64::from(sweep.0);
    if total == 0 {
        return Vec::new();
    }
    let n = (total.unsigned_abs() as i64 + QUARTER - 1) / QUARTER;
    let nf = n as f64;
    let delta = total as f64 / nf;
    let k = 4.0 / 3.0 * libm::tan(delta * RAD_PER_MDEG / 4.0);
    let (rxf, ryf) = (rx.0 as f64, ry.0 as f64);
    let start = f64::from(start.0);
    // (point offset, derivative) at θᵢ.
    let at = |i: i64| {
        let theta = start + (total * i) as f64 / nf;
        let (s, c) = sin_cos_mdeg(theta);
        ((rxf * c, -(ryf * s)), (-(rxf * s), -(ryf * c)))
    };
    let pt = |x: f64, y: f64| center + Pt2::new(Nm(round_tz(x)), Nm(round_tz(y)));
    let mut out = Vec::with_capacity(n as usize);
    let (mut p0, mut d0) = at(0);
    for i in 1..=n {
        let (p3, d3) = at(i);
        out.push(PathEl::CubicTo(
            pt(p0.0 + k * d0.0, p0.1 + k * d0.1),
            pt(p3.0 - k * d3.0, p3.1 - k * d3.1),
            pt(p3.0, p3.1),
        ));
        (p0, d0) = (p3, d3);
    }
    out
}

/// A DXF bulge segment from `p0` to `p1` as cubic Béziers, in sheet coordinates.
///
/// `bulge = tan(θ/4)`, where θ is the included angle of the arc; a positive bulge runs **counter-clockwise
/// on the sheet** (the crate's angle sense). DXF defines the bulge counter-clockwise in its y-up view; the
/// importer flips y to keep the drawing's look, so it passes the DXF value unchanged.
///
/// Returns only drawing elements (the segment starts at `p0`, where the caller already is). A zero or
/// non-finite bulge, or `p0 == p1`, gives the straight [`PathEl::LineTo`]`(p1)`. Otherwise the arc is split
/// into `n = ⌈|θ| / 90°⌉` equal segments (`n ≤ 4`), with handle length `k = 4/3 · tan(θ/(4n))`.
///
/// **Rounding, exactly:** with chord `d = p1 − p0` and `h = (1 − b²) / (4b)`, the start offset from the
/// centre is `v₀ = (−dₓ/2 − h·d_y, −d_y/2 + h·dₓ)`; `vᵢ` is `v₀` turned by `θ·i/n`. Every point other than
/// `p0` and `p1` is `p0 + round(offset)` with the offset `vᵢ − v₀ ± k·J·vᵢ` (`J(x, y) = (y, −x)`) evaluated
/// in f64 with [`libm`] and rounded ties toward zero. The end points are `p0` and `p1` themselves, and
/// adjacent segments share their rounded junction.
///
/// ```
/// use ew_dl::{PathEl, num::bulge_to_cubics};
/// use ew_units::{Nm, Pt2};
///
/// // A half circle (bulge 1) from (−1, 0) mm to (1, 0) mm, counter-clockwise on the sheet: through the
/// // bottom, since a counter-clockwise turn from the left point goes down first.
/// let p0 = Pt2::new(Nm(-1_000_000), Nm(0));
/// let p1 = Pt2::new(Nm(1_000_000), Nm(0));
/// let els = bulge_to_cubics(p0, p1, 1.0);
/// assert_eq!(els.len(), 2);
/// assert!(matches!(els[0], PathEl::CubicTo(_, _, mid) if mid == Pt2::new(Nm(0), Nm(1_000_000))));
/// assert!(matches!(els[1], PathEl::CubicTo(_, _, end) if end == p1));
/// ```
pub fn bulge_to_cubics(p0: Pt2, p1: Pt2, bulge: f64) -> Vec<PathEl> {
    if !bulge.is_finite() || bulge == 0.0 || p0 == p1 {
        return vec![PathEl::LineTo(p1)];
    }
    let d = p1 - p0;
    let (dx, dy) = (d.x.0 as f64, d.y.0 as f64);
    let theta = 4.0 * libm::atan(bulge);
    let n = (libm::ceil(libm::fabs(theta) / FRAC_PI_2) as i64).clamp(1, 4);
    let nf = n as f64;
    let k = 4.0 / 3.0 * libm::tan(theta / nf / 4.0);
    let h = (1.0 - bulge * bulge) / (4.0 * bulge);
    let v0 = (-0.5 * dx - h * dy, -0.5 * dy + h * dx);
    // vᵢ: v₀ turned by φ = θ·i/n (counter-clockwise on the sheet).
    let v = |i: i64| {
        if i == 0 {
            return v0;
        }
        let phi = theta * i as f64 / nf;
        let (s, c) = (libm::sin(phi), libm::cos(phi));
        (v0.0 * c + v0.1 * s, v0.1 * c - v0.0 * s)
    };
    let pt = |x: f64, y: f64| p0 + Pt2::new(Nm(round_tz(x)), Nm(round_tz(y)));
    let mut out = Vec::with_capacity(n as usize);
    let mut a = v0;
    for i in 1..=n {
        let b = v(i);
        // Offsets from p0: wᵢ = vᵢ − v₀; tangents J·v = (v.y, −v.x).
        let c1 = pt(a.0 - v0.0 + k * a.1, a.1 - v0.1 - k * a.0);
        let c2 = pt(b.0 - v0.0 - k * b.1, b.1 - v0.1 + k * b.0);
        let end = if i == n { p1 } else { pt(b.0 - v0.0, b.1 - v0.1) };
        out.push(PathEl::CubicTo(c1, c2, end));
        a = b;
    }
    out
}

/// The length of the vector `v`, `√(x² + y²)` via [`libm`], rounded ties toward zero.
///
/// ```
/// use ew_dl::num::length;
/// use ew_units::{Nm, Pt2};
///
/// assert_eq!(length(Pt2::new(Nm(3), Nm(-4))), Nm(5));
/// ```
pub fn length(v: Pt2) -> Nm {
    let (x, y) = (v.x.0 as f64, v.y.0 as f64);
    Nm(round_tz(libm::sqrt(x * x + y * y)))
}

/// The direction of the vector `v` as an [`Angle2`] in `[0°, 360°)`, in the crate's angle sense (0 = +x,
/// positive counter-clockwise on the sheet, so "up" on the sheet is 90°): `atan2(−y, x)` via [`libm`], rounded to
/// 10⁻³ degree ties toward zero. Exact for the axis directions; the zero vector gives 0.
///
/// ```
/// use ew_dl::num::angle_of;
/// use ew_units::{Angle2, Nm, Pt2};
///
/// assert_eq!(angle_of(Pt2::new(Nm(0), Nm(-5))), Angle2(90_000));
/// assert_eq!(angle_of(Pt2::new(Nm(1), Nm(1))), Angle2(315_000));
/// ```
pub fn angle_of(v: Pt2) -> Angle2 {
    let (x, y) = (v.x.0, v.y.0);
    match (x.signum(), y.signum()) {
        (0, 0) | (1, 0) => return Angle2::ZERO,
        (0, -1) => return Angle2(90_000),
        (-1, 0) => return Angle2(180_000),
        (0, 1) => return Angle2(270_000),
        _ => {}
    }
    let rad = libm::atan2(-(y as f64), x as f64);
    let mdeg = round_tz(rad / RAD_PER_MDEG);
    Angle2(i32::try_from(mdeg).unwrap_or(0)).normalized()
}

/// Nanometres to 10⁻³ PostScript point: `nm × 72 / 25 400`, rounded **half to even**, exact in `i128`
/// (doc 09 §4.3 rule 4). The result always fits, since it is smaller in magnitude than the input.
///
/// ```
/// use ew_dl::num::nm_to_milli_pt;
/// use ew_units::Nm;
///
/// assert_eq!(nm_to_milli_pt(Nm(25_400_000)), 72_000); // one inch
/// assert_eq!(nm_to_milli_pt(Nm(1_764)), 5); // 5.0003… → 5
/// ```
pub fn nm_to_milli_pt(nm: Nm) -> i64 {
    // 72 / 25 400 = 9 / 3175.
    div_round_even(i128::from(nm.0) * 9, 3175) as i64
}

/// 10⁻³ PostScript point to nanometres: `mpt × 25 400 / 72`, rounded half to even, exact in `i128`; `None`
/// if the result leaves the `i64` range. The inverse of [`nm_to_milli_pt`] up to its rounding: every
/// `mpt` survives `nm_to_milli_pt(milli_pt_to_nm(mpt))`.
///
/// ```
/// use ew_dl::num::{milli_pt_to_nm, nm_to_milli_pt};
/// use ew_units::Nm;
///
/// assert_eq!(milli_pt_to_nm(72_000), Some(Nm(25_400_000)));
/// assert_eq!(nm_to_milli_pt(milli_pt_to_nm(1).unwrap()), 1);
/// ```
pub fn milli_pt_to_nm(mpt: i64) -> Option<Nm> {
    i64::try_from(div_round_even(i128::from(mpt) * 3175, 9))
        .ok()
        .map(Nm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    #[test]
    fn rounding_helpers() {
        assert_eq!(round_tz(2.5), 2);
        assert_eq!(round_tz(-2.5), -2);
        assert_eq!(round_tz(2.500_000_000_000_000_4), 3);
        assert_eq!(round_tz(-1.75), -2);
        assert_eq!(round_tz(f64::NAN), 0);
        assert_eq!(round_tz(1e300), i64::MAX);
        assert_eq!(div_round_tz(5, 2), 2);
        assert_eq!(div_round_tz(-5, 2), -2);
        assert_eq!(div_round_tz(7, 4), 2);
        assert_eq!(div_round_even(5, 2), 2);
        assert_eq!(div_round_even(7, 2), 4);
        assert_eq!(div_round_even(-5, 2), -2);
        assert_eq!(div_round_even(-7, 2), -4);
        assert_eq!(div_round_even(5, -2), -2);
        assert_eq!(div_round_even(9, 4), 2);
        assert_eq!(div_round_even(11, 4), 3);
    }

    #[test]
    fn sin_cos_quadrants_are_exact() {
        assert_eq!(sin_cos(Angle2(0)), (0.0, 1.0));
        assert_eq!(sin_cos(Angle2(90_000)), (1.0, 0.0));
        assert_eq!(sin_cos(Angle2(180_000)), (0.0, -1.0));
        assert_eq!(sin_cos(Angle2(270_000)), (-1.0, 0.0));
        assert_eq!(sin_cos(Angle2(-90_000)), (-1.0, 0.0));
        assert_eq!(sin_cos(Angle2(720_000)), (0.0, 1.0));
        // Signs of zero do not matter for the products, but values do.
        let (s, c) = sin_cos(Angle2(30_000));
        assert!((s - 0.5).abs() < 1e-15 && (c - 3f64.sqrt() / 2.0).abs() < 1e-15);
        let (s2, c2) = sin_cos(Angle2(210_000));
        assert_eq!((s2, c2), (-s, -c));
    }

    #[test]
    fn exact_quarter_turns() {
        let q = p(3, 7);
        let about = p(100, -50);
        assert_eq!(rotate(q, Angle2(0), about), q);
        assert_eq!(rotate(p(103, -43), Angle2(90_000), about), p(107, -53));
        assert_eq!(rotate(p(103, -43), Angle2(180_000), about), p(97, -57));
        assert_eq!(rotate(p(103, -43), Angle2(270_000), about), p(93, -47));
        assert_eq!(rotate(p(103, -43), Angle2(-90_000), about), p(93, -47));
        assert_eq!(rotate(p(103, -43), Angle2(450_000), about), p(107, -53));
        // Large coordinates stay exact (no f64 involved).
        let big = p(i64::MAX / 4, -(i64::MAX / 4));
        assert_eq!(
            rotate(big, Angle2(90_000), Pt2::ORIGIN),
            p(-(i64::MAX / 4), -(i64::MAX / 4))
        );
    }

    /// `Orient2::R90` (clockwise) equals a turn by −90°.
    #[test]
    fn orient_r90_is_minus_ninety() {
        for q in [p(1, 0), p(0, 1), p(5, -3)] {
            assert_eq!(
                ew_units::Orient2::R90.apply(q),
                rotate(q, Angle2(-90_000), Pt2::ORIGIN)
            );
            assert_eq!(
                ew_units::Orient2::R270.apply(q),
                rotate(q, Angle2(90_000), Pt2::ORIGIN)
            );
        }
    }

    fn cubic(c1: (i64, i64), c2: (i64, i64), end: (i64, i64)) -> PathEl {
        PathEl::CubicTo(p(c1.0, c1.1), p(c2.0, c2.1), p(end.0, end.1))
    }

    /// Fixed vectors for the libm path, computed once and cross-checked against an independent
    /// re-implementation of the same formulas. They must hold bit for bit on every CI OS.
    #[test]
    fn rotation_vectors() {
        let cases: [(Pt2, i32, Pt2); 6] = [
            (p(1_000_000, 0), 45_000, p(707_107, -707_107)),
            (p(1_000_000, 0), 30_000, p(866_025, -500_000)),
            (p(12_345_678, -9_876_543), 12_345, p(9_948_639, -12_287_655)),
            (p(-250_000_000, 175_000_000), 333_333, p(-301_948_274, 44_184_160)),
            (p(1, 1), 1, p(1, 1)),
            (
                p(420_000_000_000, 297_000_000_000),
                271_828,
                p(-283_451_187_304, 429_260_322_434),
            ),
        ];
        for (q, a, want) in cases {
            assert_eq!(rotate(q, Angle2(a), Pt2::ORIGIN), want, "{q:?} by {a}");
        }
    }

    #[test]
    fn arc_vectors() {
        // A quarter circle: the standard 4/3·tan(22.5°) handle.
        let r = Nm(1_000_000);
        assert_eq!(
            arc_to_cubics(Pt2::ORIGIN, r, r, Angle2(0), Angle2(90_000)),
            [cubic(
                (1_000_000, -552_285),
                (552_285, -1_000_000),
                (0, -1_000_000)
            )]
        );
        // A QET-style elliptic arc: off-grid centre, clockwise sweep of 135° from 30° to −105°, in two
        // segments joined at −37.5°.
        let c = p(5_000_000, 3_000_000);
        assert_eq!(
            arc_point(c, Nm(4_000_000), Nm(2_500_000), Angle2(30_000)),
            p(8_464_102, 1_750_000)
        );
        assert_eq!(
            arc_to_cubics(c, Nm(4_000_000), Nm(2_500_000), Angle2(30_000), Angle2(-135_000)),
            [
                cubic(
                    (9_273_026, 2_625_686),
                    (9_158_297, 3_719_700),
                    (8_173_413, 4_521_904)
                ),
                cubic(
                    (7_188_529, 5_324_107),
                    (5_527_446, 5_676_521),
                    (3_964_724, 5_414_815)
                ),
            ]
        );
        // A full ellipse returns exactly to its start point; the quarter points are exact.
        assert_eq!(
            arc_to_cubics(p(7, 9), Nm(3_000_000), Nm(1_000_000), Angle2(0), Angle2(360_000)),
            [
                cubic((3_000_007, -552_276), (1_656_861, -999_991), (7, -999_991)),
                cubic((-1_656_847, -999_991), (-2_999_993, -552_276), (-2_999_993, 9)),
                cubic((-2_999_993, 552_294), (-1_656_847, 1_000_009), (7, 1_000_009)),
                cubic((1_656_861, 1_000_009), (3_000_007, 552_294), (3_000_007, 9)),
            ]
        );
        assert!(arc_to_cubics(Pt2::ORIGIN, r, r, Angle2(10_000), Angle2(0)).is_empty());
    }

    #[test]
    fn bulge_vectors() {
        let p0 = p(-1_000_000, 0);
        let p1 = p(1_000_000, 0);
        assert_eq!(bulge_to_cubics(p0, p1, 0.0), [PathEl::LineTo(p1)]);
        assert_eq!(bulge_to_cubics(p0, p1, f64::NAN), [PathEl::LineTo(p1)]);
        assert_eq!(bulge_to_cubics(p0, p1, f64::INFINITY), [PathEl::LineTo(p1)]);
        assert_eq!(bulge_to_cubics(p0, p0, 1.0), [PathEl::LineTo(p0)]);
        // A half circle counter-clockwise on the sheet: from the left point down through the bottom.
        let half = [
            cubic((-1_000_000, 552_285), (-552_285, 1_000_000), (0, 1_000_000)),
            cubic((552_285, 1_000_000), (1_000_000, 552_285), (1_000_000, 0)),
        ];
        assert_eq!(bulge_to_cubics(p0, p1, 1.0), half);
        // A negative bulge mirrors about the chord.
        let neg = bulge_to_cubics(p0, p1, -1.0);
        assert_eq!(neg.len(), 2);
        for (a, b) in neg.iter().zip(half.iter()) {
            assert_eq!(*a, b.map(|q| Pt2::new(q.x, -q.y)));
        }
        // An off-axis segment of 66.8° (radius 3.547 mm).
        assert_eq!(
            bulge_to_cubics(p(3_000_000, 1_000_000), p(5_500_000, 4_000_000), 0.3),
            [cubic(
                (3_158_333, 2_410_000),
                (4_141_667, 3_590_000),
                (5_500_000, 4_000_000)
            )]
        );
    }

    #[test]
    fn length_and_angle() {
        assert_eq!(length(p(0, 0)), Nm(0));
        assert_eq!(length(p(-6, 8)), Nm(10));
        assert_eq!(length(p(1_000_000, 1_000_000)), Nm(1_414_214));
        assert_eq!(angle_of(p(0, 0)), Angle2(0));
        assert_eq!(angle_of(p(5, 0)), Angle2(0));
        assert_eq!(angle_of(p(-5, 0)), Angle2(180_000));
        assert_eq!(angle_of(p(0, 5)), Angle2(270_000));
        assert_eq!(angle_of(p(1, -1)), Angle2(45_000));
        assert_eq!(angle_of(p(-1, -1)), Angle2(135_000));
        assert_eq!(angle_of(p(866_025, -500_000)), Angle2(30_000));
    }

    /// The conversion can never meet an exact tie: `nm × 9 / 3175` is a tie only if
    /// `18·nm ≡ 3175 (mod 6350)`, even against odd; the inverse `mpt × 3175 / 9` only if `6350·mpt ≡ 9 (mod 18)`, again even against
    /// odd. Half-to-even is still what doc 09 §4.3 rule 4 asks for, so the helper's ties are tested directly.
    #[test]
    fn milli_pt_tie_cases() {
        assert_eq!(div_round_even(5, 2), 2);
        assert_eq!(div_round_even(15, 2), 8);
        assert_eq!(div_round_even(-15, 2), -8);
        assert_eq!(div_round_even(-5, 2), -2);
        for nm in -20_000i64..20_000 {
            assert_ne!((i128::from(nm) * 18).rem_euclid(6350), 3175);
        }
        // The nearest neighbours of a would-be tie at 4.5 milli-pt (1587.5 nm).
        assert_eq!(nm_to_milli_pt(Nm(1_587)), 4); // 4.4986…
        assert_eq!(nm_to_milli_pt(Nm(1_588)), 5); // 4.5014…
        assert_eq!(nm_to_milli_pt(Nm(-1_587)), -4);
        assert_eq!(nm_to_milli_pt(Nm(-1_588)), -5);
        assert_eq!(nm_to_milli_pt(Nm(352_778)), 1_000); // 1 pt = 352 777.7… nm
        assert_eq!(nm_to_milli_pt(Nm(-352_778)), -1_000);
        assert_eq!(nm_to_milli_pt(Nm(25_400_000)), 72_000);
        // i64::MAX × 9 / 3175 = 26 144 991 600 533 222.76…
        assert_eq!(nm_to_milli_pt(Nm(i64::MAX)), 26_144_991_600_533_223);
        assert_eq!(nm_to_milli_pt(Nm(i64::MIN)), -26_144_991_600_533_223);
        assert_eq!(milli_pt_to_nm(1), Some(Nm(353))); // 352.78 → 353
        assert_eq!(milli_pt_to_nm(-1), Some(Nm(-353)));
        assert_eq!(milli_pt_to_nm(9), Some(Nm(3_175)));
        assert_eq!(milli_pt_to_nm(i64::MAX), None);
    }

    proptest! {
        #[test]
        fn rotate_there_and_back(x in -1_000_000_000_000i64..1_000_000_000_000, y in -1_000_000_000_000i64..1_000_000_000_000, a in any::<i32>(), cx in -1_000_000i64..1_000_000) {
            let q = p(x, y);
            let about = p(cx, -cx);
            let back = rotate(rotate(q, Angle2(a), about), Angle2(a.wrapping_neg()), about);
            prop_assert!((back.x.0 - x).abs() <= 1 && (back.y.0 - y).abs() <= 1, "{q:?} → {back:?}");
        }

        #[test]
        fn rotate_commutes_with_translation(x in -1_000_000_000i64..1_000_000_000, y in -1_000_000_000i64..1_000_000_000, a in any::<i32>(), tx in -1_000_000i64..1_000_000, ty in -1_000_000i64..1_000_000) {
            let t = p(tx, ty);
            prop_assert_eq!(rotate(p(x, y) + t, Angle2(a), t), rotate(p(x, y), Angle2(a), Pt2::ORIGIN) + t);
        }

        #[test]
        fn rotate_preserves_length(x in -1_000_000_000i64..1_000_000_000, y in -1_000_000_000i64..1_000_000_000, a in any::<i32>()) {
            let r = rotate(p(x, y), Angle2(a), Pt2::ORIGIN);
            prop_assert!((length(r).0 - length(p(x, y)).0).abs() <= 1);
        }

        #[test]
        fn arc_end_points_lie_on_the_ellipse(rx in 1i64..1_000_000_000, ry in 1i64..1_000_000_000, start in -720_000i32..720_000, sweep in -720_000i32..720_000) {
            let c = p(123, -456);
            let els = arc_to_cubics(c, Nm(rx), Nm(ry), Angle2(start), Angle2(sweep));
            prop_assert_eq!(els.len() as i64, (i64::from(sweep).abs() + 89_999) / 90_000);
            if let Some(PathEl::CubicTo(_, _, end)) = els.last() {
                prop_assert_eq!(*end, arc_point(c, Nm(rx), Nm(ry), Angle2(start + sweep)));
            }
        }

        #[test]
        fn bulge_ends_at_p1(x0 in -1_000_000_000i64..1_000_000_000, y0 in -1_000_000_000i64..1_000_000_000, x1 in -1_000_000_000i64..1_000_000_000, y1 in -1_000_000_000i64..1_000_000_000, b in -20.0f64..20.0) {
            let els = bulge_to_cubics(p(x0, y0), p(x1, y1), b);
            prop_assert!(!els.is_empty() && els.len() <= 4);
            let last = match els.last() { Some(PathEl::CubicTo(_, _, e) | PathEl::LineTo(e)) => *e, _ => unreachable!() };
            prop_assert_eq!(last, p(x1, y1));
        }

        /// Every on-curve point of a bulge arc lies on the circle through `p0` and `p1` with the included
        /// angle `4·atan(b)`, within the rounding of one point.
        #[test]
        fn bulge_points_lie_on_the_circle(x1 in -100_000_000i64..100_000_000, y1 in -100_000_000i64..100_000_000, b in 0.05f64..10.0, neg in any::<bool>()) {
            prop_assume!(x1 != 0 || y1 != 0);
            let b = if neg { -b } else { b };
            let (dx, dy) = (x1 as f64, y1 as f64);
            let h = (1.0 - b * b) / (4.0 * b);
            let (cx, cy) = (0.5 * dx + h * dy, 0.5 * dy - h * dx);
            let r = (cx * cx + cy * cy).sqrt();
            for el in bulge_to_cubics(Pt2::ORIGIN, p(x1, y1), b) {
                let e = el.end().unwrap();
                let d = ((e.x.0 as f64 - cx).powi(2) + (e.y.0 as f64 - cy).powi(2)).sqrt();
                prop_assert!((d - r).abs() <= 1.0, "{e:?}: {d} vs {r}");
            }
        }

        #[test]
        fn milli_pt_round_trip(mpt in -1_000_000_000_000i64..1_000_000_000_000) {
            prop_assert_eq!(nm_to_milli_pt(milli_pt_to_nm(mpt).unwrap()), mpt);
        }

        #[test]
        fn milli_pt_is_nearest(nm in any::<i64>()) {
            let m = i128::from(nm_to_milli_pt(Nm(nm)));
            // |nm·9 − m·3175| ≤ 3175/2.
            prop_assert!((i128::from(nm) * 9 - m * 3175).abs() * 2 <= 3175);
        }
    }
}
