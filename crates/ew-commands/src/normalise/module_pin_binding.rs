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
//! binding in the same transaction) is left as it is. Channels that existed before the transaction are
//! never touched, so an explicit unbinding stays (and a part change does not re-pin them: FT to record).
//!
//! Idempotent: a second run finds each new channel bound, or meets the same notice.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_schema::{Channel, Device, Endpoint, FnDef, FnRole, Function, Kind, LibCopy};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::normalise::channel_instantiation::{module_part_copy, module_section};
use crate::tx::TxBuilder;

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

/// The CP index of the pin named `pin` on the module's main function: the part's CP pattern when it has
/// one, else the main function's FnDef copy; the lowest index with that designation.
fn pin_idx(tx: &TxBuilder<'_>, dev: &Device, main: &Function, pin: &str) -> Option<u16> {
    let pattern = module_part_copy(tx, dev).filter(|p| !p.item.cp_pattern.is_empty());
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

impl Normaliser for ModulePinBinding {
    fn id(&self) -> &'static str {
        "module_pin_binding"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let mut fresh: Vec<std::sync::Arc<Channel>> = tx
            .changed_of(Kind::Channel)
            .into_iter()
            .filter(|k| tx.is_new(k))
            .filter_map(|k| tx.get::<Channel>(k.1.cast()))
            .filter(|c| c.binding.is_none())
            .collect();
        if fresh.is_empty() {
            return Ok(false);
        }
        fresh.sort_by_key(|c| (c.module, c.number, c.id));
        let mut changed = false;
        let mut bound: Option<BTreeSet<(Id<Function>, u16)>> = None;
        for c in fresh {
            let Some(dev) = tx.get::<Device>(c.module) else {
                continue;
            };
            let Some(pin_of) = module_section(tx, &dev).and_then(|s| s.pin_of) else {
                continue; // no physical pins: an internal channel (N2 binds valve coils)
            };
            let Some(pin) = ew_derive::channel_pin(&pin_of, c.number) else {
                tx.notice(format!(
                    "channel {} of module {} stays unbound: its part's pin map gives no pin for it",
                    c.number, c.module
                ));
                continue;
            };
            let Some(main) = main_function(tx, c.module) else {
                tx.notice(format!(
                    "channel {} of module {} stays unbound: the module has no main function to carry pin {pin}",
                    c.number, c.module
                ));
                continue;
            };
            let Some(idx) = pin_idx(tx, &dev, &main, &pin) else {
                tx.notice(format!(
                    "channel {} of module {} stays unbound: no CP of the main function {} is named {pin}",
                    c.number, c.module, main.id
                ));
                continue;
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
            if held.contains(&(main.id, idx)) {
                tx.notice(format!(
                    "channel {} of module {} stays unbound: another channel binds its pin {pin} (FR-PLC-005)",
                    c.number, c.module
                ));
                continue;
            }
            held.insert((main.id, idx));
            let endpoint = Endpoint::Cp(main.id, idx);
            tx.update::<Channel>(c.id, |c| c.binding = Some(endpoint))?;
            changed = true;
        }
        Ok(changed)
    }
}
