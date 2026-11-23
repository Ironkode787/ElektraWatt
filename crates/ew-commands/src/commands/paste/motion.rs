// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Effect 5 of `edit.paste`: how the fragment moves, by grid steps or turned about its anchor.

use super::*;

/// `round(Δk × num / den)` on one axis of a coordinate off the grid, the grid point `Δk` steps on for one on it.
pub(super) fn step_coord(grid: Grid, x: Nm, k: i64) -> Result<Nm, OutOfRange> {
    let v = if crate::geometry::on_grid(grid, x) {
        step_axis(grid, x, k).ok_or(OutOfRange)?
    } else {
        Nm(x.0.checked_add(steps_nm(grid, k)).ok_or(OutOfRange)?)
    };
    if crate::geometry::in_range(v) {
        Ok(v)
    } else {
        Err(OutOfRange)
    }
}

/// [`step_coord`] on both axes.
pub(super) fn step_point(grid: Grid, p: Pt2, k: [i64; 2]) -> Result<Pt2, OutOfRange> {
    Ok(Pt2::new(
        step_coord(grid, p.x, k[0])?,
        step_coord(grid, p.y, k[1])?,
    ))
}

/// What `place` gives: the page, its grid, the steps `Δk` and, for a fragment fitted to the page's grid, the
/// fragment's grid.
pub(super) type Placed = (std::sync::Arc<Page>, Grid, [i64; 2], Option<Grid>);

/// How effect 5 moves the fragment: by grid steps, or (a macro inserted with an orientation) turned and
/// mirrored about the anchor first.
#[derive(Clone, Copy, Debug)]
pub(super) struct Motion {
    /// The page's connection grid.
    grid: Grid,
    /// `Δk = index(at) − index(anchor)` per axis.
    steps: [i64; 2],
    /// The orientation about the anchor; `None` for `R0`, which moves by the steps alone.
    turn: Option<Turn>,
}

/// An orientation about the anchor, then the move to the target point ([`Motion`]).
#[derive(Clone, Copy, Debug)]
pub(super) struct Turn {
    orient: Orient2,
    /// The anchor and its grid indices.
    anchor: Pt2,
    ka: [i64; 2],
    /// The target point and its grid indices.
    at: Pt2,
    kt: [i64; 2],
}

/// One axis of a point relative to the anchor: whole grid steps for a grid coordinate, nanometres for any other.
#[derive(Clone, Copy)]
pub(super) enum Off {
    Steps(i64),
    Nm(i64),
}

impl Off {
    fn neg(self, yes: bool) -> Option<Off> {
        if !yes {
            return Some(self);
        }
        Some(match self {
            Off::Steps(s) => Off::Steps(s.checked_neg()?),
            Off::Nm(n) => Off::Nm(n.checked_neg()?),
        })
    }
}

impl Motion {
    /// A move by `steps` grid steps.
    pub(super) fn steps(grid: Grid, steps: [i64; 2]) -> Motion {
        Motion {
            grid,
            steps,
            turn: None,
        }
    }

    /// The orientation `orient` about `anchor`, then the move of the anchor to `at` (both grid points); a move by
    /// grid steps for `R0`.
    pub(super) fn turned(grid: Grid, steps: [i64; 2], orient: Orient2, anchor: Pt2, at: Pt2) -> Motion {
        let turn = (!orient.is_default()).then(|| Turn {
            orient,
            anchor,
            ka: [grid.snap(anchor.x), grid.snap(anchor.y)],
            at,
            kt: [grid.snap(at.x), grid.snap(at.y)],
        });
        Motion { grid, steps, turn }
    }

    /// Whether the motion turns or mirrors.
    pub(super) fn turns(&self) -> bool {
        self.turn.is_some()
    }

    /// The new position of a point: by the steps ([`step_point`]); turned, a grid coordinate keeps its whole steps
    /// from the anchor and any other its nanometres, the orientation's matrix permutes and negates the two axes, and
    /// the result is placed from the target point.
    pub(super) fn point(&self, p: Pt2) -> Result<Pt2, OutOfRange> {
        let Some(t) = self.turn else {
            return step_point(self.grid, p, self.steps);
        };
        let grid = self.grid;
        let off = |v: Nm, k: i64, a: Nm| -> Option<Off> {
            if crate::geometry::on_grid(grid, v) {
                grid.snap(v).checked_sub(k).map(Off::Steps)
            } else {
                v.0.checked_sub(a.0).map(Off::Nm)
            }
        };
        let (Some(ox), Some(oy)) = (off(p.x, t.ka[0], t.anchor.x), off(p.y, t.ka[1], t.anchor.y)) else {
            return Err(OutOfRange);
        };
        let m = t.orient.matrix();
        let pick = |row: [i8; 2]| {
            if row[0] != 0 {
                ox.neg(row[0] < 0)
            } else {
                oy.neg(row[1] < 0)
            }
        };
        let place = |o: Option<Off>, k: i64, a: Nm| -> Result<Nm, OutOfRange> {
            let v = match o.ok_or(OutOfRange)? {
                Off::Steps(s) => k.checked_add(s).and_then(|k| grid.at(k)),
                Off::Nm(n) => a.0.checked_add(n).map(Nm),
            };
            v.filter(|v| crate::geometry::in_range(*v)).ok_or(OutOfRange)
        };
        Ok(Pt2::new(
            place(pick(m[0]), t.kt[0], t.at.x)?,
            place(pick(m[1]), t.kt[1], t.at.y)?,
        ))
    }

    /// A representation's new orientation: its own, then the motion's.
    pub(super) fn orient(&self, o: Orient2) -> Orient2 {
        self.turn.map_or(o, |t| o.then(t.orient))
    }

    /// A direction (10⁻³ degree, counter-clockwise from +x in the y-down frame) after the motion: mirrored to
    /// `180° − a`, then turned clockwise by the quarter turns, in `[0°, 360°)`; unchanged by a move.
    pub(super) fn angle(&self, a: Angle2) -> Angle2 {
        let Some(t) = self.turn else { return a };
        let mut v = i64::from(a.0);
        if t.orient.is_mirrored() {
            v = 180_000 - v;
        }
        v -= i64::from(t.orient.quarter_turns()) * 90_000;
        Angle2(i32::try_from(v.rem_euclid(360_000)).unwrap_or(0))
    }

    /// Effect 5 on a graphic: every shape point moves; turned, a rectangle's corners are re-ordered, an arc's
    /// start turns as a direction and its sweep changes sign under a mirror; a radius stays.
    pub(super) fn shape(&self, shape: &mut GraphicShape) -> Result<(), OutOfRange> {
        match shape {
            GraphicShape::Line(l) => {
                l.a = self.point(l.a)?;
                l.b = self.point(l.b)?;
            }
            GraphicShape::Polyline(p) | GraphicShape::Polygon(p) => {
                for p in &mut p.points {
                    *p = self.point(*p)?;
                }
            }
            GraphicShape::Rect(r) => {
                let (a, b) = (self.point(r.min)?, self.point(r.max)?);
                r.min = Pt2::new(a.x.min(b.x), a.y.min(b.y));
                r.max = Pt2::new(a.x.max(b.x), a.y.max(b.y));
            }
            GraphicShape::Circle(c) => c.centre = self.point(c.centre)?,
            GraphicShape::Arc(a) => {
                a.centre = self.point(a.centre)?;
                if let Some(t) = self.turn {
                    a.start = self.angle(a.start);
                    if t.orient.is_mirrored() {
                        a.sweep = Angle2(a.sweep.0.checked_neg().ok_or(OutOfRange)?);
                    }
                }
            }
            GraphicShape::Unknown(_) => {}
        }
        Ok(())
    }
}
