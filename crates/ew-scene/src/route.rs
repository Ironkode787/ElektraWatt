// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The polyline a connection is drawn along on its page.
//!
//! A connection's stored route is its drawing (ADR-0033): the scene never routes. A stored route holds
//! every vertex from end `a`'s point to end `b`'s (D2) and is drawn as stored ([`connection_polyline`]). A connection
//! without one is *unrouted* (D3): it is drawn as the straight line between its ends (the page draws it dashed in the
//! thinnest pen), and as nothing where its ends meet.
//!
//! Everything is integer nanometres ([`Nm`], [`Pt2`], [`Rect`]) on the page, origin top-left, y down, as in
//! [`symbol`](crate::symbol). The functions are pure, deterministic and integer-only, and the result never leaves the
//! page: every vertex is clamped into the page rectangle.
//!
//! # Junctions
//!
//! A connection whose end is a junction passes the junction position as that end with `dir: None`, so every
//! arm of a junction ends exactly at the junction point.

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
///   end `a`'s point to end `b`'s, both included, WP-S2.19 D2; equal neighbours merged, a stale end joined by one
///   straight segment), each point clamped into `page`, equal neighbours merged again. Collinear vertices are kept:
///   the model's route is authoritative and the user placed those vertices.
/// - `stored` empty: the unrouted line `[a.pos, b.pos]`, clamped into `page`; `[a.pos]` where the ends
///   meet (nothing is drawn).
pub fn connection_polyline(a: &RouteEnd, b: &RouteEnd, stored: &[Pt2], page: Rect) -> Vec<Pt2> {
    let mut pts: Vec<Pt2> = ew_derive::stored_path(a.pos, stored, b.pos)
        .into_iter()
        .map(|p| clamp(p, page))
        .collect();
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
