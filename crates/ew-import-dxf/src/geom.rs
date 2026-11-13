// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Entities into shapes in drawing units (doc 14 §6.1's coverage table): polylines, arcs, filled polygons, texts and
//! dots in the world frame of the drawing, y up, each with its layer, dash, width and source key.
//!
//! Blocks are expanded through their inserts (nested, scaled, mirrored, MINSERT arrays) under an affine map; an arc
//! stays an arc under a similarity and is flattened otherwise. Bulges, ellipses and splines are flattened to within
//! [`TOLERANCE_NM`] after scaling; every sine, cosine and arc tangent comes from `ew-dl`'s pure-Rust numerics, so the
//! result is the same on every platform. Limits (doc 14 §11): block nesting at most [`MAX_NESTING`], at most
//! [`MAX_PRIMITIVES`] shapes after expansion, at most [`MAX_SPLINE_POINTS`] control points per spline.

use std::collections::BTreeMap;

use ew_dl::num::{atan2_deg, sin_cos_deg};
use ew_interop_api::ImportError;

use crate::parse::{Drawing, Entity, Layer};
use crate::text::{mtext_lines, percent_codes, shx_substitute};

/// The flattening tolerance of bulges, ellipses, splines and arcs under a non-uniform scale: 0.01 mm.
pub const TOLERANCE_NM: f64 = 10_000.0;

/// The deepest block nesting expanded (doc 14 §11).
pub const MAX_NESTING: usize = 32;

/// The most shapes one import expands to (doc 14 §11: INSERT and MINSERT expansion), so that a refused expansion stays
/// within the 256 MB a hostile input may cost.
pub const MAX_PRIMITIVES: usize = 1_000_000;

/// The most control points of one spline (doc 14 §11).
pub const MAX_SPLINE_POINTS: usize = 100_000;

/// The most points one flattened curve gets.
const MAX_CURVE_POINTS: usize = 4096;

/// A point in drawing units.
pub(crate) type P = (f64, f64);

/// An affine map `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Affine {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Affine {
    pub const IDENTITY: Affine = Affine {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    pub fn translate(x: f64, y: f64) -> Affine {
        Affine {
            e: x,
            f: y,
            ..Affine::IDENTITY
        }
    }

    pub fn rotate(deg: f64) -> Affine {
        let (s, c) = sin_cos_deg(deg);
        Affine {
            a: c,
            b: s,
            c: -s,
            d: c,
            e: 0.0,
            f: 0.0,
        }
    }

    pub fn scale(sx: f64, sy: f64) -> Affine {
        Affine {
            a: sx,
            d: sy,
            ..Affine::IDENTITY
        }
    }

    /// The map that applies `inner` first, then `self`.
    pub fn after(self, inner: Affine) -> Affine {
        Affine {
            a: self.a * inner.a + self.c * inner.b,
            b: self.b * inner.a + self.d * inner.b,
            c: self.a * inner.c + self.c * inner.d,
            d: self.b * inner.c + self.d * inner.d,
            e: self.a * inner.e + self.c * inner.f + self.e,
            f: self.b * inner.e + self.d * inner.f + self.f,
        }
    }

    pub fn apply(self, (x, y): P) -> P {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }

    /// The linear part applied to a vector.
    pub fn vector(self, (x, y): P) -> P {
        (self.a * x + self.c * y, self.b * x + self.d * y)
    }

    pub fn det(self) -> f64 {
        self.a * self.d - self.b * self.c
    }

    /// The largest stretch of a unit vector.
    pub fn max_scale(self) -> f64 {
        let x = (self.a * self.a + self.b * self.b).sqrt();
        let y = (self.c * self.c + self.d * self.d).sqrt();
        x.max(y)
    }

    /// The scale of a similarity (rotation, uniform scale, mirror), `None` for any other map.
    pub fn similarity(self) -> Option<f64> {
        let x = (self.a * self.a + self.b * self.b).sqrt();
        let y = (self.c * self.c + self.d * self.d).sqrt();
        let dot = self.a * self.c + self.b * self.d;
        let tol = 1e-9 * x.max(y).max(1e-300);
        ((x - y).abs() <= tol && dot.abs() <= tol * x.max(y)).then_some(x)
    }

    /// The object coordinate system of an extrusion direction projected to the x-y plane (the DXF arbitrary axis
    /// algorithm).
    pub fn ocs((nx, ny, nz): (f64, f64, f64)) -> Affine {
        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        if !len.is_finite() || len == 0.0 {
            return Affine::IDENTITY;
        }
        let n = (nx / len, ny / len, nz / len);
        if (n.0 - 0.0).abs() < 1e-12 && (n.1 - 0.0).abs() < 1e-12 && n.2 > 0.0 {
            return Affine::IDENTITY;
        }
        let cross = |u: (f64, f64, f64), v: (f64, f64, f64)| {
            (u.1 * v.2 - u.2 * v.1, u.2 * v.0 - u.0 * v.2, u.0 * v.1 - u.1 * v.0)
        };
        let ax = if n.0.abs() < 1.0 / 64.0 && n.1.abs() < 1.0 / 64.0 {
            cross((0.0, 1.0, 0.0), n)
        } else {
            cross((0.0, 0.0, 1.0), n)
        };
        let l = (ax.0 * ax.0 + ax.1 * ax.1 + ax.2 * ax.2).sqrt();
        let ax = (ax.0 / l, ax.1 / l, ax.2 / l);
        let ay = cross(n, ax);
        Affine {
            a: ax.0,
            b: ax.1,
            c: ay.0,
            d: ay.1,
            e: 0.0,
            f: 0.0,
        }
    }
}

/// How a shape's line is dashed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pattern {
    Solid,
    Dashed,
    Dotted,
    DashDot,
}

/// The horizontal alignment of a text at its anchor (the anchor is on the text's vertical middle).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HAlign {
    Left,
    Centre,
    Right,
}

/// What a shape draws, in drawing units, y up.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Geo {
    /// Straight segments through `pts`; closed back to the first point when `closed`.
    Poly { pts: Vec<P>, closed: bool },
    /// A circular arc, counter-clockwise from `start` over `sweep` degrees (360: a circle).
    Arc { c: P, r: f64, start: f64, sweep: f64 },
    /// A filled polygon.
    Fill { pts: Vec<P> },
    /// One line of text: the anchor on its vertical middle, the cap height, the angle in degrees.
    Text {
        at: P,
        height: f64,
        angle: f64,
        text: String,
        align: HAlign,
    },
    /// A point.
    Dot(P),
}

/// One shape and what it carries from its entity.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Shape {
    pub geo: Geo,
    /// The effective layer name.
    pub layer: String,
    pub dash: Pattern,
    /// A stroke width in drawing units (0: the page's default).
    pub width: f64,
    /// The source key within the layout (`<handle>`, `<insert handle>/<handle>`, `#<n>` without handles).
    pub key: String,
}

/// What a conversion noticed about the file, for the report.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Notes {
    /// Entity types not imported (doc 14 §6.1 "Not imported"), with their counts.
    pub unsupported: BTreeMap<String, u32>,
    /// Hatch patterns reduced to their boundary, with their counts.
    pub patterns: BTreeMap<String, u32>,
    /// Image files and external references never opened (their file names).
    pub external: Vec<String>,
    /// SHX fonts substituted: font file → family.
    pub shx: BTreeMap<String, &'static str>,
    /// Items on hidden layers not imported, per layer.
    pub hidden: BTreeMap<String, u32>,
    /// Items with a coordinate that is not a number, by key.
    pub malformed: Vec<String>,
    /// Items skipped for a limit (nesting, spline size), with what happened.
    pub skipped: Vec<(String, String)>,
}

/// The inherited state of a block's entities: the insert's layer and linetype for `0` and BYBLOCK.
#[derive(Clone, Debug)]
struct Inherit {
    layer: String,
    ltype: Option<String>,
    width: f64,
}

/// The shapes of one space.
pub(crate) struct Converter<'d> {
    d: &'d Drawing,
    /// The flattening tolerance in drawing units at scale 1.
    tol: f64,
    /// The layers imported even when hidden.
    show: Option<&'d std::collections::BTreeSet<String>>,
    pub shapes: Vec<Shape>,
    pub notes: Notes,
    mirrtext: bool,
}

fn limit(what: &str, limit: usize) -> ImportError {
    ImportError::LimitExceeded {
        what: what.to_owned(),
        limit: limit as u64,
    }
}

/// The angle of a vector in degrees.
fn angle_of((x, y): P) -> f64 {
    atan2_deg(y, x)
}

fn finite(p: P) -> bool {
    p.0.is_finite() && p.1.is_finite()
}

/// The points of an arc `c`, `r` from `start` over `sweep` degrees within `tol` (at least one point per 90°), the
/// start and end included.
pub(crate) fn arc_points(c: P, r: f64, start: f64, sweep: f64, tol: f64) -> Vec<P> {
    let r_abs = r.abs();
    let step = if tol >= r_abs || r_abs == 0.0 {
        90.0
    } else {
        // The sagitta of a step of angle a is r·(1 − cos(a/2)); acos via the half-angle identity.
        let half = (1.0 - tol / r_abs).clamp(-1.0, 1.0);
        2.0 * atan2_deg((1.0 - half * half).max(0.0).sqrt(), half)
    }
    .clamp(0.05, 90.0);
    let n = ((sweep.abs() / step).ceil() as usize).clamp(1, MAX_CURVE_POINTS);
    (0..=n)
        .map(|i| {
            let a = start + sweep * (i as f64) / (n as f64);
            let (s, co) = sin_cos_deg(a);
            (c.0 + r * co, c.1 + r * s)
        })
        .collect()
}

/// The points of the bulge segment from `a` to `b` (bulge = tan of a quarter of the included angle, positive
/// counter-clockwise), `a` excluded, `b` included.
fn bulge_points(a: P, b: P, bulge: f64, tol: f64) -> Vec<P> {
    if bulge == 0.0 || !bulge.is_finite() || a == b {
        return vec![b];
    }
    let theta = 4.0 * atan2_deg(bulge, 1.0);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let chord = (dx * dx + dy * dy).sqrt();
    let (s_half, _) = sin_cos_deg(theta / 2.0);
    if s_half == 0.0 {
        return vec![b];
    }
    let r = chord / (2.0 * s_half);
    // The centre lies on the chord's perpendicular bisector, at distance r·cos(θ/2) towards the bulge's left side.
    let (_, c_half) = sin_cos_deg(theta / 2.0);
    let h = r * c_half;
    let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let c = (m.0 - dy / chord * h, m.1 + dx / chord * h);
    let start = angle_of((a.0 - c.0, a.1 - c.1));
    let mut pts = arc_points(c, r.abs(), start, theta, tol);
    pts.remove(0);
    if let Some(last) = pts.last_mut() {
        *last = b;
    }
    pts
}

/// A NURBS curve of `degree` through `ctrl` (with `weights`, empty for 1) and `knots`, flattened within `tol`;
/// `None` when the knots do not fit the control points.
fn spline_points(degree: usize, ctrl: &[P], weights: &[f64], knots: &[f64], tol: f64) -> Option<Vec<P>> {
    let n = ctrl.len();
    if degree == 0 || n <= degree || knots.len() != n + degree + 1 {
        return None;
    }
    if knots.windows(2).any(|w| w[1] < w[0] || !w[1].is_finite()) {
        return None;
    }
    let w = |i: usize| weights.get(i).copied().filter(|w| *w > 0.0 && w.is_finite()).unwrap_or(1.0);
    let eval = |u: f64| -> P {
        // The span k with knots[k] <= u < knots[k + 1] within [degree, n − 1].
        let mut k = degree;
        while k < n - 1 && u >= knots[k + 1] {
            k += 1;
        }
        let mut d: Vec<(f64, f64, f64)> = (0..=degree)
            .map(|j| {
                let i = k - degree + j;
                let wi = w(i);
                (ctrl[i].0 * wi, ctrl[i].1 * wi, wi)
            })
            .collect();
        for r in 1..=degree {
            for j in (r..=degree).rev() {
                let i = k - degree + j;
                let den = knots[i + degree + 1 - r] - knots[i];
                let alpha = if den == 0.0 { 0.0 } else { (u - knots[i]) / den };
                d[j] = (
                    (1.0 - alpha) * d[j - 1].0 + alpha * d[j].0,
                    (1.0 - alpha) * d[j - 1].1 + alpha * d[j].1,
                    (1.0 - alpha) * d[j - 1].2 + alpha * d[j].2,
                );
            }
        }
        let (x, y, wt) = d[degree];
        if wt == 0.0 { (x, y) } else { (x / wt, y / wt) }
    };
    let (u0, u1) = (knots[degree], knots[n]);
    if u1 <= u0 {
        return None;
    }
    let mut pts = vec![eval(u0)];
    // Each knot span is subdivided until the midpoint lies within the tolerance of its chord.
    let spans: Vec<(f64, f64)> = (degree..n)
        .map(|k| (knots[k], knots[k + 1]))
        .filter(|(a, b)| b > a)
        .collect();
    #[allow(clippy::too_many_arguments)]
    fn refine(eval: &dyn Fn(f64) -> P, a: f64, pa: P, b: f64, pb: P, tol: f64, depth: u32, out: &mut Vec<P>) {
        let m = (a + b) / 2.0;
        let pm = eval(m);
        let (dx, dy) = (pb.0 - pa.0, pb.1 - pa.1);
        let len = (dx * dx + dy * dy).sqrt();
        let dist = if len == 0.0 {
            ((pm.0 - pa.0).powi(2) + (pm.1 - pa.1).powi(2)).sqrt()
        } else {
            ((pm.0 - pa.0) * dy - (pm.1 - pa.1) * dx).abs() / len
        };
        if depth >= 12 || out.len() >= MAX_CURVE_POINTS * 4 || (dist <= tol && depth >= 2) {
            out.push(pb);
            return;
        }
        refine(eval, a, pa, m, pm, tol, depth + 1, out);
        refine(eval, m, pm, b, pb, tol, depth + 1, out);
    }
    for (a, b) in spans {
        let pa = *pts.last()?;
        let pb = eval(b);
        refine(&eval, a, pa, b, pb, tol, 0, &mut pts);
    }
    // The curve ends at its last control point for a clamped spline.
    Some(pts)
}

impl<'d> Converter<'d> {
    /// A converter of `d` whose flattening tolerance is `tol` drawing units; `show` names layers imported though
    /// hidden.
    pub fn new(d: &'d Drawing, tol: f64, show: Option<&'d std::collections::BTreeSet<String>>) -> Converter<'d> {
        Converter {
            d,
            tol,
            show,
            shapes: Vec::new(),
            notes: Notes::default(),
            mirrtext: d.header_int("MIRRTEXT").unwrap_or(0) != 0,
        }
    }

    fn layer(&self, name: &str) -> Option<&'d Layer> {
        self.d.layers.get(&name.to_ascii_uppercase())
    }

    /// The dash of an entity's linetype name, resolved through its layer and the insert.
    fn dash(&self, e: &Entity, layer: &str, inh: Option<&Inherit>) -> Pattern {
        let mut name = e.str(6).unwrap_or_else(|| "BYLAYER".to_owned()).trim().to_ascii_uppercase();
        if name == "BYBLOCK" {
            name = inh.and_then(|i| i.ltype.clone()).unwrap_or_else(|| "BYLAYER".to_owned());
        }
        if name == "BYLAYER" {
            name = self
                .layer(layer)
                .map_or_else(|| "CONTINUOUS".to_owned(), |l| l.linetype.to_ascii_uppercase());
        }
        if name == "CONTINUOUS" || name.is_empty() {
            return Pattern::Solid;
        }
        if let Some(elements) = self.d.ltypes.get(&name).filter(|e| !e.is_empty()) {
            let dots = elements.iter().any(|x| x.abs() < 1e-9);
            let dashes = elements.iter().filter(|x| **x > 1e-9).count();
            return match (dashes, dots) {
                (0, true) => Pattern::Dotted,
                (_, true) => Pattern::DashDot,
                (0 | 1, false) if elements.len() < 2 => Pattern::Solid,
                _ => Pattern::Dashed,
            };
        }
        if name.contains("DASHDOT") || name.contains("CENTER") || name.contains("PHANTOM") {
            Pattern::DashDot
        } else if name.contains("DOT") {
            Pattern::Dotted
        } else if name.contains("DASH") || name.contains("HIDDEN") {
            Pattern::Dashed
        } else {
            Pattern::Solid
        }
    }

    fn push(&mut self, geo: Geo, layer: &str, dash: Pattern, width: f64, key: &str) -> Result<(), ImportError> {
        if self.shapes.len() >= MAX_PRIMITIVES {
            return Err(limit("DXF block expansion (shapes)", MAX_PRIMITIVES));
        }
        self.shapes.push(Shape {
            geo,
            layer: layer.to_owned(),
            dash,
            width,
            key: key.to_owned(),
        });
        Ok(())
    }

    /// The shapes of the block `name` in its own frame, its base point at the origin (its non-constant attribute
    /// definitions are not drawn).
    pub fn block_shapes(&mut self, name: &str) -> Result<(), ImportError> {
        let base = self
            .d
            .blocks
            .get(&name.trim().to_ascii_uppercase())
            .map_or((0.0, 0.0), |b| b.base);
        self.block(name, Affine::translate(-base.0, -base.1), "", 0, "0", None, 0.0)
    }

    /// The shapes of `entities` (a space or a block's content) under `t`.
    pub fn entities(
        &mut self,
        entities: &[Entity],
        t: Affine,
        prefix: &str,
        depth: usize,
    ) -> Result<(), ImportError> {
        self.walk(entities, t, prefix, depth, None)
    }

    fn walk(
        &mut self,
        entities: &[Entity],
        t: Affine,
        prefix: &str,
        depth: usize,
        inh: Option<&Inherit>,
    ) -> Result<(), ImportError> {
        for (n, e) in entities.iter().enumerate() {
            let own = e.handle().unwrap_or_else(|| format!("#{n}"));
            let key = if prefix.is_empty() { own } else { format!("{prefix}/{own}") };
            self.entity(e, t, &key, depth, inh)?;
        }
        Ok(())
    }

    fn entity(
        &mut self,
        e: &Entity,
        t: Affine,
        key: &str,
        depth: usize,
        inh: Option<&Inherit>,
    ) -> Result<(), ImportError> {
        let mut layer = e.layer();
        if layer == "0" {
            if let Some(i) = inh {
                layer.clone_from(&i.layer);
            }
        }
        if let Some(l) = self.layer(&layer) {
            let shown = self.show.is_some_and(|s| s.contains(&l.name));
            if l.hidden && !shown {
                *self.notes.hidden.entry(l.name.clone()).or_default() += 1;
                return Ok(());
            }
        }
        let dash = self.dash(e, &layer, inh);
        // A lineweight (code 370, 1/100 mm) is converted by the caller's unit; it is kept in millimetres here as a
        // negative width so that it is not scaled with the drawing.
        let lw = e.i(370, -1);
        let width = if lw > 0 { -(lw as f64) / 100.0 } else { inh.map_or(0.0, |i| i.width) };
        let ocs = t.after(Affine::ocs(e.extrusion()));
        match e.kind.as_str() {
            "LINE" => {
                let (a, b) = (e.pt(10), e.pt(11));
                self.poly(vec![t.apply(a), t.apply(b)], false, &layer, dash, width, key)?;
            }
            "LWPOLYLINE" => {
                let closed = e.i(70, 0) & 1 != 0;
                let cw = e.f(43, 0.0);
                let mut verts: Vec<(P, f64)> = Vec::new();
                for p in &e.pairs {
                    match p.code {
                        10 => verts.push(((p.num().unwrap_or(0.0), 0.0), 0.0)),
                        20 => {
                            if let Some(v) = verts.last_mut() {
                                v.0.1 = p.num().unwrap_or(0.0);
                            }
                        }
                        42 => {
                            if let Some(v) = verts.last_mut() {
                                v.1 = p.num().unwrap_or(0.0);
                            }
                        }
                        _ => {}
                    }
                }
                let width = if cw > 0.0 { cw * ocs.max_scale() } else { width };
                let pts = self.bulged(&verts, closed, ocs);
                self.poly(pts, closed, &layer, dash, width, key)?;
            }
            "POLYLINE" => {
                let flags = e.i(70, 0);
                if flags & (16 | 64) != 0 {
                    *self.notes.unsupported.entry("POLYLINE (mesh)".to_owned()).or_default() += 1;
                    return Ok(());
                }
                let closed = flags & 1 != 0;
                let map = if flags & 8 != 0 { t } else { ocs };
                let verts: Vec<(P, f64)> = e
                    .children
                    .iter()
                    // Spline frame control points are not on the curve.
                    .filter(|v| v.i(70, 0) & 16 == 0)
                    .map(|v| (v.pt(10), v.f(42, 0.0)))
                    .collect();
                let cw = e.f(40, 0.0);
                let width = if cw > 0.0 { cw * map.max_scale() } else { width };
                let pts = self.bulged(&verts, closed, map);
                self.poly(pts, closed, &layer, dash, width, key)?;
            }
            "ARC" | "CIRCLE" => {
                let c = e.pt(10);
                let r = e.f(40, 0.0);
                let (start, sweep) = if e.kind == "ARC" {
                    let (s, en) = (e.f(50, 0.0), e.f(51, 360.0));
                    let mut sweep = (en - s).rem_euclid(360.0);
                    if sweep == 0.0 {
                        sweep = 360.0;
                    }
                    (s, sweep)
                } else {
                    (0.0, 360.0)
                };
                self.arc(c, r, start, sweep, ocs, &layer, dash, width, key)?;
            }
            "ELLIPSE" => {
                let c = e.pt(10);
                let m = e.pt(11);
                let ratio = e.f(40, 1.0);
                let (t0, t1) = (e.f(41, 0.0), e.f(42, std::f64::consts::TAU));
                let n = e.extrusion();
                // The minor axis is the extrusion crossed with the major axis.
                let minor = if n.2 < 0.0 { (m.1 * ratio, -m.0 * ratio) } else { (-m.1 * ratio, m.0 * ratio) };
                let deg = |rad: f64| rad * (180.0 / std::f64::consts::PI);
                let (a0, mut sweep) = (deg(t0), deg(t1 - t0).rem_euclid(360.0));
                if sweep == 0.0 {
                    sweep = 360.0;
                }
                let rmax = (m.0 * m.0 + m.1 * m.1).sqrt() * t.max_scale();
                let tol = self.tol;
                let unit = arc_points((0.0, 0.0), 1.0, a0, sweep, if rmax > 0.0 { tol / rmax } else { 1.0 });
                let pts: Vec<P> = unit
                    .into_iter()
                    .map(|(co, s)| t.apply((c.0 + m.0 * co + minor.0 * s, c.1 + m.1 * co + minor.1 * s)))
                    .collect();
                let closed = (sweep - 360.0).abs() < 1e-9;
                self.poly(pts, closed, &layer, dash, width, key)?;
            }
            "SPLINE" => self.spline(e, t, &layer, dash, width, key)?,
            "SOLID" | "TRACE" => {
                // The third and fourth corners are written in the order 1, 2, 4, 3 around the outline.
                let (p1, p2, p3) = (e.pt(10), e.pt(11), e.pt(12));
                let p4 = if e.get(13).is_some() { e.pt(13) } else { p3 };
                let mut pts = vec![p1, p2, p4, p3];
                pts.dedup();
                let pts: Vec<P> = pts.into_iter().map(|p| ocs.apply(p)).collect();
                if pts.len() >= 3 {
                    self.push(Geo::Fill { pts }, &layer, dash, width, key)?;
                }
            }
            "POINT" => {
                let p = t.apply(e.pt(10));
                if finite(p) {
                    self.push(Geo::Dot(p), &layer, dash, width, key)?;
                }
            }
            "HATCH" => self.hatch(e, ocs, &layer, dash, width, key)?,
            "TEXT" | "ATTRIB" | "ATTDEF" => {
                if e.kind != "TEXT" && e.i(70, 0) & 1 != 0 {
                    return Ok(());
                }
                let raw = if e.kind == "ATTDEF" { e.str(2) } else { e.str(1) }.unwrap_or_default();
                self.text(e, &percent_codes(&raw), ocs, &layer, key)?;
            }
            "MTEXT" => self.mtext(e, t, &layer, key)?,
            "INSERT" => self.insert(e, t, ocs, &layer, width, key, depth)?,
            "DIMENSION" | "ACAD_TABLE" => {
                // Drawn through their anonymous blocks (*D…, *T…), whose geometry is in the entity's frame.
                if let Some(name) = e.str(2) {
                    self.block(&name, ocs, key, depth, &layer, e.str(6), width)?;
                }
            }
            "LEADER" => {
                let pts: Vec<P> = e
                    .pairs
                    .iter()
                    .filter(|p| p.code == 10)
                    .zip(e.pairs.iter().filter(|p| p.code == 20))
                    .map(|(x, y)| t.apply((x.num().unwrap_or(0.0), y.num().unwrap_or(0.0))))
                    .collect();
                self.poly(pts, false, &layer, dash, width, key)?;
            }
            "IMAGE" => {
                let p = e.pt(10);
                let (u, v) = (e.pt(11), e.pt(12));
                let (w, h) = e.pt(13);
                let corner = |i: f64, j: f64| t.apply((p.0 + u.0 * i + v.0 * j, p.1 + u.1 * i + v.1 * j));
                let pts = vec![corner(0.0, 0.0), corner(w, 0.0), corner(w, h), corner(0.0, h)];
                let file = e
                    .str(340)
                    .and_then(|h| self.d.images.get(&h.trim().to_ascii_uppercase()).cloned())
                    .unwrap_or_default();
                // The placeholder names the file, never its folders (no local path enters the project).
                let name = file.rsplit(['/', '\\']).next().unwrap_or("").to_owned();
                self.notes.external.push(if name.is_empty() { "image".to_owned() } else { name.clone() });
                let middle = corner(w / 2.0, h / 2.0);
                let height = ((v.0 * v.0 + v.1 * v.1).sqrt() * h / 10.0).abs();
                self.poly(pts, true, &layer, dash, width, key)?;
                if !name.is_empty() && height > 0.0 {
                    let angle = angle_of(t.vector(u));
                    let geo = Geo::Text {
                        at: middle,
                        height,
                        angle,
                        text: name,
                        align: HAlign::Centre,
                    };
                    self.push(geo, &layer, dash, width, &format!("{key}.text"))?;
                }
            }
            "EW-VIEWPORT" => {
                // A paper-space viewport (see `emit::layout_entities`): the model space at its scale, clipped to it.
                let c = e.pt(10);
                let (w, h) = (e.f(40, 0.0), e.f(41, 0.0));
                let view = e.pt(12);
                let s = e.f(45, 1.0);
                let m = t
                    .after(Affine::translate(c.0, c.1))
                    .after(Affine::scale(s, s))
                    .after(Affine::translate(-view.0, -view.1));
                let start = self.shapes.len();
                self.walk(&e.children, m, key, depth, None)?;
                let (lo, hi) = ((c.0 - w / 2.0, c.1 - h / 2.0), (c.0 + w / 2.0, c.1 + h / 2.0));
                let tol = self.tol;
                let clipped: Vec<Shape> = self.shapes.drain(start..).flat_map(|s| clip(s, lo, hi, tol)).collect();
                self.shapes.extend(clipped);
            }
            "VIEWPORT" | "SEQEND" | "ENDBLK" | "BLOCK" | "VERTEX" => {}
            other => {
                *self.notes.unsupported.entry(other.to_owned()).or_default() += 1;
            }
        }
        Ok(())
    }

    /// The vertices of a polyline with bulges, mapped by `map` (bulge arcs flattened in the polyline's own frame).
    fn bulged(&self, verts: &[(P, f64)], closed: bool, map: Affine) -> Vec<P> {
        let tol = self.tol / map.max_scale().max(1e-300);
        let mut pts: Vec<P> = Vec::with_capacity(verts.len());
        let Some(first) = verts.first() else {
            return pts;
        };
        pts.push(first.0);
        let n = verts.len();
        let segments = if closed { n } else { n.saturating_sub(1) };
        for i in 0..segments {
            let (a, bulge) = verts[i];
            let b = verts[(i + 1) % n].0;
            pts.extend(bulge_points(a, b, bulge, tol));
        }
        if closed && pts.len() > 1 && pts.first() == pts.last() {
            pts.pop();
        }
        pts.into_iter().map(|p| map.apply(p)).collect()
    }

    fn poly(&mut self, pts: Vec<P>, closed: bool, layer: &str, dash: Pattern, width: f64, key: &str) -> Result<(), ImportError> {
        let mut pts = pts;
        pts.dedup();
        if pts.len() < 2 {
            return Ok(());
        }
        if !pts.iter().all(|p| finite(*p)) {
            self.notes.malformed.push(key.to_owned());
            return Ok(());
        }
        self.push(Geo::Poly { pts, closed }, layer, dash, width, key)
    }

    #[allow(clippy::too_many_arguments)]
    fn arc(
        &mut self,
        c: P,
        r: f64,
        start: f64,
        sweep: f64,
        map: Affine,
        layer: &str,
        dash: Pattern,
        width: f64,
        key: &str,
    ) -> Result<(), ImportError> {
        if !(finite(c) && r.is_finite() && start.is_finite() && sweep.is_finite()) || r <= 0.0 {
            if !r.is_finite() || !finite(c) {
                self.notes.malformed.push(key.to_owned());
            }
            return Ok(());
        }
        match map.similarity() {
            Some(s) => {
                let mirrored = map.det() < 0.0;
                let (sv, cv) = sin_cos_deg(start);
                let mut start2 = angle_of(map.vector((cv, sv)));
                let mut sweep2 = sweep;
                if mirrored {
                    // A mirror turns the arc clockwise: drawn counter-clockwise from its other end.
                    sweep2 = sweep;
                    start2 -= sweep;
                }
                if (sweep - 360.0).abs() < 1e-9 {
                    start2 = 0.0;
                    sweep2 = 360.0;
                }
                let geo = Geo::Arc {
                    c: map.apply(c),
                    r: r * s,
                    start: start2,
                    sweep: sweep2,
                };
                self.push(geo, layer, dash, width, key)
            }
            None => {
                let tol = self.tol / map.max_scale().max(1e-300);
                let pts: Vec<P> = arc_points(c, r, start, sweep, tol).into_iter().map(|p| map.apply(p)).collect();
                let closed = (sweep - 360.0).abs() < 1e-9;
                self.poly(pts, closed, layer, dash, width, key)
            }
        }
    }

    fn spline(&mut self, e: &Entity, t: Affine, layer: &str, dash: Pattern, width: f64, key: &str) -> Result<(), ImportError> {
        let pairs_of = |x: i32| -> Vec<P> {
            e.pairs
                .iter()
                .filter(|p| p.code == x)
                .zip(e.pairs.iter().filter(|p| p.code == x + 10))
                .map(|(a, b)| (a.num().unwrap_or(f64::NAN), b.num().unwrap_or(f64::NAN)))
                .collect()
        };
        let ctrl = pairs_of(10);
        if ctrl.len() > MAX_SPLINE_POINTS {
            self.notes.skipped.push((
                key.to_owned(),
                format!("the spline has more than {MAX_SPLINE_POINTS} control points"),
            ));
            return Ok(());
        }
        let knots: Vec<f64> = e.pairs.iter().filter(|p| p.code == 40).filter_map(|p| p.num()).collect();
        let weights: Vec<f64> = e.pairs.iter().filter(|p| p.code == 41).filter_map(|p| p.num()).collect();
        let degree = usize::try_from(e.i(71, 3)).unwrap_or(3).clamp(1, 32);
        let closed = e.i(70, 0) & 1 != 0;
        let tol = self.tol / t.max_scale().max(1e-300);
        let pts = if ctrl.is_empty() {
            // A spline given by its fit points only: drawn through them.
            pairs_of(11)
        } else {
            spline_points(degree, &ctrl, &weights, &knots, tol).unwrap_or(ctrl)
        };
        let pts: Vec<P> = pts.into_iter().map(|p| t.apply(p)).collect();
        self.poly(pts, closed, layer, dash, width, key)
    }

    fn hatch(&mut self, e: &Entity, ocs: Affine, layer: &str, dash: Pattern, width: f64, key: &str) -> Result<(), ImportError> {
        let solid = e.i(70, 0) & 1 != 0;
        let pattern = e.str(2).unwrap_or_default().trim().to_owned();
        let tol = self.tol / ocs.max_scale().max(1e-300);
        let pairs = &e.pairs;
        let Some(start) = pairs.iter().position(|p| p.code == 91) else {
            return Ok(());
        };
        let paths = usize::try_from(pairs[start].int().unwrap_or(0)).unwrap_or(0);
        let mut i = start + 1;
        // The next value of `code` at or after the cursor.
        let take = |i: &mut usize, code: i32| -> Option<f64> {
            while *i < pairs.len() {
                let p = &pairs[*i];
                *i += 1;
                if p.code == code {
                    return p.num();
                }
                if p.code == 92 || p.code == 75 {
                    *i -= 1;
                    return None;
                }
            }
            None
        };
        let mut loops: Vec<(Vec<P>, bool)> = Vec::new();
        for _ in 0..paths.min(10_000) {
            let Some(flags) = take(&mut i, 92) else { break };
            let flags = flags as i64;
            let outer = flags & (1 | 16) != 0;
            let mut pts: Vec<P> = Vec::new();
            if flags & 2 != 0 {
                let bulges = take(&mut i, 72).unwrap_or(0.0) != 0.0;
                let _closed = take(&mut i, 73);
                let n = take(&mut i, 93).unwrap_or(0.0).max(0.0) as usize;
                let mut verts: Vec<(P, f64)> = Vec::new();
                for _ in 0..n.min(MAX_SPLINE_POINTS) {
                    let x = take(&mut i, 10).unwrap_or(0.0);
                    let y = take(&mut i, 20).unwrap_or(0.0);
                    let b = if bulges { take(&mut i, 42).unwrap_or(0.0) } else { 0.0 };
                    verts.push(((x, y), b));
                }
                pts = self.bulged(&verts, true, Affine::IDENTITY);
            } else {
                let edges = take(&mut i, 93).unwrap_or(0.0).max(0.0) as usize;
                for _ in 0..edges.min(MAX_SPLINE_POINTS) {
                    let kind = take(&mut i, 72).unwrap_or(1.0) as i64;
                    match kind {
                        1 => {
                            let a = (take(&mut i, 10).unwrap_or(0.0), take(&mut i, 20).unwrap_or(0.0));
                            let b = (take(&mut i, 11).unwrap_or(0.0), take(&mut i, 21).unwrap_or(0.0));
                            if pts.last() != Some(&a) {
                                pts.push(a);
                            }
                            pts.push(b);
                        }
                        2 | 3 => {
                            let c = (take(&mut i, 10).unwrap_or(0.0), take(&mut i, 20).unwrap_or(0.0));
                            let (m, ratio) = if kind == 3 {
                                let m = (take(&mut i, 11).unwrap_or(1.0), take(&mut i, 21).unwrap_or(0.0));
                                (m, take(&mut i, 40).unwrap_or(1.0))
                            } else {
                                ((take(&mut i, 40).unwrap_or(0.0), 0.0), 1.0)
                            };
                            let (a0, a1) = (take(&mut i, 50).unwrap_or(0.0), take(&mut i, 51).unwrap_or(360.0));
                            let ccw = take(&mut i, 73).unwrap_or(1.0) != 0.0;
                            // A clockwise edge gives its angles mirrored: drawn from −start back to −end.
                            let (from, sweep) = if ccw {
                                (a0, (a1 - a0).rem_euclid(360.0))
                            } else {
                                (-a0, -((a1 - a0).rem_euclid(360.0)))
                            };
                            let sweep = if sweep == 0.0 { 360.0 } else { sweep };
                            let r = (m.0 * m.0 + m.1 * m.1).sqrt();
                            let minor = (-m.1 * ratio, m.0 * ratio);
                            for (co, s) in arc_points((0.0, 0.0), 1.0, from, sweep, if r > 0.0 { tol / r } else { 1.0 }) {
                                pts.push((c.0 + m.0 * co + minor.0 * s, c.1 + m.1 * co + minor.1 * s));
                            }
                        }
                        _ => {
                            // A spline edge: degree, rational, periodic, knots, control points (weights), fit points.
                            let degree = take(&mut i, 94).unwrap_or(3.0) as usize;
                            let _rational = take(&mut i, 73);
                            let _periodic = take(&mut i, 74);
                            let nk = take(&mut i, 95).unwrap_or(0.0).max(0.0) as usize;
                            let nc = take(&mut i, 96).unwrap_or(0.0).max(0.0) as usize;
                            if nc > MAX_SPLINE_POINTS || nk > MAX_SPLINE_POINTS * 2 {
                                break;
                            }
                            let knots: Vec<f64> = (0..nk).map(|_| take(&mut i, 40).unwrap_or(0.0)).collect();
                            let ctrl: Vec<P> = (0..nc)
                                .map(|_| (take(&mut i, 10).unwrap_or(0.0), take(&mut i, 20).unwrap_or(0.0)))
                                .collect();
                            let curve = spline_points(degree.clamp(1, 32), &ctrl, &[], &knots, tol).unwrap_or(ctrl);
                            pts.extend(curve);
                        }
                    }
                }
            }
            // The boundary's source objects (code 97, then as many 330 handles) are skipped by the cursor.
            pts.dedup();
            let pts: Vec<P> = pts.into_iter().map(|p| ocs.apply(p)).collect();
            if pts.len() >= 3 && pts.iter().all(|p| finite(*p)) {
                loops.push((pts, outer));
            }
        }
        if !solid {
            *self.notes.patterns.entry(if pattern.is_empty() { "?".to_owned() } else { pattern }).or_default() += 1;
        }
        let any_outer = loops.iter().any(|(_, o)| *o);
        for (n, (pts, outer)) in loops.into_iter().enumerate() {
            let k = if n == 0 { key.to_owned() } else { format!("{key}.{n}") };
            if solid && (outer || !any_outer) {
                self.push(Geo::Fill { pts }, layer, dash, width, &k)?;
            } else {
                self.push(Geo::Poly { pts, closed: true }, layer, dash, width, &k)?;
            }
        }
        Ok(())
    }

    /// The font family substitution of a text style, recorded once per SHX font.
    fn note_font(&mut self, e: &Entity) {
        let style = e.str(7).unwrap_or_else(|| "STANDARD".to_owned()).trim().to_ascii_uppercase();
        let font = self.d.styles.get(&style).cloned().unwrap_or_else(|| "txt".to_owned());
        if let Some(family) = shx_substitute(&font) {
            let file = font.rsplit(['/', '\\']).next().unwrap_or(&font).trim().to_ascii_lowercase();
            let file = if file.ends_with(".shx") { file } else { format!("{file}.shx") };
            self.notes.shx.insert(file, family);
        }
    }

    /// One placed text line: `base` its baseline anchor of alignment `align` in the entity's frame, `up` the
    /// distance from the baseline to the text's middle; mapped through `map`.
    #[allow(clippy::too_many_arguments)]
    fn placed(
        &mut self,
        base: P,
        rot: f64,
        height: f64,
        up: f64,
        align: HAlign,
        text: String,
        map: Affine,
        layer: &str,
        key: &str,
    ) -> Result<(), ImportError> {
        if text.trim().is_empty() || !(height.is_finite() && height > 0.0) || !finite(base) {
            return Ok(());
        }
        let (s, c) = sin_cos_deg(rot);
        let middle = (base.0 - s * up, base.1 + c * up);
        let dir = map.vector((c, s));
        let upv = map.vector((-s, c));
        let mut angle = angle_of(dir);
        let mut align = align;
        if map.det() < 0.0 && !self.mirrtext {
            // A mirrored text stays readable: it runs the other way from its mirrored anchor.
            angle += 180.0;
            align = match align {
                HAlign::Left => HAlign::Right,
                HAlign::Right => HAlign::Left,
                HAlign::Centre => HAlign::Centre,
            };
        }
        let geo = Geo::Text {
            at: map.apply(middle),
            height: height * (upv.0 * upv.0 + upv.1 * upv.1).sqrt(),
            angle,
            text,
            align,
        };
        self.push(geo, layer, Pattern::Solid, 0.0, key)
    }

    fn text(&mut self, e: &Entity, text: &str, map: Affine, layer: &str, key: &str) -> Result<(), ImportError> {
        self.note_font(e);
        let h = e.f(40, 2.5);
        let rot = e.f(50, 0.0);
        let (ha, va) = (e.i(72, 0), e.i(73, 0));
        let flags = e.i(71, 0);
        let p10 = e.pt(10);
        let p11 = if e.get(11).is_some() { e.pt(11) } else { p10 };
        let (base, align, rot) = match ha {
            1 | 4 => (p11, HAlign::Centre, rot),
            2 => (p11, HAlign::Right, rot),
            3 | 5 => {
                // Aligned and fit: centred between the two points, along their direction.
                let mid = ((p10.0 + p11.0) / 2.0, (p10.1 + p11.1) / 2.0);
                let rot = if p10 == p11 { rot } else { angle_of((p11.0 - p10.0, p11.1 - p10.1)) };
                (mid, HAlign::Centre, rot)
            }
            _ if va != 0 => (p11, HAlign::Left, rot),
            _ => (p10, HAlign::Left, rot),
        };
        let up = match (ha, va) {
            (4, _) | (_, 2) => 0.0,
            (_, 3) => -h / 2.0,
            _ => h / 2.0,
        };
        // A backwards (2) or upside-down (4) text is mirrored in its own frame.
        let mut m = map;
        if flags & 6 != 0 {
            let sx = if flags & 2 != 0 { -1.0 } else { 1.0 };
            let sy = if flags & 4 != 0 { -1.0 } else { 1.0 };
            m = map
                .after(Affine::translate(base.0, base.1))
                .after(Affine::rotate(rot))
                .after(Affine::scale(sx, sy))
                .after(Affine::rotate(-rot))
                .after(Affine::translate(-base.0, -base.1));
        }
        self.placed(base, rot, h, up, align, text.to_owned(), m, layer, key)
    }

    fn mtext(&mut self, e: &Entity, t: Affine, layer: &str, key: &str) -> Result<(), ImportError> {
        self.note_font(e);
        let mut raw = String::new();
        for p in e.pairs.iter().filter(|p| p.code == 3) {
            raw.push_str(&p.text());
        }
        raw.push_str(&e.str(1).unwrap_or_default());
        let lines = mtext_lines(&raw);
        let h = e.f(40, 2.5);
        let spacing = h * 5.0 / 3.0 * e.f(44, 1.0).clamp(0.25, 4.0);
        let rot = if e.get(11).is_some() { angle_of(e.pt(11)) } else { e.f(50, 0.0) };
        let attach = e.i(71, 1).clamp(1, 9);
        let align = match (attach - 1) % 3 {
            0 => HAlign::Left,
            1 => HAlign::Centre,
            _ => HAlign::Right,
        };
        let n = lines.len() as f64;
        let block = h + (n - 1.0) * spacing;
        // The distance from the insertion point down to the first line's middle.
        let first = match (attach - 1) / 3 {
            0 => h / 2.0,
            1 => h / 2.0 - block / 2.0,
            _ => h / 2.0 - block,
        };
        let at = e.pt(10);
        let (s, c) = sin_cos_deg(rot);
        // MTEXT's insertion point and direction are in the world frame; an extrusion of −z mirrors them.
        let map = if e.extrusion().2 < 0.0 { t.after(Affine::scale(-1.0, 1.0)) } else { t };
        for (k, line) in lines.into_iter().enumerate() {
            let down = first + spacing * k as f64;
            let base = (at.0 + s * down, at.1 - c * down);
            let line_key = if k == 0 { key.to_owned() } else { format!("{key}.{k}") };
            self.placed(base, rot, h, 0.0, align, line, map, layer, &line_key)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn insert(
        &mut self,
        e: &Entity,
        t: Affine,
        ocs: Affine,
        layer: &str,
        width: f64,
        key: &str,
        depth: usize,
    ) -> Result<(), ImportError> {
        let name = e.str(2).unwrap_or_default();
        let ins = e.pt(10);
        let (sx, sy) = (e.f(41, 1.0), e.f(42, 1.0));
        let rot = e.f(50, 0.0);
        let (cols, rows) = (e.i(70, 1).clamp(1, 100_000), e.i(71, 1).clamp(1, 100_000));
        let (cs, rs) = (e.f(44, 0.0), e.f(45, 0.0));
        // An array larger than the expansion limit is refused before a cell is expanded (an empty block's cells cost
        // time though they add no shape).
        if usize::try_from(rows * cols).unwrap_or(usize::MAX) > MAX_PRIMITIVES {
            return Err(limit("DXF block expansion (shapes)", MAX_PRIMITIVES));
        }
        let ltype = e.str(6).map(|s| s.trim().to_ascii_uppercase());
        for r in 0..rows {
            for c in 0..cols {
                let cell = if rows * cols == 1 { key.to_owned() } else { format!("{key}[{r},{c}]") };
                let base = self
                    .d
                    .blocks
                    .get(&name.trim().to_ascii_uppercase())
                    .map_or((0.0, 0.0), |b| b.base);
                let m = ocs
                    .after(Affine::translate(ins.0, ins.1))
                    .after(Affine::rotate(rot))
                    .after(Affine::translate(c as f64 * cs, r as f64 * rs))
                    .after(Affine::scale(sx, sy))
                    .after(Affine::translate(-base.0, -base.1));
                self.block(&name, m, &cell, depth, layer, ltype.clone(), width)?;
            }
        }
        // The attributes carry their own position (in the insert's parent frame).
        for (n, a) in e.children.iter().enumerate() {
            let own = a.handle().unwrap_or_else(|| format!("#{n}"));
            let k = format!("{key}/{own}");
            self.entity(a, t, &k, depth, None)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn block(
        &mut self,
        name: &str,
        m: Affine,
        key: &str,
        depth: usize,
        layer: &str,
        ltype: Option<String>,
        width: f64,
    ) -> Result<(), ImportError> {
        let d = self.d;
        let Some(block) = d.blocks.get(&name.trim().to_ascii_uppercase()) else {
            return Ok(());
        };
        if block.flags & 4 != 0 {
            self.notes.external.push(block.name.clone());
            return Ok(());
        }
        if depth >= MAX_NESTING {
            self.notes.skipped.push((
                key.to_owned(),
                format!("the block {} is nested deeper than {MAX_NESTING} levels", block.name),
            ));
            return Ok(());
        }
        let inh = Inherit {
            layer: layer.to_owned(),
            ltype,
            width,
        };
        let entities = &block.entities;
        // A block's attribute definitions are drawn only when constant; the inserts' ATTRIBs print the values.
        for (n, e) in entities.iter().enumerate() {
            if e.kind == "ATTDEF" && e.i(70, 0) & 2 == 0 {
                continue;
            }
            let own = e.handle().unwrap_or_else(|| format!("#{n}"));
            let k = format!("{key}/{own}");
            if e.kind == "ATTDEF" {
                let text = percent_codes(&e.str(1).unwrap_or_default());
                let lay = if e.layer() == "0" { layer.to_owned() } else { e.layer() };
                self.text(e, &text, m.after(Affine::ocs(e.extrusion())), &lay, &k)?;
                continue;
            }
            self.entity(e, m, &k, depth + 1, Some(&inh))?;
        }
        Ok(())
    }
}

/// The parts of `s` inside the rectangle `lo`–`hi` (a viewport): polylines cut into the runs inside (Liang–Barsky per
/// segment), filled polygons cut to the rectangle (Sutherland–Hodgman), arcs flattened and cut when they cross it,
/// texts and dots kept when their anchor lies inside.
fn clip(s: Shape, lo: P, hi: P, tol: f64) -> Vec<Shape> {
    let inside = |p: P| p.0 >= lo.0 && p.0 <= hi.0 && p.1 >= lo.1 && p.1 <= hi.1;
    let with = |geo: Geo, n: usize| Shape {
        geo,
        key: if n == 0 { s.key.clone() } else { format!("{}.{n}", s.key) },
        layer: s.layer.clone(),
        dash: s.dash,
        width: s.width,
    };
    let runs = |pts: &[P], closed: bool| -> Vec<Vec<P>> {
        let mut out: Vec<Vec<P>> = Vec::new();
        let mut run: Vec<P> = Vec::new();
        let n = pts.len();
        let segs = if closed { n } else { n.saturating_sub(1) };
        for i in 0..segs {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            match segment(a, b, lo, hi) {
                Some((p, q)) => {
                    if run.last() != Some(&p) {
                        if run.len() >= 2 {
                            out.push(std::mem::take(&mut run));
                        }
                        run.clear();
                        run.push(p);
                    }
                    run.push(q);
                }
                None => {
                    if run.len() >= 2 {
                        out.push(std::mem::take(&mut run));
                    }
                    run.clear();
                }
            }
        }
        if run.len() >= 2 {
            out.push(run);
        }
        out
    };
    match &s.geo {
        Geo::Poly { pts, closed } => {
            if pts.iter().all(|p| inside(*p)) {
                return vec![s];
            }
            runs(pts, *closed)
                .into_iter()
                .enumerate()
                .map(|(n, pts)| with(Geo::Poly { pts, closed: false }, n))
                .collect()
        }
        Geo::Arc { c, r, start, sweep } => {
            let pts = arc_points(*c, *r, *start, *sweep, tol);
            if pts.iter().all(|p| inside(*p)) {
                return vec![s];
            }
            runs(&pts, false)
                .into_iter()
                .enumerate()
                .map(|(n, pts)| with(Geo::Poly { pts, closed: false }, n))
                .collect()
        }
        Geo::Fill { pts } => {
            let mut poly = pts.clone();
            // Sutherland–Hodgman against the four edges.
            let edges: [(usize, f64, bool); 4] =
                [(0, lo.0, true), (0, hi.0, false), (1, lo.1, true), (1, hi.1, false)];
            for (axis, v, keep_above) in edges {
                let get = |p: P| if axis == 0 { p.0 } else { p.1 };
                let ins = |p: P| if keep_above { get(p) >= v } else { get(p) <= v };
                let cut = |a: P, b: P| {
                    let t = (v - get(a)) / (get(b) - get(a));
                    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
                };
                let mut out = Vec::new();
                for i in 0..poly.len() {
                    let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
                    match (ins(a), ins(b)) {
                        (true, true) => out.push(b),
                        (true, false) => out.push(cut(a, b)),
                        (false, true) => {
                            out.push(cut(a, b));
                            out.push(b);
                        }
                        (false, false) => {}
                    }
                }
                poly = out;
                if poly.is_empty() {
                    break;
                }
            }
            if poly.len() >= 3 {
                vec![with(Geo::Fill { pts: poly }, 0)]
            } else {
                Vec::new()
            }
        }
        Geo::Text { at, .. } | Geo::Dot(at) => {
            if inside(*at) {
                vec![s]
            } else {
                Vec::new()
            }
        }
    }
}

/// The part of the segment `a`–`b` inside the rectangle `lo`–`hi` (Liang–Barsky).
fn segment(a: P, b: P, lo: P, hi: P) -> Option<(P, P)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [(-dx, a.0 - lo.0), (dx, hi.0 - a.0), (-dy, a.1 - lo.1), (dy, hi.1 - a.1)] {
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
    (t0 <= t1).then_some(((a.0 + dx * t0, a.1 + dy * t0), (a.0 + dx * t1, a.1 + dy * t1)))
}
