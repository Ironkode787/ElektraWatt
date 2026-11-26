// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #8a, a wire's own values follow the wire (doc 13 §2.3).
//!
//! A wire is derived: the connections between two clamps, some of them shared with other wires (a star's feed arm, a
//! chain's arm). What a person set for one wire (`WireOverrides`: its length, colour, size, flags, the shown
//! annotation, a member a newer format added, and each end set by hand) is stored on its connections. Every command
//! that changes the wiring can strand such a value on a stretch that is now shared, lose it with a removed connection
//! or leave an end with the wrong clamp; this one normaliser puts every value back with its wire, from the state before
//! the command and the state after it, whatever the command was.
//!
//! It runs when the transaction changed the wiring (a connection made, removed, or given other ends or another kind; a
//! junction; an interruption point), over the potentials that holds, after `connection_split` and `junction_cleanup`,
//! whose results it reads. Each wire after the command wants ([`want`]):
//!
//! - a wire with the same two clamps as a wire before (`ew_derive::wire_changes`): that wire's values, each end the
//!   one of its clamp;
//! - a wire whose clamps changed (a device placed on a wire cuts it in two, a deleted arm joins two wires, a junction
//!   turned from a chain into a star): what divides of the wires it is made from (the wires before that lost a clamp
//!   it ends at; the one whose ID it keeps first, the others filling what that one leaves unset), each end from the
//!   wire that ended at its clamp, and no typed length: nobody measured it;
//! - any other wire (new): what the command set on its own connections.
//!
//! Then each value is stored where the wire reads it ([`store`]): a member on a connection several wires share stays
//! only where every wire through it has that value; a wire's other values stand once, on its first own connection
//! (`PhysicalWire::own`), where a single own connection already holding the value keeps it; an end set by hand stands
//! once, in the slot of a connection of the wire's own that leads to its clamp, and never on a shared connection. A wire
//! left without a connection of its own (the middle wire of a chain of three at one junction) keeps no value of its
//! own: the Connections table shows its row read-only. A second run finds every value in place and changes nothing.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::{
    CpRef, EndKind, PhysicalWire, PhysicalWiresOf, PotentialPartition, Q, end_through, hand_ends, members,
    wire_changes, wire_values, with_members,
};
use ew_ids::Id;
use ew_model::{AnyRow, EndpointKey};
use ew_schema::{Connection, EndOverride, Function, FunctionKind, Kind};
use serde_json::Value;

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// Normaliser #8a.
#[derive(Clone, Copy, Debug, Default)]
pub struct OwnValues;

/// What the transaction changed of the wiring: the endpoints of the connections it made, removed or gave other ends or
/// another kind (before and after), its junctions, and its interruption points.
fn touched(tx: &TxBuilder<'_>) -> (BTreeSet<EndpointKey>, BTreeSet<Id<Function>>) {
    let mut keys = BTreeSet::new();
    let mut ips = BTreeSet::new();
    for key in tx.changed_keys() {
        let (before, after) = (tx.base_row(&key), tx.row(&key));
        match key.0 {
            Kind::Connection => {
                let conn = |r: &Option<AnyRow>| match r {
                    Some(AnyRow::Connection(c)) => Some(c.clone()),
                    _ => None,
                };
                let (b, a) = (conn(&before), conn(&after));
                let same = match (&b, &a) {
                    (Some(b), Some(a)) => b.a == a.a && b.b == a.b && b.kind == a.kind,
                    _ => false,
                };
                if !same {
                    for c in b.iter().chain(a.iter()) {
                        keys.extend([&c.a, &c.b].into_iter().filter_map(EndpointKey::of));
                    }
                }
            }
            Kind::Junction => {
                keys.insert(EndpointKey::Junction(key.1.cast()));
            }
            Kind::Function => {
                let ip = |r: &Option<AnyRow>| match r {
                    Some(AnyRow::Function(f)) => match &f.kind {
                        FunctionKind::Ip(spec) => Some(Some(spec.clone())),
                        _ => Some(None),
                    },
                    _ => None,
                };
                let (b, a) = (ip(&before), ip(&after));
                let is_ip = b.clone().flatten().is_some() || a.clone().flatten().is_some();
                if is_ip && b != a {
                    ips.insert(key.1.cast());
                }
            }
            _ => {}
        }
    }
    (keys, ips)
}

/// Every wire of the potentials at `keys` and of those that hold an interruption point of `ips`, once, by ID.
fn wires_of(q: &Q, keys: &BTreeSet<EndpointKey>, ips: &BTreeSet<Id<Function>>) -> Vec<PhysicalWire> {
    let partition = q.get::<PotentialPartition>(());
    let mut potentials = BTreeSet::new();
    for k in keys {
        if let Some(p) = partition.potential_at(*k) {
            potentials.insert(p.id);
        }
    }
    if !ips.is_empty() {
        for p in partition.potentials() {
            if p.ips.iter().any(|f| ips.contains(f)) {
                potentials.insert(p.id);
            }
        }
    }
    let mut by_id: BTreeMap<Id<Connection>, PhysicalWire> = BTreeMap::new();
    for p in potentials {
        for w in &q.get::<PhysicalWiresOf>(p).wires {
            by_id.entry(w.id).or_insert_with(|| w.clone());
        }
    }
    by_id.into_values().collect()
}

/// What one wire wants: its members by name (`ew_derive::members`) and its ends set by hand by clamp.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
struct Want {
    members: BTreeMap<String, Value>,
    ends: BTreeMap<CpRef, EndOverride>,
}

/// The two clamps of a wire, sorted.
fn clamps(w: &PhysicalWire) -> [CpRef; 2] {
    let mut c = [w.ends[0].cp, w.ends[1].cp];
    c.sort();
    c
}

/// What each wire of `after` wants (see the module), from the wires of `before`.
fn want(qb: &Q, before: &[PhysicalWire], qa: &Q, after: &[PhysicalWire]) -> Vec<Want> {
    let had: Vec<Want> = before
        .iter()
        .map(|w| {
            let v = wire_values(qb, w);
            Want {
                members: v.members,
                ends: (0..2)
                    .filter_map(|i| v.ends[i].clone().map(|e| (w.ends[i].cp, e)))
                    .collect(),
            }
        })
        .collect();
    let at = |list: &[PhysicalWire], w: &PhysicalWire| list.iter().position(|x| x.id == w.id);
    let mut same: BTreeMap<usize, usize> = BTreeMap::new();
    let mut by_id: BTreeMap<usize, usize> = BTreeMap::new();
    for pair in wire_changes(before, after) {
        if let (Some(b), Some(a)) = (pair.before, pair.after)
            && let (Some(ib), Some(ia)) = (at(before, b), at(after, a))
        {
            if clamps(b) == clamps(a) {
                same.insert(ia, ib);
            } else {
                by_id.insert(ia, ib);
            }
        }
    }
    let kept: BTreeSet<usize> = same.values().copied().collect();
    after
        .iter()
        .enumerate()
        .map(|(ia, a)| {
            if let Some(ib) = same.get(&ia) {
                return had[*ib].clone();
            }
            // The wires it is made from: the one whose ID it keeps first, then in order.
            let mut from: Vec<usize> = by_id.get(&ia).copied().into_iter().collect();
            for (ib, b) in before.iter().enumerate() {
                if !kept.contains(&ib)
                    && !from.contains(&ib)
                    && clamps(b).iter().any(|c| clamps(a).contains(c))
                {
                    from.push(ib);
                }
            }
            if from.is_empty() {
                // New: what the command set on its own connections.
                let conns = qa.connections();
                let mut out = Want::default();
                for c in &a.own {
                    if let Some(row) = conns.get(*c) {
                        for (k, v) in members(&row.overrides) {
                            out.members.entry(k).or_insert(v);
                        }
                    }
                }
                for (i, h) in hand_ends(qa, a).into_iter().enumerate() {
                    if let Some(e) = h.and_then(|h| h.value) {
                        out.ends.insert(a.ends[i].cp, e);
                    }
                }
                return out;
            }
            let mut out = Want::default();
            for ib in &from {
                for (k, v) in &had[*ib].members {
                    if k != "length" {
                        out.members.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                }
                for e in &a.ends {
                    if let Some(end) = had[*ib].ends.get(&e.cp) {
                        out.ends.entry(e.cp).or_insert_with(|| end.clone());
                    }
                }
            }
            out
        })
        .collect()
}

/// A connection's overrides while they are put in order: the members by name, and its two ends.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Row {
    members: BTreeMap<String, Value>,
    ends: [Option<EndOverride>; 2],
}

/// Stores what each wire of `wires` wants (see the module): the connections whose overrides change, with them.
fn store(qa: &Q, wires: &[PhysicalWire], wants: &[Want]) -> Result<Vec<(Id<Connection>, Row)>, CmdError> {
    let conns = qa.connections();
    let mut rows: BTreeMap<Id<Connection>, Row> = BTreeMap::new();
    let mut through: BTreeMap<Id<Connection>, Vec<usize>> = BTreeMap::new();
    for (n, w) in wires.iter().enumerate() {
        for c in &w.constituents {
            let Some(row) = conns.get(*c) else {
                return Err(CmdError::internal(format!(
                    "own_values: the wire's connection {c} is not there"
                )));
            };
            rows.entry(*c).or_insert_with(|| Row {
                members: members(&row.overrides),
                ends: [row.overrides.end_a.clone(), row.overrides.end_b.clone()],
            });
            let list = through.entry(*c).or_default();
            if !list.contains(&n) {
                list.push(n);
            }
        }
    }
    let original = rows.clone();
    // A shared connection keeps a member only where every wire through it has that value, and no end.
    for (c, ws) in through.iter().filter(|(_, ws)| ws.len() > 1) {
        let Some(row) = rows.get_mut(c) else { continue };
        row.members
            .retain(|k, v| ws.iter().all(|n| wants[*n].members.get(k) == Some(&*v)));
        row.ends = [None, None];
    }
    for (n, w) in wires.iter().enumerate() {
        let Some(&first) = w.own.first() else { continue };
        let want = &wants[n];
        // The members: once, on the first own connection, unless one own connection holds it already or a shared
        // connection gives it.
        let mut keys: BTreeSet<String> = want.members.keys().cloned().collect();
        for c in &w.own {
            keys.extend(
                rows.get(c)
                    .map(|r| r.members.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_default(),
            );
        }
        for k in keys {
            let holders: Vec<Id<Connection>> = w
                .own
                .iter()
                .copied()
                .filter(|c| rows.get(c).is_some_and(|r| r.members.contains_key(&k)))
                .collect();
            let wanted = want.members.get(&k);
            let in_place = match (wanted, holders.as_slice()) {
                (None, []) => true,
                (Some(v), [one]) => rows.get(one).and_then(|r| r.members.get(&k)) == Some(v),
                (Some(v), []) => w
                    .constituents
                    .iter()
                    .any(|c| !w.own.contains(c) && rows.get(c).and_then(|r| r.members.get(&k)) == Some(v)),
                _ => false,
            };
            if in_place {
                continue;
            }
            for h in holders {
                if let Some(r) = rows.get_mut(&h) {
                    r.members.remove(&k);
                }
            }
            if let (Some(v), Some(r)) = (wanted, rows.get_mut(&first)) {
                r.members.insert(k, v.clone());
            }
        }
        // The ends: once, in a slot of an own connection that leads to the clamp.
        let slots: Vec<(Id<Connection>, usize, Option<usize>)> = w
            .own
            .iter()
            .filter_map(|c| w.constituents.iter().position(|x| x == c).map(|k| (*c, k)))
            .flat_map(|(c, k)| [0, 1].map(|s| (c, s, end_through(qa, w, k, s == 0))))
            .collect();
        let value = |rows: &BTreeMap<Id<Connection>, Row>, (c, s): (Id<Connection>, usize)| {
            rows.get(&c).and_then(|r| r.ends[s].clone())
        };
        for (c, s, end) in &slots {
            if end.is_none()
                && value(&rows, (*c, *s)).is_some()
                && let Some(r) = rows.get_mut(c)
            {
                r.ends[*s] = None;
            }
        }
        for i in 0..2 {
            let wanted = (w.ends[i].kind == EndKind::Device)
                .then(|| want.ends.get(&w.ends[i].cp))
                .flatten();
            let mine: Vec<(Id<Connection>, usize)> = slots
                .iter()
                .filter(|(_, _, e)| *e == Some(i))
                .map(|(c, s, _)| (*c, *s))
                .collect();
            let held: Vec<(Id<Connection>, usize)> = mine
                .iter()
                .copied()
                .filter(|at| value(&rows, *at).is_some())
                .collect();
            if let (Some(v), [one]) = (wanted, held.as_slice())
                && value(&rows, *one).as_ref() == Some(v)
            {
                continue;
            }
            if wanted.is_none() && held.is_empty() {
                continue;
            }
            for (c, s) in held {
                if let Some(r) = rows.get_mut(&c) {
                    r.ends[s] = None;
                }
            }
            let target = mine
                .iter()
                .find(|(c, _)| *c == first)
                .or_else(|| mine.first())
                .copied();
            if let (Some(v), Some((c, s))) = (wanted, target)
                && let Some(r) = rows.get_mut(&c)
            {
                r.ends[s] = Some(v.clone());
            }
        }
    }
    Ok(rows
        .into_iter()
        .filter(|(c, r)| original.get(c) != Some(r))
        .collect())
}

impl Normaliser for OwnValues {
    fn id(&self) -> &'static str {
        "own_values"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let (keys, ips) = touched(tx);
        if keys.is_empty() && ips.is_empty() {
            return Ok(false);
        }
        let now = tx.planning_model()?.clone();
        let (qb, qa) = (Q::new(tx.base_model()), Q::new(&now));
        // The wires after: of the potentials the changes touch, and of those the wires before them stand in now (a
        // wire whose far end the command removed); the wires before: of the same potentials in the state before.
        let around = |q: &Q, wires: &[PhysicalWire]| -> BTreeSet<EndpointKey> {
            let mut out = keys.clone();
            for w in wires {
                out.extend(w.ends.iter().map(|e| e.cp.key()));
                for c in &w.constituents {
                    if let Some(conn) = q.connections().get(*c) {
                        out.extend([&conn.a, &conn.b].into_iter().filter_map(EndpointKey::of));
                    }
                }
            }
            out
        };
        let first = wires_of(&qb, &keys, &ips);
        let after = wires_of(&qa, &around(&qa, &first), &ips);
        let before = wires_of(&qb, &around(&qb, &after), &ips);
        let wants = want(&qb, &before, &qa, &after);
        let changes = store(&qa, &after, &wants)?;
        let changed = !changes.is_empty();
        for (c, row) in changes {
            let Some(current) = tx.get::<Connection>(c) else {
                continue;
            };
            let mut o = with_members(&current.overrides, &row.members).map_err(|e| {
                CmdError::internal(format!("own_values: a wire's values do not read back ({e})"))
            })?;
            [o.end_a, o.end_b] = row.ends;
            tx.update::<Connection>(c, |conn| conn.overrides = o)?;
        }
        Ok(changed)
    }
}
