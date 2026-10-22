// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #3, channel instantiation (N1; doc 06 §3.2, doc 10 §2.4, §3.3; FT-270), v0 subset.
//!
//! Trigger: a device of this transaction that carries the `plc_module` aspect and is new, gained the
//! aspect, or changed its parts or the aspect's `part_use`. Effect: the module's channels `0 … count − 1`
//! of the part's `plc.channels` (the part use `plc_module.part_use`, else the first part use whose part
//! has a channel spec), each `Channel {module, number, kind}` with the ID `v5(module, "channel", number)`
//! (ADR-0004: instantiated records have v5 IDs), so two branches that add the same module create the same
//! records. A channel that exists already is kept as it is (its authored data stays). Surplus channels
//! (a part change to fewer channels) are not removed: that needs the confirmation of FR-PLC-053, which is
//! not built in v0. Idempotent: a second run finds every channel present.

use ew_ids::v5;
use ew_mltext::MlText;
use ew_schema::{AddrSpec, Channel, Device, Extra, Kind, LibCopy, Part, PlcModule};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// Normaliser #3.
#[derive(Clone, Copy, Debug, Default)]
pub struct ChannelInstantiation;

/// Whether the device's module data changed in this transaction: new, aspect added, parts or part use
/// changed.
fn triggered(tx: &TxBuilder<'_>, dev: &Device, module: &PlcModule) -> bool {
    let base = tx
        .base_row(&(Kind::Device, dev.id.cast()))
        .and_then(|r| r.get::<Device>().cloned());
    match base {
        None => true,
        Some(b) => match &b.aspects.plc_module {
            None => true,
            Some(m) => b.parts != dev.parts || m.part_use != module.part_use,
        },
    }
}

impl Normaliser for ChannelInstantiation {
    fn id(&self) -> &'static str {
        "channel_instantiation"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let mut changed = false;
        for key in tx.changed_of(Kind::Device) {
            let Some(dev) = tx.get::<Device>(key.1.cast()) else {
                continue;
            };
            let Some(module) = dev.aspects.plc_module.as_ref() else {
                continue;
            };
            if !triggered(tx, &dev, module) {
                continue;
            }
            let spec_of = |i: usize| {
                dev.parts.get(i).and_then(|u| {
                    tx.get::<LibCopy<Part>>(u.part.copy)
                        .and_then(|p| p.item.sections.plc.as_ref().and_then(|s| s.channels.clone()))
                })
            };
            let spec = match module.part_use {
                Some(i) => spec_of(usize::from(i)),
                None => (0..dev.parts.len()).find_map(spec_of),
            };
            let Some(spec) = spec else {
                continue;
            };
            for number in 0..spec.count {
                let id = v5::plc_channel_id(dev.id, number);
                if tx.exists::<Channel>(id) {
                    continue;
                }
                tx.insert(Channel {
                    id,
                    v: 1,
                    module: dev.id,
                    number,
                    designation: None,
                    kind: spec.kind.clone(),
                    address: AddrSpec::Auto,
                    state: None,
                    symbol: None,
                    comment: MlText::default(),
                    binding: None,
                    target: None,
                    source_ref: None,
                    extra: Extra::new(),
                })?;
                changed = true;
            }
        }
        Ok(changed)
    }
}
