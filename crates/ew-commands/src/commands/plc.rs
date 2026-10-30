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
    AddrSpec, Channel, Device, Endpoint, Extra, FnDef, Function, IoDir, Kind, LibCopy, Part, PartUse,
    PlcArea, PlcConfigProject,
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
///
/// In a command log (and the blind harness's `command.run`) the channel and the CP may also be named by tags
/// (FT-721): the channel as `<module tag>#<number>` or `<module tag>:<designation>`, the function as `<device tag>`
/// or `<device tag>:<slot>` with the CP by `designation` instead of `idx`, as
/// `{"channel": "-201U5#3", "endpoint": {"cp": {"function": "-201Y3", "designation": "14"}}}`; the replay resolves
/// them into this ID form before the command runs, and refuses an unknown or ambiguous name with up to five
/// candidates.
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
            // FX38a: the CPs the function has as the pages draw them (`function_cps`): a black-box card's come from
            // its part's CP pattern, not from the generic black-box FnDef, which defines none
            let device = match f.device {
                Some(d) => tx.get::<Device>(d),
                None => None,
            };
            let has = match &device {
                Some(d) => super::common::function_cp_indices(tx, &f, d).contains(&idx),
                None => tx.require::<LibCopy<FnDef>>(f.def)?.item.cps.get(&idx).is_some(),
            };
            if !has {
                let source = if device.as_ref().is_some_and(|d| {
                    matches!(f.kind, ew_schema::FunctionKind::BlackBox)
                        && d.parts.iter().any(|u| {
                            tx.get::<LibCopy<Part>>(u.part.copy)
                                .is_some_and(|p| !p.item.cp_pattern.is_empty())
                        })
                }) {
                    "its device's part pattern defines the CPs of a black box".to_owned()
                } else {
                    format!("its FnDef copy {} or its part's template defines the CPs", f.def)
                };
                return tx.reject(format!("function {function} has no CP {idx} ({source})"));
            }
            // the channels that hold the CP: an index lookup, not a scan of the channel table (FT-319)
            let others: Vec<Id<Channel>> = tx
                .channels_at_cp(function, idx)
                .into_iter()
                .filter(|c| *c != self.channel)
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

/// A direction of a module's I/O range: the start `plc.set_start` sets.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StartDir {
    /// The input start (area I).
    Input,
    /// The output start (area Q).
    Output,
}

/// `plc.set_start` (`SetStart`, doc 10 §3.9 `plc.set_start{modules, dir, spec}`, FR-PLC-013, FR-PLC-014; WP-S1.21
/// D8): sets the start of one direction of one or more modules.
///
/// `spec` is `auto` (the default: the module takes the first free range after the modules before it, algorithm A1),
/// `excluded` (not addressed), `tbd` (an open item, reported by EW-PLC-015) or a start text in the notation of the
/// module's configuration project's profile (the shipped default profile while the module has no station): the area
/// and the byte, as `Q10`, `Q10.0` or `QB10` under the TIA profile and `%QX10` under IEC, stored as a `manual` start.
/// The keywords are read without regard to case. Rejected: a module that does not exist ([`CmdError::NotFound`]) or
/// has no `plc_module` aspect; at `/spec`, a text the profile does not read (with the profile's example start) and a
/// start in another area than the direction's (an input start is in area I, an output start in area Q). A module
/// whose part has no range in `dir` takes the start anyway, with a notice that it has no effect. A start equal to the
/// current one writes nothing. One undo step. Outcome: the first module.
///
/// Renders nothing by itself: only the modules whose start value changes are re-addressed (the per-module cut-off
/// of D9: a manual start outside the automatic path moves no other module, doc 10 §3.1).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetStart {
    /// The module devices (at least one).
    #[schemars(with = "Vec<UuidS>")]
    pub modules: Vec<Id<Device>>,
    /// The direction whose start is set.
    pub dir: StartDir,
    /// `auto`, `excluded`, `tbd` or a start text of the profile (`Q10`).
    pub spec: String,
}

impl Command for SetStart {
    const NAME: &'static str = "plc.set_start";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let Some(first) = self.modules.first().copied() else {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/modules",
                "name at least one module",
            ));
        };
        let word = self.spec.trim().to_ascii_lowercase();
        let mut notices = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for m in &self.modules {
            if !seen.insert(*m) {
                continue;
            }
            let dev = tx.require::<Device>(*m)?;
            let Some(aspect) = dev.aspects.plc_module.as_ref() else {
                return tx.reject(format!(
                    "device {m} is not a module: it has no `plc_module` aspect; a start belongs to a module"
                ));
            };
            let spec = match word.as_str() {
                "auto" => AddrSpec::Auto,
                "excluded" => AddrSpec::Excluded,
                "tbd" => AddrSpec::Tbd,
                _ => {
                    let profile = module_profile(tx, *m);
                    let a = ew_derive::parse_plc_start(&profile, &self.spec)
                        .map_err(|e| CmdError::invalid_params(Self::NAME, "/spec", e))?;
                    let (want, name, area) = match self.dir {
                        StartDir::Input => (PlcArea::I, "an input", ew_derive::ew_plcaddr::Area::I),
                        StartDir::Output => (PlcArea::Q, "an output", ew_derive::ew_plcaddr::Area::Q),
                    };
                    if a.area != want {
                        let example = ew_derive::ew_plcaddr::builtin(&profile)
                            .map(|p| ew_derive::start_example(p, area))
                            .unwrap_or_default();
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            "/spec",
                            format!(
                                "`{}` is not {name} start: {name} start is in area {}, like {example}",
                                self.spec,
                                if want == PlcArea::I { "I" } else { "Q" }
                            ),
                        ));
                    }
                    AddrSpec::Manual(a)
                }
            };
            let dir = match self.dir {
                StartDir::Input => IoDir::In,
                StartDir::Output => IoDir::Out,
            };
            let has_range = module_section(tx, &dev).is_some_and(|s| s.io.iter().any(|r| r.dir == dir));
            if !has_range {
                notices.push(format!(
                    "module {m} has no {} range in its part: the start has no effect until it has one",
                    if dir == IoDir::In { "input" } else { "output" }
                ));
            }
            let current = match self.dir {
                StartDir::Input => &aspect.start.input,
                StartDir::Output => &aspect.start.output,
            };
            if *current == spec {
                continue;
            }
            tx.update::<Device>(*m, |d| {
                if let Some(module) = d.aspects.plc_module.as_mut() {
                    match self.dir {
                        StartDir::Input => module.start.input = spec,
                        StartDir::Output => module.start.output = spec,
                    }
                }
            })?;
        }
        let mut outcome = Outcome::primary((Kind::Device, first.cast()));
        outcome.notices = notices;
        Ok(outcome)
    }
}

/// `plc.set_profile` (`SetProfile`, doc 10 §3.9 `plc.set_profile{cfg, profile}`, FR-PLC-008, FR-PLC-007; WP-S1.21
/// D8): pins another shipped address profile on a configuration project.
///
/// `profile` names a profile this build ships, by its reference `id@version` or its ID alone, without regard to case
/// (`iec-61131-3` pins `iec-61131-3@2027.1`); every other text is rejected at `/profile` with the list of shipped
/// profiles. The change renders only: addresses are stored canonically, so the project files differ in the one
/// `profile` member, and every address prints in the new notation (`Q0.1` → `%QX0.1`). A profile equal to the current
/// one writes nothing. The configuration project exists ([`CmdError::NotFound`]). One undo step. Outcome: the
/// configuration project.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetProfile {
    /// The configuration project.
    #[schemars(with = "UuidS")]
    pub config_project: Id<PlcConfigProject>,
    /// The shipped profile: `id@version` or the ID (`iec-61131-3`).
    pub profile: String,
}

impl Command for SetProfile {
    const NAME: &'static str = "plc.set_profile";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let Some(reference) = ew_derive::shipped_address_profile(&self.profile) else {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/profile",
                format!(
                    "`{}` is not a shipped address profile; the shipped profiles are {}",
                    self.profile,
                    ew_derive::shipped_address_profiles().join(", ")
                ),
            ));
        };
        let cfg = tx.require::<PlcConfigProject>(self.config_project)?;
        if cfg.profile.as_str() != reference {
            let profile = Ident::from_input(&reference)
                .map_err(|e| CmdError::internal(format!("plc.set_profile: {reference}: {e}")))?;
            tx.update::<PlcConfigProject>(self.config_project, |c| c.profile = profile)?;
        }
        Ok(Outcome::primary((
            Kind::PlcConfigProject,
            self.config_project.cast(),
        )))
    }
}

/// The profile reference that reads a module's start texts: its configuration project's, else the default.
fn module_profile(tx: &TxBuilder<'_>, module: Id<Device>) -> String {
    crate::commands::fields::channel_profile(tx, module)
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
        // the module's channels from the index (FT-319), not a scan of the channel table
        let mut surplus: Vec<_> = tx
            .channels_of_module(m)
            .into_iter()
            .filter_map(|id| tx.get::<Channel>(id))
            .filter(|c| c.number >= count)
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
