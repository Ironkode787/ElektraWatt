// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integer geometry of the editor commands (ARCH §4.5: i64 nanometres, no float equality): the positions of
//! a representation's connection points, point-on-route tests and route splitting.

use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{Function, LibCopy, Page, Representation, SymbolDef};
use ew_units::{Nm, Pt2};

use crate::tx::TxBuilder;

/// One drawn connection point of a representation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RepCp {
    /// The symbol's CP index (`SymbolCp.idx`).
    pub symbol_cp: u16,
    /// The function the CP belongs to.
    pub function: Id<Function>,
    /// The function's CP index.
    pub idx: u16,
    /// The position on the page.
    pub pos: Pt2,
}

/// `a + b`, saturating (coordinates stay far inside ±2⁵³ in practice; saturation keeps the arithmetic
/// total).
pub fn add(a: Pt2, b: Pt2) -> Pt2 {
    Pt2::new(Nm(a.x.0.saturating_add(b.x.0)), Nm(a.y.0.saturating_add(b.y.0)))
}

/// `a − b`, saturating.
pub fn sub(a: Pt2, b: Pt2) -> Pt2 {
    Pt2::new(Nm(a.x.0.saturating_sub(b.x.0)), Nm(a.y.0.saturating_sub(b.y.0)))
}

/// The drawn CPs of a representation, in symbol-CP order: the symbol copy's CPs (base variant) placed at
/// `rep.pos + rep.orient(cp.pos)`, each mapped to a function CP by the stored `pin_map` entry or, by
/// default, to `(functions[0], cp.fn_cp)` (doc 07 §5: the pin map is derived unless it deviates). A symbol
/// copy that is not embedded yields no CPs.
pub fn rep_cps(tx: &TxBuilder<'_>, rep: &Representation) -> Vec<RepCp> {
    let Some(symbol): Option<Arc<LibCopy<SymbolDef>>> = tx.get::<LibCopy<SymbolDef>>(rep.symbol) else {
        return Vec::new();
    };
    symbol
        .item
        .cps
        .as_slice()
        .iter()
        .filter_map(|cp| {
            let (function, idx) = match rep.pin_map.get(&cp.idx) {
                Some(p) => (p.function, p.idx),
                None => (*rep.functions.first()?, cp.fn_cp),
            };
            Some(RepCp {
                symbol_cp: cp.idx,
                function,
                idx,
                pos: add(rep.pos, rep.orient.apply(cp.pos)),
            })
        })
        .collect()
}

/// The position of function CP `(function, idx)` as drawn by its representation on `page` (the smallest
/// representation ID if several draw it); `None` when no representation on that page draws it or the
/// symbol geometry is unknown.
pub fn cp_position(tx: &TxBuilder<'_>, function: Id<Function>, idx: u16, page: Id<Page>) -> Option<Pt2> {
    tx.reps_of_function(function).into_iter().find_map(|rid| {
        let rep = tx.get::<Representation>(rid)?;
        if rep.page != page {
            return None;
        }
        rep_cps(tx, &rep)
            .into_iter()
            .find(|c| c.function == function && c.idx == idx)
            .map(|c| c.pos)
    })
}

/// Whether `p` lies on the closed segment `a`–`b` (exact: collinear by an `i128` cross product, inside the
/// bounding box).
pub fn on_segment(p: Pt2, a: Pt2, b: Pt2) -> bool {
    let (px, py) = (i128::from(p.x.0), i128::from(p.y.0));
    let (ax, ay) = (i128::from(a.x.0), i128::from(a.y.0));
    let (bx, by) = (i128::from(b.x.0), i128::from(b.y.0));
    let cross = (bx - ax) * (py - ay) - (by - ay) * (px - ax);
    cross == 0 && px >= ax.min(bx) && px <= ax.max(bx) && py >= ay.min(by) && py <= ay.max(by)
}

/// Where `p` lies on a polyline: `(segment index, Manhattan distance from the segment's start)`, for the
/// first segment containing it. The pair orders points along the polyline from its first vertex; `None`
/// when `p` is not on it or the polyline has fewer than two vertices.
pub fn locate(route: &[Pt2], p: Pt2) -> Option<(usize, i128)> {
    route.windows(2).enumerate().find_map(|(i, w)| {
        on_segment(p, w[0], w[1]).then(|| {
            let dx = (i128::from(p.x.0) - i128::from(w[0].x.0)).abs();
            let dy = (i128::from(p.y.0) - i128::from(w[0].y.0)).abs();
            (i, dx + dy)
        })
    })
}

/// Splits a polyline at a point on it: the vertices up to the point (ending with it) and from the point
/// (starting with it). A point equal to a vertex is not repeated beyond those two halves. `None` when the
/// point is not on the polyline.
pub fn split_route(route: &[Pt2], at: Pt2) -> Option<(Vec<Pt2>, Vec<Pt2>)> {
    let (seg, _) = locate(route, at)?;
    let mut first: Vec<Pt2> = route[..=seg].to_vec();
    if first.last() != Some(&at) {
        first.push(at);
    }
    let mut second = vec![at];
    second.extend(route[seg + 1..].iter().copied().skip_while(|v| *v == at));
    Some((first, second))
}

/// Moves the route end next to a moved endpoint and keeps the adjacent segment orthogonal (the v0
/// stand-in for re-solving the route parts next to a moved CP, doc 06 §5.4). `at_start` selects the first
/// vertex (endpoint `a`), otherwise the last (endpoint `b`). The end vertex moves by `delta`; its
/// neighbour follows on the axis that keeps their segment horizontal or vertical.
pub fn follow_end(route: &mut [Pt2], at_start: bool, delta: Pt2) {
    let n = route.len();
    if n == 0 {
        return;
    }
    let (end, next) = if at_start {
        (0, 1)
    } else {
        (n - 1, n.wrapping_sub(2))
    };
    let old = route[end];
    route[end] = add(old, delta);
    if n >= 2 {
        let neighbour = route[next];
        if neighbour.y == old.y && neighbour.x != old.x {
            route[next] = Pt2::new(neighbour.x, route[end].y);
        } else if neighbour.x == old.x && neighbour.y != old.y {
            route[next] = Pt2::new(route[end].x, neighbour.y);
        }
    }
}

/// Translates every vertex of a route.
pub fn translate(route: &mut [Pt2], delta: Pt2) {
    for v in route {
        *v = add(*v, delta);
    }
}
