// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The orthogonal router the canvas tools call without a `Session` (WP-S2.04 §4.3, §8.1, D7): the pure function
//! `ew_commands::route::auto_route` on the plain integers and the [`Dir`] a tool holds. The Wire tool routes its
//! ghost with it and passes the result as `connection.draw`'s `route`, so the committed route is exactly the ghost
//! and the router never runs on replay.
//!
//! **Contract.** The full vertex list from `from.at` to `to.at`: deterministic (the tie rule of §8.1, independent of
//! the obstacle order), every interior vertex on the grid or on the coordinate of the end next to it, the ends
//! exact; without a path inside the area, the provisional L route. Page nm, y down. See `ew_commands::route` for
//! the algorithm.

use ew_commands::route::{self, Heading};

use crate::geometry::Dir;

/// One end of a route: its exact page point and, for a connection point, the direction a wire leaves it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RouteEnd {
    /// The page point, nm.
    pub at: (i64, i64),
    /// The leaving direction of a connection point; `None` for a junction or a split point.
    pub dir: Option<Dir>,
}

/// What the router routes (WP-S2.04 §8.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RouteRequest {
    /// The first end.
    pub from: RouteEnd,
    /// The second end.
    pub to: RouteEnd,
    /// The obstacle rectangles `[x0, y0, x1, y1]`, page nm: the bodies of the page's representations other than
    /// those of the two ends, inflated by one grid step.
    pub obstacles: Vec<[i64; 4]>,
    /// The connection grid `(num, den)` nm.
    pub grid: (i64, i64),
    /// The search area `[x0, y0, x1, y1]`, page nm: the bounding box of both ends grown by 4 grid steps, clipped to
    /// the page's drawing area.
    pub area: [i64; 4],
    /// The direction the route arrived at `from` with, for a first end without a direction (a fixed bend's incoming
    /// direction, FT-437): the route never doubles back over it; a turn there costs no bend. Ignored at a CP end.
    pub arrive: Option<Dir>,
}

fn heading(d: Dir) -> Heading {
    match d {
        Dir::Up => Heading::Up,
        Dir::Down => Heading::Down,
        Dir::Left => Heading::Left,
        Dir::Right => Heading::Right,
    }
}

fn end(e: RouteEnd) -> route::RouteEnd {
    route::RouteEnd {
        at: e.at,
        dir: e.dir.map(heading),
    }
}

/// The route of `req` (see the module documentation).
pub fn auto_route(req: &RouteRequest) -> Vec<(i64, i64)> {
    route::auto_route(&route::RouteRequest {
        from: end(req.from),
        to: end(req.to),
        obstacles: req.obstacles.clone(),
        grid: req.grid,
        area: req.area,
        arrive: req.arrive.map(heading),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wrapper_routes_as_the_commands_router() {
        let mm = 1_000_000;
        let req = RouteRequest {
            from: RouteEnd {
                at: (0, 0),
                dir: Some(Dir::Right),
            },
            to: RouteEnd {
                at: (10 * mm, 10 * mm),
                dir: Some(Dir::Up),
            },
            obstacles: vec![],
            grid: (mm, 1),
            area: [-4 * mm, -4 * mm, 14 * mm, 14 * mm],
            arrive: None,
        };
        assert_eq!(auto_route(&req), vec![(0, 0), (10 * mm, 0), (10 * mm, 10 * mm)]);
    }
}
