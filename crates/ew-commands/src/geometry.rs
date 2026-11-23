// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integer geometry of the editor commands (ARCH §4.5: i64 nanometres, no float equality): the positions of
//! a representation's connection points, the path a connection without a route is drawn along, point-on-route
//! tests, route splitting and the connection-grid checks of doc 06 §5.7.
//!
//! **Range.** Every stored coordinate lies within ±[`MAX_COORD`] = ±(2⁵³ − 1) nm, the range a record can
//! be written in (ADR-0005). The arithmetic here is checked: a result outside that range is
//! [`OutOfRange`], which the commands turn into a `Rejected` in words. Point tests treat a
//! point outside the range as lying on nothing, which also keeps their `i128` products from overflowing.

use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{Connection, Dir4, Endpoint, Function, Junction, LibCopy, Page, Representation, SymbolDef};
use ew_units::{Grid, Nm, Pt2, arith};

use crate::error::CmdError;
use crate::text::{Text, TextArg, text};
use crate::tx::TxBuilder;

/// The largest coordinate magnitude a record can hold: 2⁵³ − 1 nm (ADR-0005).
pub const MAX_COORD: i64 = ew_units::MAX_SAFE_INTEGER;

/// A coordinate computation left ±[`MAX_COORD`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OutOfRange;

impl std::fmt::Display for OutOfRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "a coordinate would leave the drawable range; move it nearer the page origin"
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
    /// Whether a wire leaves it along y: its direction (`cp.dir`) turned by the representation's orientation, as
    /// the page draws it ([`ew_derive::oriented_dir`], [`ew_derive::dir_vertical`]); `None` when the direction is
    /// unknown.
    pub vertical: Option<bool>,
    /// The way a wire leaves it, a unit step on the page (`(0, −1)`: up), its direction turned as for `vertical`;
    /// `None` when the direction is unknown.
    pub out: Option<Pt2>,
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
/// `rep.pos + rep.orient(cp.pos)` and mapped to `(functions[0], cp.idx)`, with `symbol_cp` = `cp.idx`. A point within
/// 1 nm of a position of the page's grid is the grid's own number for it ([`Grid::settle_point`]). The rule of
/// `ew_derive`'s `cp_point`, the scene and the Wire tool's targets, so the CP-exact rule of [`require_route`] covers
/// a black box's pins as it covers a symbol's. A symbol copy that is not embedded yields no CPs; a CP whose position would leave ±[`MAX_COORD`] is
/// not drawn (the commands that place or move a representation refuse that with [`rep_cps_checked`]).
pub fn rep_cps(tx: &TxBuilder<'_>, rep: &Representation) -> Vec<RepCp> {
    cps_of(tx, rep).into_iter().filter_map(Result::ok).collect()
}

/// [`rep_cps`], but [`OutOfRange`] when a drawn CP's position leaves ±[`MAX_COORD`].
pub fn rep_cps_checked(tx: &TxBuilder<'_>, rep: &Representation) -> Result<Vec<RepCp>, OutOfRange> {
    cps_of(tx, rep).into_iter().collect()
}

/// The grid point of `grid` nearest `p`, axis by axis; [`OutOfRange`] on overflow.
pub(crate) fn nearest_grid_point(grid: Grid, p: Pt2) -> Result<Pt2, OutOfRange> {
    match (grid.snap_point(p.x), grid.snap_point(p.y)) {
        (Some(x), Some(y)) if in_range(x) && in_range(y) => Ok(Pt2::new(x, y)),
        _ => Err(OutOfRange),
    }
}

/// The position at which `rep` stands fitted to `grid` alone (a fitted symbol turned; a page's or a paste's items are
/// fitted together by [`fit`]): moved so that its first connection point ([`rep_cps`]' order) lies on the nearest grid
/// point, the others
/// keeping their places relative to it (a symbol whose own pin spacing does not fit the grid has its first pin on
/// it); a representation without connection points has its position there.
pub(crate) fn fitted_pos(tx: &TxBuilder<'_>, rep: &Representation, grid: Grid) -> Result<Pt2, OutOfRange> {
    let at = rep_cps_checked(tx, rep)?.first().map_or(rep.pos, |c| c.pos);
    let to = nearest_grid_point(grid, at)?;
    add(rep.pos, sub(to, at)?)
}

/// The body of `rep` as its symbol's graphics bound it (a drawn box: its outline), on the page, `[x0, y0, x1, y1]`.
pub(crate) fn rep_body(tx: &TxBuilder<'_>, rep: &Representation) -> Option<[i64; 4]> {
    let corners: Vec<Pt2> = match rep.box_.as_ref().filter(|b| !b.outline.is_empty()) {
        Some(b) => b.outline.iter().map(|p| rep.orient.apply(*p)).collect(),
        None => {
            let (lo, hi) = tx.get::<LibCopy<SymbolDef>>(rep.symbol)?.item.graphic_bounds()?;
            vec![rep.orient.apply(lo), rep.orient.apply(hi)]
        }
    };
    let xs = corners.iter().map(|p| p.x.0 + rep.pos.x.0);
    let ys = corners.iter().map(|p| p.y.0 + rep.pos.y.0);
    Some([xs.clone().min()?, ys.clone().min()?, xs.max()?, ys.max()?])
}

/// One thing the grid fit moves ([`fit`]): the point that goes onto the grid and the box it takes on the page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct FitItem {
    /// The point that goes onto the grid: a symbol's first connection point (its position without one), a junction's
    /// point.
    at: Pt2,
    /// The box's least corner.
    lo: Pt2,
    /// The box's greatest corner.
    hi: Pt2,
    /// The pins on the box's near and on its far side that leave along the axis, x then y, with a wire that does not
    /// run straight on to its other end (`faces[0][0]`: such pins on its left edge, whose wires leave towards what
    /// stands left of it and turn): each wants a grid line of its own beside that side.
    faces: [[u32; 2]; 2],
}

impl FitItem {
    /// A representation as the fit moves it: by its first connection point ([`rep_cps`]' order, its position without
    /// one), taking its body ([`rep_body`]) and its connection points.
    pub(crate) fn rep(tx: &TxBuilder<'_>, rep: &Representation) -> Result<FitItem, OutOfRange> {
        let cps = rep_cps_checked(tx, rep)?;
        let at = cps.first().map_or(rep.pos, |c| c.pos);
        let mut item = FitItem::point(at);
        let body =
            rep_body(tx, rep).map(|[x0, y0, x1, y1]| [Pt2::new(Nm(x0), Nm(y0)), Pt2::new(Nm(x1), Nm(y1))]);
        for p in cps.iter().map(|c| c.pos).chain(body.into_iter().flatten()) {
            item.lo = Pt2::new(item.lo.x.min(p.x), item.lo.y.min(p.y));
            item.hi = Pt2::new(item.hi.x.max(p.x), item.hi.y.max(p.y));
        }
        for c in &cps {
            let ax = match c.vertical {
                Some(vertical) => usize::from(vertical),
                None => continue,
            };
            if !turns(tx, c, rep.page, ax) {
                continue;
            }
            let v = on_axis(c.pos, ax);
            item.faces[ax][0] += u32::from(v == on_axis(item.lo, ax));
            item.faces[ax][1] += u32::from(v == on_axis(item.hi, ax));
        }
        Ok(item)
    }

    /// A point as the fit moves it (a junction): onto the grid, taking no room but its own.
    pub(crate) fn point(at: Pt2) -> FitItem {
        FitItem {
            at,
            lo: at,
            hi: at,
            faces: [[0; 2]; 2],
        }
    }
}

/// Whether CP `c` of a representation on `page`, whose wire leaves along axis `ax`, has a wire on the page that does not
/// run straight on to its other end (that end off the pin's line, or a route that leaves it): a wire that turns along
/// the side the pin is on, and so wants a grid line there ([`FitItem::faces`]).
fn turns(tx: &TxBuilder<'_>, c: &RepCp, page: Id<Page>, ax: usize) -> bool {
    let me = Endpoint::Cp(c.function, c.idx);
    let in_line = |p: &Pt2| on_axis(*p, 1 - ax) == on_axis(c.pos, 1 - ax);
    tx.connections_at(ew_model::EndpointKey::Cp(c.function, c.idx))
        .into_iter()
        .filter_map(|id| tx.get::<Connection>(id))
        .filter(|w| w.page == Some(page))
        .any(|w| {
            let other = if w.a == me { &w.b } else { &w.a };
            let straight =
                end_point(tx, other, page).is_some_and(|p| in_line(&p)) && w.route.iter().all(in_line);
            !straight
        })
}

/// What the grid fit keeps room for beside the gaps between the items it moves ([`fit`], doc 06 §8 item 3, FT-2010).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(crate) struct FitRoom {
    /// The drawing area as the router reads it, its least and greatest corners: an item with pins on a side that faces
    /// its edge, their wires turning along it ([`FitItem::faces`]), which stood a step or more from that edge, keeps a
    /// grid line for each between, a step from the edge, from each other and from the item.
    pub(crate) area: Option<[Pt2; 2]>,
    /// The straight wires between pins of two items that stood in line: each end as its item's index among the items
    /// and its pin's point.
    pub(crate) wires: Vec<[(usize, Pt2); 2]>,
}

/// A wire of [`FitRoom::wires`] whose pins stood in line on the axis one pass of [`fit`] moves along: its ends'
/// items, the pins' one coordinate on the axis, the extent it runs across, and, where the pin of only one end lands on
/// the grid (the other's symbol's own spacing keeps it off), that end: the line that keeps a step of room.
struct Line {
    ends: [usize; 2],
    at: i128,
    span: (i128, i128),
    room: Option<usize>,
}

/// A point's coordinate on axis `ax` (0: x, 1: y).
fn on_axis(p: Pt2, ax: usize) -> i128 {
    i128::from(if ax == 0 { p.x.0 } else { p.y.0 })
}

/// The wires of `room` whose pins stood in line on axis `ax`, as [`fit_axis`] reads them; `moved` is what the pass
/// of the other axis moved each item by.
fn lines(grid: Grid, items: &[FitItem], room: &FitRoom, ax: usize, moved: &[i128]) -> Vec<Line> {
    // Whether the pin at `pin` of item `k` lands on the grid when the item's point goes to its nearest grid point.
    let lands = |k: usize, pin: i128| {
        let a = on_axis(items[k].at, ax);
        i64::try_from(a)
            .ok()
            .and_then(|v| grid.at(grid.snap(Nm(v))))
            .and_then(|g| i64::try_from(i128::from(g.0) - a + pin).ok())
            .is_some_and(|v| on_grid(grid, grid.settle(Nm(v))))
    };
    room.wires
        .iter()
        .filter(|[(i, p), (j, q)]| i != j && *i.max(j) < items.len() && on_axis(*p, ax) == on_axis(*q, ax))
        .map(|&[(i, p), (j, q)]| {
            let (s, t) = (on_axis(p, 1 - ax) + moved[i], on_axis(q, 1 - ax) + moved[j]);
            let at = on_axis(p, ax);
            Line {
                ends: [i, j],
                at,
                span: (s.min(t), s.max(t)),
                room: match (lands(i, at), lands(j, at)) {
                    (true, false) => Some(i),
                    (false, true) => Some(j),
                    _ => None,
                },
            }
        })
        .collect()
}

/// The 1 nm of rounding a grid of fractions of a nanometre leaves between two whole steps, with a nanometre to spare: a
/// gap that closes by no more is kept.
const FIT_SLACK: i128 = 2;

/// The grid fit of doc 06 §8 item 3 (a page's grid changed, a paste onto a page of another grid): the displacement
/// that brings each of `items` onto `grid`, in their order. Axis by axis, x first, each item goes to the grid point
/// nearest its point; then, taken in the order of their boxes' near edges, each moves on by as few whole steps further
/// right (down) as keep the gap between it and every item before it that faces it: a gap of a step or more (room for
/// a wire) at least what it was or two steps, whichever is less; a narrower one at least open (no overlap). Two items
/// face each other along x when their boxes share some height, along y when they share some width after the x pass;
/// items whose boxes overlapped along the axis keep no rule between them. So no two items that stood apart end on one
/// another, things that face each other keep their order, and two pins more than a step apart keep a grid line between
/// them for a wire; what is given up is the exact nearest point of an item pushed on. With `room` (FT-2010) an item
/// with pins facing an edge of the drawing area whose wires turn along it, which stood a step or more from it, keeps a
/// grid line for each between, a step apart and a step from the edge and from the item (pushed on from the near edge,
/// taken back from the far one as far as its gaps allow); an item wired straight to
/// a pin of an item placed before it, the two pins in line, goes as near in line with that pin as the grid allows; and
/// where the pin of only one of the two lands on the grid, what faces that pin's line between the two pins keeps a
/// step from it. [`OutOfRange`] when a point would leave the drawable range.
pub(crate) fn fit(grid: Grid, items: &[FitItem], room: &FitRoom) -> Result<Vec<Pt2>, OutOfRange> {
    let across = |ax: usize, moved: &[i128]| -> Vec<(i128, i128)> {
        items
            .iter()
            .zip(moved)
            .map(|(i, d)| (on_axis(i.lo, ax) + d, on_axis(i.hi, ax) + d))
            .collect()
    };
    let edges = |ax: usize| room.area.map(|[lo, hi]| (on_axis(lo, ax), on_axis(hi, ax)));
    let still = vec![0; items.len()];
    let dx = fit_axis(
        grid,
        items,
        0,
        &across(1, &still),
        edges(0),
        &lines(grid, items, room, 0, &still),
    )?;
    let dy = fit_axis(
        grid,
        items,
        1,
        &across(0, &dx),
        edges(1),
        &lines(grid, items, room, 1, &dx),
    )?;
    dx.into_iter()
        .zip(dy)
        .map(|(a, b)| match (i64::try_from(a), i64::try_from(b)) {
            (Ok(a), Ok(b)) => Ok(Pt2::new(Nm(a), Nm(b))),
            _ => Err(OutOfRange),
        })
        .collect()
}

/// [`fit`] along axis `ax` (0: x, 1: y): the displacement of each item; `across` is each item's extent on the other
/// axis, by which two items face each other, `edges` the drawing area's least and greatest coordinate on the axis,
/// `lines` the straight wires whose pins stood in line on it.
fn fit_axis(
    grid: Grid,
    items: &[FitItem],
    ax: usize,
    across: &[(i128, i128)],
    edges: Option<(i128, i128)>,
    lines: &[Line],
) -> Result<Vec<i128>, OutOfRange> {
    let axis = |p: Pt2| on_axis(p, ax);
    let at = |k: i64| grid.at(k).map(|v| i128::from(v.0)).ok_or(OutOfRange);
    let index = |v: i128| i64::try_from(v).map(|v| grid.snap(Nm(v))).map_err(|_| OutOfRange);
    let (step, two) = (at(1)? - at(0)?, at(2)? - at(0)?);
    // What a gap keeps: room for a wire (a step or more) at least what it was up to two steps, else no overlap.
    let keep = |gap: i128| if gap >= step { gap.min(two) } else { 0 };
    let row_k = row_steps(grid, items, &axis, across, step, &keep)?;
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&i| (axis(items[i].lo), axis(items[i].hi), i));
    let mut shift = vec![0i128; items.len()];
    let mut placed = vec![false; items.len()];
    // Whether item `i` runs across where line `l` runs, between its pins, and is not one of its ends.
    let beside =
        |i: usize, l: &Line| !l.ends.contains(&i) && across[i].0 < l.span.1 && l.span.0 < across[i].1;
    for (n, &j) in order.iter().enumerate() {
        let item = &items[j];
        let (a, lo, hi) = (axis(item.at), axis(item.lo), axis(item.hi));
        // A row's place; else as near in line with the pin of a placed item it is wired straight to as the grid allows;
        // else its nearest grid point.
        let follow = lines.iter().find_map(|l| {
            let other = match l.ends {
                [e, o] if e == j => o,
                [o, e] if e == j => o,
                _ => return None,
            };
            placed[other].then(|| a + shift[other])
        });
        let mut k = match row_k[j] {
            Some(k) => k,
            None => index(follow.unwrap_or(a))?,
        };
        let mut d = at(k)? - a;
        // The least displacement that keeps every gap before it.
        let mut need = order[..n]
            .iter()
            .filter(|&&i| across[i].0 <= across[j].1 && across[j].0 <= across[i].1)
            .filter_map(|&i| {
                let gap = lo - axis(items[i].hi);
                (gap >= 0).then(|| axis(items[i].hi) + shift[i] + keep(gap) - FIT_SLACK - lo)
            })
            .max();
        let mut raise = |v: i128| need = Some(need.map_or(v, |n| n.max(v)));
        // Pins facing the near edge keep a grid line each, a step from the edge and from one another, and a step from
        // the item (FT-2010).
        if let Some((e0, _)) = edges
            && item.faces[ax][0] > 0
            && lo - e0 >= step
        {
            let mut g = index(e0 + step)?;
            if at(g)? < e0 + step {
                g = g.checked_add(1).ok_or(OutOfRange)?;
            }
            let past = g.checked_add(i64::from(item.faces[ax][0])).ok_or(OutOfRange)?;
            raise(at(past)? - FIT_SLACK - lo);
        }
        // A wire that cannot stay in line keeps a step of room beside the line of its pin on the grid.
        for l in lines {
            match l.room {
                Some(h) if h == j => {
                    for &i in order[..n]
                        .iter()
                        .filter(|&&i| beside(i, l) && axis(items[i].hi) <= l.at)
                    {
                        raise(axis(items[i].hi) + shift[i] + step - FIT_SLACK - l.at);
                    }
                }
                Some(h) if placed[h] && beside(j, l) && lo >= l.at => {
                    raise(l.at + shift[h] + step - FIT_SLACK - lo)
                }
                _ => {}
            }
        }
        if let Some(need) = need.filter(|need| *need > d) {
            let to = i64::try_from(a + need).map_err(|_| OutOfRange)?;
            k = k.max(grid.snap(Nm(to)));
            while at(k)? - a < need {
                k = k.checked_add(1).ok_or(OutOfRange)?;
            }
            d = at(k)? - a;
        }
        // Pins facing the far edge keep a grid line each as well, taken back as far as the gaps before it allow.
        if let Some((_, e1)) = edges
            && item.faces[ax][1] > 0
            && e1 - hi >= step
        {
            let mut g = index(e1 - step)?;
            if at(g)? > e1 - step {
                g = g.checked_sub(1).ok_or(OutOfRange)?;
            }
            let short = g.checked_sub(i64::from(item.faces[ax][1])).ok_or(OutOfRange)?;
            let most = at(short)? + FIT_SLACK - hi;
            while d > most {
                let back = at(k.checked_sub(1).ok_or(OutOfRange)?)? - a;
                if need.is_some_and(|need| back < need) {
                    break;
                }
                k -= 1;
                d = back;
            }
        }
        if !(-i128::from(MAX_COORD)..=i128::from(MAX_COORD)).contains(&(a + d)) {
            return Err(OutOfRange);
        }
        shift[j] = d;
        placed[j] = true;
    }
    Ok(shift)
}

/// The grid index each member of a row goes to in [`fit_axis`] (doc 06 §8 item 3, FT-1935); `None` for an item of no
/// row. A row is three or more items of one extent `across` whose points stood at one pitch of a `step` or more along
/// the axis: its first goes to its nearest grid point, the others follow at one pitch of whole steps, the old pitch's
/// nearest that keeps the gap between every two of them by `keep`.
fn row_steps(
    grid: Grid,
    items: &[FitItem],
    axis: &impl Fn(Pt2) -> i128,
    across: &[(i128, i128)],
    step: i128,
    keep: &impl Fn(i128) -> i128,
) -> Result<Vec<Option<i64>>, OutOfRange> {
    let at = |k: i64| grid.at(k).map(|v| i128::from(v.0)).ok_or(OutOfRange);
    let mut out = vec![None; items.len()];
    let mut lines: std::collections::BTreeMap<(i128, i128), Vec<usize>> = Default::default();
    for (i, a) in across.iter().enumerate() {
        lines.entry(*a).or_default().push(i);
    }
    for members in lines.values_mut() {
        members.sort_by_key(|&i| (axis(items[i].at), i));
        let pitch_at = |s: usize| axis(items[members[s + 1]].at) - axis(items[members[s]].at);
        let mut s = 0;
        while s + 2 < members.len() {
            let pitch = pitch_at(s);
            let mut e = s + 1;
            while e + 1 < members.len() && (pitch_at(e) - pitch).abs() <= FIT_SLACK {
                e += 1;
            }
            if e - s < 2 || pitch < step {
                s += 1;
                continue;
            }
            let run = &members[s..=e];
            let width = run
                .iter()
                .map(|&i| axis(items[i].hi) - axis(items[i].lo))
                .max()
                .unwrap_or(0);
            let first = i64::try_from(axis(items[run[0]].at)).map_err(|_| OutOfRange)?;
            let k0 = grid.snap(Nm(first));
            let mut n = i64::try_from(((2 * pitch + step) / (2 * step)).max(1)).map_err(|_| OutOfRange)?;
            // The pitch keeps the gap between every two members of the run, as the fit keeps every gap before an item.
            let kept = |n: i64| -> Result<bool, OutOfRange> {
                for m in 1..i64::try_from(run.len()).map_err(|_| OutOfRange)? {
                    let to = n
                        .checked_mul(m)
                        .and_then(|d| k0.checked_add(d))
                        .ok_or(OutOfRange)?;
                    if at(to)? - at(k0)? - width < keep(i128::from(m) * pitch - width) - FIT_SLACK {
                        return Ok(false);
                    }
                }
                Ok(true)
            };
            while !kept(n)? {
                n = n.checked_add(1).ok_or(OutOfRange)?;
            }
            for (m, &i) in (0i64..).zip(run) {
                out[i] = Some(
                    n.checked_mul(m)
                        .and_then(|d| k0.checked_add(d))
                        .ok_or(OutOfRange)?,
                );
            }
            s = e + 1;
        }
    }
    Ok(out)
}

/// The point by which `rep` keeps to `grid`: its position when that is a grid point, else its first connection
/// point ([`rep_cps`]' order) when that is one (a symbol fitted to a grid its own pin spacing does not fit, whose
/// position lies between grid points); `None` for neither, and for a position 1 nm beside a grid point, which is off
/// the grid although its pins' points are the grid's numbers. A move by grid steps steps this point, so a fitted symbol
/// moves, turns and pastes as any other and keeps its first pin on the grid.
pub(crate) fn grid_anchor(tx: &TxBuilder<'_>, rep: &Representation, grid: Grid) -> Option<Pt2> {
    if point_on_grid(grid, rep.pos) {
        return Some(rep.pos);
    }
    if grid.settle_point(rep.pos) != rep.pos {
        return None;
    }
    rep_cps(tx, rep)
        .first()
        .map(|c| c.pos)
        .filter(|p| point_on_grid(grid, *p))
}

/// [`require_on_grid`] for a representation: its [`grid_anchor`] on the grid of `page`, else the refusal of its
/// position off the grid.
pub(crate) fn require_rep_on_grid(
    tx: &TxBuilder<'_>,
    page: Id<Page>,
    rep: &Representation,
    field: &str,
) -> Result<(), CmdError> {
    let grid = tx.require::<Page>(page)?.grid;
    if grid_anchor(tx, rep, grid).is_some() {
        return Ok(());
    }
    require_on_grid(tx, page, rep.pos, field)
}

fn cps_of(tx: &TxBuilder<'_>, rep: &Representation) -> Vec<Result<RepCp, OutOfRange>> {
    let Some(symbol): Option<Arc<LibCopy<SymbolDef>>> = tx.get::<LibCopy<SymbolDef>>(rep.symbol) else {
        return Vec::new();
    };
    // A point within 1 nm of a grid position is the grid's own number for it (`Grid::settle_point`, doc 06a §2.1).
    let grid = tx.get::<Page>(rep.page).map_or(Grid::MM4, |p| p.grid);
    let placed = |local: Pt2| {
        if point_in_range(local) {
            add(rep.pos, rep.orient.apply(local)).map(|p| {
                Some(grid.settle_point(p))
                    .filter(|s| point_in_range(*s))
                    .unwrap_or(p)
            })
        } else {
            Err(OutOfRange)
        }
    };
    let vertical = |dir: &Dir4| {
        ew_derive::oriented_dir(dir, rep.orient)
            .as_ref()
            .and_then(ew_derive::dir_vertical)
    };
    let out = |dir: &Dir4| {
        let (x, y) = match ew_derive::oriented_dir(dir, rep.orient)? {
            Dir4::Up => (0, -1),
            Dir4::Right => (1, 0),
            Dir4::Down => (0, 1),
            Dir4::Left => (-1, 0),
            Dir4::Unknown(_) => return None,
        };
        Some(Pt2::new(Nm(x), Nm(y)))
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
                    vertical: vertical(&cp.dir),
                    out: out(&cp.dir),
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
            let (function, idx) = rep.fn_cp_of(cp)?;
            Some(placed(cp.pos).map(|pos| RepCp {
                symbol_cp: cp.idx,
                function,
                idx,
                pos,
                vertical: vertical(&cp.dir),
                out: out(&cp.dir),
            }))
        })
        .collect()
}

/// The position of function CP `(function, idx)` as drawn by its representation on `page` (the smallest
/// representation ID if several draw it); `None` when no representation on that page draws it or the
/// symbol geometry is unknown.
pub fn cp_position(tx: &TxBuilder<'_>, function: Id<Function>, idx: u16, page: Id<Page>) -> Option<Pt2> {
    drawn_cp(tx, function, idx, page).map(|c| c.pos)
}

/// The drawn CP of [`cp_position`]: function CP `(function, idx)` as its representation on `page` draws it.
pub fn drawn_cp(tx: &TxBuilder<'_>, function: Id<Function>, idx: u16, page: Id<Page>) -> Option<RepCp> {
    tx.reps_of_function(function).into_iter().find_map(|rid| {
        let rep = tx.get::<Representation>(rid)?;
        if rep.page != page {
            return None;
        }
        rep_cps(tx, &rep)
            .into_iter()
            .find(|c| c.function == function && c.idx == idx)
    })
}

/// The point of end `e` on `page`: a CP's as [`cp_position`] places it, a junction's own; `None` for an end not drawn
/// there.
pub fn end_point(tx: &TxBuilder<'_>, e: &Endpoint, page: Id<Page>) -> Option<Pt2> {
    match e {
        Endpoint::Cp(function, idx) => cp_position(tx, *function, *idx, page),
        Endpoint::Junction(j) => tx.get::<Junction>(*j).filter(|j| j.page == page).map(|j| j.at),
        Endpoint::Unknown(_) => None,
    }
}

/// The straight route of an autoconnect hint's wire from `a` to `b` on `page` (a proposal is a
/// straight, clear run): both ends' points when they lie on one line; empty, so unrouted, otherwise.
pub fn hint_route(tx: &TxBuilder<'_>, a: &Endpoint, b: &Endpoint, page: Id<Page>) -> Vec<Pt2> {
    let (Some(p), Some(q)) = (end_point(tx, a, page), end_point(tx, b, page)) else {
        return Vec::new();
    };
    if p == q {
        return Vec::new();
    }
    if p.x == q.x || p.y == q.y {
        return vec![p, q];
    }
    Vec::new()
}

/// Whether `p` lies on the closed segment `a`–`b` (exact: collinear by an `i128` cross product, inside the
/// bounding box; [`ew_derive::on_segment`], the test of the page's drawn paths). A point outside ±[`MAX_COORD`]
/// lies on nothing; within the range the differences are below 2⁵⁴ and their products below 2¹⁰⁸, so the `i128`
/// arithmetic cannot overflow.
pub fn on_segment(p: Pt2, a: Pt2, b: Pt2) -> bool {
    point_in_range(p) && point_in_range(a) && point_in_range(b) && ew_derive::on_segment(p, a, b)
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
/// vertex moves by `delta`; the far end never moves. Every segment that was horizontal or vertical stays
/// so:
///
/// - when the end's segment keeps its axis after the move, nothing else changes;
/// - else, when the neighbour is an interior vertex whose other segment is perpendicular to the end's segment (or
///   has no length), the neighbour follows on the moved axis (its other segment stays perpendicular);
/// - else (the neighbour is the far end, as on a straight two-vertex route, or its other segment is collinear) a
///   dog-leg is inserted: two vertices at the grid coordinate nearest the middle between the moved end and the
///   neighbour, so the end's segment leaves along its old axis and meets the neighbour along it.
///
/// A segment that was neither horizontal nor vertical keeps only its moved end; one without length (both ends of the
/// route at one point) leaves along an L when the end moves on both axes. Consecutive equal vertices are merged, but a
/// route keeps two vertices, its two ends, also where they meet. On [`OutOfRange`] the route is unchanged.
pub fn follow_end(route: &mut Vec<Pt2>, grid: Grid, at_start: bool, delta: Pt2) -> Result<(), OutOfRange> {
    if at_start {
        follow_ends(route, grid, Some(delta), None)
    } else {
        follow_ends(route, grid, None, Some(delta))
    }
}

/// Moves both route ends: end `a` by `da` and end `b` by `db` (each by [`follow_end`]'s rule, end `a` first); equal
/// displacements translate the whole route. The far end of each step stays where the other displacement put it,
/// so every end that met its CP before meets the CP's new position after. On [`OutOfRange`] the route is
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
            // A route whose two ends met (stored as one vertex) gets its second end back before one end
            // moves, else the far end would move with it.
            if next.len() == 1 {
                next.push(next[0]);
            }
            if let Some(a) = da {
                follow_start(&mut next, grid, a)?;
            }
            if let Some(b) = db {
                next.reverse();
                follow_start(&mut next, grid, b)?;
                next.reverse();
            }
            next.dedup();
            // The ends stay two vertices where they meet.
            if next.len() == 1 {
                next.push(next[0]);
            }
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
    if old == nb {
        // An end segment without length: the end leaves the neighbour along an L, never diagonally.
        if new.x != nb.x && new.y != nb.y {
            route.insert(1, Pt2::new(new.x, nb.y));
        }
        return Ok(());
    }
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

/// Whether a coordinate is a position of the half grid of `grid` (half its step): where a wire's corner may stand
/// between close pins (ADR-0033 amendment 3). A grid of 1 nm has no half grid.
pub fn on_half_grid(grid: Grid, x: Nm) -> bool {
    on_grid(grid, x) || Grid::new(grid.num(), grid.den().saturating_mul(2)).is_ok_and(|h| on_grid(h, x))
}

/// Whether both coordinates of a point are grid positions.
pub fn point_on_grid(grid: Grid, p: Pt2) -> bool {
    on_grid(grid, p.x) && on_grid(grid, p.y)
}

/// The refusal of a point off the grid of `page`, in words (a refusal reaches the window's banner): what
/// the point is, its position in mm and its page, no nm and no JSON Pointer ("the wire's corner at x 77, y 161 mm
/// on page 101 is not on the drawing grid"). A fragment: each caller ends it with the step the user can take.
pub(crate) fn off_grid(tx: &TxBuilder<'_>, what: impl Into<TextArg>, page: Id<Page>, p: Pt2) -> Text {
    text!(
        "refusal-geometry-off-grid",
        what.into(),
        crate::words::point(tx, page, p)
    )
}

/// What the JSON Pointer of a position checked by [`require_on_grid`] names, in words: a split point
/// (`…/on_connection/at`), an outline vertex (`/outline/<i>`), a box CP (`…/cps/<i>/pos`), else a position.
fn what_of(field: &str) -> Text {
    if field.ends_with("/at") {
        text!("refusal-geometry-branch-point")
    } else if field.starts_with("/outline/") {
        text!("refusal-geometry-outline-corner")
    } else if field.contains("cps/") {
        text!("refusal-geometry-box-cp")
    } else {
        text!("refusal-geometry-position")
    }
}

/// D3 (doc 06 §5.7: CPs never leave the connection grid, in any tool and any origin): a position at which
/// a command places or to which it moves a representation must lie on the connection grid of its page
/// (`Page.grid`, ADR-0027) and within ±[`MAX_COORD`]. Front-ends snap before they submit. `field` is the
/// JSON Pointer of the parameter (`/pos`), from which both refusals name the point in words: the range
/// refusal by its position in mm ("the position at x 9007199254.7, y 0 mm lies outside the drawable range; move it
/// nearer the page origin"), the grid refusal by its position and page ("the position at x 8.4, y 15.9 mm on page
/// 102 is not on the drawing grid; move it onto the grid").
pub fn require_on_grid(tx: &TxBuilder<'_>, page: Id<Page>, pos: Pt2, field: &str) -> Result<(), CmdError> {
    let grid = tx.require::<Page>(page)?.grid;
    if !point_in_range(pos) {
        return tx.reject(text!(
            "refusal-geometry-out-of-range",
            what_of(field),
            crate::words::position(pos)
        ));
    }
    if point_on_grid(grid, pos) {
        return Ok(());
    }
    tx.reject(text!(
        "refusal-geometry-onto-grid",
        off_grid(tx, what_of(field), page, pos)
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
    // Not accepted: it is out of range or off the grid, which `require_on_grid` refuses.
    require_on_grid(tx, page, at, field)
}

/// D3 for a route given to `connection.draw` or `connection.route` on `page` (an empty route has no
/// geometry and passes):
///
/// - an end at a CP that a representation on `page` draws must be exactly that CP's position (route
///   vertex 0 at endpoint `a`, the last vertex at endpoint `b`), and an end at a junction on `page` exactly the
///   junction's point (a route holds both ends);
/// - every coordinate of every vertex lies on the page's connection grid or its half grid (a corner between close
///   pins, ADR-0033 amendment 3), or equals that coordinate of a route end that meets its CP: a CP position can sit
///   up to 1 nm off a rational grid (the rounding pitfall of `Grid::cell`), and the segment leaving it stays
///   orthogonal;
/// - every vertex lies within ±[`MAX_COORD`];
/// - the route is tidy ([`route_defect`]): no vertex repeats the one before it, and no segment
///   runs over another one (a stretch of positive length shared on one line, a doubling back included).
///   A collinear vertex where the route runs straight on is no defect.
///
/// Every refusal names the vertex in words, without a JSON Pointer or nanometres, and ends with what
/// the user can do: "the wire's corner at x 15.9, y 7.9 mm on page 102 is not on the drawing grid" (the first vertex
/// is the wire's start, the last its end). `_field`, the JSON Pointer of the route parameter (`/route`), is named by
/// none of them.
pub fn require_route(
    tx: &TxBuilder<'_>,
    page: Id<Page>,
    a: &Endpoint,
    b: &Endpoint,
    route: &[Pt2],
    _field: &str,
) -> Result<(), CmdError> {
    let (Some(first), Some(last)) = (route.first().copied(), route.last().copied()) else {
        return Ok(());
    };
    let grid = tx.require::<Page>(page)?.grid;
    let what = |i: usize| {
        if i == 0 {
            text!("refusal-geometry-wire-start")
        } else if i + 1 == route.len() {
            text!("refusal-geometry-wire-end")
        } else {
            text!("refusal-geometry-wire-corner")
        }
    };
    for (i, v) in route.iter().enumerate() {
        if !point_in_range(*v) {
            return tx.reject(text!(
                "refusal-geometry-route-out-of-range",
                what(i),
                crate::words::point(tx, page, *v)
            ));
        }
    }
    let mut exact: Vec<Pt2> = Vec::new();
    for (e, v, i) in [(a, first, 0), (b, last, route.len() - 1)] {
        // A junction end's point is the route's end vertex too.
        if let Endpoint::Junction(j) = e {
            let Some(at) = tx.get::<Junction>(*j).filter(|j| j.page == page).map(|j| j.at) else {
                continue;
            };
            if v != at {
                let (here, there) = (crate::words::position(v), crate::words::position(at));
                let junction = text!("refusal-geometry-junction");
                let miss = if here == there {
                    text!("refusal-geometry-misses", junction)
                } else {
                    text!("refusal-geometry-does-not-meet", junction, there)
                };
                return tx.reject(text!(
                    "refusal-geometry-end-off-junction",
                    what(i),
                    crate::words::point(tx, page, v),
                    miss
                ));
            }
            exact.push(at);
            continue;
        }
        let Endpoint::Cp(function, idx) = e else {
            continue;
        };
        let Some(cp) = cp_position(tx, *function, *idx, page) else {
            continue;
        };
        if v != cp {
            let is_ip = tx
                .get::<Function>(*function)
                .is_some_and(|f| matches!(f.kind, ew_schema::FunctionKind::Ip(_)));
            let pin = if is_ip {
                text!(
                    "refusal-geometry-ip-pin",
                    crate::words::interruption_point(tx, *function)
                )
            } else {
                text!(
                    "refusal-geometry-pin",
                    crate::normalise::channel_instantiation::cp_words(tx, *function, *idx)
                )
            };
            let (at, cp_at) = (crate::words::position(v), crate::words::position(cp));
            // two positions that print alike in mm are told apart by the gap, not by the same words twice
            let miss = if at == cp_at {
                text!("refusal-geometry-misses", pin)
            } else {
                text!("refusal-geometry-does-not-meet", pin, cp_at)
            };
            return tx.reject(text!(
                "refusal-geometry-end-off-cp",
                what(i),
                crate::words::point(tx, page, v),
                miss
            ));
        }
        exact.push(cp);
    }
    for (i, v) in route.iter().enumerate() {
        let ok_x = on_half_grid(grid, v.x) || exact.iter().any(|c| c.x == v.x);
        let ok_y = on_half_grid(grid, v.y) || exact.iter().any(|c| c.y == v.y);
        if !(ok_x && ok_y) {
            return tx.reject(text!(
                "refusal-geometry-corner-onto-grid",
                off_grid(tx, what(i), page, *v)
            ));
        }
    }
    match route_defect(route) {
        None => Ok(()),
        Some(RouteDefect::Duplicate(i)) => tx.reject(text!(
            "refusal-geometry-duplicate-corner",
            what(i),
            crate::words::point(tx, page, route[i])
        )),
        Some(RouteDefect::Overlap(i, j)) => tx.reject(text!(
            "refusal-geometry-overlap",
            crate::words::page(tx, page),
            crate::words::position(route[j]),
            crate::words::position(route[j + 1]),
            crate::words::position(route[i]),
            crate::words::position(route[i + 1])
        )),
    }
}

/// What makes a route untidy ([`route_defect`]).
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
            let g = arith::gcd(dx.unsigned_abs(), dy.unsigned_abs()).max(1) as i128;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// FT-1935: twelve narrow symbols in a row two QET steps apart (5.29 mm, less than a 4 mm step between their
    /// bodies) went each to its own nearest 4 mm point and were pushed on by turns; as a row they keep one pitch.
    #[test]
    fn a_row_that_had_one_pitch_keeps_one() {
        let (x0, pitch, width) = (174_625_000, 5_291_667, 2_645_833);
        let items: Vec<FitItem> = (0..12)
            .map(|k| {
                let at = Pt2::new(Nm(x0 + k * pitch), Nm(80_000_000));
                FitItem {
                    at,
                    lo: at,
                    hi: Pt2::new(Nm(at.x.0 + width), Nm(90_000_000)),
                    faces: [[0; 2]; 2],
                }
            })
            .collect();
        let d = fit(Grid::MM4, &items, &FitRoom::default()).expect("in range");
        let xs: Vec<i64> = items.iter().zip(&d).map(|(i, d)| i.at.x.0 + d.x.0).collect();
        let pitch = xs[1] - xs[0];
        assert!(xs.windows(2).all(|w| w[1] - w[0] == pitch), "{xs:?}");
    }

    /// An item of the fit: its point `at` and its box from `lo` to `hi`, in nm; `faces` as [`FitItem::faces`].
    fn item(at: (i64, i64), lo: (i64, i64), hi: (i64, i64), faces: [[u32; 2]; 2]) -> FitItem {
        let p = |(x, y): (i64, i64)| Pt2::new(Nm(x), Nm(y));
        FitItem {
            at: p(at),
            lo: p(lo),
            hi: p(hi),
            faces,
        }
    }

    /// The 4 mm grid's step, and the 2 nm the fit's rounding may take from a gap.
    const STEP: i64 = 4_000_000;
    const SLACK: i64 = 2;

    /// FT-2010: a card whose two supply pins face the drawing area's left edge 1.98 steps from it, their wires turning
    /// along it, keeps two grid lines between, a step from the edge, from each other and from the card, a way out for
    /// each wire; a box whose pin faces the right edge as far keeps one. The card went to its nearest grid point 2.3
    /// steps from the edge, the box towards the edge.
    #[test]
    fn pins_facing_the_edge_keep_a_grid_line_each() {
        let (e0, e1) = (5_291_667, 275_166_667);
        let area = [Pt2::new(Nm(e0), Nm(e0)), Pt2::new(Nm(e1), Nm(174_625_000))];
        let card = item(
            (26_458_333, 37_041_667),
            (13_229_167, 21_166_667),
            (187_854_167, 37_041_667),
            [[2, 0], [0, 1]],
        );
        let boxed = item(
            (e1 - 10_937_500, 100_000_000),
            (e1 - 20_937_500, 95_000_000),
            (e1 - 7_937_500, 110_000_000),
            [[0, 1], [0, 0]],
        );
        let room = FitRoom {
            area: Some(area),
            wires: Vec::new(),
        };
        let d = fit(Grid::MM4, &[card, boxed], &room).expect("in range");
        let line = |v: i64| Grid::MM4.snap_point(Nm(v)).expect("in range").0;
        // The first grid line a step inside the left edge, and the last a step inside the right.
        let (first, last) = (line(e0 + STEP + STEP / 2), line(e1 - STEP - STEP / 2));
        assert!(first >= e0 + STEP && last <= e1 - STEP);
        let lo = card.lo.x.0 + d[0].x.0;
        assert!(first + 2 * STEP <= lo + SLACK, "the card's left edge at {lo} nm");
        let hi = boxed.hi.x.0 + d[1].x.0;
        assert!(hi + STEP <= last + SLACK, "the box's right edge at {hi} nm");
    }

    /// FT-2010: two terminals wired straight down from a contactor's first two pins (5.29 mm apart, so on the 4 mm
    /// grid only its first stands on a grid line); the first terminal stays in line with the first pin, and the second,
    /// which cannot, keeps a step of room beside its line from the first, whose corner stood a quarter step beside it.
    /// And a symbol pushed on by its neighbour takes the one wired straight under it along.
    #[test]
    fn two_pins_wired_straight_stay_in_line_or_keep_a_step_of_room() {
        let (pitch, top, low) = (5_291_667, 121_708_333, 132_291_667);
        let contactor = item(
            (47_625_000, top),
            (44_979_167, 108_479_167),
            (60_854_167, top),
            [[0; 2]; 2],
        );
        let x1 = item(
            (47_625_000, low),
            (44_648_833, low),
            (50_604_167, 140_562_500),
            [[0; 2]; 2],
        );
        let x2 = item(
            (47_625_000 + pitch, 142_875_000),
            (50_931_250, 142_875_000),
            (54_900_000, 150_145_833),
            [[0; 2]; 2],
        );
        let pin = |x: i64, y: i64| Pt2::new(Nm(x), Nm(y));
        let room = FitRoom {
            area: None,
            wires: vec![
                [(0, pin(47_625_000, top)), (1, pin(47_625_000, low))],
                [
                    (0, pin(47_625_000 + pitch, top)),
                    (2, pin(47_625_000 + pitch, 142_875_000)),
                ],
            ],
        };
        let d = fit(Grid::MM4, &[contactor, x1, x2], &room).expect("in range");
        assert_eq!(contactor.at.x.0 + d[0].x.0, x1.at.x.0 + d[1].x.0, "in line");
        let (corner, line) = (x1.hi.x.0 + d[1].x.0, x2.at.x.0 + d[2].x.0);
        assert!(
            corner + STEP <= line + SLACK,
            "the corner at {corner} nm, the line at {line} nm"
        );

        // A neighbour that goes 1.9 mm right pushes the symbol beside it on; the symbol under it follows.
        let left = item(
            (2_100_000, 0),
            (2_100_000, 0),
            (10_000_000, 10_000_000),
            [[0; 2]; 2],
        );
        let pushed = item(
            (15_000_000, 0),
            (15_000_000, 0),
            (20_000_000, 10_000_000),
            [[0; 2]; 2],
        );
        let under = item(
            (15_000_000, 30_000_000),
            (15_000_000, 30_000_000),
            (22_000_000, 40_000_000),
            [[0; 2]; 2],
        );
        let room = FitRoom {
            area: None,
            wires: vec![[(1, pin(15_000_000, 10_000_000)), (2, pin(15_000_000, 30_000_000))]],
        };
        let d = fit(Grid::MM4, &[left, pushed, under], &room).expect("in range");
        // Its nearest grid point is 1 mm on.
        assert!(d[1].x > Nm(1_000_000), "pushed on: {:?}", d[1]);
        assert_eq!(d[1].x, d[2].x, "in line");
    }
}
