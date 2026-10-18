// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shaped glyph runs. `ew-text` (the shared shaping stack of ADR-0015 decision 4) produces them; this crate
//! only defines them, so every output draws the same glyphs at the same positions (FR-OUT-018).

use std::fmt;

use ew_units::{Nm, Pt2, Rect};

use crate::{Brush, Transform};

/// A font face, identified by the SHA-256 of the font file and the face index inside it.
///
/// Doc 09 §4.3 makes "font files by hash" part of the export input, so a key that does not depend on the
/// session's font interning order keeps exports reproducible. `Debug` prints the hash as hex.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontKey {
    /// SHA-256 of the font file's bytes.
    pub sha256: [u8; 32],
    /// Face index in a collection (`.ttc`); 0 for a single-face file.
    pub face_index: u16,
}

impl fmt::Debug for FontKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hex: String = self.sha256.iter().map(|b| format!("{b:02x}")).collect();
        f.debug_struct("FontKey")
            .field("sha256", &hex)
            .field("face_index", &self.face_index)
            .finish()
    }
}

/// One positioned glyph of a [`ShapedRun`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Glyph {
    /// Glyph id in the font.
    pub id: u32,
    /// Pen position of the glyph origin in run space, nm.
    pub x: Nm,
    /// Baseline offset of the glyph origin in run space, nm, y down.
    pub y: Nm,
}

/// A run of shaped glyphs in one font, size and brush.
///
/// **Coordinates.** Glyph positions and [`layout_box`](ShapedRun::layout_box) are in *run space*: nm, y
/// down, the baseline start at `(0, 0)`. A run-space point `q` lands at `origin + transform.apply(q)` in
/// the parent's coordinates: `transform` turns the run about its origin (a free text's [`Angle2`] or an
/// orientation; its translation is normally zero) and `origin` places it.
///
/// Glyphs are not mirrored by producers: doc 07 §4.1 keeps text readable under mirrored symbol variants,
/// which `ew-text` and `ew-scene` resolve before the IR.
///
/// [`Angle2`]: ew_units::Angle2
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ShapedRun {
    /// The font face.
    pub font: FontKey,
    /// Font size (em height) in nm.
    pub size: Nm,
    /// Where run space's origin, the start of the baseline, lands in the parent.
    pub origin: Pt2,
    /// Run space to the parent, about `origin`.
    pub transform: Transform,
    /// The glyphs in visual order.
    pub glyphs: Vec<Glyph>,
    /// The source text, for PDF text extraction with real spaces (FR-OUT-008, D-33).
    pub text: String,
    /// The fill of the glyphs.
    pub brush: Brush,
    /// The run's logical box in run space: x from the pen start to the advance end, y from −ascent to
    /// +descent of the font. Filled by `ew-text` from the font metrics; used for link annotation rectangles
    /// (doc 09 §4.2 "over the xref text's bounding box"), picking and bounds.
    pub layout_box: Rect,
}

impl ShapedRun {
    /// Maps a run-space point to the parent: `origin + transform.apply(q)`.
    pub fn to_parent(&self, q: Pt2) -> Pt2 {
        self.origin + self.transform.apply(q)
    }

    /// The corners of [`layout_box`](ShapedRun::layout_box) in the parent, in the order top-left,
    /// bottom-left, bottom-right, top-right of run space. For a rotated run these four points form the
    /// rotated rectangle a PDF link annotation's `QuadPoints` need.
    pub fn layout_quad(&self) -> [Pt2; 4] {
        let (a, b) = (self.layout_box.min(), self.layout_box.max());
        [a, Pt2::new(a.x, b.y), b, Pt2::new(b.x, a.y)].map(|q| self.to_parent(q))
    }

    /// The parent-space points that bound the run: the layout box corners and every glyph origin.
    pub fn bound_points(&self) -> impl Iterator<Item = Pt2> + '_ {
        self.layout_quad()
            .into_iter()
            .chain(self.glyphs.iter().map(|g| self.to_parent(Pt2::new(g.x, g.y))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_units::{Angle2, Orient2};

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    fn run(transform: Transform) -> ShapedRun {
        ShapedRun {
            font: FontKey {
                sha256: [7; 32],
                face_index: 0,
            },
            size: Nm(2_500_000),
            origin: p(10_000_000, 20_000_000),
            transform,
            glyphs: vec![
                Glyph {
                    id: 36,
                    x: Nm(0),
                    y: Nm(0),
                },
                Glyph {
                    id: 37,
                    x: Nm(1_500_000),
                    y: Nm(0),
                },
            ],
            text: "AB".into(),
            brush: Brush::BLACK,
            layout_box: Rect::from_points(p(0, -2_000_000), p(3_000_000, 500_000)),
        }
    }

    #[test]
    fn run_space_to_parent() {
        let r = run(Transform::IDENTITY);
        assert_eq!(r.to_parent(p(1, 2)), p(10_000_001, 20_000_002));
        assert_eq!(
            r.layout_quad(),
            [
                p(10_000_000, 18_000_000),
                p(10_000_000, 20_500_000),
                p(13_000_000, 20_500_000),
                p(13_000_000, 18_000_000)
            ]
        );
        // A vertical text reading bottom to top: +90° turns the baseline to point up the sheet.
        let v = run(Transform::from_angle(Angle2(90_000)));
        assert_eq!(v.to_parent(p(3_000_000, 0)), p(10_000_000, 17_000_000));
        assert_eq!(v.bound_points().count(), 6);
        // Orientation and angle compose like any transform.
        let o = run(Transform::from_orient(Orient2::R270));
        assert_eq!(o.to_parent(p(3_000_000, 0)), v.to_parent(p(3_000_000, 0)));
    }
}
