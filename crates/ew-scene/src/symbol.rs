// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Symbol geometry lowering: [`SymbolDef`] to an `ew-dl` display list per orientation variant (doc 07 §2.3,
//! §3.2, §4.1; doc 06 §2.5; ADR-0015 decision 2).
//!
//! [`lower`] is pure and deterministic: the same definition and variant give an identical
//! [`SymbolGeometry`]. Everything is in the symbol's own frame (origin at the symbol origin, y down) with
//! the variant already applied; a placement adds only a translation.
//!
//! # Graphics
//!
//! Each primitive becomes one [`Item::path`] in definition order (a drop shadow adds one before its box);
//! texts become [`PlacedText`]s instead, since glyphs are shaped later by `ew-text`.
//!
//! | Primitive | Path |
//! |---|---|
//! | `Line` | [`Path::line`] |
//! | `Polyline` | [`Path::polyline`] |
//! | `Rect` | [`Path::rounded_rect`] (= [`Path::rect`] without radii); the shadow below |
//! | `Circle` | [`Path::circle`] |
//! | `Ellipse` | [`Path::ellipse`] about the box centre |
//! | `Arc` | [`Path::arc`] about the box centre ([`num::arc_to_cubics`](ew_dl::num::arc_to_cubics)), never filled |
//! | `Polygon` | [`Path::polygon`] if `closed`, else [`Path::polyline`] (a fill then closes implicitly) |
//! | `Text` | a [`PlacedText`] with [`TextSource::Static`] |
//! | `Image` | an [`Item::image`] over its rectangle, in a group carrying the variant (images turn and mirror with the symbol); its [`ImageData`] carries the asset's SHA-256 with format PNG and **empty bytes**, a placeholder until assets reach the scene, so renderers must resolve the bytes by hash |
//! | `Unknown` | nothing |
//!
//! A primitive with neither stroke nor fill draws nothing and is omitted.
//!
//! - **Box centres.** The centre of a box with an odd width or height in nm is rounded toward the box's
//!   minimum and the radius is half the size rounded toward zero, so the lowered ellipse may lie up to 1 nm
//!   inside the box.
//! - **Pens** ([`stroke`]): house pens map one to one to [`DlPen`]s; an explicit width is
//!   [`DlStroke::solid`]; an unknown pen counts as 0.26 mm. Dash patterns are scaled by the width
//!   ([`DashPattern::scaled`]). Butt caps and miter joins throughout. The stroke's colour becomes its
//!   [`Brush::Solid`] ([`DlStroke::with_brush`]).
//! - **Colours** ([`colour`]): `Rgb` maps one to one; `ByLayer` (and unknown colours) resolve to
//!   [`LAYER_COLOUR`], a placeholder black until layer colours exist. Strokes and fills are both
//!   [`Brush::Solid`].
//! - **Drop shadow** (FR-SYM-003): a `RectPrim.shadow` of `s > 0` adds a filled shape offset by `(s, s)`,
//!   **down and to the right** on the sheet, painted before the box, in the box's stroke colour (else its
//!   fill colour). For a box without corner radii and `s` below its width and height, the shape is only the
//!   visible L-shaped part (the offset box minus the box), so an unfilled box stays empty inside; otherwise
//!   it is the whole offset box, which the box itself paints over. `s ≤ 0` draws no shadow.
//!
//! # Variants
//!
//! The variant is [`Orient2::apply`], the integer matrix of doc 07 §4.1 (R90 is `(x, y) → (−y, x)` with y
//! down; M mirrors x before rotating). Every path point (Bézier control points included) maps exactly, so
//! an arc's sweep direction flips under a mirror automatically. CP positions map exactly and CP directions
//! map as directions ([`transform_dir`]); CP indices and their order never change (FR-SYM-004).
//!
//! # Texts
//!
//! Text positions transform, glyphs do not (doc 07 §3.2 rule 6, doc 06 §2.5), and turning or mirroring a symbol
//! never turns a text (owner, 2026-11-15: a text stays upright): a text keeps the angle it has in the symbol frame,
//! made readable by [`upright_angle`], in every variant. 90° is [`Angle2`] `90_000`, counter-clockwise on the
//! sheet: text reading bottom to top. Glyphs are **never mirrored**: a [`PlacedText`] carries a rotation only, so the
//! glyph run built from it has an unmirrored transform. The alignment is chosen instead ([`placed_align`]) so that
//! the text lies on the side of its anchor the variant maps its side in the symbol to; a connection point's
//! designation that a quarter turn brings centred above or below its point, beside a line that now leaves sideways,
//! runs along that line away from the body instead ([`slot_align`]). For a text without its own rotation:
//!
//! | Variant | A `R0` | B `R90` | C `R180` | D `R270` | E `M0` | F `M90` | G `M180` | H `M270` |
//! |---|---|---|---|---|---|---|---|---|
//! | Text angle | 0° | 0° | 0° | 0° | 0° | 0° | 0° | 0° |
//! | Middle right becomes | middle right | bottom centre | middle left | top centre | middle left | top centre | middle right | bottom centre |
//!
//! On a symbol turned a quarter the slot texts that print level are placed anew round the body instead
//! ([`slot_places`]): its tags centred above a body lying flat, its other texts centred below it, line under line.
//!
//! Slot texts come after static texts; a slot text's style key is the slot's own `style`, else none: it prints in the
//! binding of [`default_style_key`], which asks whether the symbol draws a terminal, a fact of its function definition
//! that the geometry does not hold. The page resolves it against the project's table ([`slot_style_key`], ADR-0032),
//! which a symbol's geometry does not depend on.
//!
//! # Anchor
//!
//! [`resolve_anchor`]: `Origin` → the origin; `Slot(kind)` → the first slot of that kind (the origin if
//! none); `GraphicCentre` → the centre of the graphics' bounds (the origin without graphics); `Named(p)` →
//! `p`; unknown → the origin. The point is snapped to `def.grid` per coordinate with ties toward the origin
//! ([`Grid::snap`]; doc 07 §3.2 rule 5) and then transformed; snapping commutes with the variant because the
//! grid is the same on both axes and `snap` is odd-symmetric.
//!
//! # Bounds
//!
//! [`SymbolGeometry::bounds`] is the smallest rectangle containing every path point (control points
//! included, stroke width excluded), every image corner, every CP and every text position; the origin alone
//! if there is none of these.

use std::sync::Arc;

use ew_dl::{
    Brush, Color, DashPattern, DisplayList, ImageData, ImageFormat, Item, Path, Pen as DlPen,
    Stroke as DlStroke, Transform,
};
use ew_mltext::MlText;
use ew_schema::{
    Align, AnchorPolicy, Colour, Dash, Dir4, Pen, Primitive, RectPrim, SlotKind, Stroke, SymbolDef, TextSlot,
};
use ew_text::TextStyleTable;
use ew_units::{Angle2, Grid, Nm, Orient2, Pt2, Rect};

/// The colour `Colour::ByLayer` resolves to until layer colours exist: black.
pub const LAYER_COLOUR: Color = Color::BLACK;

/// The style key of a static text without a style.
pub const DEFAULT_TEXT_STYLE: &str = "text.normal";

/// The geometry of one symbol in one orientation variant, in the symbol's frame.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SymbolGeometry {
    /// The graphics, painted in order.
    pub graphics: DisplayList,
    /// The connection points, in definition order.
    pub cps: Vec<CpPlacement>,
    /// Static texts (in graphics order), then slot texts (in slot order).
    pub texts: Vec<PlacedText>,
    /// The resolved cross-reference anchor, on `grid`.
    pub anchor: Pt2,
    /// The bounds (see the module documentation).
    pub bounds: Rect,
    /// The symbol's connection grid.
    pub grid: Grid,
}

/// A connection point placed in a variant.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CpPlacement {
    /// The symbol's CP index.
    pub idx: u16,
    /// The FnDef CP it draws.
    pub fn_cp: u16,
    /// Position.
    pub pos: Pt2,
    /// Direction a wire leaves the CP.
    pub dir: Dir4,
}

/// What a placed text prints.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TextSource {
    /// A text slot, resolved from the function's data when the page is built.
    Slot(SlotKind),
    /// A static symbol text.
    Static(MlText),
}

/// A text placed in a variant: an anchor point, its upright angle and the layout parameters `ew-text` needs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlacedText {
    /// What it prints.
    pub source: TextSource,
    /// The anchor point.
    pub pos: Pt2,
    /// The angle, counter-clockwise on the sheet, in `[0°, 90°] ∪ (270°, 360°)`: the symbol frame's, whatever the
    /// variant ([`upright_angle`]).
    pub angle: Angle2,
    /// Alignment at `pos` after the variant ([`placed_align`]), so the text keeps its side of the anchor.
    pub align: Align,
    /// The text style key: a static text's (its own, else the default style), a slot's own; `None` for a slot that
    /// names none, which prints in its kind's style ([`default_style_key`]).
    pub style_key: Option<String>,
    /// Maximum width before shrinking.
    pub max_width: Option<Nm>,
    /// Visible by default.
    pub visible: bool,
}

/// Lowers `def` in `variant`.
pub fn lower(def: &SymbolDef, variant: Orient2) -> SymbolGeometry {
    let t = Transform::from_orient(variant);
    let bus = ew_derive::is_bus_symbol(def);
    let mut graphics = DisplayList::new();
    let mut texts = Vec::new();
    let mut points: Vec<Pt2> = Vec::new();
    for prim in &def.graphics {
        for item in lower_primitive(prim) {
            let item = place_item(item, variant);
            points.extend(item_points(&item));
            graphics.push(item);
        }
        if let Primitive::Text(text) = prim {
            let (angle, align) = if bus {
                turned_with_body(&text.align, text.angle, variant)
            } else {
                (
                    upright_angle(text.angle),
                    placed_align(&text.align, text.angle, variant),
                )
            };
            texts.push(PlacedText {
                source: TextSource::Static(text.text.clone()),
                pos: variant.apply(text.pos),
                angle,
                align,
                style_key: Some(
                    text.style
                        .as_ref()
                        .map_or_else(|| DEFAULT_TEXT_STYLE.to_owned(), |s| s.as_str().to_owned()),
                ),
                max_width: None,
                visible: true,
            });
        }
    }
    let places = slot_places(def, variant, |i| def.text_slots[i].visible);
    for (slot, (pos, align)) in def.text_slots.iter().zip(places) {
        texts.push(PlacedText {
            source: TextSource::Slot(slot.kind.clone()),
            pos,
            angle: slot_angle(def, slot.rot, variant),
            align,
            style_key: slot.style.as_ref().map(|s| s.as_str().to_owned()),
            max_width: slot.max_width,
            visible: slot.visible,
        });
    }
    let cps: Vec<CpPlacement> = def
        .cps
        .iter()
        .map(|cp| CpPlacement {
            idx: cp.idx,
            fn_cp: cp.fn_cp,
            pos: variant.apply(cp.pos),
            dir: transform_dir(&cp.dir, variant),
        })
        .collect();
    points.extend(cps.iter().map(|cp| cp.pos));
    points.extend(texts.iter().map(|text| text.pos));
    let bounds = points
        .into_iter()
        .map(Rect::from_point)
        .reduce(Rect::union)
        .unwrap_or(Rect::from_point(Pt2::ORIGIN));
    SymbolGeometry {
        graphics,
        cps,
        texts,
        anchor: t.apply(resolve_anchor(def)),
        bounds,
        grid: def.grid,
    }
}

/// The items of one primitive in the symbol frame (R0), before the variant: zero, one or (a box with a
/// drop shadow) two items; texts give none.
///
/// An `Image` lowers to an item whose [`ImageData`] carries the asset's SHA-256 with format PNG and **empty
/// bytes**: a placeholder until assets reach the scene, so renderers must resolve the bytes by hash.
pub fn lower_primitive(prim: &Primitive) -> Vec<Item> {
    let fill_of = |c: &Option<Colour>| c.as_ref().map(|c| Brush::Solid(colour(c)));
    let item = |path: Path, s: &Option<Stroke>, fill: Option<Brush>| {
        let st = s.as_ref().map(stroke);
        if st.is_none() && fill.is_none() {
            Vec::new()
        } else {
            vec![Item::path(path, st, fill)]
        }
    };
    match prim {
        Primitive::Line(p) => item(Path::line(p.from, p.to), &p.stroke, None),
        Primitive::Polyline(p) => item(Path::polyline(&p.points), &p.stroke, None),
        Primitive::Rect(p) => {
            let r: Rect = p.rect.0;
            let mut items = shadow(p).into_iter().collect::<Vec<_>>();
            items.extend(item(
                Path::rounded_rect(r, p.rx, p.ry),
                &p.stroke,
                fill_of(&p.fill),
            ));
            items
        }
        Primitive::Circle(p) => item(Path::circle(p.centre, p.r), &p.stroke, fill_of(&p.fill)),
        Primitive::Ellipse(p) => {
            let (c, rx, ry) = centre_radii(p.rect.0);
            item(Path::ellipse(c, rx, ry), &p.stroke, fill_of(&p.fill))
        }
        Primitive::Arc(p) => {
            let (c, rx, ry) = centre_radii(p.rect.0);
            item(Path::arc(c, rx, ry, p.start, p.sweep), &p.stroke, None)
        }
        Primitive::Polygon(p) => {
            let path = if p.closed {
                Path::polygon(&p.points)
            } else {
                Path::polyline(&p.points)
            };
            item(path, &p.stroke, fill_of(&p.fill))
        }
        Primitive::Image(p) => {
            let data = ImageData {
                sha256: p.asset.0,
                format: ImageFormat::Png,
                bytes: Arc::from(Vec::new()),
            };
            vec![Item::image(Arc::new(data), p.rect.0)]
        }
        Primitive::Text(_) | Primitive::Unknown(_) => Vec::new(),
    }
}

/// The centre and radii of an axis-aligned box: centre `min + size / 2`, radii `size / 2`, halving toward
/// zero.
fn centre_radii(r: Rect) -> (Pt2, Nm, Nm) {
    let (rx, ry) = (Nm(r.width().0 / 2), Nm(r.height().0 / 2));
    (Pt2::new(r.min().x + rx, r.min().y + ry), rx, ry)
}

/// The drop-shadow item of a box, if it has one (see the module documentation).
fn shadow(p: &RectPrim) -> Option<Item> {
    let s = p.shadow.filter(|s| s.0 > 0)?;
    let colour_src = p
        .stroke
        .as_ref()
        .map(|st| &st.colour)
        .or(p.fill.as_ref())
        .unwrap_or(&Colour::ByLayer);
    let brush = Brush::Solid(colour(colour_src));
    let r: Rect = p.rect.0;
    let (x0, y0, x1, y1) = (r.min().x, r.min().y, r.max().x, r.max().y);
    let square = p.rx.0 <= 0 || p.ry.0 <= 0;
    let path = if square && s < r.width() && s < r.height() {
        let pt = Pt2::new;
        Path::polygon(&[
            pt(x1, y0 + s),
            pt(x1 + s, y0 + s),
            pt(x1 + s, y1 + s),
            pt(x0 + s, y1 + s),
            pt(x0 + s, y1),
            pt(x1, y1),
        ])
    } else {
        let off = Rect::from_points(Pt2::new(x0 + s, y0 + s), Pt2::new(x1 + s, y1 + s));
        Path::rounded_rect(off, p.rx, p.ry)
    };
    Some(Item::path(path, None, Some(brush)))
}

/// An item placed in `variant`: paths map point by point, images go into a group with the variant.
fn place_item(item: Item, variant: Orient2) -> Item {
    if variant == Orient2::R0 {
        return item;
    }
    let t = Transform::from_orient(variant);
    match item.kind {
        ew_dl::ItemKind::Path(p) => Item::path(p.path.transformed(t), p.stroke, p.fill),
        kind => Item::group(Some(t), None, vec![Item::new(kind)]),
    }
}

/// The points of an item for the bounds: path points, or the four corners of grouped images under the
/// group's transform.
fn item_points(item: &Item) -> Vec<Pt2> {
    match &item.kind {
        ew_dl::ItemKind::Path(p) => p.path.points().collect(),
        ew_dl::ItemKind::Image(img) => corners(img.rect).to_vec(),
        ew_dl::ItemKind::Group(g) => {
            let t = g.transform.unwrap_or(Transform::IDENTITY);
            g.items.iter().flat_map(item_points).map(|p| t.apply(p)).collect()
        }
        _ => Vec::new(),
    }
}

fn corners(r: Rect) -> [Pt2; 4] {
    let (a, b) = (r.min(), r.max());
    [a, Pt2::new(b.x, a.y), b, Pt2::new(a.x, b.y)]
}

/// An `ew-schema` stroke as an `ew-dl` stroke (see the module documentation).
pub fn stroke(s: &Stroke) -> DlStroke {
    let base = match &s.pen {
        Pen::P013 => DlStroke::pen(DlPen::P013),
        Pen::P018 => DlStroke::pen(DlPen::P018),
        Pen::P026 => DlStroke::pen(DlPen::P026),
        Pen::P042 => DlStroke::pen(DlPen::P042),
        Pen::P052 => DlStroke::pen(DlPen::P052),
        Pen::P130 => DlStroke::pen(DlPen::P130),
        Pen::Width(w) => DlStroke::solid(*w),
        Pen::Unknown(_) => DlStroke::pen(DlPen::P026),
    };
    let base = base.with_brush(Brush::Solid(colour(&s.colour)));
    match &s.dash {
        Dash::Dash42 => base.dashed(DashPattern::Dash42),
        Dash::Dot12 => base.dashed(DashPattern::Dot12),
        Dash::DashDot => base.dashed(DashPattern::DashDot),
        Dash::Solid | Dash::Unknown(_) => base,
    }
}

/// A schema colour as a concrete colour: `ByLayer` and unknown colours are [`LAYER_COLOUR`].
pub fn colour(c: &Colour) -> Color {
    match c {
        Colour::Rgb(rgb) => Color::from_rgb8(rgb.0[0], rgb.0[1], rgb.0[2]),
        Colour::ByLayer | Colour::Unknown(_) => LAYER_COLOUR,
    }
}

/// `d` turned by `variant` as a direction; an unknown direction is kept.
pub fn transform_dir(d: &Dir4, variant: Orient2) -> Dir4 {
    let v = match d {
        Dir4::Up => (0, -1),
        Dir4::Right => (1, 0),
        Dir4::Down => (0, 1),
        Dir4::Left => (-1, 0),
        Dir4::Unknown(_) => return d.clone(),
    };
    let p = variant.apply(Pt2::new(Nm(v.0), Nm(v.1)));
    match (p.x.0, p.y.0) {
        (0, -1) => Dir4::Up,
        (1, 0) => Dir4::Right,
        (0, 1) => Dir4::Down,
        _ => Dir4::Left,
    }
}

/// The angle a text drawn at `angle` in the symbol frame prints at, on any placement of its symbol: turning or
/// mirroring a placement never turns a text (owner, 2026-11-15), so it is `angle` itself, made readable (a direction
/// in `(90°, 270°]`, which would read upside down or top to bottom, is turned by 180°). The result is normalised to
/// `[0°, 360°)` and lies in `[0°, 90°] ∪ (270°, 360°)`.
///
/// ```
/// use ew_scene::symbol::upright_angle;
/// use ew_units::Angle2;
///
/// assert_eq!(upright_angle(Angle2(0)), Angle2(0));
/// assert_eq!(upright_angle(Angle2(90_000)), Angle2(90_000));
/// assert_eq!(upright_angle(Angle2(180_000)), Angle2(0));
/// assert_eq!(upright_angle(Angle2(-30_000)), Angle2(330_000));
/// ```
pub fn upright_angle(angle: Angle2) -> Angle2 {
    readable(angle).0
}

/// The angle a slot of `def` drawn at `angle` in the symbol frame prints at on a symbol in `variant`: [`upright_angle`],
/// except on a bus symbol, whose texts turn with its body ([`turned_with_body`]).
pub fn slot_angle(def: &SymbolDef, angle: Angle2, variant: Orient2) -> Angle2 {
    if ew_derive::is_bus_symbol(def) {
        turned_with_body(&Align::TopLeft, angle, variant).0
    } else {
        upright_angle(angle)
    }
}

/// Where a text of a bus symbol (doc 10 §2.7), drawn at `angle` and aligned `align` in the symbol frame, prints on the
/// symbol in `variant`: its texts stand inside its body, so they turn with it, still reading from the bottom or the
/// right. Its angle is its line's once the variant has turned it, made readable ([`upright_angle`]); its alignment keeps
/// its box on the side of its anchor that the variant maps it to, read in the printed text's frame. An alignment of a
/// newer format is kept, the text then printed as [`placed_align`] prints it.
pub fn turned_with_body(align: &Align, angle: Angle2, variant: Orient2) -> (Angle2, Align) {
    let Some((h, v)) = align_parts(align) else {
        return (upright_angle(angle), align.clone());
    };
    const R: i64 = 1_000_000;
    let base = |a: Angle2| ew_dl::num::arc_point(Pt2::ORIGIN, Nm(R), Nm(R), a);
    let down = |b: Pt2| Pt2::new(Nm(-b.y.0), b.x);
    let (upright, turned) = readable(angle);
    let (b0, d0) = (base(upright), down(base(upright)));
    // Into the box, in the symbol frame, then on the sheet.
    let (dx, dy) = if turned { (h, v) } else { (-h, -v) };
    // Every value is at most 2·R, so the sheet vector fits `i64`.
    let into = |a: Nm, b: Nm| Nm((dx * i128::from(a.0) + dy * i128::from(b.0)) as i64);
    let moved = variant.apply(Pt2::new(into(b0.x, d0.x), into(b0.y, d0.y)));
    // The printed line: of the quarter turns of the text's angle and their mirror images, the one along the turned line.
    let line = variant.apply(b0);
    let near = |p: Pt2| (p.x.0 - line.x.0).abs() <= 2 && (p.y.0 - line.y.0).abs() <= 2;
    let Some(printed) = (0..4)
        .flat_map(|k| [upright.0 + k * 90_000, 180_000 - upright.0 + k * 90_000])
        .map(|a| upright_angle(Angle2(a)))
        .find(|a| near(base(*a)) || near(Pt2::new(Nm(-base(*a).x.0), Nm(-base(*a).y.0))))
    else {
        return (upright_angle(angle), placed_align(align, angle, variant));
    };
    let (bp, dp) = (base(printed), down(base(printed)));
    let dot = |a: Pt2| i128::from(moved.x.0) * i128::from(a.x.0) + i128::from(moved.y.0) * i128::from(a.y.0);
    let side = |c: i128| {
        if 2 * c.abs() < i128::from(R) * i128::from(R) {
            0
        } else {
            c.signum()
        }
    };
    (printed, align_of(-side(dot(bp)), -side(dot(dp))))
}

/// The readable angle of `angle` and whether the readability turn of 180° was applied to reach it.
fn readable(angle: Angle2) -> (Angle2, bool) {
    let a = i64::from(angle.normalized().0);
    let turned = a > 90_000 && a <= 270_000;
    let a = if turned { a - 180_000 } else { a };
    (Angle2(a.rem_euclid(360_000) as i32), turned)
}

/// The alignment of a text drawn at `angle` and aligned `align` in the symbol frame, on a symbol in `variant`. The
/// text prints at [`upright_angle`]; its alignment is chosen so that the text lies on the side of its anchor that the
/// variant maps its side in the symbol to: the direction from the anchor into the text's box (the opposite of the
/// alignment: a text aligned middle right lies left of its anchor) is mapped by the variant and read back in the
/// text's own frame, and a component of at least half its length keeps its sign. So a tag left of its body (middle
/// right) on a symbol turned a quarter clockwise stands centred above it (bottom centre), and a designation beside a
/// connection point stays beside it, on the side away from the line. A baseline alignment is read as a bottom one (the
/// text lies above its anchor) and stays on the baseline while the text still lies above it; brought below it (a text
/// at 180°, a symbol turned half round) it is a top alignment, beside it a middle one (doc 09 §4.8). An alignment of a
/// newer format is kept.
pub fn placed_align(align: &Align, angle: Angle2, variant: Orient2) -> Align {
    keep_baseline(align, side_align(align, angle, variant))
}

/// [`placed_align`] with a baseline alignment read as a bottom one.
fn side_align(align: &Align, angle: Angle2, variant: Orient2) -> Align {
    let Some((h, v)) = align_parts(align) else {
        return align.clone();
    };
    let (upright, turned) = readable(angle);
    // Into the box, in the printed text's frame (x along the baseline, y down the glyphs); the readability turn
    // reverses both.
    let (dx, dy) = if turned { (h, v) } else { (-h, -v) };
    const R: i64 = 1_000_000;
    let base = ew_dl::num::arc_point(Pt2::ORIGIN, Nm(R), Nm(R), upright);
    let down = Pt2::new(Nm(-base.y.0), base.x);
    let (bx, by, wx, wy) = (
        i128::from(base.x.0),
        i128::from(base.y.0),
        i128::from(down.x.0),
        i128::from(down.y.0),
    );
    // Every value is at most 2·R, so the sheet vector fits `i64`.
    let sheet = Pt2::new(Nm((dx * bx + dy * wx) as i64), Nm((dx * by + dy * wy) as i64));
    let moved = variant.apply(sheet);
    let (mx, my) = (i128::from(moved.x.0), i128::from(moved.y.0));
    let side = |c: i128| {
        if 2 * c.abs() < i128::from(R) * i128::from(R) {
            0
        } else {
            c.signum()
        }
    };
    align_of(-side(mx * bx + my * by), -side(mx * wx + my * wy))
}

/// The alignment of slot `slot` of `def`, drawn at `angle`, on a symbol in `variant`: [`placed_align`], except for a
/// connection point's designation printing horizontally that the variant brought centred above or below its anchor
/// (it stood beside it in the symbol) while its point's line leaves sideways: it runs along the line, away from the
/// body (left-aligned on a line leaving right), so that it does not run into the body.
pub fn slot_align(def: &SymbolDef, slot: &TextSlot, angle: Angle2, variant: Orient2) -> Align {
    let placed = side_align(&slot.align, angle, variant);
    let SlotKind::CpDesignation(idx) = &slot.kind else {
        return keep_baseline(&slot.align, placed);
    };
    let (Some((h0, _)), Some((0, v))) = (align_parts(&slot.align), align_parts(&placed)) else {
        return keep_baseline(&slot.align, placed);
    };
    if h0 == 0 || upright_angle(angle) != Angle2::ZERO {
        return keep_baseline(&slot.align, placed);
    }
    let along = match def
        .cps
        .iter()
        .find(|cp| cp.idx == *idx)
        .map(|cp| transform_dir(&cp.dir, variant))
    {
        Some(Dir4::Right) => align_of(-1, v),
        Some(Dir4::Left) => align_of(1, v),
        _ => placed,
    };
    keep_baseline(&slot.align, along)
}

/// Where each slot of `def` prints on a symbol in `variant` where no override moves or turns it, by slot index: its
/// anchor in the symbol's frame after the variant and its alignment. A slot that `shown` leaves out keeps both as its
/// slot gives them, so a text an override moves keeps the alignment it was stored with and prints where it did.
/// A slot stands at its point turned by the variant, aligned by [`slot_align`], except on a symbol with connection
/// points turned a quarter (B, D, F, H): there its texts that print level are placed anew round its body so that none
/// stands on another or on the symbol (owner, 2026-11-22). The body is the bounds of its graphics and its static texts'
/// anchors in `variant`, and a step above or below the point of each pin's designation that prints there. Lying flat (no
/// pin leaves up or down), its tags stand centred above the body and its other texts centred below it; standing up (a
/// symbol drawn lying), all stand right-aligned left of the body from its top, the tags first. Each stands a quarter
/// step clear of the body and a line from the one before it, in the order they stand down the unturned symbol; a line
/// is a step and a half, and only the slots that `shown` takes take one. A pin's designation keeps its place at its pin.
pub fn slot_places(def: &SymbolDef, variant: Orient2, shown: impl Fn(usize) -> bool) -> Vec<(Pt2, Align)> {
    // A bus symbol's texts stand inside its body and turn with it (doc 10 §2.7).
    if ew_derive::is_bus_symbol(def) {
        return def
            .text_slots
            .iter()
            .map(|s| (variant.apply(s.pos), turned_with_body(&s.align, s.rot, variant).1))
            .collect();
    }
    let mut places: Vec<(Pt2, Align)> = def
        .text_slots
        .iter()
        .map(|s| (variant.apply(s.pos), slot_align(def, s, s.rot, variant)))
        .collect();
    let Some((body, flat)) = turned_body(def, variant) else {
        return places;
    };
    let quarters = |n: i64| def.grid.nm_of_tick(n, 4).map_or(0, |d| d.0);
    let (clear, step, pitch) = (quarters(1), quarters(4), quarters(6));
    let (mut top, mut bottom) = (body.min().y.0, body.max().y.0);
    for (i, (at, align)) in places.iter().enumerate() {
        let s = &def.text_slots[i];
        if !matches!(s.kind, SlotKind::CpDesignation(_)) || !shown(i) {
            continue;
        }
        let (up, down) = match align_parts(align).map_or(0, |(_, v)| v) {
            1 => (step, 0),
            -1 => (0, step),
            _ => (step / 2, step / 2),
        };
        top = top.min(at.y.0.saturating_sub(up));
        bottom = bottom.max(at.y.0.saturating_add(down));
    }
    let tag = |i: usize| matches!(def.text_slots[i].kind, SlotKind::Tag);
    let mut anew: Vec<usize> = (0..def.text_slots.len())
        .filter(|&i| {
            let s = &def.text_slots[i];
            !matches!(s.kind, SlotKind::CpDesignation(_)) && upright_angle(s.rot) == Angle2::ZERO
        })
        .collect();
    anew.sort_by_key(|&i| (!tag(i), def.text_slots[i].pos.y, def.text_slots[i].pos.x, i));
    let left = body.min().x.0.saturating_sub(clear);
    let centre = body.min().x.0.saturating_add(body.width().0 / 2);
    let (mut above, mut below) = (0i64, 0i64);
    for i in anew {
        let up = flat && tag(i);
        let line = if up { &mut above } else { &mut below };
        let off = pitch.saturating_mul(*line);
        let (at, align) = if !flat {
            (
                Pt2::new(Nm(left), Nm(body.min().y.0.saturating_add(off))),
                Align::TopRight,
            )
        } else if up {
            (
                Pt2::new(Nm(centre), Nm(top.saturating_sub(clear).saturating_sub(off))),
                Align::BottomCentre,
            )
        } else {
            (
                Pt2::new(Nm(centre), Nm(bottom.saturating_add(clear).saturating_add(off))),
                Align::TopCentre,
            )
        };
        if shown(i) {
            places[i] = (at, align);
            *line += 1;
        }
    }
    places
}

/// The body a symbol turned a quarter places its texts round ([`slot_places`]): the bounds of its graphics in
/// `variant`, its own texts' anchors included, and whether it lies flat (none of its pins leaves up or down). `None`
/// for a variant that is no quarter turn and for a symbol without connection points or graphics.
fn turned_body(def: &SymbolDef, variant: Orient2) -> Option<(Rect, bool)> {
    if variant.quarter_turns() % 2 == 0 || def.cps.is_empty() {
        return None;
    }
    let body = def
        .graphics
        .iter()
        .flat_map(lower_primitive)
        .map(|item| place_item(item, variant))
        .flat_map(|item| item_points(&item))
        .chain(def.graphics.iter().filter_map(|prim| match prim {
            Primitive::Text(text) => Some(variant.apply(text.pos)),
            _ => None,
        }))
        .map(Rect::from_point)
        .reduce(Rect::union)?;
    let flat = !def
        .cps
        .iter()
        .any(|cp| matches!(transform_dir(&cp.dir, variant), Dir4::Up | Dir4::Down));
    Some((body, flat))
}

/// The horizontal and vertical parts of `align`, each −1 (left, top), 0 (centre, middle) or 1 (right, bottom), a
/// baseline alignment's vertical part read as bottom; `None` for an alignment of a newer format.
fn align_parts(align: &Align) -> Option<(i128, i128)> {
    Some(match align {
        Align::TopLeft => (-1, -1),
        Align::TopCentre => (0, -1),
        Align::TopRight => (1, -1),
        Align::MiddleLeft => (-1, 0),
        Align::Centre => (0, 0),
        Align::MiddleRight => (1, 0),
        Align::BottomLeft | Align::BaselineLeft => (-1, 1),
        Align::BottomCentre | Align::BaselineCentre => (0, 1),
        Align::BottomRight | Align::BaselineRight => (1, 1),
        Align::Unknown(_) => return None,
    })
}

/// `placed` (a nine-way alignment from `align` read as [`align_parts`] reads it) on the baseline again when `align` is
/// a baseline alignment and the text still lies above its anchor (`placed` is a bottom one).
fn keep_baseline(align: &Align, placed: Align) -> Align {
    let baseline = matches!(
        align,
        Align::BaselineLeft | Align::BaselineCentre | Align::BaselineRight
    );
    match placed {
        Align::BottomLeft if baseline => Align::BaselineLeft,
        Align::BottomCentre if baseline => Align::BaselineCentre,
        Align::BottomRight if baseline => Align::BaselineRight,
        other => other,
    }
}

/// The alignment of the parts `h` and `v` ([`align_parts`]; any value above 0 counts as 1).
fn align_of(h: i128, v: i128) -> Align {
    match (h.signum(), v.signum()) {
        (-1, -1) => Align::TopLeft,
        (0, -1) => Align::TopCentre,
        (1, -1) => Align::TopRight,
        (-1, 0) => Align::MiddleLeft,
        (0, 0) => Align::Centre,
        (1, 0) => Align::MiddleRight,
        (-1, 1) => Align::BottomLeft,
        (0, 1) => Align::BottomCentre,
        _ => Align::BottomRight,
    }
}

/// The text-style key of a slot of a symbol that draws a terminal when `terminal` is set: its own `style` member when
/// it names a style of `styles` (the project's effective table, ADR-0032), else [`default_style_key`] of its kind. The
/// second value is the unknown name when the slot's style is not in the table and the kind's style was used instead.
pub fn slot_style_key<'a>(
    terminal: bool,
    slot: &'a TextSlot,
    styles: &TextStyleTable,
) -> (&'a str, Option<&'a str>) {
    match &slot.style {
        Some(name) if styles.get(name.as_str()).is_some() => (name.as_str(), None),
        Some(name) => (default_style_key(terminal, &slot.kind), Some(name.as_str())),
        None => (default_style_key(terminal, &slot.kind), None),
    }
}

/// The default text-style key bound to a slot kind of a symbol that draws a terminal when `terminal` is set
/// (`ew_schema::slot_default_style`; [`RepView::terminal`](crate::page::RepView::terminal)): `terminal` for a terminal
/// symbol's tag, `tag.main` for other tags, `cp.small` for CP designations, `data.small` for every other known kind,
/// `text.normal` for unknown kinds.
pub fn default_style_key(terminal: bool, kind: &SlotKind) -> &'static str {
    ew_schema::slot_default_style(terminal, kind)
}

/// The anchor of `def` in its own frame (R0), snapped to `def.grid` (see the module documentation).
pub fn resolve_anchor(def: &SymbolDef) -> Pt2 {
    let raw = match &def.anchor {
        AnchorPolicy::Slot(kind) => def
            .text_slots
            .iter()
            .find(|s| &s.kind == kind)
            .map_or(Pt2::ORIGIN, |s| s.pos),
        AnchorPolicy::GraphicCentre => graphics_bounds(def).map_or(Pt2::ORIGIN, |r| {
            Pt2::new(r.min().x + Nm(r.width().0 / 2), r.min().y + Nm(r.height().0 / 2))
        }),
        AnchorPolicy::Named(p) => *p,
        AnchorPolicy::Origin | AnchorPolicy::Unknown(_) => Pt2::ORIGIN,
    };
    let snap = |x: Nm| def.grid.snap_point(x).unwrap_or(x);
    Pt2::new(snap(raw.x), snap(raw.y))
}

/// The bounds of the graphics in the symbol frame (paths and images; texts excluded).
fn graphics_bounds(def: &SymbolDef) -> Option<Rect> {
    def.graphics
        .iter()
        .flat_map(lower_primitive)
        .flat_map(|item| item_points(&item))
        .map(Rect::from_point)
        .reduce(Rect::union)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_dl::PathEl;

    #[test]
    fn a_turned_or_mirrored_text_keeps_its_angle_and_its_side() {
        // A tag left of its body (middle right): the side of the body it lands on, in all eight variants.
        let want = [
            Align::MiddleRight,
            Align::BottomCentre,
            Align::MiddleLeft,
            Align::TopCentre,
            Align::MiddleLeft,
            Align::TopCentre,
            Align::MiddleRight,
            Align::BottomCentre,
        ];
        for (v, w) in Orient2::ALL.into_iter().zip(want) {
            assert_eq!(placed_align(&Align::MiddleRight, Angle2(0), v), w, "{v:?}");
        }
        // A designation right of a connection line leaving up, starting at the point and running up and right
        // (bottom left): turned a quarter clockwise the line leaves right, and the text stands below it.
        assert_eq!(
            placed_align(&Align::BottomLeft, Angle2(0), Orient2::R90),
            Align::TopLeft
        );
        // A text at 90° (reading bottom to top) keeps 90°; mirrored, it keeps its side along the baseline.
        assert_eq!(upright_angle(Angle2(90_000)), Angle2(90_000));
        assert_eq!(
            placed_align(&Align::MiddleLeft, Angle2(90_000), Orient2::M0),
            Align::MiddleLeft
        );
        assert_eq!(
            placed_align(&Align::TopLeft, Angle2(90_000), Orient2::M0),
            Align::BottomLeft
        );
        // Every result is readable, for every input angle.
        for a in (0..360_000).step_by(7_500) {
            let r = upright_angle(Angle2(a)).0;
            assert!(
                (0..=90_000).contains(&r) || (270_001..360_000).contains(&r),
                "{a} → {r}"
            );
        }
    }

    #[test]
    fn directions() {
        assert_eq!(transform_dir(&Dir4::Up, Orient2::R90), Dir4::Right);
        assert_eq!(transform_dir(&Dir4::Right, Orient2::R90), Dir4::Down);
        assert_eq!(transform_dir(&Dir4::Right, Orient2::M0), Dir4::Left);
        assert_eq!(transform_dir(&Dir4::Up, Orient2::M0), Dir4::Up);
        assert_eq!(transform_dir(&Dir4::Up, Orient2::M90), Dir4::Right);
        assert_eq!(transform_dir(&Dir4::Left, Orient2::R270), Dir4::Down);
    }

    #[test]
    fn square_shadow_is_the_visible_l() {
        // The L-shaped shadow of a 10 × 6 box with s = 2 covers exactly the offset box minus the box.
        let r = Rect::from_points(Pt2::new(Nm(0), Nm(0)), Pt2::new(Nm(10), Nm(6)));
        let p = RectPrim {
            rect: r.into(),
            rx: Nm(0),
            ry: Nm(0),
            shadow: Some(Nm(2)),
            stroke: None,
            fill: None,
            extra: Default::default(),
        };
        let item = shadow(&p).unwrap();
        let ew_dl::ItemKind::Path(path) = item.kind else {
            panic!("path");
        };
        assert_eq!(path.fill, Some(Brush::Solid(LAYER_COLOUR)));
        assert_eq!(path.path.els.len(), 7);
        assert_eq!(path.path.els[0], PathEl::MoveTo(Pt2::new(Nm(10), Nm(2))));
        assert_eq!(
            path.path.bounds(),
            Some(Rect::from_points(Pt2::new(Nm(2), Nm(2)), Pt2::new(Nm(12), Nm(8))))
        );
    }
}
