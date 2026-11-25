// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one text placer (docs/design/06a-drafting.md §5): [`settle_labels`] gives every label of a page its place
//! from the places of its kind, against the parts' bodies, the wires, the fixed texts and the labels placed before
//! it. The scene calls it when it draws a page, and [`crate::measure`] calls it, so the canvas, the PDF and the checks
//! see one page. The places of each kind, the rules that change them and the choice are on the functions that make
//! them, each citing its section of §5.

use std::collections::{BTreeMap, BTreeSet};

use crate::consts::{LABEL_CLASH, LABEL_CLEAR, LABEL_RANK, TEXT_CROSSED, TEXT_STEP, TICKS_PER_STEP};
use crate::geom::{self, Buckets, Seg};
use crate::model::{
    DraftPage, LabelId, LabelKind, Owner, Part, PartId, PinId, Places, Pt, Rect, Side, Tick, WireId,
};

mod places;
mod units;

use places::{bx, candidates};
use units::{stacked_marks, stagger, units};

/// Where the text placer set a label.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settled {
    /// The label.
    pub id: LabelId,
    /// Its box on the page.
    pub rect: Rect,
    /// Its place down its list: 0 at its first place.
    pub rank: u32,
    /// It reads turned a quarter, from the bottom (a wire number along a vertical stretch): its box is its size
    /// turned.
    pub turned: bool,
}

/// One label, or the block of labels of one side of a part, as the placer moves it.
#[derive(Clone, Debug)]
struct Unit {
    /// The labels (by index into the page's labels) and the offset of each one's box from the unit's box.
    members: Vec<(usize, Pt)>,
    size: (Tick, Tick),
    kind: LabelKind,
    owner: Owner,
    /// The adapter's places on the page.
    given: Vec<Rect>,
    along: bool,
    /// The two-rows rule gives it "one line lower" as its second place.
    stagger: bool,
}

/// The things a label keeps clear of, filed in buckets.
struct Ground<'a> {
    page: &'a DraftPage,
    parts: BTreeMap<PartId, &'a Part>,
    /// Each pin on the page: its point, its side and its part.
    pins: BTreeMap<PinId, (Pt, Option<Side>, PartId)>,
    /// The pins a wire ends at.
    wired: BTreeSet<PinId>,
    bodies: Vec<(PartId, Rect)>,
    body_grid: Buckets,
    segs: Vec<(WireId, Seg)>,
    seg_grid: Buckets,
    fixed: Vec<Rect>,
    fixed_grid: Buckets,
}

impl<'a> Ground<'a> {
    fn of(page: &'a DraftPage) -> Ground<'a> {
        let mut parts = BTreeMap::new();
        let pins = page.pins();
        let mut bodies = Vec::new();
        let mut body_grid = Buckets::default();
        let mut sorted: Vec<&Part> = page.parts.iter().collect();
        sorted.sort_by_key(|p| p.id);
        for p in sorted {
            parts.insert(p.id, p);
            if !p.frame_only {
                body_grid.insert(bodies.len(), p.bounds());
                bodies.push((p.id, p.bounds()));
            }
        }
        let wired = page.wired();
        let mut segs = Vec::new();
        let mut seg_grid = Buckets::default();
        let mut wires: Vec<_> = page.wires.iter().collect();
        wires.sort_by_key(|w| w.id);
        for w in wires {
            for s in geom::segments(&geom::clean(&w.route)) {
                seg_grid.insert(segs.len(), geom::seg_box(s));
                segs.push((w.id, s));
            }
        }
        let mut fixed: Vec<Rect> = page.fixed_texts.iter().map(|t| t.rect).collect();
        fixed.sort();
        let mut fixed_grid = Buckets::default();
        for (i, r) in fixed.iter().enumerate() {
            fixed_grid.insert(i, *r);
        }
        Ground {
            page,
            parts,
            pins,
            wired,
            bodies,
            body_grid,
            segs,
            seg_grid,
            fixed,
            fixed_grid,
        }
    }

    /// The part whose body a label of `owner` may stand on: its own. A part without pins (a mark drawn as a symbol, a
    /// module's glyph) holds its texts inside its body or keeps them beside it: one across its outline is in its way
    /// ([`Ground::across_own`]).
    fn own_part(&self, owner: Owner) -> Option<PartId> {
        match owner {
            Owner::Part(p) => Some(p),
            _ => None,
        }
    }

    /// Whether `r` lies across the outline of its owner's body, the owner a part without pins.
    fn across_own(&self, owner: Owner, r: Rect) -> bool {
        let Owner::Part(p) = owner else { return false };
        self.parts
            .get(&p)
            .is_some_and(|x| x.pins.is_empty() && x.bounds().overlaps(r) && !x.bounds().holds(r))
    }

    /// Whether `r` lies in the drawing area and off the frame's bands.
    fn inside(&self, r: Rect) -> bool {
        self.page.area.holds(r) && !self.page.keep_out.iter().any(|k| k.overlaps(r))
    }

    /// Whether a label of `owner` at `r` is clear of every body but its own part's (with `bodies`), every wire and every
    /// fixed text; at its author's place (`at_first`) a fixed text only must not lie on it.
    fn clear(&self, owner: Owner, r: Rect, (bodies, at_first): (bool, bool)) -> bool {
        let g = r.grown(LABEL_CLEAR);
        let own = self.own_part(owner);
        self.inside(r)
            && !(bodies
                && self
                    .body_grid
                    .near(g)
                    .into_iter()
                    .any(|i| Some(self.bodies[i].0) != own && self.bodies[i].1.overlaps(g)))
            && !self
                .seg_grid
                .near(g)
                .into_iter()
                .any(|i| geom::crosses(self.segs[i].1, g))
            && !self.fixed_grid.near(g).into_iter().any(|i| {
                if at_first {
                    geom::texts_meet(self.fixed[i], r)
                } else {
                    self.fixed[i].overlaps(g)
                }
            })
            && !self.across_own(owner, r)
            && self.reads_as_own(owner, r)
    }

    /// Whether a label of a part at `r` stands nearer its part than any other part of its kind (a part with a body of
    /// the same size: the terminals of a strip, the contacts of a row), as §1.3 asks of every label.
    fn reads_as_own(&self, owner: Owner, r: Rect) -> bool {
        let Owner::Part(p) = owner else { return true };
        let Some(own) = self.parts.get(&p).map(|x| x.bounds()) else {
            return true;
        };
        let d = geom::gap(r, own);
        !self.body_grid.near(r.grown(d)).into_iter().any(|i| {
            self.bodies[i].0 != p
                && self.bodies[i].1.size() == own.size()
                && geom::gap(r, self.bodies[i].1) < d
        })
    }

    /// What a label of `owner` at `r` costs against the ground: [`LABEL_CLASH`] for each body but its own and each
    /// fixed text it lies on and for leaving the drawing area; [`TEXT_CROSSED`] and [`TEXT_STEP`] per tick inside for
    /// each wire across it. In sixteenths of a step.
    fn cost(&self, owner: Owner, r: Rect) -> i64 {
        let own = self.own_part(owner);
        let step = i64::from(TICKS_PER_STEP);
        let mut clashes = i64::from(!self.inside(r)) + i64::from(self.across_own(owner, r));
        clashes += self
            .body_grid
            .near(r)
            .into_iter()
            .filter(|&i| Some(self.bodies[i].0) != own && self.bodies[i].1.overlaps(r))
            .count() as i64;
        clashes += self
            .fixed_grid
            .near(r)
            .into_iter()
            .filter(|&i| self.fixed[i].overlaps(r))
            .count() as i64;
        // A wire nearer than LABEL_CLEAR runs across the text as a reader sees it (§1.3): one along its edge too.
        let g = r.grown(LABEL_CLEAR);
        let mut across: BTreeMap<WireId, i64> = BTreeMap::new();
        for i in self.seg_grid.near(g) {
            if let Some(s) = geom::stretch_inside(self.segs[i].1, g) {
                *across.entry(self.segs[i].0).or_default() += geom::length(s);
            }
        }
        clashes * LABEL_CLASH * step
            + across
                .values()
                .map(|t| TEXT_CROSSED * step + TEXT_STEP * t)
                .sum::<i64>()
    }
}

/// The geometry of a part that its labels' places follow from.
struct Shape {
    body: Rect,
    /// Its pins on the page with their sides.
    pins: Vec<(Pt, Option<Side>)>,
    /// Its pins leave up or down, or none has a side.
    upright: bool,
    /// A wire leaves it upward.
    wire_up: bool,
    /// Its pins a wire leaves, with their sides: their ways out.
    ways: Vec<(Pt, Side)>,
    /// The right edge of the body and of its pins' numbers at their first places, which a text at a right corner
    /// stands beyond.
    right: Tick,
}

impl Shape {
    fn of(ground: &Ground<'_>, part: &Part) -> Shape {
        let pins: Vec<(Pt, Option<Side>)> = part.pins.iter().map(|p| (part.at.plus(p.at), p.side)).collect();
        let upright =
            pins.iter().any(|(_, s)| s.is_some_and(Side::vertical)) || pins.iter().all(|(_, s)| s.is_none());
        let wire_up = part
            .pins
            .iter()
            .any(|p| p.side == Some(Side::Up) && ground.wired.contains(&p.id));
        let ways = part
            .pins
            .iter()
            .filter(|p| ground.wired.contains(&p.id))
            .filter_map(|p| p.side.map(|s| (part.at.plus(p.at), s)))
            .collect();
        let body = part.bounds();
        let right = ground
            .page
            .labels
            .iter()
            .filter(|l| l.owner == Owner::Part(part.id) && matches!(l.kind, LabelKind::PinNumber(_)))
            .filter_map(|l| match &l.places {
                Places::At(at) => at.first().map(|a| Rect::sized(part.at.plus(*a), l.size).x1),
                Places::AlongWire => None,
            })
            .fold(body.x1, Tick::max);
        Shape {
            body,
            pins,
            upright,
            wire_up,
            ways,
            right,
        }
    }
}

/// The labels of `page` where the text placer sets them, by label ID; a label without a place (an owner not on the
/// page, a wire without a route and no place given) is left out. The choice (§5.3):
///
/// Labels (blocks) are placed in the order of their kind's priority, then of their owner's place (top, then left),
/// then of their ID. Each takes its first place that lies in the drawing area and off the frame's bands and is clear
/// by [`LABEL_CLEAR`] of every part's body but its own (an outline drawn round other parts is no body), every wire
/// segment, every fixed text and every label placed before it. At its author's place it only must not lie on a fixed
/// text or a text of its own part, and a pin's number there may stand on another part's body; two texts of one part
/// at their author's places, one over the other or one a pin's number, do not count against each other. A label
/// stands nowhere it reads as another part's (nearer another part of the same body size than its own); a label of a
/// part without pins stays inside its outline or clear of it; a place that is the first place of a label not yet
/// placed is taken only when no other is clear. A text whose author's place lies on a text of its own part placed
/// before it has the place under that text second.
///
/// A label without a clear place takes its place of least cost ([`LABEL_CLASH`] for each text and body it lies on
/// and for leaving the drawing area, [`TEXT_CROSSED`] and [`TEXT_STEP`] per tick for each wire across it or nearer
/// than [`LABEL_CLEAR`],
/// [`LABEL_RANK`] for each place down its list; ties to the earlier place) of those inside the drawing area: a text
/// never stands outside it. Where every place leaves the area, its places moved in (onto its own part's body too) are
/// its places. There a symbol's text does not count the first place of another text of its symbol against itself. One repair round follows: for each such
/// label, each label it lies on is tried at each of its other clear places in turn, and that label then at its first
/// clear place; the first change that lowers the labels' cost on the page is kept, and while the label still lies on
/// another, the next one it lay on is tried. Every list is read in a fixed order, so the
/// result does not depend on the order of the page's lists.
pub fn settle_labels(page: &DraftPage) -> Vec<Settled> {
    let ground = Ground::of(page);
    let mut units = units(&ground);
    stacked_marks(&ground, &mut units);
    stagger(&mut units);
    // The order of §5.3.
    let mut order: Vec<usize> = (0..units.len()).collect();
    let key = |u: &Unit| {
        let at = match u.owner {
            Owner::Part(p) => ground.parts.get(&p).map(|p| p.at),
            Owner::Wire(w) => page
                .wires
                .iter()
                .find(|x| x.id == w)
                .and_then(|x| x.route.iter().min_by_key(|p| (p.y, p.x)).copied()),
            Owner::Mark(m) => page.marks.iter().find(|x| x.id == m).map(|x| x.line.top_left()),
        }
        .or_else(|| u.given.first().map(|r| r.top_left()))
        .unwrap_or_default();
        (u.kind.priority(), at.y, at.x, page.labels[u.members[0].0].id)
    };
    order.sort_by_key(|&i| key(&units[i]));
    let mut placed: Vec<Option<(Rect, u32, bool)>> = vec![None; units.len()];
    let mut lists: Vec<Vec<(Rect, bool)>> = vec![Vec::new(); units.len()];
    let mut grid = Buckets::default();
    // The first places the adapters gave: a label that has to leave its own does not take another's.
    let mut firsts = Buckets::default();
    for (u, unit) in units.iter().enumerate() {
        if let Some(r) = unit.given.first() {
            firsts.insert(u, *r);
        }
    }
    let mut numbers: Vec<(Rect, bool, Seg)> = Vec::new();
    let mut clashing: Vec<usize> = Vec::new();
    let rows = like_rows(&ground, &units);
    let mut row_of: BTreeMap<usize, usize> = BTreeMap::new();
    for (i, row) in rows.iter().enumerate() {
        for &u in row {
            row_of.insert(u, i);
        }
    }
    let mut tried = vec![false; rows.len()];
    for &u in &order {
        // A unit its row's decision placed.
        if placed[u].is_some() {
            continue;
        }
        if let Some(&i) = row_of.get(&u)
            && !tried[i]
        {
            tried[i] = true;
            let mut state = (&mut placed[..], &mut grid, &mut lists[..]);
            if place_row(&ground, &units, &rows[i], (&firsts, &numbers), &mut state) {
                continue;
            }
        }
        let list = list_of(&ground, &units, &placed, &numbers, u);
        if list.is_empty() {
            continue;
        }
        let clear = list.iter().enumerate().position(|(k, (r, _))| {
            let at_first = authors(&units[u], k);
            ground.clear(units[u].owner, *r, (bodies(&units[u], at_first), at_first))
                && labels_clear(&units, &placed, &grid, u, (*r, at_first))
                && (at_first || reserved(&units, &placed, &firsts, u, *r) == 0)
        });
        // Without a clear place, the place of least cost of those in the drawing area (§5.3): a text never stands
        // outside it. For a symbol's text, the first place of another text of its symbol not yet placed costs nothing
        // there (that text moves on): a symbol's texts stand by their own symbol before a neighbour's.
        let inside: Vec<usize> = (0..list.len()).filter(|&k| ground.inside(list[k].0)).collect();
        let pool = if inside.is_empty() {
            (0..list.len()).collect()
        } else {
            inside
        };
        let own: Vec<usize> = (0..units.len())
            .filter(|&v| {
                units[v].owner == units[u].owner && units[v].kind.own_text() && units[u].kind.own_text()
            })
            .collect();
        let k = clear.unwrap_or_else(|| {
            pool.into_iter()
                .min_by_key(|&k| {
                    let taken = if authors(&units[u], k) {
                        0
                    } else {
                        reserved_except(&units, &placed, &firsts, &own, list[k].0)
                    };
                    let cost = unit_cost(&ground, &units, &placed, &grid, u, list[k].0, k);
                    (cost + taken as i64 * LABEL_CLASH * i64::from(TICKS_PER_STEP), k)
                })
                .unwrap_or(0)
        });
        if clear.is_none() {
            clashing.push(u);
        }
        let (r, turned) = list[k];
        placed[u] = Some((r, k as u32, turned));
        grid.insert(u, r);
        if units[u].along
            && let Owner::Wire(w) = units[u].owner
            && let Some(s) = nearest_stretch(page, w, r)
        {
            numbers.push((r, turned, s));
        }
        lists[u] = list;
    }
    repair(&ground, &units, &lists, &clashing, &mut placed, &mut grid);
    let mut out: Vec<Settled> = Vec::new();
    for (u, unit) in units.iter().enumerate() {
        let Some((r, rank, turned)) = placed[u] else {
            continue;
        };
        for &(i, offset) in &unit.members {
            let rect = if unit.members.len() == 1 {
                r
            } else {
                Rect::sized(r.top_left().plus(offset), page.labels[i].size)
            };
            out.push(Settled {
                id: page.labels[i].id,
                rect,
                rank,
                turned,
            });
        }
    }
    out.sort_by_key(|s| s.id);
    out
}

/// The places of unit `u` in order (its [`candidates`]), with line under line: a text whose author's place lies on a
/// text of its own part placed before it (a turned symbol sets them on one row) has the place under that text second.
fn list_of(
    ground: &Ground<'_>,
    units: &[Unit],
    placed: &[Option<(Rect, u32, bool)>],
    numbers: &[(Rect, bool, Seg)],
    u: usize,
) -> Vec<(Rect, bool)> {
    let mut list = candidates(ground, &units[u], numbers);
    if let (Some(first), Owner::Part(_)) = (units[u].given.first(), units[u].owner)
        && !matches!(units[u].kind, LabelKind::PinNumber(_))
        && let Some(bottom) = (0..units.len())
            .filter(|&v| v != u && units[v].owner == units[u].owner)
            .filter_map(|v| placed[v].map(|(r, _, _)| r))
            .filter(|r| geom::texts_meet(*r, *first) && !geom::stacked(*r, *first))
            .map(|r| r.y1)
            .max()
    {
        let under = (bx(first.x0, bottom + LABEL_CLEAR, first.size()), false);
        if !list.contains(&under) {
            list.insert(1.min(list.len()), under);
        }
    }
    list
}

/// The rows of the row rule ([`place_row`]): the units of one kind and label number on parts of one shape (the
/// same body and pins) whose origins stand level, two or more, left to right. A pin's number, a text along a wire and
/// a text of the two-rows rule (wider than its row's pitch: staggered) are no row's.
fn like_rows(ground: &Ground<'_>, units: &[Unit]) -> Vec<Vec<usize>> {
    // A row's key: the kind, the label number, the block's size in labels, the line its parts stand on, their shape.
    type Key = (LabelKind, u32, usize, Tick, (Rect, Vec<(Pt, Option<Side>)>));
    let mut rows: BTreeMap<Key, Vec<(Tick, usize)>> = BTreeMap::new();
    for (u, unit) in units.iter().enumerate() {
        let Owner::Part(p) = unit.owner else { continue };
        if unit.along || unit.stagger || matches!(unit.kind, LabelKind::PinNumber(_)) {
            continue;
        }
        let Some(part) = ground.parts.get(&p) else {
            continue;
        };
        let pins = part.pins.iter().map(|x| (x.at, x.side)).collect();
        let n = ground.page.labels[unit.members[0].0].id.n;
        rows.entry((unit.kind, n, unit.members.len(), part.at.y, (part.body, pins)))
            .or_default()
            .push((part.at.x, u));
    }
    rows.into_values()
        .filter(|row| row.len() > 1)
        .map(|mut row| {
            row.sort();
            row.into_iter().map(|(_, u)| u).collect()
        })
        .collect()
}

/// What the choice writes as it places units: each unit's place (its box, its place down its list, turned), the
/// buckets of the placed boxes, and each unit's list.
type Placing<'s> = (
    &'s mut [Option<(Rect, u32, bool)>],
    &'s mut Buckets,
    &'s mut [Vec<(Rect, bool)>],
);

/// **The row rule.** Like parts that stand level (one shape: the same body and pins; their origins on one line) take
/// one place for each of their texts: when the first of them comes in the order, the place the same number down every
/// one's list at which all of them are clear is taken for all, so the tags and cross-references of a row of relays
/// stand alike. Where their lists are not alike or no place is clear for all, each is placed on its own; texts wider
/// than their row's pitch keep the two-rows rule.
///
/// The units of `row` take the first place, the same number down each one's list,
/// at which every one of them is clear, each tried against the ones before it. `false`, with nothing placed, where
/// their lists are not alike (a wire leaves one of them upward) or no place is clear for all: then each is placed on
/// its own.
fn place_row(
    ground: &Ground<'_>,
    units: &[Unit],
    row: &[usize],
    (firsts, numbers): (&Buckets, &[(Rect, bool, Seg)]),
    (placed, grid, lists): &mut Placing<'_>,
) -> bool {
    let row_lists: Vec<Vec<(Rect, bool)>> = row
        .iter()
        .map(|&u| list_of(ground, units, placed, numbers, u))
        .collect();
    let n = row_lists.first().map_or(0, Vec::len);
    if n == 0 || row_lists.iter().any(|l| l.len() != n) {
        return false;
    }
    for k in 0..n {
        let mut set: Vec<usize> = Vec::new();
        let all = row.iter().zip(&row_lists).all(|(&u, list)| {
            let (r, turned) = list[k];
            let at_first = authors(&units[u], k);
            // The row's own first places are not reserved against it: its members move together.
            let taken = reserved_except(units, placed, firsts, row, r);
            let free = ground.clear(units[u].owner, r, (bodies(&units[u], at_first), at_first))
                && labels_clear(units, placed, grid, u, (r, at_first))
                && (at_first || taken == 0);
            if free {
                placed[u] = Some((r, k as u32, turned));
                grid.insert(u, r);
                set.push(u);
            }
            free
        });
        if all {
            for (&u, list) in row.iter().zip(row_lists) {
                lists[u] = list;
            }
            return true;
        }
        for u in set {
            if let Some((r, _, _)) = placed[u].take() {
                grid.remove(u, r);
            }
        }
    }
    false
}

/// [`reserved`] for a member of `row`: the first places of the units not yet placed, the row's own left out.
fn reserved_except(
    units: &[Unit],
    placed: &[Option<(Rect, u32, bool)>],
    firsts: &Buckets,
    row: &[usize],
    r: Rect,
) -> usize {
    let g = r.grown(LABEL_CLEAR);
    firsts
        .near(g)
        .into_iter()
        .filter(|&v| {
            !row.contains(&v) && placed[v].is_none() && units[v].given.first().is_some_and(|f| f.overlaps(g))
        })
        .count()
}

/// Each label of a part at the first place of its list, by label ID: where it stands when nothing is in its way. The
/// placer reads a part's footprint from these, so a text that stepped aside on the page as drawn does not change where
/// the part is set.
pub(crate) fn first_places(page: &DraftPage) -> Vec<(LabelId, Rect)> {
    let ground = Ground::of(page);
    let mut out: Vec<(LabelId, Rect)> = Vec::new();
    let mut units = units(&ground);
    stacked_marks(&ground, &mut units);
    for unit in units {
        if !matches!(unit.owner, Owner::Part(_)) {
            continue;
        }
        let Some((r, _)) = candidates(&ground, &unit, &[]).first().copied() else {
            continue;
        };
        for &(i, offset) in &unit.members {
            let rect = if unit.members.len() == 1 {
                r
            } else {
                Rect::sized(r.top_left().plus(offset), page.labels[i].size)
            };
            out.push((page.labels[i].id, rect));
        }
    }
    out.sort_by_key(|x| x.0);
    out
}

/// The stretch of wire `w`'s route that the number at `r` stands beside: the nearest one.
fn nearest_stretch(page: &DraftPage, w: WireId, r: Rect) -> Option<Seg> {
    let wire = page.wires.iter().find(|x| x.id == w)?;
    let c = r.centre();
    geom::runs(&geom::clean(&wire.route))
        .into_iter()
        .min_by_key(|(a, b)| {
            let box_ = Rect::new(*a, *b);
            let dx = (box_.x0 - c.x).max(c.x - box_.x1).max(0);
            let dy = (box_.y0 - c.y).max(c.y - box_.y1).max(0);
            i64::from(dx) + i64::from(dy)
        })
}

/// Whether a unit at a place keeps clear of the other parts' bodies: every label but a pin's number at its author's
/// place, which belongs to its pin wherever the author drew the symbol.
fn bodies(unit: &Unit, at_first: bool) -> bool {
    !(at_first && matches!(unit.kind, LabelKind::PinNumber(_)))
}

/// The first places of the units not yet placed that `r` comes within [`LABEL_CLEAR`] of.
fn reserved(
    units: &[Unit],
    placed: &[Option<(Rect, u32, bool)>],
    firsts: &Buckets,
    u: usize,
    r: Rect,
) -> usize {
    let g = r.grown(LABEL_CLEAR);
    firsts
        .near(g)
        .into_iter()
        .filter(|&v| v != u && placed[v].is_none() && units[v].given.first().is_some_and(|f| f.overlaps(g)))
        .count()
}

/// Whether place `k` of a unit is the one its adapter gave first: its author's.
fn authors(unit: &Unit, k: usize) -> bool {
    k == 0 && !unit.given.is_empty()
}

/// Whether two placed units of one owner stand where each one's author put it, one over the other or one of them a
/// pin's number: the symbol author's layout, and they do not count against each other (their boxes may overlap by the
/// fonts' line gaps; two texts that the author's layout, turned, sets on one row do count).
fn both_authors(
    units: &[Unit],
    (u, at_u, r): (usize, bool, Rect),
    (v, at_v, o): (usize, bool, Rect),
) -> bool {
    at_u && at_v && units[u].owner == units[v].owner && authors_layout((units[u].kind, r), (units[v].kind, o))
}

/// Whether two texts of one owner, both where their author put them, stand as the author's layout: one over the other,
/// or one of them a pin's number.
pub(crate) fn authors_layout((a, r): (LabelKind, Rect), (b, o): (LabelKind, Rect)) -> bool {
    let pin = |k: LabelKind| matches!(k, LabelKind::PinNumber(_));
    pin(a) || pin(b) || geom::stacked(r, o)
}

/// Whether `r` (its unit's author's place or not) keeps [`LABEL_CLEAR`] from every placed label but unit `u`. At its
/// author's place it only must not lie on a label of its own part, and not even that where both stand at their
/// author's places as the author laid them out ([`both_authors`]).
fn labels_clear(
    units: &[Unit],
    placed: &[Option<(Rect, u32, bool)>],
    grid: &Buckets,
    u: usize,
    (r, at_first): (Rect, bool),
) -> bool {
    let g = r.grown(LABEL_CLEAR);
    !grid.near(g).into_iter().any(|v| {
        let Some((o, k, _)) = placed[v] else { return false };
        if v == u || both_authors(units, (u, at_first, r), (v, authors(&units[v], k as usize), o)) {
            return false;
        }
        if at_first && units[v].owner == units[u].owner {
            geom::texts_meet(o, r)
        } else {
            o.overlaps(g)
        }
    })
}

/// What unit `u` costs at `r`, `k` places down its list, against the ground and the labels placed but itself.
fn unit_cost(
    ground: &Ground<'_>,
    units: &[Unit],
    placed: &[Option<(Rect, u32, bool)>],
    grid: &Buckets,
    u: usize,
    r: Rect,
    k: usize,
) -> i64 {
    let step = i64::from(TICKS_PER_STEP);
    let on_labels = grid
        .near(r)
        .into_iter()
        .filter(|&v| {
            v != u
                && placed[v].is_some_and(|(o, vk, _)| {
                    !both_authors(
                        units,
                        (u, authors(&units[u], k), r),
                        (v, authors(&units[v], vk as usize), o),
                    ) && geom::texts_meet(o, r)
                })
        })
        .count() as i64;
    ground.cost(units[u].owner, r) + on_labels * LABEL_CLASH * step + k as i64 * LABEL_RANK * step
}

/// The labels' cost on the page, every placed unit at its place.
fn total(ground: &Ground<'_>, units: &[Unit], placed: &[Option<(Rect, u32, bool)>], grid: &Buckets) -> i64 {
    (0..units.len())
        .filter_map(|u| placed[u].map(|(r, k, _)| unit_cost(ground, units, placed, grid, u, r, k as usize)))
        .sum()
}

/// The one repair round of §5.3.
fn repair(
    ground: &Ground<'_>,
    units: &[Unit],
    lists: &[Vec<(Rect, bool)>],
    clashing: &[usize],
    placed: &mut [Option<(Rect, u32, bool)>],
    grid: &mut Buckets,
) {
    let clear_at = |placed: &[Option<(Rect, u32, bool)>], grid: &Buckets, u: usize, k: usize| {
        let at_first = authors(&units[u], k);
        ground.clear(
            units[u].owner,
            lists[u][k].0,
            (bodies(&units[u], at_first), at_first),
        ) && labels_clear(units, placed, grid, u, (lists[u][k].0, at_first))
    };
    for &u in clashing {
        let Some((ur, _, _)) = placed[u] else { continue };
        let under: Vec<usize> = grid
            .near(ur)
            .into_iter()
            .filter(|&v| v != u && placed[v].is_some_and(|(o, _, _)| o.overlaps(ur)))
            .collect();
        'under: for v in under {
            let (Some((vr, vk, vt)), Some(uplace)) = (placed[v], placed[u]) else {
                continue;
            };
            let before = total(ground, units, placed, grid);
            // Each other clear place of the label it stands on, in order (a mark's name above its arrow, one line
            // higher, frees its cross-reference's places beside it): the first where the change costs less stands.
            for nk in (0..lists[v].len()).filter(|&k| k != vk as usize) {
                if !clear_at(placed, grid, v, nk) {
                    continue;
                }
                let (nr, nt) = lists[v][nk];
                grid.remove(v, vr);
                placed[v] = Some((nr, nk as u32, nt));
                grid.insert(v, nr);
                // The label it stood on takes its first clear place now.
                if let Some(uk) = (0..lists[u].len()).find(|&k| clear_at(placed, grid, u, k)) {
                    grid.remove(u, uplace.0);
                    placed[u] = Some((lists[u][uk].0, uk as u32, lists[u][uk].1));
                    grid.insert(u, lists[u][uk].0);
                }
                if total(ground, units, placed, grid) < before {
                    // Kept; the next label it lies on is tried while it still lies on one (a mark's cross-reference
                    // on a pin's number and on its own name: both move).
                    let Some((now, _, _)) = placed[u] else {
                        break 'under;
                    };
                    let still = grid
                        .near(now)
                        .into_iter()
                        .any(|w| w != u && placed[w].is_some_and(|(o, _, _)| o.overlaps(now)));
                    if still {
                        continue 'under;
                    }
                    break 'under;
                }
                // Not better: both go back.
                if let Some((r, _, _)) = placed[u] {
                    grid.remove(u, r);
                }
                grid.remove(v, nr);
                placed[u] = Some(uplace);
                placed[v] = Some((vr, vk, vt));
                grid.insert(u, uplace.0);
                grid.insert(v, vr);
            }
        }
    }
}
