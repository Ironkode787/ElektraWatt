// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `rep.join_device` (`JoinDevice`, WP-S2.18 D3, FT-628, FR-DEV-038): a placed representation re-targeted to another
//! device after placing, its wires kept.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_model::EndpointKey;
use ew_schema::{
    Bond, Channel, Connection, ContactKind, Device, DeviceLink, DeviceState, Endpoint, FnDef, FnRole,
    Function, FunctionKind, Kind, LibCopy, Part, PotentialDef, Representation, TagScheme, TemplateFn,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::UuidS;
use crate::tx::TxBuilder;

/// `rep.join_device` (`JoinDevice`, WP-S2.18 D3, FT-628): the placed representation `rep` joins `device` in one
/// transaction, at the same page, position and orientation, its wires kept.
///
/// It takes one of D1's two ways:
///
/// - **A slot first.** When `device` has an unplaced normal function of the representation's FnDef (a part's template
///   slot, FR-UI-017; the one of the lowest template entry, then the lowest ID), the representation draws that
///   function, which keeps its role and template entry; every connection, potential definition, bond and channel
///   binding or target at the old function moves to it, CP for CP (the FnDef is the same, so the CP indices line up),
///   and the representation's pin map follows.
/// - **Else a new auxiliary function.** The representation's function moves to `device` as an auxiliary function
///   (role aux, no template entry); its record, its CP overrides and everything attached to it stay.
///
/// The source device has exactly one function (the representation's), no part use and no other representation (D3);
/// it is removed in the same transaction, with the old function in the slot case. What the removed records carry
/// besides that (the device's function text, process tag, structure, frozen tag and its other non-default members;
/// in the slot case the old function's CP overrides and its other own members) goes with them: the join goes ahead
/// and the outcome's notices name each dropped member (FX37a). A main function never lands on a
/// device that has one (EW-INV-002): the moved function is auxiliary or fills a slot. Refused, with the pointer: a
/// representation that draws not exactly one normal function of a device, or whose device is a terminal or a strip,
/// has another function, a part or another representation, or takes part in a device link, which the join would lose
/// (`/rep`; remove the link first); the representation's own device (`/device`); a
/// device that cannot take the FnDef by D2's rule (`/device`): it has no unplaced function of the FnDef, and the FnDef
/// is no contact (NO, NC, change-over) or the device's family takes no auxiliary functions (its main function is no
/// coil and none of its parts' templates names the FnDef; terminals and strips never do); a connected CP the function
/// would no longer have on `device` (`/device`; its part's template draws other CPs). Interactive. Outcome: the
/// representation.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JoinDevice {
    /// The placed representation.
    #[schemars(with = "UuidS")]
    pub rep: Id<Representation>,
    /// The device it joins.
    #[schemars(with = "UuidS")]
    pub device: Id<Device>,
}

/// The FnDef copy a template entry names.
fn entry_def(t: &TemplateFn) -> Id<FnDef> {
    ew_ids::v5::lib_copy_id(t.fndef.library.as_uuid(), t.fndef.item.as_uuid())
}

/// The non-default members of the source device that its removal drops, in record order, as the user reads them. Its
/// kind and structure count only where they differ from the target's (the representation reads the target's
/// afterwards).
fn dropped_device_members(device: &Device, target: &Device) -> Vec<&'static str> {
    let mut out = Vec::new();
    let tag = &device.tag;
    if tag.scheme != TagScheme::Default {
        out.push("numbering scheme");
    }
    if tag.frozen.is_some() {
        out.push("frozen tag");
    }
    if device.kind.is_some() && device.kind != target.kind {
        out.push("kind");
    }
    if device.parent.is_some() {
        out.push("parent device");
    }
    if device.position.is_some() {
        out.push("position");
    }
    if !device.structure.is_empty() && device.structure != target.structure {
        out.push("structure");
    }
    if !device.function_text.is_empty() {
        out.push("function text");
    }
    if device.process_tag.is_some() {
        out.push("process tag");
    }
    if !device.tech_data.is_empty() {
        out.push("technical data");
    }
    if device.state != DeviceState::Used {
        out.push("state");
    }
    if device.source_ref.is_some() {
        out.push("import reference");
    }
    let a = &device.aspects;
    if a.cable.is_some() {
        out.push("cable data");
    }
    if a.assembly.is_some() {
        out.push("assembly data");
    }
    if a.plc_station.is_some() {
        out.push("PLC station data");
    }
    if a.plc_module.is_some() {
        out.push("PLC module data");
    }
    if !device.extra.is_empty() || !tag.extra.is_empty() {
        out.push("unknown members");
    }
    out
}

/// The own members of the old function that the slot case drops (its record goes; the slot keeps its own).
fn dropped_function_members(function: &Function) -> Vec<&'static str> {
    let mut out = Vec::new();
    if !function.cp_overrides.is_empty() {
        out.push("connection point overrides");
    }
    if function.source_ref.is_some() {
        out.push("import reference");
    }
    if !function.extra.is_empty() {
        out.push("unknown members");
    }
    out
}

/// `a`, `a and b`, `a, b and c`.
fn listed(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// The verb after [`listed`]: one member name is singular unless it ends in `s` (of the names above, only "unknown
/// members" and "connection point overrides" do, and they are plurals).
fn was_or_were(items: &[&str]) -> &'static str {
    match items {
        [one] if !one.ends_with('s') => "was",
        _ => "were",
    }
}

/// Whether `device` is a terminal or a strip.
fn terminal_or_strip(device: &Device) -> bool {
    device.aspects.terminal.is_some() || device.aspects.strip.is_some()
}

impl Command for JoinDevice {
    const NAME: &'static str = "rep.join_device";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let rep = tx.require::<Representation>(self.rep)?;
        let not_eligible = |why: &str| {
            Err(CmdError::invalid_params(
                Self::NAME,
                "/rep",
                format!(
                    "{why}; a representation joins another device only when its device has exactly one function, \
                     no part and no other representation"
                ),
            ))
        };
        let [function] = rep.functions.as_slice() else {
            return not_eligible("the representation draws not exactly one function");
        };
        let function = *function;
        let f = tx.require::<Function>(function)?;
        if f.kind != FunctionKind::Normal {
            return not_eligible("the representation draws an interruption point or a black box");
        }
        let Some(source) = f.device else {
            return not_eligible("the representation's function has no device");
        };
        let source_device = tx.require::<Device>(source)?;
        if terminal_or_strip(&source_device) {
            return not_eligible("the representation's device is a terminal or a strip");
        }
        if tx.functions_of_device(source) != [function] {
            return not_eligible("the representation's device has another function");
        }
        if !source_device.parts.is_empty() {
            return not_eligible("the representation's device has a part");
        }
        if tx.reps_of_function(function) != [self.rep] {
            return not_eligible("the representation's function has another representation");
        }
        // The join removes the source device, and its device links with it (they cascade): refused, not lost.
        if let Some(link) = tx
            .iter::<DeviceLink>()
            .into_iter()
            .filter(|l| l.from == source || l.to == source)
            .map(|l| l.id)
            .min()
        {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/rep",
                format!(
                    "the representation's device takes part in device link {link}, which the join would lose; \
                     remove the link first"
                ),
            ));
        }
        if self.device == source {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/device",
                format!("the representation belongs to device {source} already; name another device"),
            ));
        }
        let target = tx.require::<Device>(self.device)?;
        // D1: a slot of the FnDef first, the lowest template entry, then the lowest ID.
        let mut slots: Vec<(u16, Id<Function>)> = tx
            .functions_of_device(self.device)
            .into_iter()
            .filter_map(|s| tx.get::<Function>(s).map(|r| (s, r)))
            .filter(|(s, r)| {
                r.def == f.def && r.kind == FunctionKind::Normal && tx.reps_of_function(*s).is_empty()
            })
            .map(|(s, r)| (r.template_entry.unwrap_or(u16::MAX), s))
            .collect();
        slots.sort();
        let slot = slots.first().map(|(_, s)| *s);
        if slot.is_none() {
            // D2: a contact joins a device whose family takes auxiliary functions.
            let contact = tx.get::<LibCopy<FnDef>>(f.def).is_some_and(|d| {
                matches!(
                    d.item.contact_kind,
                    ContactKind::No | ContactKind::Nc | ContactKind::Co
                )
            });
            let coil = tx.functions_of_device(self.device).into_iter().any(|m| {
                tx.get::<Function>(m).is_some_and(|m| {
                    m.role == FnRole::Main
                        && tx
                            .get::<LibCopy<FnDef>>(m.def)
                            .is_some_and(|d| d.item.contact_kind == ContactKind::Coil)
                })
            });
            let templated = target.parts.iter().any(|u| {
                tx.get::<LibCopy<Part>>(u.part.copy)
                    .is_some_and(|p| p.item.function_template.iter().any(|t| entry_def(t) == f.def))
            });
            if !contact || terminal_or_strip(&target) || !(coil || templated) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    "/device",
                    format!(
                        "device {} cannot take function definition {}: it has no unplaced function of it, and {}",
                        self.device,
                        f.def,
                        if contact {
                            "its family takes no auxiliary contacts (no coil, no part naming it)"
                        } else {
                            "only a contact joins as an auxiliary function"
                        }
                    ),
                ));
            }
        }
        // The function the representation draws afterwards, as it will be.
        let after = match slot {
            Some(s) => (*tx.require::<Function>(s)?).clone(),
            None => {
                let mut moved = (*f).clone();
                moved.device = Some(self.device);
                moved.role = FnRole::Aux;
                moved.template_entry = None;
                moved
            }
        };
        // Every attached CP must exist on the new function (the same FnDef, so the indices line up unless a part's
        // template draws other CPs).
        let now = crate::commands::common::function_cp_indices(tx, &after, &target);
        let mut attached: BTreeSet<u16> = BTreeSet::new();
        for c in tx.connections_of_function(function) {
            if let Some(c) = tx.get::<Connection>(c) {
                for e in [&c.a, &c.b] {
                    if let Endpoint::Cp(g, i) = e
                        && *g == function
                    {
                        attached.insert(*i);
                    }
                }
            }
        }
        let fn_cps = crate::commands::common::function_cp_indices(tx, &f, &source_device);
        for i in &fn_cps {
            let at = EndpointKey::Cp(function, *i);
            if !tx.potential_defs_at(at).is_empty()
                || !tx.bonds_at(at).is_empty()
                || !tx.channels_at_cp(function, *i).is_empty()
            {
                attached.insert(*i);
            }
        }
        let lost: Vec<String> = attached.difference(&now).map(u16::to_string).collect();
        if !lost.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/device",
                format!(
                    "on device {} the function has no connection point {}, which is connected now; join another \
                     device or disconnect it first",
                    self.device,
                    lost.join(", ")
                ),
            ));
        }
        match slot {
            Some(slot) => {
                let map = |e: &Endpoint| match e {
                    Endpoint::Cp(g, i) if *g == function => Endpoint::Cp(slot, *i),
                    other => other.clone(),
                };
                // The pin map keeps its keys (the symbol CPs): only the function it names changes.
                let pins: Vec<ew_schema::PinMapEntry> = rep
                    .pin_map
                    .iter()
                    .map(|p| {
                        let mut p = p.clone();
                        if p.function == function {
                            p.function = slot;
                        }
                        p
                    })
                    .collect();
                let pins = ew_schema::KeyedList::from_vec(pins)
                    .map_err(|e| CmdError::internal(format!("{}: the pin map: {e}", Self::NAME)))?;
                tx.update::<Representation>(self.rep, |r| {
                    r.functions = vec![slot];
                    r.pin_map = pins;
                })?;
                for c in tx.connections_of_function(function) {
                    tx.update::<Connection>(c, |c| {
                        c.a = map(&c.a);
                        c.b = map(&c.b);
                    })?;
                }
                let (mut defs, mut bonds) = (BTreeSet::new(), BTreeSet::new());
                for i in &fn_cps {
                    let at = EndpointKey::Cp(function, *i);
                    defs.extend(tx.potential_defs_at(at));
                    bonds.extend(tx.bonds_at(at));
                }
                for d in defs {
                    tx.update::<PotentialDef>(d, |d| d.at = map(&d.at))?;
                }
                for b in bonds {
                    tx.update::<Bond>(b, |b| {
                        b.a = map(&b.a);
                        b.b = map(&b.b);
                    })?;
                }
                let channels: Vec<Id<Channel>> = tx
                    .iter::<Channel>()
                    .into_iter()
                    .filter(|c| {
                        c.target == Some(function)
                            || matches!(c.binding, Some(Endpoint::Cp(g, _)) if g == function)
                    })
                    .map(|c| c.id)
                    .collect();
                for c in channels {
                    tx.update::<Channel>(c, |c| {
                        c.binding = c.binding.as_ref().map(map);
                        if c.target == Some(function) {
                            c.target = Some(slot);
                        }
                    })?;
                }
            }
            None => tx.put(after),
        }
        // The emptied source device goes (in the slot case with the old function, which nothing names now); the
        // notices name what goes with them.
        let mut outcome = Outcome::primary((Kind::Representation, self.rep.cast()));
        let device_members = dropped_device_members(&source_device, &target);
        if !device_members.is_empty() {
            outcome.notices.push(format!(
                "the joined device's {} {} dropped with it",
                listed(&device_members),
                was_or_were(&device_members)
            ));
        }
        if slot.is_some() {
            let function_members = dropped_function_members(&f);
            if !function_members.is_empty() {
                outcome.notices.push(format!(
                    "the old function's {} {} dropped with it",
                    listed(&function_members),
                    was_or_were(&function_members)
                ));
            }
        }
        tx.delete((Kind::Device, source.cast()))?;
        Ok(outcome)
    }
}
