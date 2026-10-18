// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Path`: integer Bézier paths, a builder and the shape helpers.

use ew_units::{Angle2, Nm, Pt2, Rect};

use crate::Transform;
use crate::num::{self, div_round_tz};

/// One element of a [`Path`], in nm on the sheet (or in a symbol's own frame).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PathEl {
    /// Starts a new subpath at the point.
    MoveTo(Pt2),
    /// A straight segment to the point.
    LineTo(Pt2),
    /// A quadratic Bézier with one control point, to the second point.
    QuadTo(Pt2, Pt2),
    /// A cubic Bézier with two control points, to the third point.
    CubicTo(Pt2, Pt2, Pt2),
    /// Closes the current subpath with a straight segment to its start.
    ClosePath,
}

impl PathEl {
    /// The element's points: control points first, end point last; none for [`PathEl::ClosePath`].
    pub fn points(self) -> impl Iterator<Item = Pt2> {
        let (pts, n): ([Pt2; 3], usize) = match self {
            PathEl::MoveTo(a) | PathEl::LineTo(a) => ([a, a, a], 1),
            PathEl::QuadTo(a, b) => ([a, b, b], 2),
            PathEl::CubicTo(a, b, c) => ([a, b, c], 3),
            PathEl::ClosePath => ([Pt2::ORIGIN; 3], 0),
        };
        pts.into_iter().take(n)
    }

    /// The end point; `None` for [`PathEl::ClosePath`].
    pub fn end(self) -> Option<Pt2> {
        match self {
            PathEl::MoveTo(a) | PathEl::LineTo(a) | PathEl::QuadTo(_, a) | PathEl::CubicTo(_, _, a) => {
                Some(a)
            }
            PathEl::ClosePath => None,
        }
    }

    /// The element with every point mapped by `f`.
    pub fn map(self, mut f: impl FnMut(Pt2) -> Pt2) -> PathEl {
        match self {
            PathEl::MoveTo(p) => PathEl::MoveTo(f(p)),
            PathEl::LineTo(p) => PathEl::LineTo(f(p)),
            PathEl::QuadTo(a, b) => PathEl::QuadTo(f(a), f(b)),
            PathEl::CubicTo(a, b, c) => PathEl::CubicTo(f(a), f(b), f(c)),
            PathEl::ClosePath => PathEl::ClosePath,
        }
    }
}

/// The numerator of the circle constant κ = 0.552 284 749 8 (four cubics per full ellipse); the
/// denominator is [`KAPPA_DEN`]. A handle of radius `r` is `r × KAPPA_NUM / KAPPA_DEN`, rounded ties toward
/// zero in `i128`.
pub const KAPPA_NUM: i64 = 5_522_847_498;
/// The denominator of κ, 10¹⁰.
pub const KAPPA_DEN: i64 = 10_000_000_000;

/// The cubic handle length for radius `r`: `r · κ`, rounded ties toward zero.
///
/// ```
/// use ew_dl::kappa_handle;
/// use ew_units::Nm;
///
/// assert_eq!(kappa_handle(Nm(1_000_000)), Nm(552_285));
/// assert_eq!(kappa_handle(Nm(-1_000_000)), Nm(-552_285));
/// ```
pub fn kappa_handle(r: Nm) -> Nm {
    Nm(div_round_tz(i128::from(r.0) * i128::from(KAPPA_NUM), i128::from(KAPPA_DEN)) as i64)
}

/// A path of straight and Bézier segments in integer nanometres.
///
/// A well-formed path is empty or starts with [`PathEl::MoveTo`]; the [`PathBuilder`] guarantees this by
/// starting a drawing command without a current point at the origin, and [`to_kurbo`](Path::to_kurbo)
/// applies the same rule to a hand-built element list.
///
/// **Winding.** Every closed shape helper winds **counter-clockwise as seen on the sheet** (the positive
/// angle sense of the [`num`](crate::num) module), so shapes combined into one path fill predictably under
/// the non-zero rule.
///
/// ```
/// use ew_dl::{Path, PathEl};
/// use ew_units::{Nm, Pt2, Rect};
///
/// let r = Rect::from_points(Pt2::new(Nm(0), Nm(0)), Pt2::new(Nm(10), Nm(5)));
/// let path = Path::rect(r);
/// assert_eq!(path.els.len(), 5);
/// assert_eq!(path.els[4], PathEl::ClosePath);
/// assert_eq!(path.bounds(), Some(r));
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Path {
    /// The elements in drawing order.
    pub els: Vec<PathEl>,
}

impl Path {
    /// An empty path.
    pub const fn new() -> Path {
        Path { els: Vec::new() }
    }

    /// A builder for a new path.
    ///
    /// ```
    /// use ew_dl::Path;
    /// use ew_units::{Nm, Pt2};
    ///
    /// let p = |x, y| Pt2::new(Nm(x), Nm(y));
    /// let path = Path::builder()
    ///     .move_to(p(0, 0))
    ///     .line_to(p(10, 0))
    ///     .quad_to(p(15, 5), p(10, 10))
    ///     .close()
    ///     .build();
    /// assert_eq!(path.els.len(), 4);
    /// ```
    pub fn builder() -> PathBuilder {
        PathBuilder::default()
    }

    /// Whether the path has no elements.
    pub fn is_empty(&self) -> bool {
        self.els.is_empty()
    }

    /// Every point of the path, control points included, in element order.
    pub fn points(&self) -> impl Iterator<Item = Pt2> + '_ {
        self.els.iter().flat_map(|el| el.points())
    }

    /// The smallest rectangle containing every point of the path, **control points included** (so it
    /// contains the curve, exactly in nm, without stroke width); `None` for a path without points.
    pub fn bounds(&self) -> Option<Rect> {
        bounds_of(self.points())
    }

    /// The path with every point mapped by `t` (Bézier curves map exactly through their control points
    /// under an affine map; each point is rounded as [`Transform::apply`] states).
    pub fn transformed(&self, t: Transform) -> Path {
        Path {
            els: self.els.iter().map(|el| el.map(|p| t.apply(p))).collect(),
        }
    }

    /// The path as a `kurbo::BezPath` in f64 nm, for renderers and picking. Element counts are preserved,
    /// except that a path not starting with `MoveTo` gets a leading `MoveTo(origin)` (the builder's rule).
    pub fn to_kurbo(&self) -> kurbo::BezPath {
        let pt = |p: Pt2| kurbo::Point::new(p.x.0 as f64, p.y.0 as f64);
        let mut v = Vec::with_capacity(self.els.len() + 1);
        if !matches!(self.els.first(), None | Some(PathEl::MoveTo(_))) {
            v.push(kurbo::PathEl::MoveTo(kurbo::Point::ORIGIN));
        }
        v.extend(self.els.iter().map(|el| match *el {
            PathEl::MoveTo(a) => kurbo::PathEl::MoveTo(pt(a)),
            PathEl::LineTo(a) => kurbo::PathEl::LineTo(pt(a)),
            PathEl::QuadTo(a, b) => kurbo::PathEl::QuadTo(pt(a), pt(b)),
            PathEl::CubicTo(a, b, c) => kurbo::PathEl::CurveTo(pt(a), pt(b), pt(c)),
            PathEl::ClosePath => kurbo::PathEl::ClosePath,
        }));
        kurbo::BezPath::from_vec(v)
    }

    /// A straight line from `a` to `b`.
    pub fn line(a: Pt2, b: Pt2) -> Path {
        Path {
            els: vec![PathEl::MoveTo(a), PathEl::LineTo(b)],
        }
    }

    /// An open polyline through `pts`; empty for no points.
    pub fn polyline(pts: &[Pt2]) -> Path {
        let mut els = Vec::with_capacity(pts.len());
        let mut it = pts.iter();
        if let Some(&first) = it.next() {
            els.push(PathEl::MoveTo(first));
            els.extend(it.map(|&p| PathEl::LineTo(p)));
        }
        Path { els }
    }

    /// A closed polygon through `pts` (the closing edge is the `ClosePath`); empty for no points. The
    /// winding is the order of `pts`.
    pub fn polygon(pts: &[Pt2]) -> Path {
        let mut path = Path::polyline(pts);
        if !path.els.is_empty() {
            path.els.push(PathEl::ClosePath);
        }
        path
    }

    /// An axis-aligned rectangle: starts at the top-left corner and winds counter-clockwise on the sheet
    /// (down the left edge first). Five elements.
    pub fn rect(r: Rect) -> Path {
        let (a, b) = (r.min(), r.max());
        Path {
            els: vec![
                PathEl::MoveTo(a),
                PathEl::LineTo(Pt2::new(a.x, b.y)),
                PathEl::LineTo(b),
                PathEl::LineTo(Pt2::new(b.x, a.y)),
                PathEl::ClosePath,
            ],
        }
    }

    /// A rectangle with elliptic corners of radii `rx`, `ry`, each corner one cubic with κ handles.
    ///
    /// Negative radii count as 0; radii larger than half the side are clamped to it (QET and SVG
    /// semantics). If either radius is 0 the result is [`Path::rect`]. Otherwise the path starts at the left
    /// end of the top edge and winds counter-clockwise on the sheet; edges of zero length are omitted, and
    /// the top edge is drawn by the `ClosePath`.
    pub fn rounded_rect(r: Rect, rx: Nm, ry: Nm) -> Path {
        let rx = Nm(rx.0.clamp(0, r.width().0 / 2));
        let ry = Nm(ry.0.clamp(0, r.height().0 / 2));
        if rx.0 == 0 || ry.0 == 0 {
            return Path::rect(r);
        }
        let (hx, hy) = (kappa_handle(rx), kappa_handle(ry));
        let (x0, y0, x1, y1) = (r.min().x, r.min().y, r.max().x, r.max().y);
        let p = Pt2::new;
        let mut els = vec![PathEl::MoveTo(p(x0 + rx, y0))];
        let line = |els: &mut Vec<PathEl>, to: Pt2| {
            if !matches!(els.last(), Some(PathEl::CubicTo(_, _, end)) if *end == to) {
                els.push(PathEl::LineTo(to));
            }
        };
        // Top-left corner, left edge.
        els.push(PathEl::CubicTo(
            p(x0 + rx - hx, y0),
            p(x0, y0 + ry - hy),
            p(x0, y0 + ry),
        ));
        line(&mut els, p(x0, y1 - ry));
        // Bottom-left corner, bottom edge.
        els.push(PathEl::CubicTo(
            p(x0, y1 - ry + hy),
            p(x0 + rx - hx, y1),
            p(x0 + rx, y1),
        ));
        line(&mut els, p(x1 - rx, y1));
        // Bottom-right corner, right edge.
        els.push(PathEl::CubicTo(
            p(x1 - rx + hx, y1),
            p(x1, y1 - ry + hy),
            p(x1, y1 - ry),
        ));
        line(&mut els, p(x1, y0 + ry));
        // Top-right corner; the top edge back to the start is the ClosePath.
        els.push(PathEl::CubicTo(
            p(x1, y0 + ry - hy),
            p(x1 - rx + hx, y0),
            p(x1 - rx, y0),
        ));
        els.push(PathEl::ClosePath);
        Path { els }
    }

    /// An axis-aligned ellipse as four cubics with κ handles: starts at 3 o'clock `(cx + rx, cy)` and winds
    /// counter-clockwise on the sheet (through the top first), like a full-turn
    /// [`arc`](Path::arc) from 0°. Six elements.
    pub fn ellipse(center: Pt2, rx: Nm, ry: Nm) -> Path {
        let (hx, hy) = (kappa_handle(rx), kappa_handle(ry));
        let (cx, cy) = (center.x, center.y);
        let p = Pt2::new;
        Path {
            els: vec![
                PathEl::MoveTo(p(cx + rx, cy)),
                PathEl::CubicTo(p(cx + rx, cy - hy), p(cx + hx, cy - ry), p(cx, cy - ry)),
                PathEl::CubicTo(p(cx - hx, cy - ry), p(cx - rx, cy - hy), p(cx - rx, cy)),
                PathEl::CubicTo(p(cx - rx, cy + hy), p(cx - hx, cy + ry), p(cx, cy + ry)),
                PathEl::CubicTo(p(cx + hx, cy + ry), p(cx + rx, cy + hy), p(cx + rx, cy)),
                PathEl::ClosePath,
            ],
        }
    }

    /// A circle: [`ellipse`](Path::ellipse) with equal radii.
    pub fn circle(center: Pt2, r: Nm) -> Path {
        Path::ellipse(center, r, r)
    }

    /// An open elliptic arc in the QET convention: `MoveTo` the start point, then the cubics of
    /// [`num::arc_to_cubics`]. A zero sweep gives the lone `MoveTo`.
    pub fn arc(center: Pt2, rx: Nm, ry: Nm, start: Angle2, sweep: Angle2) -> Path {
        let mut els = vec![PathEl::MoveTo(num::arc_point(center, rx, ry, start))];
        els.extend(num::arc_to_cubics(center, rx, ry, start, sweep));
        Path { els }
    }
}

/// The smallest rectangle containing all `pts`; `None` if there are none.
pub(crate) fn bounds_of(pts: impl IntoIterator<Item = Pt2>) -> Option<Rect> {
    pts.into_iter().map(Rect::from_point).reduce(Rect::union)
}

/// Builds a [`Path`] element by element. Consuming methods, so calls chain.
///
/// A drawing command (`line_to`, `quad_to`, `cubic_to`) on an empty builder first inserts
/// `MoveTo(origin)`, so the result is always well formed; `close` on an empty builder does nothing.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct PathBuilder {
    els: Vec<PathEl>,
}

impl PathBuilder {
    fn draw(mut self, el: PathEl) -> PathBuilder {
        if self.els.is_empty() {
            self.els.push(PathEl::MoveTo(Pt2::ORIGIN));
        }
        self.els.push(el);
        self
    }

    /// Starts a new subpath at `p`.
    pub fn move_to(mut self, p: Pt2) -> PathBuilder {
        self.els.push(PathEl::MoveTo(p));
        self
    }

    /// A straight segment to `p`.
    pub fn line_to(self, p: Pt2) -> PathBuilder {
        self.draw(PathEl::LineTo(p))
    }

    /// A quadratic Bézier with control point `c` to `p`.
    pub fn quad_to(self, c: Pt2, p: Pt2) -> PathBuilder {
        self.draw(PathEl::QuadTo(c, p))
    }

    /// A cubic Bézier with control points `c1`, `c2` to `p`.
    pub fn cubic_to(self, c1: Pt2, c2: Pt2, p: Pt2) -> PathBuilder {
        self.draw(PathEl::CubicTo(c1, c2, p))
    }

    /// Closes the current subpath.
    pub fn close(mut self) -> PathBuilder {
        if !self.els.is_empty() {
            self.els.push(PathEl::ClosePath);
        }
        self
    }

    /// The finished path.
    pub fn build(self) -> Path {
        Path { els: self.els }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    fn count(path: &Path) -> (usize, usize, usize, usize, usize) {
        let mut c = (0, 0, 0, 0, 0);
        for el in &path.els {
            match el {
                PathEl::MoveTo(_) => c.0 += 1,
                PathEl::LineTo(_) => c.1 += 1,
                PathEl::QuadTo(..) => c.2 += 1,
                PathEl::CubicTo(..) => c.3 += 1,
                PathEl::ClosePath => c.4 += 1,
            }
        }
        c
    }

    #[test]
    fn kappa_constant() {
        assert_eq!(KAPPA_NUM as f64 / KAPPA_DEN as f64, 0.552_284_749_8);
        // κ = 4/3 · (√2 − 1) to ten places.
        let exact = 4.0 / 3.0 * (2f64.sqrt() - 1.0);
        assert!((exact - 0.552_284_749_8).abs() < 1e-10);
        assert_eq!(kappa_handle(Nm(10_000_000_000)), Nm(5_522_847_498));
        assert_eq!(kappa_handle(Nm(1)), Nm(1)); // 0.55 → 1
        assert_eq!(kappa_handle(Nm(0)), Nm(0));
        assert_eq!(kappa_handle(Nm(5)), Nm(3)); // 2.761…
        // Ties of the shared rounding helper go toward zero.
        assert_eq!(div_round_tz(25, 10), 2);
        assert_eq!(div_round_tz(-25, 10), -2);
    }

    #[test]
    fn line_polyline_polygon() {
        let l = Path::line(p(1, 2), p(-3, 4));
        assert_eq!(count(&l), (1, 1, 0, 0, 0));
        assert_eq!(l.bounds(), Some(Rect::from_points(p(-3, 2), p(1, 4))));
        let pts = [p(0, 0), p(10, 0), p(10, 10), p(0, 7)];
        let pl = Path::polyline(&pts);
        assert_eq!(count(&pl), (1, 3, 0, 0, 0));
        assert_ne!(pl.els.last(), Some(&PathEl::ClosePath));
        let pg = Path::polygon(&pts);
        assert_eq!(count(&pg), (1, 3, 0, 0, 1));
        assert_eq!(pg.els.last(), Some(&PathEl::ClosePath));
        assert_eq!(pg.bounds(), Some(Rect::from_points(p(0, 0), p(10, 10))));
        assert!(Path::polyline(&[]).is_empty());
        assert!(Path::polygon(&[]).is_empty());
        assert_eq!(Path::new().bounds(), None);
    }

    #[test]
    fn rect_winds_counter_clockwise() {
        let r = Rect::from_points(p(0, 0), p(10, 5));
        let path = Path::rect(r);
        assert_eq!(
            path.els,
            [
                PathEl::MoveTo(p(0, 0)),
                PathEl::LineTo(p(0, 5)),
                PathEl::LineTo(p(10, 5)),
                PathEl::LineTo(p(10, 0)),
                PathEl::ClosePath,
            ]
        );
        assert!(signed_area(&path) < 0);
    }

    /// Σ (xᵢ·yᵢ₊₁ − xᵢ₊₁·yᵢ) over the on-curve points. The formula is positive for counter-clockwise in a
    /// y-up frame; with y down, counter-clockwise **on the sheet** is negative.
    fn signed_area(path: &Path) -> i128 {
        let pts: Vec<Pt2> = path.els.iter().filter_map(|el| el.end()).collect();
        (0..pts.len())
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                i128::from(a.x.0) * i128::from(b.y.0) - i128::from(b.x.0) * i128::from(a.y.0)
            })
            .sum()
    }

    #[test]
    fn rounded_rect_shapes() {
        let r = Rect::from_points(p(0, 0), p(20_000_000, 10_000_000));
        let rr = Path::rounded_rect(r, Nm(2_000_000), Nm(1_000_000));
        assert_eq!(count(&rr), (1, 3, 0, 4, 1));
        assert_eq!(rr.bounds(), Some(r));
        assert!(signed_area(&rr) < 0);
        assert_eq!(rr.els[0], PathEl::MoveTo(p(2_000_000, 0)));
        assert_eq!(
            rr.els[1],
            PathEl::CubicTo(
                p(2_000_000 - 1_104_569, 0),
                p(0, 1_000_000 - 552_285),
                p(0, 1_000_000)
            )
        );
        // Clamped to half the sides: the straight vertical edges vanish, the horizontal ones stay.
        let pill = Path::rounded_rect(r, Nm(4_000_000), Nm(99_000_000));
        assert_eq!(count(&pill), (1, 1, 0, 4, 1));
        assert_eq!(pill.bounds(), Some(r));
        // A full ellipse when both radii reach half the sides.
        let oval = Path::rounded_rect(r, Nm(10_000_000), Nm(5_000_000));
        assert_eq!(count(&oval), (1, 0, 0, 4, 1));
        // Zero or negative radius: a plain rectangle.
        assert_eq!(Path::rounded_rect(r, Nm(0), Nm(5)), Path::rect(r));
        assert_eq!(Path::rounded_rect(r, Nm(-5), Nm(5)), Path::rect(r));
    }

    #[test]
    fn circle_and_ellipse() {
        let c = Path::circle(p(5_000_000, 5_000_000), Nm(1_000_000));
        assert_eq!(count(&c), (1, 0, 0, 4, 1));
        assert_eq!(
            c.bounds(),
            Some(Rect::from_points(
                p(4_000_000, 4_000_000),
                p(6_000_000, 6_000_000)
            ))
        );
        assert_eq!(
            c.els[1],
            PathEl::CubicTo(
                p(6_000_000, 4_447_715),
                p(5_552_285, 4_000_000),
                p(5_000_000, 4_000_000)
            )
        );
        assert!(signed_area(&c) < 0);
        let e = Path::ellipse(p(0, 0), Nm(3), Nm(2));
        assert_eq!(e.bounds(), Some(Rect::from_points(p(-3, -2), p(3, 2))));
        assert_eq!(e.els.first(), Some(&PathEl::MoveTo(p(3, 0))));
    }

    /// The κ ellipse and a full-turn libm arc agree to within 1 nm (radii below about 16 m).
    #[test]
    fn ellipse_agrees_with_full_arc() {
        for (rx, ry) in [
            (1_000_000, 1_000_000),
            (3_000_000, 1_250_000),
            (420_000_000, 297_000_000),
        ] {
            let e = Path::ellipse(p(17, -3), Nm(rx), Nm(ry));
            let a = Path::arc(p(17, -3), Nm(rx), Nm(ry), Angle2(0), Angle2(360_000));
            assert_eq!(e.els.len(), a.els.len() + 1);
            for (x, y) in e.points().zip(a.points()) {
                assert!(
                    (x.x.0 - y.x.0).abs() <= 1 && (x.y.0 - y.y.0).abs() <= 1,
                    "{x:?} {y:?}"
                );
            }
        }
    }

    #[test]
    fn builder() {
        let path = Path::builder()
            .move_to(p(0, 0))
            .line_to(p(10, 0))
            .quad_to(p(15, 5), p(10, 10))
            .cubic_to(p(5, 15), p(0, 15), p(0, 10))
            .close()
            .build();
        assert_eq!(count(&path), (1, 1, 1, 1, 1));
        assert_eq!(path.bounds(), Some(Rect::from_points(p(0, 0), p(15, 15))));
        // Drawing without a current point starts at the origin; closing nothing does nothing.
        let implicit = Path::builder().line_to(p(5, 5)).build();
        assert_eq!(
            implicit.els,
            [PathEl::MoveTo(Pt2::ORIGIN), PathEl::LineTo(p(5, 5))]
        );
        assert!(Path::builder().close().build().is_empty());
    }

    #[test]
    fn to_kurbo_conversion() {
        let path = Path::builder()
            .move_to(p(1, 2))
            .line_to(p(3, 4))
            .quad_to(p(5, 6), p(7, 8))
            .cubic_to(p(9, 10), p(11, 12), p(13, 14))
            .close()
            .build();
        let k = path.to_kurbo();
        assert_eq!(k.elements().len(), 5);
        assert_eq!(
            k.elements()[3],
            kurbo::PathEl::CurveTo((9.0, 10.0).into(), (11.0, 12.0).into(), (13.0, 14.0).into())
        );
        let bad = Path {
            els: vec![PathEl::LineTo(p(1, 1))],
        };
        assert_eq!(bad.to_kurbo().elements().len(), 2);
        assert!(Path::new().to_kurbo().elements().is_empty());
    }

    #[test]
    fn transformed_paths() {
        let path = Path::rect(Rect::from_points(p(0, 0), p(10, 5)));
        let t = Transform::new(p(100, 100), ew_units::Orient2::R90, Angle2::ZERO);
        let moved = path.transformed(t);
        assert_eq!(moved.bounds(), Some(Rect::from_points(p(95, 100), p(100, 110))));
    }

    #[test]
    fn el_points_and_map() {
        assert_eq!(PathEl::MoveTo(p(1, 2)).points().collect::<Vec<_>>(), [p(1, 2)]);
        assert_eq!(
            PathEl::QuadTo(p(1, 2), p(3, 4)).points().collect::<Vec<_>>(),
            [p(1, 2), p(3, 4)]
        );
        assert_eq!(PathEl::ClosePath.points().count(), 0);
        assert_eq!(PathEl::CubicTo(p(1, 1), p(2, 2), p(3, 3)).end(), Some(p(3, 3)));
        assert_eq!(PathEl::ClosePath.end(), None);
        let c = PathEl::CubicTo(p(1, 1), p(2, 2), p(3, 3)).map(|q| q + p(1, 0));
        assert_eq!(c, PathEl::CubicTo(p(2, 1), p(3, 2), p(4, 3)));
    }

    fn pt() -> impl Strategy<Value = Pt2> {
        (-1_000_000_000i64..1_000_000_000, -1_000_000_000i64..1_000_000_000).prop_map(|(x, y)| p(x, y))
    }

    fn el() -> impl Strategy<Value = PathEl> {
        prop_oneof![
            pt().prop_map(PathEl::MoveTo),
            pt().prop_map(PathEl::LineTo),
            (pt(), pt()).prop_map(|(a, b)| PathEl::QuadTo(a, b)),
            (pt(), pt(), pt()).prop_map(|(a, b, c)| PathEl::CubicTo(a, b, c)),
            Just(PathEl::ClosePath),
        ]
    }

    fn path() -> impl Strategy<Value = Path> {
        (pt(), proptest::collection::vec(el(), 0..24)).prop_map(|(start, mut els)| {
            els.insert(0, PathEl::MoveTo(start));
            Path { els }
        })
    }

    proptest! {
        #[test]
        fn bounds_contain_every_point(path in path()) {
            let b = path.bounds().unwrap();
            for q in path.points() {
                prop_assert!(b.contains(q));
            }
            // And are tight: each edge is touched by some point.
            prop_assert!(path.points().any(|q| q.x == b.min().x));
            prop_assert!(path.points().any(|q| q.y == b.max().y));
        }

        #[test]
        fn to_kurbo_keeps_element_counts(path in path()) {
            let k = path.to_kurbo();
            prop_assert_eq!(k.elements().len(), path.els.len());
            for (a, b) in path.els.iter().zip(k.elements()) {
                let same = matches!(
                    (a, b),
                    (PathEl::MoveTo(_), kurbo::PathEl::MoveTo(_))
                        | (PathEl::LineTo(_), kurbo::PathEl::LineTo(_))
                        | (PathEl::QuadTo(..), kurbo::PathEl::QuadTo(..))
                        | (PathEl::CubicTo(..), kurbo::PathEl::CurveTo(..))
                        | (PathEl::ClosePath, kurbo::PathEl::ClosePath)
                );
                prop_assert!(same);
            }
        }

        #[test]
        fn shape_bounds(c in pt(), rx in 0i64..1_000_000_000, ry in 0i64..1_000_000_000) {
            let e = Path::ellipse(c, Nm(rx), Nm(ry));
            prop_assert_eq!(e.bounds(), Some(Rect::from_points(c - p(rx, ry), c + p(rx, ry))));
            let r = Rect::from_points(c, c + p(rx, ry));
            let rr = Path::rounded_rect(r, Nm(ry / 3), Nm(rx / 3));
            prop_assert_eq!(rr.bounds(), Some(r));
            prop_assert_eq!(rr.els.last(), Some(&PathEl::ClosePath));
        }
    }
}
