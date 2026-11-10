// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The frame a representation's slot texts are placed in: [`text_frame`].
//!
//! A slot text's position and angle (the symbol slot's, else the representation's `SlotOverride`) are points and
//! angles of this frame: the text sits at `origin + orient(pos)`, its angle is made readable and its alignment flips
//! under `orient` (`ew_scene::symbol::readable_angle`, `placed_align`). For every representation but one the frame is
//! the representation's own (`pos`, `orient`).
//!
//! **A half-turned black box.** A black box (a symbol without CPs on a representation that carries
//! `Representation.box`) in `Orient2::R180` (a half turn, no mirror) keeps its bounding box when it is turned
//! (`rep.transform` turns it about its centre), so its texts would otherwise land on the other side of the box. Its
//! slot texts are placed as on the same box unturned: the frame's origin is `2C − pos`, where `C` is the centre of the
//! box on the page (of its outline's points, else of its CPs', each at `pos + orient(p)`; `2C` is min + max per axis,
//! no rounding), and the frame has no turn. So a text keeps its place relative to the box through a half turn and
//! back, and an override stays in the unturned frame. A box with neither an outline nor CPs, or whose frame would
//! overflow, keeps the representation's frame.
//!
//! The readers that place a slot text all go through it: the page scene's slot texts (`ew-scene`), the tag anchor of
//! the page tables (`ew-api`) and a slot anchor ([`crate::anchor_point_of`]).

use ew_schema::{BoxSpec, Representation, SlotKind, SymbolDef};
use ew_units::{Nm, Orient2, Pt2};

/// The origin on the page and the orientation in which a representation's slot texts are placed (see the module
/// documentation).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextFrame {
    /// The page point of the frame's origin, in nm.
    pub origin: Pt2,
    /// The frame's orientation.
    pub orient: Orient2,
}

impl TextFrame {
    /// The page point of `local`, a point of the frame: `origin + orient(local)`; `None` on overflow.
    pub fn place(self, local: Pt2) -> Option<Pt2> {
        self.origin.checked_add(self.orient.checked_apply(local)?)
    }
}

/// The frame in which `rep`, drawn with `symbol` (its symbol copy's item), places its slot texts (see the module
/// documentation): the representation's own frame, except for a half-turned black box.
pub fn text_frame(rep: &Representation, symbol: &SymbolDef) -> TextFrame {
    let black_box = rep.box_.as_ref().filter(|_| symbol.cps.is_empty());
    frame_of(rep.pos, rep.orient, black_box)
}

/// Whether slot `slot` of `symbol` is shown on `rep`: the representation's `visible` override of the slot, else its
/// arrangement (the slot is shown when the named arrangement lists it; an arrangement the symbol does not define
/// counts as none), else the slot's own `visible` (FR-SYM-012). `false` for a slot the symbol does not have. The page
/// scene's slot texts and the strip tags of terminals drawn alone decide by it.
pub fn slot_shown(rep: &Representation, symbol: &SymbolDef, slot: u16) -> bool {
    let Some(def) = symbol.text_slots.get(usize::from(slot)) else {
        return false;
    };
    if let Some(v) = rep
        .text_slots
        .iter()
        .find(|o| o.slot == slot)
        .and_then(|o| o.visible)
    {
        return v;
    }
    match rep
        .arrangement
        .as_ref()
        .and_then(|name| symbol.arrangements.iter().find(|a| a.name == *name))
    {
        Some(a) => a.slots.contains(&slot),
        None => def.visible,
    }
}

/// The index of the first `tag` slot of `symbol` when [`slot_shown`] on `rep`; `None` when the symbol has no tag
/// slot or its first one is hidden (where a terminal drawn alone can carry its strip's tag).
pub fn tag_slot_shown(rep: &Representation, symbol: &SymbolDef) -> Option<u16> {
    let i = symbol
        .text_slots
        .iter()
        .position(|s| matches!(s.kind, SlotKind::Tag))?;
    let i = u16::try_from(i).ok()?;
    slot_shown(rep, symbol, i).then_some(i)
}

/// [`text_frame`] of a representation at `pos` in `orient`, with `black_box` its box when it is a black box.
fn frame_of(pos: Pt2, orient: Orient2, black_box: Option<&BoxSpec>) -> TextFrame {
    let own = TextFrame { origin: pos, orient };
    let Some(b) = black_box.filter(|_| orient == Orient2::R180) else {
        return own;
    };
    let local: Vec<Pt2> = if b.outline.is_empty() {
        b.cps.iter().map(|c| c.pos).collect()
    } else {
        b.outline.clone()
    };
    // The box's points turned (a turn that overflows keeps the representation's frame).
    let Some(turned) = local
        .iter()
        .map(|p| orient.checked_apply(*p))
        .collect::<Option<Vec<Pt2>>>()
    else {
        return own;
    };
    // min + max per axis of the page points: twice the box's centre `C`; the origin is `2C − pos`.
    let origin_axis = |at: Nm, axis: fn(Pt2) -> Nm| -> Option<Nm> {
        let page: Vec<i128> = turned
            .iter()
            .map(|p| i128::from(at.0) + i128::from(axis(*p).0))
            .collect();
        let twice_centre = page.iter().min()? + page.iter().max()?;
        i64::try_from(twice_centre - i128::from(at.0)).ok().map(Nm)
    };
    match (origin_axis(pos.x, |p| p.x), origin_axis(pos.y, |p| p.y)) {
        (Some(x), Some(y)) => TextFrame {
            origin: Pt2::new(x, y),
            orient: Orient2::R0,
        },
        _ => own,
    }
}

#[cfg(test)]
mod tests {
    use ew_schema::{BoxCp, Dir4};

    use super::*;

    const POS: Pt2 = Pt2::new(Nm(1_000), Nm(2_000));

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    fn boxed(outline: Vec<Pt2>, cps: Vec<Pt2>) -> BoxSpec {
        BoxSpec {
            outline,
            cps: cps
                .into_iter()
                .zip(0u16..)
                .map(|(pos, idx)| BoxCp {
                    idx,
                    pos,
                    dir: Dir4::Up,
                    extra: Default::default(),
                })
                .collect(),
            inner: Vec::new(),
            extra: Default::default(),
        }
    }

    #[test]
    fn a_half_turned_black_box_places_its_texts_as_unturned_over_the_same_box() {
        let outline = vec![p(0, 0), p(300, 0), p(300, 100), p(0, 100)];
        let b = boxed(outline.clone(), vec![]);
        let frame = frame_of(POS, Orient2::R180, Some(&b));
        // On the page the box covers (700, 1900) to (1000, 2000).
        assert_eq!(
            frame,
            TextFrame {
                origin: p(700, 1_900),
                orient: Orient2::R0
            }
        );
        // The outline unturned in the frame is the turned outline on the page.
        let mut turned: Vec<Pt2> = outline.iter().map(|q| POS + Orient2::R180.apply(*q)).collect();
        let mut framed: Vec<Pt2> = outline.iter().map(|q| frame.place(*q).unwrap()).collect();
        turned.sort();
        framed.sort();
        assert_eq!(turned, framed);
    }

    #[test]
    fn without_an_outline_the_box_cps_give_the_centre() {
        let b = boxed(vec![], vec![p(0, 20), p(60, 20)]);
        assert_eq!(
            frame_of(POS, Orient2::R180, Some(&b)),
            TextFrame {
                origin: p(940, 1_960),
                orient: Orient2::R0
            }
        );
    }

    #[test]
    fn every_other_representation_keeps_its_own_frame() {
        let b = boxed(vec![p(0, 0), p(300, 100)], vec![]);
        for orient in Orient2::ALL.into_iter().filter(|o| *o != Orient2::R180) {
            assert_eq!(
                frame_of(POS, orient, Some(&b)),
                TextFrame { origin: POS, orient },
                "{orient:?}"
            );
        }
        let own = TextFrame {
            origin: POS,
            orient: Orient2::R180,
        };
        // Not a black box (no box, or a symbol with CPs).
        assert_eq!(frame_of(POS, Orient2::R180, None), own);
        // A box without an outline or CPs.
        assert_eq!(frame_of(POS, Orient2::R180, Some(&boxed(vec![], vec![]))), own);
    }
}
