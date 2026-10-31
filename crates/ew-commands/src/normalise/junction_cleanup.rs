// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #8, junction clean-up (doc 06 §3.2, doc 08 §3.1 CI-04).
//!
//! The candidates are the junctions this transaction touched: a changed junction row, or a junction that a
//! changed connection ends at (old or new row). They form a **worklist** inside one run (F2): each step
//! takes the smallest junction ID left, and the junctions at the far ends of every connection it removes
//! or merges are queued again, so a cascade (J1 → J3 → J4) is cleaned up completely in one run and a
//! second run changes nothing (the `Normaliser` idempotency contract). For each existing junction taken,
//! by the number of connections ending at it:
//!
//! - **two**: the junction dissolves. The kept member keeps its ID, its number and its overrides, and takes
//!   the other member's far endpoint and route; the other member is removed (and replaced by the kept one
//!   in the sequence of a junction at its far end and in the wire markers that list it). The kept member is
//!   the one whose number is not `Unassigned`, so an assigned or frozen number is never dropped silently; if
//!   both carry the same number, or neither carries one, it is the member listed first in the junction's
//!   sequence (the feed arm). Two different numbers cannot be merged: the junction is left untouched and the
//!   command is rejected, naming both numbers (the user clears one or deletes the connections separately).
//!   If both far ends are the same endpoint, the merged connection would loop: both members are removed
//!   instead;
//! - **one**: the junction is removed together with that connection;
//! - **none**: the junction is removed.
//!
//! Potential definitions and bonds at a removed junction are dropped, and bonds realised by a removed
//! connection lose their `via`, as the delete policies prescribe (doc 03 §4.2). The removals are written
//! directly rather than through `Model::delete_plan`, because the transaction is in a transient state
//! here; `TxBuilder::remove_row` takes every removed record out of the macro instances that list it (the unlink of
//! `macro_instance.members[][1]`). Wire-number transfer to the kept member is normaliser #11 (not in v0).

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_model::{AnyRow, EndpointKey};
use ew_schema::{Bond, Connection, Endpoint, Junction, Kind, NumberSpec};
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

/// Removes a connection: drops it from the sequence of junctions at its ends, clears `via` of bonds realised
/// by it, and takes it out of the wire markers that list it: with `carrier` (a dissolving junction's kept
/// member, which carries the wire on) a marker lists the carrier in its place; a marker left empty is removed
/// (EW-INV-023).
fn remove_connection(
    tx: &mut TxBuilder<'_>,
    id: Id<Connection>,
    carrier: Option<Id<Connection>>,
) -> Result<(), CmdError> {
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
    tx.unlink_markers(id, carrier, None);
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

/// The junctions at the ends of a connection (as it is now), for the worklist.
fn junctions_of(tx: &TxBuilder<'_>, id: Id<Connection>) -> Vec<Id<Junction>> {
    tx.get::<Connection>(id).map_or_else(Vec::new, |c| {
        [&c.a, &c.b]
            .into_iter()
            .filter_map(|e| match e {
                Endpoint::Junction(j) => Some(*j),
                _ => None,
            })
            .collect()
    })
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

/// Orders the two members of a dissolving junction as (keep, gone): the one with a number is kept; two
/// different numbers are rejected.
fn prefer_numbered(
    tx: &TxBuilder<'_>,
    j: Id<Junction>,
    first: Id<Connection>,
    second: Id<Connection>,
) -> Result<(Id<Connection>, Id<Connection>), CmdError> {
    let (c1, c2) = (
        tx.require::<Connection>(first)?,
        tx.require::<Connection>(second)?,
    );
    if same_endpoint(&away_from(&c1, j).0, &away_from(&c2, j).0) {
        // The loop case removes both members: no number is carried over.
        return Ok((first, second));
    }
    let (n1, n2) = (c1.number.clone(), c2.number.clone());
    let unassigned = |n: &NumberSpec| matches!(n, NumberSpec::Unassigned);
    if !unassigned(&n1) && !unassigned(&n2) && n1 != n2 {
        return tx.reject(format!(
            "{} would merge a wire {} and a wire {}, which carry different numbers; clear one of the \
             numbers or delete the wires separately",
            crate::words::junction(tx, j),
            number_words(&n1),
            number_words(&n2)
        ));
    }
    if unassigned(&n1) && !unassigned(&n2) {
        Ok((second, first))
    } else {
        Ok((first, second))
    }
}

/// A wire's number in words for a refusal (FT-779): `numbered "101"` for a frozen number, `with assigned number 5`
/// for an assigned one (its counter: a wire's printed number is derived and two members of one physical wire print
/// the same), so two different numbers never read the same; or how it is numbered.
fn number_words(n: &NumberSpec) -> String {
    match n {
        NumberSpec::Frozen(i) => format!("numbered \"{i}\""),
        NumberSpec::Assigned(a) => format!("with assigned number {}", a.counter),
        NumberSpec::Unassigned => "without a number".to_owned(),
        NumberSpec::Unknown(_) => "with a number of a newer format".to_owned(),
    }
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
        remove_connection(tx, keep, None)?;
        remove_connection(tx, gone, None)?;
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
    remove_connection(tx, gone, Some(keep))?;
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
        let mut work = candidates(tx);
        while let Some(j) = work.pop_first() {
            let Some(junction) = tx.get::<Junction>(j) else {
                continue;
            };
            let members = tx.connections_at(EndpointKey::Junction(j));
            // The far ends of the members are re-examined after this step: a member removed or merged
            // here changes the connection count there.
            let requeue: BTreeSet<Id<Junction>> = members
                .iter()
                .flat_map(|c| junctions_of(tx, *c))
                .filter(|far| *far != j)
                .collect();
            match members.as_slice() {
                [] => {
                    remove_junction(tx, j);
                    changed = true;
                }
                [only] => {
                    remove_connection(tx, *only, None)?;
                    remove_junction(tx, j);
                    work.extend(requeue);
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
                    let (first, second) = prefer_numbered(tx, j, first, second)?;
                    dissolve(tx, j, first, second)?;
                    work.extend(requeue);
                    changed = true;
                }
                _ => {}
            }
        }
        Ok(changed)
    }
}
