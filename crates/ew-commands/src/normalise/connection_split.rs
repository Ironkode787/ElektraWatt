// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #7, connection split (doc 06 §3.2 and §5.3, FR-CON-008), and the split itself, which
//! `connection.draw` uses for an endpoint on an existing connection.

use ew_ids::Id;
use ew_model::{AnyKey, EndpointKey};
use ew_schema::{Connection, Endpoint, Extra, Junction, Kind, NumberSpec, Representation};
use ew_units::Pt2;

use crate::error::CmdError;
use crate::geometry::{locate, rep_cps, split_route};
use crate::normalise::Normaliser;
use crate::text::text;
use crate::tx::TxBuilder;

/// Splits connection W (A → B) into W′ (A → `first_end`) and W″ (`second_end` → B) (doc 06 §5.3):
///
/// - the kept part keeps W's ID, so its wire number (assigned or frozen) stays (FR-NUM-023): W′, or W″ with
///   `keep_second`; W′'s route is W's route up to `first_at`, W″'s from `second_at`;
/// - the other part gets a new ID, W's kind, page and overrides and `NumberSpec::Unassigned` (online wire numbering
///   is off in v0); it takes W's place in the sequence of a junction at its far end;
/// - bonds realised by W keep naming the kept part.
///
/// `first_at` and `second_at` must lie on W's route in that order (they may be equal, for a junction). A
/// connection without a route has no geometry to check: both parts get empty routes. Returns the new part's ID.
pub fn split_connection(
    tx: &mut TxBuilder<'_>,
    connection: Id<Connection>,
    first_at: Pt2,
    first_end: Endpoint,
    second_at: Pt2,
    second_end: Endpoint,
    keep_second: bool,
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
    let new_id: Id<Connection> = tx.mint();
    let new_part = |a: Endpoint, b: Endpoint, route: Vec<Pt2>| Connection {
        id: new_id,
        v: 1,
        a,
        b,
        page: w.page,
        route,
        kind: w.kind.clone(),
        overrides: w.overrides.clone(),
        number: NumberSpec::Unassigned,
        source_ref: None,
        computed: w.computed,
        extra: Extra::new(),
    };
    let mut kept = (*w).clone();
    let (new, far) = if keep_second {
        kept.a = second_end;
        kept.route = r2;
        (new_part(w.a.clone(), first_end, r1), &w.a)
    } else {
        kept.b = first_end;
        kept.route = r1;
        (new_part(second_end, w.b.clone(), r2), &w.b)
    };
    tx.put(kept);
    tx.insert(new)?;
    if let Endpoint::Junction(j) = far {
        tx.update::<Junction>(*j, |j| {
            for c in &mut j.sequence {
                if *c == connection {
                    *c = new_id;
                }
            }
        })?;
    }
    Ok(new_id)
}

/// Whether the From end of the physical wire that `connection` is part of (doc 08 §3.8: the end its number is
/// counted from) lies beyond the connection's `b` end, in the snapshot before the transaction. A device put onto
/// the wire then leaves the wire's record on the part to `b`, so that its number stays where the page printed it
/// (FR-CON-008). `false` where it lies beyond `a` or cannot be told (a connection new in this transaction).
fn from_end_beyond_b(tx: &TxBuilder<'_>, connection: Id<Connection>) -> bool {
    let q = ew_derive::Q::new(tx.base_model());
    let Some(w) = q.connections().get(connection) else {
        return false;
    };
    let partition = q.get::<ew_derive::PotentialPartition>(());
    let Some(potential) = EndpointKey::of(&w.a).and_then(|k| partition.potential_at(k)) else {
        return false;
    };
    let wires = q.get::<ew_derive::PhysicalWiresOf>(potential.id);
    // A star's feed arm is part of every branch's wire: the wire it carries decides, else the first.
    let Some(wire) = wires
        .wires
        .iter()
        .filter(|p| p.constituents.contains(&connection))
        .min_by_key(|p| p.id != connection)
    else {
        return false;
    };
    let Some(k) = wire.constituents.iter().position(|c| *c == connection) else {
        return false;
    };
    // Whether an end of W meets the wire's side toward `ends[i]`: that end itself or a constituent on that side.
    let meets = |e: &Endpoint, i: usize| {
        let side = if i == 0 { &wire.constituents[..k] } else { &wire.constituents[k + 1..] };
        let end = EndpointKey::Cp(wire.ends[i].cp.function, wire.ends[i].cp.idx);
        EndpointKey::of(e).is_some_and(|key| {
            key == end
                || side.iter().filter_map(|c| q.connections().get(*c)).any(|c| {
                    EndpointKey::of(&c.a) == Some(key) || EndpointKey::of(&c.b) == Some(key)
                })
        })
    };
    let a_toward_first = if meets(&w.a, 0) || meets(&w.b, 1) {
        true
    } else if meets(&w.b, 0) || meets(&w.a, 1) {
        false
    } else {
        return false;
    };
    let mut oriented = wire.clone();
    let from_is_last = ew_derive::orient_wire(&q, &mut oriented);
    a_toward_first == from_is_last
}

/// Normaliser #7.
///
/// A representation created by this transaction whose functions have no connection yet, and of whose
/// drawn CPs at least two distinct positions lie on the stored route of one connection of its page (in ID order;
/// connections already attached to its functions are skipped), splits that connection: the part from `a` ends at the CP met first along the path,
/// the part to `b` starts at the CP met last, and the part toward the wire's From end keeps the connection's record
/// (FR-CON-008). The representation then has connections, so a second run does nothing.
///
/// A connection's path is the one the page draws it along: its stored route. A
/// connection without one is unrouted and has no wire geometry: it is drawn as a dashed straight line
/// between its ends, and it is not split. A pin's point and a wire's vertex on one grid position are one number
/// (docs/design/06a-drafting.md §2.1), so the comparisons are exact.
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
                let path: &[Pt2] = &c.route;
                let mut on: Vec<((usize, i128), usize)> = cps
                    .iter()
                    .enumerate()
                    .filter_map(|(i, cp)| locate(path, cp.pos).map(|at| (at, i)))
                    .collect();
                on.sort();
                let (Some(&(first_at, i1)), Some(&(last_at, i2))) = (on.first(), on.last()) else {
                    continue;
                };
                if first_at == last_at || cps[i1].pos == cps[i2].pos {
                    continue;
                }
                let (c1, c2) = (cps[i1], cps[i2]);
                // Two wires of one pin that share their run (docs/design/06a-drafting.md §3.4) are no place for a part:
                // cut, the other would run on through its pins.
                let shared = on_page.iter().any(|o| {
                    *o != cid
                        && tx.get::<Connection>(*o).is_some_and(|w| {
                            locate(&w.route, c1.pos).is_some() && locate(&w.route, c2.pos).is_some()
                        })
                });
                if shared {
                    return tx.reject(text!("refusal-split-shared-run"));
                }
                let keep_second = from_end_beyond_b(tx, cid);
                split_connection(
                    tx,
                    cid,
                    c1.pos,
                    Endpoint::Cp(c1.function, c1.idx),
                    c2.pos,
                    Endpoint::Cp(c2.function, c2.idx),
                    keep_second,
                )?;
                changed = true;
                break;
            }
        }
        Ok(changed)
    }
}

/// Whether two endpoints are the same.
pub(crate) fn same_endpoint(a: &Endpoint, b: &Endpoint) -> bool {
    match (EndpointKey::of(a), EndpointKey::of(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}
