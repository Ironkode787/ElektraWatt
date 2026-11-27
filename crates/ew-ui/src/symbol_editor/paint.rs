// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! How the symbol editor draws a draft: the drawing through the production CPU renderer, the marks of its connection
//! points, link points and cross-reference point, and where each thing lies (its bounds, the thing under a point).

use crate::theme;
use egui::{Pos2, Stroke, Vec2};

/// The radius of a connection point's ring in points: with its designation beside it, and in a small preview.
const CP_RING_PT: f32 = 3.5;
/// See [`CP_RING_PT`].
const CP_RING_SMALL_PT: f32 = 1.8;
/// How far a mark's name stands from its point, along each axis, in points.
const NAME_OFFSET_PT: f32 = 5.0;
/// The radius of a link point's ring in points.
const LINK_RING_PT: f32 = 4.0;
/// The radius of the anchor's ring in points.
const ANCHOR_RING_PT: f32 = 6.0;
/// Half the length of the arms of a named anchor's cross in points.
const ANCHOR_ARM_PT: f32 = 9.0;
/// How far the named anchor's word stands from the anchor, along each axis, in points.
const ANCHOR_NAME_PT: f32 = 8.0;
use ew_viewmodel::ew_api::symbols::{self, SymbolDraft};
use ew_viewmodel::shell::symbol_editor::Item;
use ew_viewmodel::strings;

fn label(key: strings::Key<'_>) -> &'static str {
    strings::text(key)
}

pub(super) fn paint(
    painter: &egui::Painter,
    draft: &SymbolDraft,
    variant: u8,
    origin: Pos2,
    scale: f32,
    texts: bool,
    styles: &symbols::TextStyleTable,
) {
    let t = theme::current(painter.ctx());
    let Ok(g) = draft.preview(variant) else {
        return;
    };
    let at = |p: symbols::units::Pt2| {
        origin + Vec2::new(p.x.0 as f32 / 1_000_000.0, p.y.0 as f32 / 1_000_000.0) * scale
    };
    let list = symbols::preview_display_list(draft, &g, styles, &slot_placeholder);
    paint_graphics(painter, &list, variant, origin, scale);
    for cp in &g.cps {
        let p = at(cp.pos);
        painter.circle_stroke(
            p,
            if texts { CP_RING_PT } else { CP_RING_SMALL_PT },
            Stroke::new(t.border_width, t.canvas_selection.color32()),
        );
        if texts
            && !draft
                .symbol
                .text_slots
                .iter()
                .any(|s| s.visible && s.kind == symbols::schema::SlotKind::CpDesignation(cp.idx))
        {
            let name = draft
                .function
                .cps
                .iter()
                .find(|c| c.idx == cp.fn_cp)
                .map(|c| c.designation.as_str())
                .unwrap_or("");
            painter.text(
                p + Vec2::new(NAME_OFFSET_PT, -NAME_OFFSET_PT),
                egui::Align2::LEFT_BOTTOM,
                name,
                theme::font(&t.type_small),
                t.canvas_selection.color32(),
            );
        }
    }
    {
        if texts {
            use symbols::units::Orient2;
            let orientation = [
                Orient2::R0,
                Orient2::R90,
                Orient2::R180,
                Orient2::R270,
                Orient2::M0,
                Orient2::M90,
                Orient2::M180,
                Orient2::M270,
            ][usize::from(variant)];
            for point in &draft.symbol.link_points {
                let pos = at(orientation.apply(point.pos));
                painter.circle_stroke(
                    pos,
                    LINK_RING_PT,
                    Stroke::new(t.border_width, t.canvas_snap.color32()),
                );
                painter.text(
                    pos + Vec2::splat(NAME_OFFSET_PT),
                    egui::Align2::LEFT_TOP,
                    point.name.as_str(),
                    theme::font(&t.type_small),
                    t.canvas_snap.color32(),
                );
            }
            painter.circle_stroke(
                at(g.anchor),
                ANCHOR_RING_PT,
                Stroke::new(t.border_width, t.canvas_snap.color32()),
            );
            // A cross-reference anchor of its own shows as a marker: its circle crossed, and its name.
            if matches!(draft.symbol.anchor, symbols::schema::AnchorPolicy::Named(_)) {
                let p = at(g.anchor);
                let stroke = Stroke::new(t.border_width, t.canvas_snap.color32());
                let (x, y) = (Vec2::new(ANCHOR_ARM_PT, 0.0), Vec2::new(0.0, ANCHOR_ARM_PT));
                painter.line_segment([p - x, p + x], stroke);
                painter.line_segment([p - y, p + y], stroke);
                painter.text(
                    p + Vec2::new(ANCHOR_NAME_PT, -ANCHOR_NAME_PT),
                    egui::Align2::LEFT_BOTTOM,
                    label(ew_viewmodel::key!("symbol-anchor")),
                    theme::font(&t.type_small),
                    t.canvas_snap.color32(),
                );
            }
        }
    }
}

thread_local! {
    // The tag the drawing shows as its sample (`ShellModel::drawing_sample_tag`), set as each frame begins.
    static SAMPLE_TAG: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// Sets the tag the drawing shows as its sample this frame.
pub(super) fn set_sample_tag(tag: String) {
    SAMPLE_TAG.with(|t| *t.borrow_mut() = tag);
}

/// SM25#11: a slot previews as the words of its kind, not one placeholder for all; the tag as its sample.
fn slot_placeholder(kind: &symbols::schema::SlotKind) -> String {
    if matches!(kind, symbols::schema::SlotKind::Tag) {
        return SAMPLE_TAG.with(|t| t.borrow().clone());
    }
    label(match kind {
        symbols::schema::SlotKind::FunctionText => ew_viewmodel::key!("symbol-function-text"),
        symbols::schema::SlotKind::TechData => ew_viewmodel::key!("symbol-tech-data"),
        symbols::schema::SlotKind::Xref => ew_viewmodel::key!("symbol-xref"),
        _ => ew_viewmodel::key!("symbol-text"),
    })
    .to_owned()
}

/// The bounds of variant `variant` in millimetres as [`paint`] draws it, its texts included.
pub(super) fn drawn_bounds(
    draft: &SymbolDraft,
    variant: u8,
    styles: &symbols::TextStyleTable,
) -> Option<[f32; 4]> {
    let g = draft.preview(variant).ok()?;
    let b = symbols::preview_display_list(draft, &g, styles, &slot_placeholder)
        .bounds()
        .unwrap_or(g.bounds);
    Some(sides(b))
}

/// The origin and the scale that show `bounds` (millimetres) whole and centred in `area`; a drawing smaller than 8 mm
/// shows at the scale of 8 mm.
pub(super) fn variant_view(bounds: [f32; 4], area: egui::Rect) -> (Pos2, f32) {
    let (width, height) = (bounds[2] - bounds[0], bounds[3] - bounds[1]);
    let scale = (area.width() / width.max(8.0)).min(area.height() / height.max(8.0));
    let centre = Vec2::new(bounds[0] + bounds[2], bounds[1] + bounds[3]) / 2.0;
    (area.center() - centre * scale, scale)
}
/// The bounds of graphic `index` in millimetres: as drawn, a text's glyphs included, else as the preview
/// bounds it.
fn graphic_bounds(draft: &SymbolDraft, index: usize, styles: &symbols::TextStyleTable) -> Option<[f32; 4]> {
    let mut d = draft.clone();
    d.symbol.graphics = vec![d.symbol.graphics.get(index)?.clone()];
    d.symbol.cps = Default::default();
    d.symbol.text_slots.clear();
    d.symbol.link_points.clear();
    let g = d.preview(0).ok()?;
    let b = symbols::preview_display_list(&d, &g, styles, &|_| String::new())
        .bounds()
        .unwrap_or(g.bounds);
    Some(sides(b))
}

/// The bounds of text field `index` in millimetres as drawn (the words of its kind stand for its text); the point it
/// stands at for a field that draws nothing.
fn slot_bounds(draft: &SymbolDraft, index: usize, styles: &symbols::TextStyleTable) -> Option<[f32; 4]> {
    let slot = draft.symbol.text_slots.get(index)?;
    let mut d = draft.clone();
    d.symbol.graphics.clear();
    d.symbol.text_slots = vec![slot.clone()];
    d.symbol.link_points.clear();
    let drawn = d
        .preview(0)
        .ok()
        .and_then(|g| symbols::preview_display_list(&d, &g, styles, &slot_placeholder).bounds());
    let at = mm(slot.pos);
    Some(drawn.map_or([at.x, at.y, at.x, at.y], sides))
}

fn sides(b: symbols::units::Rect) -> [f32; 4] {
    [
        b.min().x.0 as f32 / 1e6,
        b.min().y.0 as f32 / 1e6,
        b.max().x.0 as f32 / 1e6,
        b.max().y.0 as f32 / 1e6,
    ]
}

/// A point of the symbol in millimetres.
fn mm(p: symbols::units::Pt2) -> Vec2 {
    Vec2::new(p.x.0 as f32 / 1e6, p.y.0 as f32 / 1e6)
}

/// Where one thing lies.
struct Place {
    item: Item,
    /// Its bounds in millimetres; a point's are the point itself.
    b: [f32; 4],
    /// A connection point, a link point or the cross-reference point: picked by its distance, outlined by its reach.
    point: bool,
    /// Whether a click on the drawing can take it (a text field that is not shown cannot).
    shown: bool,
    /// The outline of a shape that is not filled, as segments in millimetres: a click and a band that touches meet
    /// its line, not its face; `None` for a thing met by its face.
    edges: Option<Vec<[Vec2; 2]>>,
}

/// The outline of graphic `g` as segments in millimetres: what a click or a band that touches meets of a shape that
/// is not filled. `None` for a filled shape, a text or a picture, which are met by their face.
fn edges(g: &symbols::schema::Primitive) -> Option<Vec<[Vec2; 2]>> {
    use symbols::schema::Primitive as P;
    let path = |points: Vec<Vec2>, closed: bool| {
        let mut out: Vec<[Vec2; 2]> = points.windows(2).map(|w| [w[0], w[1]]).collect();
        if closed && let (Some(first), Some(last)) = (points.first(), points.last()) {
            out.push([*last, *first]);
        }
        out
    };
    // An ellipse in the rectangle `r` from angle `start` over `sweep` (degrees, counter-clockwise), in 48 steps.
    let ellipse = |r: symbols::units::Rect, start: f32, sweep: f32| {
        let (a, b) = (mm(r.min()), mm(r.max()));
        let (c, radius) = ((a + b) / 2.0, (b - a) / 2.0);
        let points = (0..=48)
            .map(|k| {
                let angle = (start + sweep * k as f32 / 48.0).to_radians();
                c + Vec2::new(radius.x * angle.cos(), -radius.y * angle.sin())
            })
            .collect();
        path(points, false)
    };
    let pts = |points: &[symbols::units::Pt2]| points.iter().map(|p| mm(*p)).collect::<Vec<_>>();
    match g {
        P::Line(l) => Some(vec![[mm(l.from), mm(l.to)]]),
        P::Polyline(l) => Some(path(pts(&l.points), false)),
        P::Polygon(p) if p.fill.is_none() => Some(path(pts(&p.points), p.closed)),
        P::Rect(r) if r.fill.is_none() => {
            let (a, b) = (mm(r.rect.0.min()), mm(r.rect.0.max()));
            let corners = vec![a, Vec2::new(b.x, a.y), b, Vec2::new(a.x, b.y)];
            Some(path(corners, true))
        }
        P::Circle(c) if c.fill.is_none() => {
            let (centre, r) = (mm(c.centre), c.r.0 as f32 / 1e6);
            let points = (0..=48)
                .map(|k| {
                    let angle = std::f32::consts::TAU * k as f32 / 48.0;
                    centre + Vec2::new(r * angle.cos(), r * angle.sin())
                })
                .collect();
            Some(path(points, false))
        }
        P::Ellipse(e) if e.fill.is_none() => Some(ellipse(e.rect.0, 0.0, 360.0)),
        P::Arc(a) => Some(ellipse(
            a.rect.0,
            a.start.0 as f32 / 1000.0,
            a.sweep.0 as f32 / 1000.0,
        )),
        _ => None,
    }
}

/// The distance from `p` to the segment `s`.
fn distance(p: Vec2, s: [Vec2; 2]) -> f32 {
    let d = s[1] - s[0];
    let k = if d.length_sq() > 0.0 {
        ((p - s[0]).dot(d) / d.length_sq()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (s[0] + d * k - p).length()
}

/// Whether the segment `s` meets the rectangle `r` ([x0, y0, x1, y1]): the part of it inside the rectangle, clipped
/// against each side in turn, is not empty.
fn meets(s: [Vec2; 2], r: [f32; 4]) -> bool {
    let d = s[1] - s[0];
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for (p, q) in [
        (-d.x, s[0].x - r[0]),
        (d.x, r[2] - s[0].x),
        (-d.y, s[0].y - r[1]),
        (d.y, r[3] - s[0].y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else if p < 0.0 {
            low = low.max(q / p);
        } else {
            high = high.min(q / p);
        }
    }
    low <= high
}

/// Where each thing of a draft lies in millimetres, read once per change of the draft: a text's glyphs need the
/// renderer's measure, which a frame does not repeat.
pub(super) struct Bounds(Vec<Place>);

impl Bounds {
    /// The bounds of every thing of `draft`.
    pub(super) fn of(draft: &SymbolDraft, styles: &symbols::TextStyleTable) -> Bounds {
        let s = &draft.symbol;
        let point = |item, p: Vec2| Place {
            item,
            b: [p.x, p.y, p.x, p.y],
            point: true,
            shown: true,
            edges: None,
        };
        let mut out: Vec<Place> = s
            .cps
            .iter()
            .enumerate()
            .map(|(i, c)| point(Item::Cp(i), mm(c.pos)))
            .collect();
        out.extend(
            s.link_points
                .iter()
                .enumerate()
                .map(|(i, l)| point(Item::Link(i), mm(l.pos))),
        );
        if let symbols::schema::AnchorPolicy::Named(p) = s.anchor {
            out.push(point(Item::Anchor, mm(p)));
        }
        out.extend(s.text_slots.iter().enumerate().filter_map(|(i, slot)| {
            Some(Place {
                item: Item::Slot(i),
                b: slot_bounds(draft, i, styles)?,
                point: false,
                shown: slot.visible,
                edges: None,
            })
        }));
        out.extend(s.graphics.iter().enumerate().filter_map(|(i, g)| {
            Some(Place {
                item: Item::Graphic(i),
                b: graphic_bounds(draft, i, styles)?,
                point: false,
                shown: true,
                edges: edges(g),
            })
        }));
        Bounds(out)
    }

    /// Whether a band that takes what it touches (`band` in millimetres, [x0, y0, x1, y1]) takes `item`: a shape that
    /// is not filled when the band meets its line, anything else when it meets its bounds.
    pub(super) fn touched(&self, item: Item, band: [f32; 4]) -> bool {
        let Some(place) = self.0.iter().find(|p| p.item == item) else {
            return false;
        };
        match &place.edges {
            Some(edges) => edges.iter().any(|s| meets(*s, band)),
            None => {
                let b = place.b;
                b[0] <= band[2] && band[0] <= b[2] && b[1] <= band[3] && band[1] <= b[3]
            }
        }
    }

    /// The bounds of `item`: a point's the square of half-side `reach` around it.
    pub(super) fn of_item(&self, item: Item, reach: f32) -> Option<[f32; 4]> {
        let place = self.0.iter().find(|p| p.item == item)?;
        let b = place.b;
        Some(if place.point {
            [b[0] - reach, b[1] - reach, b[2] + reach, b[3] + reach]
        } else {
            b
        })
    }

    /// The thing under `p` for the Select tool: the nearest connection point, link point or cross-reference point
    /// within `reach` (they lie on the lines that lead to them, so they come first), else the topmost text field on
    /// show whose text holds `p`, else the smallest shape that `p` meets (within `slop` of the line of a shape that is
    /// not filled, inside the widened bounds of a filled one), so a text is picked before the rectangle behind it; of
    /// equally small ones the topmost.
    pub(super) fn pick(&self, p: Vec2, slop: f32, reach: f32) -> Option<Item> {
        let holds = |b: [f32; 4]| {
            p.x >= b[0] - slop && p.y >= b[1] - slop && p.x <= b[2] + slop && p.y <= b[3] + slop
        };
        let nearest = self
            .0
            .iter()
            .filter(|place| place.point)
            .map(|place| ((Vec2::new(place.b[0], place.b[1]) - p).length(), place.item))
            .filter(|(d, _)| *d <= reach)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, item)) = nearest {
            return Some(item);
        }
        let texts = self.0.iter().rev().filter(|place| !place.point && place.shown);
        if let Some(place) = texts
            .clone()
            .find(|place| matches!(place.item, Item::Slot(_)) && holds(place.b))
        {
            return Some(place.item);
        }
        let meets = |place: &Place| match &place.edges {
            Some(edges) => edges.iter().any(|s| distance(p, *s) <= slop),
            None => holds(place.b),
        };
        texts
            .filter(|place| matches!(place.item, Item::Graphic(_)) && meets(place))
            .map(|place| ((place.b[2] - place.b[0]) * (place.b[3] - place.b[1]), place.item))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, item)| item)
    }
}

/// The production CPU renderer handles fills, dashes and stroke widths. Cache one viewport per variant.
fn paint_graphics(painter: &egui::Painter, list: &ew_dl::DisplayList, variant: u8, origin: Pos2, scale: f32) {
    use std::hash::{Hash, Hasher};
    use symbols::units::{Nm, Pt2};
    let rect = painter.clip_rect();
    let shrink = (2048.0 / rect.width().max(rect.height())).min(1.0);
    let width = (rect.width() * shrink).ceil().max(1.0);
    let height = (rect.height() * shrink).ceil().max(1.0);
    let offset = (origin - rect.min) * shrink;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    list.hash(&mut h);
    for v in [width, height, offset.x, offset.y, scale] {
        v.to_bits().hash(&mut h);
    }
    let signature = h.finish();
    let id = egui::Id::unique(("symbol-raster", variant, rect.height() > 100.0));
    let cached = painter
        .ctx()
        .data(|d| d.get_temp::<(u64, egui::TextureHandle)>(id));
    let texture = if let Some((key, texture)) = cached.filter(|(key, _)| *key == signature) {
        let _ = key;
        texture
    } else {
        let placement = ew_render_cpu::PagePlacement {
            width: Nm((width * 100_000.0) as i64),
            height: Nm((height * 100_000.0) as i64),
            scale: (((scale * shrink * 100_000.0).round() as u64).max(1), 1_000_000),
            offset: Pt2::new(
                Nm((offset.x * 100_000.0) as i64),
                Nm((offset.y * 100_000.0) as i64),
            ),
            overlay: Default::default(),
        };
        let options = ew_render_cpu::RenderOptions {
            dpi: 254,
            background: ew_dl::Color::from_rgba8(0, 0, 0, 0),
        };
        let Ok((image, _)) = ew_render_cpu::render_placed(
            list,
            &placement,
            &options,
            &ew_viewmodel::ew_api::output::font_bytes()
                .iter()
                .map(|(key, bytes)| {
                    (
                        *key,
                        ew_render_cpu::FontBytes {
                            data: bytes.data.clone(),
                            index: bytes.index,
                        },
                    )
                })
                .collect::<std::collections::BTreeMap<_, _>>(),
            None,
        ) else {
            return;
        };
        let texture = painter.ctx().load_texture(
            "symbol",
            egui::ColorImage::from_rgba_unmultiplied(
                [image.width as usize, image.height as usize],
                &image.pixels,
            ),
            egui::TextureOptions::LINEAR,
        );
        painter
            .ctx()
            .data_mut(|d| d.insert_temp(id, (signature, texture.clone())));
        texture
    };
    painter.image(
        texture.id(),
        rect,
        egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        theme::NO_TINT,
    );
}
