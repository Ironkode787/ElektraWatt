// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The polyline a connection is drawn along on its page.
//!
//! A connection's stored route is its drawing (ADR-0033): the scene never routes. A stored route holds
//! every vertex from end `a`'s point to end `b`'s and is drawn as stored ([`connection_polyline`]). A connection
//! without one is *unrouted*: it is drawn as a right-angled line between its ends
//! (`ew_drafting::unrouted_line`; the page draws it dashed in the thinnest pen), never as a diagonal, and as
//! nothing where its ends meet.
//!
//! Everything is integer nanometres ([`Nm`], [`Pt2`], [`Rect`]) on the page, origin top-left, y down, as in
//! [`symbol`](crate::symbol). The functions are pure, deterministic and integer-only, and the result never leaves the
//! page: every vertex is clamped into the page rectangle.
//!
//! # Junctions
//!
//! A connection whose end is a junction passes the junction position as that end with `dir: None`, so every
//! arm of a junction ends exactly at the junction point.

use crate::draft::side;
use ew_schema::Dir4;
use ew_units::{Nm, Pt2, Rect};

/// One end of a connection: its position and, for a CP end, the direction a wire leaves it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteEnd {
    /// Position of the end on the page.
    pub pos: Pt2,
    /// `None` for a junction end or an unknown direction.
    pub dir: Option<Dir4>,
}

/// The polyline of a connection from `a` to `b` (both ends included).
///
/// - `stored` non-empty: `ew_derive::stored_path(a.pos, stored, b.pos)` (the stored route holds every vertex from
///   end `a`'s point to end `b`'s, both included; equal neighbours merged, a stale end joined by one
///   straight segment), each point clamped into `page`, equal neighbours merged again. Collinear vertices are kept:
///   the model's route is authoritative and the user placed those vertices.
/// - `stored` empty: the unrouted line, right-angled (`ew_drafting::unrouted_line`: straight for aligned
///   ends, else one corner, the first leg along `a`'s direction), clamped into `page`; `[a.pos]` where the ends
///   meet (nothing is drawn).
pub fn connection_polyline(a: &RouteEnd, b: &RouteEnd, stored: &[Pt2], page: Rect) -> Vec<Pt2> {
    let path = if stored.is_empty() {
        let end = |e: &RouteEnd| ((e.pos.x.0, e.pos.y.0), e.dir.as_ref().and_then(side));
        ew_drafting::unrouted_line(end(a), end(b))
            .into_iter()
            .map(|(x, y)| Pt2::new(Nm(x), Nm(y)))
            .collect()
    } else {
        ew_derive::stored_path(a.pos, stored, b.pos)
    };
    let mut pts: Vec<Pt2> = path.into_iter().map(|p| clamp(p, page)).collect();
    pts.dedup();
    pts
}

/// `p` pulled onto the page rectangle, per axis.
fn clamp(p: Pt2, page: Rect) -> Pt2 {
    let (lo, hi) = (page.min(), page.max());
    Pt2::new(clamp_nm(p.x, lo.x, hi.x), clamp_nm(p.y, lo.y, hi.y))
}

fn clamp_nm(v: Nm, lo: Nm, hi: Nm) -> Nm {
    v.max(lo).min(hi)
}

/// The unit step from `a` to `b` on one line; `None` for one point or a slant.
fn unit(a: Pt2, b: Pt2) -> Option<(i64, i64)> {
    let (dx, dy) = ((b.x - a.x).0, (b.y - a.y).0);
    match (dx.signum(), dy.signum()) {
        (0, 0) => None,
        (sx, 0) => Some((sx, 0)),
        (0, sy) => Some((0, sy)),
        _ => None,
    }
}

/// The point where two lines part, the direction they arrived there in, and the length of their last shared run.
type Parting = (Pt2, (i64, i64), Nm);

/// Where two lines that start at one point part: walked from it together while they run the same way; `None` when they
/// share nothing.
fn parting(a: &[Pt2], b: &[Pt2]) -> Option<Parting> {
    let (mut ia, mut ib) = (1usize, 1usize);
    let mut cur = *a.first()?;
    let mut found: Option<Parting> = None;
    while ia < a.len() && ib < b.len() {
        let (ta, tb) = (a[ia], b[ib]);
        let d = unit(cur, ta)?;
        if unit(cur, tb) != Some(d) {
            break;
        }
        let len = |t: Pt2| (t.x - cur.x).0.abs() + (t.y - cur.y).0.abs();
        let l = len(ta).min(len(tb));
        let next = Pt2::new(cur.x + Nm(d.0 * l), cur.y + Nm(d.1 * l));
        let run = match found {
            Some((_, prev, so_far)) if prev == d => so_far + Nm(l),
            _ => Nm(l),
        };
        found = Some((next, d, run));
        cur = next;
        if cur == ta {
            ia += 1;
        }
        if cur == tb {
            ib += 1;
        }
    }
    found
}

/// The drawn lines of the wires of one pin, where they share their run out of it, parted as the trade draws two wires
/// on one terminal (docs/design/06a-drafting.md §3.4): where the run parts, a wire that turns while another goes on
/// straight, or of two that turn away from each other the one that turns right or down, leaves the run by a 45° bevel
/// from half a `step` before the parting to half a `step` along its own way (the size of the pilot's reference set); the
/// other keeps its square corner. A run or a way on shorter than a step keeps its corner. `pins`: each pin's point with the lines (indices into `lines`) that end at
/// it.
pub fn bevel_partings(lines: &mut [Vec<Pt2>], pins: &[(Pt2, Vec<usize>)], step: Nm) {
    for (p, members) in pins {
        if members.len() < 2 {
            continue;
        }
        // Each member's line from the pin outward, and whether it was turned for that.
        let out: Vec<Option<(Vec<Pt2>, bool)>> = members
            .iter()
            .map(|&k| {
                let l = lines.get(k)?;
                if l.first() == Some(p) {
                    Some((l.clone(), false))
                } else if l.last() == Some(p) {
                    Some((l.iter().rev().copied().collect(), true))
                } else {
                    None
                }
            })
            .collect();
        // Each member's parting with the member it shares the longest run with.
        let mut parts: Vec<Option<Parting>> = vec![None; members.len()];
        for i in 0..members.len() {
            for j in 0..members.len() {
                if i == j {
                    continue;
                }
                let (Some((a, _)), Some((b, _))) = (&out[i], &out[j]) else {
                    continue;
                };
                if let Some(x) = parting(a, b) {
                    let far = |q: &Parting| (q.0.x - p.x).0.abs() + (q.0.y - p.y).0.abs();
                    if parts[i].as_ref().is_none_or(|q| far(&x) > far(q)) {
                        parts[i] = Some(x);
                    }
                }
            }
        }
        // The way each member goes on from its parting: its next direction there.
        let way = |i: usize| -> Option<(usize, (i64, i64), Nm)> {
            let (line, _) = out[i].as_ref()?;
            let (x, d_in, _) = parts[i]?;
            let k = line.iter().position(|q| *q == x)?;
            let next = *line.get(k + 1)?;
            let d = unit(x, next)?;
            (d != (-d_in.0, -d_in.1)).then_some((k, d, Nm((next.x - x.x).0.abs() + (next.y - x.y).0.abs())))
        };
        for i in 0..members.len() {
            let Some((x, d_in, run)) = parts[i] else { continue };
            let Some((k, d_out, on)) = way(i) else { continue };
            if d_out == d_in || run < step || on < step {
                continue;
            }
            // The others parting at the same point: one going straight on gives the turn the bevel; else the turn right
            // or down takes it.
            let others: Vec<usize> = (0..members.len())
                .filter(|j| *j != i && parts[*j].is_some_and(|q| q.0 == x))
                .collect();
            if others.is_empty() {
                continue;
            }
            let straight = others.iter().any(|j| {
                let Some((line, _)) = out[*j].as_ref() else {
                    return false;
                };
                // A member straight through the parting has no vertex there.
                !line.contains(&x) || way(*j).is_some_and(|w| w.1 == d_in)
            });
            if !straight && !matches!(d_out, (1, 0) | (0, 1)) {
                continue;
            }
            let Some((mut line, turned)) = out[i].clone() else {
                continue;
            };
            let leg = step.0 / 2;
            let back = Pt2::new(x.x - Nm(d_in.0 * leg), x.y - Nm(d_in.1 * leg));
            let on_way = Pt2::new(x.x + Nm(d_out.0 * leg), x.y + Nm(d_out.1 * leg));
            line.splice(k..=k, [back, on_way]);
            line.dedup();
            if turned {
                line.reverse();
            }
            if let Some(l) = lines.get_mut(members[i]) {
                *l = line;
            }
        }
    }
}
