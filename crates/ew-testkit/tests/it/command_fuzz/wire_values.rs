// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Oracle (g): a wire's own values follow the wire (doc 13 §2.3).
//!
//! The fuzz base gives the sample's wires values of their own ([`seed`]): each wire with a connection of its own a
//! length of its own, every second a colour and a size, every third its ends set by hand. After every command step
//! that changed the wiring (a wire whose constituents or clamps differ), [`check`] holds, reading the wires as every
//! list reads them (`ew_derive::wire_values`), never how the normaliser stores them:
//!
//! - a wire of before and after with the same two clamps (matched by `ew_derive::wire_changes`) that has a connection
//!   of its own has the same values, each end the one of its clamp;
//! - a wire that is new or whose clamps changed, with a clamp of a function that existed before, has no typed length;
//!   of the other members only what a wire it was made from had (one that lost a clamp it ends at); with one such
//!   wire, all of them (what divides); and each end set by hand the one a wire it was made from had at that clamp;
//! - of the wires that changed: no member stands twice on the wire's own connections, no end set by hand stands on a
//!   connection several wires share, and a member a shared connection holds is the value of every wire through it.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::{
    PhysicalWire, PhysicalWiresOf, PotentialPartition, Q, WireValues, wire_changes, wire_values,
};
use ew_ids::Id;
use ew_model::{AnyRow, Model, Op};
use ew_schema::{Connection, EndOverride};
use serde_json::json;

/// Every physical wire of the project once, by ID.
fn wires(q: &Q) -> Vec<PhysicalWire> {
    let partition = q.get::<PotentialPartition>(());
    let mut by_id: BTreeMap<Id<Connection>, PhysicalWire> = BTreeMap::new();
    for p in partition.potentials() {
        for w in &q.get::<PhysicalWiresOf>(p.id).wires {
            by_id.entry(w.id).or_insert_with(|| w.clone());
        }
    }
    by_id.into_values().collect()
}

/// The two clamps of a wire, sorted.
fn clamps(w: &PhysicalWire) -> [ew_derive::CpRef; 2] {
    let mut c = [w.ends[0].cp, w.ends[1].cp];
    c.sort();
    c
}

/// The ends set by hand of a wire, by clamp.
fn ends_by_clamp(w: &PhysicalWire, v: &WireValues) -> BTreeMap<ew_derive::CpRef, EndOverride> {
    (0..2)
        .filter_map(|i| v.ends[i].clone().map(|e| (w.ends[i].cp, e)))
        .collect()
}

/// The ops that give the sample's wires values of their own (see the module).
pub fn seed(model: &Model) -> Vec<Op> {
    let q = Q::new(model);
    let mut rows: BTreeMap<Id<Connection>, Connection> = BTreeMap::new();
    for (n, w) in wires(&q).iter().filter(|w| !w.own.is_empty()).enumerate() {
        let at = w.own[0];
        let Some(c) = q.connections().get(at) else {
            continue;
        };
        let row = rows.entry(at).or_insert_with(|| (**c).clone());
        let mm = 1000 + 7 * i64::try_from(n).unwrap_or(0);
        row.overrides.length = Some(ew_units::Nm(mm * 1_000_000));
        if n % 2 == 0 {
            row.overrides.colour = serde_json::from_value(json!(["BU"])).ok();
            row.overrides.csa = serde_json::from_value(json!("1.5")).ok();
        }
        if n % 3 == 0 {
            for (i, h) in ew_derive::hand_ends(&q, w).into_iter().enumerate() {
                let Some(h) = h else { continue };
                let word = if i == 0 { "bare" } else { "ring_lug" };
                let end: Option<EndOverride> = serde_json::from_value(json!({"treatment": word})).ok();
                let row = rows.entry(h.at.0).or_insert_with(|| (**c).clone());
                if h.at.1 {
                    row.overrides.end_a = end;
                } else {
                    row.overrides.end_b = end;
                }
            }
        }
    }
    rows.into_values()
        .filter_map(|after| {
            let before = q.connections().get(after.id)?.clone();
            Some(Op::update(AnyRow::Connection(before), AnyRow::of(after)))
        })
        .collect()
}

/// The oracle (see the module); `before` and `after` are the states around one committed step.
pub fn check(before: &Q, after: &Q) -> Result<(), String> {
    let (wb, wa) = (wires(before), wires(after));
    let shape = |w: &PhysicalWire| (w.id, w.constituents.clone(), clamps(w));
    let shapes_before: BTreeSet<_> = wb.iter().map(shape).collect();
    let changed: Vec<bool> = wa.iter().map(|w| !shapes_before.contains(&shape(w))).collect();
    if !changed.iter().any(|c| *c) && wb.len() == wa.len() {
        return Ok(());
    }
    let vb: Vec<WireValues> = wb.iter().map(|w| wire_values(before, w)).collect();
    let va: Vec<WireValues> = wa.iter().map(|w| wire_values(after, w)).collect();
    let index = |list: &[PhysicalWire], w: &PhysicalWire| list.iter().position(|x| x.id == w.id);
    let pairs = wire_changes(&wb, &wa);
    let same_clamps: BTreeMap<usize, usize> = pairs
        .iter()
        .filter_map(|p| match (p.before, p.after) {
            (Some(b), Some(a)) if clamps(b) == clamps(a) => Some((index(&wa, a)?, index(&wb, b)?)),
            _ => None,
        })
        .collect();
    let kept_before: BTreeSet<usize> = same_clamps.values().copied().collect();
    let name = |w: &PhysicalWire| format!("the wire {} ({:?} to {:?})", w.id, w.ends[0].cp, w.ends[1].cp);

    for (ia, a) in wa.iter().enumerate() {
        if let Some(&ib) = same_clamps.get(&ia) {
            if a.own.is_empty() {
                continue;
            }
            let b = &wb[ib];
            if va[ia].members != vb[ib].members {
                return Err(format!(
                    "{} kept its clamps but not its values: {:?} before, {:?} after",
                    name(a),
                    vb[ib].members,
                    va[ia].members
                ));
            }
            if ends_by_clamp(a, &va[ia]) != ends_by_clamp(b, &vb[ib]) {
                return Err(format!(
                    "{} kept its clamps but not its ends set by hand: {:?} before, {:?} after",
                    name(a),
                    ends_by_clamp(b, &vb[ib]),
                    ends_by_clamp(a, &va[ia])
                ));
            }
            continue;
        }
        let old_clamp = a
            .ends
            .iter()
            .any(|e| before.functions().get(e.cp.function).is_some());
        if !old_clamp {
            continue;
        }
        if va[ia].members.contains_key("length") {
            return Err(format!(
                "{} is new or changed its clamps and has a typed length",
                name(a)
            ));
        }
        let from: Vec<usize> = (0..wb.len())
            .filter(|ib| !kept_before.contains(ib))
            .filter(|ib| clamps(&wb[*ib]).iter().any(|c| clamps(a).contains(c)))
            .collect();
        for (k, v) in &va[ia].members {
            if !from.iter().any(|ib| vb[*ib].members.get(k) == Some(v)) {
                return Err(format!(
                    "{} has {k} = {v}, which no wire it was made from had (made from {:?})",
                    name(a),
                    from.iter().map(|ib| wb[*ib].id).collect::<Vec<_>>()
                ));
            }
        }
        if let [ib] = from.as_slice() {
            let mut divides = vb[*ib].members.clone();
            divides.remove("length");
            if divides != va[ia].members {
                return Err(format!(
                    "{} was made from the wire {} and does not keep what divides: {divides:?} before, {:?} after",
                    name(a),
                    wb[*ib].id,
                    va[ia].members
                ));
            }
        }
        let ends = ends_by_clamp(a, &va[ia]);
        for (cp, e) in &ends {
            let had = from
                .iter()
                .any(|ib| ends_by_clamp(&wb[*ib], &vb[*ib]).get(cp) == Some(e));
            if !had {
                return Err(format!(
                    "{} has an end set by hand at {cp:?} that no wire it was made from had there",
                    name(a)
                ));
            }
        }
        if !a.own.is_empty() {
            for ib in &from {
                for (cp, e) in ends_by_clamp(&wb[*ib], &vb[*ib]) {
                    let alone = from
                        .iter()
                        .filter(|j| ends_by_clamp(&wb[**j], &vb[**j]).contains_key(&cp))
                        .count()
                        == 1;
                    if clamps(a).contains(&cp) && alone && ends.get(&cp) != Some(&e) {
                        return Err(format!(
                            "{} lost the end set by hand at {cp:?} of the wire {} it was made from",
                            name(a),
                            wb[*ib].id
                        ));
                    }
                }
            }
        }
    }

    // How the changed wires store their values.
    let mut through: BTreeMap<Id<Connection>, Vec<usize>> = BTreeMap::new();
    for (ia, a) in wa.iter().enumerate() {
        let distinct: BTreeSet<Id<Connection>> = a.constituents.iter().copied().collect();
        for c in distinct {
            through.entry(c).or_default().push(ia);
        }
    }
    let conns = after.connections();
    for a in wa.iter().zip(&changed).filter(|(_, c)| **c).map(|(a, _)| a) {
        let mut held: BTreeMap<String, usize> = BTreeMap::new();
        for c in &a.own {
            let Some(row) = conns.get(*c) else { continue };
            for k in ew_derive::members(&row.overrides).into_keys() {
                *held.entry(k).or_default() += 1;
            }
        }
        if let Some((k, _)) = held.iter().find(|(_, n)| **n > 1) {
            return Err(format!("{} holds {k} on two of its own connections", name(a)));
        }
        for c in &a.constituents {
            let Some(row) = conns.get(*c) else { continue };
            let wires_here = through.get(c).map_or(&[][..], Vec::as_slice);
            if wires_here.len() < 2 {
                continue;
            }
            if row.overrides.end_a.is_some() || row.overrides.end_b.is_some() {
                return Err(format!(
                    "the connection {c}, which several wires share, holds an end set by hand (of {})",
                    name(a)
                ));
            }
            for (k, v) in ew_derive::members(&row.overrides) {
                if let Some(other) = wires_here.iter().find(|j| va[**j].members.get(&k) != Some(&v)) {
                    return Err(format!(
                        "the connection {c}, which several wires share, holds {k} = {v}, which {} does not have",
                        name(&wa[*other])
                    ));
                }
            }
        }
    }
    Ok(())
}
