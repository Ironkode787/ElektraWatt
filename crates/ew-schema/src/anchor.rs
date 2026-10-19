// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The cross-reference anchor of a symbol in its own frame (FR-SYM-010, FR-XRF-005, D-14; doc 06 §5.1).
//!
//! One exact, integer definition shared by every layer that needs the anchor: the importers that place a
//! representation so that its anchor lands in a given cell, `ew-derive`'s `rep_anchor_cell` and the scene's
//! cross-reference anchor. It reads only the symbol definition, never rendered geometry.

use ew_units::{Nm, Pt2, Rect};

use crate::library::{AnchorPolicy, Primitive, SymbolDef};

impl SymbolDef {
    /// The anchor in symbol coordinates (unsnapped, before the representation's orientation and position):
    ///
    /// - `Origin` and `Unknown`: the origin.
    /// - `Named(p)`: `p`.
    /// - `Slot(kind)`: the position of the **first** text slot of that kind — or `slot_pos(index)` when that
    ///   returns a point (a representation's `SlotOverride.pos` for the slot index, in symbol coordinates);
    ///   the origin when the symbol has no slot of that kind.
    /// - `GraphicCentre`: [`SymbolDef::graphic_centre`].
    ///
    /// `None` only on arithmetic overflow.
    pub fn anchor_point_with(&self, slot_pos: impl Fn(u16) -> Option<Pt2>) -> Option<Pt2> {
        match &self.anchor {
            AnchorPolicy::Origin | AnchorPolicy::Unknown(_) => Some(Pt2::ORIGIN),
            AnchorPolicy::Named(p) => Some(*p),
            AnchorPolicy::Slot(kind) => {
                let Some((index, slot)) = self.text_slots.iter().enumerate().find(|(_, s)| s.kind == *kind)
                else {
                    return Some(Pt2::ORIGIN);
                };
                let moved = u16::try_from(index).ok().and_then(slot_pos);
                Some(moved.unwrap_or(slot.pos))
            }
            AnchorPolicy::GraphicCentre => self.graphic_centre(),
        }
    }

    /// The anchor in symbol coordinates without slot overrides: [`SymbolDef::anchor_point_with`] with none.
    pub fn anchor_point(&self) -> Option<Pt2> {
        self.anchor_point_with(|_| None)
    }

    /// The centre of the integer bounding box of the symbol's graphics, `(min + max) / 2` per axis with an
    /// exact tie (an odd sum) rounded **toward zero**; the origin for a symbol without graphics; `None` on
    /// overflow.
    ///
    /// The box holds line ends, polyline and polygon points, rectangle, ellipse, arc and image rectangles
    /// (their `Bounds` corners), circle centre ± radius and text positions. Stroke widths, Bézier
    /// approximations, text extents, connection points and slots do not count; unknown primitives are skipped.
    pub fn graphic_centre(&self) -> Option<Pt2> {
        let mut bb = Bb::default();
        for g in &self.graphics {
            match g {
                Primitive::Line(l) => {
                    bb.pt(l.from);
                    bb.pt(l.to);
                }
                Primitive::Polyline(p) => p.points.iter().for_each(|p| bb.pt(*p)),
                Primitive::Polygon(p) => p.points.iter().for_each(|p| bb.pt(*p)),
                Primitive::Rect(r) => bb.rect(r.rect.0),
                Primitive::Ellipse(e) => bb.rect(e.rect.0),
                Primitive::Arc(a) => bb.rect(a.rect.0),
                Primitive::Circle(c) => {
                    let (x, y, r) = (
                        i128::from(c.centre.x.0),
                        i128::from(c.centre.y.0),
                        i128::from(c.r.0),
                    );
                    bb.add(x - r, y - r);
                    bb.add(x + r, y + r);
                }
                Primitive::Text(t) => bb.pt(t.pos),
                Primitive::Image(i) => bb.rect(i.rect.0),
                Primitive::Unknown(_) => {}
            }
        }
        let Some([x0, y0, x1, y1]) = bb.0 else {
            return Some(Pt2::ORIGIN);
        };
        // i128 division truncates toward zero, which rounds the only possible fraction (a half) toward zero.
        let x = i64::try_from((x0 + x1) / 2).ok()?;
        let y = i64::try_from((y0 + y1) / 2).ok()?;
        Some(Pt2::new(Nm(x), Nm(y)))
    }
}

/// An integer bounding box `[min x, min y, max x, max y]`, grown point by point (i128: no overflow for any
/// `i64` coordinate plus or minus an `i64` radius).
#[derive(Default)]
struct Bb(Option<[i128; 4]>);

impl Bb {
    fn add(&mut self, x: i128, y: i128) {
        let b = self.0.get_or_insert([x, y, x, y]);
        b[0] = b[0].min(x);
        b[1] = b[1].min(y);
        b[2] = b[2].max(x);
        b[3] = b[3].max(y);
    }

    fn pt(&mut self, p: Pt2) {
        self.add(i128::from(p.x.0), i128::from(p.y.0));
    }

    fn rect(&mut self, r: Rect) {
        self.pt(r.min());
        self.pt(r.max());
    }
}

#[cfg(test)]
mod tests {
    use ew_units::Grid;

    use crate::library::{CirclePrim, LinePrim, SlotKind, TextSlot};

    use super::*;

    fn pt(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    fn symbol(anchor: AnchorPolicy, graphics: Vec<Primitive>) -> SymbolDef {
        let json = serde_json::json!({
            "fndef": {"library": "00000000-0000-0000-0000-000000000000",
                      "item": "00000000-0000-0000-0000-000000000000", "version_req": "*"},
            "grid": {"num": 7_937_500, "den": 3},
        });
        let mut s: SymbolDef = serde_json::from_value(json).expect("a minimal symbol");
        assert_eq!(s.grid, Grid::QET_10PX);
        s.anchor = anchor;
        s.graphics = graphics;
        s
    }

    fn line(a: Pt2, b: Pt2) -> Primitive {
        Primitive::Line(LinePrim {
            from: a,
            to: b,
            stroke: None,
            extra: Default::default(),
        })
    }

    #[test]
    fn origin_named_and_unknown() {
        let g = vec![line(pt(10, 10), pt(30, 50))];
        assert_eq!(
            symbol(AnchorPolicy::Origin, g.clone()).anchor_point(),
            Some(Pt2::ORIGIN)
        );
        assert_eq!(
            symbol(AnchorPolicy::Named(pt(7, -3)), g.clone()).anchor_point(),
            Some(pt(7, -3))
        );
        assert_eq!(
            symbol(AnchorPolicy::Unknown(serde_json::json!("future")), g).anchor_point(),
            Some(Pt2::ORIGIN)
        );
    }

    #[test]
    fn graphic_centre_is_the_integer_box_centre() {
        let s = symbol(AnchorPolicy::GraphicCentre, vec![line(pt(10, 10), pt(30, 50))]);
        assert_eq!(s.anchor_point(), Some(pt(20, 30)));
        // A circle counts with its radius.
        let c = Primitive::Circle(CirclePrim {
            centre: pt(100, 0),
            r: Nm(10),
            stroke: None,
            fill: None,
            extra: Default::default(),
        });
        let s = symbol(AnchorPolicy::GraphicCentre, vec![line(pt(0, 0), pt(0, 0)), c]);
        assert_eq!(s.graphic_centre(), Some(pt(55, 0)));
        // No graphics: the origin.
        assert_eq!(
            symbol(AnchorPolicy::GraphicCentre, Vec::new()).anchor_point(),
            Some(Pt2::ORIGIN)
        );
    }

    #[test]
    fn graphic_centre_ties_round_toward_zero() {
        // (0 + 5) / 2 = 2.5 → 2; (−5 + 0) / 2 = −2.5 → −2.
        let s = symbol(AnchorPolicy::GraphicCentre, vec![line(pt(0, -5), pt(5, 0))]);
        assert_eq!(s.anchor_point(), Some(pt(2, -2)));
    }

    #[test]
    fn graphic_centre_does_not_overflow() {
        let s = symbol(
            AnchorPolicy::GraphicCentre,
            vec![line(pt(i64::MAX, i64::MIN), pt(i64::MAX, i64::MIN))],
        );
        assert_eq!(s.anchor_point(), Some(pt(i64::MAX, i64::MIN)));
        let c = Primitive::Circle(CirclePrim {
            centre: pt(i64::MAX, 0),
            r: Nm(i64::MAX),
            stroke: None,
            fill: None,
            extra: Default::default(),
        });
        // Centre x = (0 + 2·MAX) / 2 = MAX: still representable.
        assert_eq!(
            symbol(AnchorPolicy::GraphicCentre, vec![c]).anchor_point(),
            Some(pt(i64::MAX, 0))
        );
    }

    #[test]
    fn slot_anchor_takes_the_first_slot_of_its_kind_and_overrides() {
        let mut s = symbol(AnchorPolicy::Slot(SlotKind::Tag), Vec::new());
        assert_eq!(s.anchor_point(), Some(Pt2::ORIGIN), "no slot of the kind");
        let slot = |kind: SlotKind, p: Pt2| -> TextSlot {
            serde_json::from_value(serde_json::json!({
                "kind": kind, "pos": p, "align": "top_left",
            }))
            .expect("a text slot")
        };
        s.text_slots = vec![
            slot(SlotKind::FunctionText, pt(1, 1)),
            slot(SlotKind::Tag, pt(40, -20)),
            slot(SlotKind::Tag, pt(99, 99)),
        ];
        assert_eq!(s.anchor_point(), Some(pt(40, -20)));
        assert_eq!(
            s.anchor_point_with(|i| (i == 1).then_some(pt(5, 5))),
            Some(pt(5, 5))
        );
        assert_eq!(
            s.anchor_point_with(|i| (i == 0).then_some(pt(5, 5))),
            Some(pt(40, -20))
        );
    }
}
