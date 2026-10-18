// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shape primitives: `line` (with end markers), `rect`, `ellipse`, `circle`, `arc`, `polygon`.
//!
//! See the mapping table of [the module above](super).
//!
//! # Line end markers
//!
//! `end1` sits at `(x1, y1)` and `end2` at `(x2, y2)`. For an end point `E` and the other end point `O`,
//! let `d = E − O` (integer nm), `|d| = isqrt_round(dx² + dy²)` ([`crate::decimal::isqrt_round`]), `L` the
//! marker length (`length1` / `length2` in px, default 1.5 px, converted), and define, all in `i128` with
//! [`crate::decimal::div_round_half_even`]:
//!
//! - `back(t) = E − round(d × t / |d|)` (the point `t` nm back along the line from `E`);
//! - `side(p, t) = p + round(n × t / |d|)` with `n = (−dy, dx)` (the point `t` nm to one side of `p`).
//!
//! | `endN` | Primitive appended after the line (same stroke as the line) |
//! |---|---|
//! | `none` (default) | nothing |
//! | `simple` | [`Primitive::Polyline`] `[side(back(L), L/2), E, side(back(L), −L/2)]` (an open arrow head) |
//! | `triangle` | [`Primitive::Polygon`] `[E, side(back(L), L/2), side(back(L), −L/2)]`, closed, filled with the line's colour |
//! | `circle` | [`Primitive::Circle`] centre `back(L/2)`, radius `L/2` (integer halving toward zero), filled with the line's colour |
//! | `diamond` | [`Primitive::Polygon`] `[E, side(back(L/2), L/2), back(L), side(back(L/2), −L/2)]`, closed, filled with the line's colour |
//! | anything else | nothing; reported (EW-IMP-004, info: "line end `<value>` not supported, drawn without marker") |
//!
//! "Filled with the line's colour" is the stroke's colour, or [`ew_schema::Colour::ByLayer`] when the line has
//! no stroke. The line itself is not shortened. A zero-length line (`|d| = 0`) gets no markers and no report.
//! `end1` markers come before `end2` markers.

use ew_schema::{
    ArcPrim, Bounds, CirclePrim, Colour, EllipsePrim, Extra, LinePrim, PolygonPrim, PolylinePrim, Primitive,
    RectPrim, Stroke,
};
use ew_units::{Nm, Pt2, Rect};
use roxmltree::Node;

use super::{ItemError, Parsed, PrimCtx};
use crate::decimal::{Decimal, DecimalError, div_round_half_even, isqrt_round};
use crate::style::{StyleSpec, parse_style};

/// Maps an exact-decimal arithmetic failure to an [`ItemError`].
fn ov<T>(r: Result<T, DecimalError>, what: &str) -> Result<T, ItemError> {
    r.map_err(|e| ItemError(format!("{what}: {e}")))
}

/// Parses the `style` attribute of `node`, reporting each unsupported pair and keeping it in `extra`.
fn style(node: Node<'_, '_>, ctx: &mut PrimCtx<'_>, extra: &mut Extra) -> StyleSpec {
    let s = parse_style(node.attribute("style").unwrap_or(""));
    for (k, v) in &s.unsupported {
        ctx.unsupported(node, format!("style {k}:{v} not supported"));
        extra.insert(format!("qet.style.{k}"), serde_json::Value::String(v.clone()));
    }
    s
}

/// The marker length attribute `name` in nm (default 1.5 px).
fn end_length(node: Node<'_, '_>, ctx: &PrimCtx<'_>, name: &str) -> Result<Nm, ItemError> {
    let d = ctx.decimal_or(node, name, Decimal::new(15, -1))?;
    ctx.nm(d, name)
}

/// `line`: a [`Primitive::Line`] plus its end markers.
///
/// # Errors
///
/// [`ItemError`] for a missing or unparsable coordinate or length.
pub fn line(node: Node<'_, '_>, ctx: &mut PrimCtx<'_>, out: &mut Parsed) -> Result<(), ItemError> {
    let from = ctx.point(node, "x1", "y1")?;
    let to = ctx.point(node, "x2", "y2")?;
    let length1 = end_length(node, ctx, "length1")?;
    let length2 = end_length(node, ctx, "length2")?;
    let mut extra = Extra::new();
    let stroke = style(node, ctx, &mut extra).stroke;
    let mut markers = Vec::new();
    for (attr, end, other, length) in [("end1", from, to, length1), ("end2", to, from, length2)] {
        let kind = node.attribute(attr).unwrap_or("none");
        if !matches!(kind, "none" | "simple" | "triangle" | "circle" | "diamond") {
            ctx.unsupported(
                node,
                format!("line end {kind} not supported, drawn without marker"),
            );
        }
        markers.extend(end_marker(kind, end, other, length, stroke.as_ref())?);
    }
    out.graphics.push(Primitive::Line(LinePrim {
        from,
        to,
        stroke,
        extra,
    }));
    out.graphics.extend(markers);
    Ok(())
}

/// `E - round(d × t / |d|)` or `p + round(n × t / |d|)` helper: the component `v × t / len`, rounded.
fn scaled(v: i128, t: i128, len: i128) -> i128 {
    div_round_half_even(v * t, len)
}

/// An `i128` point back to a [`Pt2`].
fn pt(x: i128, y: i128) -> Result<Pt2, ItemError> {
    let conv = |v: i128| {
        i64::try_from(v)
            .map(Nm)
            .map_err(|_| ItemError("marker coordinate out of range".to_owned()))
    };
    Ok(Pt2::new(conv(x)?, conv(y)?))
}

/// The marker primitive of end type `kind` at end point `end` of a line from `other`, by the table of the
/// module documentation; `Ok(None)` for `none`, a zero-length line, or an unsupported type (which the caller
/// reports).
///
/// # Errors
///
/// [`ItemError`] only for arithmetic overflow.
pub fn end_marker(
    kind: &str,
    end: Pt2,
    other: Pt2,
    length: Nm,
    stroke: Option<&Stroke>,
) -> Result<Option<Primitive>, ItemError> {
    if !matches!(kind, "simple" | "triangle" | "circle" | "diamond") {
        return Ok(None);
    }
    let (ex, ey) = (i128::from(end.x.0), i128::from(end.y.0));
    let (dx, dy) = (ex - i128::from(other.x.0), ey - i128::from(other.y.0));
    let len = isqrt_round(dx.unsigned_abs().pow(2) + dy.unsigned_abs().pow(2));
    let len = i128::try_from(len).map_err(|_| ItemError("line length out of range".to_owned()))?;
    if len == 0 {
        return Ok(None);
    }
    let l = i128::from(length.0);
    let half = l / 2;
    // back(t): the point t nm back along the line from E.
    let back = |t: i128| (ex - scaled(dx, t, len), ey - scaled(dy, t, len));
    // side(p, t): the point t nm to one side of p, along n = (-dy, dx).
    let side = |p: (i128, i128), t: i128| (p.0 + scaled(-dy, t, len), p.1 + scaled(dx, t, len));
    let colour = stroke.map_or(Colour::ByLayer, |s| s.colour.clone());
    let stroke = stroke.cloned();
    let e = (ex, ey);
    let marker = match kind {
        "simple" => {
            let b = back(l);
            Primitive::Polyline(PolylinePrim {
                points: vec![pt_of(side(b, half))?, pt_of(e)?, pt_of(side(b, -half))?],
                stroke,
                extra: Extra::new(),
            })
        }
        "triangle" => {
            let b = back(l);
            Primitive::Polygon(PolygonPrim {
                points: vec![pt_of(e)?, pt_of(side(b, half))?, pt_of(side(b, -half))?],
                closed: true,
                stroke,
                fill: Some(colour),
                extra: Extra::new(),
            })
        }
        "circle" => Primitive::Circle(CirclePrim {
            centre: pt_of(back(half))?,
            r: Nm(i64::try_from(half).map_err(|_| ItemError("marker radius out of range".to_owned()))?),
            stroke,
            fill: Some(colour),
            extra: Extra::new(),
        }),
        _ => {
            let m = back(half);
            Primitive::Polygon(PolygonPrim {
                points: vec![
                    pt_of(e)?,
                    pt_of(side(m, half))?,
                    pt_of(back(l))?,
                    pt_of(side(m, -half))?,
                ],
                closed: true,
                stroke,
                fill: Some(colour),
                extra: Extra::new(),
            })
        }
    };
    Ok(Some(marker))
}

/// A tuple point to a [`Pt2`].
fn pt_of(p: (i128, i128)) -> Result<Pt2, ItemError> {
    pt(p.0, p.1)
}

/// The normalised box over `(x, y)`–`(x + width, y + height)`.
fn bbox(node: Node<'_, '_>, ctx: &PrimCtx<'_>) -> Result<Bounds, ItemError> {
    let x = ctx.decimal(node, "x")?;
    let y = ctx.decimal(node, "y")?;
    let w = ctx.decimal(node, "width")?;
    let h = ctx.decimal(node, "height")?;
    let x2 = ov(x.checked_add(w), "x + width")?;
    let y2 = ov(y.checked_add(h), "y + height")?;
    let a = Pt2::new(ctx.rel_x(x)?, ctx.rel_y(y)?);
    let b = Pt2::new(ctx.rel_x(x2)?, ctx.rel_y(y2)?);
    Ok(Bounds(Rect::from_points(a, b)))
}

/// The absolute value of a decimal.
fn abs(d: Decimal) -> Result<Decimal, ItemError> {
    if d.is_negative() {
        ov(Decimal::ZERO.checked_sub(d), "abs")
    } else {
        Ok(d)
    }
}

/// `rect`.
///
/// # Errors
///
/// [`ItemError`] for a missing or unparsable coordinate or length.
pub fn rect(node: Node<'_, '_>, ctx: &mut PrimCtx<'_>, out: &mut Parsed) -> Result<(), ItemError> {
    let rect = bbox(node, ctx)?;
    let rx = abs(ctx.decimal_or(node, "rx", Decimal::ZERO)?)?;
    let ry = abs(ctx.decimal_or(node, "ry", Decimal::ZERO)?)?;
    let (rx, ry) = (ctx.nm(rx, "rx")?, ctx.nm(ry, "ry")?);
    let mut extra = Extra::new();
    let s = style(node, ctx, &mut extra);
    out.graphics.push(Primitive::Rect(RectPrim {
        rect,
        rx,
        ry,
        shadow: None,
        stroke: s.stroke,
        fill: s.fill,
        extra,
    }));
    Ok(())
}

/// `ellipse`.
///
/// # Errors
///
/// [`ItemError`] for a missing or unparsable coordinate or length.
pub fn ellipse(node: Node<'_, '_>, ctx: &mut PrimCtx<'_>, out: &mut Parsed) -> Result<(), ItemError> {
    let rect = bbox(node, ctx)?;
    let mut extra = Extra::new();
    let s = style(node, ctx, &mut extra);
    out.graphics.push(Primitive::Ellipse(EllipsePrim {
        rect,
        stroke: s.stroke,
        fill: s.fill,
        extra,
    }));
    Ok(())
}

/// `circle`.
///
/// # Errors
///
/// [`ItemError`] for a missing or unparsable coordinate or length.
pub fn circle(node: Node<'_, '_>, ctx: &mut PrimCtx<'_>, out: &mut Parsed) -> Result<(), ItemError> {
    let x = ctx.decimal(node, "x")?;
    let y = ctx.decimal(node, "y")?;
    let d = ctx.decimal(node, "diameter")?;
    let half = ov(d.half(), "diameter")?;
    let centre = Pt2::new(
        ctx.rel_x(ov(x.checked_add(half), "x + d/2")?)?,
        ctx.rel_y(ov(y.checked_add(half), "y + d/2")?)?,
    );
    let r = ctx.nm(abs(half)?, "diameter")?;
    let mut extra = Extra::new();
    let s = style(node, ctx, &mut extra);
    out.graphics.push(Primitive::Circle(CirclePrim {
        centre,
        r,
        stroke: s.stroke,
        fill: s.fill,
        extra,
    }));
    Ok(())
}

/// `arc`.
///
/// # Errors
///
/// [`ItemError`] for a missing or unparsable coordinate, length or angle.
pub fn arc(node: Node<'_, '_>, ctx: &mut PrimCtx<'_>, out: &mut Parsed) -> Result<(), ItemError> {
    let rect = bbox(node, ctx)?;
    let start = ov(ctx.decimal(node, "start")?.deg_to_angle(), "start")?;
    let sweep = ov(ctx.decimal(node, "angle")?.deg_to_angle(), "angle")?;
    let mut extra = Extra::new();
    let s = style(node, ctx, &mut extra);
    if s.fill.is_some() {
        ctx.unsupported(node, "arc filling not supported, drawn unfilled");
    }
    out.graphics.push(Primitive::Arc(ArcPrim {
        rect,
        start,
        sweep,
        stroke: s.stroke,
        extra,
    }));
    Ok(())
}

/// `polygon`.
///
/// # Errors
///
/// [`ItemError`] for an unparsable coordinate or fewer than two points.
pub fn polygon(node: Node<'_, '_>, ctx: &mut PrimCtx<'_>, out: &mut Parsed) -> Result<(), ItemError> {
    let mut points = Vec::new();
    for k in 1.. {
        let xname = format!("x{k}");
        if node.attribute(xname.as_str()).is_none() {
            break;
        }
        points.push(ctx.point(node, &xname, &format!("y{k}"))?);
    }
    if points.len() < 2 {
        return Err(ItemError(format!(
            "polygon has {} point(s), at least 2 needed",
            points.len()
        )));
    }
    let closed = node.attribute("closed") != Some("false");
    let mut extra = Extra::new();
    let s = style(node, ctx, &mut extra);
    out.graphics.push(if closed {
        Primitive::Polygon(PolygonPrim {
            points,
            closed: true,
            stroke: s.stroke,
            fill: s.fill,
            extra,
        })
    } else {
        Primitive::Polyline(PolylinePrim {
            points,
            stroke: s.stroke,
            extra,
        })
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use ew_interop_api::{Code, ReportEntry};
    use ew_schema::Dash;
    use ew_units::Grid;

    use super::*;

    type Parser = fn(Node<'_, '_>, &mut PrimCtx<'_>, &mut Parsed) -> Result<(), ItemError>;

    fn run(xml: &str, f: Parser) -> (Result<(), ItemError>, Parsed, Vec<ReportEntry>) {
        let doc = roxmltree::Document::parse(xml).unwrap();
        let mut ctx = PrimCtx::new("t.elmt", None, Decimal::ZERO, Decimal::ZERO);
        let mut out = Parsed::default();
        let r = f(doc.root_element(), &mut ctx, &mut out);
        (r, out, ctx.entries)
    }

    fn px(s: &str) -> Nm {
        Decimal::parse(s).unwrap().px_to_nm().unwrap()
    }

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    fn default_stroke() -> Option<Stroke> {
        StyleSpec::default().stroke
    }

    fn only_prim(out: &Parsed) -> &Primitive {
        assert_eq!(out.graphics.len(), 1, "{:?}", out.graphics);
        &out.graphics[0]
    }

    #[test]
    fn line_plain() {
        let (r, out, entries) = run(r#"<line x1="0" y1="-20" x2="10" y2="5.5"/>"#, line);
        r.unwrap();
        assert!(entries.is_empty());
        let Primitive::Line(l) = only_prim(&out) else {
            panic!()
        };
        assert_eq!(l.from, Pt2::new(Nm(0), Grid::QET_10PX.at(-2).unwrap()));
        assert_eq!(l.to, Pt2::new(px("10"), px("5.5")));
        assert_eq!(l.stroke, default_stroke());
        assert!(l.extra.is_empty());
    }

    #[test]
    fn line_exponent_coordinate() {
        let (r, out, _) = run(r#"<line x1="1.23457e+01" y1="0" x2="0" y2="0"/>"#, line);
        r.unwrap();
        let Primitive::Line(l) = only_prim(&out) else {
            panic!()
        };
        assert_eq!(l.from.x, px("12.3457"));
    }

    #[test]
    fn missing_attribute_is_item_error() {
        let (r, out, _) = run(r#"<line x1="0" y1="0" x2="1"/>"#, line);
        assert!(r.unwrap_err().0.contains("y2"));
        assert!(out.graphics.is_empty());
        let (r, ..) = run(r#"<line x1="abc" y1="0" x2="1" y2="1"/>"#, line);
        assert!(r.is_err());
    }

    /// A vertical line from (0, 0) to (0, 10 px) with `length2="3"` and the end type `kind` at end 2.
    fn vertical(kind: &str) -> (Parsed, Vec<ReportEntry>) {
        let xml = format!(r#"<line x1="0" y1="0" x2="0" y2="10" end2="{kind}" length2="3"/>"#);
        let (r, out, entries) = run(&xml, line);
        r.unwrap();
        (out, entries)
    }

    #[test]
    fn end_marker_simple() {
        let (n, l) = (px("10").0, px("3").0);
        let h = l / 2;
        let (out, entries) = vertical("simple");
        assert!(entries.is_empty());
        assert_eq!(out.graphics.len(), 2);
        let Primitive::Polyline(m) = &out.graphics[1] else {
            panic!()
        };
        assert_eq!(m.points, vec![p(-h, n - l), p(0, n), p(h, n - l)]);
        assert_eq!(m.stroke, default_stroke());
    }

    #[test]
    fn end_marker_triangle() {
        let (n, l) = (px("10").0, px("3").0);
        let h = l / 2;
        let (out, _) = vertical("triangle");
        let Primitive::Polygon(m) = &out.graphics[1] else {
            panic!()
        };
        assert_eq!(m.points, vec![p(0, n), p(-h, n - l), p(h, n - l)]);
        assert!(m.closed);
        assert_eq!(m.fill, Some(Colour::ByLayer));
        assert_eq!(m.stroke, default_stroke());
    }

    #[test]
    fn end_marker_circle() {
        let (n, l) = (px("10").0, px("3").0);
        let h = l / 2;
        let (out, _) = vertical("circle");
        let Primitive::Circle(m) = &out.graphics[1] else {
            panic!()
        };
        assert_eq!((m.centre, m.r), (p(0, n - h), Nm(h)));
        assert_eq!(m.fill, Some(Colour::ByLayer));
    }

    #[test]
    fn end_marker_diamond() {
        let (n, l) = (px("10").0, px("3").0);
        let h = l / 2;
        let (out, _) = vertical("diamond");
        let Primitive::Polygon(m) = &out.graphics[1] else {
            panic!()
        };
        assert_eq!(m.points, vec![p(0, n), p(-h, n - h), p(0, n - l), p(h, n - h)]);
        assert!(m.closed);
        assert_eq!(m.fill, Some(Colour::ByLayer));
    }

    #[test]
    fn end1_marker_comes_first_and_points_the_other_way() {
        let (n, l) = (px("10").0, px("3").0);
        let h = l / 2;
        let xml =
            r#"<line x1="0" y1="0" x2="0" y2="10" end1="simple" end2="circle" length1="3" length2="3"/>"#;
        let (r, out, _) = run(xml, line);
        r.unwrap();
        assert_eq!(out.graphics.len(), 3);
        let Primitive::Polyline(m1) = &out.graphics[1] else {
            panic!()
        };
        // E = (0, 0), d = (0, -n), normal (n, 0): side(back(L), t) = (t, L).
        assert_eq!(m1.points, vec![p(h, l), p(0, 0), p(-h, l)]);
        let Primitive::Circle(m2) = &out.graphics[2] else {
            panic!()
        };
        assert_eq!(m2.centre, p(0, n - h));
    }

    #[test]
    fn marker_fill_uses_stroke_colour() {
        let xml = r#"<line x1="0" y1="0" x2="0" y2="10" end2="triangle" style="color:red"/>"#;
        let (r, out, _) = run(xml, line);
        r.unwrap();
        let Primitive::Line(l) = &out.graphics[0] else {
            panic!()
        };
        let colour = l.stroke.as_ref().unwrap().colour.clone();
        assert_ne!(colour, Colour::ByLayer);
        let Primitive::Polygon(m) = &out.graphics[1] else {
            panic!()
        };
        assert_eq!(m.fill, Some(colour));
    }

    #[test]
    fn marker_without_stroke_fills_by_layer() {
        let m = end_marker("triangle", p(0, 100), p(0, 0), Nm(10), None)
            .unwrap()
            .unwrap();
        let Primitive::Polygon(m) = m else { panic!() };
        assert_eq!(m.fill, Some(Colour::ByLayer));
        assert_eq!(m.stroke, None);
    }

    #[test]
    fn diagonal_marker_uses_rounded_division() {
        let xml = r#"<line x1="0" y1="0" x2="30" y2="40" end2="triangle"/>"#;
        let (r, out, _) = run(xml, line);
        r.unwrap();
        let (dx, dy) = (i128::from(px("30").0), i128::from(px("40").0));
        assert_eq!((dx, dy), (7_937_500, 10_583_333));
        let len = isqrt_round((dx * dx + dy * dy) as u128) as i128;
        assert_eq!(len, 13_229_166);
        let l = i128::from(px("1.5").0);
        assert_eq!(l, 396_875);
        let bx = dx - div_round_half_even(dx * l, len);
        let by = dy - div_round_half_even(dy * l, len);
        let h = l / 2;
        let sx = |t: i128| bx + div_round_half_even(-dy * t, len);
        let sy = |t: i128| by + div_round_half_even(dx * t, len);
        let Primitive::Polygon(m) = &out.graphics[1] else {
            panic!()
        };
        let want = vec![
            p(dx as i64, dy as i64),
            p(sx(h) as i64, sy(h) as i64),
            p(sx(-h) as i64, sy(-h) as i64),
        ];
        assert_eq!(m.points, want);
        // 3-4-5: the base centre is 0.6 L and 0.8 L back from E.
        assert_eq!((bx, by), (7_937_500 - 238_125, 10_583_333 - 317_500));
    }

    #[test]
    fn unsupported_end_reported_and_line_kept() {
        let xml = r#"<line x1="0" y1="0" x2="0" y2="10" end1="none" end2="square"/>"#;
        let (r, out, entries) = run(xml, line);
        r.unwrap();
        assert_eq!(out.graphics.len(), 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].code, Code::DROPPED_BY_DESIGN);
        assert_eq!(
            entries[0].message,
            "line end square not supported, drawn without marker"
        );
    }

    #[test]
    fn zero_length_line_has_no_marker() {
        let xml = r#"<line x1="5" y1="5" x2="5" y2="5" end1="simple" end2="triangle"/>"#;
        let (r, out, entries) = run(xml, line);
        r.unwrap();
        assert_eq!(out.graphics.len(), 1);
        assert!(entries.is_empty());
    }

    #[test]
    fn rect_negative_width_normalised_with_radii() {
        let xml = r#"<rect x="10" y="20" width="-30" height="10" rx="2" ry="-3"/>"#;
        let (r, out, entries) = run(xml, rect);
        r.unwrap();
        assert!(entries.is_empty());
        let Primitive::Rect(rc) = only_prim(&out) else {
            panic!()
        };
        assert_eq!(
            rc.rect,
            Bounds(Rect::from_points(
                p(px("-20").0, px("20").0),
                p(px("10").0, px("30").0)
            ))
        );
        assert_eq!((rc.rx, rc.ry, rc.shadow), (px("2"), px("3"), None));
        assert_eq!(rc.fill, None);
    }

    #[test]
    fn rect_fill_and_dashed_style() {
        let xml = r#"<rect x="0" y="0" width="4" height="4" style="line-style:dashed;filling:black"/>"#;
        let (r, out, _) = run(xml, rect);
        r.unwrap();
        let Primitive::Rect(rc) = only_prim(&out) else {
            panic!()
        };
        assert_eq!(rc.fill, Some(Colour::ByLayer));
        assert_eq!(rc.stroke.as_ref().unwrap().dash, Dash::Dash42);
    }

    #[test]
    fn ellipse_box() {
        let (r, out, _) = run(r#"<ellipse x="-5" y="-2.5" width="10" height="5"/>"#, ellipse);
        r.unwrap();
        let Primitive::Ellipse(e) = only_prim(&out) else {
            panic!()
        };
        assert_eq!(
            e.rect,
            Bounds(Rect::from_points(
                p(px("-5").0, px("-2.5").0),
                p(px("5").0, px("2.5").0)
            ))
        );
    }

    #[test]
    fn circle_odd_diameter() {
        let (r, out, _) = run(r#"<circle x="-7.5" y="2" diameter="15"/>"#, circle);
        r.unwrap();
        let Primitive::Circle(c) = only_prim(&out) else {
            panic!()
        };
        assert_eq!(c.centre, Pt2::new(px("0"), px("9.5")));
        assert_eq!(c.r, px("7.5"));
        assert_eq!(c.r, Nm(1_984_375));
    }

    #[test]
    fn arc_keeps_angles_and_reports_fill() {
        let xml =
            r#"<arc x="0" y="0" width="10" height="20" start="90" angle="-180.5" style="filling:red"/>"#;
        let (r, out, entries) = run(xml, arc);
        r.unwrap();
        let Primitive::Arc(a) = only_prim(&out) else {
            panic!()
        };
        assert_eq!(
            (a.start, a.sweep),
            (ew_units::Angle2(90_000), ew_units::Angle2(-180_500))
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].code, Code::DROPPED_BY_DESIGN);
        let (r, _, entries) = run(
            r#"<arc x="0" y="0" width="1" height="1" start="0" angle="1"/>"#,
            arc,
        );
        r.unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn polygon_closed_by_default() {
        let xml = r#"<polygon x1="0" y1="0" x2="10" y2="0" x3="10" y3="10" style="filling:black"/>"#;
        let (r, out, _) = run(xml, polygon);
        r.unwrap();
        let Primitive::Polygon(g) = only_prim(&out) else {
            panic!()
        };
        assert!(g.closed);
        assert_eq!(
            g.points,
            vec![p(0, 0), p(px("10").0, 0), p(px("10").0, px("10").0)]
        );
        assert_eq!(g.fill, Some(Colour::ByLayer));
    }

    #[test]
    fn polygon_not_closed_is_polyline_without_fill() {
        let xml = r#"<polygon x1="0" y1="0" x2="10" y2="0" closed="false" style="filling:black"/>"#;
        let (r, out, entries) = run(xml, polygon);
        r.unwrap();
        assert!(entries.is_empty());
        let Primitive::Polyline(g) = only_prim(&out) else {
            panic!()
        };
        assert_eq!(g.points.len(), 2);
    }

    #[test]
    fn polygon_stops_at_first_missing_x_and_needs_two_points() {
        let (r, out, _) = run(r#"<polygon x1="0" y1="0"/>"#, polygon);
        assert!(r.is_err());
        assert!(out.graphics.is_empty());
        let (r, ..) = run(r#"<polygon x1="0" y1="0" x2="1"/>"#, polygon);
        assert!(r.unwrap_err().0.contains("y2"));
        let (r, out, _) = run(r#"<polygon x1="0" y1="0" x2="1" y2="1" x4="5" y4="5"/>"#, polygon);
        r.unwrap();
        let Primitive::Polygon(g) = only_prim(&out) else {
            panic!()
        };
        assert_eq!(g.points.len(), 2);
    }

    #[test]
    fn unsupported_style_key_reported_and_kept() {
        let xml = r#"<rect x="0" y="0" width="1" height="1" style="line-style:wavy;filling:none"/>"#;
        let (r, out, entries) = run(xml, rect);
        r.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].code, Code::DROPPED_BY_DESIGN);
        assert_eq!(entries[0].message, "style line-style:wavy not supported");
        let Primitive::Rect(rc) = only_prim(&out) else {
            panic!()
        };
        assert_eq!(
            rc.extra["qet.style.line-style"],
            serde_json::Value::String("wavy".to_owned())
        );
    }

    #[test]
    fn origin_is_subtracted() {
        let doc = roxmltree::Document::parse(r#"<line x1="10" y1="10" x2="20" y2="10"/>"#).unwrap();
        let mut ctx = PrimCtx::new("t.elmt", None, Decimal::from_int(10), Decimal::from_int(10));
        let mut out = Parsed::default();
        line(doc.root_element(), &mut ctx, &mut out).unwrap();
        let Primitive::Line(l) = &out.graphics[0] else {
            panic!()
        };
        assert_eq!(l.from, p(0, 0));
    }
}
