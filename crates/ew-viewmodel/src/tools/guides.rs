// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Placing by hand, helped (docs/design/06a-drafting.md §6): the Place tool's floating symbol and a symbol dragged by
//! Select ask [`ew_api::drafting::guides`] after the ordinary snap, unless Alt is held, on the focused page's
//! [`CanvasPage::draft`](crate::canvas::CanvasPage::draft). The answer is the grid point the part takes (the snap's,
//! or its neighbour across an In-line guide) and the guides to draw: an In-line guide dotted in the `snap` token, solid
//! with its tick across when the pins stand in line, a Level guide dotted in the `ghost` token. Nothing is stored.

use ew_api::drafting::consts::TICKS_PER_STEP;
use ew_api::drafting::{self, GuideKind, Part, PartId, Pin, PinId, Pt, Rect, Side, Tick};
use ew_api::geometry::Dir;
use ew_api::output::Uuid;
use ew_api::routing::Ticks;
use ew_units::Grid;

use super::{OverlayItem, Style, Token, ToolCx};

/// A tick of `i64` as the model's `i32`, saturating.
fn tick(v: i64) -> Tick {
    Tick::try_from(v).unwrap_or(if v < 0 { Tick::MIN } else { Tick::MAX })
}

/// The page's conversion into ticks, `None` for a grid the units refuse.
fn grid(cx: &ToolCx<'_>) -> Option<Ticks> {
    let (num, den) = cx.page.tables.grid;
    Grid::new(u64::try_from(num).ok()?, u64::try_from(den).ok()?)
        .ok()
        .map(Ticks::of)
}

/// `k` grid steps in ticks.
pub(crate) fn steps(k: (i64, i64)) -> Pt {
    let whole = |k: i64| tick(k.saturating_mul(i64::from(TICKS_PER_STEP)));
    Pt::new(whole(k.0), whole(k.1))
}

/// A part floating on the pointer at grid index `index`, from its shape relative to its origin in page nm: its body
/// `[x0, y0, x1, y1]` and its pins with their directions.
pub(crate) fn hanging(
    cx: &ToolCx<'_>,
    index: (i64, i64),
    body: [i64; 4],
    pins: impl Iterator<Item = ((i64, i64), Dir)>,
) -> Option<Part> {
    let g = grid(cx)?;
    let side = |d: Dir| match d {
        Dir::Up => Side::Up,
        Dir::Down => Side::Down,
        Dir::Left => Side::Left,
        Dir::Right => Side::Right,
    };
    Some(Part {
        id: PartId(Uuid::nil()),
        at: steps(index),
        body: Rect::new(g.pt((body[0], body[1])), g.pt((body[2], body[3]))),
        frame_only: false,
        pins: pins
            .map(|(at, d)| Pin {
                id: PinId(Uuid::nil()),
                at: g.pt(at),
                side: Some(side(d)),
                off_grid: false,
            })
            .collect(),
        unit: 0,
        role: drafting::Role::Other,
        locked: false,
        off_grid: false,
    })
}

/// The grid steps the nudge moves the hanging `part` by (none, or one across a guide) and the overlay of its guides,
/// for `part` at the ordinary snap's place and `pointer`, the page point the pointer asks its origin to be at.
pub(crate) fn guide(cx: &ToolCx<'_>, part: &Part, pointer: (i64, i64)) -> ((i64, i64), Vec<OverlayItem>) {
    let Some(g) = grid(cx) else {
        return ((0, 0), Vec::new());
    };
    let s = cx.snapper;
    let step = (s.sheet_of_index(1, 0) - s.sheet_of_index(0, 0)) as f64;
    let guided = drafting::guides(&cx.page.draft, part, g.pt(pointer), cx.points_per_nm * step);
    let moved = |a: Tick, b: Tick| i64::from(a.saturating_sub(b) / TICKS_PER_STEP);
    let nudge = (moved(guided.at.x, part.at.x), moved(guided.at.y, part.at.y));
    let sheet = |p: Pt| s.page_to_sheet((g.nm(p.x), g.nm(p.y)));
    let mut items = Vec::new();
    for guide in &guided.guides {
        let style = match (guide.kind, guide.in_line) {
            (GuideKind::InLine, true) => Style::solid(Token::Snap),
            (GuideKind::InLine, false) => Style::dotted(Token::Snap),
            (GuideKind::Level, _) => Style::dotted(Token::Ghost),
        };
        items.push(OverlayItem::Line {
            a: sheet(guide.line.0),
            b: sheet(guide.line.1),
            style,
        });
        if let Some((a, b)) = guide.tick {
            items.push(OverlayItem::Line {
                a: sheet(a),
                b: sheet(b),
                style: Style::solid(Token::Snap),
            });
        }
    }
    (nudge, items)
}
