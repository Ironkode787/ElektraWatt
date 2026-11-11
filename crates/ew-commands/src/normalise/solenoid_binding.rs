// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #13, solenoid binding (N2 = N-FL-3; doc 06 §3.2 row 13, doc 10 §3.3, doc 11 §4.2;
//! FR-PLC-051).
//!
//! A module whose part carries a solenoid-position map (`PlcSection.positions`, doc 10 §2.6) binds the coil
//! of each solenoid that a valve child declares to the channel the map assigns:
//!
//! - **Pairs.** For the valve child at position q (1-based; the map's index is p = q − 1; of two children at
//!   one position the lower ID) and each map entry s: the channel number `ew_derive::solenoid_channel(s, p)`,
//!   below the part's channel count, and the valve's coil CP by the rule of `ew_derive::solenoid_coil`
//!   (the valve declares s when s is in `fluid.solenoids` of its valve part; the coil CP is the first
//!   `ew_derive::coil_cp` among its functions in ID order). The single evaluation of the map is the one of
//!   `ew_derive::SolenoidPositions`, so the derivation's step 4 and this normaliser agree.
//! - **Trigger.** A pair is considered when its valve is new in the transaction, moved (parent or position
//!   changed), changed its parts, or gained a function; or when the pair's channel is new in the
//!   transaction (N1 created the module's channels after the valves existed, as the golden builder does).
//!   Pairs of untouched valves and old channels are never reconsidered, so an explicit unbinding stays
//!   (accepted as a deviation from FR-PLC-051, which the design pass rewords; a
//!   moved valve whose coil stays bound elsewhere gets a notice).
//! - **Effect.** The channel takes `binding = Cp(coil)` when the channel is unbound and no channel binds the
//!   coil. An explicit binding wins: a coil that a channel already binds is left as it is (FR-PLC-051), and a
//!   channel bound to something else keeps its binding while the coil stays unbound, with a notice (EW-FL-002
//!   reports the unbound solenoid). A map entry that does not resolve at the position is noticed for a
//!   triggered valve. Ties break by ID; the modules run in ID order.
//! - **Leaving the module.** Before the pairs,
//!   every device of the transaction whose parent changed (by `assembly.attach`, a paste or any other path) is
//!   checked: each channel bound to a CP of one of its functions whose module it is no longer on
//!   (`commands::is_on_module`: the module itself or one of its descendants) is unbound in the same transaction, with
//!   the notice "channel N of M is unbound: D left the module" (N the channel number, M the module, D the device). A
//!   move within the module keeps the binding. The check scans the device's bound channels, not its old module,
//!   because the pairs are keyed by the new parent. The descendants of a moved device (a sub-assembly) are checked
//!   alike, through the parent index: "channel N of M is unbound: V left the module with D" (V the descendant,
//!   D the moved device).
//! - **Children**: a module's valve children come from the parent index (`TxBuilder::children_of`), not a
//!   scan of the device table, so a prepared transaction re-runs when a child is added meanwhile.
//! - **Notices** name records in words (`crate::words`), never by ID.
//!
//! Idempotent: a second run finds each coil bound and changes nothing.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_schema::{Channel, Device, Endpoint, FnDef, Function, Kind, LibCopy, Part};

use crate::commands::is_on_module;
use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::normalise::channel_instantiation::{binding_words, channels_by_number, module_section};
use crate::tx::TxBuilder;
use crate::words;

/// Normaliser #13.
#[derive(Clone, Copy, Debug, Default)]
pub struct SolenoidBinding;

/// Whether a valve device was added, moved or re-parted in this transaction.
fn valve_changed(tx: &TxBuilder<'_>, dev: &Device) -> bool {
    match tx
        .base_row(&(Kind::Device, dev.id.cast()))
        .and_then(|r| r.get::<Device>().cloned())
    {
        None => true,
        Some(b) => b.parent != dev.parent || b.position != dev.position || b.parts != dev.parts,
    }
}

/// Whether a device of the transaction is new or has another parent than in the base snapshot.
fn parent_changed(tx: &TxBuilder<'_>, dev: &Device) -> bool {
    tx.base_row(&(Kind::Device, dev.id.cast()))
        .and_then(|r| r.get::<Device>().cloned())
        .is_none_or(|b| b.parent != dev.parent)
}

/// Unbinds every channel bound to a CP of a function of `moved` or of one of its descendants whose module
/// that device is no longer on, with a notice each. Returns whether it unbound one.
fn unbind_departed(tx: &mut TxBuilder<'_>, moved: &Device) -> Result<bool, CmdError> {
    let mut changed = false;
    for device in subtree(tx, moved.id) {
        changed |= unbind_device(tx, device, moved.id)?;
    }
    Ok(changed)
}

/// `root` and its descendants, through the parent index, `root` first and then breadth first in ID order;
/// a cycle (EW-INV-009) is visited once.
fn subtree(tx: &TxBuilder<'_>, root: Id<Device>) -> Vec<Id<Device>> {
    let mut seen = BTreeSet::from([root]);
    let mut out = vec![root];
    let mut at = 0;
    while let Some(&d) = out.get(at) {
        at += 1;
        for c in tx.children_of(d) {
            if seen.insert(c) {
                out.push(c);
            }
        }
    }
    out
}

/// Unbinds the channels bound to `device`'s functions whose module it is no longer on; `moved` is the device whose
/// parent changed (`device` itself or an ancestor).
fn unbind_device(tx: &mut TxBuilder<'_>, device: Id<Device>, moved: Id<Device>) -> Result<bool, CmdError> {
    let Some(device) = tx.get::<Device>(device) else {
        return Ok(false);
    };
    let mut functions = tx.functions_of_device(device.id);
    functions.sort();
    let mut bound: BTreeSet<Id<Channel>> = BTreeSet::new();
    for f in functions {
        let Some(func) = tx.get::<Function>(f) else {
            continue;
        };
        for idx in crate::commands::common::function_cp_indices(tx, &func, &device) {
            bound.extend(tx.channels_at_cp(f, idx));
        }
    }
    let mut changed = false;
    for id in bound {
        let Some(c) = tx.get::<Channel>(id) else {
            continue;
        };
        let on = is_on_module(Some(device.id), c.module, |d| {
            tx.get::<Device>(d).and_then(|x| x.parent)
        });
        if on {
            continue;
        }
        tx.update::<Channel>(id, |c| c.binding = None)?;
        let with = if moved == device.id {
            String::new()
        } else {
            format!(" with {}", words::device(tx, moved))
        };
        tx.notice(format!(
            "channel {} of {} is unbound: {} left the module{with}",
            c.number,
            words::device(tx, c.module),
            words::device(tx, device.id)
        ));
        changed = true;
    }
    Ok(changed)
}

/// The coil CP of `solenoid` on `valve` (`ew_derive::solenoid_coil`'s rule): `None` unless the valve
/// part (the first part use whose copy has a fluid section with solenoids) declares it; else the first of its
/// functions (in ID order) with an `ew_derive::coil_cp` for it.
fn coil_of(
    tx: &TxBuilder<'_>,
    valve: Id<Device>,
    solenoid: &ew_mltext::Ident,
) -> Option<(Id<Function>, u16)> {
    let dev = tx.get::<Device>(valve)?;
    let declared = dev.parts.iter().find_map(|u| {
        tx.get::<LibCopy<Part>>(u.part.copy)
            .and_then(|c| ew_derive::valve_solenoids(&c.item).map(<[ew_mltext::Ident]>::to_vec))
    })?;
    if !declared.contains(solenoid) {
        return None;
    }
    let mut fns = tx.functions_of_device(valve);
    fns.sort();
    fns.into_iter().find_map(|f| {
        let func = tx.get::<Function>(f)?;
        let def = tx.get::<LibCopy<FnDef>>(func.def)?;
        ew_derive::coil_cp(&def.item, &func.cp_overrides, solenoid).map(|idx| (f, idx))
    })
}

impl Normaliser for SolenoidBinding {
    fn id(&self) -> &'static str {
        "solenoid_binding"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let mut changed = false;
        // A device that left its module releases the channels bound to it, before the pairs bind again
        for key in tx.changed_of(Kind::Device) {
            let Some(dev) = tx.get::<Device>(key.1.cast()) else {
                continue;
            };
            if parent_changed(tx, &dev) {
                changed |= unbind_departed(tx, &dev)?;
            }
        }
        // the triggered valves by module, and the new channel numbers by module
        let mut valves: BTreeMap<Id<Device>, BTreeSet<Id<Device>>> = BTreeMap::new();
        for key in tx.changed_of(Kind::Device) {
            let Some(dev) = tx.get::<Device>(key.1.cast()) else {
                continue;
            };
            match dev.parent {
                Some(module) if valve_changed(tx, &dev) => {
                    valves.entry(module).or_default().insert(dev.id);
                }
                _ => {}
            }
        }
        for key in tx.changed_of(Kind::Function) {
            if !tx.is_new(&key) {
                continue;
            }
            let Some(f) = tx.get::<Function>(key.1.cast()) else {
                continue;
            };
            let Some(valve) = f.device else { continue };
            if let Some(module) = tx.get::<Device>(valve).and_then(|d| d.parent) {
                valves.entry(module).or_default().insert(valve);
            }
        }
        let mut fresh: BTreeMap<Id<Device>, BTreeSet<u16>> = BTreeMap::new();
        for key in tx.changed_of(Kind::Channel) {
            if !tx.is_new(&key) {
                continue;
            }
            if let Some(c) = tx.get::<Channel>(key.1.cast()) {
                fresh.entry(c.module).or_default().insert(c.number);
            }
        }
        let modules: BTreeSet<Id<Device>> = valves.keys().chain(fresh.keys()).copied().collect();
        for module in modules {
            let Some(dev) = tx.get::<Device>(module) else {
                continue;
            };
            let Some(section) = module_section(tx, &dev) else {
                continue;
            };
            let Some(map) = section.positions else {
                continue;
            };
            let count = section.channels.as_ref().map_or(0, |c| c.count);
            let triggered = valves.remove(&module).unwrap_or_default();
            let new_numbers = fresh.remove(&module).unwrap_or_default();
            // the child at each position, the lower ID first
            let mut at: BTreeMap<u32, Id<Device>> = BTreeMap::new();
            // The parent index, in ID order
            let children: Vec<_> = tx
                .children_of(module)
                .into_iter()
                .filter_map(|c| tx.get::<Device>(c))
                .collect();
            for c in children {
                if let Some(p) = c.position {
                    at.entry(p).or_insert(c.id);
                }
            }
            let channels = channels_by_number(tx, module);
            for position in 1..=u32::from(map.count) {
                let Some(&valve) = at.get(&position) else {
                    continue;
                };
                let valve_triggered = triggered.contains(&valve);
                for entry in &map.map {
                    let Some(coil) = coil_of(tx, valve, &entry.solenoid) else {
                        continue; // the valve does not declare this solenoid
                    };
                    let Some(number) =
                        ew_derive::solenoid_channel(entry, position - 1).filter(|n| *n < count)
                    else {
                        if valve_triggered {
                            tx.notice(format!(
                                "solenoid {} of valve {} stays unbound: the position map of module {} gives no \
                                 channel below {count} at position {position}",
                                entry.solenoid,
                                words::device(tx, valve),
                                words::device(tx, module)
                            ));
                        }
                        continue;
                    };
                    if !valve_triggered && !new_numbers.contains(&number) {
                        continue;
                    }
                    let Some(channel) = channels.get(&number) else {
                        continue; // not instantiated: N1 creates it, and this pair runs again then
                    };
                    // whether some channel binds the coil: an index lookup with read-your-writes
                    if !tx.channels_at_cp(coil.0, coil.1).is_empty() {
                        // An explicit or earlier binding of the coil wins, also
                        // after a move; a move rebinds only coils without a binding, and an explicit rebind
                        // is `plc.bind_channel` (the design pass rewords FR-PLC-051 and doc 10 §3.3). A
                        // triggered valve whose coil stays on another channel than the map's says so.
                        let on_map = tx
                            .get::<Channel>(channel.id)
                            .is_some_and(|c| c.binding == Some(Endpoint::Cp(coil.0, coil.1)));
                        if valve_triggered && !on_map {
                            tx.notice(format!(
                                "solenoid {} of valve {} stays on its channel: the position map of module {} \
                                 assigns channel {number} at position {position} (EW-PLC-012); binding the channel \
                                 again moves it",
                                entry.solenoid,
                                words::device(tx, valve),
                                words::device(tx, module)
                            ));
                        }
                        continue;
                    }
                    let current = tx.get::<Channel>(channel.id).and_then(|c| c.binding.clone());
                    if current.is_some() {
                        tx.notice(format!(
                            "solenoid {} of valve {} stays unbound: channel {number} of module {}, which the \
                             position map assigns, is {} (EW-FL-002)",
                            entry.solenoid,
                            words::device(tx, valve),
                            words::device(tx, module),
                            binding_words(tx, current.as_ref())
                        ));
                        continue;
                    }
                    tx.update::<Channel>(channel.id, |c| c.binding = Some(Endpoint::Cp(coil.0, coil.1)))?;
                    changed = true;
                }
            }
        }
        Ok(changed)
    }
}
