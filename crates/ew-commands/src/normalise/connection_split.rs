// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #7, connection split (doc 06 §3.2 and §5.3, FR-CON-008), and the split itself, which
//! `connection.draw` uses for an endpoint on an existing connection.

use std::collections::BTreeMap;

use ew_ids::Id;
use ew_model::{AnyKey, EndpointKey};
use ew_schema::{Connection, Endpoint, Extra, Junction, Kind, NumberSpec, Representation};
use ew_units::Pt2;

use crate::error::CmdError;
use crate::geometry::{locate, provisional_path_of, rep_cps, split_route};
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
            return tx.reject(text!("refusal-split-not-on-route", connection.to_string()));
        };
        let Some((_, r2)) = split_route(&rest, second_at) else {
            return tx.reject(text!("refusal-split-second-not-on-route", connection.to_string()));
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
/// drawn CPs at least two distinct positions lie on the path of one connection of its page (the connections
/// with a stored route in ID order, then those without one in ID order; connections already attached to its
/// functions are skipped), splits that connection: the part from `a` ends at the CP met first along the path,
/// the part to `b` starts at the CP met last. The representation then has connections, so a second run does
/// nothing.
///
/// A connection's path is the one the page draws it along (FT-1020, decision log ID-1104): its stored route, or,
/// without one (the wire of a `connect` hint, an import), the provisional path between its ends
/// ([`provisional_path_of`]). A connection without a route splits into two parts without one, which the page draws
/// between their own ends. A routed connection goes first because an imported page draws route-less wires on top of
/// one another (Golden A's folio 101), where a stored route is the line someone drew.
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
        // The provisional paths of the connections without a route, each computed once per run: the CPs of their
        // ends do not move here, and a split connection's entry is dropped (its ends changed).
        let mut provisional: BTreeMap<Id<Connection>, Option<Vec<Pt2>>> = BTreeMap::new();
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
            // The connections with a stored route first, then those without one: where a routed wire and the
            // provisional path of a route-less one are drawn on one line, the routed wire is split.
            'search: for routed in [true, false] {
                for &cid in &on_page {
                    let Some(c) = tx.get::<Connection>(cid) else {
                        continue;
                    };
                    if c.route.is_empty() == routed
                        || [&c.a, &c.b]
                            .iter()
                            .any(|e| e.function().is_some_and(|f| rep.functions.contains(&f)))
                    {
                        continue;
                    }
                    let path: &[Pt2] = if routed {
                        &c.route
                    } else {
                        match provisional
                            .entry(cid)
                            .or_insert_with(|| provisional_path_of(tx, &c))
                        {
                            Some(path) => path,
                            None => continue,
                        }
                    };
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
                    provisional.remove(&cid);
                    split_connection(
                        tx,
                        cid,
                        c1.pos,
                        Endpoint::Cp(c1.function, c1.idx),
                        c2.pos,
                        Endpoint::Cp(c2.function, c2.idx),
                    )?;
                    changed = true;
                    break 'search;
                }
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
