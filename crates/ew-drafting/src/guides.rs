// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Placing by hand, helped (docs/design/06a-drafting.md §6): the guides of a part that hangs on the pointer or is
//! dragged, and the light nudge onto an In-line guide's grid line.
//!
//! The part stands on the grid point the ordinary snap gives (its `at`, which the caller sets). [`guides`] never
//! computes a place for it: the nudge only decides between that grid line and its neighbour, across an In-line guide,
//! when the pointer is within [`GUIDE_CATCH`] of the neighbour (the ordinary snap takes a line within half a step),
//! and never more than [`GUIDE_CATCH_PT`] screen points beyond the half step. Level guides never nudge. Nothing is
//! stored: the caller asks again on every move, and nothing moves the part once it is set down.
//!
//! - **In line:** a pin of the hanging part that leaves on the side opposite to a pin of another part, stands on the
//!   side that pin faces, and lies within [`GUIDE_SHOW`] across of that pin's line, both pins inside the drawing area
//!   and no other part's body on the line between them (a pin it is already wired to counts whatever stands between).
//!   Per pin of the hanging part the nearest such pin across, then along. Drawn from that pin along its side through
//!   the hanging part; solid, with a tick across, when the two pins stand in line.
//! - **Level:** the nearest part across that is drawn with the same pins (the same points and sides: the same symbol
//!   in the same orientation), whose body shares the hanging part's height and whose origin is within [`GUIDE_SHOW`] of
//!   the hanging part's level. Drawn at that part's level through its top pin and the hanging part's.

use crate::consts::{GUIDE_CATCH, GUIDE_CATCH_PT, GUIDE_SHOW, TICKS_PER_STEP};
use crate::geom;
use crate::model::{DraftPage, End, Part, Pin, Pt, Rect, Side, Tick};

/// What a guide shows.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum GuideKind {
    /// A pin of the hanging part near the line of a pin it faces.
    InLine,
    /// The hanging part near the level of a part drawn alike in its row.
    Level,
}

/// A guide to draw, on the page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Guide {
    /// What it shows.
    pub kind: GuideKind,
    /// The line: an In-line guide's from the pin it faces along that pin's side through the hanging part, a Level
    /// guide's through the two parts' top pins at the level of the other part's.
    pub line: (Pt, Pt),
    /// Whether the pins stand in line (an In-line guide is then drawn solid, with [`Guide::tick`]).
    pub in_line: bool,
    /// The tick across an In-line guide whose pins stand in line: half a step from the hanging part's pin, towards
    /// the pin it faces, half a step long.
    pub tick: Option<(Pt, Pt)>,
}

/// What [`guides`] gives: the place of the hanging part and the guides to draw there.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Guided {
    /// The hanging part's origin: the ordinary snap's, or with the nudge its neighbouring grid line across an In-line
    /// guide.
    pub at: Pt,
    /// The guides at that place, In-line guides first.
    pub guides: Vec<Guide>,
}

/// The guides of `ghost`, the part that hangs on the pointer or is dragged, on `page`: `ghost.at` is the ordinary
/// snap's place, `pointer` the origin the pointer asks for (unsnapped, in ticks) and `points_per_step` the zoom in
/// screen points per grid step. A part of the page with `ghost`'s ID (the part being dragged) is not on the page for
/// it. The place is always `ghost.at` or a whole step from it, so it is a grid point when `ghost.at` is one.
pub fn guides(page: &DraftPage, ghost: &Part, pointer: Pt, points_per_step: f64) -> Guided {
    let catch = catch(points_per_step);
    let facing = in_line(page, ghost);
    let mut at = ghost.at;
    for vertical in [true, false] {
        // Across a vertical guide the nudge moves the part in x, across a horizontal one in y.
        let across = |p: Pt| if vertical { p.x } else { p.y };
        let lines: Vec<&Facing> = facing.iter().filter(|f| f.side.vertical() == vertical).collect();
        // A guide already in line at the ordinary snap's place keeps the part there.
        if lines.iter().any(|f| across(f.faced) == across(f.pin)) {
            continue;
        }
        let taken = lines
            .iter()
            .filter_map(|f| {
                let d = across(f.faced).saturating_sub(across(f.pin));
                let t = across(ghost.at).saturating_add(d);
                let off = across(pointer).saturating_sub(t).saturating_abs();
                (d.saturating_abs() == TICKS_PER_STEP && off <= catch).then_some((off, t))
            })
            .min();
        if let Some((_, t)) = taken {
            if vertical {
                at.x = t;
            } else {
                at.y = t;
            }
        }
    }
    let placed = Part { at, ..ghost.clone() };
    let body = placed.bounds();
    let half = TICKS_PER_STEP / 2;
    let mut out: Vec<Guide> = in_line(page, &placed)
        .into_iter()
        .map(|f| {
            let v = f.side.unit();
            // Along the faced pin's side, past the far edge of the hanging part's body and its pin.
            let end = match f.side {
                Side::Down => Pt::new(f.faced.x, body.y1.max(f.pin.y).saturating_add(half)),
                Side::Up => Pt::new(f.faced.x, body.y0.min(f.pin.y).saturating_sub(half)),
                Side::Right => Pt::new(body.x1.max(f.pin.x).saturating_add(half), f.faced.y),
                Side::Left => Pt::new(body.x0.min(f.pin.x).saturating_sub(half), f.faced.y),
            };
            let in_line = if f.side.vertical() {
                f.pin.x == f.faced.x
            } else {
                f.pin.y == f.faced.y
            };
            let tick = in_line.then(|| {
                // Half a step from the hanging pin towards the faced one, a quarter step to each side.
                let c = f.pin.minus(Pt::new(v.x * half, v.y * half));
                let q = half / 2;
                let w = Pt::new(v.y.abs() * q, v.x.abs() * q);
                (c.minus(w), c.plus(w))
            });
            Guide {
                kind: GuideKind::InLine,
                line: (f.faced, end),
                in_line,
                tick,
            }
        })
        .collect();
    out.extend(level(page, &placed));
    out.sort_by_key(|g| (g.kind, g.line));
    Guided { at, guides: out }
}

/// How far from a guide's line the pointer takes it, in ticks: [`GUIDE_CATCH`], but never more than
/// [`GUIDE_CATCH_PT`] screen points beyond the ordinary half step at `points_per_step`.
fn catch(points_per_step: f64) -> Tick {
    let half = TICKS_PER_STEP / 2;
    let extra = f64::from(GUIDE_CATCH_PT) * f64::from(TICKS_PER_STEP) / points_per_step;
    let extra = if extra.is_nan() {
        0
    } else {
        // Clamped to 0..=4 ticks before the cast.
        extra.floor().clamp(0.0, f64::from(GUIDE_CATCH - half)) as Tick
    };
    half + extra
}

/// A pin of the hanging part and the pin of another part it faces, on the page.
struct Facing {
    /// The hanging part's pin.
    pin: Pt,
    /// The pin it faces.
    faced: Pt,
    /// The faced pin's side.
    side: Side,
}

/// The opposite side.
fn opposite(s: Side) -> Side {
    match s {
        Side::Up => Side::Down,
        Side::Down => Side::Up,
        Side::Left => Side::Right,
        Side::Right => Side::Left,
    }
}

/// Whether `p` lies inside the closed rectangle `r`.
fn holds(r: Rect, p: Pt) -> bool {
    r.holds(Rect::new(p, p))
}

/// Per pin of `ghost` at its place, the pin of another part it faces (module documentation), the nearest across, then
/// along, then by point.
fn in_line(page: &DraftPage, ghost: &Part) -> Vec<Facing> {
    let wired = |a: &Pin, b: &Pin| {
        page.wires
            .iter()
            .any(|w| w.ends == [End::Pin(a.id), End::Pin(b.id)] || w.ends == [End::Pin(b.id), End::Pin(a.id)])
    };
    let mut out = Vec::new();
    for h in &ghost.pins {
        let Some(hs) = h.side else { continue };
        let ph = ghost.at.plus(h.at);
        if !holds(page.area, ph) {
            continue;
        }
        let mut best: Option<((Tick, i64, Pt), Facing)> = None;
        for part in page.parts.iter().filter(|p| p.id != ghost.id) {
            for o in &part.pins {
                if o.side != Some(opposite(hs)) {
                    continue;
                }
                let side = opposite(hs);
                let po = part.at.plus(o.at);
                let v = side.unit();
                let d = ph.minus(po);
                let along = i64::from(d.x) * i64::from(v.x) + i64::from(d.y) * i64::from(v.y);
                let across = if side.vertical() { d.x } else { d.y }.saturating_abs();
                if along <= 0 || across > GUIDE_SHOW || !holds(page.area, po) {
                    continue;
                }
                // The faced pin's line up to the hanging pin's level, clear of every other part's body.
                let level = if side.vertical() {
                    Pt::new(po.x, ph.y)
                } else {
                    Pt::new(ph.x, po.y)
                };
                let between = page.parts.iter().any(|q| {
                    q.id != ghost.id
                        && q.id != part.id
                        && !q.frame_only
                        && geom::crosses((po, level), q.bounds())
                });
                if between && !wired(h, o) {
                    continue;
                }
                let key = (across, along, po);
                if best.as_ref().is_none_or(|(k, _)| key < *k) {
                    best = Some((
                        key,
                        Facing {
                            pin: ph,
                            faced: po,
                            side,
                        },
                    ));
                }
            }
        }
        out.extend(best.map(|(_, f)| f));
    }
    out
}

/// The Level guide of `ghost` at its place (module documentation), if any.
fn level(page: &DraftPage, ghost: &Part) -> Option<Guide> {
    let shape = |p: &Part| {
        let mut v: Vec<(Pt, Option<Side>)> = p.pins.iter().map(|q| (q.at, q.side)).collect();
        v.sort();
        v
    };
    let pins = shape(ghost);
    let top = pins.iter().map(|(at, _)| *at).min_by_key(|at| (at.y, at.x))?;
    let body = ghost.bounds();
    let other = page
        .parts
        .iter()
        .filter(|p| {
            let b = p.bounds();
            p.id != ghost.id
                && !p.frame_only
                && p.at.y.abs_diff(ghost.at.y) <= GUIDE_SHOW.unsigned_abs()
                && b.y0 < body.y1
                && body.y0 < b.y1
                && shape(p) == pins
        })
        .min_by_key(|p| (p.at.x.abs_diff(ghost.at.x), p.at, p.id))?;
    let (a, b) = (other.at.plus(top), ghost.at.plus(top));
    let half = TICKS_PER_STEP / 2;
    let (x0, x1) = (
        a.x.min(b.x).saturating_sub(half),
        a.x.max(b.x).saturating_add(half),
    );
    Some(Guide {
        kind: GuideKind::Level,
        line: (Pt::new(x0, a.y), Pt::new(x1, a.y)),
        in_line: a.y == b.y,
        tick: None,
    })
}
