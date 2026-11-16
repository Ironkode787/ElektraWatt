// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! How the symbol editor draws a draft: the drawing through the production CPU renderer, the marks of its connection
//! points, link points and cross-reference point, and where each thing lies (its bounds, the thing under a point).

use crate::theme;
use egui::{Pos2, Stroke, Vec2};
use ew_viewmodel::ew_api::symbols::{self, SymbolDraft, drawing};
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
            if texts { 3.5 } else { 1.8 },
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
                p + Vec2::new(5.0, -5.0),
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
                painter.circle_stroke(pos, 4.0, Stroke::new(t.border_width, t.canvas_snap.color32()));
                painter.text(
                    pos + Vec2::new(5.0, 5.0),
                    egui::Align2::LEFT_TOP,
                    point.name.as_str(),
                    theme::font(&t.type_small),
                    t.canvas_snap.color32(),
                );
            }
            painter.circle_stroke(
                at(g.anchor),
                6.0,
                Stroke::new(t.border_width, t.canvas_snap.color32()),
            );
            // A cross-reference anchor of its own shows as a marker: its circle crossed, and its name.
            if matches!(draft.symbol.anchor, symbols::schema::AnchorPolicy::Named(_)) {
                let p = at(g.anchor);
                let stroke = Stroke::new(t.border_width, t.canvas_snap.color32());
                painter.line_segment([p - Vec2::new(9.0, 0.0), p + Vec2::new(9.0, 0.0)], stroke);
                painter.line_segment([p - Vec2::new(0.0, 9.0), p + Vec2::new(0.0, 9.0)], stroke);
                painter.text(
                    p + Vec2::new(8.0, -8.0),
                    egui::Align2::LEFT_BOTTOM,
                    label(strings::Key::run_time("symbol-anchor")),
                    theme::font(&t.type_small),
                    t.canvas_snap.color32(),
                );
            }
        }
    }
}

/// SM25#11: a slot previews as the words of its kind, not one placeholder for all.
fn slot_placeholder(kind: &symbols::schema::SlotKind) -> String {
    label(strings::Key::run_time(match kind {
        symbols::schema::SlotKind::FunctionText => "symbol-function-text",
        symbols::schema::SlotKind::TechData => "symbol-tech-data",
        symbols::schema::SlotKind::Xref => "symbol-xref",
        _ => "symbol-text",
    }))
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
            })
        }));
        out.extend((0..s.graphics.len()).filter_map(|i| {
            Some(Place {
                item: Item::Graphic(i),
                b: graphic_bounds(draft, i, styles)?,
                point: false,
                shown: true,
            })
        }));
        Bounds(out)
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
    /// show whose text holds `p`, else the smallest shape whose bounds (widened by `slop`) hold it, so a text is
    /// picked before the rectangle behind it; of equally small ones the topmost.
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
        texts
            .filter(|place| matches!(place.item, Item::Graphic(_)) && holds(place.b))
            .map(|place| ((place.b[2] - place.b[0]) * (place.b[3] - place.b[1]), place.item))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, item)| item)
    }
}

/// The production CPU renderer handles fills, dashes and stroke widths. Cache one viewport per variant.
fn paint_graphics(
    painter: &egui::Painter,
    list: &drawing::DisplayList,
    variant: u8,
    origin: Pos2,
    scale: f32,
) {
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
            background: drawing::Color::from_rgba8(0, 0, 0, 0),
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
