// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Provisional orthogonal routing of connections.
//!
//! **PROVISIONAL.** This module is a replaceable stand-in so that pages can be drawn before the real router
//! exists. The A* auto-route of doc 06 §5.4 (work items FT-18 and FT-43) owns routing; when it lands, it
//! replaces [`provisional_route`] and the stored routes it writes take over through [`connection_polyline`].
//!
//! Everything is integer nanometres ([`Nm`](ew_units::Nm), [`Pt2`], [`Rect`]) on the page, origin top-left,
//! y down, as in [`symbol`](crate::symbol). The functions are pure, deterministic and integer-only (no
//! arithmetic on coordinates at all, so no overflow), and the result never leaves the page: every vertex is
//! clamped into the page rectangle.
//!
//! # Rules ([`provisional_route`])
//!
//! 1. Both ends at the same position: a single point (callers draw nothing for it).
//! 2. Same x or same y: a straight segment.
//! 3. Otherwise an L with one corner, taken from the ends' own coordinates (so ends on the connection grid
//!    give a corner on the grid). The wire leaves `a` along the axis of `a.dir`: `Up`/`Down` gives the
//!    corner `(a.x, b.y)`, `Left`/`Right` gives `(b.x, a.y)`. Without a direction at `a` (a junction or an
//!    unknown direction) the direction of `b` decides, so the wire arrives at `b` along its axis:
//!    `Up`/`Down` gives `(b.x, a.y)`, `Left`/`Right` gives `(a.x, b.y)`. With neither direction the corner is
//!    `(b.x, a.y)`.
//!
//! Afterwards the polyline is cleaned: consecutive duplicate points and interior vertices collinear with
//! their neighbours on an axis are removed; the first and last vertex always stay.
//!
//! # Junctions
//!
//! A connection whose end is a junction passes the junction position as that end with `dir: None`, so every
//! arm of a junction ends exactly at the junction point.

use ew_schema::Dir4;
use ew_units::{Nm, Pt2, Rect};

/// One end of a connection: its position and, for a CP end, the direction a wire leaves it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteEnd {
    /// Position of the end on the page.
    pub pos: Pt2,
    /// `None` for a junction end or an unknown direction.
    pub dir: Option<Dir4>,
}

/// The polyline of a connection from `a` to `b` (both ends included).
///
/// - `stored` non-empty: `[a.pos] ++ stored ++ [b.pos]` (the stored route holds the interior vertices, in
///   model order), each point clamped into `page`, then cleaned (consecutive duplicates and axis-collinear
///   interior vertices removed).
/// - `stored` empty: [`provisional_route`]`(a, b, page)`.
pub fn connection_polyline(a: &RouteEnd, b: &RouteEnd, stored: &[Pt2], page: Rect) -> Vec<Pt2> {
    if stored.is_empty() {
        return provisional_route(a, b, page);
    }
    let mut pts = Vec::with_capacity(stored.len() + 2);
    pts.push(clamp(a.pos, page));
    pts.extend(stored.iter().map(|&p| clamp(p, page)));
    pts.push(clamp(b.pos, page));
    clean(pts)
}

/// The provisional orthogonal route between two ends: 1, 2 or 3 vertices.
///
/// 1. `a.pos == b.pos` gives `[a.pos]` (a single point; callers draw nothing for it).
/// 2. Same x or same y gives `[a.pos, b.pos]`.
/// 3. Otherwise an L with one corner: `(a.x, b.y)` when `a.dir` is `Up`/`Down`, `(b.x, a.y)` when it is
///    `Left`/`Right`; with no (or an unknown) `a.dir`, `b.dir` decides so the wire arrives along `b`'s
///    axis (`Up`/`Down` gives `(b.x, a.y)`, `Left`/`Right` gives `(a.x, b.y)`); with neither, `(b.x, a.y)`.
///
/// Every vertex is clamped into `page`, then the polyline is cleaned (see the module docs).
pub fn provisional_route(a: &RouteEnd, b: &RouteEnd, page: Rect) -> Vec<Pt2> {
    let (pa, pb) = (a.pos, b.pos);
    let pts = if pa == pb {
        vec![pa]
    } else if pa.x == pb.x || pa.y == pb.y {
        vec![pa, pb]
    } else {
        let vertical_first = Pt2::new(pa.x, pb.y);
        let horizontal_first = Pt2::new(pb.x, pa.y);
        let corner = match (&a.dir, &b.dir) {
            (Some(Dir4::Up | Dir4::Down), _) => vertical_first,
            (Some(Dir4::Left | Dir4::Right), _) => horizontal_first,
            (_, Some(Dir4::Up | Dir4::Down)) => horizontal_first,
            (_, Some(Dir4::Left | Dir4::Right)) => vertical_first,
            _ => horizontal_first,
        };
        vec![pa, corner, pb]
    };
    clean(pts.into_iter().map(|p| clamp(p, page)).collect())
}

/// `p` pulled onto the page rectangle, per axis.
fn clamp(p: Pt2, page: Rect) -> Pt2 {
    let (lo, hi) = (page.min(), page.max());
    Pt2::new(clamp_nm(p.x, lo.x, hi.x), clamp_nm(p.y, lo.y, hi.y))
}

fn clamp_nm(v: Nm, lo: Nm, hi: Nm) -> Nm {
    v.max(lo).min(hi)
}

/// Removes consecutive duplicate points, then interior vertices collinear with their neighbours on an axis,
/// to a fixpoint. The first and last vertex are never removed.
fn clean(pts: Vec<Pt2>) -> Vec<Pt2> {
    let mut out: Vec<Pt2> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last() == Some(&p) {
            continue;
        }
        out.push(p);
        // The newest vertex is the candidate last one, so it stays; test the one before it.
        while let [.., prev, mid, next] = out[..] {
            if collinear(prev, mid, next) {
                out.remove(out.len() - 2);
                // Removing `mid` may leave `prev == next` (a spike back onto itself).
                if out.len() >= 2 && out[out.len() - 1] == out[out.len() - 2] {
                    out.pop();
                }
            } else {
                break;
            }
        }
    }
    out
}

fn collinear(prev: Pt2, v: Pt2, next: Pt2) -> bool {
    (prev.x == v.x && v.x == next.x) || (prev.y == v.y && v.y == next.y)
}
