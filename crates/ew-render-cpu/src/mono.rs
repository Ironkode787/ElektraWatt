// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The monochrome form of a display list (doc 09 §4.2, FR-OUT-011).

use std::sync::Arc;

use ew_dl::{
    Brush, Color, DisplayList, GeometryResolver, GroupItem, Item, ItemKind, MAX_INSTANCE_DEPTH, PathItem,
};

use crate::fonts::FontResolver;
use crate::render::{RenderError, RenderOptions, RenderReport, RgbaImage, SheetSize, render};

/// The monochrome form of `list`: every stroke brush and every glyph-run brush becomes opaque black
/// (alpha kept); every fill whose colour is not white (`r`, `g`, `b` all 255) becomes its luminance
/// grey `y = (2126 r + 7152 g + 722 b + 5000) / 10000` (integers), alpha kept; white fills stay white.
/// Recurses into groups. Instances are left as they are: their geometry is not in the list, so use
/// [`render_monochrome`], which resolves them first.
pub fn monochrome(list: &DisplayList) -> DisplayList {
    DisplayList {
        items: list.items.iter().map(mono_item).collect(),
    }
}

fn black(c: Color) -> Color {
    Color::from_rgba8(0, 0, 0, c.a)
}

fn grey(c: Color) -> Color {
    if (c.r, c.g, c.b) == (255, 255, 255) {
        return c;
    }
    let y = (2126 * u32::from(c.r) + 7152 * u32::from(c.g) + 722 * u32::from(c.b) + 5000) / 10000;
    let y = y.min(255) as u8;
    Color::from_rgba8(y, y, y, c.a)
}

fn map_brush(b: Brush, f: impl Fn(Color) -> Color) -> Brush {
    match b {
        Brush::Solid(c) => Brush::Solid(f(c)),
    }
}

fn with_kind(item: &Item, kind: ItemKind) -> Item {
    Item {
        kind,
        entity: item.entity,
        link: item.link,
    }
}

fn mono_item(item: &Item) -> Item {
    let kind = match &item.kind {
        ItemKind::Path(p) => ItemKind::Path(PathItem {
            path: p.path.clone(),
            stroke: p.stroke.as_ref().map(|s| {
                let mut s = s.clone();
                s.brush = map_brush(s.brush, black);
                s
            }),
            fill: p.fill.map(|b| map_brush(b, grey)),
        }),
        ItemKind::Glyphs(run) => {
            let mut run = (**run).clone();
            run.brush = map_brush(run.brush, black);
            ItemKind::Glyphs(Arc::new(run))
        }
        ItemKind::Group(g) => ItemKind::Group(GroupItem {
            transform: g.transform,
            clip: g.clip,
            items: g.items.iter().map(mono_item).collect(),
        }),
        ItemKind::Image(_) | ItemKind::Instance(_) => item.kind.clone(),
    };
    with_kind(item, kind)
}

/// `items` with every resolvable instance replaced by a group holding its geometry under the
/// instance transform (recursively up to [`MAX_INSTANCE_DEPTH`]). Unresolved instances stay.
fn inline_instances(items: &[Item], geometry: &dyn GeometryResolver, depth: usize) -> Vec<Item> {
    items
        .iter()
        .map(|item| {
            let kind = match &item.kind {
                ItemKind::Instance(inst) => {
                    let geom = if depth < MAX_INSTANCE_DEPTH {
                        geometry.geometry(inst.symbol)
                    } else {
                        None
                    };
                    match geom {
                        Some(dl) => ItemKind::Group(GroupItem {
                            transform: Some(inst.transform),
                            clip: None,
                            items: inline_instances(&dl.items, geometry, depth + 1),
                        }),
                        None => item.kind.clone(),
                    }
                }
                ItemKind::Group(g) => ItemKind::Group(GroupItem {
                    transform: g.transform,
                    clip: g.clip,
                    items: inline_instances(&g.items, geometry, depth),
                }),
                other => other.clone(),
            };
            with_kind(item, kind)
        })
        .collect()
}

/// [`render`] of the [`monochrome`] form of `list`: resolvable instances are first inlined as groups
/// with the instance transform (so their geometry turns monochrome too), then the colours are mapped,
/// then the list is rendered. Unresolved instances are skipped and counted as in [`render`].
pub fn render_monochrome(
    list: &DisplayList,
    sheet: SheetSize,
    options: &RenderOptions,
    fonts: &dyn FontResolver,
    geometry: Option<&dyn GeometryResolver>,
) -> Result<(RgbaImage, RenderReport), RenderError> {
    let inlined;
    let source = match geometry {
        Some(g) => {
            inlined = DisplayList {
                items: inline_instances(&list.items, g, 0),
            };
            &inlined
        }
        None => list,
    };
    render(&monochrome(source), sheet, options, fonts, None)
}
