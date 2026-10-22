// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tessellation (WP-S2.02 §7.1): fills and strokes of the display list, and glyph outlines, into triangle meshes
//! with lyon.
//!
//! **Vertices** carry the point on the path, the stroke's normal and half-width (both zero for a fill), the
//! colour and the primitive's paint index within its mesh. The vertex shader places a vertex at
//! `point + normal × max(half-width, half a physical pixel)`, so every stroke is at least one physical pixel wide at
//! any zoom (architecture §8.2's screen-space lines), and the depth comes from the paint index. Vertex data is
//! written with `to_le_bytes` (all supported targets are little-endian), so no `bytemuck` and no `unsafe` is
//! needed.
//!
//! **Units.** Mesh-local coordinates are `f32` micrometres for paths (exact to 0.06 µm up to 1 m) and font units
//! for glyph outlines; the instance transform maps either to page micrometres.
//!
//! **Rules.** Fills use lyon's fill tessellator with the non-zero rule; strokes lyon's stroke tessellator with the
//! display list's width, caps, joins and its miter limit (`ew_dl::MITER_LIMIT`), dashes split first with
//! `kurbo::dash` as the CPU renderer does (a dash array that is empty or has a non-positive entry draws solid).
//! A primitive lyon cannot tessellate is skipped and counted, never a panic.

use std::panic::{AssertUnwindSafe, catch_unwind};

use ew_dl::{Cap, Color, Join, MITER_LIMIT, Stroke};
use lyon_tessellation::math::{Point, point};
use lyon_tessellation::path::Path as LyonPath;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, FillVertexConstructor, LineCap,
    LineJoin, StrokeOptions, StrokeTessellator, StrokeVertex, StrokeVertexConstructor, VertexBuffers,
};

/// The bytes of one vertex: point (2 × f32), normal (2 × f32), half-width (f32), colour (4 × u8), paint index
/// (u32).
pub const VERTEX_BYTES: usize = 28;

/// The tolerance of path tessellation: 1 µm.
pub const PATH_TOLERANCE_UM: f32 = 1.0;

/// A triangle mesh: vertices as [`VERTEX_BYTES`]-byte records and `u32` indices.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct MeshData {
    /// The vertex records.
    pub vertices: Vec<u8>,
    /// Three indices per triangle.
    pub indices: Vec<u32>,
}

impl MeshData {
    /// The number of vertices.
    pub fn vertex_count(&self) -> u32 {
        (self.vertices.len() / VERTEX_BYTES) as u32
    }

    /// Whether it has no triangle.
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// The bytes it occupies (vertices and indices).
    pub fn bytes(&self) -> u64 {
        (self.vertices.len() + self.indices.len() * 4) as u64
    }

    /// The axis-aligned unit square `[0, 1]²` as two triangles in white, paint index 0: the mesh of the LOD bars.
    pub fn unit_square() -> MeshData {
        let mut m = MeshData::default();
        for (x, y) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            push_vertex(&mut m.vertices, [x, y], [0.0, 0.0], 0.0, Color::WHITE, 0);
        }
        m.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        m
    }

    /// Appends `other`'s geometry (its colours and indices as they are).
    fn append(&mut self, buffers: &VertexBuffers<Vtx, u32>, color: Color, index: u32) {
        let base = self.vertex_count();
        for v in &buffers.vertices {
            push_vertex(&mut self.vertices, v.pos, v.normal, v.half_width, color, index);
        }
        self.indices.extend(buffers.indices.iter().map(|i| base + i));
    }
}

fn push_vertex(out: &mut Vec<u8>, pos: [f32; 2], normal: [f32; 2], half_width: f32, c: Color, index: u32) {
    for v in [pos[0], pos[1], normal[0], normal[1], half_width] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&[c.r, c.g, c.b, c.a]);
    out.extend_from_slice(&index.to_le_bytes());
}

/// One tessellated vertex before its colour and index are known.
#[derive(Clone, Copy, Debug)]
struct Vtx {
    pos: [f32; 2],
    normal: [f32; 2],
    half_width: f32,
}

struct FillCtor;

impl FillVertexConstructor<Vtx> for FillCtor {
    fn new_vertex(&mut self, vertex: FillVertex<'_>) -> Vtx {
        let p = vertex.position();
        Vtx {
            pos: [p.x, p.y],
            normal: [0.0, 0.0],
            half_width: 0.0,
        }
    }
}

struct StrokeCtor {
    half_width: f32,
}

impl StrokeVertexConstructor<Vtx> for StrokeCtor {
    fn new_vertex(&mut self, vertex: StrokeVertex<'_, '_>) -> Vtx {
        let p = vertex.position_on_path();
        let n = vertex.normal();
        Vtx {
            pos: [p.x, p.y],
            normal: [n.x, n.y],
            half_width: self.half_width,
        }
    }
}

/// A `kurbo` path (nm, or font units for glyphs) as a lyon path with every coordinate multiplied by `scale`;
/// `None` when it has no segment. Elements before the first `MoveTo` start at the origin (kurbo's rule).
pub fn lyon_path(els: impl IntoIterator<Item = kurbo::PathEl>, scale: f64) -> Option<LyonPath> {
    let pt = |p: kurbo::Point| -> Point { point((p.x * scale) as f32, (p.y * scale) as f32) };
    let mut b = LyonPath::builder();
    let mut open = false;
    let mut segments = 0usize;
    let mut start = point(0.0, 0.0);
    for el in els {
        match el {
            kurbo::PathEl::MoveTo(p) => {
                if open {
                    b.end(false);
                }
                start = pt(p);
                b.begin(start);
                open = true;
            }
            kurbo::PathEl::LineTo(p) => {
                if !open {
                    b.begin(start);
                    open = true;
                }
                b.line_to(pt(p));
                segments += 1;
            }
            kurbo::PathEl::QuadTo(c, p) => {
                if !open {
                    b.begin(start);
                    open = true;
                }
                b.quadratic_bezier_to(pt(c), pt(p));
                segments += 1;
            }
            kurbo::PathEl::CurveTo(c1, c2, p) => {
                if !open {
                    b.begin(start);
                    open = true;
                }
                b.cubic_bezier_to(pt(c1), pt(c2), pt(p));
                segments += 1;
            }
            kurbo::PathEl::ClosePath => {
                if open {
                    b.end(true);
                    open = false;
                }
            }
        }
    }
    if open {
        b.end(false);
    }
    (segments > 0).then(|| b.build())
}

/// Accumulates the primitives of one mesh.
#[derive(Default)]
pub struct MeshBuilder {
    mesh: MeshData,
    fill: FillTessellator,
    stroke: StrokeTessellator,
    /// Primitives lyon could not tessellate.
    pub failed: u32,
}

impl MeshBuilder {
    /// An empty mesh.
    pub fn new() -> MeshBuilder {
        MeshBuilder::default()
    }

    /// Fills `path` (non-zero rule) in `color` with paint index `index`, at `tolerance` (path units).
    pub fn fill(&mut self, path: &LyonPath, tolerance: f32, color: Color, index: u32) {
        let mut buffers: VertexBuffers<Vtx, u32> = VertexBuffers::new();
        let options = FillOptions::tolerance(tolerance).with_fill_rule(FillRule::NonZero);
        let fill = &mut self.fill;
        let done = catch_unwind(AssertUnwindSafe(|| {
            fill.tessellate_path(path, &options, &mut BuffersBuilder::new(&mut buffers, FillCtor))
        }));
        match done {
            Ok(Ok(())) => self.mesh.append(&buffers, color, index),
            _ => self.failed += 1,
        }
    }

    /// Strokes `path` (micrometres) with `stroke` (its width, caps, joins and the display list's miter limit; the
    /// dashes are split by the caller) in `color` with paint index `index`.
    pub fn stroke(&mut self, path: &LyonPath, stroke: &Stroke, color: Color, index: u32) {
        let width = (stroke.width.0 as f64 / 1000.0) as f32;
        if !(width > 0.0 && width.is_finite()) {
            return;
        }
        let cap = match stroke.cap {
            Cap::Butt => LineCap::Butt,
            Cap::Round => LineCap::Round,
            Cap::Square => LineCap::Square,
        };
        let join = match stroke.join {
            Join::Miter => LineJoin::Miter,
            Join::Round => LineJoin::Round,
            Join::Bevel => LineJoin::Bevel,
        };
        let options = StrokeOptions::tolerance(PATH_TOLERANCE_UM)
            .with_line_width(width)
            .with_line_cap(cap)
            .with_line_join(join)
            .with_miter_limit(MITER_LIMIT as f32);
        let mut buffers: VertexBuffers<Vtx, u32> = VertexBuffers::new();
        let ctor = StrokeCtor {
            half_width: width / 2.0,
        };
        let tess = &mut self.stroke;
        let done = catch_unwind(AssertUnwindSafe(|| {
            tess.tessellate_path(path, &options, &mut BuffersBuilder::new(&mut buffers, ctor))
        }));
        match done {
            Ok(Ok(())) => self.mesh.append(&buffers, color, index),
            _ => self.failed += 1,
        }
    }

    /// The mesh.
    pub fn finish(self) -> (MeshData, u32) {
        (self.mesh, self.failed)
    }
}

/// The elements a stroke draws: `path` itself, or its dashes split with `kurbo::dash` when the stroke has a valid
/// dash array (non-empty, every entry positive), as the CPU renderer draws them.
pub fn stroke_elements(path: &kurbo::BezPath, stroke: &Stroke) -> Vec<kurbo::PathEl> {
    match &stroke.dash {
        Some(d) if !d.segments.is_empty() && d.segments.iter().all(|n| n.0 > 0) => {
            let dashes: Vec<f64> = d.segments.iter().map(|n| n.0 as f64).collect();
            kurbo::dash(path.elements().iter().copied(), d.offset.0 as f64, &dashes).collect()
        }
        _ => path.elements().to_vec(),
    }
}

/// Reads vertex `i` of `mesh` back: point, normal, half-width, colour, paint index (tests).
pub fn read_vertex(mesh: &MeshData, i: usize) -> ([f32; 2], [f32; 2], f32, [u8; 4], u32) {
    let v = &mesh.vertices[i * VERTEX_BYTES..(i + 1) * VERTEX_BYTES];
    let f = |k: usize| f32::from_le_bytes([v[k], v[k + 1], v[k + 2], v[k + 3]]);
    (
        [f(0), f(4)],
        [f(8), f(12)],
        f(16),
        [v[20], v[21], v[22], v[23]],
        u32::from_le_bytes([v[24], v[25], v[26], v[27]]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_dl::ew_units::Nm;
    use ew_dl::{Brush, Dash};

    fn square() -> LyonPath {
        let els = [
            kurbo::PathEl::MoveTo(kurbo::Point::new(0.0, 0.0)),
            kurbo::PathEl::LineTo(kurbo::Point::new(10.0, 0.0)),
            kurbo::PathEl::LineTo(kurbo::Point::new(10.0, 10.0)),
            kurbo::PathEl::LineTo(kurbo::Point::new(0.0, 10.0)),
            kurbo::PathEl::ClosePath,
        ];
        lyon_path(els, 1.0).unwrap()
    }

    #[test]
    fn a_fill_is_triangles_with_zero_normals_and_its_colour_and_index() {
        let mut b = MeshBuilder::new();
        b.fill(&square(), PATH_TOLERANCE_UM, Color::from_rgb8(0, 0x72, 0xb2), 7);
        let (m, failed) = b.finish();
        assert_eq!(failed, 0);
        assert_eq!(m.indices.len(), 6);
        assert_eq!(m.vertex_count(), 4);
        for i in 0..4 {
            let (_, n, hw, c, idx) = read_vertex(&m, i);
            assert_eq!((n, hw, c, idx), ([0.0, 0.0], 0.0, [0, 0x72, 0xb2, 255], 7));
        }
    }

    #[test]
    fn a_stroke_carries_its_half_width_and_unit_normals() {
        let mut b = MeshBuilder::new();
        let line = lyon_path(
            [
                kurbo::PathEl::MoveTo(kurbo::Point::new(0.0, 0.0)),
                kurbo::PathEl::LineTo(kurbo::Point::new(100.0, 0.0)),
            ],
            1.0,
        )
        .unwrap();
        // 10 µm wide.
        b.stroke(&line, &Stroke::solid(Nm(10_000)), Color::BLACK, 0);
        let (m, _) = b.finish();
        assert!(m.vertex_count() >= 4);
        for i in 0..m.vertex_count() as usize {
            let (p, n, hw, _, _) = read_vertex(&m, i);
            assert_eq!(hw, 5.0);
            assert!((n[1].abs() - 1.0).abs() < 1e-5, "{n:?}");
            assert!(p[1].abs() < 1e-5);
        }
    }

    #[test]
    fn dashes_split_the_path_and_an_invalid_dash_draws_solid() {
        let path = kurbo::BezPath::from_vec(vec![
            kurbo::PathEl::MoveTo(kurbo::Point::new(0.0, 0.0)),
            kurbo::PathEl::LineTo(kurbo::Point::new(100.0, 0.0)),
        ]);
        let mut s = Stroke::solid(Nm(1));
        s.dash = Some(Dash {
            segments: vec![Nm(10), Nm(10)],
            offset: Nm(0),
        });
        let moves = |els: &[kurbo::PathEl]| {
            els.iter()
                .filter(|e| matches!(e, kurbo::PathEl::MoveTo(_)))
                .count()
        };
        assert_eq!(moves(&stroke_elements(&path, &s)), 5);
        s.dash = Some(Dash {
            segments: vec![Nm(10), Nm(0)],
            offset: Nm(0),
        });
        assert_eq!(moves(&stroke_elements(&path, &s)), 1);
        let _ = Brush::BLACK;
    }

    #[test]
    fn a_path_without_segments_is_none() {
        assert!(lyon_path([kurbo::PathEl::MoveTo(kurbo::Point::new(1.0, 1.0))], 1.0).is_none());
        assert!(lyon_path([], 1.0).is_none());
        // A segment before any MoveTo starts at the origin.
        assert!(lyon_path([kurbo::PathEl::LineTo(kurbo::Point::new(1.0, 1.0))], 1.0).is_some());
    }

    #[test]
    fn the_unit_square() {
        let m = MeshData::unit_square();
        assert_eq!((m.vertex_count(), m.indices.len()), (4, 6));
        assert_eq!(m.bytes(), 4 * VERTEX_BYTES as u64 + 24);
    }
}
