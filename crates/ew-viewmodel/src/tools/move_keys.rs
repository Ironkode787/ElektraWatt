// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Moving representations in grid steps (D13, §6.7): the steps of a drag or of the keyboard move,
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
use ew_api::output::ew_dl::EntityRef;
use ew_api::output::ew_dl::ew_units::Rect;
use ew_api::output::{PageTables, TableEnd};
use serde_json::json;

use super::{Anchor, Overlay, OverlayItem, Style, Token, ToolCx, clamp64, div_round_toward_zero};
use crate::canvas::mm3;
use crate::key;
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
    strings::format(key!("move-readout"), &[&mm3(dx), &mm3(dy)])
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

/// The movable entities of a drag that starts on `pressed` (§7.3): the selection's representations, page items and
/// structure boxes when the pressed entity is selected, else the pressed entity alone; in selection order.
pub fn moved_set(cx: &ToolCx<'_>, pressed: EntityRef) -> Vec<EntityRef> {
    if cx.selection.contains(pressed) {
        super::selection_kinds(cx.selection, &cx.page.tables)
            .into_iter()
            .filter(|(_, k)| {
                matches!(
                    k,
                    super::EntityKind::Rep | super::EntityKind::Item | super::EntityKind::StructureBox
                )
            })
            .map(|(e, _)| e)
            .collect()
    } else {
        vec![pressed]
    }
}

/// The overlay of a move of `moved` by `steps`: the ghost of their entity rectangles (items and boxes ghost by
/// their rectangles), the rubber bands of their connections and the readout next to the ghost.
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
    // The autoconnect proposals of the moved CPs: shown, never committed by a move.
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

/// The grid coordinate nearest the page coordinate `v` on the grid `(num, den)`, exactly as `ew_units::Grid::snap_point`
/// computes it (ties toward zero), so a dog-leg drawn here lands where the command puts it.
pub fn grid_point_of(grid: (i64, i64), v: i64) -> i64 {
    let (num, den) = (i128::from(grid.0.max(1)), i128::from(grid.1.max(1)));
    let k = div_round_toward_zero(i128::from(v) * den, num);
    let p = div_round_toward_zero(k * num, den);
    clamp64(p)
}

/// The autoconnect proposals of the free CPs of `moved` displaced by `steps`: the CP proposals a move
/// shows and, with connect on move, commits; a split is none.
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
    let mut found = super::autoconnect::proposals(&ghost, tables, moved, super::wire::drawing_area(cx), 0);
    // A move never splits an existing wire (ADR-0007 decision 4, ID-1509 (6)), so a split is no proposal of a move:
    // the ghost lists only the wires the release makes (C53-rt#12).
    found
        .list
        .retain(|p| matches!(p.target, super::autoconnect::ProposalTarget::Cp { .. }));
    found
}

/// `geometry::follow_ends` of `ew-commands` on a page polyline: end `a` moves by `da`, end `b` by `db`
/// (equal displacements translate); the far end never moves; a segment that keeps its axis stays, an interior
/// neighbour whose other segment is perpendicular follows on the moved axis, else a dog-leg at the grid coordinate
/// nearest the middle keeps both segments' axes. `snap` maps a page coordinate to the nearest grid coordinate. The
/// committed `rep.move` stores exactly this polyline (a test pins it). Consecutive equal vertices of `route` are
/// merged first: a drawn polyline repeats an end vertex that meets its CP (the CP and the stored route's end), and
/// the rule reads the end's segment from the stored route.
pub fn follow_polyline(
    route: &[(i64, i64)],
    snap: &dyn Fn(i64) -> i64,
    da: Option<(i64, i64)>,
    db: Option<(i64, i64)>,
) -> Vec<(i64, i64)> {
    let mut r = route.to_vec();
    r.dedup();
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

/// The command a move of `moved` by `steps` commits on release: [`move_command`] with `routes`,
/// the connections [`resolved_routes`] routes again, and with `connect` the autoconnect proposals of the moved CPs as
/// `connect` hints, so the move, its re-solved routes and its new wires are one undo step.
pub fn release_command(
    cx: &ToolCx<'_>,
    moved: &[EntityRef],
    steps: (i64, i64),
    connect: bool,
) -> Box<AnyCommand> {
    let mut cmd = move_command(&cx.page.tables, moved, steps);
    let routes = resolved_routes(cx, moved, steps);
    if !routes.is_empty() {
        cmd.params["routes"] = serde_json::Value::Array(routes);
    }
    if connect {
        let tables = &cx.page.tables;
        let hints: Vec<serde_json::Value> = move_proposals(cx, moved, steps)
            .list
            .iter()
            .filter_map(|p| match &p.target {
                // A moved CP's index in the page tables stands for its symbol CP (`autoconnect::moved_cps`).
                super::autoconnect::ProposalTarget::Cp { function, idx, .. } => {
                    let cp = tables.cps.get(usize::from(p.cp.symbol_cp))?;
                    Some(json!({
                        "rep": cp.rep.to_string(),
                        "cp": {"function": cp.function.to_string(), "idx": cp.idx},
                        "to": {"cp": {"function": function.0.to_string(), "idx": idx}},
                    }))
                }
                super::autoconnect::ProposalTarget::Split { .. } => None,
            })
            .collect();
        if !hints.is_empty() {
            cmd.params["connect"] = serde_json::Value::Array(hints);
        }
    }
    cmd
}

/// The re-solved routes of a move of `moved` by `steps`, as `rep.move`'s `routes` entries. On the page
/// geometry with the move applied (the moved representations' bodies, CPs and texts displaced as `rep.move` displaces
/// them; each connection with a moved end drawn along its rubber band), every such connection that is unrouted,
/// breaks a hard rule there or has more bends than before is routed again (`route_page`, the other connections fixed);
/// the others keep the followed route, and so does one the router finds no clear route for.
pub fn resolved_routes(cx: &ToolCx<'_>, moved: &[EntityRef], steps: (i64, i64)) -> Vec<serde_json::Value> {
    use ew_api::routing::{Measures, measure, route_page};
    let tables = &cx.page.tables;
    let before = &*cx.page.geometry;
    let delta_of = |rep: ew_api::output::Uuid| -> Option<(i64, i64)> {
        if !moved.iter().any(|e| e.0 == rep) {
            return None;
        }
        tables
            .reps
            .iter()
            .find(|r| r.id == rep)
            .map(|r| rep_delta(cx, r.pos, steps))
    };
    let shift = |r: [i64; 4], d: (i64, i64)| [r[0] + d.0, r[1] + d.1, r[2] + d.0, r[3] + d.1];
    let mut after = before.clone();
    for b in &mut after.bodies {
        if let Some(d) = delta_of(b.rep) {
            b.rect = shift(b.rect, d);
        }
    }
    for t in &mut after.texts {
        if let Some(d) = delta_of(t.rep) {
            t.rect = shift(t.rect, d);
        }
    }
    let mut moved_cps: Vec<(ew_api::output::Uuid, (i64, i64))> = Vec::new();
    for cp in &mut after.cps {
        if let Some(d) = delta_of(cp.rep) {
            cp.at = (cp.at.0 + d.0, cp.at.1 + d.1);
            moved_cps.push((cp.id, d));
        }
    }
    let snap = |v: i64| grid_point_of(cx.snapper.grid(), v);
    let end_delta = |end: ew_api::output::Uuid| moved_cps.iter().find(|(id, _)| *id == end).map(|(_, d)| *d);
    let mut affected = Vec::new();
    for c in &mut after.connections {
        let (da, db) = (end_delta(c.ends[0]), end_delta(c.ends[1]));
        c.polyline = match (da, db) {
            (None, None) => continue,
            (Some(d), Some(_)) => c.polyline.iter().map(|p| (p.0 + d.0, p.1 + d.1)).collect(),
            _ => follow_polyline(&c.polyline, &snap, da, db),
        };
        affected.push(c.id);
    }
    if affected.is_empty() {
        return Vec::new();
    }
    // D3: an unrouted connection's dashed line is no wire, so the measures read the routed connections only (as the
    // layout rules do); the unrouted ones stay in `after` as jobs.
    let routed = |g: &ew_api::routing::PageGeometry| {
        let mut g = g.clone();
        g.connections.retain(|c| c.routed);
        g
    };
    let (was, now) = (measure(&routed(before)), measure(&routed(&after)));
    let bends = |m: &Measures, id: ew_api::output::Uuid| {
        m.bends.iter().find(|b| b.connection == id).map_or(0, |b| b.bends)
    };
    let breaks = |id: ew_api::output::Uuid| {
        now.touches.iter().any(|t| t.connection == id)
            || now.overlaps.iter().any(|o| o.connections.contains(&id))
            || now.through_bodies.iter().any(|t| t.connection == id)
            || now.through_texts.iter().any(|t| t.connection == id)
    };
    let jobs: Vec<ew_api::output::Uuid> = affected
        .into_iter()
        .filter(|id| {
            let Some(c) = after.connections.iter().find(|c| c.id == *id) else {
                return false;
            };
            let unrouted = !c.routed && c.polyline.first() != c.polyline.last();
            unrouted || (c.routed && (breaks(*id) || bends(&now, *id) > bends(&was, *id)))
        })
        .collect();
    if jobs.is_empty() {
        return Vec::new();
    }
    route_page(&after, &jobs)
        .routes
        .iter()
        .map(|(id, route)| {
            let points: Vec<serde_json::Value> = route.iter().map(|p| json!([p.0, p.1])).collect();
            json!({"connection": id.to_string(), "route": points})
        })
        .collect()
}

/// `rep.move {reps, steps}` (plus `items` when page items or structure boxes move): the moved entities that are
/// representations go to `reps` (always present, because the command requires it), those that are page items or
/// structure boxes to `items`, each in `moved` order; `items` is serialised only when it is not empty, so a move of
/// representations alone serialises exactly as before.
pub fn move_command(tables: &PageTables, moved: &[EntityRef], steps: (i64, i64)) -> Box<AnyCommand> {
    let is_rep = |e: &EntityRef| tables.reps.iter().any(|r| r.id == e.0);
    let is_item = |e: &EntityRef| {
        tables.items.iter().any(|i| i.id == e.0) || tables.structure_boxes.iter().any(|b| b.id == e.0)
    };
    let reps: Vec<String> = moved
        .iter()
        .filter(|e| is_rep(e))
        .map(|e| e.0.to_string())
        .collect();
    let items: Vec<String> = moved
        .iter()
        .filter(|e| is_item(e))
        .map(|e| e.0.to_string())
        .collect();
    let mut params = json!({
        "reps": reps,
        "steps": [steps.0, steps.1],
    });
    if !items.is_empty() {
        params["items"] = json!(items);
    }
    Box::new(AnyCommand {
        name: "rep.move".into(),
        params,
    })
}
