// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #3, channel instantiation (N1; doc 06 §3.2, doc 10 §2.4, §3.3; FT-270, FT-276, FT-277).
//!
//! Trigger: a device of this transaction that carries the `plc_module` aspect and is new, gained the
//! aspect, or changed its parts or the aspect's `part_use`. The module's PLC section is the one of the part
//! use `plc_module.part_use`, else of the first part use whose part has a PLC section: the rule of
//! `ew_derive::module_part`, so N1 instantiates exactly the channels the derivation reads.
//!
//! Effect:
//! - **Create.** The channels `0 … count − 1` of the section's `channels` spec that the module lacks, each
//!   `Channel {module, number, kind}` with the ID `v5(module, "channel", number)` (ADR-0004: instantiated
//!   records have v5 IDs), so two branches that add the same module create the same records. A channel of
//!   that module and number that exists already, whatever its ID, is kept with its authored members.
//! - **Kind** (FT-276). An existing channel `0 … count − 1` whose kind differs from the spec's takes the
//!   spec's kind, with a notice: the kind is part data instantiated on the channel (doc 10 §2.4, §2.6), not
//!   an authored member, so a part change that turns outputs into inputs changes the channels with it; its
//!   binding stays (a binding to a function of the wrong kind is EW-PLC-004, 1.0). A kind of a newer format
//!   (`ChannelKind::Unknown`) is kept, with a notice, because this build cannot judge it.
//! - **Surplus** (FT-277). Channels numbered at or above the count (a part change to fewer channels, or to a
//!   part without channels) are never removed here: FR-PLC-053 removes them only after a confirmation that
//!   lists them, which is the preview of the Bulk command `plc.set_module_part`. N1 names them in a
//!   notice. The derivation reads only the channels below the count, so a surplus channel prints nowhere.
//!
//! Idempotent: a second run finds every channel present with the spec's kind, and repeats only notices,
//! which the transaction records once.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_ids::{Id, v5};
use ew_mltext::MlText;
use ew_schema::{AddrSpec, Channel, ChannelKind, Device, Endpoint, Extra, Kind, LibCopy, Part, PlcSection};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// Normaliser #3.
#[derive(Clone, Copy, Debug, Default)]
pub struct ChannelInstantiation;

/// The PLC section of a module device: the part use named by `plc_module.part_use`, else the first part use
/// whose part has a PLC section (the rule of `ew_derive::module_part`). `None` for a device without the
/// `plc_module` aspect.
pub(crate) fn module_section(tx: &TxBuilder<'_>, dev: &Device) -> Option<PlcSection> {
    module_part_copy(tx, dev).and_then(|p| p.item.sections.plc.clone())
}

/// The part copy that supplies a module device's PLC section (see [`module_section`]).
pub(crate) fn module_part_copy(tx: &TxBuilder<'_>, dev: &Device) -> Option<Arc<LibCopy<Part>>> {
    let module = dev.aspects.plc_module.as_ref()?;
    let with_plc = |i: usize| {
        dev.parts
            .get(i)
            .and_then(|u| tx.get::<LibCopy<Part>>(u.part.copy))
            .filter(|p| p.item.sections.plc.is_some())
    };
    match module.part_use {
        Some(i) => with_plc(usize::from(i)),
        None => (0..dev.parts.len()).find_map(with_plc),
    }
}

/// The channels of a module by number; of two channels with one number (an import that bypassed the v5
/// rule), the lower ID.
pub(crate) fn channels_by_number(tx: &TxBuilder<'_>, module: Id<Device>) -> BTreeMap<u16, Arc<Channel>> {
    let mut out: BTreeMap<u16, Arc<Channel>> = BTreeMap::new();
    for c in tx.iter::<Channel>().into_iter().filter(|c| c.module == module) {
        match out.get(&c.number) {
            Some(held) if held.id <= c.id => {}
            _ => {
                out.insert(c.number, c);
            }
        }
    }
    out
}

/// A channel kind as the files spell it (`do`).
pub(crate) fn kind_name(kind: &ChannelKind) -> String {
    match serde_json::to_value(kind) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => format!("{kind:?}"),
    }
}

/// A binding as a notice names it.
pub(crate) fn binding_text(binding: Option<&Endpoint>) -> String {
    match binding {
        Some(Endpoint::Cp(f, idx)) => format!("bound to CP {idx} of function {f}"),
        Some(Endpoint::Junction(j)) => format!("bound to junction {j}"),
        Some(_) => "bound to an endpoint of a newer format".to_owned(),
        None => "unbound".to_owned(),
    }
}

/// Whether the device's module data changed in this transaction: new, aspect added, parts or part use
/// changed.
fn triggered(tx: &TxBuilder<'_>, dev: &Device) -> bool {
    let Some(module) = dev.aspects.plc_module.as_ref() else {
        return false;
    };
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
            if !triggered(tx, &dev) {
                continue;
            }
            let spec = module_section(tx, &dev).and_then(|s| s.channels);
            let count = spec.as_ref().map_or(0, |s| s.count);
            // existence is keyed on (module, number), not on the v5 ID: a module pasted or imported with
            // other channel IDs keeps its channels and gets no second channel of a number
            let present = channels_by_number(tx, dev.id);
            if let Some(spec) = &spec {
                for number in (0..spec.count).filter(|n| !present.contains_key(n)) {
                    let id = v5::plc_channel_id(dev.id, number);
                    if let Some(other) = tx.get::<Channel>(id) {
                        return tx.reject(format!(
                            "channel {number} of module {} would take the ID {id}, which channel {} of module \
                             {} holds",
                            dev.id, other.number, other.module
                        ));
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
                // FT-276: the part's kind applies to the existing channels below the count
                for c in present
                    .values()
                    .filter(|c| c.number < spec.count && c.kind != spec.kind)
                {
                    if let ChannelKind::Unknown(_) = c.kind {
                        tx.notice(format!(
                            "channel {} of module {} keeps its kind of a newer format; its part gives `{}`",
                            c.number,
                            dev.id,
                            kind_name(&spec.kind)
                        ));
                        continue;
                    }
                    tx.notice(format!(
                        "channel {} of module {}: kind `{}` → `{}` from its part (it stays {})",
                        c.number,
                        dev.id,
                        kind_name(&c.kind),
                        kind_name(&spec.kind),
                        binding_text(c.binding.as_ref())
                    ));
                    let kind = spec.kind.clone();
                    tx.update::<Channel>(c.id, |c| c.kind = kind)?;
                    changed = true;
                }
            }
            // FT-277: surplus channels stay until `plc.set_module_part` is confirmed
            let surplus: Vec<&Arc<Channel>> = present.values().filter(|c| c.number >= count).collect();
            if !surplus.is_empty() {
                let list: Vec<String> = surplus
                    .iter()
                    .map(|c| format!("{} ({})", c.number, binding_text(c.binding.as_ref())))
                    .collect();
                tx.notice(format!(
                    "module {} keeps {} surplus channel(s) beyond its part's {count}: {}; \
                     `plc.set_module_part` removes them after its preview (FR-PLC-053)",
                    dev.id,
                    surplus.len(),
                    list.join(", ")
                ));
            }
        }
        Ok(changed)
    }
}
