// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `rep.join_device` (`JoinDevice`, FR-DEV-038): a placed representation re-targeted to another
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
use ew_l10n::Text;

/// Whether the first part use of `device` whose part has bus ports has one that no function of the device carries (by
/// interface and port label), as normaliser N5 pairs them (FT-841).
fn has_free_bus_port(tx: &TxBuilder<'_>, device: &Device) -> bool {
    let Some(ports) = device.parts.iter().find_map(|u| {
        tx.get::<LibCopy<Part>>(u.part.copy)
            .and_then(|p| p.item.sections.bus.clone())
            .filter(|b| !b.ports.is_empty())
            .map(|b| b.ports)
    }) else {
        return false;
    };
    let carried: Vec<(ew_mltext::Ident, Option<ew_mltext::Ident>)> = tx
        .functions_of_device(device.id)
        .into_iter()
        .filter_map(|f| tx.get::<Function>(f).and_then(|f| f.bus_port.clone()))
        .map(|p| (p.interface, p.port_label))
        .collect();
    ports.iter().any(|p| {
        !carried
            .iter()
            .any(|(i, l)| *i == p.interface && *l == p.port_label)
    })
}

/// Whether a device takes a function of FnDef `def` as a further function of its own whatever its family (FT-1860): the
/// device drawn with a second symbol, such as a supply drawn once for its input and once for its outputs, or an element
/// brought in from another program drawn for a device that has a part. A contact joins by D2's family rule instead and a
/// bus port by its free port; a terminal (it goes into a strip), an interruption point and a graphic join no device.
pub fn joins_any_device(def: &FnDef) -> bool {
    !matches!(
        def.contact_kind,
        ContactKind::No | ContactKind::Nc | ContactKind::Co | ContactKind::BusPort
    ) && !matches!(
        def.category.0.as_str(),
        "Terminal" | "TerminalEarth" | "InterruptionPoint" | "Graphic" | "ExternalReference"
    )
}

/// `rep.join_device` (`JoinDevice`): the placed representation `rep` joins `device` in one
/// transaction, at the same page, position and orientation, its wires kept.
///
/// A representation of a **terminal** joins another terminal of its strip as that terminal's next level, the whole
/// terminal with it (doc 08 §6.3; `join_level`). Otherwise:
///
/// It takes one of D1's two ways:
///
/// - **A slot first.** When `device` has an unplaced normal function of the representation's FnDef (a part's template
///   slot, FR-UI-017; the one of the lowest template entry, then the lowest ID), the representation draws that
///   function, which keeps its role and template entry; every connection, potential definition, bond and channel
///   binding or target at the old function moves to it, CP for CP (the FnDef is the same, so the CP indices line up),
///   and the representation's pin map follows.
/// - **Else a new auxiliary function.** The representation's function moves to `device` as an auxiliary function
///   (role aux, no template entry); its record, its CP overrides and everything attached to it stay. A contact needs a
///   device whose family takes auxiliary functions (D2, below); a function of another kind that a device takes as its
///   own whatever its family ([`joins_any_device`]) joins any device but a terminal or a strip (FT-1860).
///
/// The source device has no other representation and its other functions (of its part) are neither drawn nor wired
/// (D3, widened so that a coil placed from a valve part joins another valve); it is removed in the same
/// transaction with its part use and those functions, and with the old function in the slot case. What the removed records carry
/// besides that (the device's function text, process tag, structure, frozen tag and its other non-default members;
/// in the slot case the old function's CP overrides and its other own members) goes with them: the join goes ahead
/// and the outcome's notices name each dropped member. A main function never lands on a
/// device that has one (EW-INV-002): the moved function is auxiliary or fills a slot. Refused, with the pointer: a
/// representation that draws not exactly one normal function of a device, or whose device is a strip,
/// has another function that is drawn or wired, or another representation, or takes part in a device link, which the join would lose
/// (`/rep`; remove the link first); the representation's own device (`/device`); a
/// device that cannot take the FnDef by D2's rule (`/device`): it has no unplaced function of the FnDef, and the FnDef
/// is a contact (NO, NC, change-over) and the device's family takes no auxiliary functions (its main function is no
/// coil and none of its parts' templates names the FnDef; terminals and strips never do), or the FnDef joins no device
/// ([`joins_any_device`]: a terminal, an interruption point, a graphic), or the device is a terminal or a strip, unless the FnDef is a bus
/// port and the device's bus part has a port that no function of the device carries yet (FT-841: normaliser N5 then
/// gives the joined function that port); a connected CP the function
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
pub(super) fn dropped_device_members(device: &Device, target: &Device) -> Vec<Text> {
    let mut out = Vec::new();
    let tag = &device.tag;
    if tag.scheme != TagScheme::Default {
        out.push(text!("notice-member-numbering-scheme"));
    }
    if tag.frozen.is_some() {
        out.push(text!("notice-member-frozen-tag"));
    }
    if device.kind.is_some() && device.kind != target.kind {
        out.push(text!("notice-member-kind"));
    }
    if device.parent.is_some() {
        out.push(text!("notice-member-parent"));
    }
    if device.position.is_some() {
        out.push(text!("notice-member-position"));
    }
    if !device.structure.is_empty() && device.structure != target.structure {
        out.push(text!("notice-member-structure"));
    }
    if !device.function_text.is_empty() {
        out.push(text!("notice-member-function-text"));
    }
    if device.process_tag.is_some() {
        out.push(text!("notice-member-process-tag"));
    }
    if !device.tech_data.is_empty() {
        out.push(text!("notice-member-tech-data"));
    }
    if device.state != DeviceState::Used {
        out.push(text!("notice-member-state"));
    }
    if device.source_ref.is_some() {
        out.push(text!("notice-member-import-reference"));
    }
    let a = &device.aspects;
    if a.cable.is_some() {
        out.push(text!("notice-member-cable"));
    }
    if a.assembly.is_some() {
        out.push(text!("notice-member-assembly"));
    }
    if a.plc_station.is_some() {
        out.push(text!("notice-member-plc-station"));
    }
    if a.plc_module.is_some() {
        out.push(text!("notice-member-plc-module"));
    }
    if !device.extra.is_empty() || !tag.extra.is_empty() {
        out.push(text!("notice-member-unknown"));
    }
    out
}

/// The own members of the old function that the slot case drops (its record goes; the slot keeps its own).
pub(super) fn dropped_function_members(function: &Function) -> Vec<Text> {
    let mut out = Vec::new();
    if !function.cp_overrides.is_empty() {
        out.push(text!("notice-member-cp-overrides"));
    }
    if function.source_ref.is_some() {
        out.push(text!("notice-member-import-reference"));
    }
    if !function.extra.is_empty() {
        out.push(text!("notice-member-unknown"));
    }
    out
}

/// Member names in a sentence (`a`, `a and b`, `a, b and c`), and whether they take a plural verb: more than one, or
/// one whose name is a plural.
pub(super) fn listed(items: Vec<Text>) -> (ew_l10n::TextArg, bool) {
    let plural = items.len() > 1
        || items.iter().any(|t| {
            matches!(
                t.key(),
                "notice-member-unknown" | "notice-member-cp-overrides" | "notice-member-unplaced-functions"
            )
        });
    (
        crate::words::and_list(items.into_iter().map(Into::into).collect()),
        plural,
    )
}

/// Whether `device` is a terminal or a strip.
fn terminal_or_strip(device: &Device) -> bool {
    device.aspects.terminal.is_some() || device.aspects.strip.is_some()
}

/// Moves every connection, potential definition, bond and channel binding or target at function `from` (whose CPs are
/// `cps`) to function `to`: CP `i` of `from` becomes CP `idx(i)` of `to`.
pub(super) fn move_attached(
    tx: &mut TxBuilder<'_>,
    from: Id<Function>,
    cps: &BTreeSet<u16>,
    to: Id<Function>,
    idx: &dyn Fn(u16) -> u16,
) -> Result<(), CmdError> {
    let map = |e: &Endpoint| match e {
        Endpoint::Cp(g, i) if *g == from => Endpoint::Cp(to, idx(*i)),
        other => other.clone(),
    };
    for c in tx.connections_of_function(from) {
        tx.update::<Connection>(c, |c| {
            c.a = map(&c.a);
            c.b = map(&c.b);
        })?;
    }
    let (mut defs, mut bonds) = (BTreeSet::new(), BTreeSet::new());
    for i in cps {
        let at = EndpointKey::Cp(from, *i);
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
        .filter(|c| c.target == Some(from) || matches!(c.binding, Some(Endpoint::Cp(g, _)) if g == from))
        .map(|c| c.id)
        .collect();
    for c in channels {
        tx.update::<Channel>(c, |c| {
            c.binding = c.binding.as_ref().map(map);
            if c.target == Some(from) {
                c.target = Some(to);
            }
        })?;
    }
    Ok(())
}

/// The refusal of a representation that cannot join, with the reason.
fn not_eligible<T>(why: Text) -> Result<T, CmdError> {
    Err(CmdError::invalid_params(
        JoinDevice::NAME,
        "/rep",
        text!("refusal-join-not-eligible", why),
    ))
}

impl Command for JoinDevice {
    const NAME: &'static str = "rep.join_device";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let rep = tx.require::<Representation>(self.rep)?;
        let [function] = rep.functions.as_slice() else {
            return not_eligible(text!("refusal-join-not-one-function"));
        };
        let function = *function;
        let f = tx.require::<Function>(function)?;
        if f.kind != FunctionKind::Normal {
            return not_eligible(text!("refusal-join-ip-or-box"));
        }
        let Some(source) = f.device else {
            return not_eligible(text!("refusal-join-no-device"));
        };
        let source_device = tx.require::<Device>(source)?;
        // A terminal joins a terminal of its strip as that one's next level (doc 08 §6.3).
        if source_device.aspects.terminal.is_some() {
            return super::join_level::join_level(tx, self.rep, &source_device, self.device);
        }
        if terminal_or_strip(&source_device) {
            return not_eligible(text!("refusal-join-terminal"));
        }
        let others = self.check_source_may_go(tx, function, &source_device)?;
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
            self.check_takes_fndef(tx, &f, &target)?;
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
        let fn_cps = self.check_attached_cps_stay(tx, &f, &source_device, &after, &target)?;
        match slot {
            Some(slot) => {
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
                move_attached(tx, function, &fn_cps, slot, &|i| i)?;
            }
            None => tx.put(after),
        }
        // The emptied source device goes (in the slot case with the old function, which nothing names now); the
        // notices name what goes with them.
        let mut outcome = Outcome::primary((Kind::Representation, self.rep.cast()));
        outcome.notices = dropped_notices(&source_device, &target, &f, !others.is_empty(), slot.is_some());
        tx.delete((Kind::Device, source.cast()))?;
        Ok(outcome)
    }
}

impl JoinDevice {
    /// The source device may go with the join: its other functions (of its part) are neither drawn nor wired, the
    /// representation is the function's only one, the device takes part in no device link, and it is not the target.
    /// The device's other functions, which go with it and its part use.
    fn check_source_may_go(
        &self,
        tx: &TxBuilder<'_>,
        function: Id<Function>,
        source_device: &Device,
    ) -> Result<Vec<Id<Function>>, CmdError> {
        let source = source_device.id;
        let others: Vec<Id<Function>> = tx
            .functions_of_device(source)
            .into_iter()
            .filter(|g| *g != function)
            .collect();
        for g in &others {
            let other = tx.require::<Function>(*g)?;
            let attached = !tx.reps_of_function(*g).is_empty()
                || !tx.connections_of_function(*g).is_empty()
                || crate::commands::common::function_cp_indices(tx, &other, source_device)
                    .iter()
                    .any(|i| {
                        let at = EndpointKey::Cp(*g, *i);
                        !tx.potential_defs_at(at).is_empty()
                            || !tx.bonds_at(at).is_empty()
                            || !tx.channels_at_cp(*g, *i).is_empty()
                    });
            if attached {
                return not_eligible(text!("refusal-join-other-function"));
            }
        }
        if tx.reps_of_function(function) != [self.rep] {
            return not_eligible(text!("refusal-join-other-rep"));
        }
        // The join removes the source device, and its device links with it (they cascade): refused, not lost.
        if let Some(link) = tx
            .iter::<DeviceLink>()
            .into_iter()
            .filter(|l| l.from == source || l.to == source)
            .min_by_key(|l| l.id)
        {
            let other = if link.from == source { link.to } else { link.from };
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/rep",
                text!("refusal-join-device-link", crate::words::device(tx, other)),
            ));
        }
        if self.device == source {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/device",
                text!("refusal-join-same-device", crate::words::device(tx, source)),
            ));
        }
        Ok(others)
    }

    /// D2, where the target has no slot of the FnDef: a contact joins a device whose family takes auxiliary functions,
    /// a bus port a device whose bus part has a free port (FT-841), and a function of another kind any device but a
    /// terminal or a strip (FT-1860).
    fn check_takes_fndef(&self, tx: &TxBuilder<'_>, f: &Function, target: &Device) -> Result<(), CmdError> {
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
        // FT-841: a bus-port FnDef joins a device whose bus part has a port no function carries yet
        let bus_port = tx
            .get::<LibCopy<FnDef>>(f.def)
            .is_some_and(|d| d.item.contact_kind == ContactKind::BusPort);
        let free_port = bus_port && has_free_bus_port(tx, target);
        let auxiliary = contact && !terminal_or_strip(target) && (coil || templated);
        // FT-1860: a function of another kind is the device drawn once more, on any device but a terminal or a strip.
        let any = tx
            .get::<LibCopy<FnDef>>(f.def)
            .is_some_and(|d| joins_any_device(&d.item));
        let own = any && !terminal_or_strip(target);
        if !(auxiliary || free_port || own) {
            let fndef_words = tx.get::<LibCopy<FnDef>>(f.def).map_or_else(
                || ew_derive::text::record(ew_schema::Kind::Fndef),
                |d| {
                    let key = d.item.meta.key.as_ref().map(ToString::to_string);
                    crate::words::copy(tx, ew_schema::Kind::Fndef, key, &d.version)
                },
            );
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/device",
                text!(
                    "refusal-join-no-function",
                    crate::words::device(tx, self.device),
                    fndef_words,
                    if contact {
                        text!("refusal-join-no-auxiliary")
                    } else if bus_port {
                        text!("refusal-join-no-free-port")
                    } else if any {
                        text!("refusal-join-target-terminal")
                    } else {
                        text!("refusal-join-no-device-function")
                    }
                ),
            ));
        }
        Ok(())
    }

    /// Every attached CP of the function must exist on the function as it will be (the same FnDef, so the indices
    /// line up unless a part's template draws other CPs). The function's CPs on its source device.
    fn check_attached_cps_stay(
        &self,
        tx: &TxBuilder<'_>,
        f: &Function,
        source_device: &Device,
        after: &Function,
        target: &Device,
    ) -> Result<BTreeSet<u16>, CmdError> {
        let function = f.id;
        let now = crate::commands::common::function_cp_indices(tx, after, target);
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
        let fn_cps = crate::commands::common::function_cp_indices(tx, f, source_device);
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
                text!(
                    "refusal-join-cp-connected",
                    crate::words::device(tx, self.device),
                    lost.join(", ")
                ),
            ));
        }
        Ok(fn_cps)
    }
}

/// The notices that name what goes with the emptied source device (its members, its part use, its unplaced
/// functions) and, in the slot case, with the old function.
fn dropped_notices(
    source_device: &Device,
    target: &Device,
    f: &Function,
    others: bool,
    slot: bool,
) -> Vec<Text> {
    let mut notices = Vec::new();
    let mut device_members = dropped_device_members(source_device, target);
    if !source_device.parts.is_empty() {
        device_members.push(text!("notice-member-part"));
    }
    if others {
        device_members.push(text!("notice-member-unplaced-functions"));
    }
    if !device_members.is_empty() {
        notices.push(match listed(device_members) {
            (words, false) => text!("notice-join-device-dropped-one", words),
            (words, true) => text!("notice-join-device-dropped-many", words),
        });
    }
    if slot {
        let function_members = dropped_function_members(f);
        if !function_members.is_empty() {
            notices.push(match listed(function_members) {
                (words, false) => text!("notice-join-function-dropped-one", words),
                (words, true) => text!("notice-join-function-dropped-many", words),
            });
        }
    }
    notices
}
