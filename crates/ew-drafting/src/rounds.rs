// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Wires round a body (docs/design/06a-drafting.md §3.5, "Rings"): a wire from a pin of a body whose other end lies
//! behind the pin's edge leaves the pin, goes round one end of the body and on to its other end. The wires that go
//! round one end of one body are laid together as nested rings ([`crate::route`] lays them after the tracks and
//! before the search): ring `r` runs `r` steps off the body on every side it passes, the wire whose pin stands nearest
//! the corner innermost.
//!
//! The shapes, with the pin's side turned up: out of the pin to the ring's line over the edge, along it to the ring's
//! column beyond the body's end, then either down the column to the other end's line, where that end stands beyond the
//! column and faces it (a mark beside the body), or down to the ring's line under the body's far edge, along it to the
//! other end's column and into the end, where that end stands below that line and faces up (a terminal under a card
//! whose pins face up). Any other wire is the search's.

use crate::consts::TICKS_PER_STEP;
use crate::model::{Pt, Rect, Side, Tick, grid_down, grid_up};

/// A turn or mirror of the page that brings a pin's side up, and back.
#[derive(Clone, Copy)]
struct Turn(Side);

impl Turn {
    fn to(self, p: Pt) -> Pt {
        match self.0 {
            Side::Up => p,
            Side::Down => Pt::new(p.x, -p.y),
            Side::Left => Pt::new(p.y, p.x),
            Side::Right => Pt::new(p.y, -p.x),
        }
    }

    fn back(self, q: Pt) -> Pt {
        match self.0 {
            Side::Up => q,
            Side::Down => Pt::new(q.x, -q.y),
            Side::Left => Pt::new(q.y, q.x),
            Side::Right => Pt::new(-q.y, q.x),
        }
    }

    fn side(self, s: Side) -> Side {
        let u = self.to(s.unit());
        match (u.x, u.y) {
            (0, y) if y < 0 => Side::Up,
            (0, _) => Side::Down,
            (x, _) if x < 0 => Side::Left,
            _ => Side::Right,
        }
    }

    fn rect(self, r: Rect) -> Rect {
        Rect::new(self.to(Pt::new(r.x0, r.y0)), self.to(Pt::new(r.x1, r.y1)))
    }
}

/// The end of the body a ring passes: the one at the lower coordinate along the pin's edge (left of an upper or a
/// lower edge, the top of a side edge) or the other.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum Corner {
    First,
    Last,
}

/// A wire that may go round a body.
pub(crate) struct Ask {
    /// The pin on the body, and its side.
    pub pin: (Pt, Side),
    /// The body.
    pub body: Rect,
    /// The other end, and its side.
    pub other: (Pt, Side),
    /// The other wired pins of the body on the pin's side: the wires a ring's run along the edge passes.
    pub wired: Vec<Pt>,
}

impl Ask {
    /// The route of the ring round `corner` at `g` ticks off the body (module documentation), or `None` where the
    /// other end is not one a ring reaches.
    pub fn route(&self, corner: Corner, g: Tick) -> Option<Vec<Pt>> {
        let t = Turn(self.pin.1);
        let (a, e, b, se) = (
            t.to(self.pin.0),
            t.to(self.other.0),
            t.rect(self.body),
            t.side(self.other.1),
        );
        // The ring's lines on the grid, off the body's edges rounded outward to it.
        let out = grid_down(a.y.min(b.y0)) - g;
        let (col, toward) = match corner {
            Corner::First => (grid_down(b.x0) - g, Side::Right),
            Corner::Last => (grid_up(b.x1) + g, Side::Left),
        };
        let past = match corner {
            Corner::First => e.x < col,
            Corner::Last => e.x > col,
        };
        let far = grid_up(b.y1) + g;
        let pts = if se == toward && past && e.y > a.y {
            vec![a, Pt::new(a.x, out), Pt::new(col, out), Pt::new(col, e.y), e]
        } else if se == Side::Up && e.y > far {
            vec![
                a,
                Pt::new(a.x, out),
                Pt::new(col, out),
                Pt::new(col, far),
                Pt::new(e.x, far),
                e,
            ]
        } else {
            return None;
        };
        // (Every corner on the grid: a pin off it is the search's.)
        let inner = &pts[1..pts.len() - 1];
        if inner
            .iter()
            .any(|p| p.x.rem_euclid(TICKS_PER_STEP) != 0 || p.y.rem_euclid(TICKS_PER_STEP) != 0)
        {
            return None;
        }
        Some(pts.into_iter().map(|p| t.back(p)).collect())
    }

    /// How many of the other wired pins of the edge a ring round `corner` passes on its way out.
    pub fn passes(&self, corner: Corner) -> usize {
        let t = Turn(self.pin.1);
        let a = t.to(self.pin.0);
        self.wired
            .iter()
            .map(|p| t.to(*p))
            .filter(|p| {
                p.y == a.y
                    && match corner {
                        Corner::First => p.x < a.x,
                        Corner::Last => p.x > a.x,
                    }
            })
            .count()
    }

    /// The distance of the pin from `corner` along its edge: the ring's order, nearest innermost.
    pub fn reach(&self, corner: Corner) -> Tick {
        let t = Turn(self.pin.1);
        let (a, b) = (t.to(self.pin.0), t.rect(self.body));
        match corner {
            Corner::First => a.x - b.x0,
            Corner::Last => b.x1 - a.x,
        }
    }
}
