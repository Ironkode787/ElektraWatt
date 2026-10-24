// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page items and structure boxes drawn (WP-S2.04 part 4a, §10.3): graphics and structure-box outlines in
//! [`Layer::Symbols`](ew_dl::Layer::Symbols), free texts, wire-marker groups and captions in
//! [`Layer::Labels`](ew_dl::Layer::Labels); no new layer, so the permissive display-list crate is unchanged. The PDF
//! writes the same display list.
//!
//! | Item | Drawn as |
//! |---|---|
//! | graphic | its shape (line, polyline, rectangle, circle, arc in the QET convention, polygon) in the page's wire stroke ([`wire_stroke`]) unless `stroke` sets the width; `dashed`, `dotted` and `dash_dot` are the pen-width multiples of doc 07 §2.3; a closed shape with `fill` is filled black |
//! | free text | its printed text ([`ItemView::text`]) at `at` in its style (absent: `text.normal`), aligned left, centred or right on `at` and vertically on its middle, at a readable angle with the alignment flipped as slot texts are (doc 06 §2.5) |
//! | wire marker | when shown ([`ItemView::shown`]): the `ew.gen.wire-marker` symbol at `at` (turned for 90°) and the group text in style `marker`, centred, as the per-connection marker is drawn |
//! | structure box | its outline in the page's wire stroke width, dotted (0.5 mm on, 1 mm off) or dash-dot (4 mm on, 1 mm off, 0.5 mm on, 1 mm off); its caption in `tag.main` above the outline's top-left vertex (the smallest `y`, then `x`) |
//!
//! Items and boxes are drawn in ID order, each item tagged with its entity reference so that WP-S2.02's entity
//! rectangles pick it. A variant or shape this build does not type draws nothing.

use std::sync::Arc;

use ew_dl::{Brush, Dash, DashPattern, EntityRef, Item, Path, Stroke, Transform};
use ew_schema::{
    Align, BoxStyle, FreeText, Graphic, GraphicShape, LineDash, PageItem, PageItemKind, StructureBox,
    TextAlign,
};
use ew_text::DEFAULT_STYLE_KEY;
use ew_units::{Angle2, Nm, Orient2, Pt2, Rect};

use crate::page::{Builder, MARKER_STYLE, PageView, TextSpec, wire_stroke};
use crate::symbol;

/// One page item of the page with what it prints.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ItemView {
    /// The record.
    pub item: Arc<PageItem>,
    /// The printed text: a free text's text or bound value, a wire marker's group text; empty for a graphic.
    pub text: String,
    /// For a wire marker: whether the group is drawn (one of its connections would show a marker in the project's
    /// mode). Ignored for the other variants.
    pub shown: bool,
}

/// One structure box of the page with its printed caption.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StructureBoxView {
    /// The record.
    pub sbox: Arc<StructureBox>,
    /// The printed caption (the caption and, with `external`, the external reference); empty prints nothing.
    pub caption: String,
}

/// The text style of structure-box captions.
pub const CAPTION_STYLE: &str = "tag.main";

/// The dash of a dotted structure box: 0.5 mm on, 1 mm off.
pub const DOTTED: [Nm; 2] = [Nm(500_000), Nm(1_000_000)];

/// The dash of a dash-dot structure box: 4 mm on, 1 mm off, 0.5 mm on, 1 mm off.
pub const DASH_DOT: [Nm; 4] = [Nm(4_000_000), Nm(1_000_000), Nm(500_000), Nm(1_000_000)];

fn sorted_items(view: &PageView) -> Vec<&ItemView> {
    let mut items: Vec<&ItemView> = view.items.iter().collect();
    items.sort_by_key(|i| i.item.id);
    items
}

fn sorted_boxes(view: &PageView) -> Vec<&StructureBoxView> {
    let mut boxes: Vec<&StructureBoxView> = view.structure_boxes.iter().collect();
    boxes.sort_by_key(|b| b.sbox.id);
    boxes
}

/// The path of a graphic's shape and whether it is closed; `None` for a shape this build does not type.
fn shape_path(shape: &GraphicShape) -> Option<(Path, bool)> {
    Some(match shape {
        GraphicShape::Line(l) => (Path::polyline(&[l.a, l.b]), false),
        GraphicShape::Polyline(p) => (Path::polyline(&p.points), false),
        GraphicShape::Rect(r) => (Path::rect(Rect::from_points(r.min, r.max)), true),
        GraphicShape::Circle(c) => (Path::circle(c.centre, c.r), true),
        GraphicShape::Arc(a) => (Path::arc(a.centre, a.r, a.r, a.start, a.sweep), false),
        GraphicShape::Polygon(p) => (Path::polygon(&p.points), true),
        GraphicShape::Unknown(_) => return None,
    })
}

/// The stroke of a graphic: its width or the page's wire stroke, with its dash pattern.
fn graphic_stroke(view: &PageView, g: &Graphic) -> Stroke {
    let base = match &g.stroke {
        Some(s) => Stroke::solid(s.width),
        None => wire_stroke(view.grid),
    };
    match g.dash {
        LineDash::Dashed => base.dashed(DashPattern::Dash42),
        LineDash::Dotted => base.dashed(DashPattern::Dot12),
        LineDash::DashDot => base.dashed(DashPattern::DashDot),
        LineDash::Solid | LineDash::Unknown(_) => base,
    }
}

/// The stroke of a structure box's outline: the page's wire stroke width with the style's dash.
fn box_stroke(view: &PageView, style: &BoxStyle) -> Stroke {
    let base = wire_stroke(view.grid);
    let segments: Vec<Nm> = match style {
        BoxStyle::Dotted => DOTTED.to_vec(),
        BoxStyle::DashDot => DASH_DOT.to_vec(),
        BoxStyle::Unknown(_) => return base,
    };
    Stroke {
        dash: Some(Dash {
            segments,
            offset: Nm::ZERO,
        }),
        ..base
    }
}

/// The alignment of a free text at its point (horizontal from `align`, vertical on the middle).
fn text_align(align: Option<&TextAlign>) -> Align {
    match align {
        Some(TextAlign::Centre) => Align::Centre,
        Some(TextAlign::Right) => Align::MiddleRight,
        Some(TextAlign::Left | TextAlign::Unknown(_)) | None => Align::MiddleLeft,
    }
}

/// The top-left vertex of an outline: the smallest `y`, then the smallest `x`.
fn top_left(outline: &[Pt2]) -> Option<Pt2> {
    outline.iter().copied().min_by_key(|p| (p.y, p.x))
}

impl Builder<'_> {
    /// [`Layer::Symbols`](ew_dl::Layer::Symbols): the graphics, then the structure-box outlines, by ID.
    pub(crate) fn item_graphics(&mut self, view: &PageView) {
        for iv in sorted_items(view) {
            let PageItemKind::Graphic(g) = &iv.item.kind else {
                continue;
            };
            let Some((path, closed)) = shape_path(&g.shape) else {
                continue;
            };
            let fill = (closed && g.fill).then_some(Brush::BLACK);
            self.push(
                Item::path(path, Some(graphic_stroke(view, g)), fill)
                    .with_entity(EntityRef(iv.item.id.as_uuid())),
            );
        }
        for bv in sorted_boxes(view) {
            let b = &bv.sbox;
            if b.outline.len() < 2 {
                continue;
            }
            self.push(
                Item::path(Path::polygon(&b.outline), Some(box_stroke(view, &b.style)), None)
                    .with_entity(EntityRef(b.id.as_uuid())),
            );
        }
    }

    /// [`Layer::Labels`](ew_dl::Layer::Labels): the free texts and the shown wire-marker groups by ID, then the
    /// structure-box captions by box ID.
    pub(crate) fn item_labels(&mut self, view: &PageView) {
        for iv in sorted_items(view) {
            let entity = EntityRef(iv.item.id.as_uuid());
            match &iv.item.kind {
                PageItemKind::FreeText(t) => self.free_text(iv, t, entity),
                PageItemKind::WireMarker(m) if iv.shown && !iv.text.is_empty() => {
                    let vertical = m.angle == Angle2(90_000);
                    let (orient, angle) = if vertical {
                        (Orient2::R270, Angle2(90_000))
                    } else {
                        (Orient2::R0, Angle2::ZERO)
                    };
                    if let Some(s) = &view.wire_marker {
                        let key = self.geometry(s, orient);
                        self.push(Item::instance(key, Transform::from_translation(m.at)).with_entity(entity));
                    }
                    self.text(&TextSpec {
                        text: &iv.text,
                        at: m.at,
                        angle,
                        align: &Align::Centre,
                        style_key: MARKER_STYLE,
                        entity,
                        link: None,
                        items: &[],
                    });
                }
                _ => {}
            }
        }
        for bv in sorted_boxes(view) {
            let Some(at) = top_left(&bv.sbox.outline) else {
                continue;
            };
            self.text(&TextSpec {
                text: &bv.caption,
                at,
                angle: Angle2::ZERO,
                align: &Align::BottomLeft,
                style_key: CAPTION_STYLE,
                entity: EntityRef(bv.sbox.id.as_uuid()),
                link: None,
                items: &[],
            });
        }
    }

    fn free_text(&mut self, iv: &ItemView, t: &FreeText, entity: EntityRef) {
        let style = t.style.as_ref().map_or(DEFAULT_STYLE_KEY, |s| s.as_str());
        let align = text_align(t.align.as_ref());
        self.text(&TextSpec {
            text: &iv.text,
            at: t.at,
            angle: symbol::readable_angle(t.angle, Orient2::R0),
            align: &symbol::placed_align(&align, t.angle, Orient2::R0),
            style_key: style,
            entity,
            link: None,
            items: &[],
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_top_left_vertex_is_the_smallest_y_then_x() {
        let p = |x, y| Pt2::new(Nm(x), Nm(y));
        assert_eq!(top_left(&[p(5, 0), p(0, 0), p(0, 9)]), Some(p(0, 0)));
        assert_eq!(top_left(&[p(5, 3), p(9, 1), p(0, 9)]), Some(p(9, 1)));
        assert_eq!(top_left(&[]), None);
    }

    #[test]
    fn alignments_map_on_the_middle() {
        assert_eq!(text_align(None), Align::MiddleLeft);
        assert_eq!(text_align(Some(&TextAlign::Centre)), Align::Centre);
        assert_eq!(text_align(Some(&TextAlign::Right)), Align::MiddleRight);
    }
}
