// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #14, module pin binding (N3; doc 06 §3.2 row 14, doc 10 §3.3; FT-275).
//!
//! A channel created in this transaction (by N1 or an import), unbound, on a module whose part carries a
//! channel → pin map (`PlcSection.pin_of`, doc 10 §3.3) takes `binding = Cp(main, idx)`: the pin designation
//! `ew_derive::channel_pin(pin_of, number)` ("X2:1") resolved **by name** on the module device's main
//! function (the lowest ID of its functions with role main). The CPs searched are the part's CP pattern when
//! it has one (black boxes and template functions take their CPs from it, doc 07 §4.2), else the CPs of the
//! main function's FnDef copy; the first CP with that designation, by index, is the pin (review ruling,
//! cycle 18: a designation survives a reordering of the FnDef's CPs, an index does not). The binding is set
//! only when the pin resolves and no other channel binds it (a CP is in at most one channel, FR-PLC-005);
//! otherwise the channel stays unbound and a notice says why. A channel that is bound already (an explicit
//! binding in the same transaction) is left as it is. Channels that existed before the transaction are not bound
//! by this rule, so an explicit unbinding stays.
//!
//! **Re-pinning after a part change** (FT-321; WP-S1.21 part 5). A module device of the transaction whose PLC part
//! copy (the rule of [`module_part_copy`]) differs from the one of the base snapshot re-pins its existing channels
//! below the new part's channel count, before the new channels are bound: a channel whose binding is
//! `Cp(main, i)`, where `i` is the pin that the **old** part's `pin_of` gives its number (resolved by designation in
//! the old part's CP pattern, else the main function's FnDef, as above), moves to the pin the new part gives it. It
//! is unbound, with the notice "channel N of M is unbound: part P gives it no pin" (N the channel number, M the
//! module, P the part number), when the new part has no `pin_of` entry for it or no CP of that designation, and with
//! "channel N of M is unbound: another channel binds its pin X (FR-PLC-005)" (X the pin) when a channel that is not
//! re-pinned binds the new pin.
//! The re-pinned channels release their old pins first, so pins that shift between them never collide. A channel
//! bound to anything else (another CP, a coil, a junction) is left as it is, and so are surplus channels (removed
//! only by `plc.set_module_part`'s preview, FR-PLC-053).
//!
//! Notices name records in words (`crate::words`), never by ID.
//!
//! Idempotent: a second run finds each new channel bound, or meets the same notice, and no part change.

use std::sync::Arc;

use ew_ids::Id;
use ew_model::Entity;
use ew_schema::{Channel, Device, Endpoint, FnDef, FnRole, Function, Kind, LibCopy, Part};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::normalise::channel_instantiation::{channels_by_number, module_part_copy, module_section};
use crate::tx::TxBuilder;
use crate::words;

/// Normaliser #14.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModulePinBinding;

/// The main function of a device: the lowest ID among its functions with role main.
fn main_function(tx: &TxBuilder<'_>, device: Id<Device>) -> Option<std::sync::Arc<Function>> {
    let mut fns = tx.functions_of_device(device);
    fns.sort();
    fns.into_iter()
        .filter_map(|f| tx.get::<Function>(f))
        .find(|f| f.role == FnRole::Main)
}

/// The CP index of the pin named `pin` on the module's main function: the CP pattern of `part` (the module's PLC part)
/// when it has one, else the main function's FnDef copy; the lowest index with that designation.
fn pin_idx(tx: &TxBuilder<'_>, part: Option<&LibCopy<Part>>, main: &Function, pin: &str) -> Option<u16> {
    let pattern = part.filter(|p| !p.item.cp_pattern.is_empty());
    match pattern {
        Some(part) => part
            .item
            .cp_pattern
            .iter()
            .filter(|c| c.designation.as_str() == pin)
            .map(|c| c.idx)
            .min(),
        None => tx
            .get::<LibCopy<FnDef>>(main.def)?
            .item
            .cps
            .iter()
            .filter(|c| c.designation.as_str() == pin)
            .map(|c| c.idx)
            .min(),
    }
}

/// The PLC part copy of a module device as the base snapshot held it (the rule of [`module_part_copy`] over base
/// rows), or `None` when the device or its `plc_module` aspect is new.
fn base_part_copy(tx: &TxBuilder<'_>, dev: &Device) -> Option<Arc<LibCopy<Part>>> {
    let base = tx
        .base_row(&(Kind::Device, dev.id.cast()))
        .and_then(|r| r.get::<Device>().cloned())?;
    let module = base.aspects.plc_module.as_ref()?;
    let copy = |i: usize| {
        let id = base.parts.get(i)?.part.copy;
        tx.base_row(&LibCopy::<Part>::key(id))
            .and_then(|r| r.get::<LibCopy<Part>>().cloned())
            .filter(|p| p.item.sections.plc.is_some())
    };
    match module.part_use {
        Some(i) => copy(usize::from(i)),
        None => (0..base.parts.len()).find_map(copy),
    }
}

/// A pin a part gives a channel: its designation and its CP index on the main function.
type Pin = (String, u16);

/// FT-321: re-pins the channels of `dev` after its PLC part changed from `old` to `new` (see the module documentation).
fn repin(
    tx: &mut TxBuilder<'_>,
    dev: &Device,
    old: &LibCopy<Part>,
    new: &LibCopy<Part>,
) -> Result<bool, CmdError> {
    let Some(old_map) = old.item.sections.plc.as_ref().and_then(|s| s.pin_of.clone()) else {
        return Ok(false); // the old part gave no pins: nothing was pinned by it
    };
    let Some(main) = main_function(tx, dev.id) else {
        return Ok(false);
    };
    let new_section = new.item.sections.plc.clone();
    let new_map = new_section.as_ref().and_then(|s| s.pin_of.clone());
    let count = new_section
        .as_ref()
        .and_then(|s| s.channels.as_ref())
        .map_or(0, |c| c.count);
    // the channels on their old pins and the pin the new part gives each (None: no pin)
    let mut moves: Vec<(Arc<Channel>, Option<Pin>)> = Vec::new();
    for (number, c) in channels_by_number(tx, dev.id) {
        if number >= count {
            continue; // surplus: removed by `plc.set_module_part`, never re-pinned
        }
        let Some(old_idx) =
            ew_derive::channel_pin(&old_map, number).and_then(|pin| pin_idx(tx, Some(old), &main, &pin))
        else {
            continue;
        };
        if c.binding != Some(Endpoint::Cp(main.id, old_idx)) {
            continue; // bound elsewhere, or unbound: left as it is
        }
        let target = new_map
            .as_ref()
            .and_then(|m| ew_derive::channel_pin(m, number))
            .and_then(|pin| pin_idx(tx, Some(new), &main, &pin).map(|i| (pin, i)));
        if target.as_ref().is_some_and(|(_, i)| *i == old_idx) {
            continue; // the same pin
        }
        moves.push((c, target));
    }
    if moves.is_empty() {
        return Ok(false);
    }
    // release every old pin first, so pins that shift between the re-pinned channels never collide
    for (c, _) in &moves {
        tx.update::<Channel>(c.id, |c| c.binding = None)?;
    }
    let module = words::device(tx, dev.id);
    for (c, target) in moves {
        match target {
            None => tx.notice(format!(
                "channel {} of {module} is unbound: part {} gives it no pin",
                c.number,
                new.item.part_number.as_str()
            )),
            Some((pin, idx)) if !tx.channels_at_cp(main.id, idx).is_empty() => tx.notice(format!(
                "channel {} of {module} is unbound: another channel binds its pin {pin} (FR-PLC-005)",
                c.number
            )),
            Some((_, idx)) => {
                tx.update::<Channel>(c.id, |c| c.binding = Some(Endpoint::Cp(main.id, idx)))?;
            }
        }
    }
    Ok(true)
}

impl Normaliser for ModulePinBinding {
    fn id(&self) -> &'static str {
        "module_pin_binding"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let mut changed = false;
        // FT-321: the modules whose PLC part changed re-pin their existing channels, in ID order
        for key in tx.changed_of(Kind::Device) {
            let Some(dev) = tx.get::<Device>(key.1.cast()) else {
                continue;
            };
            if dev.aspects.plc_module.is_none() {
                continue;
            }
            let (Some(old), Some(new)) = (base_part_copy(tx, &dev), module_part_copy(tx, &dev)) else {
                continue;
            };
            if old != new {
                changed |= repin(tx, &dev, &old, &new)?;
            }
        }
        let mut fresh: Vec<std::sync::Arc<Channel>> = tx
            .changed_of(Kind::Channel)
            .into_iter()
            .filter(|k| tx.is_new(k))
            .filter_map(|k| tx.get::<Channel>(k.1.cast()))
            .filter(|c| c.binding.is_none())
            .collect();
        fresh.sort_by_key(|c| (c.module, c.number, c.id));
        for c in fresh {
            let Some(dev) = tx.get::<Device>(c.module) else {
                continue;
            };
            let Some(pin_of) = module_section(tx, &dev).and_then(|s| s.pin_of) else {
                continue; // no physical pins: an internal channel (N2 binds valve coils)
            };
            let Some(pin) = ew_derive::channel_pin(&pin_of, c.number) else {
                let why = ew_derive::pin_pattern_error(&pin_of, c.number)
                    .map_or_else(String::new, |e| format!(" ({e})"));
                tx.notice(format!(
                    "channel {} of {} stays unbound: its part's pin map gives no pin for it{why}",
                    c.number,
                    words::device(tx, c.module)
                ));
                continue;
            };
            let Some(main) = main_function(tx, c.module) else {
                tx.notice(format!(
                    "channel {} of {} stays unbound: the module has no main function to carry pin {pin}",
                    c.number,
                    words::device(tx, c.module)
                ));
                continue;
            };
            let part = module_part_copy(tx, &dev);
            let Some(idx) = pin_idx(tx, part.as_deref(), &main, &pin) else {
                tx.notice(format!(
                    "channel {} of {} stays unbound: no CP of its main function is named {pin}",
                    c.number,
                    words::device(tx, c.module)
                ));
                continue;
            };
            // whether some channel binds the pin: an index lookup with read-your-writes (FT-319)
            if !tx.channels_at_cp(main.id, idx).is_empty() {
                tx.notice(format!(
                    "channel {} of {} stays unbound: another channel binds its pin {pin} (FR-PLC-005)",
                    c.number,
                    words::device(tx, c.module)
                ));
                continue;
            }
            let endpoint = Endpoint::Cp(main.id, idx);
            tx.update::<Channel>(c.id, |c| c.binding = Some(endpoint))?;
            changed = true;
        }
        Ok(changed)
    }
}
