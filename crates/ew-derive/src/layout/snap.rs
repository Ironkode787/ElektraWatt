// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rail model and the mode resolution (WP-S4.02 D3 to D5, doc 12 §3.3, §3.4): pure functions over the items of
//! one rail, shared by the layout tool's ghost (part 3) and the `layout.place` and `layout.move` commands, so the ghost
//! and the commit agree by construction (D4).
//!
//! A rail holds *items* along `s ∈ [0, L]` (integer nanometres from `path[0]`): each placement's bare footprint
//! interval `[s0, s1]` from `rail_occupancy`, its clearance pads (non-strips only) and, for a strip, the width `w` of
//! the end-bracket part it declares. Nothing here reads the zoom or a window: target snapping is the tool's (D4).
//!
//! **Minimum gap** between consecutive items `a` (left) and `b` (right) ([`min_gap`]): strip–strip `w(a)`; strip–other
//! `w(a) + pad_before(b)`; other–strip `pad_after(a) + w(b)`; other–other `pad_after(a) + pad_before(b)`; at the rail
//! start before a strip `w(strip)` ([`start_gap`]), at the rail end after a strip `w(strip)` ([`end_gap`]), else 0.
//!
//! **Runs and brackets** ([`runs`], [`brackets`]): consecutive strips are in one run when their gap is 0 or less than
//! `w(a) + w(b)`; a run of `k` strips has `k + 1` brackets, one at its start attributed to its first strip and one after
//! each strip attributed to it; a strip that declares no bracket part contributes none.
//!
//! **Modes** ([`resolve`]): free, push and swap as D5 states them. Loaded items that overlap or lie outside `[0, L]`
//! are accepted as given; only the items a mode moves are re-checked. Every function is total: no input panics, and
//! the arithmetic saturates.

use ew_ids::Id;
use ew_schema::Placement;

/// One item of a rail (D3): a placement's footprint interval along the axis and what spaces it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RailItem {
    /// Where the bare footprint starts along the axis, in nanometres.
    pub s0: i128,
    /// Where it ends (`s1 ≥ s0` for a well-formed item; a reversed one counts as zero wide).
    pub s1: i128,
    /// Whether the item is a strip placed as a whole.
    pub strip: bool,
    /// A strip's declared end-bracket width (its first part use whose part has kind `end_bracket`; the part's
    /// `mech.width`, absent: 0); `None` when the strip declares no bracket part. Ignored for a non-strip.
    pub w: Option<i128>,
    /// The clearance pads `[before, after]` along the axis (a non-strip's; ignored for a strip).
    pub pad: [i128; 2],
    /// Whether the placement is locked: push and swap never move it.
    pub locked: bool,
    /// The placement; `None` for an item that is not placed yet (the item a tool drags in, a candidate).
    pub placement: Option<Id<Placement>>,
}

impl RailItem {
    /// The footprint's width along the axis (`s1 − s0`, at least 0).
    pub fn width(&self) -> i128 {
        self.s1.saturating_sub(self.s0).max(0)
    }

    /// The bracket width that spaces this item: a strip's declared `w` (at least 0), else 0.
    fn bw(&self) -> i128 {
        if self.strip { self.w.unwrap_or(0).max(0) } else { 0 }
    }

    /// The pad before (a non-strip's, at least 0).
    fn pad_before(&self) -> i128 {
        if self.strip { 0 } else { self.pad[0].max(0) }
    }

    /// The pad after (a non-strip's, at least 0).
    fn pad_after(&self) -> i128 {
        if self.strip { 0 } else { self.pad[1].max(0) }
    }

    /// The item with its footprint starting at `s0`, its width kept.
    fn at(&self, s0: i128) -> RailItem {
        RailItem {
            s0,
            s1: s0.saturating_add(self.width()),
            ..*self
        }
    }
}

/// The minimum gap between consecutive items `a` (left) and `b` (right) (D3).
pub fn min_gap(a: &RailItem, b: &RailItem) -> i128 {
    match (a.strip, b.strip) {
        (true, true) => a.bw(),
        (true, false) => a.bw().saturating_add(b.pad_before()),
        (false, true) => a.pad_after().saturating_add(b.bw()),
        (false, false) => a.pad_after().saturating_add(b.pad_before()),
    }
}

/// The minimum gap between the rail start and a first item `b` (D3): a strip's bracket width, else 0.
pub fn start_gap(b: &RailItem) -> i128 {
    b.bw()
}

/// The minimum gap between a last item `a` and the rail end (D3): a strip's bracket width, else 0.
pub fn end_gap(a: &RailItem) -> i128 {
    a.bw()
}

/// The indices of `items` in rail order: `(s0, s1, placement)`, then the index.
fn order(items: &[RailItem]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..items.len()).collect();
    idx.sort_by_key(|i| (items[*i].s0, items[*i].s1, items[*i].placement, *i));
    idx
}

/// The runs of strips (D3): each run lists the indices into `items` of its strips in rail order. Consecutive items (in
/// rail order) that are both strips are in one run when their gap is 0 or less (an overlap) or less than `w(a) + w(b)`;
/// a non-strip between two strips separates them.
pub fn runs(items: &[RailItem]) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut prev: Option<usize> = None;
    for i in order(items) {
        let cur = &items[i];
        if !cur.strip {
            prev = None;
            continue;
        }
        let joins = prev.is_some_and(|p| {
            let a = &items[p];
            let gap = cur.s0.saturating_sub(a.s1);
            gap <= 0 || gap < a.bw().saturating_add(cur.bw())
        });
        match out.last_mut() {
            Some(run) if joins => run.push(i),
            _ => out.push(vec![i]),
        }
        prev = Some(i);
    }
    out
}

/// Where an end bracket stands relative to the strip it is attributed to (D3).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum BracketRole {
    /// At the start of a run, before its first strip.
    RunStart,
    /// After its strip.
    After,
}

/// One end bracket over items: its interval along the axis, the index of the strip it is attributed to and its role.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct BracketSpan {
    /// Where the bracket starts.
    pub s0: i128,
    /// Where it ends (`s0 + w`).
    pub s1: i128,
    /// The index into the items of the strip it is attributed to.
    pub item: usize,
    /// Before its run or after its strip (C44-p3#4: a bracket of zero length cannot tell it from its interval).
    pub role: BracketRole,
}

/// The end brackets of `items` (D3), sorted by `(s0, s1, item, role)`: per run, one at its start `[s0 − w, s0]` attributed
/// to its first strip, and one after each strip `[s1, s1 + w]` attributed to it; a strip that declares no bracket part
/// (`w` is `None`) contributes none. The same rule as `group_accessories`, over any arrangement, so a tool can show the
/// brackets of a candidate.
pub fn brackets(items: &[RailItem]) -> Vec<BracketSpan> {
    let mut out = Vec::new();
    for run in runs(items) {
        let Some(&first) = run.first() else {
            continue;
        };
        let f = &items[first];
        if f.w.is_some() {
            out.push(BracketSpan {
                s0: f.s0.saturating_sub(f.bw()),
                s1: f.s0,
                item: first,
                role: BracketRole::RunStart,
            });
        }
        for &k in &run {
            let s = &items[k];
            if s.w.is_some() {
                out.push(BracketSpan {
                    s0: s.s1,
                    s1: s.s1.saturating_add(s.bw()),
                    item: k,
                    role: BracketRole::After,
                });
            }
        }
    }
    out.sort();
    out
}

/// The free length of a rail (D12): `L − Σ item widths − Σ bracket widths`, in nanometres (R1 of Golden A: 341 mm,
/// R2: 381.4 mm). Overlapping items count each.
pub fn free_length(length: i128, items: &[RailItem], brackets: &[BracketSpan]) -> i128 {
    let used = items
        .iter()
        .map(RailItem::width)
        .chain(brackets.iter().map(|b| b.s1.saturating_sub(b.s0).max(0)))
        .fold(0i128, i128::saturating_add);
    length.saturating_sub(used)
}

/// Next-fit (WP-S4.02 D6, `layout.auto_fill`): the smallest left edge `s ≥ floor` at which `item` (its width, strip
/// flag, bracket width and pads; its `s0`, `s1` are not read as a position) fits on a rail of length `length` holding
/// `items` (any order), as free mode judges a fit: the minimum gap to the item before it in rail order (by `s0`) and to
/// the item after it and the rail ends' gaps are held; an earlier item that ends later than the one before it is only
/// cleared by its bare end (no pad, no bracket; FT-942). `None` when no such place exists. Nothing moves: this is the placement
/// rule of a fill, not a mode ([`resolve`] stays the one resolver of a drop).
pub fn next_fit(items: &[RailItem], length: i128, item: &RailItem, floor: i128) -> Option<i128> {
    let mut sorted: Vec<RailItem> = items.to_vec();
    sorted.sort_by_key(|i| (i.s0, i.s1, i.placement));
    let wm = item.width();
    let rail_lo = start_gap(item).max(floor);
    let rail_hi = length.saturating_sub(end_gap(item)).saturating_sub(wm);
    let mut max_s1 = i128::MIN;
    for k in 0..=sorted.len() {
        let mut lo = rail_lo;
        if let Some(a) = k.checked_sub(1).and_then(|p| sorted.get(p)) {
            max_s1 = max_s1.max(a.s1);
            lo = lo.max(a.s1.saturating_add(min_gap(a, item))).max(max_s1);
        }
        let mut hi = rail_hi;
        if let Some(b) = sorted.get(k) {
            hi = hi.min(b.s0.saturating_sub(min_gap(item, b)).saturating_sub(wm));
        }
        if lo <= hi {
            return Some(lo);
        }
    }
    None
}

/// How a dropped item is resolved against its neighbours (D5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnapMode {
    /// Stay where it fits, else the nearest position where it fits; nothing else moves.
    Free,
    /// Insert, moving the occupants to the right as far as their minimum gaps require.
    Push,
    /// Exchange with the placement under the item's centre on the same rail.
    Swap,
}

/// The result of [`resolve`]: where the item's footprint starts and the neighbours that move with it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Resolution {
    /// The item's footprint's new left edge, in nanometres from `path[0]`.
    pub s_left: i128,
    /// Each moved neighbour with its footprint's new left edge, in rail order; a neighbour that keeps its place is not
    /// listed (nor is the item itself).
    pub moved: Vec<(Id<Placement>, i128)>,
}

/// Why [`resolve`] refuses a drop (D5); the caller words it with the rail's label and the devices' tags.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnapError {
    /// Free: no gap of the rail is wide enough for the item of this width (nanometres).
    NoGap {
        /// The item's width.
        width: i128,
    },
    /// Push or swap of a locked item (the item itself; free moves it whatever its lock).
    MovedLocked,
    /// Push or swap would move this locked placement (or it is in the swapped segment).
    Locked(Option<Id<Placement>>),
    /// Push or swap would move an item past the rail end, with its rail-end gap, by `by` nanometres; `neighbour` is
    /// `None` when the item itself passes it.
    PastEnd {
        /// The neighbour that passes the end; `None`: the item itself.
        neighbour: Option<Id<Placement>>,
        /// By how much.
        by: i128,
    },
    /// Swap of an item that is not on this rail (another rail, or not placed yet).
    SwapAcrossRails,
    /// Swap with no placement under the item's centre.
    SwapNoTarget,
}

/// `resolve(items, L, moved, s_left, mode)` (D4, D5): where `moved`, dropped with its footprint's left edge at
/// `s_left`, lands on a rail of length `length` holding `items`, and which neighbours move.
///
/// `items` is the rail's occupancy (any order; the moved placement may be among them and is removed first). `moved`
/// gives the item's width (`s1 − s0`), strip flag, bracket width, pads, lock and placement; its `s0`, `s1` are not read
/// as a position. The rules:
///
/// - **Free:** if the item fits at `s_left`, it stays; else it goes to the position in the gap closest to `s_left`
///   where it fits, ties to the smaller `s`; no gap fits: [`SnapError::NoGap`]. It fits when the minimum gap to the item
///   before it in rail order (by `s0`) and to the item after it and the rail ends' gaps are held; an earlier item that
///   ends later than the one before it is only cleared by its bare end, without its pad or bracket (FT-942).
/// - **Push:** the occupants whose centre is left of the item's centre stay (one centred exactly on it counts as
///   right); the item starts at `max(s_left, the left neighbour's end + minimum gap)` (no left neighbour: the rail
///   start's gap); each occupant to the right moves right only as far as its minimum gap to its new left neighbour
///   requires, cascading, and the cascade stops at the first that need not move. Refused: a locked item, a locked
///   occupant that would move, the last moved item passing `L` with its rail-end gap.
/// - **Swap:** only for an item on this rail; the target is the occupant under the item's centre (`s0 ≤ c < s1`).
///   The segment from the first to the last of the two is re-laid from its first item's place with the two exchanged,
///   every other item in its order, each gap (the one before the segment included) kept where it still meets the new
///   minimum gap, else the minimum gap; the items after it cascade as in push. Refused: a locked item, a locked
///   placement in the segment or one that would move, the last moved item passing `L`.
///
/// # Errors
///
/// The [`SnapError`] of the first rule that refuses.
pub fn resolve(
    items: &[RailItem],
    length: i128,
    moved: RailItem,
    s_left: i128,
    mode: SnapMode,
) -> Result<Resolution, SnapError> {
    let mut all: Vec<RailItem> = items.to_vec();
    all.sort_by_key(|i| (i.s0, i.s1, i.placement));
    let is_moved = |i: &RailItem| moved.placement.is_some() && i.placement == moved.placement;
    match mode {
        SnapMode::Free => {
            let others: Vec<RailItem> = all.into_iter().filter(|i| !is_moved(i)).collect();
            free(&others, length, &moved, s_left)
        }
        SnapMode::Push => {
            if moved.locked {
                return Err(SnapError::MovedLocked);
            }
            let others: Vec<RailItem> = all.into_iter().filter(|i| !is_moved(i)).collect();
            push(&others, length, &moved, s_left)
        }
        SnapMode::Swap => {
            let Some(oi) = all.iter().position(is_moved) else {
                return Err(SnapError::SwapAcrossRails);
            };
            if moved.locked {
                return Err(SnapError::MovedLocked);
            }
            swap(&all, oi, length, &moved, s_left)
        }
    }
}

/// D5's free mode over the other items (rail order).
fn free(others: &[RailItem], length: i128, m: &RailItem, s_left: i128) -> Result<Resolution, SnapError> {
    let wm = m.width();
    let rail_lo = start_gap(m);
    let rail_hi = length.saturating_sub(end_gap(m)).saturating_sub(wm);
    let mut best: Option<(i128, i128)> = None;
    let mut max_s1 = i128::MIN;
    for k in 0..=others.len() {
        let mut lo = rail_lo;
        if let Some(a) = k.checked_sub(1).and_then(|p| others.get(p)) {
            max_s1 = max_s1.max(a.s1);
            lo = lo.max(a.s1.saturating_add(min_gap(a, m))).max(max_s1);
        }
        let mut hi = rail_hi;
        if let Some(b) = others.get(k) {
            hi = hi.min(b.s0.saturating_sub(min_gap(m, b)).saturating_sub(wm));
        }
        if lo > hi {
            continue;
        }
        let c = s_left.clamp(lo, hi);
        let d = c.saturating_sub(s_left).saturating_abs();
        if best.is_none_or(|b| (d, c) < b) {
            best = Some((d, c));
        }
    }
    match best {
        Some((_, c)) => Ok(Resolution {
            s_left: c,
            moved: Vec::new(),
        }),
        None => Err(SnapError::NoGap { width: wm }),
    }
}

/// Twice an item's centre (exact in integers).
fn centre2(i: &RailItem) -> i128 {
    i.s0.saturating_add(i.s1)
}

/// D5's push mode over the other items (rail order).
fn push(others: &[RailItem], length: i128, m: &RailItem, s_left: i128) -> Result<Resolution, SnapError> {
    let cm = s_left.saturating_mul(2).saturating_add(m.width());
    let (left, right): (Vec<RailItem>, Vec<RailItem>) = others.iter().partition(|o| centre2(o) < cm);
    // The left neighbour: the left occupant that ends last (the last in rail order on a tie).
    let neighbour = left.iter().fold(None::<&RailItem>, |best, o| match best {
        Some(b) if b.s1 > o.s1 => Some(b),
        _ => Some(o),
    });
    let base = neighbour.map_or_else(|| start_gap(m), |n| n.s1.saturating_add(min_gap(n, m)));
    let placed = m.at(s_left.max(base));
    let moved = cascade(placed, &right, length, true)?;
    Ok(Resolution {
        s_left: placed.s0,
        moved,
    })
}

/// The push cascade: `first` is the last re-laid item (the dropped item, or the end of a swapped segment), `rest` the
/// items after it in rail order. Each item moves right only as far as its minimum gap to its moved left neighbour
/// requires; the cascade stops at the first that need not move. Then the last moved item, with its rail-end gap when it
/// is the last item of the rail, must end by `length`. `first_is_item`: `first` is the dropped item (a past-end refusal
/// then names no neighbour). Push and swap run it, and so does a strip that grows by inserted terminals (`ew-commands`,
/// FT-1441), with `first` the grown strip.
///
/// # Errors
///
/// [`SnapError::Locked`] for a locked item that would move, [`SnapError::PastEnd`] when the last moved item passes
/// `length`; the moves are then not returned.
pub fn cascade(
    first: RailItem,
    rest: &[RailItem],
    length: i128,
    first_is_item: bool,
) -> Result<Vec<(Id<Placement>, i128)>, SnapError> {
    let mut moved = Vec::new();
    let mut prev = first;
    let mut last: Option<usize> = None;
    for (i, o) in rest.iter().enumerate() {
        let need = prev.s1.saturating_add(min_gap(&prev, o));
        if need <= o.s0 {
            break;
        }
        if o.locked {
            return Err(SnapError::Locked(o.placement));
        }
        let n = o.at(need);
        if let Some(p) = o.placement {
            moved.push((p, need));
        }
        prev = n;
        last = Some(i);
    }
    let is_last = match last {
        Some(i) => i + 1 == rest.len(),
        None => rest.is_empty(),
    };
    let end = if is_last {
        prev.s1.saturating_add(end_gap(&prev))
    } else {
        prev.s1
    };
    let by = end.saturating_sub(length);
    if by > 0 {
        let neighbour = match last {
            Some(_) => prev.placement,
            None if first_is_item => None,
            None => prev.placement,
        };
        return Err(SnapError::PastEnd { neighbour, by });
    }
    Ok(moved)
}

/// D5's swap mode: `all` in rail order, `oi` the moved placement's index in it.
fn swap(
    all: &[RailItem],
    oi: usize,
    length: i128,
    m: &RailItem,
    s_left: i128,
) -> Result<Resolution, SnapError> {
    let cm = s_left.saturating_mul(2).saturating_add(m.width());
    let Some(t) = all.iter().enumerate().position(|(i, o)| {
        i != oi && o.placement != m.placement && o.s0.saturating_mul(2) <= cm && cm < o.s1.saturating_mul(2)
    }) else {
        return Err(SnapError::SwapNoTarget);
    };
    if all[t].locked {
        return Err(SnapError::Locked(all[t].placement));
    }
    let (i, j) = (oi.min(t), oi.max(t));
    let original = &all[i..=j];
    let mut seq: Vec<RailItem> = original.to_vec();
    seq.swap(0, j - i);
    let pos_m = if oi == i { j - i } else { 0 };
    seq[pos_m] = RailItem {
        s0: 0,
        s1: m.width(),
        ..*m
    };
    if let Some(l) = seq.iter().enumerate().find(|(k, x)| *k != pos_m && x.locked) {
        return Err(SnapError::Locked(l.1.placement));
    }
    let before = i.checked_sub(1).map(|p| all[p]);
    let mut prev_end = before.map_or(0, |b| b.s1);
    let mut moved = Vec::new();
    let mut new_left = s_left;
    for k in 0..seq.len() {
        let kept = if k == 0 {
            original[0].s0.saturating_sub(prev_end)
        } else {
            original[k].s0.saturating_sub(original[k - 1].s1)
        };
        let mg = match (k, before) {
            (0, Some(b)) => min_gap(&b, &seq[0]),
            (0, None) => start_gap(&seq[0]),
            _ => min_gap(&seq[k - 1], &seq[k]),
        };
        let s0 = prev_end.saturating_add(kept.max(mg));
        let old = seq[k];
        seq[k] = old.at(s0);
        prev_end = seq[k].s1;
        if k == pos_m {
            new_left = s0;
        } else if let Some(p) = old.placement {
            let was = original.iter().find(|x| x.placement == Some(p)).map(|x| x.s0);
            if was != Some(s0) {
                moved.push((p, s0));
            }
        }
    }
    let last = seq[seq.len() - 1];
    let tail = cascade(last, &all[j + 1..], length, pos_m + 1 == seq.len())?;
    moved.extend(tail);
    Ok(Resolution {
        s_left: new_left,
        moved,
    })
}

#[cfg(test)]
mod tests;
