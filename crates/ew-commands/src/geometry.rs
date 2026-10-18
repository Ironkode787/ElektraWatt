// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integer geometry of the editor commands (ARCH §4.5: i64 nanometres, no float equality): the positions of
//! a representation's connection points, point-on-route tests, route splitting and the connection-grid
//! checks of doc 06 §5.7.
//!
//! **Range.** Every stored coordinate lies within ±[`MAX_COORD`] = ±(2⁵³ − 1) nm, the range a record can
//! be written in (ADR-0005). The arithmetic here is checked: a result outside that range is
//! [`OutOfRange`], which the commands turn into a `Rejected` naming the parameter. Point tests treat a
//! point outside the range as lying on nothing, which also keeps their `i128` products from overflowing.

use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{Connection, Endpoint, Function, LibCopy, Page, Representation, SymbolDef};
use ew_units::{Grid, Nm, Pt2};

use crate::error::CmdError;
use crate::tx::TxBuilder;

/// The largest coordinate magnitude a record can hold: 2⁵³ − 1 nm (ADR-0005).
pub const MAX_COORD: i64 = (1 << 53) - 1;

/// A coordinate computation left ±[`MAX_COORD`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OutOfRange;

impl std::fmt::Display for OutOfRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "a coordinate would leave the drawable range of ±{MAX_COORD} nm"
        )
    }
}

/// Whether a coordinate is within ±[`MAX_COORD`].
pub fn in_range(x: Nm) -> bool {
    (-MAX_COORD..=MAX_COORD).contains(&x.0)
}

/// Whether both coordinates of a point are within ±[`MAX_COORD`].
pub fn point_in_range(p: Pt2) -> bool {
    in_range(p.x) && in_range(p.y)
}

fn coord(v: Option<i64>) -> Result<Nm, OutOfRange> {
    v.map(Nm).filter(|x| in_range(*x)).ok_or(OutOfRange)
}

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

/// `a + b`, checked: [`OutOfRange`] when a coordinate of the result leaves ±[`MAX_COORD`].
pub fn add(a: Pt2, b: Pt2) -> Result<Pt2, OutOfRange> {
    Ok(Pt2::new(
        coord(a.x.0.checked_add(b.x.0))?,
        coord(a.y.0.checked_add(b.y.0))?,
    ))
}

/// `a − b`, checked like [`add`].
pub fn sub(a: Pt2, b: Pt2) -> Result<Pt2, OutOfRange> {
    Ok(Pt2::new(
        coord(a.x.0.checked_sub(b.x.0))?,
        coord(a.y.0.checked_sub(b.y.0))?,
    ))
}

/// The drawn CPs of a representation, in symbol-CP order: the symbol copy's CPs (base variant) placed at
/// `rep.pos + rep.orient(cp.pos)`, each mapped to a function CP by the stored `pin_map` entry or, by
/// default, to `(functions[0], cp.fn_cp)` (doc 07 §5: the pin map is derived unless it deviates). A symbol
/// copy that is not embedded yields no CPs; a CP whose position would leave ±[`MAX_COORD`] is not drawn
/// (the commands that place or move a representation refuse that with [`rep_cps_checked`]).
pub fn rep_cps(tx: &TxBuilder<'_>, rep: &Representation) -> Vec<RepCp> {
    cps_of(tx, rep).into_iter().filter_map(Result::ok).collect()
}

/// [`rep_cps`], but [`OutOfRange`] when a drawn CP's position leaves ±[`MAX_COORD`].
pub fn rep_cps_checked(tx: &TxBuilder<'_>, rep: &Representation) -> Result<Vec<RepCp>, OutOfRange> {
    cps_of(tx, rep).into_iter().collect()
}

fn cps_of(tx: &TxBuilder<'_>, rep: &Representation) -> Vec<Result<RepCp, OutOfRange>> {
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
            let local = cp.pos;
            let pos = if point_in_range(local) {
                add(rep.pos, rep.orient.apply(local))
            } else {
                Err(OutOfRange)
            };
            Some(pos.map(|pos| RepCp {
                symbol_cp: cp.idx,
                function,
                idx,
                pos,
            }))
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
/// bounding box). A point outside ±[`MAX_COORD`] lies on nothing; within the range the differences are
/// below 2⁵⁴ and their products below 2¹⁰⁸, so the `i128` arithmetic cannot overflow.
pub fn on_segment(p: Pt2, a: Pt2, b: Pt2) -> bool {
    if !(point_in_range(p) && point_in_range(a) && point_in_range(b)) {
        return false;
    }
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
/// neighbour follows on the axis that keeps their segment horizontal or vertical. On [`OutOfRange`] the
/// route is unchanged.
pub fn follow_end(route: &mut [Pt2], at_start: bool, delta: Pt2) -> Result<(), OutOfRange> {
    let n = route.len();
    if n == 0 {
        return Ok(());
    }
    let (end, next) = if at_start {
        (0, 1)
    } else {
        (n - 1, n.wrapping_sub(2))
    };
    let old = route[end];
    route[end] = add(old, delta)?;
    if n >= 2 {
        let neighbour = route[next];
        if neighbour.y == old.y && neighbour.x != old.x {
            route[next] = Pt2::new(neighbour.x, route[end].y);
        } else if neighbour.x == old.x && neighbour.y != old.y {
            route[next] = Pt2::new(route[end].x, neighbour.y);
        }
    }
    Ok(())
}

/// Translates every vertex of a route. On [`OutOfRange`] the route is unchanged.
pub fn translate(route: &mut [Pt2], delta: Pt2) -> Result<(), OutOfRange> {
    let moved = route
        .iter()
        .map(|v| add(*v, delta))
        .collect::<Result<Vec<Pt2>, OutOfRange>>()?;
    route.copy_from_slice(&moved);
    Ok(())
}

/// Whether a coordinate is a position of the grid: the stored (rounded) coordinate of a grid index
/// (`Grid::at(Grid::snap(x)) == x`, ADR-0027), the exact test of doc 06 §5.7.
pub fn on_grid(grid: Grid, x: Nm) -> bool {
    grid.snap_point(x) == Some(x)
}

/// Whether both coordinates of a point are grid positions.
pub fn point_on_grid(grid: Grid, p: Pt2) -> bool {
    on_grid(grid, p.x) && on_grid(grid, p.y)
}

fn fmt_pt(p: Pt2) -> String {
    format!("[{}, {}]", p.x.0, p.y.0)
}

fn grid_name(grid: Grid) -> String {
    if grid.den() == 1 {
        format!("{} nm", grid.num())
    } else {
        format!("{}/{} nm", grid.num(), grid.den())
    }
}

/// The nearest grid point, for the remedy of a refusal.
fn nearest(grid: Grid, p: Pt2) -> String {
    match (grid.snap_point(p.x), grid.snap_point(p.y)) {
        (Some(x), Some(y)) => fmt_pt(Pt2::new(x, y)),
        _ => "none in range".into(),
    }
}

/// D3 (doc 06 §5.7: CPs never leave the connection grid, in any tool and any origin): a position at which
/// a command places or to which it moves a representation must lie on the connection grid of its page
/// (`Page.grid`, ADR-0027) and within ±[`MAX_COORD`]. Front-ends snap before they submit. `field` is the
/// JSON Pointer of the parameter (`/pos`), named in the refusal together with the nearest grid point.
pub fn require_on_grid(tx: &TxBuilder<'_>, page: Id<Page>, pos: Pt2, field: &str) -> Result<(), CmdError> {
    let grid = tx.require::<Page>(page)?.grid;
    if !point_in_range(pos) {
        return tx.reject(format!(
            "`{field}` {} lies outside the drawable range of ±{MAX_COORD} nm",
            fmt_pt(pos)
        ));
    }
    if point_on_grid(grid, pos) {
        return Ok(());
    }
    tx.reject(format!(
        "`{field}` {} is not on the connection grid of page {page} ({}); snap it, for example to {}",
        fmt_pt(pos),
        grid_name(grid),
        nearest(grid, pos)
    ))
}

/// D3 for a split point: the point at which a junction is placed on connection `w` (drawn on `page`) by
/// `connection.draw` or a `device.place` hint. It is accepted when it is on the grid like any position
/// ([`require_on_grid`]), or, per coordinate that is off the grid, when that coordinate is the constant
/// coordinate of an axis-aligned segment of `w`'s route that contains the point and equals that
/// coordinate of one of `w`'s ends at a CP drawn on `page`: the CP-exact rule of [`require_route`] (a CP
/// can sit up to 1 nm off a rational grid, and the wire leaving it keeps that coordinate), so such a wire
/// stays branchable. No other tolerance applies. A point that is not on the route is left to the split
/// itself to refuse. `field` is the JSON Pointer of the point.
pub fn require_split_point(
    tx: &TxBuilder<'_>,
    w: &Connection,
    page: Id<Page>,
    at: Pt2,
    field: &str,
) -> Result<(), CmdError> {
    let grid = tx.require::<Page>(page)?.grid;
    if point_in_range(at) {
        let exact: Vec<Pt2> = [&w.a, &w.b]
            .into_iter()
            .filter_map(|e| match e {
                Endpoint::Cp(function, idx) => cp_position(tx, *function, *idx, page),
                _ => None,
            })
            .collect();
        // Whether `at` lies on a segment of the route that stays at coordinate `c` of `axis`.
        let on_constant = |axis: fn(Pt2) -> Nm, c: Nm| {
            w.route
                .windows(2)
                .any(|s| on_segment(at, s[0], s[1]) && axis(s[0]) == c && axis(s[1]) == c)
        };
        let ok_x = on_grid(grid, at.x) || (exact.iter().any(|c| c.x == at.x) && on_constant(|p| p.x, at.x));
        let ok_y = on_grid(grid, at.y) || (exact.iter().any(|c| c.y == at.y) && on_constant(|p| p.y, at.y));
        if ok_x && ok_y {
            return Ok(());
        }
    }
    // Not accepted: it is out of range or off the grid, which `require_on_grid` refuses with the remedy.
    require_on_grid(tx, page, at, field)
}

/// D3 for a route given to `connection.draw` or `connection.route` on `page` (an empty route has no
/// geometry and passes):
///
/// - an end at a CP that a representation on `page` draws must be exactly that CP's position (route
///   vertex 0 at endpoint `a`, the last vertex at endpoint `b`);
/// - every coordinate of every vertex lies on the page's connection grid, or equals that coordinate of
///   a route end that meets its CP: a CP position can sit up to 1 nm off a rational grid (the rounding
///   pitfall of `Grid::cell`), and the segment leaving it stays orthogonal;
/// - every vertex lies within ±[`MAX_COORD`].
///
/// `field` is the JSON Pointer of the route parameter (`/route`); a refusal names the vertex
/// (`/route/2`).
pub fn require_route(
    tx: &TxBuilder<'_>,
    page: Id<Page>,
    a: &Endpoint,
    b: &Endpoint,
    route: &[Pt2],
    field: &str,
) -> Result<(), CmdError> {
    let (Some(first), Some(last)) = (route.first().copied(), route.last().copied()) else {
        return Ok(());
    };
    let grid = tx.require::<Page>(page)?.grid;
    for (i, v) in route.iter().enumerate() {
        if !point_in_range(*v) {
            return tx.reject(format!(
                "`{field}/{i}` {} lies outside the drawable range of ±{MAX_COORD} nm",
                fmt_pt(*v)
            ));
        }
    }
    let mut exact: Vec<Pt2> = Vec::new();
    for (e, v, i) in [(a, first, 0), (b, last, route.len() - 1)] {
        let Endpoint::Cp(function, idx) = e else {
            continue;
        };
        let Some(cp) = cp_position(tx, *function, *idx, page) else {
            continue;
        };
        if v != cp {
            return tx.reject(format!(
                "`{field}/{i}` {} does not meet connection point {idx} of function {function} at {}; \
                 end the route at its CP",
                fmt_pt(v),
                fmt_pt(cp)
            ));
        }
        exact.push(cp);
    }
    for (i, v) in route.iter().enumerate() {
        let ok_x = on_grid(grid, v.x) || exact.iter().any(|c| c.x == v.x);
        let ok_y = on_grid(grid, v.y) || exact.iter().any(|c| c.y == v.y);
        if !(ok_x && ok_y) {
            return tx.reject(format!(
                "`{field}/{i}` {} is not on the connection grid of page {page} ({}); snap it, for \
                 example to {}",
                fmt_pt(*v),
                grid_name(grid),
                nearest(grid, *v)
            ));
        }
    }
    Ok(())
}
