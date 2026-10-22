// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! PLC records (doc 10 §2.2–§2.4, §3.3, §3.9; FT-270, FT-277): `plc.config_project.create`
//! (`CreatePlcConfigProject`), `plc.bind_channel` (`BindChannel`) and `plc.set_module_part`
//! (`SetModulePart`, Bulk).
//!
//! Stations and modules are device aspects, set with `fields.set` (`plc_station`, `plc_module`); the
//! channels of a module come from its part data through normaliser N1 (`channel_instantiation`) when the
//! `plc_module` aspect is added or the module's part changes, and are bound by N2 (valve coils, by the
//! part's solenoid-position map) and N3 (module pins, by the part's pin map). N1 never removes a channel:
//! `plc.set_module_part` does, after its preview (FR-PLC-053).

use ew_ids::Id;
use ew_mltext::Ident;
use ew_schema::{
    Channel, Device, Endpoint, Extra, FnDef, Function, Kind, LibCopy, Part, PartUse, PlcConfigProject,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::commands::common::EndpointParam;
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::normalise::channel_instantiation::{binding_text, module_section};
use crate::proxy::{IdentS, UuidS};
use crate::tx::TxBuilder;

/// `plc.config_project.create` (`CreatePlcConfigProject`): a configuration project, the target
/// PLC-software project that scopes address and symbol uniqueness (doc 10 §2.2, FR-PLC-002).
///
/// Creates `PlcConfigProject {name, profile, cpu}`. Rejected: a name another configuration project has
/// (compared exactly); a profile that is not `id@version` with both parts non-empty (at `/profile`). The
/// CPU device, when given, must exist. Stations name the configuration project in their `plc_station`
/// aspect (`fields.set`); while one does, the configuration project cannot be deleted. Outcome: the
/// configuration project.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatePlcConfigProject {
    /// The name ("PLC1").
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// The pinned address profile, `id@version` ("siemens-tia-intl@2027.1", doc 10 §2.5).
    #[schemars(with = "IdentS")]
    pub profile: Ident,
    /// The CPU device; absent when the master sits outside the project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub cpu: Option<Id<Device>>,
}

impl Command for CreatePlcConfigProject {
    const NAME: &'static str = "plc.config_project.create";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let well_formed = self
            .profile
            .as_str()
            .split_once('@')
            .is_some_and(|(id, version)| !id.is_empty() && !version.is_empty() && !version.contains('@'));
        if !well_formed {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/profile",
                format!(
                    "`{}` is not a profile reference `id@version` (\"siemens-tia-intl@2027.1\")",
                    self.profile
                ),
            ));
        }
        if let Some(cpu) = self.cpu {
            tx.require::<Device>(cpu)?;
        }
        if let Some(other) = tx
            .iter::<PlcConfigProject>()
            .into_iter()
            .find(|p| p.name == self.name)
        {
            return tx.reject(format!(
                "configuration project {} exists ({}); names are unique",
                self.name, other.id
            ));
        }
        let id: Id<PlcConfigProject> = tx.mint();
        tx.insert(PlcConfigProject {
            id,
            v: 1,
            name: self.name.clone(),
            profile: self.profile.clone(),
            cpu: self.cpu,
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::PlcConfigProject, id.cast())))
    }
}

/// `plc.bind_channel` (`BindChannel`, doc 10 §3.3, §3.9 `plc.bind_channel{channel, endpoint}`): binds a
/// channel to its signal CP, rebinds it, or unbinds it (FR-PLC-020).
///
/// Sets `Channel.binding` to the CP (`{"cp": {"function", "idx"}}`: a module pin, or the internal CP of
/// the coil a valve-terminal channel serves) or, with `endpoint: null`, removes it. The channel and the
/// function exist ([`CmdError::NotFound`]); the function must belong to the channel's module or to one of
/// its descendants (else rejected); the CP index must be a CP of the function's FnDef copy (else
/// rejected); a junction or a point on a connection is not a signal CP (at `/endpoint`). Another channel
/// bound to the same CP is unbound in the same transaction (a CP is in at most one channel, FR-PLC-005),
/// and the outcome's notices name it. An explicit binding wins over the solenoid-position map: N2 never
/// rebinds a coil that a channel binds, nor a channel that is bound. Outcome: the channel.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BindChannel {
    /// The channel.
    #[schemars(with = "UuidS")]
    pub channel: Id<Channel>,
    /// The signal CP; `null` unbinds.
    pub endpoint: Option<EndpointParam>,
}

impl Command for BindChannel {
    const NAME: &'static str = "plc.bind_channel";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let binding = match &self.endpoint {
            None => None,
            Some(EndpointParam::Cp { function, idx }) => Some((*function, *idx)),
            Some(_) => {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    "/endpoint",
                    "a channel binds a CP (`{\"cp\": {\"function\", \"idx\"}}`), not a junction or a point on a \
                     connection",
                ));
            }
        };
        let channel = tx.require::<Channel>(self.channel)?;
        let mut notices = Vec::new();
        if let Some((function, idx)) = binding {
            let f = tx.require::<Function>(function)?;
            // owner ruling (cycle 16): the signal CP lies on the module itself (a pin) or on one of its
            // descendants (the coil of a valve on the module); anything else is refused
            let mut at = f.device;
            let mut steps = 0usize;
            let inside = loop {
                match at {
                    Some(d) if d == channel.module => break true,
                    Some(d) if steps <= 4096 => {
                        at = tx.get::<Device>(d).and_then(|x| x.parent);
                        steps += 1;
                    }
                    _ => break false,
                }
            };
            if !inside {
                return tx.reject(format!(
                    "function {function} is not on module {} or one of its children; a channel binds a pin of \
                     its module or the coil of a valve on it",
                    channel.module
                ));
            }
            let def = tx.require::<LibCopy<FnDef>>(f.def)?;
            if def.item.cps.get(&idx).is_none() {
                return tx.reject(format!(
                    "function {function} has no CP {idx} (its FnDef copy {} defines the CPs)",
                    f.def
                ));
            }
            let endpoint = Endpoint::Cp(function, idx);
            let others: Vec<Id<Channel>> = tx
                .iter::<Channel>()
                .into_iter()
                .filter(|c| c.id != self.channel && c.binding.as_ref() == Some(&endpoint))
                .map(|c| c.id)
                .collect();
            for other in others {
                tx.update::<Channel>(other, |c| c.binding = None)?;
                notices.push(format!(
                    "channel {other} was bound to the same CP and is unbound (FR-PLC-005)"
                ));
            }
        }
        let endpoint = binding.map(|(f, idx)| Endpoint::Cp(f, idx));
        tx.update::<Channel>(self.channel, |c| c.binding = endpoint)?;
        let mut outcome = Outcome::primary((Kind::Channel, self.channel.cast()));
        outcome.notices = notices;
        Ok(outcome)
    }
}

/// `plc.set_module_part` (`SetModulePart`, FR-PLC-053, doc 10 §3.3 N1, §8 item 3): changes a module's part
/// and removes the channels the new part no longer has, after the confirmation that lists them.
///
/// Sets `Device.parts` to `parts` (absent: the parts stay as they are) and deletes the module's channels
/// numbered at or above the channel count of the resulting PLC section (the part use named by
/// `plc_module.part_use`, else the first part use whose part has a PLC section; a part without a channel
/// spec has a count of 0). Their bindings, symbols and
/// comments are members of the channel record and go with it; no other record refers to a channel, and
/// exchange records do not exist yet (doc 10 §4). Normaliser N1 then creates the missing channels and applies
/// the part's channel kind, as for any part change.
///
/// **Bulk:** it always runs as a preview first (ADR-0008), and committing the preview is FR-PLC-053's
/// confirmation: the preview's diff lists every deleted channel, and its outcome has one notice per channel
/// with its number, binding and symbol. `fields.set` on `parts` stays the interactive way to change a part:
/// it never removes a channel, and N1 names the surplus channels it leaves, which this command without
/// `parts` removes. Rejected: a device without the `plc_module` aspect; new `parts` in which the aspect's
/// `part_use` names no entry (at `/parts`: the count would fall to 0 and every channel go); a part copy that
/// does not exist and a module that does not exist ([`CmdError::NotFound`]). A module whose parts and
/// channels need no change gives an empty transaction,
/// which commits nothing. Undoable as one step. Outcome: the module.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetModulePart {
    /// The module device.
    #[schemars(with = "UuidS")]
    pub module: Id<Device>,
    /// The module's new part uses (the whole list, as `fields.set` on `parts` takes it); absent: the parts
    /// stay and only the surplus channels are removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parts: Option<Vec<PartUse>>,
}

impl Command for SetModulePart {
    const NAME: &'static str = "plc.set_module_part";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let m = self.module;
        let before = tx.require::<Device>(m)?;
        let Some(aspect) = before.aspects.plc_module.as_ref() else {
            return tx.reject(format!(
                "device {m} is not a module: it has no `plc_module` aspect (set it with `fields.set`)"
            ));
        };
        if let Some(parts) = &self.parts {
            // review ruling (cycle 18): a `part_use` that names no entry of the new parts would give a count
            // of 0 and delete every channel; it is refused, the part use is changed first
            if let Some(i) = aspect.part_use.filter(|i| usize::from(*i) >= parts.len()) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    "/parts",
                    format!(
                        "the module's `plc_module.part_use` is {i}, which names no entry of the {} new part \
                         use(s); change `plc_module.part_use` first",
                        parts.len()
                    ),
                ));
            }
            for u in parts {
                tx.require::<LibCopy<Part>>(u.part.copy)?;
            }
            let parts = parts.clone();
            tx.update::<Device>(m, |d| d.parts = parts)?;
        }
        let dev = tx.require::<Device>(m)?;
        let count = module_section(tx, &dev)
            .and_then(|s| s.channels)
            .map_or(0, |c| c.count);
        let mut surplus: Vec<_> = tx
            .iter::<Channel>()
            .into_iter()
            .filter(|c| c.module == m && c.number >= count)
            .collect();
        surplus.sort_by_key(|c| (c.number, c.id));
        let mut notices = Vec::new();
        let mut doomed = Vec::new();
        for c in surplus {
            let symbol = c
                .symbol
                .as_ref()
                .map_or_else(|| "no symbol".to_owned(), |s| format!("symbol {s}"));
            notices.push(format!(
                "channel {} of module {m} is removed: beyond its part's {count} channels; it was {}, {symbol}",
                c.number,
                binding_text(c.binding.as_ref()),
            ));
            doomed.push((Kind::Channel, c.id.cast()));
        }
        if !doomed.is_empty() {
            tx.delete_many(&doomed)?;
        }
        let mut outcome = Outcome::primary((Kind::Device, m.cast()));
        outcome.notices = notices;
        Ok(outcome)
    }
}
