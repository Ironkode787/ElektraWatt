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
//! CP's new position along its polyline as `geometry::follow_end` moves it ([`follow_polyline`]: the end vertex by
//! the CP's own displacement, the far end fixed, a dog-leg where a segment cannot follow; FT-423); a connection with
//! both ends moved is drawn translated.

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
        let snap = |v: i64| grid_point_of(cx.snapper.grid(), v);
        let line: Vec<(i64, i64)> = match (da, db) {
            (None, None) => continue,
            // Both ends moved: drawn translated (§6.7).
            (Some(d), Some(_)) => c.polyline.iter().map(|p| (p.0 + d.0, p.1 + d.1)).collect(),
            _ => follow_polyline(&c.polyline, &snap, da, db),
        };
        items.push(OverlayItem::Polyline {
            points: line.iter().map(|p| cx.snapper.page_to_sheet(*p)).collect(),
            style: Style::dashed(Token::RubberBand),
        });
    }
    // The autoconnect proposals of the moved CPs (WP-S2.04 §8.3): shown, never committed by a move.
    let found = move_proposals(cx, moved, steps);
    for p in &found.list {
        if let super::autoconnect::ProposalTarget::Cp { at, .. } = &p.target {
            items.push(OverlayItem::Line {
                a: cx.snapper.page_to_sheet(p.cp.at),
                b: cx.snapper.page_to_sheet(*at),
                style: Style::dashed(Token::Ghost),
            });
        }
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

/// `num / den` rounded to the nearest integer, exact ties toward zero (`den > 0`): `ew_units::Grid`'s rule.
fn div_round_toward_zero(num: i128, den: i128) -> i128 {
    let q = num / den;
    let r = num % den;
    if 2 * r.abs() > den { q + num.signum() } else { q }
}

/// The grid coordinate nearest the page coordinate `v` on the grid `(num, den)`, exactly as `ew_units::Grid::snap_point`
/// computes it (ties toward zero), so a dog-leg drawn here lands where the command puts it.
pub fn grid_point_of(grid: (i64, i64), v: i64) -> i64 {
    let (num, den) = (i128::from(grid.0.max(1)), i128::from(grid.1.max(1)));
    let k = div_round_toward_zero(i128::from(v) * den, num);
    let p = div_round_toward_zero(k * num, den);
    i64::try_from(p).unwrap_or(if p < 0 { i64::MIN } else { i64::MAX })
}

/// The autoconnect proposals of the free CPs of `moved` displaced by `steps` (WP-S2.04 §8.3): shown during a move,
/// never committed by it.
pub fn move_proposals(
    cx: &ToolCx<'_>,
    moved: &[EntityRef],
    steps: (i64, i64),
) -> super::autoconnect::Proposals {
    let tables = &cx.page.tables;
    let delta = |rep: EntityRef| {
        moved
            .contains(&rep)
            .then(|| tables.reps.iter().find(|r| r.id == rep.0))
            .flatten()
            .map(|r| rep_delta(cx, r.pos, steps))
    };
    let ghost = super::autoconnect::moved_cps(tables, &delta);
    super::autoconnect::proposals(&ghost, tables, moved, super::wire::drawing_area(cx), 0)
}

/// `geometry::follow_ends` of `ew-commands` on a page polyline (FT-423): end `a` moves by `da`, end `b` by `db`
/// (equal displacements translate); the far end never moves; a segment that keeps its axis stays, an interior
/// neighbour whose other segment is perpendicular follows on the moved axis, else a dog-leg at the grid coordinate
/// nearest the middle keeps both segments' axes. `snap` maps a page coordinate to the nearest grid coordinate. The
/// committed `rep.move` stores exactly this polyline (a test pins it).
pub fn follow_polyline(
    route: &[(i64, i64)],
    snap: &dyn Fn(i64) -> i64,
    da: Option<(i64, i64)>,
    db: Option<(i64, i64)>,
) -> Vec<(i64, i64)> {
    let mut r = route.to_vec();
    match (da, db) {
        (Some(a), Some(b)) if a == b => r.iter().map(|p| (p.0 + a.0, p.1 + a.1)).collect(),
        _ => {
            if let Some(a) = da {
                follow_start(&mut r, snap, a);
            }
            if let Some(b) = db {
                r.reverse();
                follow_start(&mut r, snap, b);
                r.reverse();
            }
            r.dedup();
            r
        }
    }
}

/// [`follow_polyline`] at the first vertex.
fn follow_start(r: &mut Vec<(i64, i64)>, snap: &dyn Fn(i64) -> i64, d: (i64, i64)) {
    let Some(&old) = r.first() else { return };
    let new = (old.0 + d.0, old.1 + d.1);
    r[0] = new;
    if r.len() < 2 || d == (0, 0) {
        return;
    }
    let nb = r[1];
    let horizontal = old.1 == nb.1 && old.0 != nb.0;
    let vertical = old.0 == nb.0 && old.1 != nb.1;
    let keeps = (horizontal && new.1 == nb.1) || (vertical && new.0 == nb.0);
    if !(keeps || !(horizontal || vertical)) {
        let perpendicular = r
            .get(2)
            .is_some_and(|m| (horizontal && nb.0 == m.0) || (vertical && nb.1 == m.1));
        if perpendicular {
            r[1] = if horizontal { (nb.0, new.1) } else { (new.0, nb.1) };
        } else if horizontal {
            let xm = snap(new.0 + (nb.0 - new.0) / 2);
            r.splice(1..1, [(xm, new.1), (xm, nb.1)]);
        } else {
            let ym = snap(new.1 + (nb.1 - new.1) / 2);
            r.splice(1..1, [(new.0, ym), (nb.0, ym)]);
        }
    }
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
