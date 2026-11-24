// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The geometry of a black box's outline: its grips and the reshape they make, where a pin can stand on its edges, the handles a press takes, and the overlays of the grips and of a reshape.

use super::{BoxCpAt, cp_json};
use crate::key;
use crate::strings::Key;
use crate::tools::wire::PageGridMap;
use crate::tools::{MarkerKind, Overlay, OverlayItem, Style, Token, ToolCx, rect_of};
use ew_api::AnyCommand;
use ew_api::geometry::{Dir, orient_dir, orient_point};
use ew_api::output::TableRep;
use ew_dl::EntityRef;
use serde_json::{Value, json};

// -------------------------------------------------------------------------------------------------------------------
// Grips

/// The inverse of orientation `o` (`0..=7`): the quarter turns swap, the mirrored ones are their own inverse.
pub fn orient_inverse(o: u8) -> u8 {
    match o % 8 {
        1 => 3,
        3 => 1,
        other => other,
    }
}

/// The grip points of an outline (page nm): the vertices, then the midpoints of the edges `v0v1, …, v(n−1)v0`.
pub fn grip_points(outline: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let n = outline.len();
    let mut out = outline.to_vec();
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        out.push((a.0 + (b.0 - a.0) / 2, a.1 + (b.1 - a.1) / 2));
    }
    out
}

/// Whether an outline is an axis-aligned rectangle as the black box tool stores it (four vertices, `v0v1` and
/// `v2v3` horizontal, `v1v2` and `v3v0` vertical).
pub fn is_rectangle(o: &[(i64, i64)]) -> bool {
    o.len() == 4 && o[0].1 == o[1].1 && o[1].0 == o[2].0 && o[2].1 == o[3].1 && o[3].0 == o[0].0
}

/// Twice the signed area of a polygon (positive: clockwise on the sheet, whose y axis points down).
pub fn signed_area2(poly: &[(i64, i64)]) -> i128 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            i128::from(a.0) * i128::from(b.1) - i128::from(b.0) * i128::from(a.1)
        })
        .sum()
}

/// The outline after grip `grip` moved to `to` (page nm, absolute). Errors: `reason-grip-not-axis` for an edge grip
/// of an edge that is neither horizontal nor vertical; `reason-box-too-small` for an outline without area;
/// `reason-no-grip` for a grip the outline does not have.
pub fn reshape_by_grip(
    outline: &[(i64, i64)],
    grip: usize,
    to: (i64, i64),
) -> Result<Vec<(i64, i64)>, Key<'static>> {
    let n = outline.len();
    if n < 3 || grip >= 2 * n {
        return Err(key!("reason-no-grip"));
    }
    let mut out = outline.to_vec();
    if grip < n {
        let i = grip;
        if is_rectangle(outline) {
            for j in [(i + n - 1) % n, (i + 1) % n] {
                if outline[j].0 == outline[i].0 {
                    out[j].0 = to.0;
                }
                if outline[j].1 == outline[i].1 {
                    out[j].1 = to.1;
                }
            }
        }
        out[i] = to;
    } else {
        let i = grip - n;
        let j = (i + 1) % n;
        let (a, b) = (outline[i], outline[j]);
        if a.1 == b.1 && a.0 != b.0 {
            out[i].1 = to.1;
            out[j].1 = to.1;
        } else if a.0 == b.0 && a.1 != b.1 {
            out[i].0 = to.0;
            out[j].0 = to.0;
        } else {
            return Err(key!("reason-grip-not-axis"));
        }
    }
    let (before, after) = (signed_area2(outline), signed_area2(&out));
    if after == 0 {
        return Err(key!("reason-box-too-small"));
    }
    // A grip never turns the box inside out (its CPs would then point inwards).
    if before.signum() != after.signum() {
        return Err(key!("reason-box-inside-out"));
    }
    Ok(out)
}

/// Whether the pins `cps` of the box `outline` (page nm) still fit when its outline becomes `new` (the same vertices
/// moved), on the edges along x (`along_x`) or along y: an axis-aligned edge whose extent along its axis changed holds
/// its pins one grid step apart and one step to spare at each corner, as `box.reshape` re-centres them. A pin belongs to
/// the edge it lies on, at a corner to the one it leaves by
/// its direction.
pub(super) fn pins_fit(
    grid: PageGridMap,
    outline: &[(i64, i64)],
    cps: &[BoxCpAt],
    new: &[(i64, i64)],
    along_x: bool,
) -> bool {
    let n = outline.len();
    let area = signed_area2(outline);
    (0..n).all(|i| {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let (na, nb) = (new[i], new[(i + 1) % n]);
        let horizontal = na.1 == nb.1 && na.0 != nb.0;
        let axis = |p: (i64, i64)| if horizontal { p.0 } else { p.1 };
        if horizontal != along_x || (na.0 != nb.0 && na.1 != nb.1) {
            return true;
        }
        if (axis(a).min(axis(b)), axis(a).max(axis(b))) == (axis(na).min(axis(nb)), axis(na).max(axis(nb))) {
            return true;
        }
        let mut at: Vec<i64> = cps
            .iter()
            .filter(|(_, p, dir)| {
                let edge = |j: usize| on_outline(&[outline[j], outline[(j + 1) % n]], *p);
                let leaves = |j: usize| outward(outline[j], outline[(j + 1) % n], area) == Some(*dir);
                let mine = (0..n).filter(|j| edge(*j)).collect::<Vec<_>>();
                mine.iter().find(|j| leaves(**j)).or(mine.first()) == Some(&i)
            })
            .map(|(_, p, _)| axis(*p))
            .collect();
        at.sort_unstable();
        at.dedup();
        let steps = (grid.index(axis(nb)) - grid.index(axis(na))).abs();
        let n = i64::try_from(at.len()).unwrap_or(i64::MAX / 4);
        at.is_empty() || steps > n
    })
}

/// Where a grip stops and the outline there ([`grip_stop`]).
pub type GripStop = ((i64, i64), Vec<(i64, i64)>);

/// Where grip `grip` of the box `outline` with the pins `cps` stops when dragged to the grid point `to` (page nm), and
/// the outline there: at `to`, else, where the pins would not fit ([`pins_fit`]) or the box would be narrower or lower
/// than `room` (its least width and height in grid steps, [`TableBox::room`](ew_api::output::TableBox::room)) while it
/// gets narrower or lower (the pin texts of two opposite sides would touch), as near to `to` as they fit, moved back
/// towards the grip's own point a grid step at a time on each axis that is too short. Errors as [`reshape_by_grip`].
pub fn grip_stop(
    grid: PageGridMap,
    outline: &[(i64, i64)],
    cps: &[BoxCpAt],
    room: (i64, i64),
    grip: usize,
    to: (i64, i64),
) -> Result<GripStop, Key<'static>> {
    let extent = |o: &[(i64, i64)], along_x: bool| {
        let vs = o.iter().map(|p| if along_x { p.0 } else { p.1 });
        vs.clone().max().zip(vs.min()).map_or(0, |(a, b)| a - b)
    };
    // Whether `new` gets narrower (along x) or lower than the pin texts of the two sides across it need.
    let short = |new: &[(i64, i64)], along_x: bool| {
        let least = if along_x { room.0 } else { room.1 };
        extent(new, along_x) < extent(outline, along_x)
            && extent(new, along_x) < least.saturating_mul(grid.step())
    };
    let home = grip_points(outline).get(grip).copied().unwrap_or(to);
    let mut at = to;
    let mut new = reshape_by_grip(outline, grip, at)?;
    for along_x in [true, false] {
        let toward = |v: i64, h: i64| {
            let k = grid.index(v);
            grid.coord(if h > v { k + 1 } else { k - 1 })
        };
        while !pins_fit(grid, outline, cps, &new, along_x) || short(&new, along_x) {
            let (v, h) = if along_x { (at.0, home.0) } else { (at.1, home.1) };
            if v == h {
                break;
            }
            let next = toward(v, h);
            // Never past the grip's own point.
            let next = if (h - v).abs() < (next - v).abs() { h } else { next };
            if along_x {
                at.0 = next;
            } else {
                at.1 = next;
            }
            new = reshape_by_grip(outline, grip, at)?;
        }
    }
    Ok((at, new))
}

/// The outward normal of edge `a`–`b` of a polygon with the signed area `area2`; `None` for an edge that is not
/// axis-aligned.
pub fn outward(a: (i64, i64), b: (i64, i64), area2: i128) -> Option<Dir> {
    let v = ((b.0 - a.0).signum(), (b.1 - a.1).signum());
    if (v.0 != 0 && v.1 != 0) || v == (0, 0) || area2 == 0 {
        return None;
    }
    let n = if area2 > 0 { (v.1, -v.0) } else { (-v.1, v.0) };
    Dir::of_vector(n)
}

/// The grid point on the outline (page nm) nearest to `p`, with the edge's outward normal: per axis-aligned edge the
/// point projected, clamped and snapped along it, a grid step clear of both corners where the edge is long enough (a
/// pin never stands on a corner); ties by the smaller edge index. `None` without such an edge.
pub fn nearest_on_outline(
    grid: PageGridMap,
    outline: &[(i64, i64)],
    p: (i64, i64),
) -> Option<((i64, i64), Dir)> {
    let n = outline.len();
    let area = signed_area2(outline);
    let snap_between = |t: i64, a: i64, b: i64| {
        let (lo, hi) = (a.min(b), a.max(b));
        let (inner_lo, inner_hi) = (grid.coord(grid.index(lo) + 1), grid.coord(grid.index(hi) - 1));
        let (lo, hi) = if inner_lo <= inner_hi && lo < inner_lo && inner_hi < hi {
            (inner_lo, inner_hi)
        } else {
            (lo, hi)
        };
        let t = t.clamp(lo, hi);
        let k = grid.index(t);
        [k - 1, k, k + 1]
            .into_iter()
            .map(|k| grid.coord(k))
            .filter(|c| (lo..=hi).contains(c))
            .min_by_key(|c| ((c - t).abs(), (c - a).abs()))
            .unwrap_or(a)
    };
    let mut best: Option<(i128, usize, (i64, i64), Dir)> = None;
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let q = if a.1 == b.1 && a.0 != b.0 {
            (snap_between(p.0, a.0, b.0), a.1)
        } else if a.0 == b.0 && a.1 != b.1 {
            (a.0, snap_between(p.1, a.1, b.1))
        } else {
            continue;
        };
        let Some(dir) = outward(a, b, area) else {
            continue;
        };
        let d = crate::tools::dist2(q, p);
        if best.is_none_or(|(bd, bi, ..)| d < bd || (d == bd && i < bi)) {
            best = Some((d, i, q, dir));
        }
    }
    best.map(|(_, _, q, d)| (q, d))
}

/// A pin's point on the page and the direction a wire leaves it.
pub(super) type PinAt = ((i64, i64), Dir);

/// Where a drag of pin `idx` of the box `outline` with the pins `cps` (page nm) to `p` puts it, with the direction it
/// leaves in (a pin dragged on the smallest box went round its corner onto the next side): a grid point at least
/// one step from both corners of an axis-aligned edge, no other pin's (read settled), nearest `p`. The pin keeps its
/// own side while `p` lies across from it (between that side's corners, them included); beyond them, the nearest side
/// that has a free point. A side too short for a point clear of its corners takes none. `None` where no side has a free point.
pub fn pin_target(
    grid: PageGridMap,
    outline: &[(i64, i64)],
    cps: &[BoxCpAt],
    idx: u16,
    p: (i64, i64),
) -> Option<((i64, i64), Dir)> {
    let n = outline.len();
    let area = signed_area2(outline);
    let settle = |v: i64| grid.coord(grid.index(v));
    let taken = |q: (i64, i64)| {
        cps.iter()
            .any(|c| c.0 != idx && settle(c.1.0) == settle(q.0) && settle(c.1.1) == settle(q.1))
    };
    let me = cps.iter().find(|c| c.0 == idx).map(|c| c.1);
    // Each axis-aligned edge: its free points clear of the corners, and whether it holds the pin now.
    let mut best: Option<(bool, i128, usize, PinAt)> = None;
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let along_x = a.1 == b.1 && a.0 != b.0;
        if !along_x && !(a.0 == b.0 && a.1 != b.1) {
            continue;
        }
        let Some(dir) = outward(a, b, area) else {
            continue;
        };
        let (lo, hi) = if along_x {
            (a.0.min(b.0), a.0.max(b.0))
        } else {
            (a.1.min(b.1), a.1.max(b.1))
        };
        let (k_lo, k_hi) = (grid.index(lo) + 1, grid.index(hi) - 1);
        if k_lo > k_hi {
            continue;
        }
        let point = |k: i64| {
            if along_x {
                (grid.coord(k), a.1)
            } else {
                (a.0, grid.coord(k))
            }
        };
        let own = me.is_some_and(|m| on_outline(&[a, b], m));
        let across = if along_x { p.0 } else { p.1 };
        let k = grid.index(across.clamp(grid.coord(k_lo), grid.coord(k_hi)));
        // The free point nearest the pointer along the edge: outward from the nearest grid point.
        let hit = (0..=(k_hi - k_lo))
            .flat_map(|d| [k - d, k + d])
            .filter(|k| (k_lo..=k_hi).contains(k))
            .map(point)
            .find(|q| !taken(*q));
        let Some(q) = hit else { continue };
        // The pin's own side wins while the pointer lies across from it, its corners included (the pointer snaps to
        // the grid, so a pointer just short of a corner reads as on it).
        let keep = own && (lo..=hi).contains(&across);
        let d = crate::tools::dist2(q, p);
        let better = match &best {
            None => true,
            Some((bk, bd, bi, ..)) => (keep, -d, std::cmp::Reverse(i)) > (*bk, -*bd, std::cmp::Reverse(*bi)),
        };
        if better {
            best = Some((keep, d, i, (q, dir)));
        }
    }
    best.map(|(.., at)| at)
}

/// The new pin a wire ending at the grid point `p` on the box `b` would take: on a box whose pins are its own,
/// `p` on an axis-aligned edge, no corner and no pin's point (read settled); the smallest index the box does not use,
/// leaving along the edge's outward normal. `None` anywhere else.
pub fn free_edge_pin(grid: PageGridMap, b: &ew_api::output::TableBox, p: (i64, i64)) -> Option<BoxCpAt> {
    if !b.own_pins {
        return None;
    }
    let settle = |q: (i64, i64)| (grid.index(q.0), grid.index(q.1));
    if b.outline.iter().any(|v| settle(*v) == settle(p)) || b.cps.iter().any(|c| settle(c.1) == settle(p)) {
        return None;
    }
    let n = b.outline.len();
    let area = signed_area2(&b.outline);
    let dir = (0..n).find_map(|i| {
        let (a, c) = (b.outline[i], b.outline[(i + 1) % n]);
        on_outline(&[a, c], p).then(|| outward(a, c, area)).flatten()
    })?;
    let idx = (0..=u16::MAX).find(|i| !b.cps.iter().any(|c| c.0 == *i))?;
    Some((idx, p, dir))
}

/// Whether `p` lies on an edge of the closed outline (exact).
pub fn on_outline(outline: &[(i64, i64)], p: (i64, i64)) -> bool {
    let n = outline.len();
    (0..n).any(|i| {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let cross =
            i128::from(b.0 - a.0) * i128::from(p.1 - a.1) - i128::from(b.1 - a.1) * i128::from(p.0 - a.0);
        cross == 0 && p.0 >= a.0.min(b.0) && p.0 <= a.0.max(b.0) && p.1 >= a.1.min(b.1) && p.1 <= a.1.max(b.1)
    })
}

/// The `box.reshape` of black box `rep` with the absolute outline `outline` and, when given, the absolute CPs
/// `(idx, point, direction)` (page nm), converted to the representation's frame (relative to its position, under the
/// inverse of its orientation).
pub fn reshape_command(rep: &TableRep, outline: &[(i64, i64)], cps: Option<&[BoxCpAt]>) -> Box<AnyCommand> {
    let names = rep
        .black_box
        .as_ref()
        .map(|b| b.names.clone())
        .unwrap_or_default();
    reshape_named(rep, outline, cps, &names)
}

/// [`reshape_command`] with the box's own pins named by `names` (function CP index, name): a pin moved, added,
/// renamed or kept keeps the name it has there.
pub fn reshape_named(
    rep: &TableRep,
    outline: &[(i64, i64)],
    cps: Option<&[BoxCpAt]>,
    names: &[(u16, String)],
) -> Box<AnyCommand> {
    let inv = orient_inverse(rep.orient);
    let rel = |p: (i64, i64)| orient_point(inv, (p.0 - rep.pos.0, p.1 - rep.pos.1));
    let mut params = json!({
        "rep": rep.id.to_string(),
        "outline": outline.iter().map(|p| { let r = rel(*p); vec![r.0, r.1] }).collect::<Vec<_>>(),
    });
    if let Some(cps) = cps {
        params["cps"] = Value::Array(
            cps.iter()
                .map(|(i, p, d)| {
                    let mut cp = cp_json(*i, rel(*p), orient_dir(inv, *d));
                    if let Some((_, name)) = names.iter().find(|(j, _)| j == i) {
                        cp["name"] = Value::from(name.as_str());
                    }
                    cp
                })
                .collect(),
        );
    }
    Box::new(AnyCommand {
        name: "box.reshape".into(),
        params,
    })
}

/// The black box `e` of the page's tables, when it is one.
pub fn black_box_rep<'t>(cx: &'t ToolCx<'_>, e: EntityRef) -> Option<&'t TableRep> {
    cx.page
        .tables
        .reps
        .iter()
        .find(|r| r.id == e.0 && r.black_box.is_some())
}

/// The selection's one black box, when exactly one entity is selected and it is a black box.
pub fn selected_black_box<'t>(cx: &'t ToolCx<'_>) -> Option<&'t TableRep> {
    match cx.selection.items.as_slice() {
        [one] => black_box_rep(cx, *one),
        _ => None,
    }
}

/// What a press on a selected box takes ([`handle_at`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoxHandle {
    /// Grip `k` ([`grip_points`]).
    Grip(usize),
    /// The pin of this function CP index.
    Pin(u16),
}

/// The handle of the selected box `rep` under the sheet point `at`: a corner grip within [`crate::pointer::GRIP_SLOP_PT`], else a pin
/// within [`crate::pointer::HOVER_SLOP_PT`], else an edge's middle grip within [`crate::pointer::GRIP_SLOP_PT`]; the nearest of each
/// kind, ties by index.
pub fn handle_at(cx: &ToolCx<'_>, rep: &TableRep, at: (i64, i64)) -> Option<BoxHandle> {
    let b = rep.black_box.as_ref()?;
    let n = b.outline.len();
    let near = |p: (i64, i64), slop: f64| {
        let s = cx.snapper.page_to_sheet(p);
        cx.within(s, at, slop).then(|| crate::tools::dist2(s, at))
    };
    let grips = grip_points(&b.outline);
    let best = |range: std::ops::Range<usize>| {
        range
            .filter_map(|k| near(grips[k], crate::pointer::GRIP_SLOP_PT).map(|d| (d, k)))
            .min()
            .map(|(_, k)| k)
    };
    if let Some(k) = best(0..n) {
        return Some(BoxHandle::Grip(k));
    }
    if let Some((_, idx)) = b
        .cps
        .iter()
        .filter_map(|(idx, p, _)| near(*p, crate::pointer::HOVER_SLOP_PT).map(|d| (d, *idx)))
        .min()
    {
        return Some(BoxHandle::Pin(idx));
    }
    best(n..2 * n).map(BoxHandle::Grip)
}

/// Which way grip `grip` of a box `outline` resizes it, for the pointer's shape before the press.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GripResize {
    /// A vertical edge's middle: sideways.
    Sideways,
    /// A horizontal edge's middle: up and down.
    UpDown,
    /// The top-left or the bottom-right corner.
    CornerNwSe,
    /// The top-right or the bottom-left corner.
    CornerNeSw,
}

/// The [`GripResize`] of grip `grip` of `outline`; `None` for an edge that is neither horizontal nor vertical.
pub fn grip_resize(outline: &[(i64, i64)], grip: usize) -> Option<GripResize> {
    let n = outline.len();
    if grip < n {
        let (xs, ys) = (outline.iter().map(|p| p.0), outline.iter().map(|p| p.1));
        let mid = (
            (xs.clone().min()? + xs.max()?) / 2,
            (ys.clone().min()? + ys.max()?) / 2,
        );
        let p = outline[grip];
        return Some(if (p.0 < mid.0) == (p.1 < mid.1) {
            GripResize::CornerNwSe
        } else {
            GripResize::CornerNeSw
        });
    }
    let i = grip.checked_sub(n)?;
    let (a, b) = (*outline.get(i)?, outline[(i + 1) % n]);
    if a.1 == b.1 {
        Some(GripResize::UpDown)
    } else if a.0 == b.0 {
        Some(GripResize::Sideways)
    } else {
        None
    }
}

/// The overlay of a selected black box: the wire's round handle on each grip, hot under the pointer, and a dashed
/// square per CP, 3 points across each half (shapes, not colour, tell them apart; NFR-A11Y-03).
pub fn grips_overlay(cx: &ToolCx<'_>, rep: &TableRep, pointer: Option<(i64, i64)>) -> Overlay {
    let Some(b) = &rep.black_box else {
        return Overlay::default();
    };
    let hot = pointer.and_then(|at| handle_at(cx, rep, at));
    let r = cx.nm_of_points(3.0).round() as i64;
    let square = |p: (i64, i64), style: Style| {
        let s = cx.snapper.page_to_sheet(p);
        OverlayItem::Rect {
            r: rect_of((s.0 - r, s.1 - r), (s.0 + r, s.1 + r)),
            style,
        }
    };
    let mut items: Vec<OverlayItem> = grip_points(&b.outline)
        .into_iter()
        .enumerate()
        .map(|(k, p)| OverlayItem::Grip {
            at: cx.snapper.page_to_sheet(p),
            hot: hot == Some(BoxHandle::Grip(k)),
        })
        .collect();
    items.extend(
        b.cps
            .iter()
            .map(|(_, p, _)| square(*p, Style::dashed(Token::Ghost))),
    );
    Overlay::new(items)
}

/// The overlay of a reshape in progress: the new outline dashed and its CPs.
pub fn reshape_overlay(cx: &ToolCx<'_>, outline: &[(i64, i64)], cps: &[(i64, i64)]) -> Overlay {
    let mut pts: Vec<(i64, i64)> = outline.iter().map(|p| cx.snapper.page_to_sheet(*p)).collect();
    if let Some(&first) = pts.first() {
        pts.push(first);
    }
    let mut items = vec![OverlayItem::Polyline {
        points: pts,
        style: Style::dashed(Token::Ghost),
    }];
    items.extend(cps.iter().map(|p| OverlayItem::Marker {
        at: cx.snapper.page_to_sheet(*p),
        kind: MarkerKind::Cp,
    }));
    Overlay::new(items)
}
