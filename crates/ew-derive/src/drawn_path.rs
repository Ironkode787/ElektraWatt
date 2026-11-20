// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The path a connection is drawn along on its page (doc 06 §5.4), one rule for the page and the model:
//! the page scene (`ew-scene`, `route::connection_polyline`) draws a wire along it,
//! normaliser 7 of `ew-commands` splits a connection under a dropped symbol along it (its ends placed by
//! [`rep_cp_end`](crate::rep_cp_end)), and EW-TERM-006 names a terminal lying on a wire it does not join.
//!
//! - A connection with a stored route is drawn along it as stored ([`stored_path`]): the route holds every vertex
//!   from end `a`'s point to end `b`'s, both included (ADR-0033); equal neighbours are merged, and an
//!   end whose point differs from the route's end vertex (a stale route) is joined to it by one straight segment.
//! - A connection without one has no wire geometry: it is *unrouted* (the scene draws a dashed right-angled
//!   line between its ends, `ew_drafting::unrouted_line`, which no rule reads) or, with its
//!   ends at one point, drawn as nothing.
//! - An end at a CP is the CP's point and leaving direction on the page ([`cp_end`]); an end at a junction is the
//!   junction's point, without a direction. A connection one of whose ends is not drawn on the page is not drawn.
//!
//! A stored route and a junction are read with one number per grid position ([`read_connection`],
//! [`read_junction`], docs/design/06a-drafting.md §2.1): a vertex or a junction's point within 1 nm of a position of
//! the page's grid is the grid's own number for it, as a connection point's is. The record keeps its numbers until a
//! command writes it, and then writes them as read.
//!
//! The scene then pulls a vertex that lies off the sheet onto the sheet's edge (the sheet is the frame's, which only
//! the scene knows); these paths keep every vertex where it is, so they differ from the drawing only next to an end
//! off the sheet.

use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{Connection, Dir4, Endpoint, Junction, Page};
use ew_units::{Grid, Nm, Orient2, Pt2};

use crate::Q;
use crate::cp_point::cp_end;

/// A stored route as it is read on a page of `grid`: each vertex on the grid's own number where it lies within 1 nm of
/// a grid position ([`Grid::settle_point`]), equal neighbours merged; a route of two equal ends keeps both. A
/// coordinate is read the same wherever it stands, so a stretch along a line stays along it.
pub fn read_route(grid: Grid, route: &[Pt2]) -> Vec<Pt2> {
    let mut out: Vec<Pt2> = route.iter().map(|p| grid.settle_point(*p)).collect();
    out.dedup();
    if out.len() == 1 && route.len() > 1 {
        out.push(out[0]);
    }
    out
}

/// The connection `c` as it is read on a page of `grid`: its route by [`read_route`]; `c` itself where nothing
/// changes.
pub fn read_connection(c: &Arc<Connection>, grid: Grid) -> Arc<Connection> {
    let route = read_route(grid, &c.route);
    if route == c.route {
        return Arc::clone(c);
    }
    Arc::new(Connection {
        route,
        ..(**c).clone()
    })
}

/// The junction `j` as it is read on a page of `grid`: its point on the grid's own number where it lies within 1 nm
/// of a grid position; `j` itself where nothing changes.
pub fn read_junction(j: &Arc<Junction>, grid: Grid) -> Arc<Junction> {
    let at = grid.settle_point(j.at);
    if at == j.at {
        return Arc::clone(j);
    }
    Arc::new(Junction { at, ..(**j).clone() })
}

/// The grid of `page` for reading its points (a 4 mm step, which reads every point as stored, for a page that is
/// not there).
pub fn page_grid(q: &Q, page: Id<Page>) -> Grid {
    q.pages().get(page).map_or(Grid::MM4, |p| p.grid)
}

/// Whether a direction runs along y: `Some(true)` for `Up` and `Down`, `Some(false)` for `Left` and `Right`, `None`
/// for an unknown direction.
pub fn dir_vertical(d: &Dir4) -> Option<bool> {
    match d {
        Dir4::Up | Dir4::Down => Some(true),
        Dir4::Left | Dir4::Right => Some(false),
        Dir4::Unknown(_) => None,
    }
}

/// The direction `d` turned by the orientation `o` (a representation's), as the page scene turns a CP's direction;
/// `None` for an unknown direction.
pub fn oriented_dir(d: &Dir4, o: Orient2) -> Option<Dir4> {
    let v = match d {
        Dir4::Up => (0, -1),
        Dir4::Right => (1, 0),
        Dir4::Down => (0, 1),
        Dir4::Left => (-1, 0),
        Dir4::Unknown(_) => return None,
    };
    let p = o.apply(Pt2::new(Nm(v.0), Nm(v.1)));
    match (p.x.0, p.y.0) {
        (0, -1) => Some(Dir4::Up),
        (1, 0) => Some(Dir4::Right),
        (0, 1) => Some(Dir4::Down),
        (-1, 0) => Some(Dir4::Left),
        _ => None,
    }
}

/// Whether `p` lies on the closed segment `a`–`b`: exactly, collinear by an `i128` cross product and inside the
/// segment's bounding box. A product that would overflow `i128` (a coordinate far beyond the ±(2⁵³ − 1) nm a record
/// holds, ADR-0005) counts as off the segment.
pub fn on_segment(p: Pt2, a: Pt2, b: Pt2) -> bool {
    let (px, py) = (i128::from(p.x.0), i128::from(p.y.0));
    let (ax, ay) = (i128::from(a.x.0), i128::from(a.y.0));
    let (bx, by) = (i128::from(b.x.0), i128::from(b.y.0));
    let cross = (bx - ax)
        .checked_mul(py - ay)
        .zip((by - ay).checked_mul(px - ax))
        .and_then(|(l, r)| l.checked_sub(r));
    cross == Some(0) && px >= ax.min(bx) && px <= ax.max(bx) && py >= ay.min(by) && py <= ay.max(by)
}

/// The path connection `c` is drawn along on `page` (see the module documentation), from its end `a` to its end `b`;
/// `None` when it is no wire there: it belongs to another page (or none), an end is not drawn on `page`, or it has no
/// stored route (unrouted).
pub fn drawn_path(q: &Q, c: &Connection, page: Id<Page>) -> Option<Vec<Pt2>> {
    if c.page != Some(page) || c.route.is_empty() {
        return None;
    }
    let grid = page_grid(q, page);
    let end = |e: &Endpoint| match e {
        Endpoint::Cp(function, idx) => cp_end(q, *function, *idx, page),
        Endpoint::Junction(j) => q
            .junctions()
            .get(*j)
            .filter(|j| j.page == page)
            .map(|j| (read_junction(j, grid).at, None)),
        Endpoint::Unknown(_) => None,
    };
    let ((a, _), (b, _)) = (end(&c.a)?, end(&c.b)?);
    Some(stored_path(a, &read_route(grid, &c.route), b))
}

/// The path a stored `route` is drawn along between the ends' points `a` and `b`: the route as stored,
/// both ends included, equal neighbours merged; an end whose point differs from the route's end vertex (a stale route,
/// an end moved without the route following) is joined to it by one straight segment. An empty route gives `[a, b]`
/// (`[a]` for equal points).
pub fn stored_path(a: Pt2, route: &[Pt2], b: Pt2) -> Vec<Pt2> {
    let mut path: Vec<Pt2> = Vec::with_capacity(route.len() + 2);
    for p in std::iter::once(a)
        .chain(route.iter().copied())
        .chain(std::iter::once(b))
    {
        if path.last() != Some(&p) {
            path.push(p);
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    #[test]
    fn a_point_on_a_segment_is_exact() {
        let (a, b) = (p(0, 0), p(30, 10));
        assert!(on_segment(p(15, 5), a, b));
        assert!(on_segment(a, a, b) && on_segment(b, a, b));
        assert!(!on_segment(p(15, 6), a, b));
        assert!(!on_segment(p(60, 20), a, b), "collinear beyond the end");
        // (−10, 9) lies on the segment between these ends (on x + y = −1), but (2⁶⁴ − 1)(2⁶³ + 9) overflows `i128`.
        let (from, to) = (p(i64::MAX, i64::MIN), p(i64::MIN, i64::MAX));
        assert!(on_segment(p(0, -1), from, to), "a product that fits");
        assert!(!on_segment(p(-10, 9), from, to), "an overflowing product");
    }

    #[test]
    fn a_direction_turns_with_the_orientation() {
        assert_eq!(oriented_dir(&Dir4::Up, Orient2::R90), Some(Dir4::Right));
        assert_eq!(oriented_dir(&Dir4::Right, Orient2::R90), Some(Dir4::Down));
        assert_eq!(oriented_dir(&Dir4::Right, Orient2::M0), Some(Dir4::Left));
        assert_eq!(oriented_dir(&Dir4::Up, Orient2::M0), Some(Dir4::Up));
        assert_eq!(oriented_dir(&Dir4::Up, Orient2::M90), Some(Dir4::Right));
        assert_eq!(oriented_dir(&Dir4::Left, Orient2::R270), Some(Dir4::Down));
        assert_eq!(
            oriented_dir(&Dir4::Unknown(serde_json::json!("north")), Orient2::R0),
            None
        );
    }
}
