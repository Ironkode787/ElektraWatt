// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #8, junction clean-up (doc 06 §3.2, doc 08 §3.1 CI-04).
//!
//! The candidates are the junctions this transaction touched: a changed junction row, or a junction that a
//! changed connection ends at (old or new row). For each existing candidate in ID order, by the number of
//! connections ending at it:
//!
//! - **two**: the junction dissolves. The member listed first in its sequence (the feed arm) keeps its ID,
//!   its number and its overrides, and takes the other member's far endpoint and route; the other member
//!   is removed (and replaced by the kept one in the sequence of a junction at its far end). If both far
//!   ends are the same endpoint, the merged connection would loop: both members are removed instead;
//! - **one**: the junction is removed together with that connection;
//! - **none**: the junction is removed.
//!
//! Potential definitions and bonds at a removed junction are dropped, and bonds realised by a removed
//! connection lose their `via`, as the delete policies prescribe (doc 03 §4.2). The removals are written
//! directly rather than through `Model::delete_plan`, because the transaction is in a transient state
//! here. Wire-number transfer to the kept member is normaliser #11 (not in v0).

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_model::{AnyRow, EndpointKey};
use ew_schema::{Bond, Connection, Endpoint, Junction, Kind};
use ew_units::Pt2;

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::normalise::connection_split::same_endpoint;
use crate::tx::TxBuilder;

/// Normaliser #8.
#[derive(Clone, Copy, Debug, Default)]
pub struct JunctionCleanup;

fn candidates(tx: &TxBuilder<'_>) -> BTreeSet<Id<Junction>> {
    let mut out = BTreeSet::new();
    for key in tx.changed_keys() {
        match key.0 {
            Kind::Junction => {
                out.insert(key.1.cast());
            }
            Kind::Connection => {
                for row in [tx.row(&key), tx.base_row(&key)].into_iter().flatten() {
                    if let AnyRow::Connection(c) = row {
                        for e in [&c.a, &c.b] {
                            if let Endpoint::Junction(j) = e {
                                out.insert(*j);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Removes a connection: drops it from the sequence of junctions at its ends and clears `via` of bonds
/// realised by it.
fn remove_connection(tx: &mut TxBuilder<'_>, id: Id<Connection>) -> Result<(), CmdError> {
    let Some(c) = tx.get::<Connection>(id) else {
        return Ok(());
    };
    for e in [&c.a, &c.b] {
        if let Endpoint::Junction(j) = e {
            if tx.exists::<Junction>(*j) {
                tx.update::<Junction>(*j, |j| j.sequence.retain(|m| *m != id))?;
            }
        }
    }
    for b in tx.iter::<Bond>() {
        if b.via == Some(id) {
            tx.update::<Bond>(b.id, |b| b.via = None)?;
        }
    }
    tx.remove_row((Kind::Connection, id.cast()));
    Ok(())
}

/// Removes a junction with the definitions and bonds at it.
fn remove_junction(tx: &mut TxBuilder<'_>, id: Id<Junction>) {
    let at = EndpointKey::Junction(id);
    for d in tx.potential_defs_at(at) {
        tx.remove_row((Kind::PotentialDef, d.cast()));
    }
    for b in tx.bonds_at(at) {
        tx.remove_row((Kind::Bond, b.cast()));
    }
    tx.remove_row((Kind::Junction, id.cast()));
}

/// The far endpoint of a connection seen from a junction, and its route oriented from the junction.
fn away_from(c: &Connection, j: Id<Junction>) -> (Endpoint, Vec<Pt2>) {
    if c.a == Endpoint::Junction(j) {
        (c.b.clone(), c.route.clone())
    } else {
        let mut r = c.route.clone();
        r.reverse();
        (c.a.clone(), r)
    }
}

/// Concatenates two routes that meet at a common vertex, without repeating it.
fn join(mut first: Vec<Pt2>, second: Vec<Pt2>) -> Vec<Pt2> {
    let skip = usize::from(!first.is_empty() && first.last() == second.first());
    first.extend(second.into_iter().skip(skip));
    first
}

fn dissolve(
    tx: &mut TxBuilder<'_>,
    j: Id<Junction>,
    keep: Id<Connection>,
    gone: Id<Connection>,
) -> Result<(), CmdError> {
    let kept = tx.require::<Connection>(keep)?;
    let other = tx.require::<Connection>(gone)?;
    let (far_other, route_other) = away_from(&other, j);
    let (far_kept, _) = away_from(&kept, j);
    if same_endpoint(&far_kept, &far_other) {
        remove_connection(tx, keep)?;
        remove_connection(tx, gone)?;
        remove_junction(tx, j);
        return Ok(());
    }
    let mut merged = (*kept).clone();
    if kept.b == Endpoint::Junction(j) {
        merged.b = far_other.clone();
        merged.route = join(kept.route.clone(), route_other);
    } else {
        merged.a = far_other.clone();
        let mut towards = route_other;
        towards.reverse();
        merged.route = join(towards, kept.route.clone());
    }
    remove_connection(tx, gone)?;
    tx.put(merged);
    if let Endpoint::Junction(far) = far_other {
        tx.update::<Junction>(far, |f| {
            if !f.sequence.contains(&keep) {
                f.sequence.push(keep);
            }
        })?;
    }
    remove_junction(tx, j);
    Ok(())
}

impl Normaliser for JunctionCleanup {
    fn id(&self) -> &'static str {
        "junction_cleanup"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let mut changed = false;
        for j in candidates(tx) {
            let Some(junction) = tx.get::<Junction>(j) else {
                continue;
            };
            let members = tx.connections_at(EndpointKey::Junction(j));
            match members.as_slice() {
                [] => {
                    remove_junction(tx, j);
                    changed = true;
                }
                [only] => {
                    remove_connection(tx, *only)?;
                    remove_junction(tx, j);
                    changed = true;
                }
                [x, y] => {
                    let first = junction
                        .sequence
                        .iter()
                        .find(|c| *c == x || *c == y)
                        .copied()
                        .unwrap_or(*x);
                    let second = if first == *x { *y } else { *x };
                    dissolve(tx, j, first, second)?;
                    changed = true;
                }
                _ => {}
            }
        }
        Ok(changed)
    }
}
