// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #14, module pin binding (N3; doc 06 §3.2 row 14, doc 10 §3.3; FT-275).
//!
//! A channel created in this transaction (by N1 or an import), unbound, on a module whose part carries a
//! channel → pin map (`PlcSection.pin_of`, doc 10 §3.3) takes `binding = Cp(main, pin)`: the CP index
//! `ew_derive::channel_pin(pin_of, number)` on the module device's main function (the lowest ID of its
//! functions with role main). The binding is set only when that CP exists in the function's FnDef copy and
//! no other channel binds it (a CP is in at most one channel, FR-PLC-005); otherwise the channel stays
//! unbound and a notice says why. A channel that is bound already (an explicit binding in the same
//! transaction) is left as it is. Channels that existed before the transaction are never touched, so an
//! explicit unbinding stays.
//!
//! Idempotent: a second run finds each new channel bound, or meets the same notice.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_schema::{Channel, Device, Endpoint, FnDef, FnRole, Function, Kind, LibCopy};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::normalise::channel_instantiation::module_section;
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
            let has_cp = tx
                .get::<LibCopy<FnDef>>(main.def)
                .is_some_and(|d| d.item.cps.get(&pin).is_some());
            if !has_cp {
                tx.notice(format!(
                    "channel {} of module {} stays unbound: its pin {pin} is not a CP of the main function {}",
                    c.number, c.module, main.id
                ));
                continue;
            }
            let held = bound.get_or_insert_with(|| {
                tx.iter::<Channel>()
                    .into_iter()
                    .filter_map(|c| match c.binding {
                        Some(Endpoint::Cp(f, idx)) => Some((f, idx)),
                        _ => None,
                    })
                    .collect()
            });
            if held.contains(&(main.id, pin)) {
                tx.notice(format!(
                    "channel {} of module {} stays unbound: another channel binds its pin {pin} (FR-PLC-005)",
                    c.number, c.module
                ));
                continue;
            }
            held.insert((main.id, pin));
            let endpoint = Endpoint::Cp(main.id, pin);
            tx.update::<Channel>(c.id, |c| c.binding = Some(endpoint))?;
            changed = true;
        }
        Ok(changed)
    }
}
