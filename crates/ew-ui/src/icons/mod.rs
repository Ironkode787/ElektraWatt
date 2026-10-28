// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The icon set (WP-S2.17 part 6, D9; doc 16a §4.11): Tabler Icons 3.50.0 (MIT) and the project's own glyphs,
//! converted by `cargo xtask gen-icons` into [`generated`] and drawn through epaint. No runtime SVG parsing.
//!
//! `ew-viewmodel` names an icon by its `&'static str` key (the Tabler name, `ew-<name>` for the project's
//! glyphs) and stays free of egui; [`IconId::from_key`] maps the key. Stroke width, size and colour are the
//! caller's (the token module supplies them); [`paint`] draws the 24-unit grid scaled into a square centred in
//! the rect, with round caps and joins emulated by discs at the segment ends.

// The table is generated with one segment per line; rustfmt would reflow it and break `gen-icons --check`.
#[rustfmt::skip]
mod generated;

pub use generated::{COUNT, IconId, KEYS};

use egui::epaint::{CubicBezierShape, PathShape, PathStroke, QuadraticBezierShape};
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

/// One segment of an icon path on the 24-unit grid (the generated table's data).
#[allow(
    dead_code,
    reason = "the data format covers the segment kinds kurbo emits; the current set needs not all"
)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Seg {
    /// Move to.
    M(f32, f32),
    /// Line to.
    L(f32, f32),
    /// Quadratic curve: control, end.
    Q(f32, f32, f32, f32),
    /// Cubic curve: two controls, end.
    C(f32, f32, f32, f32, f32, f32),
    /// Close the subpath.
    Z,
}

/// The grid the icons are drawn on.
const GRID: f32 = 24.0;
/// The flattening tolerance of the curves, in points.
const TOLERANCE: f32 = 0.05;

impl IconId {
    /// The icon's key (`"arrow-back-up"`, `"ew-cable"`).
    pub fn key(self) -> &'static str {
        KEYS[self as usize]
    }

    /// The icon named by `key`, if the table has it.
    pub fn from_key(key: &str) -> Option<IconId> {
        KEYS.binary_search(&key).ok().map(|i| IconId::ALL[i])
    }

    pub(crate) fn segments(self) -> &'static [Seg] {
        generated::PATHS[self as usize]
    }
}

/// The shapes of `id` drawn in the square of side `min(rect.width, rect.height)` centred in `rect`, with a stroke
/// of `stroke_px` points (the caller multiplies nothing: egui scales points by the pixels per point).
pub fn shapes(id: IconId, rect: Rect, color: Color32, stroke_px: f32) -> Vec<Shape> {
    let side = rect.width().min(rect.height());
    let scale = side / GRID;
    let origin = rect.center() - Vec2::splat(side / 2.0);
    let at = |x: f32, y: f32| origin + Vec2::new(x, y) * scale;
    let stroke = Stroke::new(stroke_px, color);
    let radius = stroke_px / 2.0;

    let mut out = Vec::new();
    let mut points: Vec<Pos2> = Vec::new();
    let mut joints: Vec<Pos2> = Vec::new();
    let flush = |points: &mut Vec<Pos2>, joints: &mut Vec<Pos2>, closed: bool, out: &mut Vec<Shape>| {
        if points.len() >= 2 {
            let path = if closed {
                PathShape::closed_line(std::mem::take(points), stroke)
            } else {
                PathShape::line(std::mem::take(points), stroke)
            };
            out.push(Shape::Path(path));
        }
        points.clear();
        for p in joints.drain(..) {
            out.push(Shape::circle_filled(p, radius, color));
        }
    };
    let mut start = Pos2::ZERO;
    for seg in id.segments() {
        let current = points.last().copied().unwrap_or(start);
        match *seg {
            Seg::M(x, y) => {
                flush(&mut points, &mut joints, false, &mut out);
                start = at(x, y);
                points.push(start);
                joints.push(start);
            }
            Seg::L(x, y) => {
                let p = at(x, y);
                points.push(p);
                joints.push(p);
            }
            Seg::Q(x1, y1, x, y) => {
                let q = QuadraticBezierShape::from_points_stroke(
                    [current, at(x1, y1), at(x, y)],
                    false,
                    Color32::TRANSPARENT,
                    PathStroke::NONE,
                );
                points.extend(q.flatten(Some(TOLERANCE)).into_iter().skip(1));
                joints.push(at(x, y));
            }
            Seg::C(x1, y1, x2, y2, x, y) => {
                let c = CubicBezierShape::from_points_stroke(
                    [current, at(x1, y1), at(x2, y2), at(x, y)],
                    false,
                    Color32::TRANSPARENT,
                    PathStroke::NONE,
                );
                points.extend(c.flatten(Some(TOLERANCE)).into_iter().skip(1));
                joints.push(at(x, y));
            }
            Seg::Z => {
                if points.len() >= 2 && points.last() == points.first() {
                    points.pop();
                }
                flush(&mut points, &mut joints, true, &mut out);
            }
        }
    }
    flush(&mut points, &mut joints, false, &mut out);
    out
}

/// Draws `id` into `rect` (doc 16a §4.11): `stroke_px` and the rect's size come from the size and stroke tokens,
/// `color` from the context's role colour.
pub fn paint(painter: &Painter, id: IconId, rect: Rect, color: Color32, stroke_px: f32) {
    painter.extend(shapes(id, rect, color, stroke_px));
}
