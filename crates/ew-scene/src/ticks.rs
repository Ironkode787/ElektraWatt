// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The conversion of one page between nanometres and the layout's ticks (1/16 of the page's grid step,
//! `ew_drafting::consts::TICKS_PER_STEP`), with its rounding rules named: a point (`Grid::tick_of`), a box rounded
//! outward (`Grid::tick_span`) and an area rounded inward.
//!
//! Not here: the rules themselves on one coordinate (`ew_units::Grid`); the page model in ticks (`ew_drafting`); the
//! page the labels layer builds with it (`draft.rs`).

use ew_drafting::Tick;
use ew_drafting::consts::TICKS_PER_STEP;
use ew_units::{Grid, Nm, Pt2, Rect};

/// Ticks to a grid step, as `ew-units` takes it.
const PER: u32 = TICKS_PER_STEP as u32;

/// A tick of `i64` as the model's `i32`, saturating.
fn tick(v: i64) -> Tick {
    Tick::try_from(v).unwrap_or(if v < 0 { Tick::MIN } else { Tick::MAX })
}

/// The page's grid as the conversion between nm and ticks.
#[derive(Clone, Copy)]
pub(crate) struct Ticks(pub(crate) Grid);

impl Ticks {
    pub(crate) fn pt(self, p: Pt2) -> (ew_drafting::Pt, bool) {
        let (x, xo) = self.0.tick_of(p.x, PER);
        let (y, yo) = self.0.tick_of(p.y, PER);
        (ew_drafting::Pt::new(tick(x), tick(y)), xo && yo)
    }

    /// A box rounded outward.
    pub(crate) fn rect(self, r: Rect) -> ew_drafting::Rect {
        let (x0, x1) = self.0.tick_span(r.min().x, r.max().x, PER);
        let (y0, y1) = self.0.tick_span(r.min().y, r.max().y, PER);
        ew_drafting::Rect {
            x0: tick(x0),
            y0: tick(y0),
            x1: tick(x1),
            y1: tick(y1),
        }
    }

    /// An area rounded inward.
    pub(crate) fn inward(self, r: Rect) -> ew_drafting::Rect {
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

    /// A move in ticks as a move in nm.
    pub(crate) fn nm(self, d: ew_drafting::Pt) -> Pt2 {
        let one = |t: Tick| self.0.nm_of_tick(i64::from(t), PER).unwrap_or(Nm::ZERO);
        Pt2::new(one(d.x), one(d.y))
    }
}
