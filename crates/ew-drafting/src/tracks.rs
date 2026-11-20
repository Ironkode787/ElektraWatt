// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tracks (docs/design/06a-drafting.md §3.5): the wires that step sideways between two rows of pins, assigned
//! together so that they nest and do not cross. The router lays them after the trunks and before the search
//! ([`crate::route`]); [`tracks_needed`] counts them for the placer (§4.5).
//!
//! A **stepping wire** is a job between two pins that cannot be one segment and whose shape is known: a **Z** between
//! two pins that face each other and are not in line (it leaves along the first pin, steps sideways once, arrives
//! along the second), or a **U** between two pins on the same side (out, across, back). Its one free number is the
//! **track**: the line of its middle segment.
//!
//! A wire is read in its **frame**: *along* the rows, and *deep* across them, growing away from the row it leaves (a
//! Z leaves its upper pin, or its left one; a U leaves its row outward). Its **span** is the stretch along between its
//! two legs; its tracks lie from [`TRACK_PITCH`] beyond the row it leaves to [`STUB`] before the pin it arrives at (a
//! U's to a step inside the drawing area). Stepping wires of one frame whose depths and spans overlap form a
//! **channel**. Within a channel:
//!
//! 1. *Order.* Wire *p* takes a track before wire *q* (nearer to the row it leaves) when a leg of *p* would otherwise
//!    cross *q*'s middle: a leg of *p* that leaves the near row lies within *q*'s span, or a leg of *q* that arrives at
//!    the far row lies within *p*'s span. A leg on a pin that the other wire also ends at counts for neither. Where two
//!    wires ask for both orders, one crossing is unavoidable and the pair is ordered by its legs' places (the near leg
//!    first, smaller first). The wires are then taken in that order, ties by the left end of the span; a ring of three
//!    or more is broken at the wire with the fewest wires still before it.
//! 2. *Assignment.* Each wire takes the first track, from the near side, that lies [`TRACK_PITCH`] beyond the tracks
//!    of the wires that must come before it and as far from the track of every wire whose span meets its own: the
//!    left-edge method with its order constraints. When the channel has too few such tracks for its wires, the
//!    assignment is made again with [`TRACK_PITCH_NARROW`].
//! 3. *Laid* by the router where the whole route is clear in the occupancy; a track that is not is passed over, and a
//!    wire with no free track goes to the search.

use std::collections::{BTreeMap, BTreeSet};

use crate::consts::{BODY_GAP, STUB, TICKS_PER_STEP, TRACK_PITCH, TRACK_PITCH_NARROW};
use crate::model::{PinId, Pt, Rect, Side, Tick};

/// How many tracks the stepping wires between a row of pins and the row they face will need (§4.5): each wire by the
/// coordinates along the rows of the pin it leaves and of the pin it arrives at, the rows as far apart as need be. They
/// are counted with the order and the assignment the router uses, at [`TRACK_PITCH`]; a wire whose two pins are in line
/// needs none.
pub fn tracks_needed(wires: &[(Tick, Tick)]) -> usize {
    let lanes: Vec<Lane> = wires
        .iter()
        .filter(|(a, b)| a != b)
        .map(|&(a, b)| Lane {
            legs: [
                Leg {
                    along: a,
                    near: true,
                    pin: None,
                },
                Leg {
                    along: b,
                    near: false,
                    pin: None,
                },
            ],
            from: 0,
            last: Tick::MAX / 2,
        })
        .collect();
    assign(&lanes, TRACK_PITCH, &mut |_, _| true)
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>()
        .len()
}

/// A leg of a stepping wire.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Leg {
    /// Where it stands along the rows.
    pub along: Tick,
    /// Whether it leaves the near row (else it arrives at the far row).
    pub near: bool,
    /// The pin it stands on.
    pub pin: Option<PinId>,
}

/// A stepping wire in its frame.
#[derive(Clone, Debug)]
pub(crate) struct Lane {
    /// Its two legs.
    pub legs: [Leg; 2],
    /// The depth of the row it leaves: its tracks are counted from there.
    pub from: Tick,
    /// The deepest track it may take.
    pub last: Tick,
}

impl Lane {
    fn span(&self) -> (Tick, Tick) {
        let (a, b) = (self.legs[0].along, self.legs[1].along);
        (a.min(b), a.max(b))
    }

    /// Whether `leg` lies within this wire's span and on no pin of its own.
    fn holds(&self, leg: Leg) -> bool {
        let (lo, hi) = self.span();
        lo <= leg.along
            && leg.along <= hi
            && !self
                .legs
                .iter()
                .any(|o| o.along == leg.along && o.pin.is_some() && o.pin == leg.pin)
    }

    /// Whether the two spans meet.
    fn meets(&self, other: &Lane) -> bool {
        let ((a0, a1), (b0, b1)) = (self.span(), other.span());
        a0 <= b1 && b0 <= a1
    }

    /// The order among a pair that asks for both orders: its legs' places, the near leg first.
    fn places(&self) -> (Tick, Tick) {
        let mut legs = self.legs;
        legs.sort_by_key(|l| (!l.near, l.along));
        (legs[0].along, legs[1].along)
    }
}

/// Whether `p` must take a track before `q` (module documentation, 1).
fn before(p: &Lane, q: &Lane) -> bool {
    p.legs.iter().any(|l| l.near && q.holds(*l)) || q.legs.iter().any(|l| !l.near && p.holds(*l))
}

/// The order of `lanes` (module documentation, 1) and, per lane, the lanes that must come before it.
fn order(lanes: &[Lane]) -> (Vec<usize>, Vec<Vec<usize>>) {
    let n = lanes.len();
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (before(&lanes[i], &lanes[j]), before(&lanes[j], &lanes[i]));
            let first = match (a, b) {
                (true, true) => Some((lanes[i].places(), i) < (lanes[j].places(), j)),
                (true, false) => Some(true),
                (false, true) => Some(false),
                (false, false) => None,
            };
            match first {
                Some(true) => preds[j].push(i),
                Some(false) => preds[i].push(j),
                None => {}
            }
        }
    }
    let key = |k: usize| {
        let (lo, hi) = lanes[k].span();
        (lo, hi, k)
    };
    let mut done = vec![false; n];
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        let waiting = |k: usize| preds[k].iter().filter(|p| !done[**p]).count();
        let next = (0..n)
            .filter(|k| !done[*k])
            .min_by_key(|k| (waiting(*k), key(*k)));
        let Some(k) = next else { break };
        done[k] = true;
        out.push(k);
    }
    (out, preds)
}

/// The tracks of `lanes` at `pitch` (module documentation, 2), each the depth of its middle; `accept` is asked for each
/// track in turn (the router lays the wire there when it is clear) and a lane without one has `None`.
fn assign(lanes: &[Lane], pitch: Tick, accept: &mut dyn FnMut(usize, Tick) -> bool) -> Vec<Option<Tick>> {
    let (order, preds) = order(lanes);
    let mut tracks: Vec<Option<Tick>> = vec![None; lanes.len()];
    for k in order {
        let lane = &lanes[k];
        let mut d = preds[k]
            .iter()
            .filter_map(|p| tracks[*p])
            .map(|t| t + pitch)
            .fold(lane.from + TRACK_PITCH, Tick::max);
        d = (d + TICKS_PER_STEP - 1).div_euclid(TICKS_PER_STEP) * TICKS_PER_STEP;
        while d <= lane.last {
            let spaced = lanes.iter().zip(&tracks).all(|(other, t)| match t {
                Some(t) => !other.meets(lane) || (d - t).abs() >= pitch,
                None => true,
            });
            if spaced && accept(k, d) {
                tracks[k] = Some(d);
                break;
            }
            d += TICKS_PER_STEP;
        }
    }
    tracks
}

/// The pitch of a channel: [`TRACK_PITCH`] where its wires find room at it, else [`TRACK_PITCH_NARROW`].
fn pitch(lanes: &[Lane]) -> Tick {
    if assign(lanes, TRACK_PITCH, &mut |_, _| true)
        .iter()
        .all(Option::is_some)
    {
        TRACK_PITCH
    } else {
        TRACK_PITCH_NARROW
    }
}

/// The channels of `lanes` (of one frame): the lanes joined, directly or through others, by depths and spans that
/// overlap; each channel in the order of its first lane, its lanes in order.
fn channels(lanes: &[Lane]) -> Vec<Vec<usize>> {
    let n = lanes.len();
    let mut root: Vec<usize> = (0..n).collect();
    fn find(root: &[usize], mut k: usize) -> usize {
        while root[k] != k {
            k = root[k];
        }
        k
    }
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (&lanes[i], &lanes[j]);
            if a.meets(b) && a.from <= b.last && b.from <= a.last {
                let (x, y) = (find(&root, i), find(&root, j));
                root[x.max(y)] = x.min(y);
            }
        }
    }
    let mut out: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for k in 0..n {
        out.entry(find(&root, k)).or_default().push(k);
    }
    out.into_values().collect()
}

/// The frame of a stepping wire: whether its middle runs across the page, and whether its depth grows with the
/// coordinate across the middle (`1`) or against it (`-1`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct Frame {
    horizontal: bool,
    sense: Tick,
}

impl Frame {
    fn along(self, p: Pt) -> Tick {
        if self.horizontal { p.x } else { p.y }
    }

    fn depth(self, p: Pt) -> Tick {
        self.sense * if self.horizontal { p.y } else { p.x }
    }

    /// The route from `a` to `b` with its middle at depth `d`.
    pub fn route(self, a: Pt, b: Pt, d: Tick) -> Vec<Pt> {
        let c = self.sense * d;
        if self.horizontal {
            vec![a, Pt::new(a.x, c), Pt::new(b.x, c), b]
        } else {
            vec![a, Pt::new(c, a.y), Pt::new(c, b.y), b]
        }
    }
}

/// The stepping wire between the pins `a` and `b` (each its point, side and ID), its U's tracks within `area`: its
/// frame and lane; `None` for a wire of another shape.
pub(crate) fn stepping(a: (Pt, Side, PinId), b: (Pt, Side, PinId), area: Rect) -> Option<(Frame, Lane)> {
    let ((pa, sa, ia), (pb, sb, ib)) = (a, b);
    if sa.vertical() != sb.vertical() {
        return None;
    }
    let horizontal = sa.vertical();
    let probe = Frame {
        horizontal,
        sense: 1,
    };
    if probe.along(pa) == probe.along(pb) {
        return None;
    }
    // Depth grows down, or right, from the near row.
    let grows = |s: Side| matches!(s, Side::Down | Side::Right);
    let leg = |p: Pt, pin: PinId, near: bool| Leg {
        along: probe.along(p),
        near,
        pin: Some(pin),
    };
    if sa == sb {
        // A U: out, across, back; its tracks beyond its deeper pin, a step inside the drawing area.
        let frame = Frame {
            horizontal,
            sense: if grows(sa) { 1 } else { -1 },
        };
        let edge = match (horizontal, frame.sense > 0) {
            (true, true) => area.y1,
            (true, false) => area.y0,
            (false, true) => area.x1,
            (false, false) => area.x0,
        };
        let lane = Lane {
            legs: [leg(pa, ia, true), leg(pb, ib, true)],
            from: frame.depth(pa).max(frame.depth(pb)),
            last: frame.sense * edge - BODY_GAP,
        };
        return Some((frame, lane));
    }
    // A Z: the near pin faces down (or right) at the far pin, which faces back.
    let ((n, ni), (f, fi)) = if grows(sa) {
        ((pa, ia), (pb, ib))
    } else {
        ((pb, ib), (pa, ia))
    };
    let frame = Frame {
        horizontal,
        sense: 1,
    };
    if frame.depth(n) >= frame.depth(f) {
        return None;
    }
    let lane = Lane {
        legs: [leg(n, ni, true), leg(f, fi, false)],
        from: frame.depth(n),
        last: frame.depth(f) - STUB,
    };
    Some((frame, lane))
}

/// The tracks of the stepping wires `wires` of one frame (each by its lane): per channel at its pitch, `accept` asked
/// for each track in turn with the wire's index in `wires`; per wire its track, or `None`.
pub(crate) fn lay(lanes: &[Lane], accept: &mut dyn FnMut(usize, Tick) -> bool) -> Vec<Option<Tick>> {
    let mut out = vec![None; lanes.len()];
    for channel in channels(lanes) {
        let sub: Vec<Lane> = channel.iter().map(|k| lanes[*k].clone()).collect();
        let tracks = assign(&sub, pitch(&sub), &mut |i, d| accept(channel[i], d));
        for (i, t) in tracks.into_iter().enumerate() {
            out[channel[i]] = t;
        }
    }
    out
}
