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
/// default, to `(functions[0], cp.fn_cp)` (doc 07 §5: the pin map is derived unless it deviates). A black box
/// (a symbol without CPs and a `box`) draws its box CPs instead, in stored order, each at
/// `rep.pos + rep.orient(cp.pos)` and mapped to `(functions[0], cp.idx)`, with `symbol_cp` = `cp.idx`: the rule of
/// `ew_derive`'s `cp_point`, the scene and the Wire tool's targets, so the CP-exact rule of [`require_route`] covers
/// a black box's pins as it covers a symbol's (both can sit up to 1 nm off a rational grid after a move or a
/// transform). A symbol copy that is not embedded yields no CPs; a CP whose position would leave ±[`MAX_COORD`] is
/// not drawn (the commands that place or move a representation refuse that with [`rep_cps_checked`]).
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
    let placed = |local: Pt2| {
        if point_in_range(local) {
            add(rep.pos, rep.orient.apply(local))
        } else {
            Err(OutOfRange)
        }
    };
    if symbol.item.cps.is_empty() {
        let (Some(b), Some(&function)) = (&rep.box_, rep.functions.first()) else {
            return Vec::new();
        };
        return b
            .cps
            .iter()
            .map(|cp| {
                placed(cp.pos).map(|pos| RepCp {
                    symbol_cp: cp.idx,
                    function,
                    idx: cp.idx,
                    pos,
                })
            })
            .collect();
    }
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
            Some(placed(cp.pos).map(|pos| RepCp {
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

/// Moves the route end next to a moved endpoint (the v0 stand-in for re-solving the route parts next to a moved CP,
/// doc 06 §5.4). `at_start` selects the first vertex (endpoint `a`), otherwise the last (endpoint `b`). The end
/// vertex moves by `delta`; the far end never moves (FT-423). Every segment that was horizontal or vertical stays
/// so:
///
/// - when the end's segment keeps its axis after the move, nothing else changes;
/// - else, when the neighbour is an interior vertex whose other segment is perpendicular to the end's segment (or
///   has no length), the neighbour follows on the moved axis (its other segment stays perpendicular);
/// - else (the neighbour is the far end, as on a straight two-vertex route, or its other segment is collinear) a
///   dog-leg is inserted: two vertices at the grid coordinate nearest the middle between the moved end and the
///   neighbour, so the end's segment leaves along its old axis and meets the neighbour along it.
///
/// A segment that was neither horizontal nor vertical keeps only its moved end. Consecutive equal vertices are
/// merged. On [`OutOfRange`] the route is unchanged.
pub fn follow_end(route: &mut Vec<Pt2>, grid: Grid, at_start: bool, delta: Pt2) -> Result<(), OutOfRange> {
    if at_start {
        follow_ends(route, grid, Some(delta), None)
    } else {
        follow_ends(route, grid, None, Some(delta))
    }
}

/// Moves both route ends: end `a` by `da` and end `b` by `db` (each by [`follow_end`]'s rule, end `a` first); equal
/// displacements translate the whole route. The far end of each step stays where the other displacement put it,
/// so every end that met its CP before meets the CP's new position after (FT-423). On [`OutOfRange`] the route is
/// unchanged.
pub fn follow_ends(
    route: &mut Vec<Pt2>,
    grid: Grid,
    da: Option<Pt2>,
    db: Option<Pt2>,
) -> Result<(), OutOfRange> {
    let mut next = route.clone();
    match (da, db) {
        (Some(a), Some(b)) if a == b => translate(&mut next, a)?,
        _ => {
            if let Some(a) = da {
                follow_start(&mut next, grid, a)?;
            }
            if let Some(b) = db {
                next.reverse();
                follow_start(&mut next, grid, b)?;
                next.reverse();
            }
            next.dedup();
        }
    }
    *route = next;
    Ok(())
}

/// The grid coordinate nearest the middle of `a` and `b`.
fn mid_on_grid(grid: Grid, a: Nm, b: Nm) -> Result<Nm, OutOfRange> {
    grid.snap_point(Nm(a.0 + (b.0 - a.0) / 2)).ok_or(OutOfRange)
}

/// [`follow_end`] at the first vertex.
fn follow_start(route: &mut Vec<Pt2>, grid: Grid, d: Pt2) -> Result<(), OutOfRange> {
    let Some(&old) = route.first() else {
        return Ok(());
    };
    let new = add(old, d)?;
    route[0] = new;
    if route.len() < 2 || d == Pt2::ORIGIN {
        return Ok(());
    }
    let nb = route[1];
    let horizontal = old.y == nb.y && old.x != nb.x;
    let vertical = old.x == nb.x && old.y != nb.y;
    let keeps = (horizontal && new.y == nb.y) || (vertical && new.x == nb.x);
    if keeps || !(horizontal || vertical) {
        return Ok(());
    }
    // The neighbour's other segment stays vertical (horizontal) when the neighbour moves along it, also when it has no
    // length yet (the other end's step left two equal vertices).
    let perpendicular = route
        .get(2)
        .is_some_and(|m| (horizontal && nb.x == m.x) || (vertical && nb.y == m.y));
    if perpendicular {
        route[1] = if horizontal {
            Pt2::new(nb.x, new.y)
        } else {
            Pt2::new(new.x, nb.y)
        };
    } else if horizontal {
        let xm = mid_on_grid(grid, new.x, nb.x)?;
        route.splice(1..1, [Pt2::new(xm, new.y), Pt2::new(xm, nb.y)]);
    } else {
        let ym = mid_on_grid(grid, new.y, nb.y)?;
        route.splice(1..1, [Pt2::new(new.x, ym), Pt2::new(nb.x, ym)]);
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

/// The refusal of a point off the grid of `page`, in words (a refusal reaches the window's banner, FT-756): what
/// the point is, its position in mm and its page, no nm and no JSON Pointer ("the wire's corner at x 77, y 161 mm
/// on page 101 is not on the drawing grid").
pub(crate) fn off_grid(tx: &TxBuilder<'_>, what: &str, page: Id<Page>, p: Pt2) -> String {
    format!(
        "{what} at {} is not on the drawing grid",
        crate::words::point(tx, page, p)
    )
}

/// What the JSON Pointer of a position checked by [`require_on_grid`] names, in words: a split point
/// (`…/on_connection/at`), an outline vertex (`/outline/<i>`), a box CP (`…/cps/<i>/pos`), else a position.
fn what_of(field: &str) -> &'static str {
    if field.ends_with("/at") {
        "the branch point"
    } else if field.starts_with("/outline/") {
        "the outline's corner"
    } else if field.contains("cps/") {
        "the box's connection point"
    } else {
        "the position"
    }
}

/// D3 (doc 06 §5.7: CPs never leave the connection grid, in any tool and any origin): a position at which
/// a command places or to which it moves a representation must lie on the connection grid of its page
/// (`Page.grid`, ADR-0027) and within ±[`MAX_COORD`]. Front-ends snap before they submit. `field` is the
/// JSON Pointer of the parameter (`/pos`), named in the range refusal; the grid refusal names the point in words
/// (as in "the position at x 8.4, y 15.9 mm on page 102 is not on the drawing grid").
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
    tx.reject(off_grid(tx, what_of(field), page, pos))
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
    // Not accepted: it is out of range or off the grid, which `require_on_grid` refuses.
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
/// - every vertex lies within ±[`MAX_COORD`];
/// - the route is tidy (FT-437, [`route_defect`]): no vertex repeats the one before it, and no segment
///   runs over another one (a stretch of positive length shared on one line, a doubling back included).
///   A collinear vertex where the route runs straight on is no defect.
///
/// `field` is the JSON Pointer of the route parameter (`/route`); a refusal names the vertex
/// (`/route/2`), for an overlap the later segment's first vertex, except the grid refusal, which names the vertex in
/// words (as in "the wire's corner at x 15.9, y 7.9 mm on page 102 is not on the drawing grid"; the first
/// vertex is the wire's start, the last its end).
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
                "`{field}/{i}` {} does not meet connection point {idx} of {} at {}; end the route at its \
                 connection point",
                fmt_pt(v),
                crate::words::function(tx, *function),
                fmt_pt(cp)
            ));
        }
        exact.push(cp);
    }
    for (i, v) in route.iter().enumerate() {
        let ok_x = on_grid(grid, v.x) || exact.iter().any(|c| c.x == v.x);
        let ok_y = on_grid(grid, v.y) || exact.iter().any(|c| c.y == v.y);
        if !(ok_x && ok_y) {
            let what = if i == 0 {
                "the wire's start"
            } else if i + 1 == route.len() {
                "the wire's end"
            } else {
                "the wire's corner"
            };
            return tx.reject(off_grid(tx, what, page, *v));
        }
    }
    match route_defect(route) {
        None => Ok(()),
        Some(RouteDefect::Duplicate(i)) => tx.reject(format!(
            "`{field}/{i}` {} repeats the vertex before it; remove the duplicate vertex",
            fmt_pt(route[i])
        )),
        Some(RouteDefect::Overlap(i, j)) => tx.reject(format!(
            "`{field}/{j}` the segment from {} to {} runs over the segment from {} to {} (`{field}/{i}`); a \
             route never overlaps itself, remove the stretch it runs back over",
            fmt_pt(route[j]),
            fmt_pt(route[j + 1]),
            fmt_pt(route[i]),
            fmt_pt(route[i + 1])
        )),
    }
}

/// What makes a route untidy (FT-437, [`route_defect`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RouteDefect {
    /// Vertex `i` equals vertex `i − 1`.
    Duplicate(usize),
    /// The segment from vertex `j` runs over the segment from vertex `i` (`i < j`): the two share a stretch of
    /// positive length on one line.
    Overlap(usize, usize),
}

/// A segment on its line: (reduced direction, offset, interval start, interval end, segment index).
type LineSeg = ((i128, i128), i128, i128, i128, usize);

/// The first defect of a route whose vertices lie within ±[`MAX_COORD`]: the first repeated vertex, else an
/// overlapping pair of segments, else `None`. Segments of any direction count (a route is meant to be orthogonal,
/// but this check does not require it); two segments that only touch at a point do not overlap, so a collinear
/// vertex where the route runs straight on is no defect, and a U-turn is. Deterministic, `O(n log n)`: the segments
/// are grouped by their line and each line's intervals swept in order; the pair reported is the first the sweep
/// meets (the least by line, then by interval start, then by index).
pub fn route_defect(route: &[Pt2]) -> Option<RouteDefect> {
    if let Some(i) = (1..route.len()).find(|&i| route[i] == route[i - 1]) {
        return Some(RouteDefect::Duplicate(i));
    }
    // Each segment as (line direction, line offset, interval start, interval end, index), in i128: the direction
    // reduced by the gcd with its first non-zero component positive, the offset its cross product with the start,
    // the interval the projections of the two ends on the direction.
    let mut segs: Vec<LineSeg> = route
        .windows(2)
        .enumerate()
        .map(|(k, w)| {
            let (ax, ay) = (i128::from(w[0].x.0), i128::from(w[0].y.0));
            let (bx, by) = (i128::from(w[1].x.0), i128::from(w[1].y.0));
            let (mut dx, mut dy) = (bx - ax, by - ay);
            let g = gcd(dx.unsigned_abs(), dy.unsigned_abs()).max(1) as i128;
            (dx, dy) = (dx / g, dy / g);
            if dx < 0 || (dx == 0 && dy < 0) {
                (dx, dy) = (-dx, -dy);
            }
            let offset = dx * ay - dy * ax;
            let (ta, tb) = (dx * ax + dy * ay, dx * bx + dy * by);
            ((dx, dy), offset, ta.min(tb), ta.max(tb), k)
        })
        .collect();
    segs.sort_unstable();
    // The segment reaching furthest along the current line so far: (line, offset, end, index).
    let mut reach: Option<((i128, i128), i128, i128, usize)> = None;
    for &(dir, offset, lo, hi, k) in &segs {
        match reach {
            Some((d, o, end, i)) if d == dir && o == offset => {
                if lo < end {
                    return Some(RouteDefect::Overlap(i.min(k), i.max(k)));
                }
                if hi > end {
                    reach = Some((dir, offset, hi, k));
                }
            }
            _ => reach = Some((dir, offset, hi, k)),
        }
    }
    None
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
