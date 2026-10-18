// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Properties of symbol lowering over random symbols in all eight orientation variants.

mod common;

use common::{at, cp, item_points, pt, slot, stroke, symbol};
use ew_mltext::MlText;
use ew_scene::symbol::lower;
use ew_schema::{
    Align, AnchorPolicy, Bounds, CirclePrim, Dir4, Extra, LinePrim, Primitive, RectPrim, SlotKind, SymbolDef,
    TextPrim,
};
use ew_units::{Angle2, Nm, Orient2, Pt2, Rect};
use proptest::prelude::*;

fn coord() -> impl Strategy<Value = i64> {
    -100_000_000i64..=100_000_000
}

fn point() -> impl Strategy<Value = Pt2> {
    (coord(), coord()).prop_map(|(x, y)| pt(x, y))
}

fn dir() -> impl Strategy<Value = Dir4> {
    prop_oneof![
        Just(Dir4::Up),
        Just(Dir4::Right),
        Just(Dir4::Down),
        Just(Dir4::Left)
    ]
}

fn primitive() -> impl Strategy<Value = Primitive> {
    prop_oneof![
        (point(), point()).prop_map(|(from, to)| Primitive::Line(LinePrim {
            from,
            to,
            stroke: Some(stroke()),
            extra: Extra::new(),
        })),
        (point(), point(), proptest::option::of(0i64..10_000_000)).prop_map(|(a, b, shadow)| {
            Primitive::Rect(RectPrim {
                rect: Bounds(Rect::from_points(a, b)),
                rx: Nm(0),
                ry: Nm(0),
                shadow: shadow.map(Nm),
                stroke: Some(stroke()),
                fill: None,
                extra: Extra::new(),
            })
        }),
        (point(), 0i64..10_000_000).prop_map(|(centre, r)| Primitive::Circle(CirclePrim {
            centre,
            r: Nm(r),
            stroke: Some(stroke()),
            fill: None,
            extra: Extra::new(),
        })),
    ]
}

fn anchor() -> impl Strategy<Value = AnchorPolicy> {
    prop_oneof![
        Just(AnchorPolicy::Origin),
        Just(AnchorPolicy::GraphicCentre),
        Just(AnchorPolicy::Slot(SlotKind::Tag)),
        point().prop_map(AnchorPolicy::Named),
    ]
}

/// A symbol with 1 to 6 CPs on the QET grid, a few primitives, texts and slots.
fn symbol_def() -> impl Strategy<Value = SymbolDef> {
    (
        proptest::collection::vec((-50i64..50, -50i64..50, dir(), 0u16..4), 1..=6),
        proptest::collection::vec(primitive(), 0..=4),
        proptest::collection::vec((point(), 0i32..360_000), 0..=2),
        proptest::collection::vec((point(), 0i32..360_000), 0..=2),
        anchor(),
    )
        .prop_map(|(cps, graphics, statics, slots, anchor)| {
            let cps = cps
                .into_iter()
                .enumerate()
                .map(|(i, (kx, ky, d, fn_cp))| cp(i as u16, fn_cp, Pt2::new(at(kx), at(ky)), d))
                .collect();
            let mut graphics = graphics;
            for (pos, angle) in statics {
                graphics.push(Primitive::Text(TextPrim {
                    pos,
                    text: MlText::neutral("T"),
                    style: None,
                    align: Align::MiddleLeft,
                    angle: Angle2(angle),
                    extra: Extra::new(),
                }));
            }
            let slots = slots
                .into_iter()
                .map(|(pos, rot)| slot(SlotKind::Tag, pos, Angle2(rot), Align::Centre))
                .collect();
            let mut def = symbol(graphics, cps, slots);
            def.anchor = anchor;
            def
        })
}

fn all_points(g: &ew_scene::symbol::SymbolGeometry) -> Vec<Pt2> {
    g.graphics.items.iter().flat_map(item_points).collect()
}

proptest! {
    #[test]
    fn on_grid_invariance(def in symbol_def()) {
        for v in Orient2::ALL {
            let g = lower(&def, v);
            for p in g.cps.iter().map(|c| c.pos).chain([g.anchor]) {
                for c in [p.x, p.y] {
                    prop_assert_eq!(g.grid.at(g.grid.snap(c)), Some(c), "{:?}", v);
                }
            }
        }
    }

    #[test]
    fn cp_indices_and_designations_survive(def in symbol_def()) {
        let table = ["a", "b", "c", "d"].map(String::from).to_vec();
        let designations = |items: Vec<u16>| {
            let mut d: Vec<&str> = items.into_iter().map(|f| table[usize::from(f)].as_str()).collect();
            d.sort_unstable();
            d
        };
        let want_idx: Vec<u16> = def.cps.iter().map(|c| c.idx).collect();
        let mut want_pairs: Vec<(u16, u16)> = def.cps.iter().map(|c| (c.idx, c.fn_cp)).collect();
        want_pairs.sort_unstable();
        let want_des = designations(def.cps.iter().map(|c| c.fn_cp).collect());
        for v in Orient2::ALL {
            let g = lower(&def, v);
            let idx: Vec<u16> = g.cps.iter().map(|c| c.idx).collect();
            prop_assert_eq!(&idx, &want_idx, "{:?}", v);
            let mut pairs: Vec<(u16, u16)> = g.cps.iter().map(|c| (c.idx, c.fn_cp)).collect();
            pairs.sort_unstable();
            prop_assert_eq!(&pairs, &want_pairs, "{:?}", v);
            prop_assert_eq!(designations(g.cps.iter().map(|c| c.fn_cp).collect()), want_des.clone(), "{:?}", v);
        }
    }

    #[test]
    fn bounds_contain_everything(def in symbol_def()) {
        for v in Orient2::ALL {
            let g = lower(&def, v);
            for p in g.cps.iter().map(|c| c.pos).chain(g.texts.iter().map(|t| t.pos)).chain(all_points(&g)) {
                prop_assert!(g.bounds.contains(p), "{:?}: {:?} outside {:?}", v, p, g.bounds);
            }
        }
    }

    #[test]
    fn deterministic(def in symbol_def()) {
        for v in Orient2::ALL {
            let (a, b) = (lower(&def, v), lower(&def, v));
            prop_assert_eq!(&a.graphics, &b.graphics);
            prop_assert_eq!(a, b);
        }
    }

    #[test]
    fn exact_under_the_variant(def in symbol_def()) {
        let base = lower(&def, Orient2::R0);
        let base_points = all_points(&base);
        for v in Orient2::ALL {
            let g = lower(&def, v);
            for (placed, original) in g.cps.iter().zip(&def.cps) {
                prop_assert_eq!(placed.pos, v.apply(original.pos), "{:?}", v);
            }
            for (placed, original) in g.texts.iter().zip(&base.texts) {
                prop_assert_eq!(placed.pos, v.apply(original.pos), "{:?}", v);
            }
            prop_assert_eq!(g.anchor, v.apply(base.anchor), "{:?}", v);
            let want: Vec<Pt2> = base_points.iter().map(|&p| v.apply(p)).collect();
            prop_assert_eq!(all_points(&g), want, "{:?}", v);
        }
    }

    #[test]
    fn text_is_readable(def in symbol_def()) {
        for v in Orient2::ALL {
            for t in lower(&def, v).texts {
                prop_assert!(
                    matches!(t.angle.0, 0..=90_000 | 270_001..=359_999),
                    "{:?}: {:?}", v, t.angle
                );
            }
        }
    }
}
