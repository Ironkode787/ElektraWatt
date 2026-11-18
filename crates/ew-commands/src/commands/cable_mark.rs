// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A cable's mark placed with its cores (rc.3): when drawn wires become a cable's cores in one command (`cable.create`
//! and `conductor.add` with `wires`, the Cable form), each page they lie on gets the cable's mark at once, a wire
//! marker listing them that the page draws as the cable's mark (`ew-scene`, "Cable marks"). A bus line that a cable
//! carries (`bus.cable.set`, rc.5) gets the mark the same way, listed as a core is.
//!
//! - **A page with a mark of the cable** already (a marker listing one of its cores) takes the new cores into that
//!   mark's list; a core some marker lists already stays where it is.
//! - **Else the place** ([`placement`]): from the stored routes of the cores on the page (a connection without a route
//!   is left out), for a horizontal line across their vertical segments and for a vertical line across their
//!   horizontal ones, the stretch between two consecutive segment ends that the most cores run through side by side;
//!   the most cores first, then a stretch of at least [`ROOM`], then the horizontal line (across vertical wires, as
//!   panel builders mostly draw it), then the longer stretch. The mark stands in that stretch's middle, its cores
//!   listed in the order the line crosses them. No stretch any core runs through: no mark.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_schema::{
    Conductor, Connection, ConnectionKind, Device, Extra, MarkerField, Page, PageItem, PageItemKind,
    WireMarker,
};
use ew_units::{Angle2, Nm, Pt2};

use crate::error::CmdError;
use crate::tx::TxBuilder;

/// The shortest stretch a mark prefers, so that its strokes and core names have room: 6 mm.
const ROOM: i64 = 6_000_000;

/// Places or extends the marks of `cable` for the connections `wires` that just became its cores (module
/// documentation).
pub(crate) fn mark_cores(
    tx: &mut TxBuilder<'_>,
    cable: Id<Device>,
    wires: &[Id<Connection>],
) -> Result<(), CmdError> {
    let mut by_page: BTreeMap<Id<Page>, Vec<Id<Connection>>> = BTreeMap::new();
    for w in wires {
        if let Some(page) = tx.get::<Connection>(*w).and_then(|c| c.page)
            && tx.markers_of_connection(*w).is_empty()
        {
            by_page.entry(page).or_default().push(*w);
        }
    }
    let own: BTreeSet<Id<Conductor>> = tx.conductors_of_cable(cable).into_iter().collect();
    for (page, cores) in by_page {
        let mark = tx.page_items_on_page(page).into_iter().find(|i| {
            tx.get::<PageItem>(*i)
                .and_then(|item| item.kind.wire_marker().cloned())
                .is_some_and(|m| {
                    m.connections.iter().any(|c| {
                        tx.get::<Connection>(*c).is_some_and(|c| match &c.kind {
                            ConnectionKind::Core(k) => own.contains(k),
                            ConnectionKind::Bus(line) => line.cable == Some(cable),
                            _ => false,
                        })
                    })
                })
        });
        if let Some(mark) = mark {
            tx.update::<PageItem>(mark, |item| {
                if let PageItemKind::WireMarker(m) = &mut item.kind {
                    m.connections.extend(cores.iter().copied());
                }
            })?;
            continue;
        }
        let routes: Vec<(Id<Connection>, Vec<Pt2>)> = cores
            .iter()
            .filter_map(|c| Some((*c, tx.get::<Connection>(*c)?.route.clone())))
            .collect();
        let Some((at, angle, connections)) = placement(&routes) else {
            continue;
        };
        let id = tx.mint::<PageItem>();
        tx.insert(PageItem {
            id,
            v: 1,
            page,
            layer: None,
            kind: PageItemKind::WireMarker(WireMarker {
                connections,
                at,
                angle,
                fields: vec![MarkerField::Csa, MarkerField::Colour],
                tag_at: None,
                extra: Extra::new(),
            }),
            extra: Extra::new(),
        })?;
    }
    Ok(())
}

/// The marks follow the connections `connections` whose kind this transaction set (rc.3, `fields.set` and
/// `fields.set_many`: Connect cores…, the Cable form on a cable that exists, Properties): a connection that left a cable
/// leaves that cable's marks (a wire marker all of whose connections were cores of that cable), and a mark with no
/// connection left goes; a connection that became a cable's core gets the cable's mark on its page, or joins the one
/// there ([`mark_cores`]). A core that moves to another core of the same cable keeps its mark.
pub(crate) fn follow_kinds(tx: &mut TxBuilder<'_>, connections: &[Id<Connection>]) -> Result<(), CmdError> {
    let cable_of = |tx: &TxBuilder<'_>, kind: &ConnectionKind| match kind {
        ConnectionKind::Core(k) => tx.get::<Conductor>(*k).map(|c| c.cable),
        ConnectionKind::Bus(line) => line.cable,
        _ => None,
    };
    let base_kind = |tx: &TxBuilder<'_>, c: Id<Connection>| {
        tx.base_row(&(ew_schema::Kind::Connection, c.cast()))
            .and_then(|r| r.get::<Connection>().map(|c| c.kind.clone()))
    };
    let mut joined: BTreeMap<Id<Device>, Vec<Id<Connection>>> = BTreeMap::new();
    for c in connections {
        let before = base_kind(tx, *c).and_then(|k| cable_of(tx, &k));
        let now = tx.get::<Connection>(*c).and_then(|x| cable_of(tx, &x.kind));
        if before == now {
            continue;
        }
        if let Some(left) = before {
            for mark in tx.markers_of_connection(*c) {
                let Some(m) = tx
                    .get::<PageItem>(mark)
                    .and_then(|i| i.kind.wire_marker().cloned())
                else {
                    continue;
                };
                let of_cable = m
                    .connections
                    .iter()
                    .all(|x| base_kind(tx, *x).and_then(|k| cable_of(tx, &k)) == Some(left));
                if !of_cable {
                    continue;
                }
                if m.connections.iter().all(|x| x == c) {
                    tx.delete_many(&[(ew_schema::Kind::PageItem, mark.cast())])?;
                } else {
                    tx.update::<PageItem>(mark, |item| {
                        if let PageItemKind::WireMarker(m) = &mut item.kind {
                            m.connections.retain(|x| x != c);
                        }
                    })?;
                }
            }
        }
        if let Some(cable) = now {
            joined.entry(cable).or_default().push(*c);
        }
    }
    for (cable, wires) in joined {
        mark_cores(tx, cable, &wires)?;
    }
    Ok(())
}

/// A mark's place: its point, its angle and the connections it crosses in the line's order.
type Placement = (Pt2, Angle2, Vec<Id<Connection>>);

/// The place of a mark over the routed connections `routes` (module documentation): its point, its angle (0: a
/// horizontal line across vertical wires; 90000: a vertical line across horizontal ones) and the connections it
/// crosses in the line's order; `None` when no connection has a segment for the line to cross.
pub(crate) fn placement(routes: &[(Id<Connection>, Vec<Pt2>)]) -> Option<Placement> {
    // (count, roomy, horizontal line, length) ranks a stretch; the best wins, the first of equal ones.
    let mut best: Option<((usize, bool, bool, i64), Placement)> = None;
    for horizontal_line in [true, false] {
        // Per connection its segments the line would cross: (position across the line, from, to along the wire).
        let segments: Vec<(Id<Connection>, i64, i64, i64)> = routes
            .iter()
            .flat_map(|(id, pts)| {
                pts.windows(2).filter_map(move |w| {
                    let (a, b) = (w[0], w[1]);
                    let (across, from, to) = if horizontal_line {
                        (a.x == b.x).then_some((a.x.0, a.y.0, b.y.0))?
                    } else {
                        (a.y == b.y).then_some((a.y.0, a.x.0, b.x.0))?
                    };
                    (from != to).then_some((*id, across, from.min(to), from.max(to)))
                })
            })
            .collect();
        let mut ends: Vec<i64> = segments.iter().flat_map(|s| [s.2, s.3]).collect();
        ends.sort_unstable();
        ends.dedup();
        for w in ends.windows(2) {
            let (lo, hi) = (w[0], w[1]);
            // The connections running through the whole stretch, each once, by where the line crosses them.
            let mut crossed: Vec<(i64, Id<Connection>)> = Vec::new();
            for (id, across, from, to) in &segments {
                if *from <= lo && hi <= *to && !crossed.iter().any(|(_, c)| c == id) {
                    crossed.push((*across, *id));
                }
            }
            if crossed.is_empty() {
                continue;
            }
            crossed.sort();
            let rank = (crossed.len(), hi - lo >= ROOM, horizontal_line, hi - lo);
            if best.as_ref().is_some_and(|b| b.0 >= rank) {
                continue;
            }
            let mid = lo + (hi - lo) / 2;
            let first = crossed.first().map_or(0, |c| c.0);
            let last = crossed.last().map_or(0, |c| c.0);
            let centre = first + (last - first) / 2;
            let (at, angle) = if horizontal_line {
                (Pt2::new(Nm(centre), Nm(mid)), Angle2::ZERO)
            } else {
                (Pt2::new(Nm(mid), Nm(centre)), Angle2(90_000))
            };
            best = Some((rank, (at, angle, crossed.into_iter().map(|c| c.1).collect())));
        }
    }
    best.map(|(_, place)| place)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x * 1_000_000), Nm(y * 1_000_000))
    }

    /// Three supply wires that come in from the left and turn down side by side: the line crosses the vertical runs
    /// where all three run, horizontally, in the middle of that stretch, left to right.
    #[test]
    fn three_wires_turning_down_get_a_horizontal_line_where_all_run() {
        let id = |n: u128| Id::from_uuid(ew_ids::Uuid::from_u128(n));
        let (a, b, c) = (id(1), id(2), id(3));
        let routes = vec![
            (a, vec![p(16, 12), p(88, 12), p(88, 36)]),
            (b, vec![p(16, 16), p(80, 16), p(80, 36)]),
            (c, vec![p(16, 20), p(72, 20), p(72, 36)]),
        ];
        let (at, angle, order) = placement(&routes).unwrap();
        // Horizontally the three run side by side from 16 to 72 mm (56 mm), vertically from 20 to 36 mm (16 mm):
        // both roomy, the horizontal line wins.
        assert_eq!(angle, Angle2::ZERO);
        assert_eq!(at, p(80, 28));
        assert_eq!(order, [c, b, a]);
    }

    /// Wires that only run horizontally get a vertical line; a wire without a route is left out; nothing to cross
    /// gives no mark.
    #[test]
    fn horizontal_wires_get_a_vertical_line_and_unrouted_ones_none() {
        let id = |n: u128| Id::from_uuid(ew_ids::Uuid::from_u128(n));
        let (a, b, c) = (id(1), id(2), id(3));
        let routes = vec![
            (a, vec![p(10, 40), p(50, 40)]),
            (b, vec![p(10, 44), p(50, 44)]),
            (c, Vec::new()),
        ];
        let (at, angle, order) = placement(&routes).unwrap();
        assert_eq!(angle, Angle2(90_000));
        assert_eq!(at, p(30, 42));
        assert_eq!(order, [a, b]);
        assert_eq!(placement(&[(c, Vec::new())]), None);
    }
}
