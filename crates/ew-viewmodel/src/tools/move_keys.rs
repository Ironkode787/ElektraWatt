// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Moving representations in grid steps (WP-S2.04 §7.3, D13, §6.7): the steps of a drag or of the keyboard move,
//! the overlay (the ghost of the moved entities' rectangles, the rubber bands of their connections and the
//! readout) and the command, `rep.move` with `steps`. Shared by Select's drag rows and its keyboard move (M).
//!
//! **Steps.** The page displacement of the pointer or the grid cursor since the press (or since M), per axis
//! `k = round(d × den / num)` in `i128`, halves away from zero; a representation then moves to the grid point at
//! its own index plus `k`, which the command computes exactly.
//!
//! **Rubber bands** (§6.7). A connection with exactly one end on a moved representation is drawn from the moved
//! CP's new position along its polyline as `geometry::follow_end` moves it (the end vertex by the CP's own
//! displacement, its neighbour keeping the segment orthogonal); a connection with both ends moved is drawn
//! translated.

use std::collections::BTreeSet;

use ew_api::AnyCommand;
use ew_api::output::TableEnd;
use ew_api::output::ew_dl::EntityRef;
use ew_api::output::ew_dl::ew_units::Rect;
use serde_json::json;

use super::{Anchor, Overlay, OverlayItem, Style, Token, ToolCx};
use crate::canvas::mm3;
use crate::strings;

/// The grid steps of a displacement from the sheet point `from` to `to` on the page of `cx`.
pub fn steps_between(cx: &ToolCx<'_>, from: (i64, i64), to: (i64, i64)) -> (i64, i64) {
    let a = cx.snapper.sheet_to_page(from);
    let b = cx.snapper.sheet_to_page(to);
    (
        cx.snapper.grid_index_of(b.0.saturating_sub(a.0)),
        cx.snapper.grid_index_of(b.1.saturating_sub(a.1)),
    )
}

/// The sheet displacement of `steps` grid steps (the ghost's offset).
pub fn sheet_offset(cx: &ToolCx<'_>, steps: (i64, i64)) -> (i64, i64) {
    let s = cx.snapper;
    (
        s.sheet_of_index(steps.0, 0) - s.sheet_of_index(0, 0),
        s.sheet_of_index(steps.1, 1) - s.sheet_of_index(0, 1),
    )
}

/// The readout of a move: `dx 7.938 dy 0.000 mm`, the sheet displacement of the steps.
pub fn readout(cx: &ToolCx<'_>, steps: (i64, i64)) -> String {
    let (dx, dy) = sheet_offset(cx, steps);
    strings::format("move-readout", &[&mm3(dx), &mm3(dy)])
}

/// The page displacement of a representation at page point `pos` moved by `steps` (its own grid index plus the
/// steps, as `rep.move` computes it).
fn rep_delta(cx: &ToolCx<'_>, pos: (i64, i64), steps: (i64, i64)) -> (i64, i64) {
    let s = cx.snapper;
    let axis = |p: i64, k: i64| {
        let i = s.grid_index_of(p);
        s.page_of_index(i.saturating_add(k))
            .saturating_sub(s.page_of_index(i))
    };
    (axis(pos.0, steps.0), axis(pos.1, steps.1))
}

/// The movable entities of a drag that starts on `pressed` (§7.3): the representations of the selection when the
/// pressed entity is selected, else the pressed entity alone; in selection order.
pub fn moved_set(cx: &ToolCx<'_>, pressed: EntityRef) -> Vec<EntityRef> {
    let reps: BTreeSet<_> = cx.page.tables.reps.iter().map(|r| r.id).collect();
    if cx.selection.contains(pressed) {
        cx.selection
            .items
            .iter()
            .filter(|e| reps.contains(&e.0))
            .copied()
            .collect()
    } else {
        vec![pressed]
    }
}

/// The overlay of a move of `moved` by `steps`: the ghost of their entity rectangles, the rubber bands of their
/// connections and the readout next to the ghost.
pub fn move_overlay(cx: &ToolCx<'_>, moved: &[EntityRef], steps: (i64, i64)) -> Overlay {
    let offset = sheet_offset(cx, steps);
    let rects: Vec<Rect> = cx
        .page
        .entities
        .iter()
        .filter(|(e, _)| moved.contains(e))
        .map(|(_, r)| *r)
        .collect();
    let mut items = Vec::new();
    let tables = &cx.page.tables;
    let moved_ids: BTreeSet<_> = moved.iter().map(|e| e.0).collect();
    // The displacement of each end drawn by a moved representation.
    let end_delta = |end: &TableEnd| -> Option<(i64, i64)> {
        let TableEnd::Cp { function, idx } = end else {
            return None;
        };
        let cp = tables
            .cps
            .iter()
            .find(|c| c.function == *function && c.idx == *idx)?;
        if !moved_ids.contains(&cp.rep) {
            return None;
        }
        let rep = tables.reps.iter().find(|r| r.id == cp.rep)?;
        Some(rep_delta(cx, rep.pos, steps))
    };
    for c in &tables.connections {
        let (da, db) = (end_delta(&c.a), end_delta(&c.b));
        let line: Vec<(i64, i64)> = match (da, db) {
            (None, None) => continue,
            (Some(d), Some(_)) => c.polyline.iter().map(|p| (p.0 + d.0, p.1 + d.1)).collect(),
            (Some(d), None) => follow_end(&c.polyline, true, d),
            (None, Some(d)) => follow_end(&c.polyline, false, d),
        };
        items.push(OverlayItem::Polyline {
            points: line.iter().map(|p| cx.snapper.page_to_sheet(*p)).collect(),
            style: Style::dashed(Token::RubberBand),
        });
    }
    let label_at = rects
        .iter()
        .copied()
        .reduce(Rect::union)
        .map(|r| (r.max().x.0 + offset.0, r.min().y.0 + offset.1));
    items.insert(0, OverlayItem::Ghost { rects, offset });
    if let Some(at) = label_at {
        items.push(OverlayItem::Label {
            at,
            text: readout(cx, steps),
            anchor: Anchor::Right,
        });
    }
    Overlay::new(items)
}

/// `geometry::follow_end` on a page polyline: the end vertex moves by `d`, its neighbour follows on the axis that
/// keeps their segment horizontal or vertical.
fn follow_end(route: &[(i64, i64)], at_start: bool, d: (i64, i64)) -> Vec<(i64, i64)> {
    let mut r = route.to_vec();
    let n = r.len();
    if n == 0 {
        return r;
    }
    let (end, next) = if at_start {
        (0, 1)
    } else {
        (n - 1, n.wrapping_sub(2))
    };
    let old = r[end];
    r[end] = (old.0 + d.0, old.1 + d.1);
    if n >= 2 {
        let nb = r[next];
        if nb.1 == old.1 && nb.0 != old.0 {
            r[next] = (nb.0, r[end].1);
        } else if nb.0 == old.0 && nb.1 != old.1 {
            r[next] = (r[end].0, nb.1);
        }
    }
    r
}

/// `rep.move {reps, steps}`.
pub fn move_command(moved: &[EntityRef], steps: (i64, i64)) -> Box<AnyCommand> {
    Box::new(AnyCommand {
        name: "rep.move".into(),
        params: json!({
            "reps": moved.iter().map(|e| e.0.to_string()).collect::<Vec<_>>(),
            "steps": [steps.0, steps.1],
        }),
    })
}
