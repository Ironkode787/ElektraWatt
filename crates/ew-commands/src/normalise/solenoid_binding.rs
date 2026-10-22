// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #13, solenoid binding (N2 = N-FL-3; doc 06 §3.2 row 13, doc 10 §3.3, doc 11 §4.2; FT-275,
//! FR-PLC-051).
//!
//! A module whose part carries a solenoid-position map (`PlcSection.positions`, doc 10 §2.6) binds the coil
//! of each solenoid that a valve child declares to the channel the map assigns:
//!
//! - **Pairs.** For the valve child at position q (1-based; the map's index is p = q − 1; of two children at
//!   one position the lower ID) and each map entry s: the channel number `ew_derive::solenoid_channel(s, p)`,
//!   below the part's channel count, and the valve's coil CP `ew_derive::coil_cp_idx` over its functions in
//!   ID order (a valve declares s when one of its functions has a non-physical CP named s, until N-FL-1 and
//!   the valve part's typed `solenoids` exist). The single evaluation of the map is the one of
//!   `ew_derive::SolenoidPositions`, so the derivation's step 4 and this normaliser agree.
//! - **Trigger.** A pair is considered when its valve is new in the transaction, moved (parent or position
//!   changed), changed its parts, or gained a function; or when the pair's channel is new in the
//!   transaction (N1 created the module's channels after the valves existed, as the golden builder does).
//!   Pairs of untouched valves and old channels are never reconsidered, so an explicit unbinding stays
//!   (owner ruling, cycle 18: accepted as a deviation from FR-PLC-051, which the design pass rewords; a
//!   moved valve whose coil stays bound elsewhere gets a notice).
//! - **Effect.** The channel takes `binding = Cp(coil)` when the channel is unbound and no channel binds the
//!   coil. An explicit binding wins: a coil that a channel already binds is left as it is (FR-PLC-051), and a
//!   channel bound to something else keeps its binding while the coil stays unbound, with a notice (EW-FL-002
//!   reports the unbound solenoid). A map entry that does not resolve at the position is noticed for a
//!   triggered valve. Ties break by ID; the modules run in ID order.
//!
//! Idempotent: a second run finds each coil bound and changes nothing.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_schema::{Channel, Device, Endpoint, FnDef, Function, Kind, LibCopy};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::normalise::channel_instantiation::{binding_text, channels_by_number, module_section};
use crate::tx::TxBuilder;

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

/// The coil CP of `solenoid` on `valve`: the first of its functions (in ID order) whose FnDef copy has a
/// non-physical CP named `solenoid`.
fn coil_of(
    tx: &TxBuilder<'_>,
    valve: Id<Device>,
    solenoid: &ew_mltext::Ident,
) -> Option<(Id<Function>, u16)> {
    let mut fns = tx.functions_of_device(valve);
    fns.sort();
    fns.into_iter().find_map(|f| {
        let def = tx.get::<LibCopy<FnDef>>(tx.get::<Function>(f)?.def)?;
        ew_derive::coil_cp_idx(&def.item, solenoid).map(|idx| (f, idx))
    })
}

impl Normaliser for SolenoidBinding {
    fn id(&self) -> &'static str {
        "solenoid_binding"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
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
        if modules.is_empty() {
            return Ok(false);
        }
        let mut changed = false;
        // the CPs some channel binds, read once when first needed
        let mut bound: Option<BTreeSet<(Id<Function>, u16)>> = None;
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
            let mut children: Vec<_> = tx
                .iter::<Device>()
                .into_iter()
                .filter(|d| d.parent == Some(module))
                .collect();
            children.sort_by_key(|d| d.id);
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
                                "solenoid {} of valve {valve} stays unbound: the position map of module \
                                 {module} gives no channel below {count} at position {position}",
                                entry.solenoid
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
                    let held = bound.get_or_insert_with(|| {
                        tx.iter::<Channel>()
                            .into_iter()
                            .filter_map(|c| match c.binding {
                                Some(Endpoint::Cp(f, idx)) => Some((f, idx)),
                                _ => None,
                            })
                            .collect()
                    });
                    if held.contains(&coil) {
                        // Owner ruling (cycle 18): an explicit or earlier binding of the coil wins, also
                        // after a move; a move rebinds only coils without a binding, and an explicit rebind
                        // is `plc.bind_channel` (the design pass rewords FR-PLC-051 and doc 10 §3.3). A
                        // triggered valve whose coil stays on another channel than the map's says so.
                        let on_map = tx
                            .get::<Channel>(channel.id)
                            .is_some_and(|c| c.binding == Some(Endpoint::Cp(coil.0, coil.1)));
                        if valve_triggered && !on_map {
                            tx.notice(format!(
                                "solenoid {} of valve {valve} stays on its channel: the position map of module \
                                 {module} assigns channel {number} at position {position} (EW-PLC-012); \
                                 `plc.bind_channel` rebinds it",
                                entry.solenoid
                            ));
                        }
                        continue;
                    }
                    let current = tx.get::<Channel>(channel.id).and_then(|c| c.binding.clone());
                    if current.is_some() {
                        tx.notice(format!(
                            "solenoid {} of valve {valve} stays unbound: channel {number} of module {module}, \
                             which the position map assigns, is {} (EW-FL-002)",
                            entry.solenoid,
                            binding_text(current.as_ref())
                        ));
                        continue;
                    }
                    held.insert(coil);
                    tx.update::<Channel>(channel.id, |c| c.binding = Some(Endpoint::Cp(coil.0, coil.1)))?;
                    changed = true;
                }
            }
        }
        Ok(changed)
    }
}
