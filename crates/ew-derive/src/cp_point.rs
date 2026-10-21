// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Where a function CP is drawn: `cp_point` and `cp_cell` (doc 06 §5.1, doc 08 §6.7).
//!
//! The position rules are those of the page scene (`ew-scene`, module `page`), restated over `Q` so that a
//! derived query (the wire numbers) can order wires by the frame cell of their ends:
//!
//! - A symbol CP `c` of a representation sits at `pos + orient(c.pos)` and draws the function CP of its
//!   `pin_map` entry, else `(functions[0], c.fn_cp)`. When several representations on the page draw the same
//!   function CP, the smallest representation ID wins.
//! - A **black box** (a symbol without CPs on a representation that carries `Representation.box`) draws box
//!   CP `c` of its first function at `pos + orient(c.pos)`.
//! - A **strip segment** draws the terminals of its strip (doc 08 §6.7, decision log ID-143, ID-180): segment
//!   `j` of the strip's `m` segments, in (page position, y, x, ID) order, draws the terminals
//!   `⌊jN/m⌋ … ⌊(j+1)N/m⌋ − 1` of its `N` terminals in (`seq`, ID) order. Terminal `k` of the segment has its
//!   side-1 point `k + 1` grid steps along the segment's x axis and its side-2 point the same distance from
//!   the bottom of the symbol (the maximum y of its graphics, CPs and text positions in `R0`, snapped to the
//!   symbol grid) below it; the steps turn with the orientation. The terminal's main function (else its first)
//!   draws the CPs of its sides; CPs without a side are not drawn.

use ew_ids::Id;
use ew_schema::{Device, FnRole, Function, Page, Primitive, RepType, Representation, Side, SymbolDef};
use ew_units::{Grid, Nm, Pt2};

use crate::cell::Cell;
use crate::{CellOf, DrawnPageOrder, FunctionCps, PageRef, Q};

/// The point of function CP `(function, idx)` as drawn on `page`, in nm on the page; `None` when no
/// representation or strip segment on that page draws it or a symbol copy is missing.
pub fn cp_point(q: &Q, function: Id<Function>, idx: u16, page: Id<Page>) -> Option<Pt2> {
    let mut reps: Vec<Id<Representation>> = q
        .idx()
        .reps_of_function(function)
        .filter(|r| q.reps().get(*r).is_some_and(|r| r.page == page))
        .collect();
    reps.sort();
    reps.into_iter()
        .find_map(|rid| {
            let rep = q.reps().get(rid)?;
            rep_cp_point(q, rep, function, idx)
        })
        .or_else(|| segment_cp_point(q, function, idx, page))
}

/// The frame cell of function CP `(function, idx)` as drawn on `page` (the cell of [`cp_point`]); `None`
/// when it is not drawn there or its point lies outside the frame grid.
pub fn cp_cell(q: &Q, function: Id<Function>, idx: u16, page: Id<Page>) -> Option<Cell> {
    let point = cp_point(q, function, idx, page)?;
    q.get::<CellOf>((page, point)).as_ref().clone()
}

/// The point at which `rep` draws `(function, idx)` by its symbol's CPs or its black box.
fn rep_cp_point(q: &Q, rep: &Representation, function: Id<Function>, idx: u16) -> Option<Pt2> {
    let symbol = q.symbols().get(rep.symbol)?;
    let placed = |local: Pt2| rep.pos.checked_add(rep.orient.checked_apply(local)?);
    let by_symbol = symbol.item.cps.as_slice().iter().find_map(|cp| {
        let (f, i) = match rep.pin_map.as_slice().iter().find(|p| p.symbol_cp == cp.idx) {
            Some(p) => (p.function, p.idx),
            None => (*rep.functions.first()?, cp.fn_cp),
        };
        ((f, i) == (function, idx)).then(|| placed(cp.pos)).flatten()
    });
    if by_symbol.is_some() || !symbol.item.cps.is_empty() {
        return by_symbol;
    }
    let b = rep.box_.as_ref()?;
    if rep.functions.first() != Some(&function) {
        return None;
    }
    b.cps.iter().find(|c| c.idx == idx).and_then(|c| placed(c.pos))
}

/// The point at which a strip segment on `page` draws terminal CP `(function, idx)`.
fn segment_cp_point(q: &Q, function: Id<Function>, idx: u16, page: Id<Page>) -> Option<Pt2> {
    let terminal = q.functions().get(function)?.device?;
    let strip = q.devices().get(terminal)?.aspects.terminal.as_ref()?.strip;
    // The terminal draws its main function (else its first one), and the CP must lie on one of its sides.
    let mut fns: Vec<Id<Function>> = q.idx().functions_of_device(terminal).collect();
    fns.sort();
    let main = fns
        .iter()
        .copied()
        .find(|f| q.functions().get(*f).is_some_and(|f| f.role == FnRole::Main))
        .or_else(|| fns.first().copied())?;
    if main != function {
        return None;
    }
    let cp = q
        .get::<FunctionCps>(function)
        .iter()
        .find(|c| c.idx == idx)
        .cloned()?;
    let side2 = match cp.side? {
        Side::Side1 => false,
        Side::Side2 => true,
        _ => return None,
    };

    // The strip's segments in (page position, y, x, ID) order.
    let order = q.get::<DrawnPageOrder>(());
    let mut segments: Vec<(usize, i64, i64, Id<Representation>)> = q
        .idx()
        .functions_of_device(strip)
        .collect::<Vec<_>>()
        .into_iter()
        .flat_map(|f| q.idx().reps_of_function(f).collect::<Vec<_>>())
        .filter_map(|r| q.reps().get(r))
        .filter(|r| r.rep_type == RepType::StripSegment)
        .map(|r| {
            let at = order.position(PageRef::Drawn(r.page)).unwrap_or(usize::MAX);
            (at, r.pos.y.0, r.pos.x.0, r.id)
        })
        .collect();
    segments.sort_unstable();
    segments.dedup();
    let m = segments.len();
    // The strip's terminals in (seq, ID) order.
    let mut terminals: Vec<(u32, Id<Device>)> = q
        .idx()
        .terminals_of_strip(strip)
        .filter_map(|t| Some((q.devices().get(t)?.aspects.terminal.as_ref()?.seq, t)))
        .collect();
    terminals.sort_unstable();
    let n = terminals.len();
    let t = terminals.iter().position(|(_, d)| *d == terminal)?;
    // Segment j draws terminals j·n/m .. (j+1)·n/m: the segment of terminal t and its place in it.
    let j = (0..m).find(|j| (j * n / m..(j + 1) * n / m).contains(&t))?;
    let k = t - j * n / m;
    let seg = q.reps().get(segments[j].3)?;
    if seg.page != page {
        return None;
    }
    let symbol = q.symbols().get(seg.symbol)?;
    let grid = symbol.item.grid;
    let bottom = grid.snap(Nm(symbol_bottom(&symbol.item)));
    let steps = i64::try_from(k + 1).ok()?;
    segment_point(grid, seg, steps, if side2 { bottom } else { 0 })
}

/// The page point `(i, j)` grid steps from a strip segment's origin in its symbol frame: the step vector
/// turns with the orientation; on a grid-point origin the result is the exact grid point, else the origin
/// plus the rounded offset (the rule of `ew-scene`). `None` on overflow.
fn segment_point(grid: Grid, rep: &Representation, i: i64, j: i64) -> Option<Pt2> {
    let v = rep.orient.checked_apply(Pt2::new(Nm(i), Nm(j)))?;
    let axis = |origin: Nm, steps: i64| {
        let k = grid.snap(origin);
        if grid.at(k) == Some(origin) {
            grid.at(k.checked_add(steps)?)
        } else {
            Some(Nm(origin.0.checked_add(grid.at(steps)?.0)?))
        }
    };
    Some(Pt2::new(axis(rep.pos.x, v.x.0)?, axis(rep.pos.y, v.y.0)?))
}

/// The largest y of a symbol in `R0`: its drawn graphics, CPs, text slots and text primitives, as the page
/// scene's symbol bounds take them (`ew-scene`, `symbol::lower`); 0 for a symbol without any point, whose
/// bounds are the origin.
fn symbol_bottom(def: &SymbolDef) -> i64 {
    let mut max: Option<i64> = None;
    let mut add = |y: Nm| max = Some(max.map_or(y.0, |m| m.max(y.0)));
    for prim in &def.graphics {
        match prim {
            Primitive::Line(p) if p.stroke.is_some() => {
                add(p.from.y);
                add(p.to.y);
            }
            Primitive::Polyline(p) if p.stroke.is_some() => p.points.iter().for_each(|p| add(p.y)),
            Primitive::Polygon(p) if p.stroke.is_some() || p.fill.is_some() => {
                p.points.iter().for_each(|p| add(p.y));
            }
            Primitive::Rect(p) if p.stroke.is_some() || p.fill.is_some() => {
                add(p.rect.0.min().y);
                add(p.rect.0.max().y);
            }
            Primitive::Circle(p) if p.stroke.is_some() || p.fill.is_some() => {
                add(Nm(p.centre.y.0.saturating_add(p.r.0)));
                add(Nm(p.centre.y.0.saturating_sub(p.r.0)));
            }
            Primitive::Ellipse(p) if p.stroke.is_some() || p.fill.is_some() => {
                add(p.rect.0.min().y);
                add(p.rect.0.max().y);
            }
            Primitive::Arc(p) if p.stroke.is_some() => {
                add(p.rect.0.min().y);
                add(p.rect.0.max().y);
            }
            Primitive::Image(p) => {
                add(p.rect.0.min().y);
                add(p.rect.0.max().y);
            }
            Primitive::Text(t) => add(t.pos.y),
            _ => {}
        }
    }
    def.cps.as_slice().iter().for_each(|c| add(c.pos.y));
    def.text_slots.iter().for_each(|s| add(s.pos.y));
    max.unwrap_or(0)
}
