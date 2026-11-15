// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! PLC records (doc 10 §2.2–§2.4, §3.3, §3.9): `plc.config_project.create`
//! (`CreatePlcConfigProject`), `plc.bind_channel` (`BindChannel`), `plc.set_module_part`
//! (`SetModulePart`, Bulk), `plc.set_start`, `plc.set_profile`, and the Bulk commands of freezing and
//! re-addressing: `plc.freeze` (`FreezeAddresses`), `plc.readdress` (`Readdress`) and
//! `plc.convert_manual_to_auto` (`ConvertManualToAuto`).
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
use crate::normalise::channel_instantiation::{binding_words, module_section};
use crate::proxy::{IdentS, UuidS};
use crate::text::text;
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
                text!("refusal-plc-profile-reference", self.profile.to_string()),
            ));
        }
        if let Some(cpu) = self.cpu {
            tx.require::<Device>(cpu)?;
        }
        if tx
            .iter::<PlcConfigProject>()
            .into_iter()
            .any(|p| p.name == self.name)
        {
            return tx.reject(text!("refusal-plc-project-exists", self.name.to_string()));
        }
        let id: Id<PlcConfigProject> = tx.mint();
        tx.insert(PlcConfigProject {
            id,
            v: 1,
            name: self.name.clone(),
            profile: self.profile.clone(),
            cpu: self.cpu,
            symbols: None,
            ownership: None,
            exchange: None,
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::PlcConfigProject, id.cast())))
    }
}

/// Whether a function on `device` may take a channel of `module`: the device is the module
/// itself (a pin of the card) or one of its descendants (the coil of a valve on it), following `parent` up at most
/// 4096 steps. A function on no device is on no module. [`BindChannel`] refuses a binding outside this rule; a host
/// asks the same question before it commits, so that it can refuse in words.
pub fn is_on_module(
    device: Option<Id<Device>>,
    module: Id<Device>,
    parent: impl Fn(Id<Device>) -> Option<Id<Device>>,
) -> bool {
    let mut at = device;
    let mut steps = 0usize;
    loop {
        match at {
            Some(d) if d == module => return true,
            Some(d) if steps <= 4096 => {
                at = parent(d);
                steps += 1;
            }
            _ => return false,
        }
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
/// With `attach_at`, the device of the endpoint's function first becomes a child of the channel's module at that
/// position, as `assembly.attach` makes it (with its checks), so that a device wired off the card is attached and
/// bound in one undo step (FT-774).
///
/// In a command log (and the blind harness's `command.run`) the channel and the CP may also be named by tags:
/// the channel as `<module tag>#<number>` or `<module tag>:<designation>`, the function as `<device tag>`
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
    /// The position at which the endpoint's device is attached to the channel's module before the binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attach_at: Option<u32>,
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
                    text!("refusal-plc-bind-endpoint"),
                ));
            }
        };
        let channel = tx.require::<Channel>(self.channel)?;
        let mut notices = Vec::new();
        if let (Some(position), Some((function, _))) = (self.attach_at, binding)
            && let Some(device) = tx.require::<Function>(function)?.device
        {
            super::assembly::AttachChildren {
                parent: channel.module,
                children: vec![super::assembly::ChildSpec {
                    device,
                    position: Some(position),
                }],
            }
            .run(tx)?;
        }
        if let Some((function, idx)) = binding {
            let f = tx.require::<Function>(function)?;
            // The signal CP lies on the module itself (a pin) or on one of its
            // descendants (the coil of a valve on the module); anything else is refused
            let inside = is_on_module(f.device, channel.module, |d| {
                tx.get::<Device>(d).and_then(|x| x.parent)
            });
            if !inside {
                return tx.reject(text!(
                    "refusal-plc-channel-not-on-module",
                    crate::words::function(tx, function),
                    crate::words::device(tx, channel.module)
                ));
            }
            // The CPs the function has as the pages draw them (`function_cps`): a black-box card's come from
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
                    "its function definition or its part's template defines the CPs".to_owned()
                };
                return tx.reject(text!(
                    "refusal-plc-channel-no-cp",
                    crate::words::function(tx, function),
                    idx.to_string(),
                    source.to_string()
                ));
            }
            // the channels that hold the CP: an index lookup, not a scan of the channel table
            let others: Vec<Id<Channel>> = tx
                .channels_at_cp(function, idx)
                .into_iter()
                .filter(|c| *c != self.channel)
                .collect();
            for other in others {
                // The notice names the displaced channel in words (its number and whether it is on the same
                // module), never by its record ID; a host that knows the derived address says more (the PLC
                // navigator's toast names the address and the pin).
                let words = match tx.get::<Channel>(other) {
                    Some(o) if o.module == channel.module => {
                        format!("channel {} of the same module", o.number)
                    }
                    Some(o) => format!("channel {} of another module", o.number),
                    None => "another channel".to_owned(),
                };
                tx.update::<Channel>(other, |c| c.binding = None)?;
                notices.push(format!(
                    "{words} was bound to this connection point and is now unbound"
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

/// `plc.set_start` (`SetStart`, doc 10 §3.9 `plc.set_start{modules, dir, spec}`, FR-PLC-013, FR-PLC-014):
/// sets the start of one direction of one or more modules.
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
                text!("refusal-params-no-module"),
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
                return tx.reject(text!(
                    "refusal-plc-address-not-module",
                    crate::words::device(tx, *m)
                ));
            };
            let spec = match word.as_str() {
                "auto" => AddrSpec::Auto,
                "excluded" => AddrSpec::Excluded,
                "tbd" => AddrSpec::Tbd,
                _ => {
                    let profile = module_profile(tx, *m);
                    let at_hand = crate::commands::fields::profiles_in(tx);
                    // The parser's refusal by key.
                    let a = ew_derive::parse_plc_start(&at_hand, &profile, &self.spec).map_err(|_| {
                        let why = crate::error::plc_text_refusal(&at_hand, &profile, &self.spec, true);
                        CmdError::invalid_params(Self::NAME, "/spec", text!("refusal-params-detail", why))
                    })?;
                    let (want, name, area) = match self.dir {
                        StartDir::Input => (
                            PlcArea::I,
                            text!("refusal-plc-start-input"),
                            ew_derive::ew_plcaddr::Area::I,
                        ),
                        StartDir::Output => (
                            PlcArea::Q,
                            text!("refusal-plc-start-output"),
                            ew_derive::ew_plcaddr::Area::Q,
                        ),
                    };
                    if a.area != want {
                        let example = ew_derive::pinned_profile(&at_hand, &profile)
                            .map(|p| ew_derive::start_example(&p, area))
                            .unwrap_or_default();
                        return Err(CmdError::invalid_params(
                            Self::NAME,
                            "/spec",
                            text!(
                                "refusal-plc-start-area",
                                self.spec.clone(),
                                name,
                                if want == PlcArea::I { "I" } else { "Q" },
                                example
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
                    "module {} has no {} range in its part: the start has no effect until it has one",
                    crate::words::device(tx, *m),
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
        super::plc_exchange::refuse_owned_edits(tx)?;
        let mut outcome = Outcome::primary((Kind::Device, first.cast()));
        outcome.notices = notices;
        Ok(outcome)
    }
}

/// `plc.set_profile` (`SetProfile`, doc 10 §3.9 `plc.set_profile{cfg, profile}`, FR-PLC-008, FR-PLC-007):
/// pins another address profile at hand on a configuration project.
///
/// `profile` names a profile at hand, built in or held by the project as a `plc_profile` copy (a library's profile
/// is embedded by the front end with `library.embed` first), by its reference `id@version` or its ID alone, without
/// regard to case (`iec-61131-3` pins `iec-61131-3@2027.1`); every other text is rejected at `/profile` with the list
/// of the profiles at hand. The change renders only: addresses are stored canonically, so the project files differ
/// in the one `profile` member, and every address prints in the new notation (`Q0.1` → `%QX0.1`). A profile equal to
/// the current one writes nothing. The configuration project exists ([`CmdError::NotFound`]). One undo step.
/// Outcome: the configuration project.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetProfile {
    /// The configuration project.
    #[schemars(with = "UuidS")]
    pub config_project: Id<PlcConfigProject>,
    /// The profile at hand: `id@version` or the ID (`iec-61131-3`).
    pub profile: String,
}

impl Command for SetProfile {
    const NAME: &'static str = "plc.set_profile";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let at_hand = crate::commands::fields::profiles_in(tx);
        let Some(reference) = ew_derive::address_profile_named(&at_hand, &self.profile) else {
            let list: Vec<String> = at_hand.iter().map(|p| p.reference()).collect();
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/profile",
                text!(
                    "refusal-plc-profile-unknown",
                    self.profile.to_string(),
                    list.join(", ")
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
            return tx.reject(text!("refusal-plc-not-module", crate::words::device(tx, m)));
        };
        if let Some(parts) = &self.parts {
            // A `part_use` that names no entry of the new parts would give a count
            // of 0 and delete every channel; it is refused, the part use is changed first
            if let Some(i) = aspect.part_use.filter(|i| usize::from(*i) >= parts.len()) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    "/parts",
                    text!(
                        "refusal-plc-module-part-use",
                        i.to_string(),
                        parts.len().to_string()
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
        // the module's channels from the index, not a scan of the channel table
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
            // The module by its tag and the binding in words, as N2's notices name them
            notices.push(format!(
                "channel {} of module {} is removed: beyond its part's {count} channels; it was {}, {symbol}",
                c.number,
                crate::words::device(tx, m),
                binding_words(tx, c.binding.as_ref()),
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

// ---------------------------------------------------------------------------------------------------------
// Freezing and re-addressing (doc 10 §3.9, FR-PLC-014, FR-PLC-015)

/// The modules a re-address command acts on: the named modules (each with the `plc_module` aspect), else every module
/// of the configuration project in allocation order, from the transaction's base snapshot (the command starts there).
fn scope_modules(
    tx: &TxBuilder<'_>,
    name: &'static str,
    config_project: Option<Id<PlcConfigProject>>,
    modules: &[Id<Device>],
) -> Result<Vec<Id<Device>>, CmdError> {
    if !modules.is_empty() {
        let mut out = Vec::new();
        for m in modules {
            let dev = tx.require::<Device>(*m)?;
            if dev.aspects.plc_module.is_none() {
                return tx.reject(text!("refusal-plc-not-module", crate::words::device(tx, *m)));
            }
            if !out.contains(m) {
                out.push(*m);
            }
        }
        return Ok(out);
    }
    let Some(cfg) = config_project else {
        return Err(CmdError::invalid_params(
            name,
            "/modules",
            text!("refusal-plc-scope-empty"),
        ));
    };
    tx.require::<PlcConfigProject>(cfg)?;
    let q = ew_derive::Q::new(tx.base_model());
    Ok(q.get::<ew_derive::ModuleOrder>(cfg)
        .iter()
        .map(|m| m.module)
        .collect())
}

/// Every module of the configuration projects that `modules` belong to, in allocation order: the modules whose
/// automatic addresses a change of `modules` can move.
fn neighbourhood(q: &ew_derive::Q, modules: &[Id<Device>]) -> Vec<Id<Device>> {
    let mut cfgs: Vec<Id<PlcConfigProject>> = Vec::new();
    for m in modules {
        let cfg = ew_derive::station_of(q, *m).and_then(|s| {
            q.devices()
                .get(s)
                .and_then(|d| d.aspects.plc_station.as_ref().map(|p| p.config_project))
        });
        if let Some(cfg) = cfg.filter(|c| !cfgs.contains(c)) {
            cfgs.push(cfg);
        }
    }
    let mut out: Vec<Id<Device>> = Vec::new();
    for cfg in cfgs {
        out.extend(q.get::<ew_derive::ModuleOrder>(cfg).iter().map(|m| m.module));
    }
    for m in modules {
        if !out.contains(m) {
            out.push(*m);
        }
    }
    out
}

/// A derived address as a notice prints it: the profile's text, `TBD`, or `no address`.
fn address_words(a: &ew_derive::ChannelAddr) -> String {
    match a {
        ew_derive::ChannelAddr::At { text, .. } => text.clone(),
        ew_derive::ChannelAddr::Tbd => "TBD".to_owned(),
        ew_derive::ChannelAddr::Excluded | ew_derive::ChannelAddr::Unresolved => "no address".to_owned(),
    }
}

/// The printed address of every channel of `modules` in `q`, by channel, with its module and number.
fn printed_addresses(
    q: &ew_derive::Q,
    modules: &[Id<Device>],
) -> Vec<(Id<Channel>, Id<Device>, u16, String)> {
    let mut out = Vec::new();
    for m in modules {
        for e in q.get::<ew_derive::ChannelAddresses>(*m).iter() {
            out.push((e.channel, *m, e.number, address_words(&e.address)));
        }
    }
    out
}

/// The re-address preview (FR-PLC-014): one notice per channel of `modules` whose printed address the transaction
/// changes, `channel 3 of -201U5: Q10.3 → Q2.3`, in allocation and channel order, computed on the transaction's
/// result.
fn readdress_notices(
    tx: &TxBuilder<'_>,
    before: &[(Id<Channel>, Id<Device>, u16, String)],
    modules: &[Id<Device>],
) -> Result<Vec<String>, CmdError> {
    let after = tx
        .base_model()
        .apply_history(&tx.ops(), tx.base_rev().next())
        .map_err(|e| CmdError::internal(format!("re-address preview: {e}")))?;
    let qa = ew_derive::Q::new(&after);
    let now: std::collections::BTreeMap<Id<Channel>, String> = printed_addresses(&qa, modules)
        .into_iter()
        .map(|(c, _, _, t)| (c, t))
        .collect();
    let mut out = Vec::new();
    for (channel, module, number, old) in before {
        let Some(new) = now.get(channel) else {
            continue;
        };
        if new != old {
            out.push(format!(
                "channel {number} of {}: {old} → {new}",
                crate::words::device(tx, *module)
            ));
        }
    }
    Ok(out)
}

/// `plc.freeze` (`FreezeAddresses`, doc 10 §3.9 `plc.freeze{cfg}`, FR-PLC-015): freezes the derived channel addresses
/// of a configuration project.
///
/// Every used or reserved channel (effective state, doc 10 §3.2) of the configuration project's modules whose address
/// is `auto` and derives to an address takes that address as a `manual` one, so that a later change of a start, a slot
/// or a part map moves no printed address and EW-PLC-012 (`manual`) reports every channel whose map now differs. A
/// spare or deactivated channel keeps its `auto` address, which cannot move while its module's start is pinned; a
/// `tbd`, `excluded`, `manual` or unresolved address stays. Every `auto` module start that A1 places takes its place as
/// a `manual` start too, so that a module inserted before a frozen one is placed around it (A1 reserves manual starts
/// first) instead of moving it from under its frozen channels. **Bulk** (the preview lists every channel); undoable as one step. Refused in words: the
/// configuration project's profile is tag-based (FR-PLC-009), whose addresses follow the station and the slot and
/// never move. The configuration project exists ([`CmdError::NotFound`]). Outcome: the configuration project, with a
/// notice that counts the frozen addresses.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FreezeAddresses {
    /// The configuration project.
    #[schemars(with = "UuidS")]
    pub config_project: Id<PlcConfigProject>,
}

impl Command for FreezeAddresses {
    const NAME: &'static str = "plc.freeze";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let cfg = tx.require::<PlcConfigProject>(self.config_project)?;
        let base = tx.base_model().clone();
        let q = ew_derive::Q::new(&base);
        if ew_derive::profile_of(&q, self.config_project).is_some_and(|p| !p.absolute) {
            return tx.reject(text!(
                "refusal-plc-freeze-tag-based",
                cfg.name.as_str(),
                cfg.profile.as_str()
            ));
        }
        let mut frozen = 0usize;
        for m in q.get::<ew_derive::ModuleOrder>(self.config_project).iter() {
            // The start too, in the stored form of `plc.set_start`, so that a module inserted before this one is
            // placed around it (A1 pass 1 reserves manual starts).
            if let Some(start) = *q.get::<ew_derive::ModuleStartOf>(m.module) {
                let pinned = |spec: &AddrSpec, at: Option<ew_derive::Start>, area| match (spec, at) {
                    (AddrSpec::Auto, Some(ew_derive::Start::At(byte))) => Some(AddrSpec::Manual(
                        ew_derive::stored(ew_derive::ew_plcaddr::PlcAddress::unit(
                            area,
                            ew_derive::ew_plcaddr::Size::Bit,
                            byte,
                        )),
                    )),
                    _ => None,
                };
                if let Some(p) = tx
                    .get::<Device>(m.module)
                    .and_then(|d| d.aspects.plc_module.clone())
                {
                    let input = pinned(&p.start.input, start.input, ew_derive::ew_plcaddr::Area::I);
                    let output = pinned(&p.start.output, start.output, ew_derive::ew_plcaddr::Area::Q);
                    if input.is_some() || output.is_some() {
                        tx.update::<Device>(m.module, |d| {
                            if let Some(p) = d.aspects.plc_module.as_mut() {
                                p.start.input = input.unwrap_or_else(|| p.start.input.clone());
                                p.start.output = output.unwrap_or_else(|| p.start.output.clone());
                            }
                        })?;
                    }
                }
            }
            // A spare or deactivated channel keeps its derived address: the pinned start holds it in place, and a
            // manual address on it would be EW-PLC-009's "manual address but no connection".
            let states = q.get::<ew_derive::ChannelStates>(m.module);
            for e in q.get::<ew_derive::ChannelAddresses>(m.module).iter() {
                let ew_derive::ChannelAddr::At { address, .. } = &e.address else {
                    continue;
                };
                let kept = states.iter().any(|(c, s)| {
                    *c == e.channel
                        && !matches!(
                            s,
                            ew_derive::EffectiveState::Used | ew_derive::EffectiveState::Reserved
                        )
                });
                if kept {
                    continue;
                }
                if tx
                    .get::<Channel>(e.channel)
                    .is_some_and(|c| c.address == AddrSpec::Auto)
                {
                    let manual = AddrSpec::Manual(ew_derive::stored(*address));
                    tx.update::<Channel>(e.channel, |c| c.address = manual)?;
                    frozen += 1;
                }
            }
        }
        let mut outcome = Outcome::primary((Kind::PlcConfigProject, self.config_project.cast()));
        if frozen > 0 {
            outcome.notices.push(format!(
                "{frozen} channel address(es) of configuration project {} frozen as manual addresses",
                cfg.name
            ));
        }
        Ok(outcome)
    }
}

/// `plc.readdress` (`Readdress`, doc 10 §3.9 `plc.readdress{scope}`, FR-PLC-014): re-addresses a selection of modules
/// or a configuration project by the profile's allocation scheme.
///
/// The scope is `modules` (each with the `plc_module` aspect), else every module of `config_project`. Every `manual`
/// start of a module in scope becomes `auto`, and so does every `manual` channel address of its channels; `tbd` and
/// `excluded` stay. Algorithm A1 then places the modules afresh. **Bulk:** the preview's notices list every channel of
/// the affected configuration projects whose printed address changes (`channel 3 of -201U5: Q10.3 → Q2.3`), the
/// following automatic modules included; committing the preview applies it as one undo step. Rejected: neither
/// `modules` nor `config_project` (at `/modules`), a device without the `plc_module` aspect, a record that does not
/// exist ([`CmdError::NotFound`]); a start the PLC owns since an import, until `plc.unlock` lifts the protection (doc
/// 10 §3.9; `plc.set_start` and `plc.convert_manual_to_auto` refuse it likewise). Outcome: the first module, or the
/// configuration project.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Readdress {
    /// The configuration project whose modules are re-addressed when `modules` is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub config_project: Option<Id<PlcConfigProject>>,
    /// The module devices to re-address.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<UuidS>")]
    pub modules: Vec<Id<Device>>,
}

impl Command for Readdress {
    const NAME: &'static str = "plc.readdress";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let modules = scope_modules(tx, Self::NAME, self.config_project, &self.modules)?;
        let base = tx.base_model().clone();
        let q = ew_derive::Q::new(&base);
        let affected = neighbourhood(&q, &modules);
        let before = printed_addresses(&q, &affected);
        for m in &modules {
            let manual_start = tx
                .get::<Device>(*m)
                .and_then(|d| d.aspects.plc_module.clone())
                .is_some_and(|p| {
                    matches!(p.start.input, AddrSpec::Manual(_))
                        || matches!(p.start.output, AddrSpec::Manual(_))
                });
            if manual_start {
                tx.update::<Device>(*m, |d| {
                    if let Some(p) = d.aspects.plc_module.as_mut() {
                        for s in [&mut p.start.input, &mut p.start.output] {
                            if matches!(s, AddrSpec::Manual(_)) {
                                *s = AddrSpec::Auto;
                            }
                        }
                    }
                })?;
            }
            for c in tx.channels_of_module(*m) {
                if tx
                    .get::<Channel>(c)
                    .is_some_and(|c| matches!(c.address, AddrSpec::Manual(_)))
                {
                    tx.update::<Channel>(c, |c| c.address = AddrSpec::Auto)?;
                }
            }
        }
        super::plc_exchange::refuse_owned_edits(tx)?;
        let notices = readdress_notices(tx, &before, &affected)?;
        let mut outcome = match (modules.first(), self.config_project) {
            (Some(m), _) if !self.modules.is_empty() => Outcome::primary((Kind::Device, m.cast())),
            (_, Some(cfg)) => Outcome::primary((Kind::PlcConfigProject, cfg.cast())),
            _ => Outcome::default(),
        };
        outcome.notices = notices;
        Ok(outcome)
    }
}

/// `plc.convert_manual_to_auto` (`ConvertManualToAuto`, doc 10 §3.9 `plc.convert_manual_to_auto{scope}`,
/// FR-PLC-014): drops the manual values that equal the derived ones.
///
/// The scope is that of [`Readdress`]. A `manual` channel address equal to the address A2 derives for the channel
/// becomes `auto`; a `manual` start equal to the start A1 places the module at as `auto` becomes `auto` (judged on the
/// result, so a start that only another manual start holds in place stays), unless that moves the printed address of
/// any channel of the affected configuration projects (an automatic module after it), as [`Readdress`] compares
/// them: then the start stays too. Nothing prints differently. Under a tag-based profile, which prints a manual
/// address as the derived one, every manual channel address becomes `auto`. Every manual value that stays gets a
/// notice with both values (`channel 3 of -201U5 keeps its manual address Q10.3 (derived: Q2.3)`, `module -201U5 keeps
/// its manual output start Q10 (automatic: Q2)`), or with the reason (`module -201U3 keeps its manual output start
/// Q1: as an automatic start it would move the addresses of other modules`). **Bulk**; one undo step. Rejected as
/// [`Readdress`]. Outcome: the first module, or the configuration project.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConvertManualToAuto {
    /// The configuration project whose modules are converted when `modules` is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub config_project: Option<Id<PlcConfigProject>>,
    /// The module devices to convert.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<UuidS>")]
    pub modules: Vec<Id<Device>>,
}

impl Command for ConvertManualToAuto {
    const NAME: &'static str = "plc.convert_manual_to_auto";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let modules = scope_modules(tx, Self::NAME, self.config_project, &self.modules)?;
        let base = tx.base_model().clone();
        let q = ew_derive::Q::new(&base);
        let affected = neighbourhood(&q, &modules);
        let before = printed_addresses(&q, &affected);
        let mut notices = Vec::new();
        // Channel addresses: at the derived address (under a tag-based profile every manual address, which prints as
        // derived), or kept with a notice.
        for m in &modules {
            for e in q.get::<ew_derive::ChannelAddresses>(*m).iter() {
                let Some(c) = tx.get::<Channel>(e.channel) else {
                    continue;
                };
                if !matches!(c.address, AddrSpec::Manual(_)) {
                    continue;
                }
                let at_derived = matches!(
                    &e.address,
                    ew_derive::ChannelAddr::At { address, .. } if e.auto == Some(*address)
                );
                if at_derived {
                    tx.update::<Channel>(e.channel, |c| c.address = AddrSpec::Auto)?;
                } else {
                    let derived = e
                        .auto
                        .zip(module_profile_of(&q, *m).filter(|p| p.absolute))
                        .map_or_else(
                            || "none".to_owned(),
                            |(auto, p)| p.render(&auto, ew_derive::ew_plcaddr::Purpose::Drawing),
                        );
                    notices.push(format!(
                        "channel {} of {} keeps its manual address {} (derived: {derived})",
                        e.number,
                        crate::words::device(tx, *m),
                        address_words(&e.address)
                    ));
                }
            }
        }
        // Module starts: converted together, then the ones whose automatic start differs are restored, until none
        // differs and no channel of the affected configuration projects prints another address (a start that keeps
        // its own place can still move the automatic modules after it): when none differs but something moved, the
        // last candidate is restored. Each round restores at least one, so it ends.
        let mut candidates: Vec<(Id<Device>, bool, u32)> = Vec::new();
        for m in &modules {
            let Some(p) = tx.get::<Device>(*m).and_then(|d| d.aspects.plc_module.clone()) else {
                continue;
            };
            for (output, spec) in [(false, &p.start.input), (true, &p.start.output)] {
                if let AddrSpec::Manual(a) = spec {
                    candidates.push((*m, output, a.byte));
                }
            }
        }
        let original: Vec<(Id<Device>, ew_schema::IoStarts)> = modules
            .iter()
            .filter_map(|m| {
                tx.get::<Device>(*m)
                    .and_then(|d| d.aspects.plc_module.as_ref().map(|p| (*m, p.start.clone())))
            })
            .collect();
        let mut kept: Vec<KeptStart> = Vec::new();
        while !candidates.is_empty() {
            for (m, output, _) in &candidates {
                tx.update::<Device>(*m, |d| {
                    if let Some(p) = d.aspects.plc_module.as_mut() {
                        if *output {
                            p.start.output = AddrSpec::Auto;
                        } else {
                            p.start.input = AddrSpec::Auto;
                        }
                    }
                })?;
            }
            let after = tx
                .base_model()
                .apply_history(&tx.ops(), tx.base_rev().next())
                .map_err(|e| CmdError::internal(format!("plc.convert_manual_to_auto: {e}")))?;
            let qa = ew_derive::Q::new(&after);
            let placed = |m: Id<Device>, output: bool| -> Option<u32> {
                let s = (*qa.get::<ew_derive::ModuleStartOf>(m))?;
                match if output { s.output } else { s.input } {
                    Some(ew_derive::Start::At(b)) => Some(b),
                    _ => None,
                }
            };
            let (mut stay, mut differ): (Vec<_>, Vec<_>) = candidates
                .iter()
                .copied()
                .partition(|(m, output, byte)| placed(*m, *output) == Some(*byte));
            let others_moved = differ.is_empty() && printed_addresses(&qa, &affected) != before;
            if others_moved {
                differ.extend(stay.pop());
            }
            if differ.is_empty() {
                break;
            }
            for (m, output, byte) in &differ {
                let start = original.iter().find(|(x, _)| x == m).map(|(_, s)| {
                    if *output {
                        s.output.clone()
                    } else {
                        s.input.clone()
                    }
                });
                if let Some(start) = start {
                    tx.update::<Device>(*m, |d| {
                        if let Some(p) = d.aspects.plc_module.as_mut() {
                            if *output {
                                p.start.output = start;
                            } else {
                                p.start.input = start;
                            }
                        }
                    })?;
                }
                kept.push((*m, *output, *byte, placed(*m, *output), others_moved));
            }
            candidates = stay;
        }
        for (m, output, byte, auto, others_moved) in kept {
            let area = if output {
                ew_derive::ew_plcaddr::Area::Q
            } else {
                ew_derive::ew_plcaddr::Area::I
            };
            let start = |b: u32| {
                module_profile_of(&q, m).map_or_else(
                    || b.to_string(),
                    |p| {
                        p.render(
                            &ew_derive::ew_plcaddr::PlcAddress::unit(
                                area,
                                ew_derive::ew_plcaddr::Size::Bit,
                                b,
                            ),
                            ew_derive::ew_plcaddr::Purpose::Drawing,
                        )
                    },
                )
            };
            let (module, dir) = (
                crate::words::device(tx, m),
                if output { "output" } else { "input" },
            );
            notices.push(if others_moved {
                format!(
                    "module {module} keeps its manual {dir} start {}: as an automatic start it would move the \
                     addresses of other modules",
                    start(byte)
                )
            } else {
                format!(
                    "module {module} keeps its manual {dir} start {} (automatic: {})",
                    start(byte),
                    auto.map_or_else(|| "none".to_owned(), start)
                )
            });
        }
        super::plc_exchange::refuse_owned_edits(tx)?;
        let mut outcome = match (modules.first(), self.config_project) {
            (Some(m), _) if !self.modules.is_empty() => Outcome::primary((Kind::Device, m.cast())),
            (_, Some(cfg)) => Outcome::primary((Kind::PlcConfigProject, cfg.cast())),
            _ => Outcome::default(),
        };
        outcome.notices = notices;
        Ok(outcome)
    }
}

/// A manual start `plc.convert_manual_to_auto` keeps: the module, whether it is the output start, the manual byte, the
/// byte A1 places it at as automatic, and whether it stays because converting it would move other modules.
type KeptStart = (Id<Device>, bool, u32, Option<u32>, bool);

/// The address profile of a module's configuration project in `q`.
fn module_profile_of(
    q: &ew_derive::Q,
    module: Id<Device>,
) -> Option<std::sync::Arc<ew_derive::ew_plcaddr::AddressProfile>> {
    let station = ew_derive::station_of(q, module)?;
    let cfg = q
        .devices()
        .get(station)?
        .aspects
        .plc_station
        .as_ref()?
        .config_project;
    ew_derive::profile_of(q, cfg)
}

/// `plc.add_module` (`AddModule`, doc 10 §6 the rack and slot editor, FR-PLC-028): adds a module of a part to a
/// station, in a slot.
///
/// Creates `Device {tag: {letter: the station's, counter: position, scheme: child}, parent: station, position, parts:
/// [part], plc_module: {slot}}` at the position after the station's last child; normaliser N1 instantiates its
/// channels. With `slot`, the module's slot is `rack.slot` (rack 0 when `rack` is absent); without, it is positional,
/// so the module follows the station's modules. With `count` (the rack dialog's count field), that many modules of the
/// part, in the slots from `slot` on, or one after the other. Refused: a station without the `plc_station` aspect; a part whose PLC
/// section is not a module's (in words). A slot outside the part's allowed slots and a station beyond its limits are
/// EW-PLC-011's prevent rule. `copies` are library
/// documents embedded first, in the same transaction, as `device.place` embeds them (a library part the project does not
/// hold yet). One undo step. Outcome: the module.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddModule {
    /// The station device.
    #[schemars(with = "UuidS")]
    pub station: Id<Device>,
    /// The module's part use.
    pub part: PartUse,
    /// The rack; absent: rack 0 when `slot` is given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rack: Option<u16>,
    /// The slot; absent: positional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<u16>,
    /// How many modules of the part fill the slots from `slot` on (or follow positionally); absent: one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u16>,
    /// Library documents (`lib/` JSON form) embedded first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<serde_json::Value>,
}

impl Command for AddModule {
    const NAME: &'static str = "plc.add_module";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        crate::commands::assign::embed_closure(Self::NAME, tx, &self.copies)?;
        let station = tx.require::<Device>(self.station)?;
        if station.aspects.plc_station.is_none() {
            return tx.reject(text!(
                "refusal-plc-add-module-no-station",
                crate::words::device(tx, self.station)
            ));
        }
        let part = tx.require::<LibCopy<Part>>(self.part.part.copy)?;
        let module = part
            .item
            .sections
            .plc
            .as_ref()
            .is_some_and(|p| p.role == ew_schema::PlcRole::Module);
        if !module {
            return tx.reject(text!(
                "refusal-plc-add-module-part",
                part.item.part_number.as_str()
            ));
        }
        let mut first = None;
        for n in 0..self.count.unwrap_or(1).max(1) {
            let id = self.add_one(tx, &station, self.slot.map(|s| s.saturating_add(n)))?;
            first.get_or_insert(id);
        }
        Ok(Outcome::primary((Kind::Device, first.unwrap_or(self.station).cast())))
    }
}

impl AddModule {
    /// One module of the part under `station`, at its next position, in `slot` (rack `rack`, else 0) or positional.
    fn add_one(&self, tx: &mut TxBuilder<'_>, station: &Device, slot: Option<u16>) -> Result<Id<Device>, CmdError> {
        let position = tx
            .children_of(self.station)
            .into_iter()
            .filter_map(|c| tx.get::<Device>(c).and_then(|d| d.position))
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let slot = match slot {
            Some(slot) => ew_schema::SlotPath::Rack(ew_schema::RackSlot {
                rack: self.rack.unwrap_or(0),
                slot,
                subslot: None,
                extra: Extra::new(),
            }),
            None => ew_schema::SlotPath::Positional,
        };
        let id: Id<Device> = tx.mint();
        tx.insert(Device {
            id,
            v: 1,
            tag: ew_schema::TagSpec {
                scheme: ew_schema::TagScheme::Child,
                ..ew_schema::TagSpec::new(station.tag.letter.clone(), Some(position))
            },
            kind: None,
            parent: Some(self.station),
            position: Some(position),
            structure: Default::default(),
            parts: vec![self.part.clone()],
            function_text: Default::default(),
            process_tag: None,
            tech_data: Default::default(),
            state: Default::default(),
            source_ref: None,
            aspects: ew_schema::DeviceAspects {
                plc_module: Some(ew_schema::PlcModule {
                    slot,
                    ..ew_schema::PlcModule::default()
                }),
                ..ew_schema::DeviceAspects::default()
            },
            extra: Extra::new(),
        })?;
        Ok(id)
    }
}

/// One module of `plc.add_station`: its part, in the next slot.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StationModule {
    /// The module's part use.
    pub part: PartUse,
}

/// `plc.add_station` (`AddStation`, F18 and F19 of the rc.1 test, doc 10 §2.3 "The rack comes first"): makes a station
/// of a PLC with its modules, in one undo step.
///
/// The station is a new device of `part`, drawn on no page yet (tag letter `A`, the online numbering gives it a
/// counter), or the existing `device` (a bus device given to a PLC from where it stands). It gets `plc_station` in
/// `config_project`, or in a new configuration project `PLC<n>` when that is absent (the station its CPU when it is
/// one). Its role is the one normaliser N0 gives its PLC section (valve terminal, head station or CPU) and, with a
/// section that declares I/O, the station's own module (`plc_module {integrated}`); a device whose parts have no PLC
/// section becomes an I/O device with an open rack. Then each of `modules` becomes a device under the station, as
/// `plc.add_module` makes it, at the next position, positional; normaliser N1 instantiates their channels. Refused in
/// words: both or neither of `device` and `part`; a device that is a station or a PLC module already; a `part` whose
/// PLC section is not a station's; a module part that is not a module's. `copies` are library documents embedded first.
/// Outcome: the station.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddStation {
    /// The existing device that becomes the station; absent: a new device of `part`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub device: Option<Id<Device>>,
    /// The new station's head part (without `device`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<PartUse>,
    /// The configuration project (the PLC) it belongs to; absent: a new one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub config_project: Option<Id<PlcConfigProject>>,
    /// Its modules in slot order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<StationModule>,
    /// Library documents (`lib/` JSON form) embedded first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<serde_json::Value>,
}

impl Command for AddStation {
    const NAME: &'static str = "plc.add_station";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        use crate::normalise::plc_aspects::{create_config_project, station_role};
        crate::commands::assign::embed_closure(Self::NAME, tx, &self.copies)?;
        let id = match (&self.device, &self.part) {
            (Some(d), None) => {
                let dev = tx.require::<Device>(*d)?;
                if dev.aspects.plc_station.is_some() || dev.aspects.plc_module.is_some() {
                    return tx.reject(text!("refusal-plc-station-already", crate::words::device(tx, *d)));
                }
                *d
            }
            (None, Some(part)) => {
                let copy = tx.require::<LibCopy<Part>>(part.part.copy)?;
                if !copy
                    .item
                    .sections
                    .plc
                    .as_ref()
                    .is_some_and(|p| p.role == ew_schema::PlcRole::Station)
                {
                    return tx.reject(text!(
                        "refusal-plc-station-part",
                        copy.item.part_number.as_str()
                    ));
                }
                let id: Id<Device> = tx.mint();
                tx.insert(Device {
                    id,
                    v: 1,
                    tag: ew_schema::TagSpec::new(
                        Ident::from_input("A")
                            .map_err(|e| CmdError::internal(format!("the letter A is no identifier ({e:?})")))?,
                        None,
                    ),
                    kind: None,
                    parent: None,
                    position: None,
                    structure: Default::default(),
                    parts: vec![part.clone()],
                    function_text: Default::default(),
                    process_tag: None,
                    tech_data: Default::default(),
                    state: Default::default(),
                    source_ref: None,
                    aspects: ew_schema::DeviceAspects::default(),
                    extra: Extra::new(),
                })?;
                id
            }
            _ => {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    "/device",
                    text!("refusal-plc-station-device-or-part"),
                ));
            }
        };
        let dev = tx.require::<Device>(id)?;
        let section = dev
            .parts
            .iter()
            .find_map(|u| tx.get::<LibCopy<Part>>(u.part.copy)?.item.sections.plc.clone())
            .filter(|s| s.role == ew_schema::PlcRole::Station);
        let role = section
            .as_ref()
            .map_or(ew_schema::StationRole::IoDevice, |s| station_role(tx, &dev, s));
        let cfg = match self.config_project {
            Some(c) => {
                let cfg = tx.require::<PlcConfigProject>(c)?;
                if cfg.cpu.is_none() && role == ew_schema::StationRole::Cpu {
                    tx.update::<PlcConfigProject>(c, |x| x.cpu = Some(id))?;
                }
                c
            }
            None => create_config_project(tx, &dev, (role == ew_schema::StationRole::Cpu).then_some(id))?,
        };
        let integrated = section.as_ref().is_some_and(|s| !s.io.is_empty());
        tx.update::<Device>(id, |d| {
            d.aspects.plc_station = Some(ew_schema::PlcStation {
                config_project: cfg,
                role,
                order: None,
                firmware: None,
                extra: Extra::new(),
            });
            if integrated {
                d.aspects.plc_module = Some(ew_schema::PlcModule {
                    integrated: true,
                    ..ew_schema::PlcModule::default()
                });
            }
        })?;
        for m in &self.modules {
            AddModule {
                station: id,
                part: m.part.clone(),
                rack: None,
                slot: None,
                count: None,
                copies: Vec::new(),
            }
            .run(tx)?;
        }
        Ok(Outcome::primary((Kind::Device, id.cast())))
    }
}

/// `plc.remove_from_rack` (`RemoveFromRack`, F17 of the rc.1 test, the quick fix of EW-PLC-017): takes a device out of
/// its station's rack.
///
/// The device leaves the station it sits under (`parent` and `position` absent). A device that nothing describes as a
/// module ([`ew_derive::not_a_module`]: a bus node whose part has no PLC data, attached to a CPU) also loses its
/// `plc_module` aspect and every channel of it, which no part gives. A module that is one keeps its channels and waits
/// under Modules without a station. Refused in words: a device that is neither a module in a station nor such a device (a
/// station itself, a valve on a valve terminal, a loose device). One undo step. Outcome: the device.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoveFromRack {
    /// The device.
    #[schemars(with = "UuidS")]
    pub device: Id<Device>,
}

impl Command for RemoveFromRack {
    const NAME: &'static str = "plc.remove_from_rack";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let id = self.device;
        let dev = tx.require::<Device>(id)?;
        let q = ew_derive::Q::new(tx.base_model());
        let fault = ew_derive::not_a_module(&q, id);
        let in_station = dev.aspects.plc_station.is_none()
            && dev.aspects.plc_module.is_some()
            && dev.parent.is_some_and(|p| ew_derive::station_of(&q, p).is_some());
        if !fault && !in_station {
            return tx.reject(text!("refusal-plc-not-in-rack", crate::words::device(tx, id)));
        }
        tx.update::<Device>(id, |d| {
            if in_station {
                d.parent = None;
                d.position = None;
            }
            if fault {
                d.aspects.plc_module = None;
            }
        })?;
        if fault {
            let doomed: Vec<_> = tx
                .channels_of_module(id)
                .into_iter()
                .map(|c| (Kind::Channel, c.cast()))
                .collect();
            if !doomed.is_empty() {
                tx.delete_many(&doomed)?;
            }
        }
        Ok(Outcome::primary((Kind::Device, id.cast())))
    }
}
