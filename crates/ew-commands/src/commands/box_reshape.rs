// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `box.reshape` (`ReshapeBox`, FR-DEV-039, §9.4): a black box's outline and connection points
//! changed, its connections kept.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_schema::{BoxCp, BoxSpec, Connection, Dir4, Endpoint, Function, Kind, Page, Representation};
use ew_units::{Grid, Nm, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::commands::connection::follow_moved_cps;
use crate::error::CmdError;
use crate::geometry::{OutOfRange, add, on_grid, on_segment, point_in_range, require_on_grid, sub};
use crate::meta::CommandMeta;
use crate::proxy::{Pt2S, UuidS};
use crate::text::text;
use crate::tx::TxBuilder;

/// `box.reshape` (`ReshapeBox`, FR-DEV-039): replaces the outline of a black box (a representation with a `box`),
/// and its connection points with `cps`, or re-projects them onto the new outline.
///
/// - `outline` is relative to the representation's `pos` (under its orientation, as `box.black` stores it): at
///   least three vertices, each on the page's connection grid in absolute terms (`pos + orient(v)`).
/// - **With `cps`** the box's CPs are replaced: function CP indices distinct, each CP on the grid in absolute terms
///   and on the outline (on an edge `v(i) v(i+1)`, the outline closed).
/// - **Without `cps`** every CP is re-projected (doc 06 §8 item 13): it keeps the index of the edge it lay on
///   (the first edge containing it, in outline order; on a corner the edge it leaves by its direction) and its
///   direction. The CPs of one edge keep their order and
///   their spacing and stand centred on the new edge as a group, on grid points, at least one grid step from both
///   corners (half a step towards the edge's smaller coordinate when they cannot be centred exactly); where the edge
///   is shorter than they need, the spacing closes up, each gap to at most the largest whole number of steps that
///   lets them fit, down to one step. An edge whose extent along its axis did not change keeps its CPs where they
///   are along it (the edge may have moved across), unless `centre` asks for every edge to be centred (Centre the
///   pins). A CP whose edge index no longer exists, or that lay on no edge, moves to the nearest grid point on the
///   nearest remaining edge (ties by the smaller edge index) and leaves along that edge's outward normal, listed as a
///   notice. On an edge that is neither horizontal nor vertical the grid point is the edge's first vertex (polygon
///   outlines are a follow-up), also listed as a notice.
/// - The representation's position becomes the outline's smallest corner (its top-left one unturned): the outline,
///   the CPs and the inner graphics are stored relative to it, so the box's texts, which stand relative to the
///   position, keep their place relative to that corner when the box is made smaller or larger from its left or top.
/// - Inner graphics keep their place on the page. Routes of the connections on the page at the box's CPs follow each
///   moved CP (`geometry::follow_end`).
///
/// Rejected: a representation that does not exist or has no box; fewer than three outline vertices or one off the
/// grid; with `cps`, two CPs with one index, a CP off the grid or off the outline, or one moved onto a corner of the
/// outline; without `cps`, an edge too short for the CPs it carries (one grid step apart and one to spare at each
/// corner); a box CP that a connection on the page ends at and that the new CPs no longer draw (keep it in `cps`).
/// Interactive. Outcome: the representation.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReshapeBox {
    /// The black box's representation.
    #[schemars(with = "UuidS")]
    pub rep: Id<Representation>,
    /// The new outline, relative to the representation's position.
    #[schemars(with = "Vec<Pt2S>")]
    pub outline: Vec<Pt2>,
    /// The new connection points; absent re-projects the current ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cps: Option<Vec<BoxCp>>,
    /// Without `cps`: centre the CPs of every edge, also of an edge whose extent did not change (Centre the pins).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub centre: bool,
}

/// The direction of a unit vector.
fn dir_of_vector(v: (i64, i64)) -> Option<Dir4> {
    match v {
        (0, -1) => Some(Dir4::Up),
        (1, 0) => Some(Dir4::Right),
        (0, 1) => Some(Dir4::Down),
        (-1, 0) => Some(Dir4::Left),
        _ => None,
    }
}

/// The grid coordinate inside `[min(a, b), max(a, b)]` nearest to `target`, ties toward `a`. `a` and `b` are grid
/// coordinates, so one exists.
fn snap_between(grid: Grid, target: i64, a: i64, b: i64) -> i64 {
    let (lo, hi) = (a.min(b), a.max(b));
    let t = target.clamp(lo, hi);
    let k = grid.snap(Nm(t));
    let mut best = a;
    for c in [k - 1, k, k + 1].into_iter().filter_map(|k| grid.at(k)) {
        let c = c.0;
        if c < lo || c > hi || !on_grid(grid, Nm(c)) {
            continue;
        }
        let (d, bd) = ((c - t).abs(), (best - t).abs());
        if d < bd || (d == bd && (c - a).abs() < (best - a).abs()) {
            best = c;
        }
    }
    best
}

/// The grid point on the edge `a`–`b` nearest to `p` (an axis-aligned edge: `p` projected, clamped and snapped
/// along it); the edge's first vertex on another edge.
fn nearest_on_edge(grid: Grid, a: Pt2, b: Pt2, p: Pt2) -> Pt2 {
    if a.y == b.y {
        Pt2::new(Nm(snap_between(grid, p.x.0, a.x.0, b.x.0)), a.y)
    } else if a.x == b.x {
        Pt2::new(a.x, Nm(snap_between(grid, p.y.0, a.y.0, b.y.0)))
    } else {
        a
    }
}

/// The squared distance of two points in `i128`.
fn dist2(a: Pt2, b: Pt2) -> i128 {
    let dx = i128::from(a.x.0) - i128::from(b.x.0);
    let dy = i128::from(a.y.0) - i128::from(b.y.0);
    dx * dx + dy * dy
}

/// Twice the signed area of a polygon (positive: clockwise on the sheet, whose y axis points down).
fn signed_area2(poly: &[Pt2]) -> i128 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            i128::from(a.x.0) * i128::from(b.y.0) - i128::from(b.x.0) * i128::from(a.y.0)
        })
        .sum()
}

/// The outward normal of edge `a`–`b` of a polygon with the signed area `area2`; `None` for an edge that is not
/// axis-aligned or a degenerate polygon.
fn outward(a: Pt2, b: Pt2, area2: i128) -> Option<Dir4> {
    let v = ((b.x.0 - a.x.0).signum(), (b.y.0 - a.y.0).signum());
    if (v.0 != 0 && v.1 != 0) || v == (0, 0) || area2 == 0 {
        return None;
    }
    // Clockwise on the sheet (y down): the outside is to the left of the travel, (x, y) → (y, −x).
    let n = if area2 > 0 { (v.1, -v.0) } else { (-v.1, v.0) };
    dir_of_vector(n)
}

/// The edges `(i, a, b)` of a closed polygon.
fn edges(poly: &[Pt2]) -> impl Iterator<Item = (usize, Pt2, Pt2)> + '_ {
    let n = poly.len();
    (0..n).map(move |i| (i, poly[i], poly[(i + 1) % n]))
}

/// The grid coordinates of CPs that stood at `at` (along one axis, in their order along it) centred as a group on the
/// edge from `a` to `b` (coordinates along the same axis), on grid points at least one step inside both ends: the
/// gaps kept, else each closed up to the largest whole number of steps that lets them fit, down to one step; `None`
/// when even one step apart they do not fit.
fn centred(grid: Grid, a: i64, b: i64, at: &[i64]) -> Option<Vec<i64>> {
    let (lo, hi) = (a.min(b), a.max(b));
    let mut first = grid.snap(Nm(lo));
    if grid.at(first)?.0 < lo {
        first += 1;
    }
    let mut last = grid.snap(Nm(hi));
    if grid.at(last)?.0 > hi {
        last -= 1;
    }
    // The steps between the first and the last CP's places, one step inside each corner.
    let room = last.checked_sub(first)?.checked_sub(2)?;
    let ks: Vec<i64> = at.iter().map(|x| grid.snap(Nm(*x))).collect();
    let gaps: Vec<i64> = ks.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
    let floor = 1;
    // A gap closed up to `cap`, and opened to one step where it is narrower (two CPs on one point).
    let gap = |g: i64, cap: i64| g.min(cap).max(floor);
    let fits = |cap: i64| gaps.iter().map(|g| gap(*g, cap)).sum::<i64>() <= room;
    let mut cap = gaps.iter().copied().max().unwrap_or(0).max(floor);
    if !fits(cap) {
        if !fits(floor) {
            return None;
        }
        // The largest cap that fits, by halving the interval (the gaps can be long on a long edge).
        let (mut ok, mut bad) = (floor, cap);
        while bad - ok > 1 {
            let mid = ok + (bad - ok) / 2;
            if fits(mid) {
                ok = mid;
            } else {
                bad = mid;
            }
        }
        cap = ok;
    }
    let span: i64 = gaps.iter().map(|g| gap(*g, cap)).sum();
    let mut k = first + 1 + (room - span) / 2;
    let mut out = Vec::with_capacity(at.len());
    for i in 0..at.len() {
        if i > 0 {
            k += gap(gaps[i - 1], cap);
        }
        out.push(grid.at(k)?.0);
    }
    Some(out)
}

/// `shape` moved by `d`.
fn shift_primitive(shape: &mut ew_schema::Primitive, d: Pt2) {
    use ew_schema::Primitive as P;
    let bounds = |r: &mut ew_schema::Bounds| r.0 = ew_units::Rect::from_points(r.0.min() + d, r.0.max() + d);
    match shape {
        P::Line(l) => {
            l.from = l.from + d;
            l.to = l.to + d;
        }
        P::Polyline(p) => p.points.iter_mut().for_each(|q| *q = *q + d),
        P::Polygon(p) => p.points.iter_mut().for_each(|q| *q = *q + d),
        P::Rect(r) => bounds(&mut r.rect),
        P::Ellipse(e) => bounds(&mut e.rect),
        P::Arc(a) => bounds(&mut a.rect),
        P::Image(i) => bounds(&mut i.rect),
        P::Circle(c) => c.centre = c.centre + d,
        P::Text(t) => t.pos = t.pos + d,
        P::Unknown(_) => {}
    }
}

impl Command for ReshapeBox {
    const NAME: &'static str = "box.reshape";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let rep = tx.require::<Representation>(self.rep)?;
        let Some(old) = rep.box_.clone() else {
            return tx.reject(text!("refusal-box-reshape-no-box"));
        };
        // A valve terminal drawn as its positions takes its size from them.
        if old.slots.is_some() {
            return tx.reject(text!("refusal-box-reshape-slots"));
        }
        let page = tx.require::<Page>(rep.page)?;
        let grid = page.grid;
        if self.outline.len() < 3 {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/outline",
                text!("refusal-box-reshape-few-vertices"),
            ));
        }
        let out_of_range = |tx: &TxBuilder<'_>| -> Result<Outcome, CmdError> {
            tx.reject(text!("refusal-box-reshape-out-of-range"))
        };
        // Absolute page points of relative box points (`pos + orient(v)`), and back.
        let abs = |v: Pt2| -> Result<Pt2, OutOfRange> {
            if !point_in_range(v) {
                return Err(OutOfRange);
            }
            add(rep.pos, rep.orient.apply(v))
        };
        // A pin's or a corner's point as the page draws it: on the grid's own number where it lies within 1 nm of a
        // grid position (`Grid::settle_point`, docs/design/06a-drafting.md §2.1).
        let drawn = |v: Pt2| abs(v).map(|p| grid.settle_point(p));
        let inverse = rep.orient.inverse();
        let rel = |p: Pt2| -> Result<Pt2, OutOfRange> { sub(p, rep.pos).map(|d| inverse.apply(d)) };
        let mut new_abs: Vec<Pt2> = Vec::with_capacity(self.outline.len());
        for (i, v) in self.outline.iter().enumerate() {
            let Ok(p) = abs(*v) else { return out_of_range(tx) };
            // A vertex of the old outline is untouched and keeps its place, also off the grid (a drifted box)
            if !old.outline.contains(v) {
                require_on_grid(tx, rep.page, p, &format!("/outline/{i}"))?;
            }
            new_abs.push(p);
        }
        let mut old_abs: Vec<Pt2> = Vec::with_capacity(old.outline.len());
        for v in &old.outline {
            let Ok(p) = abs(*v) else { return out_of_range(tx) };
            old_abs.push(p);
        }
        let mut notices: Vec<String> = Vec::new();
        let cps: Vec<BoxCp> = match &self.cps {
            Some(cps) => {
                let mut seen = BTreeSet::new();
                for (i, cp) in cps.iter().enumerate() {
                    if !seen.insert(cp.idx) {
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            format!("/cps/{i}/idx"),
                            text!("refusal-box-cp-index-twice"),
                        ));
                    }
                    let Ok(p) = abs(cp.pos) else {
                        return out_of_range(tx);
                    };
                    // A pin the box has is untouched and keeps its place, as an untouched vertex does: also 1 nm
                    // beside the grid's number of its position (the box's place plus the pin's offset, each rounded,
                    // in a file of an earlier candidate). A pin that is new or moved stands on the grid.
                    let kept = old.cps.iter().any(|c| c.idx == cp.idx && c.pos == cp.pos);
                    if !kept {
                        require_on_grid(tx, rep.page, p, &format!("/cps/{i}/pos"))?;
                    }
                    if !edges(&new_abs).any(|(_, a, b)| on_segment(p, a, b)) {
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            format!("/cps/{i}/pos"),
                            text!("refusal-box-reshape-cp-off-outline"),
                        ));
                    }
                    // Two pins never stand on one point (a dragged pin landed on its neighbour's).
                    let at = grid.settle_point(p);
                    let twice = cps[..i].iter().any(|o| drawn(o.pos) == Ok(at));
                    if twice {
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            format!("/cps/{i}/pos"),
                            text!("refusal-box-reshape-cp-shared"),
                        ));
                    }
                    // A pin never stands on a corner; one stored there stays until it is moved.
                    if !kept && self.outline.iter().any(|v| drawn(*v) == Ok(at)) {
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            format!("/cps/{i}/pos"),
                            text!("refusal-box-reshape-cp-on-corner"),
                        ));
                    }
                }
                // A box's own pins have names; a box of a part draws the part's pins, edited in its device box.
                if let Some(i) = cps.iter().position(|c| c.name.is_some()) {
                    let of_part = rep
                        .functions
                        .first()
                        .and_then(|f| tx.get::<Function>(*f))
                        .and_then(|f| f.device)
                        .and_then(|d| tx.get::<ew_schema::Device>(d))
                        .is_some_and(|d| !d.parts.is_empty());
                    if of_part {
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            format!("/cps/{i}/name"),
                            text!("refusal-box-pin-of-part"),
                        ));
                    }
                }
                cps.clone()
            }
            None => {
                let area = signed_area2(&new_abs);
                let old_area = signed_area2(&old_abs);
                // Each CP's new point and direction; the CPs of an axis-aligned edge are placed as a group below.
                let mut placed: Vec<Option<(Pt2, Dir4)>> = vec![None; old.cps.len()];
                let mut groups: BTreeMap<usize, Vec<(usize, Pt2)>> = BTreeMap::new();
                for (k, cp) in old.cps.iter().enumerate() {
                    let Ok(p) = abs(cp.pos) else {
                        return out_of_range(tx);
                    };
                    // Of two edges at a corner, the one the CP leaves by its direction.
                    let containing: Vec<(usize, Pt2, Pt2)> = edges(&old_abs)
                        .filter(|(_, a, b)| on_segment(p, *a, *b))
                        .collect();
                    let leaves = |a: Pt2, b: Pt2| {
                        outward(a, b, old_area).and_then(|d| ew_derive::oriented_dir(&d, inverse))
                            == Some(cp.dir.clone())
                    };
                    let edge = containing
                        .iter()
                        .find(|(_, a, b)| leaves(*a, *b))
                        .or(containing.first())
                        .copied();
                    match edge {
                        Some((i, ..)) if i < new_abs.len() => {
                            let (na, nb) = (new_abs[i], new_abs[(i + 1) % new_abs.len()]);
                            if na.x == nb.x || na.y == nb.y {
                                groups.entry(i).or_default().push((k, p));
                            } else {
                                notices.push(format!(
                                    "connection point {} moved to the first vertex of edge {i}, which is not \
                                     horizontal or vertical",
                                    cp.idx
                                ));
                                placed[k] = Some((na, cp.dir.clone()));
                            }
                        }
                        _ => {
                            let (j, na, nb) = edges(&new_abs)
                                .min_by_key(|(j, a, b)| (dist2(p, nearest_on_edge(grid, *a, *b, p)), *j))
                                .unwrap_or((0, new_abs[0], new_abs[1]));
                            let q = nearest_on_edge(grid, na, nb, p);
                            let dir = outward(na, nb, area)
                                .and_then(|d| ew_derive::oriented_dir(&d, inverse))
                                .unwrap_or_else(|| cp.dir.clone());
                            notices.push(match edge {
                                Some((i, ..)) => format!(
                                    "connection point {} moved to edge {j}: its edge {i} no longer exists",
                                    cp.idx
                                ),
                                None => format!(
                                    "connection point {} moved to edge {j}: it lay on no edge of the outline",
                                    cp.idx
                                ),
                            });
                            placed[k] = Some((q, dir));
                        }
                    }
                }
                for (i, mut group) in groups {
                    let (a, b) = (old_abs[i], old_abs[(i + 1) % old_abs.len()]);
                    let (na, nb) = (new_abs[i], new_abs[(i + 1) % new_abs.len()]);
                    // Along x on a horizontal edge, else along y.
                    let along_x = na.y == nb.y && na.x != nb.x;
                    let coord = |p: Pt2| if along_x { p.x.0 } else { p.y.0 };
                    let onto = |c: i64| {
                        if along_x {
                            Pt2::new(Nm(c), na.y)
                        } else {
                            Pt2::new(na.x, Nm(c))
                        }
                    };
                    group.sort_by_key(|(k, p)| (coord(*p), *k));
                    let extent = |p: Pt2, q: Pt2| (coord(p).min(coord(q)), coord(p).max(coord(q)));
                    let at: Vec<i64> = group.iter().map(|(_, p)| coord(*p)).collect();
                    let points = if !self.centre && extent(a, b) == extent(na, nb) {
                        at
                    } else {
                        let Some(points) = centred(grid, coord(na), coord(nb), &at) else {
                            return tx.reject(text!(
                                "refusal-box-reshape-too-small",
                                crate::words::symbol(tx, self.rep),
                                group.len().to_string(),
                                (group.len() + 1).to_string()
                            ));
                        };
                        points
                    };
                    for ((k, _), c) in group.iter().zip(points) {
                        placed[*k] = Some((onto(c), old.cps[*k].dir.clone()));
                    }
                }
                let mut out = Vec::with_capacity(old.cps.len());
                for (cp, at) in old.cps.iter().zip(placed) {
                    let Some((at, dir)) = at else { continue };
                    let Ok(pos) = rel(at) else {
                        return out_of_range(tx);
                    };
                    out.push(BoxCp {
                        idx: cp.idx,
                        pos,
                        dir,
                        name: cp.name.clone(),
                        extra: cp.extra.clone(),
                    });
                }
                out
            }
        };
        // The connections at the box's CPs: a CP that is no longer drawn must not be connected on the page; the
        // others follow their CP.
        let function: Option<Id<Function>> = rep.functions.first().copied();
        let mut moves: BTreeMap<(Id<Function>, u16), Pt2> = BTreeMap::new();
        if let Some(f) = function {
            for cp in &old.cps {
                let Ok(before) = drawn(cp.pos) else {
                    return out_of_range(tx);
                };
                match cps.iter().find(|c| c.idx == cp.idx) {
                    Some(c) => {
                        let Ok(after) = drawn(c.pos) else {
                            return out_of_range(tx);
                        };
                        let Ok(d) = sub(after, before) else {
                            return out_of_range(tx);
                        };
                        if d != Pt2::ORIGIN {
                            moves.insert((f, cp.idx), d);
                        }
                    }
                    None => {
                        let connected = tx.connections_of_function(f).into_iter().any(|cid| {
                            tx.get::<Connection>(cid).is_some_and(|c| {
                                c.page == Some(rep.page)
                                    && [&c.a, &c.b]
                                        .into_iter()
                                        .any(|e| matches!(e, Endpoint::Cp(g, i) if *g == f && *i == cp.idx))
                            })
                        });
                        if connected {
                            return tx.reject(text!("refusal-box-reshape-cp-connected", cp.idx.to_string()));
                        }
                        notices.push(format!("connection point {} is no longer drawn", cp.idx));
                    }
                }
            }
        }
        follow_moved_cps(tx, rep.page, grid, &moves, text!("refusal-move-what-reshape"))?;
        // The position becomes the outline's smallest corner; what is stored relative to it moves the other way.
        let corner = Pt2::new(
            self.outline.iter().map(|v| v.x).min().unwrap_or(Nm::ZERO),
            self.outline.iter().map(|v| v.y).min().unwrap_or(Nm::ZERO),
        );
        let Ok(pos) = abs(corner) else {
            return out_of_range(tx);
        };
        let back = |v: Pt2| sub(v, corner);
        let (Ok(outline), Ok(cps)) = (
            self.outline
                .iter()
                .map(|v| back(*v))
                .collect::<Result<Vec<Pt2>, OutOfRange>>(),
            cps.into_iter()
                .map(|c| back(c.pos).map(|pos| BoxCp { pos, ..c }))
                .collect::<Result<Vec<BoxCp>, OutOfRange>>(),
        ) else {
            return out_of_range(tx);
        };
        let mut inner = old.inner.clone();
        for shape in &mut inner {
            shift_primitive(shape, Pt2::ORIGIN - corner);
        }
        let mut next = (*rep).clone();
        next.pos = pos;
        next.box_ = Some(BoxSpec {
            outline,
            cps,
            inner,
            slots: None,
            extra: old.extra.clone(),
        });
        tx.put(next);
        Ok(Outcome {
            notices,
            ..Outcome::primary((Kind::Representation, self.rep.cast()))
        })
    }
}

/// `box.redraw` (`RedrawBoxes`: Redraw them, after a part's device box changed): several black boxes reshaped as
/// `box.reshape` reshapes each, in turn on one transaction, so they are one preview and one undo step, all or none.
/// Rejected: no box; a box that `box.reshape` refuses (the whole step, in that command's words). Interactive. Outcome:
/// the last box's representation.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RedrawBoxes {
    /// The boxes, each as `box.reshape` takes it.
    pub boxes: Vec<ReshapeBox>,
}

impl Command for RedrawBoxes {
    const NAME: &'static str = "box.redraw";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.boxes.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/boxes",
                text!("refusal-box-redraw-none"),
            ));
        }
        let mut last = Outcome::default();
        for b in &self.boxes {
            last = b.run(tx)?;
        }
        Ok(last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qet() -> Grid {
        Grid::new(7_937_500, 3).expect("grid")
    }

    #[test]
    fn snapping_along_an_edge_stays_inside_and_on_the_grid() {
        let g = qet();
        let a = g.at(0).unwrap().0;
        let b = g.at(10).unwrap().0;
        assert_eq!(snap_between(g, g.at(3).unwrap().0 + 10, a, b), g.at(3).unwrap().0);
        assert_eq!(snap_between(g, -50_000_000, a, b), a);
        assert_eq!(snap_between(g, 500_000_000, a, b), b);
        // Reversed edges: the clamp is on the interval.
        assert_eq!(snap_between(g, 500_000_000, b, a), b);
    }

    #[test]
    fn the_outward_normal_of_a_clockwise_rectangle() {
        let p = |x: i64, y: i64| Pt2::new(Nm(x), Nm(y));
        let r = [p(0, 0), p(10, 0), p(10, 5), p(0, 5)];
        let area = signed_area2(&r);
        assert!(area > 0);
        assert_eq!(outward(r[0], r[1], area), Some(Dir4::Up));
        assert_eq!(outward(r[1], r[2], area), Some(Dir4::Right));
        assert_eq!(outward(r[2], r[3], area), Some(Dir4::Down));
        assert_eq!(outward(r[3], r[0], area), Some(Dir4::Left));
        let ccw: Vec<Pt2> = r.iter().rev().copied().collect();
        let area = signed_area2(&ccw);
        // The bottom edge, travelled to the right.
        assert_eq!(outward(ccw[0], ccw[1], area), Some(Dir4::Down));
    }
}
