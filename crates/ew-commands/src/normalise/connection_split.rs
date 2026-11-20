// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #7, connection split (doc 06 §3.2 and §5.3, FR-CON-008), and the split itself, which
//! `connection.draw` uses for an endpoint on an existing connection.

use ew_ids::Id;
use ew_model::{AnyKey, EndpointKey};
use ew_schema::{Connection, Endpoint, Extra, Junction, Kind, NumberSpec, Page, Representation};
use ew_units::Pt2;

use crate::error::CmdError;
use crate::geometry::{locate, rep_cps, settled, split_route};
use crate::normalise::Normaliser;
use crate::text::text;
use crate::tx::TxBuilder;

/// Splits connection W (A → B) into W′ (A → `first_end`) and W″ (`second_end` → B) (doc 06 §5.3):
///
/// - W′ keeps W's ID, so its wire number (assigned or frozen) stays (FR-NUM-023); its route is W's route
///   up to `first_at`;
/// - W″ gets a new ID, W's kind, page and overrides, `NumberSpec::Unassigned` (online wire numbering is
///   off in v0) and W's route from `second_at`; it takes W's place in the sequence of a junction at B;
/// - bonds realised by W keep naming W′.
///
/// `first_at` and `second_at` must lie on W's route in that order (they may be equal, for a junction). A
/// connection without a route has no geometry to check: both parts get empty routes. Returns W″'s ID.
pub fn split_connection(
    tx: &mut TxBuilder<'_>,
    connection: Id<Connection>,
    first_at: Pt2,
    first_end: Endpoint,
    second_at: Pt2,
    second_end: Endpoint,
) -> Result<Id<Connection>, CmdError> {
    let w = tx.require::<Connection>(connection)?;
    let (r1, r2) = if w.route.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        let Some((r1, rest)) = split_route(&w.route, first_at) else {
            return tx.reject(text!(
                "refusal-split-not-on-route",
                crate::words::wire(tx, connection)
            ));
        };
        let Some((_, r2)) = split_route(&rest, second_at) else {
            return tx.reject(text!(
                "refusal-split-second-not-on-route",
                crate::words::wire(tx, connection)
            ));
        };
        (r1, r2)
    };
    let second_id: Id<Connection> = tx.mint();
    let second = Connection {
        id: second_id,
        v: 1,
        a: second_end,
        b: w.b.clone(),
        page: w.page,
        route: r2,
        kind: w.kind.clone(),
        overrides: w.overrides.clone(),
        number: NumberSpec::Unassigned,
        source_ref: None,
        computed: w.computed,
        extra: Extra::new(),
    };
    let mut first = (*w).clone();
    first.b = first_end;
    first.route = r1;
    tx.put(first);
    tx.insert(second)?;
    if let Endpoint::Junction(j) = w.b {
        tx.update::<Junction>(j, |j| {
            for c in &mut j.sequence {
                if *c == connection {
                    *c = second_id;
                }
            }
        })?;
    }
    Ok(second_id)
}

/// Normaliser #7.
///
/// A representation created by this transaction whose functions have no connection yet, and of whose
/// drawn CPs at least two distinct positions lie on the stored route of one connection of its page (in ID order;
/// connections already attached to its functions are skipped), splits that connection: the part from `a` ends at the CP met first along the path,
/// the part to `b` starts at the CP met last. The representation then has connections, so a second run does
/// nothing.
///
/// A connection's path is the one the page draws it along: its stored route. A
/// connection without one is unrouted and has no wire geometry: it is drawn as a dashed straight line
/// between its ends, and it is not split.
///
/// A pin lies on the route as a reader sees it: with a grid position read as one number
/// ([`settled`](crate::geometry::settled), ADR-0033 amendment 4), so a pin whose point lies 1 nm beside the number
/// of the wire's line is on it. The wire is then cut on its own line beside the pin, and the part that ends at the
/// pin takes one more vertex, the pin's own point. The route is read in that one reading throughout: a stretch whose
/// two vertices lie on one line to a reader but 1 nm apart across it (a stored diagonal of 1 nm, as an imported
/// conductor holds) runs on that line, with a step of 1 nm at a vertex beside it, before the wire is cut ([`squared`];
/// rc.5: an import was refused, its pins found on such a stretch and no point on the stretch to cut it at).
#[derive(Clone, Copy, Debug, Default)]
pub struct ConnectionSplit;

impl Normaliser for ConnectionSplit {
    fn id(&self) -> &'static str {
        "connection_split"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let mut changed = false;
        let new_reps: Vec<AnyKey> = tx
            .changed_of(Kind::Representation)
            .into_iter()
            .filter(|k| tx.is_new(k))
            .collect();
        for key in new_reps {
            let Some(rep) = tx.get::<Representation>(key.1.cast()) else {
                continue;
            };
            if rep
                .functions
                .iter()
                .any(|f| !tx.connections_of_function(*f).is_empty())
            {
                continue;
            }
            let cps = rep_cps(tx, &rep);
            if cps.len() < 2 {
                continue;
            }
            let on_page = tx.connections_on_page(rep.page);
            // A pin lies on a wire where it does to a reader: with a grid position as one number (a pin 1 nm beside
            // the number of the wire's line, ADR-0033 amendment 4, is on it).
            let grid = tx.get::<Page>(rep.page).map(|p| p.grid);
            let settle = |p: Pt2| grid.map_or(p, |g| settled(g, p));
            // Only a wire with a stored route has wire geometry: an unrouted connection is not split.
            for &cid in &on_page {
                let Some(c) = tx.get::<Connection>(cid) else {
                    continue;
                };
                if c.route.is_empty()
                    || [&c.a, &c.b]
                        .iter()
                        .any(|e| e.function().is_some_and(|f| rep.functions.contains(&f)))
                {
                    continue;
                }
                let route = squared(&c.route, settle);
                let path: Vec<Pt2> = route.iter().copied().map(settle).collect();
                let mut on: Vec<((usize, i128), usize)> = cps
                    .iter()
                    .enumerate()
                    .filter_map(|(i, cp)| locate(&path, settle(cp.pos)).map(|at| (at, i)))
                    .collect();
                on.sort();
                let (Some(&(first_at, i1)), Some(&(last_at, i2))) = (on.first(), on.last()) else {
                    continue;
                };
                if first_at == last_at || settle(cps[i1].pos) == settle(cps[i2].pos) {
                    continue;
                }
                let (c1, c2) = (cps[i1], cps[i2]);
                // The wire is cut on its own line, at the point beside each pin: the pin's coordinate along the
                // stored segment it lies on, the segment's own number across it.
                let beside = |seg: usize, p: Pt2| match (route.get(seg), route.get(seg + 1)) {
                    (Some(u), Some(v)) if u.x == v.x && u.y != v.y => {
                        Pt2::new(u.x, p.y.max(u.y.min(v.y)).min(u.y.max(v.y)))
                    }
                    (Some(u), Some(v)) if u.y == v.y && u.x != v.x => {
                        Pt2::new(p.x.max(u.x.min(v.x)).min(u.x.max(v.x)), u.y)
                    }
                    _ => p,
                };
                let (p1, p2) = (beside(first_at.0, c1.pos), beside(last_at.0, c2.pos));
                if route != c.route {
                    tx.update::<Connection>(cid, |w| w.route.clone_from(&route))?;
                }
                let second = split_connection(
                    tx,
                    cid,
                    p1,
                    Endpoint::Cp(c1.function, c1.idx),
                    p2,
                    Endpoint::Cp(c2.function, c2.idx),
                )?;
                // Each part ends on its pin's own number: a last step of 1 nm where the pin stands beside the line.
                if p1 != c1.pos {
                    tx.update::<Connection>(cid, |w| w.route.push(c1.pos))?;
                }
                if p2 != c2.pos {
                    tx.update::<Connection>(second, |w| w.route.insert(0, c2.pos))?;
                }
                changed = true;
                break;
            }
        }
        Ok(changed)
    }
}

/// `route` read with a grid position as one number (`settle`): each stretch whose two vertices settle onto one
/// horizontal or vertical line while they differ across it runs on that line, with a step to each vertex that lies
/// beside it; every other stretch is kept as stored.
fn squared(route: &[Pt2], settle: impl Fn(Pt2) -> Pt2) -> Vec<Pt2> {
    let mut out: Vec<Pt2> = Vec::with_capacity(route.len());
    for (k, &v) in route.iter().enumerate() {
        if let Some(&u) = k.checked_sub(1).and_then(|i| route.get(i))
            && u.x != v.x
            && u.y != v.y
        {
            let (su, sv) = (settle(u), settle(v));
            let line = if su.x == sv.x {
                Some((Pt2::new(su.x, u.y), Pt2::new(su.x, v.y)))
            } else if su.y == sv.y {
                Some((Pt2::new(u.x, su.y), Pt2::new(v.x, su.y)))
            } else {
                None
            };
            // The line's own points, but where a vertex lies on it already (no step there).
            if let Some((a, b)) = line {
                if a != u {
                    out.push(a);
                }
                if b != v {
                    out.push(b);
                }
            }
        }
        out.push(v);
    }
    out
}

/// Whether two endpoints are the same.
pub(crate) fn same_endpoint(a: &Endpoint, b: &Endpoint) -> bool {
    match (EndpointKey::of(a), EndpointKey::of(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}
