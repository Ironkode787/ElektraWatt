// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser N-FL-1 `valve_functions` (doc 06 §3.2 row 1, doc 11 §4.2, FR-FLU-003): a valve's functions
//! follow its valve part.
//!
//! - **Trigger.** A device of the transaction that is new or whose `parts` changed and that has a **valve part**: the
//!   first part use whose copy has a fluid section with solenoids (`ew_derive::valve_solenoids`). A part without a
//!   fluid section changes nothing.
//! - **The template.** The valve part's `function_template`: its valve entry is the first on a FnDef of contact kind
//!   `valve`; its coil entries are those on a FnDef of contact kind `coil`, the k-th being solenoid k of
//!   `fluid.solenoids`. Every entry's FnDef copy must be embedded (else the transaction is refused, as
//!   part-first placement refuses it).
//! - **Effect.** (a) A device without a function on a FnDef of contact kind `valve` gets one from the valve entry; (b)
//!   each coil entry without its function gets one; a new function has the ID `v5(device, template_index)` (ADR-0004
//!   amendment 2), or, when that ID is a kept record, `v5(that ID, template_index)` and so on, the entry's role (auxiliary when the device already has a main function, EW-INV-002) and its index
//!   as `template_entry`; an internal coil whose solenoid differs from its FnDef CP's designation gets a designation
//!   override on that CP. (c) A coil function of the device that is no entry of the valve part's template (the old
//!   part's, after a part change) is removed; a channel bound to it is unbound with the notice `solenoid <s> of <valve>
//!   went with the old part; <channel> is unbound`, unless N2 binds the channel again in the same step. (d) A valve
//!   part whose valve entry names another FnDef **replaces the valve function's definition in place** (WP-S1.25): the
//!   function keeps its ID, its representations and its role; every CP is matched by designation to the new FnDef's
//!   CP, and the connections, closures and medium definitions at a CP follow it to the new index; those at a port the
//!   new valve lacks are deleted, each listed in a notice; a representation whose symbol draws a CP at another index
//!   than the new FnDef's gets a pin-map entry to it. The preview of the part change (`Engine::preview`) shows the
//!   notices before the commit. The drawn symbol keeps its graphics: a notice says to exchange it.
//! - **Entries and functions.** A coil function is the function of an entry when its `template_entry` names that entry
//!   and its FnDef is the entry's; the others then take, in ID order, the first entry left with their FnDef.
//!
//! Idempotent: after a run every entry has its function and no coil function is left over. Notices name records in
//! words.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_mltext::Ident;
use ew_schema::{
    Connection, ContactKind, CpOverride, Device, Endpoint, Extra, FnDef, FnRole, Function, FunctionKind,
    KeyedList, Kind, LibCopy, MediumDef, Part, PinMapEntry, Representation, SymbolDef,
};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::text::text;
use crate::tx::TxBuilder;
use crate::words;

/// The most derived IDs a new function tries past `v5(device, entry)` before the transaction is refused.
const FRESH_TRIES: usize = 8;

/// Normaliser N-FL-1.
#[derive(Clone, Copy, Debug, Default)]
pub struct ValveFunctions;

/// Whether a device of the transaction is new or changed its part uses.
fn parts_changed(tx: &TxBuilder<'_>, dev: &Device) -> bool {
    tx.base_row(&(Kind::Device, dev.id.cast()))
        .and_then(|r| r.get::<Device>().cloned())
        .is_none_or(|b| b.parts != dev.parts)
}

/// The designation that a coil function's first non-physical CP prints: its override, else the FnDef CP's.
fn coil_solenoid(def: &FnDef, func: &Function) -> Option<Ident> {
    let cp = def.cps.iter().find(|c| !c.physical)?;
    let over = func
        .cp_overrides
        .iter()
        .find(|o| o.idx == cp.idx)
        .and_then(|o| o.designation.clone());
    Some(over.unwrap_or_else(|| cp.designation.clone()))
}

impl Normaliser for ValveFunctions {
    fn id(&self) -> &'static str {
        "valve_functions"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let mut changed = false;
        for key in tx.changed_of(Kind::Device) {
            let Some(dev) = tx.get::<Device>(key.1.cast()) else {
                continue;
            };
            if !parts_changed(tx, &dev) {
                continue;
            }
            let Some(part) = dev
                .parts
                .iter()
                .filter_map(|u| tx.get::<LibCopy<Part>>(u.part.copy))
                .find(|c| ew_derive::valve_solenoids(&c.item).is_some())
            else {
                continue; // no valve part: nothing changes (D6 e)
            };
            changed |= follow_part(tx, &dev, &part)?;
        }
        Ok(changed)
    }
}

/// One device's functions made to follow its valve part.
fn follow_part(tx: &mut TxBuilder<'_>, dev: &Device, part: &LibCopy<Part>) -> Result<bool, CmdError> {
    let solenoids = ew_derive::valve_solenoids(&part.item)
        .unwrap_or_default()
        .to_vec();
    let number = part.item.part_number.as_str().to_owned();
    // (entry index, FnDef copy, contact kind, main)
    let mut entries: Vec<(usize, Id<FnDef>, ContactKind, bool)> = Vec::new();
    for (i, e) in part.item.function_template.iter().enumerate() {
        let def: Id<FnDef> = ew_ids::v5::lib_copy_id(e.fndef.library.as_uuid(), e.fndef.item.as_uuid());
        let Some(copy) = tx.get::<LibCopy<FnDef>>(def) else {
            return tx.reject(text!("refusal-part-entry-fndef-missing", i.to_string(), number));
        };
        entries.push((i, def, copy.item.contact_kind.clone(), e.main));
    }
    let mut functions = tx.functions_of_device(dev.id);
    functions.sort();
    // (function, FnDef copy, contact kind)
    let mut held: Vec<(std::sync::Arc<Function>, std::sync::Arc<LibCopy<FnDef>>)> = Vec::new();
    for f in functions {
        if let Some(func) = tx.get::<Function>(f)
            && let Some(def) = tx.get::<LibCopy<FnDef>>(func.def)
        {
            held.push((func, def));
        }
    }
    let mut has_main = held.iter().any(|(f, _)| f.role == FnRole::Main);
    let mut changed = false;
    let name = words::device(tx, dev.id);

    // (a) and (d): the valve function
    let valve_entry = entries.iter().find(|e| e.2 == ContactKind::Valve);
    let valves: Vec<&Id<FnDef>> = held
        .iter()
        .filter(|(_, d)| d.item.contact_kind == ContactKind::Valve)
        .map(|(f, _)| &f.def)
        .collect();
    let mut create: Vec<(usize, Id<FnDef>, bool, Option<Ident>)> = Vec::new();
    if let Some((i, def, _, main)) = valve_entry {
        if valves.is_empty() {
            create.push((*i, *def, *main, None));
        } else if !valves.contains(&def) {
            let old: Vec<std::sync::Arc<Function>> = held
                .iter()
                .filter(|(_, d)| d.item.contact_kind == ContactKind::Valve)
                .map(|(f, _)| f.clone())
                .collect();
            for f in old {
                replace_valve(tx, &f, *def, &name)?;
            }
            changed = true;
        }
    }

    // (b) and (c): the coil functions against the coil entries
    let coil_entries: Vec<(usize, Id<FnDef>, bool)> = entries
        .iter()
        .filter(|e| e.2 == ContactKind::Coil)
        .map(|e| (e.0, e.1, e.3))
        .collect();
    let coils: Vec<&(std::sync::Arc<Function>, std::sync::Arc<LibCopy<FnDef>>)> = held
        .iter()
        .filter(|(_, d)| d.item.contact_kind == ContactKind::Coil)
        .collect();
    let mut claimed: BTreeSet<usize> = BTreeSet::new();
    let mut matched: BTreeSet<Id<Function>> = BTreeSet::new();
    for (f, _) in &coils {
        if let Some((i, _, _)) = coil_entries
            .iter()
            .find(|(i, def, _)| f.template_entry.map(usize::from) == Some(*i) && *def == f.def)
            && claimed.insert(*i)
        {
            matched.insert(f.id);
        }
    }
    for (f, _) in &coils {
        if matched.contains(&f.id) {
            continue;
        }
        if let Some((i, _, _)) = coil_entries
            .iter()
            .find(|(i, def, _)| !claimed.contains(i) && *def == f.def)
        {
            claimed.insert(*i);
            matched.insert(f.id);
        }
    }
    for (k, (i, def, main)) in coil_entries.iter().enumerate() {
        if !claimed.contains(i) {
            create.push((*i, *def, *main, solenoids.get(k).cloned()));
        }
    }
    for (f, def) in &coils {
        if matched.contains(&f.id) {
            continue;
        }
        let solenoid = coil_solenoid(&def.item, f)
            .map(|s| s.as_str().to_owned())
            .unwrap_or_default();
        let mut bound: Vec<Id<ew_schema::Channel>> = def
            .item
            .cps
            .iter()
            .flat_map(|cp| tx.channels_at_cp(f.id, cp.idx))
            .collect();
        bound.sort();
        bound.dedup();
        let unbound: Vec<(Id<ew_schema::Channel>, String)> = bound
            .iter()
            .map(|c| (*c, words::channel(tx, *c).to_string()))
            .collect();
        for c in bound {
            tx.update::<ew_schema::Channel>(c, |c| c.binding = None)?;
        }
        if f.role == FnRole::Main {
            has_main = false;
        }
        tx.delete((Kind::Function, f.id.cast()))?;
        // N2 may bind the new coil to the same channel in this step; the notice then goes.
        for (id, c) in unbound {
            tx.notice_while_unbound(
                id,
                format!("solenoid {solenoid} of {name} went with the old part; {c} is unbound"),
            );
        }
        changed = true;
    }

    create.sort_by_key(|c| c.0);
    for (i, def, main, solenoid) in create {
        let copy = tx.require::<LibCopy<FnDef>>(def)?;
        let kind = if copy.item.contact_kind == ContactKind::BlackBox {
            FunctionKind::BlackBox
        } else {
            FunctionKind::Normal
        };
        let role = if main && !has_main {
            has_main = true;
            FnRole::Main
        } else {
            FnRole::Aux
        };
        let mut overrides = Vec::new();
        if let (Some(s), Some(cp)) = (solenoid, copy.item.cps.iter().find(|c| !c.physical))
            && cp.designation != s
        {
            overrides.push(CpOverride {
                idx: cp.idx,
                designation: Some(s),
                alias: None,
                description: None,
                unused: false,
                closure: None,
                extra: Extra::new(),
            });
        }
        // The ID `v5(device, entry)`; when that is a kept record (the old part's valve function at another
        // entry), `v5` of it and the entry again, until a free one.
        let index = u32::try_from(i).unwrap_or(u32::MAX);
        let mut id: Id<Function> = ew_ids::v5::template_function_id(dev.id, index);
        for _ in 0..FRESH_TRIES {
            if !tx.exists::<Function>(id) {
                break;
            }
            id = ew_ids::v5::template_function_id(id.cast::<Device>(), index);
        }
        if tx.exists::<Function>(id) {
            return tx.reject(text!("refusal-valve-functions-no-id", name, number));
        }
        tx.insert(Function {
            id,
            v: 1,
            device: Some(dev.id),
            def,
            role,
            kind,
            cp_overrides: KeyedList::from_vec(overrides).expect("one override"),
            source_ref: None,
            template_entry: u16::try_from(i).ok(),
            bus_port: None,
            extra: Extra::new(),
        })?;
        changed = true;
    }
    Ok(changed)
}

/// N-FL-1 (d): the valve function `f` takes the FnDef `def` in place; its CPs follow by designation.
fn replace_valve(
    tx: &mut TxBuilder<'_>,
    f: &Function,
    def: Id<FnDef>,
    name: &crate::text::TextArg,
) -> Result<(), CmdError> {
    let old_def = tx.require::<LibCopy<FnDef>>(f.def)?;
    let new_def = tx.require::<LibCopy<FnDef>>(def)?;
    // Old CP index -> new CP index, by the FnDef designation.
    let map = |idx: u16| -> Option<u16> {
        let d = &old_def.item.cps.iter().find(|c| c.idx == idx)?.designation;
        new_def
            .item
            .cps
            .iter()
            .find(|c| &c.designation == d)
            .map(|c| c.idx)
    };
    let designation = |idx: u16| {
        old_def
            .item
            .cps
            .iter()
            .find(|c| c.idx == idx)
            .map(|c| c.designation.as_str().to_owned())
            .unwrap_or_default()
    };
    // Connections.
    let mut lines = tx.connections_of_function(f.id);
    lines.sort();
    lines.dedup();
    let mut dropped: Vec<String> = Vec::new();
    for c in lines {
        let Some(conn) = tx.get::<Connection>(c) else {
            continue;
        };
        let follow = |e: &Endpoint| match e {
            Endpoint::Cp(g, idx) if *g == f.id => map(*idx).map(|n| Endpoint::Cp(*g, n)),
            other => Some(other.clone()),
        };
        match (follow(&conn.a), follow(&conn.b)) {
            (Some(a), Some(b)) => {
                if a != conn.a || b != conn.b {
                    tx.update::<Connection>(c, |x| {
                        x.a = a;
                        x.b = b;
                    })?;
                }
            }
            _ => {
                let port = [&conn.a, &conn.b]
                    .into_iter()
                    .find_map(|e| match e {
                        Endpoint::Cp(g, idx) if *g == f.id && map(*idx).is_none() => Some(designation(*idx)),
                        _ => None,
                    })
                    .unwrap_or_default();
                tx.delete((Kind::Connection, c.cast()))?;
                dropped.push(port);
            }
        }
    }
    // Medium definitions.
    let defs: Vec<std::sync::Arc<MediumDef>> = tx
        .iter::<MediumDef>()
        .into_iter()
        .filter(|d| matches!(d.at, Endpoint::Cp(g, _) if g == f.id))
        .collect();
    for d in defs {
        let Endpoint::Cp(_, idx) = d.at else { continue };
        match map(idx) {
            Some(n) if n != idx => tx.update::<MediumDef>(d.id, |x| x.at = Endpoint::Cp(f.id, n))?,
            Some(_) => {}
            None => tx.delete((Kind::MediumDef, d.id.cast()))?,
        }
    }
    // Representations: a symbol CP drawing a CP at another index gets a pin-map entry.
    for r in tx.reps_of_function(f.id) {
        let Some(rep) = tx.get::<Representation>(r) else {
            continue;
        };
        let Some(symbol) = tx.get::<LibCopy<SymbolDef>>(rep.symbol) else {
            continue;
        };
        let mut pins: Vec<PinMapEntry> = rep.pin_map.iter().cloned().collect();
        for s in symbol.item.cps.iter() {
            if pins.iter().any(|p| p.symbol_cp == s.idx) {
                continue;
            }
            if let Some(n) = map(s.fn_cp).filter(|n| *n != s.fn_cp) {
                pins.push(PinMapEntry {
                    symbol_cp: s.idx,
                    function: f.id,
                    idx: n,
                    extra: Extra::new(),
                });
            }
        }
        for p in pins.iter_mut().filter(|p| p.function == f.id) {
            if let Some(n) = map(p.idx) {
                p.idx = n;
            }
        }
        let pins = KeyedList::from_vec(pins).expect("one entry per symbol CP");
        if pins != rep.pin_map {
            tx.update::<Representation>(r, |x| x.pin_map = pins)?;
        }
    }
    // The function: its definition, its overrides of ports that remain.
    let overrides: Vec<CpOverride> = f
        .cp_overrides
        .iter()
        .filter_map(|o| map(o.idx).map(|n| CpOverride { idx: n, ..o.clone() }))
        .collect();
    tx.update::<Function>(f.id, |x| {
        x.def = def;
        x.cp_overrides = KeyedList::from_vec(overrides).expect("one override per CP");
    })?;
    tx.notice(format!(
        "{name} has the valve function of its new part; exchange the symbol to draw the new valve type"
    ));
    for port in dropped {
        tx.notice(format!(
            "the line at port {port} of {name} went: the new valve has no port {port}"
        ));
    }
    Ok(())
}
