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
//! Text positions transform, glyphs do not (doc 07 §3.2 rule 6, doc 06 §2.5): a text keeps its alignment and
//! style, and its angle is made readable from the bottom or the right of the sheet by [`readable_angle`].
//! For a text drawn at 0° in the symbol frame:
//!
//! | Variant | A `R0` | B `R90` | C `R180` | D `R270` | E `M0` | F `M90` | G `M180` | H `M270` |
//! |---|---|---|---|---|---|---|---|---|
//! | Text angle | 0° | 90° | 0° | 90° | 0° | 90° | 0° | 90° |
//!
//! 90° is [`Angle2`] `90_000`, counter-clockwise on the sheet: text reading bottom to top, which doc 06 calls
//! "R270". Glyphs are **never mirrored**: a [`PlacedText`] carries a rotation only, so the glyph run built
//! from it has an unmirrored transform. The alignment flips instead (doc 07 §4.1, ID-49): horizontally
//! (left ↔ right, centre stays) exactly when the text was turned by 180° to stay readable, vertically
//! (top ↔ bottom, middle stays) when exactly one of that turn and the variant's mirror applies. For a text
//! without its own rotation:
//!
//! | Variant | A | B | C | D | E | F | G | H |
//! |---|---|---|---|---|---|---|---|---|
//! | Horizontal | kept | flipped | flipped | kept | flipped | kept | kept | flipped |
//! | Vertical | kept | flipped | flipped | kept | kept | flipped | flipped | kept |
//! Slot texts come after static texts; a slot text's style key is the slot's own `style`, else the binding of
//! [`default_style_key`]. The page resolves it against the project's table ([`slot_style_key`], ADR-0032), which
//! a symbol's geometry does not depend on.
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

/// A text placed in a variant: an anchor point, a readable angle and the layout parameters `ew-text` needs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlacedText {
    /// What it prints.
    pub source: TextSource,
    /// The anchor point.
    pub pos: Pt2,
    /// The readable angle, counter-clockwise on the sheet, in `[0°, 90°] ∪ (270°, 360°)`.
    pub angle: Angle2,
    /// Alignment at `pos` after the variant: flipped as the readability turn and the mirror require
    /// (see the module documentation), so the text keeps its side of the anchor.
    pub align: Align,
    /// The text style key.
    pub style_key: String,
    /// Maximum width before shrinking.
    pub max_width: Option<Nm>,
    /// Visible by default.
    pub visible: bool,
}

/// Lowers `def` in `variant`.
pub fn lower(def: &SymbolDef, variant: Orient2) -> SymbolGeometry {
    let t = Transform::from_orient(variant);
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
            texts.push(PlacedText {
                source: TextSource::Static(text.text.clone()),
                pos: variant.apply(text.pos),
                angle: placement(text.angle, variant).0,
                align: placed_align(&text.align, text.angle, variant),
                style_key: text
                    .style
                    .as_ref()
                    .map_or_else(|| DEFAULT_TEXT_STYLE.to_owned(), |s| s.as_str().to_owned()),
                max_width: None,
                visible: true,
            });
        }
    }
    for slot in &def.text_slots {
        texts.push(PlacedText {
            source: TextSource::Slot(slot.kind.clone()),
            pos: variant.apply(slot.pos),
            angle: placement(slot.rot, variant).0,
            align: placed_align(&slot.align, slot.rot, variant),
            style_key: slot.style.as_ref().map_or_else(
                || default_style_key(&slot.kind).to_owned(),
                |s| s.as_str().to_owned(),
            ),
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

/// The readable angle of a text at `angle` (symbol frame) on a symbol in `variant`.
///
/// The baseline direction is transformed (a mirror maps `a` to `180° − a`, then the variant's clockwise
/// quarter turns subtract `90°` each), and a direction in `(90°, 270°]`, which would read upside down or top
/// to bottom, is turned by 180°. The result is normalised to `[0°, 360°)` and lies in
/// `[0°, 90°] ∪ (270°, 360°)`.
///
/// ```
/// use ew_scene::symbol::readable_angle;
/// use ew_units::{Angle2, Orient2};
///
/// assert_eq!(readable_angle(Angle2(0), Orient2::R180), Angle2(0));
/// assert_eq!(readable_angle(Angle2(0), Orient2::R90), Angle2(90_000));
/// assert_eq!(readable_angle(Angle2(90_000), Orient2::R90), Angle2(0));
/// assert_eq!(readable_angle(Angle2(30_000), Orient2::M0), Angle2(330_000));
/// ```
pub fn readable_angle(angle: Angle2, variant: Orient2) -> Angle2 {
    placement(angle, variant).0
}

/// The readable angle and whether the readability turn of 180° was applied to reach it.
fn placement(angle: Angle2, variant: Orient2) -> (Angle2, bool) {
    let mut a = i64::from(angle.normalized().0);
    if variant.is_mirrored() {
        a = 180_000 - a;
    }
    a -= i64::from(variant.quarter_turns()) * 90_000;
    a = a.rem_euclid(360_000);
    let turned = a > 90_000 && a <= 270_000;
    if turned {
        a -= 180_000;
    }
    (Angle2(a.rem_euclid(360_000) as i32), turned)
}

/// The alignment of a text at `angle` (symbol frame) on a symbol in `variant`: horizontal flips when the
/// text was turned by 180°, vertical when exactly one of the turn and the mirror applies (doc 07 §4.1).
/// An alignment of a newer format is kept.
pub fn placed_align(align: &Align, angle: Angle2, variant: Orient2) -> Align {
    let turned = placement(angle, variant).1;
    let flip_h = turned;
    let flip_v = turned != variant.is_mirrored();
    let (h, v) = match align {
        Align::TopLeft => (-1, -1),
        Align::TopCentre => (0, -1),
        Align::TopRight => (1, -1),
        Align::MiddleLeft => (-1, 0),
        Align::Centre => (0, 0),
        Align::MiddleRight => (1, 0),
        Align::BottomLeft => (-1, 1),
        Align::BottomCentre => (0, 1),
        Align::BottomRight => (1, 1),
        Align::Unknown(_) => return align.clone(),
    };
    let (h, v) = (if flip_h { -h } else { h }, if flip_v { -v } else { v });
    match (h, v) {
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

/// The text-style key of a slot: its own `style` member when it names a style of `styles` (the project's
/// effective table, ADR-0032), else [`default_style_key`] of its kind (decision log ID-197, ID-227). The second
/// value is the unknown name when the slot's style is not in the table and the kind's style was used instead.
pub fn slot_style_key<'a>(slot: &'a TextSlot, styles: &TextStyleTable) -> (&'a str, Option<&'a str>) {
    match &slot.style {
        Some(name) if styles.get(name.as_str()).is_some() => (name.as_str(), None),
        Some(name) => (default_style_key(&slot.kind), Some(name.as_str())),
        None => (default_style_key(&slot.kind), None),
    }
}

/// The default text-style key bound to a slot kind (doc 20 §17.5): `tag.main` for tags, `cp.small` for CP
/// designations, `data.small` for every other known kind, `text.normal` for unknown kinds. A placeholder for
/// the project's bindings (FR-SYM-013).
pub fn default_style_key(kind: &SlotKind) -> &'static str {
    match kind {
        SlotKind::Tag => "tag.main",
        SlotKind::CpDesignation(_) => "cp.small",
        SlotKind::Unknown(_) => DEFAULT_TEXT_STYLE,
        _ => "data.small",
    }
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
    fn text_angle_table() {
        let want = [0, 90_000, 0, 90_000, 0, 90_000, 0, 90_000];
        for (v, w) in Orient2::ALL.into_iter().zip(want) {
            assert_eq!(readable_angle(Angle2(0), v), Angle2(w), "{v:?}");
        }
        // Every result is readable, for every input angle.
        for a in (0..360_000).step_by(7_500) {
            for v in Orient2::ALL {
                let r = readable_angle(Angle2(a), v).0;
                assert!(
                    (0..=90_000).contains(&r) || (270_001..360_000).contains(&r),
                    "{a} {v:?} → {r}"
                );
            }
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
