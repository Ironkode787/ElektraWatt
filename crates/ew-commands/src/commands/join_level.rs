// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `rep.join_device` for a terminal (doc 08 §6.3, FT-1878): a terminal joins another terminal of its strip as that
//! one's next level, so that a block of two levels with a number on each is one terminal with one part.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_model::EndpointKey;
use ew_schema::{
    Connection, Device, DeviceLink, Endpoint, Function, Kind, LevelSpec, LibCopy, PinMapEntry,
    Representation, SymbolDef,
};

use super::common::function_cp_levels;
use super::join::{dropped_device_members, dropped_function_members, listed, move_attached, was_or_were};
use crate::command::Outcome;
use crate::error::CmdError;
use crate::text::text;
use crate::tx::TxBuilder;

/// The command whose parameters the refusals point into.
const NAME: &str = "rep.join_device";

/// The one function of a terminal for the join, or the refusal naming the terminal.
fn only_function(tx: &TxBuilder<'_>, device: Id<Device>) -> Result<Id<Function>, CmdError> {
    match tx.functions_of_device(device).as_slice() {
        [f] => Ok(*f),
        _ => Err(CmdError::invalid_params(
            NAME,
            "/rep",
            text!("refusal-join-level-functions", crate::words::device(tx, device)),
        )),
    }
}

/// The terminal `source`, drawn by the representation `rep`, becomes the next level of the terminal `target` of its
/// strip in one transaction (doc 08 §6.3):
///
/// - The level: one above the levels `target` lists, or, when it lists none, above the one its CPs carry (level 1
///   without one); `target` then lists its own level and the new one, the new one designated by `source`'s designation
///   unless that is `target`'s own (two terminals of one number become one block of that number with two levels).
/// - The CPs: `source`'s CPs of its level, in index order, become the CPs of the new level of `target`, in index
///   order (the copies `Terminal::level_copies` adds, or those of a part that has the level); its CPs without a level
///   those of `target` without one, in order.
/// - Every representation of `source`'s function draws `target`'s function, each of its symbol's CPs by its pin map
///   onto the CP it drew before; every connection, potential definition, bond and channel at `source`'s CPs moves with
///   it ([`move_attached`]).
/// - `target` keeps its part uses; one without any takes `source`'s. `source` goes, with what it carries besides (the
///   outcome's notices name it, as the join of another device does).
///
/// Refused, with the pointer: `target` is `source` or no terminal of `source`'s strip (`/device`); either has another
/// number of functions than one, `source` lists or carries more than one level, or takes part in a device link
/// (`/rep`); `target` has no new level to give, or that level lacks a CP that one of `source`'s drawn or attached CPs
/// would move to (`/device`). Outcome: the representation.
pub(super) fn join_level(
    tx: &mut TxBuilder<'_>,
    rep: Id<Representation>,
    source: &Device,
    target: Id<Device>,
) -> Result<Outcome, CmdError> {
    let Some(own) = source.aspects.terminal.as_ref() else {
        return Err(CmdError::internal(format!("{NAME}: the source is no terminal")));
    };
    if target == source.id {
        return Err(CmdError::invalid_params(
            NAME,
            "/device",
            text!("refusal-join-same-device", crate::words::device(tx, target)),
        ));
    }
    let block = tx.require::<Device>(target)?;
    let Some(theirs) = block.aspects.terminal.as_ref().filter(|t| t.strip == own.strip) else {
        return Err(CmdError::invalid_params(
            NAME,
            "/device",
            text!(
                "refusal-join-level-strip",
                crate::words::device(tx, target),
                crate::words::device(tx, own.strip)
            ),
        ));
    };
    let from = only_function(tx, source.id)?;
    let to = only_function(tx, target)?;
    if let Some(link) = tx
        .iter::<DeviceLink>()
        .into_iter()
        .filter(|l| l.from == source.id || l.to == source.id)
        .min_by_key(|l| l.id)
    {
        let other = if link.from == source.id {
            link.to
        } else {
            link.from
        };
        return Err(CmdError::invalid_params(
            NAME,
            "/rep",
            text!("refusal-join-device-link", crate::words::device(tx, other)),
        ));
    }
    let from_fn = tx.require::<Function>(from)?;
    let to_fn = tx.require::<Function>(to)?;

    // The source's CPs: one level at most.
    let source_cps = function_cp_levels(tx, &from_fn, source);
    let source_levels: BTreeSet<u8> = source_cps.iter().filter_map(|(_, l)| *l).collect();
    if source_levels.len() > 1 || own.levels.len() > 1 {
        return Err(CmdError::invalid_params(
            NAME,
            "/rep",
            text!("refusal-join-level-block", crate::words::device(tx, source.id)),
        ));
    }

    // The new level and the target's levels with it.
    let target_cps = function_cp_levels(tx, &to_fn, &block);
    let base = target_cps.iter().filter_map(|(_, l)| *l).min().unwrap_or(1);
    let mut levels = theirs.levels.clone();
    if levels.is_empty() {
        levels.push(LevelSpec {
            level: base,
            role: Default::default(),
            designation: None,
            extra: Default::default(),
        });
    }
    let top = levels.iter().map(|l| l.level).max().unwrap_or(base);
    let no_level = || {
        Err(CmdError::invalid_params(
            NAME,
            "/device",
            text!(
                "refusal-join-level-none",
                crate::words::device(tx, target),
                top.saturating_add(1).to_string()
            ),
        ))
    };
    let Some(level) = top.checked_add(1) else {
        return no_level();
    };
    levels.push(LevelSpec {
        level,
        role: Default::default(),
        designation: (own.designation != theirs.designation).then(|| own.designation.clone()),
        extra: Default::default(),
    });
    let mut after = (*block).clone();
    if let Some(t) = after.aspects.terminal.as_mut() {
        t.levels = levels;
    }
    if after.parts.is_empty() {
        after.parts = source.parts.clone();
    }
    let gained: Vec<u16> = function_cp_levels(tx, &to_fn, &after)
        .into_iter()
        .filter(|(_, l)| *l == Some(level))
        .map(|(i, _)| i)
        .collect();
    if gained.is_empty() {
        return no_level();
    }
    let shared: Vec<u16> = target_cps
        .iter()
        .filter(|(_, l)| l.is_none())
        .map(|(i, _)| *i)
        .collect();

    // The source's CPs onto the new level's (and its CPs without a level onto the target's), by position.
    let leveled: Vec<u16> = source_cps
        .iter()
        .filter(|(_, l)| l.is_some())
        .map(|(i, _)| *i)
        .collect();
    let unleveled: Vec<u16> = source_cps
        .iter()
        .filter(|(_, l)| l.is_none())
        .map(|(i, _)| *i)
        .collect();
    let onto = |i: u16| -> Option<u16> {
        let at = |list: &[u16]| list.iter().position(|x| *x == i);
        match at(&leveled) {
            Some(k) => gained.get(k).copied(),
            None => at(&unleveled).and_then(|k| shared.get(k).copied()),
        }
    };

    // What must move: the CPs the source's representations draw and those something is attached to.
    let reps = tx.reps_of_function(from);
    let mut pins: Vec<(Id<Representation>, Vec<PinMapEntry>)> = Vec::new();
    let mut needed: BTreeSet<u16> = BTreeSet::new();
    for r in &reps {
        let record = tx.require::<Representation>(*r)?;
        let symbol = tx.require::<LibCopy<SymbolDef>>(record.symbol)?;
        let mut entries: Vec<PinMapEntry> = Vec::new();
        for cp in &symbol.item.cps {
            let mut entry = match record.pin_map.iter().find(|p| p.symbol_cp == cp.idx) {
                Some(p) => p.clone(),
                None => PinMapEntry {
                    symbol_cp: cp.idx,
                    function: from,
                    idx: cp.fn_cp,
                    extra: Default::default(),
                },
            };
            if entry.function == from {
                needed.insert(entry.idx);
                entry.function = to;
                entry.idx = onto(entry.idx).unwrap_or(entry.idx);
            }
            entries.push(entry);
        }
        pins.push((*r, entries));
    }
    let all: BTreeSet<u16> = source_cps.iter().map(|(i, _)| *i).collect();
    for c in tx.connections_of_function(from) {
        if let Some(c) = tx.get::<Connection>(c) {
            for e in [&c.a, &c.b] {
                if let Endpoint::Cp(g, i) = e
                    && *g == from
                {
                    needed.insert(*i);
                }
            }
        }
    }
    for i in &all {
        let at = EndpointKey::Cp(from, *i);
        if !tx.potential_defs_at(at).is_empty()
            || !tx.bonds_at(at).is_empty()
            || !tx.channels_at_cp(from, *i).is_empty()
        {
            needed.insert(*i);
        }
    }
    if let Some(lost) = needed.iter().find(|i| onto(**i).is_none()) {
        return Err(CmdError::invalid_params(
            NAME,
            "/device",
            text!(
                "refusal-join-level-cp",
                crate::words::device(tx, target),
                level.to_string(),
                lost.to_string(),
                crate::words::device(tx, source.id)
            ),
        ));
    }

    // The words of what goes with the source, before it goes.
    let mut device_members = dropped_device_members(source, &block);
    if !source.parts.is_empty() && !block.parts.is_empty() {
        device_members.push("part");
    }
    let function_members = dropped_function_members(&from_fn);

    tx.put(after);
    for (r, entries) in pins {
        let pin_map = ew_schema::KeyedList::from_vec(entries)
            .map_err(|e| CmdError::internal(format!("{NAME}: the pin map: {e}")))?;
        tx.update::<Representation>(r, |r| {
            for f in &mut r.functions {
                if *f == from {
                    *f = to;
                }
            }
            r.pin_map = pin_map;
        })?;
    }
    move_attached(tx, from, &all, to, &|i| onto(i).unwrap_or(i))?;
    tx.delete((Kind::Device, source.id.cast()))?;

    let mut outcome = Outcome::primary((Kind::Representation, rep.cast()));
    if !device_members.is_empty() {
        outcome.notices.push(format!(
            "the joined terminal's {} {} dropped with it",
            listed(&device_members),
            was_or_were(&device_members)
        ));
    }
    if !function_members.is_empty() {
        outcome.notices.push(format!(
            "its function's {} {} dropped with it",
            listed(&function_members),
            was_or_were(&function_members)
        ));
    }
    Ok(outcome)
}
