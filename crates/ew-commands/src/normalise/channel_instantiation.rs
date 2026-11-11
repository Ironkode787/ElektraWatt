// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #3, channel instantiation (N1; doc 06 §3.2, doc 10 §2.4, §3.3).
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
//! - **Kind**. An existing channel `0 … count − 1` whose kind differs from the spec's takes the
//!   spec's kind, with a notice: the kind is part data instantiated on the channel (doc 10 §2.4, §2.6), not
//!   an authored member, so a part change that turns outputs into inputs changes the channels with it; its
//!   binding stays (a binding to a function of the wrong kind is EW-PLC-004, 1.0). A kind of a newer format
//!   (`ChannelKind::Unknown`) is kept, with a notice, because this build cannot judge it.
//! - **Surplus**. Channels numbered at or above the count (a part change to fewer channels, or to a
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
use crate::text::{Text, text};
use crate::tx::TxBuilder;
use crate::words;

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
/// rule), the one whose ID is `v5(module, "channel", number)`, else the lower ID. An index
/// lookup (`TxBuilder::channels_of_module`), not a scan of the channel table; `ew_derive::ModuleChannelIds`
/// keys the derivation the same way.
pub(crate) fn channels_by_number(tx: &TxBuilder<'_>, module: Id<Device>) -> BTreeMap<u16, Arc<Channel>> {
    let mut out: BTreeMap<u16, Arc<Channel>> = BTreeMap::new();
    // in ID order, so the first channel of a number is the lowest ID, unless the v5 one comes later
    for c in tx
        .channels_of_module(module)
        .into_iter()
        .filter_map(|id| tx.get::<Channel>(id))
    {
        let v5 = ew_ids::v5::plc_channel_id::<Device, Channel>(module, c.number);
        match out.entry(c.number) {
            std::collections::btree_map::Entry::Vacant(e) => {
                e.insert(c);
            }
            std::collections::btree_map::Entry::Occupied(mut e) => {
                if c.id == v5 {
                    e.insert(c);
                }
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

/// A binding as a notice names it, in words: "bound to
/// -201U3-Q1:14" (the CP by its device's tag and its designation), "bound to the junction at x 120, y 45 mm on page 3",
/// "bound to an endpoint of a newer format", "unbound".
pub(crate) fn binding_words(tx: &TxBuilder<'_>, binding: Option<&Endpoint>) -> String {
    match binding {
        Some(Endpoint::Cp(f, idx)) => format!("bound to {}", cp_words(tx, *f, *idx)),
        Some(Endpoint::Junction(j)) => format!("bound to {}", crate::words::junction(tx, *j)),
        Some(_) => "bound to an endpoint of a newer format".to_owned(),
        None => "unbound".to_owned(),
    }
}

/// A CP in words: its function's device tag and its designation (`-201U3-Q1:14`); the designation is the FnDef CP's,
/// else the CP pattern entry's of the device's first part that has it (a black box takes the pattern's first), else
/// "CP <index>" (`-102F2 CP 9`).
pub(crate) fn cp_words(tx: &TxBuilder<'_>, function: Id<ew_schema::Function>, idx: u16) -> Text {
    let tag = crate::words::function(tx, function);
    let f = tx.get::<ew_schema::Function>(function);
    let from_def = f
        .as_ref()
        .and_then(|f| tx.get::<LibCopy<ew_schema::FnDef>>(f.def))
        .and_then(|d| {
            d.item
                .cps
                .iter()
                .find(|c| c.idx == idx)
                .map(|c| c.designation.as_str().to_owned())
        });
    let from_pattern = || {
        let dev = tx.get::<Device>(f.as_ref()?.device?)?;
        dev.parts.iter().find_map(|u| {
            let part = tx.get::<LibCopy<Part>>(u.part.copy)?;
            part.item
                .cp_pattern
                .iter()
                .find(|c| c.idx == idx)
                .map(|c| c.designation.as_str().to_owned())
        })
    };
    let black_box = f
        .as_ref()
        .is_some_and(|f| matches!(f.kind, ew_schema::FunctionKind::BlackBox));
    let designation = if black_box {
        from_pattern().or(from_def)
    } else {
        from_def.or_else(from_pattern)
    };
    match designation {
        Some(d) => text!("refusal-words-cp", tag, d),
        None => text!("refusal-words-cp-index", tag, idx.to_string()),
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
                        return tx.reject(text!(
                            "refusal-channel-identity-taken",
                            number.to_string(),
                            words::device(tx, dev.id),
                            other.number.to_string(),
                            words::device(tx, other.module)
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
                // The part's kind applies to the existing channels below the count
                for c in present
                    .values()
                    .filter(|c| c.number < spec.count && c.kind != spec.kind)
                {
                    // The module by its tag and the binding in words, as N2's notices name them
                    if let ChannelKind::Unknown(_) = c.kind {
                        tx.notice(format!(
                            "channel {} of module {} keeps its kind of a newer format; its part gives {}",
                            c.number,
                            words::device(tx, dev.id),
                            kind_name(&spec.kind)
                        ));
                        continue;
                    }
                    tx.notice(format!(
                        "channel {} of module {} changes from {} to {} after its part; it stays {}",
                        c.number,
                        words::device(tx, dev.id),
                        kind_name(&c.kind),
                        kind_name(&spec.kind),
                        binding_words(tx, c.binding.as_ref())
                    ));
                    let kind = spec.kind.clone();
                    tx.update::<Channel>(c.id, |c| c.kind = kind)?;
                    changed = true;
                }
            }
            // Surplus channels stay until `plc.set_module_part` is confirmed
            let surplus: Vec<&Arc<Channel>> = present.values().filter(|c| c.number >= count).collect();
            if !surplus.is_empty() {
                let list: Vec<String> = surplus
                    .iter()
                    .map(|c| format!("channel {} ({})", c.number, binding_words(tx, c.binding.as_ref())))
                    .collect();
                let (n, noun) = match surplus.len() {
                    1 => ("1".to_owned(), "channel"),
                    n => (n.to_string(), "channels"),
                };
                tx.notice(format!(
                    "module {} keeps {n} surplus {noun} beyond its part's {count}: {}; Set PLC module part removes \
                     them after its preview",
                    words::device(tx, dev.id),
                    list.join(", ")
                ));
            }
        }
        Ok(changed)
    }
}
