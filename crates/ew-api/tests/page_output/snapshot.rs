// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The text form of a page display list, for the insta snapshot: one line per item, in page coordinates.

use std::fmt::Write as _;

use ew_dl::{Brush, Item, ItemKind, Layer};
use ew_scene::page::PageScene;
use ew_units::Pt2;

fn pt(p: Pt2) -> String {
    format!("({},{})", p.x.0, p.y.0)
}

fn entity(item: &Item) -> String {
    item.entity
        .map_or_else(|| "-".to_owned(), |e| e.0.simple().to_string()[..8].to_owned())
}

fn line(out: &mut String, layer: Layer, item: &Item, depth: usize) {
    let pad = " ".repeat(depth);
    let (l, e) = (layer.name(), entity(item));
    match &item.kind {
        ItemKind::Path(p) => {
            let first = p.path.points().next().map_or_else(|| "-".to_owned(), pt);
            let last = p.path.points().last().map_or_else(|| "-".to_owned(), pt);
            let stroke = p
                .stroke
                .as_ref()
                .map_or_else(|| "-".to_owned(), |s| s.width.0.to_string());
            let fill = p
                .fill
                .map_or_else(|| "-".to_owned(), |Brush::Solid(c)| c.to_hex());
            writeln!(
                out,
                "{pad}{l} path {e} els={} first={first} last={last} stroke={stroke} fill={fill}",
                p.path.els.len()
            )
            .unwrap();
        }
        ItemKind::Glyphs(r) => writeln!(
            out,
            "{pad}{l} glyphs {e} {:?} origin={} size={} angle={}",
            r.text,
            pt(r.origin),
            r.size.0,
            r.transform.angle.0
        )
        .unwrap(),
        ItemKind::Instance(i) => writeln!(
            out,
            "{pad}{l} instance {e} sym={} variant={} translate={}",
            &i.symbol.symbol.simple().to_string()[..8],
            i.symbol.variant.letter(),
            pt(i.transform.translate)
        )
        .unwrap(),
        ItemKind::Group(g) => {
            writeln!(out, "{pad}{l} group {e} items={}", g.items.len()).unwrap();
            for i in &g.items {
                line(out, layer, i, depth + 1);
            }
        }
        ItemKind::Image(_) => writeln!(out, "{pad}{l} image {e}").unwrap(),
    }
}

/// The lines of `layers` of `scene`.
pub fn display_list_text(scene: &PageScene, layers: &[Layer]) -> String {
    let mut out = String::new();
    for &layer in layers {
        let list = scene.layer(layer).expect("the page has the layer");
        for item in &list.items {
            line(&mut out, layer, item, 0);
        }
    }
    out
}
