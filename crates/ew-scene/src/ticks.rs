// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The conversion of one page between nanometres and the layout's ticks (1/16 of the page's grid step,
//! `ew_drafting::consts::TICKS_PER_STEP`), with its rounding rules named: a point ([`Ticks::point`],
//! `Grid::tick_of`), a box rounded outward ([`Ticks::outward`], `Grid::tick_span`) and an area rounded inward
//! ([`Ticks::inward`]); back to nm by `Grid::nm_of_tick`. The labels layer builds the page's `DraftPage` with it,
//! `ew-api` finishes that page and converts routes, the view model's tools convert the pointer and a floating part.
//!
//! Not here: the rules themselves on one coordinate (`ew_units::Grid`); the page model in ticks (`ew_drafting`, which
//! knows no nanometre); the page the labels layer builds with it (`draft.rs`).

use ew_drafting::consts::TICKS_PER_STEP;
use ew_drafting::{Pt, Tick};
use ew_units::{Grid, Nm, Pt2, Rect};

/// Ticks to a grid step, as `ew-units` takes it.
const PER: u32 = TICKS_PER_STEP as u32;

use ew_drafting::saturating_tick as tick;

/// The conversion of one page between nm and the layout's ticks: the page's connection grid.
#[derive(Clone, Copy, Debug)]
pub struct Ticks(Grid);

impl Ticks {
    /// The conversion of a page with the grid `grid`.
    pub fn of(grid: Grid) -> Ticks {
        Ticks(grid)
    }

    /// The conversion of a page with the grid `(num, den)` as the page tables hold it (a 4 mm step for numbers that
    /// make no grid).
    pub fn new(grid: (i64, i64)) -> Ticks {
        let g = u64::try_from(grid.0)
            .ok()
            .zip(u64::try_from(grid.1).ok())
            .and_then(|(n, d)| Grid::new(n, d).ok())
            .unwrap_or(Grid::MM4);
        Ticks(g)
    }

    /// The page's grid.
    pub fn grid(self) -> Grid {
        self.0
    }

    /// **A point**: each coordinate by `Grid::tick_of` (a coordinate within 1 nm of a half-grid position takes that
    /// position's tick); whether both lie on such a position.
    pub fn point(self, p: Pt2) -> (Pt, bool) {
        let (x, xo) = self.0.tick_of(p.x, PER);
        let (y, yo) = self.0.tick_of(p.y, PER);
        (Pt::new(tick(x), tick(y)), xo && yo)
    }

    /// [`Ticks::point`] of a point in page nm as the page tables hold it, without whether it lies on the grid.
    pub fn pt(self, p: (i64, i64)) -> Pt {
        self.point(Pt2::new(Nm(p.0), Nm(p.1))).0
    }

    /// **A box rounded outward** (a text's box, a symbol's body, a structure box's fence, a floating part's body):
    /// `Grid::tick_span` on each axis, where an end within 2 nm of a tick takes that tick, so that a box keeps its
    /// ticks wherever its symbol stands.
    pub fn outward(self, r: Rect) -> ew_drafting::Rect {
        let (x0, x1) = self.0.tick_span(r.min().x, r.max().x, PER);
        let (y0, y1) = self.0.tick_span(r.min().y, r.max().y, PER);
        ew_drafting::Rect {
            x0: tick(x0),
            y0: tick(y0),
            x1: tick(x1),
            y1: tick(y1),
        }
    }

    /// **An area rounded inward** (the drawing area): its low ends rounded up, its high ends down, never below the
    /// low ones; an end within 2 nm of a tick takes that tick, as in [`Ticks::outward`].
    pub fn inward(self, r: Rect) -> ew_drafting::Rect {
        // ceil(a) = −floor(−a); floor(b) is the low end of b's span.
        let ceil = |v: Nm| {
            tick(
                self.0
                    .tick_span(Nm(v.0.saturating_neg()), Nm::ZERO, PER)
                    .0
                    .saturating_neg(),
            )
        };
        let floor = |v: Nm| tick(self.0.tick_span(v, v, PER).0);
        let (x0, y0) = (ceil(r.min().x), ceil(r.min().y));
        ew_drafting::Rect {
            x0,
            y0,
            x1: floor(r.max().x).max(x0),
            y1: floor(r.max().y).max(y0),
        }
    }

    /// A tick as page nm (`Grid::nm_of_tick`; 0 where it overflows).
    pub fn nm(self, t: Tick) -> i64 {
        self.0.nm_of_tick(i64::from(t), PER).map_or(0, |n| n.0)
    }

    /// A move of `d` ticks in page nm.
    pub fn move_nm(self, d: Pt) -> (i64, i64) {
        (self.nm(d.x) - self.nm(0), self.nm(d.y) - self.nm(0))
    }
}
