// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Snapshot of one synthetic symbol lowered in all eight orientation variants.

mod common;

use std::fmt::Write as _;

use common::{at, cp, gp, slot, stroke, symbol};
use ew_dl::{ItemKind, PathEl};
use ew_mltext::MlText;
use ew_scene::symbol::{SymbolGeometry, TextSource, lower};
use ew_schema::{Align, Bounds, Dir4, Extra, LinePrim, Primitive, RectPrim, SlotKind, SymbolDef, TextPrim};
use ew_units::{Angle2, Orient2, Pt2, Rect};

/// A 20 px × 40 px box centred at the origin, a lead to the top CP, two CPs, a tag slot and a static "K".
fn synthetic() -> SymbolDef {
    let body = Primitive::Rect(RectPrim {
        rect: Bounds(Rect::from_points(gp(-1, -2), gp(1, 2))),
        rx: ew_units::Nm(0),
        ry: ew_units::Nm(0),
        shadow: None,
        stroke: Some(stroke()),
        fill: None,
        extra: Extra::new(),
    });
    let lead = Primitive::Line(LinePrim {
        from: gp(0, -3),
        to: gp(0, -2),
        stroke: Some(stroke()),
        extra: Extra::new(),
    });
    let label = Primitive::Text(TextPrim {
        pos: Pt2::new(at(-2), ew_units::Nm(0)),
        text: MlText::neutral("K"),
        style: None,
        align: Align::MiddleLeft,
        angle: Angle2(0),
        extra: Extra::new(),
    });
    symbol(
        vec![body, lead, label],
        vec![
            cp(0, 0, Pt2::new(ew_units::Nm(0), at(-3)), Dir4::Up),
            cp(1, 1, Pt2::new(ew_units::Nm(0), at(3)), Dir4::Down),
        ],
        vec![slot(SlotKind::Tag, gp(2, -1), Angle2(0), Align::MiddleLeft)],
    )
}

fn p(p: Pt2) -> String {
    format!("({},{})", p.x.0, p.y.0)
}

fn render_variant(out: &mut String, v: Orient2, g: &SymbolGeometry) {
    writeln!(out, "{}", v.letter()).unwrap();
    for c in &g.cps {
        writeln!(out, "  cp {} {} {:?}", c.idx, p(c.pos), c.dir).unwrap();
    }
    for t in &g.texts {
        let source = match &t.source {
            TextSource::Static(m) => format!("static {:?}", m.as_neutral().unwrap_or("?")),
            TextSource::Slot(k) => format!("slot {k:?}"),
        };
        writeln!(out, "  text {source} {} {} {:?}", p(t.pos), t.angle.0, t.align).unwrap();
    }
    writeln!(out, "  anchor {}", p(g.anchor)).unwrap();
    writeln!(out, "  bounds {} {}", p(g.bounds.min()), p(g.bounds.max())).unwrap();
    for item in &g.graphics.items {
        let ItemKind::Path(path) = &item.kind else {
            panic!("only paths expected");
        };
        let els: Vec<String> = path
            .path
            .els
            .iter()
            .map(|e| match e {
                PathEl::MoveTo(a) => format!("M{}", p(*a)),
                PathEl::LineTo(a) => format!("L{}", p(*a)),
                PathEl::QuadTo(a, b) => format!("Q{}{}", p(*a), p(*b)),
                PathEl::CubicTo(a, b, c) => format!("C{}{}{}", p(*a), p(*b), p(*c)),
                PathEl::ClosePath => "Z".to_owned(),
            })
            .collect();
        writeln!(out, "  path {}", els.join(" ")).unwrap();
    }
}

#[test]
fn synthetic_symbol_in_all_variants() {
    let def = synthetic();
    let mut text = String::new();
    for v in Orient2::ALL {
        render_variant(&mut text, v, &lower(&def, v));
    }
    insta::assert_snapshot!(text, @r#"
        A
          cp 0 (0,-7937500) Up
          cp 1 (0,7937500) Down
          text static "K" (-5291667,0) 0 MiddleLeft
          text slot Tag (5291667,-2645833) 0 MiddleLeft
          anchor (0,0)
          bounds (-5291667,-7937500) (5291667,7937500)
          path M(-2645833,-5291667) L(-2645833,5291667) L(2645833,5291667) L(2645833,-5291667) Z
          path M(0,-7937500) L(0,-5291667)
        B
          cp 0 (7937500,0) Right
          cp 1 (-7937500,0) Left
          text static "K" (0,-5291667) 90000 MiddleRight
          text slot Tag (2645833,5291667) 90000 MiddleRight
          anchor (0,0)
          bounds (-7937500,-5291667) (7937500,5291667)
          path M(5291667,-2645833) L(-5291667,-2645833) L(-5291667,2645833) L(5291667,2645833) Z
          path M(7937500,0) L(5291667,0)
        C
          cp 0 (0,7937500) Down
          cp 1 (0,-7937500) Up
          text static "K" (5291667,0) 0 MiddleRight
          text slot Tag (-5291667,2645833) 0 MiddleRight
          anchor (0,0)
          bounds (-5291667,-7937500) (5291667,7937500)
          path M(2645833,5291667) L(2645833,-5291667) L(-2645833,-5291667) L(-2645833,5291667) Z
          path M(0,7937500) L(0,5291667)
        D
          cp 0 (-7937500,0) Left
          cp 1 (7937500,0) Right
          text static "K" (0,5291667) 90000 MiddleLeft
          text slot Tag (-2645833,-5291667) 90000 MiddleLeft
          anchor (0,0)
          bounds (-7937500,-5291667) (7937500,5291667)
          path M(-5291667,2645833) L(5291667,2645833) L(5291667,-2645833) L(-5291667,-2645833) Z
          path M(-7937500,0) L(-5291667,0)
        E
          cp 0 (0,-7937500) Up
          cp 1 (0,7937500) Down
          text static "K" (5291667,0) 0 MiddleRight
          text slot Tag (-5291667,-2645833) 0 MiddleRight
          anchor (0,0)
          bounds (-5291667,-7937500) (5291667,7937500)
          path M(2645833,-5291667) L(2645833,5291667) L(-2645833,5291667) L(-2645833,-5291667) Z
          path M(0,-7937500) L(0,-5291667)
        F
          cp 0 (7937500,0) Right
          cp 1 (-7937500,0) Left
          text static "K" (0,5291667) 90000 MiddleLeft
          text slot Tag (2645833,-5291667) 90000 MiddleLeft
          anchor (0,0)
          bounds (-7937500,-5291667) (7937500,5291667)
          path M(5291667,2645833) L(-5291667,2645833) L(-5291667,-2645833) L(5291667,-2645833) Z
          path M(7937500,0) L(5291667,0)
        G
          cp 0 (0,7937500) Down
          cp 1 (0,-7937500) Up
          text static "K" (-5291667,0) 0 MiddleLeft
          text slot Tag (5291667,2645833) 0 MiddleLeft
          anchor (0,0)
          bounds (-5291667,-7937500) (5291667,7937500)
          path M(-2645833,5291667) L(-2645833,-5291667) L(2645833,-5291667) L(2645833,5291667) Z
          path M(0,7937500) L(0,5291667)
        H
          cp 0 (-7937500,0) Left
          cp 1 (7937500,0) Right
          text static "K" (0,-5291667) 90000 MiddleRight
          text slot Tag (-2645833,5291667) 90000 MiddleRight
          anchor (0,0)
          bounds (-7937500,-5291667) (7937500,5291667)
          path M(-5291667,-2645833) L(5291667,-2645833) L(5291667,2645833) L(-5291667,2645833) Z
          path M(-7937500,0) L(-5291667,0)
    "#);
}
