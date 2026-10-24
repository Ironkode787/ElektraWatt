// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `box.reshape` (`ReshapeBox`, FR-DEV-039, WP-S2.04 §5.1, §9.4): a black box's outline and connection points
//! changed, its connections kept.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_schema::{BoxCp, BoxSpec, Connection, Dir4, Endpoint, Function, Kind, Page, Representation};
use ew_units::{Grid, Nm, Orient2, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::commands::connection::follow_moved_cps;
use crate::error::CmdError;
use crate::geometry::{OutOfRange, add, on_grid, on_segment, point_in_range, require_on_grid, sub};
use crate::meta::CommandMeta;
use crate::proxy::{Pt2S, UuidS};
use crate::tx::TxBuilder;

/// `box.reshape` (`ReshapeBox`, FR-DEV-039): replaces the outline of a black box (a representation with a `box`),
/// and its connection points with `cps`, or re-projects them onto the new outline.
///
/// - `outline` is relative to the representation's `pos` (under its orientation, as `box.black` stores it): at
///   least three vertices, each on the page's connection grid in absolute terms (`pos + orient(v)`).
/// - **With `cps`** the box's CPs are replaced: function CP indices distinct, each CP on the grid in absolute terms
///   and on the outline (on an edge `v(i) v(i+1)`, the outline closed).
/// - **Without `cps`** every CP is re-projected (doc 06 §8 item 13): it keeps the index of the edge it lay on (the
///   first edge containing it, in outline order) and its offset from that edge's first vertex (measured along the
///   edge's axis), clamped to the new edge's length and snapped to the grid point along the edge nearest to it
///   (ties toward the edge's first vertex); its direction is kept. A CP whose edge index no longer exists, or that
///   lay on no edge, moves to the nearest grid point on the nearest remaining edge (ties by the smaller edge index)
///   and leaves along that edge's outward normal, listed as a notice. On an edge that is neither horizontal nor
///   vertical the grid point is the edge's first vertex (polygon outlines are a follow-up, WP-S2.04 §17.4 item 6),
///   also listed as a notice.
/// - Inner graphics are kept. Routes of the connections on the page at the box's CPs follow each moved CP
///   (`geometry::follow_end`).
///
/// Rejected: a representation that does not exist or has no box; fewer than three outline vertices or one off the
/// grid; with `cps`, two CPs with one index, a CP off the grid or off the outline; a box CP that a connection on
/// the page ends at and that the new CPs no longer draw (keep it in `cps`). Interactive. Outcome: the
/// representation.
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
}

/// The unit vector of a direction (`Up` is −y); `None` for an unknown value.
fn dir_vector(d: &Dir4) -> Option<(i64, i64)> {
    match d {
        Dir4::Up => Some((0, -1)),
        Dir4::Right => Some((1, 0)),
        Dir4::Down => Some((0, 1)),
        Dir4::Left => Some((-1, 0)),
        Dir4::Unknown(_) => None,
    }
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

/// A direction under an orientation.
fn orient_dir(o: Orient2, d: &Dir4) -> Option<Dir4> {
    let v = dir_vector(d)?;
    let p = o.apply(Pt2::new(Nm(v.0), Nm(v.1)));
    dir_of_vector((p.x.0, p.y.0))
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

/// The point at offset `d` (clamped) from `a` along the edge `a`–`b`, snapped to the grid along the edge; `None` on
/// an edge that is neither horizontal nor vertical.
fn along(grid: Grid, a: Pt2, b: Pt2, d: i64) -> Option<Pt2> {
    if a.y == b.y {
        let len = (b.x.0 - a.x.0).abs();
        let t = a.x.0 + (b.x.0 - a.x.0).signum() * d.min(len);
        Some(Pt2::new(Nm(snap_between(grid, t, a.x.0, b.x.0)), a.y))
    } else if a.x == b.x {
        let len = (b.y.0 - a.y.0).abs();
        let t = a.y.0 + (b.y.0 - a.y.0).signum() * d.min(len);
        Some(Pt2::new(a.x, Nm(snap_between(grid, t, a.y.0, b.y.0))))
    } else {
        None
    }
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

impl Command for ReshapeBox {
    const NAME: &'static str = "box.reshape";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let rep = tx.require::<Representation>(self.rep)?;
        let Some(old) = rep.box_.clone() else {
            return tx.reject(
                "the representation has no box; only a black box drawn with an outline can be reshaped",
            );
        };
        let page = tx.require::<Page>(rep.page)?;
        let grid = page.grid;
        if self.outline.len() < 3 {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/outline",
                "an outline has at least three vertices",
            ));
        }
        let out_of_range = |tx: &TxBuilder<'_>| -> Result<Outcome, CmdError> {
            tx.reject("the box lies outside the drawable range; reshape it nearer the page origin")
        };
        // Absolute page points of relative box points (`pos + orient(v)`), and back.
        let abs = |v: Pt2| -> Result<Pt2, OutOfRange> {
            if !point_in_range(v) {
                return Err(OutOfRange);
            }
            add(rep.pos, rep.orient.apply(v))
        };
        let inverse = rep.orient.inverse();
        let rel = |p: Pt2| -> Result<Pt2, OutOfRange> { sub(p, rep.pos).map(|d| inverse.apply(d)) };
        let mut new_abs: Vec<Pt2> = Vec::with_capacity(self.outline.len());
        for (i, v) in self.outline.iter().enumerate() {
            let Ok(p) = abs(*v) else { return out_of_range(tx) };
            require_on_grid(tx, rep.page, p, &format!("/outline/{i}"))?;
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
                            "two box CPs have the same function CP index",
                        ));
                    }
                    let Ok(p) = abs(cp.pos) else {
                        return out_of_range(tx);
                    };
                    require_on_grid(tx, rep.page, p, &format!("/cps/{i}/pos"))?;
                    if !edges(&new_abs).any(|(_, a, b)| on_segment(p, a, b)) {
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            format!("/cps/{i}/pos"),
                            "the connection point does not lie on the outline; put it on an edge",
                        ));
                    }
                }
                cps.clone()
            }
            None => {
                let area = signed_area2(&new_abs);
                let mut out = Vec::with_capacity(old.cps.len());
                for cp in &old.cps {
                    let Ok(p) = abs(cp.pos) else {
                        return out_of_range(tx);
                    };
                    let edge = edges(&old_abs).find(|(_, a, b)| on_segment(p, *a, *b));
                    let (at, dir) = match edge {
                        Some((i, a, _)) if i < new_abs.len() => {
                            let (na, nb) = (new_abs[i], new_abs[(i + 1) % new_abs.len()]);
                            let d = (p.x.0 - a.x.0).abs().max((p.y.0 - a.y.0).abs());
                            match along(grid, na, nb, d) {
                                Some(q) => (q, cp.dir.clone()),
                                None => {
                                    notices.push(format!(
                                        "connection point {} moved to the first vertex of edge {i}, which is not \
                                         horizontal or vertical",
                                        cp.idx
                                    ));
                                    (na, cp.dir.clone())
                                }
                            }
                        }
                        _ => {
                            let (j, na, nb) = edges(&new_abs)
                                .min_by_key(|(j, a, b)| (dist2(p, nearest_on_edge(grid, *a, *b, p)), *j))
                                .unwrap_or((0, new_abs[0], new_abs[1]));
                            let q = nearest_on_edge(grid, na, nb, p);
                            let dir = outward(na, nb, area)
                                .and_then(|d| orient_dir(inverse, &d))
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
                            (q, dir)
                        }
                    };
                    let Ok(pos) = rel(at) else { return out_of_range(tx) };
                    out.push(BoxCp {
                        idx: cp.idx,
                        pos,
                        dir,
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
                let Ok(before) = abs(cp.pos) else {
                    return out_of_range(tx);
                };
                match cps.iter().find(|c| c.idx == cp.idx) {
                    Some(c) => {
                        let Ok(after) = abs(c.pos) else {
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
                            return tx.reject(format!(
                                "box connection point {} is connected on this page and the new connection points \
                                 leave it out; keep it in `cps`",
                                cp.idx
                            ));
                        }
                        notices.push(format!("connection point {} is no longer drawn", cp.idx));
                    }
                }
            }
        }
        follow_moved_cps(tx, rep.page, grid, &moves, "the reshape")?;
        let mut next = (*rep).clone();
        next.box_ = Some(BoxSpec {
            outline: self.outline.clone(),
            cps,
            inner: old.inner.clone(),
            extra: old.extra.clone(),
        });
        tx.put(next);
        Ok(Outcome {
            notices,
            ..Outcome::primary((Kind::Representation, self.rep.cast()))
        })
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
