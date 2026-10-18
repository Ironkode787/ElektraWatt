// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Per-primitive lowering of symbol geometry at R0 and in R90, pens, colours, slots and anchors.

mod common;

use common::{at, cp, gp, pt, slot, stroke as schema_stroke, symbol};
use ew_dl::{
    Brush, Color, DashPattern, Item, ItemKind, Path, PathItem, Pen as DlPen, Stroke as DlStroke, Transform,
};
use ew_mltext::{Ident, MlText};
use ew_scene::symbol::{LAYER_COLOUR, TextSource, colour, lower, lower_primitive, resolve_anchor, stroke};
use ew_schema::{
    Align, AnchorPolicy, ArcPrim, Bounds, CirclePrim, Colour, Dash, EllipsePrim, Extra, ImagePrim, LinePrim,
    Pen, PolygonPrim, PolylinePrim, Primitive, RectPrim, Rgb, Sha256, SlotKind, Stroke, TextPrim,
};
use ew_units::{Angle2, Grid, Nm, Orient2, Pt2, Rect};

const MM: i64 = 1_000_000;

fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Rect {
    Rect::from_points(pt(x0, y0), pt(x1, y1))
}

fn path_item(item: &Item) -> &PathItem {
    match &item.kind {
        ItemKind::Path(p) => p,
        other => panic!("not a path item: {other:?}"),
    }
}

fn dl_stroke() -> DlStroke {
    DlStroke::pen(DlPen::P026)
}

fn rect_prim(
    r: Rect,
    rx: i64,
    ry: i64,
    shadow: Option<i64>,
    s: Option<Stroke>,
    fill: Option<Colour>,
) -> RectPrim {
    RectPrim {
        rect: Bounds(r),
        rx: Nm(rx),
        ry: Nm(ry),
        shadow: shadow.map(Nm),
        stroke: s,
        fill,
        extra: Extra::new(),
    }
}

fn text_prim(style: Option<&str>) -> TextPrim {
    TextPrim {
        pos: pt(10, 20),
        text: MlText::neutral("K"),
        style: style.map(|s| Ident::from_input(s).unwrap()),
        align: Align::MiddleLeft,
        angle: Angle2(0),
        extra: Extra::new(),
    }
}

#[test]
fn line_is_one_stroked_path() {
    let prim = Primitive::Line(LinePrim {
        from: pt(0, 0),
        to: pt(5 * MM, 7 * MM),
        stroke: Some(schema_stroke()),
        extra: Extra::new(),
    });
    let items = lower_primitive(&prim);
    assert_eq!(items.len(), 1);
    let p = path_item(&items[0]);
    assert_eq!(p.path, Path::line(pt(0, 0), pt(5 * MM, 7 * MM)));
    assert_eq!(p.stroke, Some(dl_stroke()));
    assert_eq!(p.fill, None);
}

#[test]
fn line_without_stroke_draws_nothing() {
    let prim = Primitive::Line(LinePrim {
        from: pt(0, 0),
        to: pt(5, 7),
        stroke: None,
        extra: Extra::new(),
    });
    assert!(lower_primitive(&prim).is_empty());
}

#[test]
fn polyline() {
    let pts = [pt(0, 0), pt(MM, 0), pt(MM, MM)];
    let prim = Primitive::Polyline(PolylinePrim {
        points: pts.to_vec(),
        stroke: Some(schema_stroke()),
        extra: Extra::new(),
    });
    let items = lower_primitive(&prim);
    assert_eq!(items.len(), 1);
    let p = path_item(&items[0]);
    assert_eq!(p.path, Path::polyline(&pts));
    assert_eq!(p.stroke, Some(dl_stroke()));
    assert_eq!(p.fill, None);
}

#[test]
fn rect_without_radii_is_plain_rect() {
    let r = rect(0, 0, 10 * MM, 6 * MM);
    let prim = Primitive::Rect(rect_prim(r, 0, 0, None, Some(schema_stroke()), None));
    let items = lower_primitive(&prim);
    assert_eq!(items.len(), 1);
    assert_eq!(path_item(&items[0]).path, Path::rect(r));
}

#[test]
fn rect_with_radii_is_rounded_rect() {
    let r = rect(0, 0, 10 * MM, 6 * MM);
    let prim = Primitive::Rect(rect_prim(r, MM, 2 * MM, None, Some(schema_stroke()), None));
    let items = lower_primitive(&prim);
    assert_eq!(items.len(), 1);
    assert_eq!(
        path_item(&items[0]).path,
        Path::rounded_rect(r, Nm(MM), Nm(2 * MM))
    );
}

#[test]
fn rect_fill_is_a_solid_brush() {
    let r = rect(0, 0, 10 * MM, 6 * MM);
    let white = Colour::Rgb(Rgb([255, 255, 255]));
    let prim = Primitive::Rect(rect_prim(
        r,
        0,
        0,
        None,
        Some(schema_stroke()),
        Some(white.clone()),
    ));
    let items = lower_primitive(&prim);
    assert_eq!(items.len(), 1);
    let p = path_item(&items[0]);
    assert_eq!(p.fill, Some(Brush::Solid(Color::from_rgb8(255, 255, 255))));
    assert_eq!(p.stroke, Some(dl_stroke()));
    // A fill alone also draws.
    let prim = Primitive::Rect(rect_prim(r, 0, 0, None, None, Some(white)));
    let items = lower_primitive(&prim);
    assert_eq!(items.len(), 1);
    assert_eq!(path_item(&items[0]).stroke, None);
    // Neither: nothing.
    let prim = Primitive::Rect(rect_prim(r, 0, 0, None, None, None));
    assert!(lower_primitive(&prim).is_empty());
}

#[test]
fn drop_shadow_of_a_square_box_is_the_l_shape() {
    let s = 2_610_000;
    let r = rect(0, 0, 10 * MM, 6 * MM);
    let prim = Primitive::Rect(rect_prim(r, 0, 0, Some(s), Some(schema_stroke()), None));
    let items = lower_primitive(&prim);
    assert_eq!(items.len(), 2);
    let sh = path_item(&items[0]);
    assert_eq!(sh.stroke, None);
    assert_eq!(sh.fill, Some(Brush::Solid(LAYER_COLOUR)));
    assert_eq!(sh.path.bounds(), Some(rect(s, s, 10 * MM + s, 6 * MM + s)));
    // The L-shape is not the whole offset box.
    assert_ne!(sh.path, Path::rect(rect(s, s, 10 * MM + s, 6 * MM + s)));
    let bx = path_item(&items[1]);
    assert_eq!(bx.path, Path::rect(r));
    assert_eq!(bx.stroke, Some(dl_stroke()));
}

#[test]
fn drop_shadow_uses_the_stroke_colour_then_the_fill_colour() {
    let r = rect(0, 0, 10 * MM, 6 * MM);
    let mut red = schema_stroke();
    red.colour = Colour::Rgb(Rgb([255, 0, 0]));
    let prim = Primitive::Rect(rect_prim(
        r,
        0,
        0,
        Some(MM),
        Some(red),
        Some(Colour::Rgb(Rgb([0, 0, 255]))),
    ));
    let items = lower_primitive(&prim);
    assert_eq!(
        path_item(&items[0]).fill,
        Some(Brush::Solid(Color::from_rgb8(255, 0, 0)))
    );
    // Without a stroke the fill colour is used.
    let prim = Primitive::Rect(rect_prim(
        r,
        0,
        0,
        Some(MM),
        None,
        Some(Colour::Rgb(Rgb([0, 0, 255]))),
    ));
    let items = lower_primitive(&prim);
    assert_eq!(items.len(), 2);
    assert_eq!(
        path_item(&items[0]).fill,
        Some(Brush::Solid(Color::from_rgb8(0, 0, 255)))
    );
}

#[test]
fn drop_shadow_of_a_rounded_box_is_the_offset_rounded_box() {
    let s = 2 * MM;
    let r = rect(0, 0, 10 * MM, 6 * MM);
    let prim = Primitive::Rect(rect_prim(r, MM, MM, Some(s), Some(schema_stroke()), None));
    let items = lower_primitive(&prim);
    assert_eq!(items.len(), 2);
    let sh = path_item(&items[0]);
    assert_eq!(
        sh.path,
        Path::rounded_rect(rect(s, s, 10 * MM + s, 6 * MM + s), Nm(MM), Nm(MM))
    );
    assert_eq!(sh.stroke, None);
    assert!(sh.fill.is_some());
}

#[test]
fn no_drop_shadow_for_zero_or_negative() {
    let r = rect(0, 0, 10 * MM, 6 * MM);
    for s in [0, -MM] {
        let prim = Primitive::Rect(rect_prim(r, 0, 0, Some(s), Some(schema_stroke()), None));
        assert_eq!(lower_primitive(&prim).len(), 1, "shadow {s}");
    }
}

#[test]
fn drop_shadow_larger_than_the_box_is_the_full_offset_box() {
    let s = 11 * MM;
    let r = rect(0, 0, 10 * MM, 6 * MM);
    let prim = Primitive::Rect(rect_prim(r, 0, 0, Some(s), Some(schema_stroke()), None));
    let items = lower_primitive(&prim);
    assert_eq!(items.len(), 2);
    assert_eq!(
        path_item(&items[0]).path,
        Path::rect(rect(s, s, 10 * MM + s, 6 * MM + s))
    );
}

#[test]
fn circle() {
    let prim = Primitive::Circle(CirclePrim {
        centre: pt(3 * MM, 4 * MM),
        r: Nm(2 * MM),
        stroke: Some(schema_stroke()),
        fill: None,
        extra: Extra::new(),
    });
    let items = lower_primitive(&prim);
    assert_eq!(items.len(), 1);
    let p = path_item(&items[0]);
    assert_eq!(p.path, Path::circle(pt(3 * MM, 4 * MM), Nm(2 * MM)));
    assert_eq!(p.stroke, Some(dl_stroke()));
}

fn ellipse_prim(r: Rect) -> Primitive {
    Primitive::Ellipse(EllipsePrim {
        rect: Bounds(r),
        stroke: Some(schema_stroke()),
        fill: None,
        extra: Extra::new(),
    })
}

#[test]
fn ellipse_over_an_even_box() {
    let items = lower_primitive(&ellipse_prim(rect(MM, 2 * MM, 11 * MM, 8 * MM)));
    assert_eq!(items.len(), 1);
    assert_eq!(
        path_item(&items[0]).path,
        Path::ellipse(pt(6 * MM, 5 * MM), Nm(5 * MM), Nm(3 * MM))
    );
}

#[test]
fn ellipse_over_an_odd_box_rounds_towards_the_minimum() {
    let items = lower_primitive(&ellipse_prim(rect(100, 200, 111, 207)));
    assert_eq!(items.len(), 1);
    // Width 11: centre min.x + 5, rx 5; height 7: centre min.y + 3, ry 3.
    assert_eq!(
        path_item(&items[0]).path,
        Path::ellipse(pt(105, 203), Nm(5), Nm(3))
    );
}

#[test]
fn arc_is_never_filled() {
    let r = rect(0, 0, 10 * MM, 6 * MM);
    let prim = Primitive::Arc(ArcPrim {
        rect: Bounds(r),
        start: Angle2(30_000),
        sweep: Angle2(120_000),
        stroke: Some(schema_stroke()),
        extra: Extra::new(),
    });
    let items = lower_primitive(&prim);
    assert_eq!(items.len(), 1);
    let p = path_item(&items[0]);
    assert_eq!(
        p.path,
        Path::arc(
            pt(5 * MM, 3 * MM),
            Nm(5 * MM),
            Nm(3 * MM),
            Angle2(30_000),
            Angle2(120_000)
        )
    );
    assert_eq!(p.fill, None);
    assert_eq!(p.stroke, Some(dl_stroke()));
}

#[test]
fn polygon_closed_and_open_keep_the_fill() {
    let pts = [pt(0, 0), pt(MM, 0), pt(MM, MM)];
    let fill = Colour::Rgb(Rgb([1, 2, 3]));
    for closed in [true, false] {
        let prim = Primitive::Polygon(PolygonPrim {
            points: pts.to_vec(),
            closed,
            stroke: Some(schema_stroke()),
            fill: Some(fill.clone()),
            extra: Extra::new(),
        });
        let items = lower_primitive(&prim);
        assert_eq!(items.len(), 1);
        let p = path_item(&items[0]);
        let want = if closed {
            Path::polygon(&pts)
        } else {
            Path::polyline(&pts)
        };
        assert_eq!(p.path, want, "closed {closed}");
        assert_eq!(p.fill, Some(Brush::Solid(Color::from_rgb8(1, 2, 3))));
    }
}

#[test]
fn text_is_a_placed_text_not_graphics() {
    assert!(lower_primitive(&Primitive::Text(text_prim(None))).is_empty());
    let def = symbol(
        vec![
            Primitive::Text(text_prim(None)),
            Primitive::Text(text_prim(Some("text.big"))),
        ],
        vec![],
        vec![],
    );
    let g = lower(&def, Orient2::R0);
    assert!(g.graphics.is_empty());
    assert_eq!(g.texts.len(), 2);
    let t = &g.texts[0];
    assert_eq!(t.source, TextSource::Static(MlText::neutral("K")));
    assert_eq!(t.style_key, "text.normal");
    assert!(t.visible);
    assert_eq!(t.max_width, None);
    assert_eq!(t.pos, pt(10, 20));
    assert_eq!(t.align, Align::MiddleLeft);
    assert_eq!(t.angle, Angle2(0));
    assert_eq!(g.texts[1].style_key, "text.big");
}

fn image_prim() -> (Primitive, Rect) {
    let r = rect(0, 0, 4 * MM, 3 * MM);
    (
        Primitive::Image(ImagePrim {
            rect: Bounds(r),
            asset: Sha256([7; 32]),
            extra: Extra::new(),
        }),
        r,
    )
}

#[test]
fn image_at_r0_and_r90() {
    let (prim, r) = image_prim();
    let items = lower_primitive(&prim);
    assert_eq!(items.len(), 1);
    let ItemKind::Image(img) = &items[0].kind else {
        panic!("image");
    };
    assert_eq!(img.image.sha256, [7; 32]);
    assert!(img.image.bytes.is_empty());
    assert_eq!(img.rect, r);

    let def = symbol(vec![prim.clone()], vec![], vec![]);
    let g0 = lower(&def, Orient2::R0);
    assert!(matches!(g0.graphics.items[0].kind, ItemKind::Image(_)));
    let g = lower(&def, Orient2::R90);
    assert_eq!(g.graphics.items.len(), 1);
    let ItemKind::Group(grp) = &g.graphics.items[0].kind else {
        panic!("group");
    };
    assert_eq!(grp.transform, Some(Transform::from_orient(Orient2::R90)));
    assert_eq!(grp.items.len(), 1);
    assert!(matches!(grp.items[0].kind, ItemKind::Image(_)));
    // The image corners enter the bounds under the variant: R90 maps (x, y) to (-y, x).
    assert_eq!(g.bounds, rect(-3 * MM, 0, 0, 4 * MM));
}

#[test]
fn unknown_primitive_draws_nothing() {
    let prim = Primitive::Unknown(serde_json::json!({"x": 1}));
    assert!(lower_primitive(&prim).is_empty());
}

#[test]
fn house_pens_map_by_name() {
    let pairs = [
        (Pen::P013, DlPen::P013),
        (Pen::P018, DlPen::P018),
        (Pen::P026, DlPen::P026),
        (Pen::P042, DlPen::P042),
        (Pen::P052, DlPen::P052),
        (Pen::P130, DlPen::P130),
    ];
    for (pen, want) in pairs {
        let s = Stroke {
            pen,
            ..schema_stroke()
        };
        assert_eq!(stroke(&s), DlStroke::pen(want));
    }
}

#[test]
fn explicit_width_pen() {
    let s = Stroke {
        pen: Pen::Width(Nm(300_000)),
        ..schema_stroke()
    };
    assert_eq!(stroke(&s), DlStroke::solid(Nm(300_000)));
}

#[test]
fn dashes_scale_with_the_pen() {
    let with = |dash| {
        stroke(&Stroke {
            dash,
            ..schema_stroke()
        })
    };
    let segs = |s: DlStroke| s.dash.expect("dashed").segments;
    assert_eq!(segs(with(Dash::Dash42)), [Nm(1_040_000), Nm(520_000)]);
    assert_eq!(segs(with(Dash::Dot12)), [Nm(260_000), Nm(520_000)]);
    assert_eq!(
        segs(with(Dash::DashDot)),
        [Nm(1_040_000), Nm(520_000), Nm(260_000), Nm(520_000)]
    );
    assert_eq!(with(Dash::Solid).dash, None);
    assert_eq!(
        with(Dash::Dash42),
        DlStroke::pen(DlPen::P026).dashed(DashPattern::Dash42)
    );
    // The pattern follows an explicit width.
    let wide = stroke(&Stroke {
        pen: Pen::Width(Nm(500_000)),
        dash: Dash::Dash42,
        ..schema_stroke()
    });
    assert_eq!(segs(wide), [Nm(2_000_000), Nm(1_000_000)]);
}

#[test]
fn colours() {
    assert_eq!(colour(&Colour::ByLayer), Color::BLACK);
    assert_eq!(colour(&Colour::ByLayer), LAYER_COLOUR);
    assert_eq!(
        colour(&Colour::Rgb(Rgb([12, 200, 255]))),
        Color::from_rgb8(12, 200, 255)
    );
}

#[test]
fn slot_texts() {
    let mut tag = slot(SlotKind::Tag, gp(1, 2), Angle2(0), Align::Centre);
    tag.max_width = Some(Nm(5 * MM));
    let mut hidden = slot(SlotKind::FunctionText, gp(0, 1), Angle2(0), Align::TopLeft);
    hidden.visible = false;
    let cpd = slot(
        SlotKind::CpDesignation(0),
        gp(3, 3),
        Angle2(0),
        Align::BottomRight,
    );
    let def = symbol(vec![], vec![], vec![tag, cpd, hidden]);
    let g = lower(&def, Orient2::R0);
    assert_eq!(g.texts.len(), 3);
    assert_eq!(g.texts[0].source, TextSource::Slot(SlotKind::Tag));
    assert_eq!(g.texts[0].style_key, "tag.main");
    assert_eq!(g.texts[0].max_width, Some(Nm(5 * MM)));
    assert!(g.texts[0].visible);
    assert_eq!(g.texts[0].pos, gp(1, 2));
    assert_eq!(g.texts[0].align, Align::Centre);
    assert_eq!(g.texts[1].style_key, "cp.small");
    assert_eq!(g.texts[2].style_key, "data.small");
    assert!(!g.texts[2].visible);
    assert_eq!(g.texts[2].max_width, None);
}

#[test]
fn statics_come_before_slots() {
    let def = symbol(
        vec![Primitive::Text(text_prim(None))],
        vec![],
        vec![slot(SlotKind::Tag, pt(0, 0), Angle2(0), Align::Centre)],
    );
    let g = lower(&def, Orient2::R0);
    assert!(matches!(g.texts[0].source, TextSource::Static(_)));
    assert!(matches!(g.texts[1].source, TextSource::Slot(_)));
}

fn box_prim(x1: Nm, y1: Nm) -> Primitive {
    Primitive::Rect(rect_prim(
        Rect::from_points(Pt2::ORIGIN, Pt2::new(x1, y1)),
        0,
        0,
        None,
        Some(schema_stroke()),
        None,
    ))
}

#[test]
fn anchor_origin_named_and_snapped() {
    let mut def = symbol(vec![], vec![], vec![]);
    assert_eq!(resolve_anchor(&def), Pt2::ORIGIN);
    assert_eq!(lower(&def, Orient2::R0).anchor, Pt2::ORIGIN);
    def.anchor = AnchorPolicy::Named(Pt2::new(Nm(2_645_834), Nm(-1)));
    assert_eq!(resolve_anchor(&def), Pt2::new(at(1), Nm(0)));
    assert_eq!(lower(&def, Orient2::R0).anchor, Pt2::new(at(1), Nm(0)));
    // The anchor is transformed by the variant: R90 maps (x, y) to (-y, x).
    assert_eq!(lower(&def, Orient2::R90).anchor, Pt2::new(Nm(0), at(1)));
}

#[test]
fn anchor_slot() {
    let mut def = symbol(
        vec![],
        vec![],
        vec![slot(
            SlotKind::Tag,
            Pt2::new(Nm(2_645_834), at(-2)),
            Angle2(0),
            Align::Centre,
        )],
    );
    def.anchor = AnchorPolicy::Slot(SlotKind::Tag);
    assert_eq!(resolve_anchor(&def), Pt2::new(at(1), at(-2)));
    def.anchor = AnchorPolicy::Slot(SlotKind::Xref);
    assert_eq!(resolve_anchor(&def), Pt2::ORIGIN);
}

#[test]
fn anchor_graphic_centre() {
    let mut def = symbol(vec![box_prim(at(2), at(4))], vec![], vec![]);
    def.anchor = AnchorPolicy::GraphicCentre;
    assert_eq!(resolve_anchor(&def), Pt2::new(at(1), at(2)));
    // Without graphics: the origin.
    let mut none = symbol(vec![], vec![cp(0, 0, gp(5, 5), ew_schema::Dir4::Up)], vec![]);
    none.anchor = AnchorPolicy::GraphicCentre;
    assert_eq!(resolve_anchor(&none), Pt2::ORIGIN);
}

#[test]
fn anchor_ties_snap_toward_zero() {
    let mut def = symbol(vec![], vec![], vec![]);
    def.grid = Grid::mm(4).unwrap();
    for (x, want) in [(2 * MM, 0), (-2 * MM, 0), (6 * MM, 4 * MM), (-6 * MM, -4 * MM)] {
        def.anchor = AnchorPolicy::Named(pt(x, 0));
        assert_eq!(resolve_anchor(&def), pt(want, 0), "x = {x}");
    }
}
