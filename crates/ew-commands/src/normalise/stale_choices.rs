// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser `stale_choices` (doc 07 §3.3 rule 8): a device's accessory choices answer the accessories its parts
//! declare, so a choice whose accessory no part names any more goes, whichever command changed the parts.
//!
//! - **Trigger.** A device of the transaction that is new or whose `parts` differ from the base snapshot; for a
//!   terminal, also its strip (before and after), whose choices answer its terminals' parts.
//! - **Effect.** Each triggered device keeps the choices whose `of` its parts declare (`ew_derive::accessory_copy`,
//!   the one key a declaration and its choices are matched by): its own part uses' parts and, for a strip, its
//!   terminals' parts.
//!
//! Idempotent: after a run every choice of a triggered device names a declared accessory.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_schema::{Device, Kind, LibCopy, Part};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// Normaliser `stale_choices`.
#[derive(Clone, Copy, Debug, Default)]
pub struct StaleChoices;

impl Normaliser for StaleChoices {
    fn id(&self) -> &'static str {
        "stale_choices"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let mut triggered: BTreeSet<Id<Device>> = BTreeSet::new();
        for key in tx.changed_of(Kind::Device) {
            let Some(dev) = tx.get::<Device>(key.1.cast()) else {
                continue;
            };
            let before = tx
                .base_row(&key)
                .and_then(|r| r.get::<Device>().cloned());
            if before.as_ref().is_some_and(|b| b.parts == dev.parts) {
                continue;
            }
            triggered.insert(dev.id);
            let strips = [dev.aspects.terminal.as_ref(), before.as_ref().and_then(|b| b.aspects.terminal.as_ref())];
            triggered.extend(strips.into_iter().flatten().map(|t| t.strip));
        }
        let mut changed = false;
        for device in triggered {
            changed |= drop_stale(tx, device)?;
        }
        Ok(changed)
    }
}

/// Drops the choices of `device` whose accessory no part of it names: its own part uses' parts and, for a strip, its
/// terminals' parts. Whether it dropped one.
fn drop_stale(tx: &mut TxBuilder<'_>, device: Id<Device>) -> Result<bool, CmdError> {
    let Some(dev) = tx.get::<Device>(device) else {
        return Ok(false);
    };
    if dev.accessory_choices.is_empty() {
        return Ok(false);
    }
    let mut holders: Vec<Id<Device>> = vec![device];
    if dev.aspects.strip.is_some() {
        holders.extend(tx.terminals_of_strip(device));
    }
    let mut named: BTreeSet<Id<Part>> = BTreeSet::new();
    for h in holders {
        let Some(holder) = tx.get::<Device>(h) else {
            continue;
        };
        for u in &holder.parts {
            if let Some(copy) = tx.get::<LibCopy<Part>>(u.part.copy) {
                named.extend(copy.item.accessories.iter().map(ew_derive::accessory_copy));
            }
        }
    }
    if dev.accessory_choices.iter().all(|c| named.contains(&c.of)) {
        return Ok(false);
    }
    tx.update::<Device>(device, |d| d.accessory_choices.retain(|c| named.contains(&c.of)))?;
    Ok(true)
}
