// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a PLC channel dropped on a field device can do (FT-753; doc 10 §3.3, FR-PLC-020): the reads behind the PLC
//! navigator's drop when the connection point under it is not on the channel's card. `plc.bind_channel` binds a
//! channel to a pin of its card or to the coil of a valve on it, and nothing else; the window reaches a field device
//! in the model's own terms:
//!
//! - a device **wired** to a channel pin of the card (through terminals: one potential, [`ew_derive::Potentials`])
//!   binds the channel to that card pin;
//! - a device not wired to the card, when the card serves valve positions (its part has a solenoid-position map,
//!   doc 10 §2.6) and the device has no parent, is **attached** to the card at its first free position
//!   (`assembly.attach`) and then bound where it was dropped;
//! - **neither** is refused in words.
//!
//! The shell asks before it commits; the commands decide.

use std::collections::BTreeSet;

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_schema::{Channel, Device, Function};
use ew_derive::{
    ChannelAddr, ChannelAddresses, ChannelsByCp, CpRef, DeviceTag, FunctionCps, PotentialPartition, Q,
    channel_pin, cp_label, module_part,
};

use crate::Session;

/// A channel pin of a card that a dropped device is wired to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CardPin {
    /// The card's function that has the pin.
    pub function: Uuid,
    /// The pin's CP index.
    pub idx: u16,
    /// The pin's label (`-101A3:4`; `cp_label`).
    pub label: String,
}

/// What a channel dropped on a CP off its card can do ([`Session::off_card_bind`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum OffCardBind {
    /// The dropped device is wired to these channel pins of the card (one or more, in pin order): the channel binds
    /// the pin, when there is exactly one.
    Wired {
        /// The dropped device's tag (`-1S1`).
        device: String,
        /// The card's tag.
        card: String,
        /// The pins.
        pins: Vec<CardPin>,
    },
    /// The device can be attached to the card, a valve terminal, at `position`, and then bound where it was dropped.
    Attach {
        /// The device.
        device: Uuid,
        /// Its tag.
        tag: String,
        /// The card (the channel's module).
        card: Uuid,
        /// The card's tag.
        card_tag: String,
        /// The first free valve position (1-based).
        position: u32,
    },
    /// Neither: the device is not wired to a channel pin of the card and cannot be attached to it.
    Neither {
        /// The device's tag (empty for a function on no device).
        device: String,
        /// The card's tag.
        card: String,
        /// The card serves valve positions, but none is free (or the device sits on another assembly).
        valve_terminal: bool,
    },
}

/// The short tag of a device (`Tag::short`), empty when it has none.
fn tag_of(q: &Q, device: Id<Device>) -> String {
    q.get::<DeviceTag>(device)
        .as_ref()
        .as_ref()
        .map(|t| t.short())
        .unwrap_or_default()
}

/// The channel pins of `module`: the CPs of its own functions whose designation the part's channel → pin map gives
/// for one of its channels (normaliser N3's pins). Empty for a card without the map (a valve terminal's internal
/// channels).
fn channel_pins(q: &Q, module: Id<Device>) -> BTreeSet<(Id<Function>, u16)> {
    let mut out = BTreeSet::new();
    let Some((_, section)) = module_part(q, module) else {
        return out;
    };
    let (Some(pin_of), Some(spec)) = (section.pin_of.as_ref(), section.channels.as_ref()) else {
        return out;
    };
    let names: BTreeSet<String> = (0..spec.count).filter_map(|n| channel_pin(pin_of, n)).collect();
    for f in q.idx().functions_of_device(module) {
        for cp in q.get::<FunctionCps>(f).iter() {
            if names.contains(&cp.designation) {
                out.insert((f, cp.idx));
            }
        }
    }
    out
}

impl Session {
    /// What the channel `channel` dropped on CP `idx` of `function` can do when that CP is off the channel's card
    /// (FT-753; see [`OffCardBind`]); `None` when the function is on the card or one of its children (the plain
    /// `plc.bind_channel` applies), or when a record is missing (the command then answers).
    ///
    /// **Wired** first: the dropped CP's potential, else the potential of any CP of the device's functions, holds
    /// channel pins of the card. A device wired to the card is never attached.
    pub fn off_card_bind(&self, channel: Uuid, function: Uuid, idx: u16) -> Option<OffCardBind> {
        let q = self.query();
        let ch = q.table::<Channel>().get(Id::from_uuid(channel))?;
        let module = ch.module;
        let fid: Id<Function> = Id::from_uuid(function);
        let f = q.functions().get(fid)?;
        let devices = q.devices();
        if ew_commands::commands::is_on_module(f.device, module, |d| devices.get(d).and_then(|x| x.parent)) {
            return None;
        }
        let card_tag = tag_of(&q, module);
        let Some(device) = f.device else {
            return Some(OffCardBind::Neither {
                device: String::new(),
                card: card_tag,
                valve_terminal: false,
            });
        };
        let tag = tag_of(&q, device);
        let pins = channel_pins(&q, module);
        if !pins.is_empty() {
            let partition = q.get::<PotentialPartition>(());
            let wired =
                |cps: &mut dyn Iterator<Item = (Id<Function>, u16)>| -> BTreeSet<(Id<Function>, u16)> {
                    let mut out = BTreeSet::new();
                    for (f, i) in cps {
                        let Some(p) = partition.potential_at(CpRef::new(f, i).key()) else {
                            continue;
                        };
                        out.extend(
                            p.cps
                                .iter()
                                .map(|c| (c.function, c.idx))
                                .filter(|c| pins.contains(c)),
                        );
                    }
                    out
                };
            let mut found = wired(&mut std::iter::once((fid, idx)));
            if found.is_empty() {
                let all: Vec<(Id<Function>, u16)> = q
                    .idx()
                    .functions_of_device(device)
                    .flat_map(|f| {
                        q.get::<FunctionCps>(f)
                            .iter()
                            .map(|c| (f, c.idx))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                found = wired(&mut all.into_iter());
            }
            if !found.is_empty() {
                let mut pins: Vec<CardPin> = found
                    .into_iter()
                    .map(|(f, i)| CardPin {
                        function: f.as_uuid(),
                        idx: i,
                        label: cp_label(&q, CpRef::new(f, i)),
                    })
                    .collect();
                pins.sort_by(|a, b| {
                    crate::output::nav_natural_key(&a.label)
                        .cmp(&crate::output::nav_natural_key(&b.label))
                        .then_with(|| (a.function, a.idx).cmp(&(b.function, b.idx)))
                });
                return Some(OffCardBind::Wired {
                    device: tag,
                    card: card_tag,
                    pins,
                });
            }
        }
        // Not wired: a valve terminal takes a device that sits on no assembly at its first free position.
        let positions = module_part(&q, module).and_then(|(_, s)| s.positions);
        if let Some(map) = positions {
            let on_assembly = devices.get(device).is_some_and(|d| d.parent.is_some());
            let taken: BTreeSet<u32> = devices
                .iter()
                .filter(|(_, d)| d.parent == Some(module))
                .filter_map(|(_, d)| d.position)
                .collect();
            let free = (1..=u32::from(map.count)).find(|p| !taken.contains(p));
            if let (false, Some(position)) = (on_assembly, free) {
                return Some(OffCardBind::Attach {
                    device: device.as_uuid(),
                    tag,
                    card: module.as_uuid(),
                    card_tag,
                    position,
                });
            }
            return Some(OffCardBind::Neither {
                device: tag,
                card: card_tag,
                valve_terminal: true,
            });
        }
        Some(OffCardBind::Neither {
            device: tag,
            card: card_tag,
            valve_terminal: false,
        })
    }

    /// The words of a channel in a sentence: its printed address (`Q0.2`), else `channel <n> of <card>`.
    pub fn channel_words(&self, channel: Uuid) -> Option<String> {
        let q = self.query();
        let ch = q.table::<Channel>().get(Id::from_uuid(channel))?;
        let address = q
            .get::<ChannelAddresses>(ch.module)
            .iter()
            .find(|e| e.channel == ch.id)
            .and_then(|e| match &e.address {
                ChannelAddr::At { text, .. } => Some(text.clone()),
                _ => None,
            });
        Some(address.unwrap_or_else(|| format!("channel {} of {}", ch.number, tag_of(&q, ch.module))))
    }

    /// The channels other than `except` bound to CP `idx` of `function`, in ID order: the ones `plc.bind_channel`
    /// unbinds when it binds `except` there (FR-PLC-005).
    pub fn channels_bound_at(&self, function: Uuid, idx: u16, except: Uuid) -> Vec<Uuid> {
        let q = self.query();
        let index = q.get::<ChannelsByCp>(());
        index
            .at_cp(Id::from_uuid(function), idx)
            .map(|c| c.as_uuid())
            .filter(|c| *c != except)
            .collect()
    }

    /// The label of CP `idx` of `function` (`-101A3:4`; `cp_label`), empty for an unknown function.
    pub fn cp_text(&self, function: Uuid, idx: u16) -> String {
        cp_label(&self.query(), CpRef::new(Id::from_uuid(function), idx))
    }
}
