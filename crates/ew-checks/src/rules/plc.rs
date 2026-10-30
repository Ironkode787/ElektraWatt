// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The MVP rules of the EW-PLC pack (doc 10 §3.10, doc 15 §4.5.1; WP-S1.21 part 3, D10): EW-PLC-001, -002, -003,
//! -007, -008, -012 and -015, in the scopes `module` (a device with the `plc_module` aspect) and `cfg` (a configuration
//! project).
//!
//! Every rule reads the derived PLC values of `ew-derive` (doc 10 §2.10): the module starts and channel addresses
//! (A1, A2), the address index, the channel states (A3), texts and the solenoid-position slots. None stores or
//! recomputes an address of its own.
//!
//! # Texts (WP-S1.16 §10)
//!
//! A channel is named `<module tag>:<designation>` (its determined designation, else its number), an address by the
//! configuration project's profile (`Q0.1`), a device by its derived tag. IDs enter only targets and fingerprints.
//! Every finding carries its message arguments; the en-GB patterns of `messages/en-GB/rules.ftl` render the messages
//! byte for byte.
//!
//! # Prevent mode
//!
//! EW-PLC-001 and EW-PLC-007 carry the catalogue's prevent letter, so release mode reports them; their prevent
//! evaluation (refusing the edit that creates an overlap or a duplicate symbol) is not built: WP-S1.21 §10.2 item 6
//! admits no refusal beyond the address parse and the profile check in this package.

use std::collections::BTreeMap;

use ew_derive::ew_ids::Id;
use ew_derive::ew_plcaddr::{Area, Size};
use ew_derive::ew_schema::{AddrSpec, Channel, ChannelKind, Device, IoDir, Kind, PlcConfigProject};
use ew_derive::{
    AddressIndex, ChannelAddr, ChannelAddresses, ChannelStates, ChannelTexts, ChannelsByCp, EffectiveState,
    ModuleChannelIds, ModuleOrder, ModuleStartOf, Q, SolenoidPositions, Start, module_part, part_confidence,
    profile_of, station_of,
};

use crate::finding::{Finding, QuickFix, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::term::full_tag;

// ---------------------------------------------------------------------------------------------------------
// Shared helpers

/// A channel as printed: `<module tag>:<designation>`, the number where the channel has no designation.
fn channel_name(q: &Q, module: Id<Device>, channel: Id<Channel>, number: u16) -> String {
    let designation = q
        .get::<ChannelTexts>(module)
        .iter()
        .find(|(id, _)| *id == channel)
        .and_then(|(_, t)| t.designation.clone())
        .unwrap_or_else(|| number.to_string());
    format!("{}:{designation}", full_tag(q, module, "module"))
}

/// A derived address as printed: the profile's text, `TBD`, or `no address`.
fn address_text(a: &ChannelAddr) -> String {
    match a {
        ChannelAddr::At { text, .. } => text.clone(),
        ChannelAddr::Tbd => "TBD".to_owned(),
        ChannelAddr::Excluded | ChannelAddr::Unresolved => "no address".to_owned(),
    }
}

/// The configuration project of a module, through its station.
fn module_cfg(q: &Q, module: Id<Device>) -> Option<Id<PlcConfigProject>> {
    let station = station_of(q, module)?;
    Some(
        q.devices()
            .get(station)?
            .aspects
            .plc_station
            .as_ref()?
            .config_project,
    )
}

/// The name of a configuration project as printed.
fn cfg_name(q: &Q, cfg: Id<PlcConfigProject>) -> String {
    q.table::<PlcConfigProject>()
        .get(cfg)
        .map_or_else(|| "a configuration project".to_owned(), |c| c.name.to_string())
}

/// The direction of a channel kind: outputs for `do` and `ao`, inputs for `di`, `ai` and counters.
fn dir_of(kind: &ChannelKind) -> Option<IoDir> {
    match kind {
        ChannelKind::Do | ChannelKind::Ao => Some(IoDir::Out),
        ChannelKind::Di | ChannelKind::Ai | ChannelKind::Counter => Some(IoDir::In),
        _ => None,
    }
}

/// The module devices of a configuration project, in allocation order.
fn cfg_modules(q: &Q, cfg: Id<PlcConfigProject>) -> Vec<Id<Device>> {
    q.get::<ModuleOrder>(cfg).iter().map(|m| m.module).collect()
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-001

/// EW-PLC-001 "Address overlap" (doc 15 §4.5.1): conflict, o p, error, cfg (FR-PLC-044).
pub const ADDRESS_OVERLAP: RuleMeta = RuleMeta {
    id: "EW-PLC-001",
    code: "110001",
    pack: "EW-PLC",
    title: "Address overlap",
    kind: RuleKind::Conflict,
    modes: Modes::letters("op"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Cfg,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-PLC-001: channel addresses of one configuration project whose bit ranges overlap in one area (`IW0` against
/// `I1.3`; `address_index(cfg)`), one finding per overlap group (FX38a: the connected run of overlapping channels,
/// so N channels on one address are one finding, not N(N−1)/2), every member channel as a target, subject the first
/// in address order, discriminator `overlap`; the text names the first [`SHOWN_OVERLAP_CHANNELS`] channels with
/// their addresses and counts the rest ("and 7 more channels"); and a word, double-word or long-word address at a
/// byte that is not a multiple of the profile's word alignment, a warning (an unverified convention), discriminator
/// `alignment`. Automatic starts never overlap (A1), so a finding involves a manual value or a part map that gives
/// two channels one bit.
///
/// A group is chained, not a clique (FX38b): with `IW0`, `I0.0` and `I1.7` only `IW0` overlaps the others, so the
/// text asks to change addresses until none overlap rather than to move all but one. Waiver drift, kept as is: the
/// group's targets are all its members, so a waiver of an overlap group stops matching when the group's membership
/// changes (a channel joins or leaves it), and the user waives the new group again.
#[derive(Clone, Copy, Debug)]
pub struct AddressOverlap;

/// The most channels an EW-PLC-001 overlap text names; the rest are counted ("and 7 more channels").
pub const SHOWN_OVERLAP_CHANNELS: usize = 5;

impl Rule for AddressOverlap {
    fn meta(&self) -> &'static RuleMeta {
        &ADDRESS_OVERLAP
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Cfg(cfg) = *scope else {
            return Vec::new();
        };
        let index = q.get::<AddressIndex>(cfg);
        let name = cfg_name(q, cfg);
        let mut out = Vec::new();
        for group in &index.groups {
            let members: Vec<&ew_derive::AddressUse> = group.iter().map(|&i| &index.uses[i]).collect();
            let Some(first) = members.first() else {
                continue;
            };
            let subject = channel_name(q, first.module, first.channel, first.number);
            let shown: Vec<String> = members
                .iter()
                .take(SHOWN_OVERLAP_CHANNELS)
                .map(|u| format!("{} ({})", channel_name(q, u.module, u.channel, u.number), u.text))
                .collect();
            let shown = shown.join(", ");
            let more = members.len().saturating_sub(SHOWN_OVERLAP_CHANNELS);
            let mut message = format!("addresses overlap in configuration project {name}: {shown}");
            if more > 0 {
                message.push_str(&format!("; and {more} more channels"));
            }
            message.push_str("; change addresses until none overlap");
            out.push(
                Finding::new(
                    &ADDRESS_OVERLAP,
                    members.iter().map(|u| Target::new(Kind::Channel, u.channel)),
                    "overlap",
                    subject,
                    message,
                )
                .arg("form", "overlap")
                .arg("channels", shown)
                .arg(
                    "more",
                    if more > 0 {
                        more.to_string()
                    } else {
                        "none".to_owned()
                    },
                )
                .arg("cfg", name.clone()),
            );
        }
        let align = profile_of(q, cfg).map_or(1, |p| p.word_align.max(1));
        for u in &index.uses {
            if u.address.size.bits() >= Size::Word.bits() && u.address.byte % align != 0 {
                let channel = channel_name(q, u.module, u.channel, u.number);
                out.push(
                    Finding::new(
                        &ADDRESS_OVERLAP,
                        [Target::new(Kind::Channel, u.channel)],
                        "alignment",
                        channel.clone(),
                        format!(
                            "{channel}: the address {} does not start at a multiple of {align} bytes, the profile's \
                             word alignment (an unverified convention)",
                            u.text
                        ),
                    )
                    .with_severity(Severity::Warning)
                    .arg("form", "alignment")
                    .arg("channel", channel)
                    .arg("address", u.text.clone())
                    .arg("align", align.to_string()),
                );
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-002

/// EW-PLC-002 "Address outside the module" (doc 15 §4.5.1): conflict, o r, error, module (FR-PLC-044).
pub const OUTSIDE_MODULE: RuleMeta = RuleMeta {
    id: "EW-PLC-002",
    code: "110002",
    pack: "EW-PLC",
    title: "Address outside the module",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Module,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-PLC-002: a channel address outside its module's I/O range in the channel's direction: another area, or bytes
/// outside `[start, start + width)`, where the start is the module's derived start (A1) and the width the units of
/// the part's ranges in that direction. Only a module whose start in that direction is an address is checked (an
/// excluded, open or unresolved start bounds nothing). Target the channel, discriminator `range`.
#[derive(Clone, Copy, Debug)]
pub struct OutsideModule;

impl Rule for OutsideModule {
    fn meta(&self) -> &'static RuleMeta {
        &OUTSIDE_MODULE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Module(module) = *scope else {
            return Vec::new();
        };
        let Some(start) = *q.get::<ModuleStartOf>(module) else {
            return Vec::new();
        };
        let Some((_, plc)) = module_part(q, module) else {
            return Vec::new();
        };
        let unit_bits = module_cfg(q, module)
            .and_then(|c| profile_of(q, c))
            .map_or(8, |p| p.unit_bits.max(1));
        let units = |dir: &IoDir| -> u32 {
            plc.io
                .iter()
                .filter(|r| r.dir == *dir)
                .map(|r| r.bits.div_ceil(unit_bits))
                .fold(0u32, u32::saturating_add)
        };
        let channels = q.table::<Channel>();
        let mut out = Vec::new();
        for e in q.get::<ChannelAddresses>(module).iter() {
            let ChannelAddr::At { address, text } = &e.address else {
                continue;
            };
            let Some(dir) = channels.get(e.channel).and_then(|c| dir_of(&c.kind)) else {
                continue;
            };
            let (area, s) = match dir {
                IoDir::In => (Area::I, start.input),
                _ => (Area::Q, start.output),
            };
            let Some(Start::At(from)) = s else {
                continue;
            };
            let to = from.saturating_add(units(&dir));
            let last = address
                .byte
                .saturating_add(address.size.bits().div_ceil(unit_bits).saturating_sub(1));
            if address.area == area && address.byte >= from && last < to {
                continue;
            }
            let channel = channel_name(q, module, e.channel, e.number);
            let tag = full_tag(q, module, "module");
            let (range, form) = if to > from {
                (format!("bytes {from} to {} of area {area:?}", to - 1), "bytes")
            } else {
                (format!("no byte of area {area:?}"), "none")
            };
            out.push(
                Finding::new(
                    &OUTSIDE_MODULE,
                    [Target::new(Kind::Channel, e.channel)],
                    "range",
                    channel.clone(),
                    format!("{channel}: the address {text} is outside the I/O range of {tag} ({range})"),
                )
                .arg("channel", channel)
                .arg("address", text.clone())
                .arg("module", tag)
                .arg("range", form)
                .arg("area", format!("{area:?}"))
                .arg("first", from.to_string())
                .arg("last", to.saturating_sub(1).to_string()),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-003

/// EW-PLC-003 "Bit index out of range" (doc 15 §4.5.1): conflict, o r, error, module (FR-PLC-044).
pub const BIT_OUT_OF_RANGE: RuleMeta = RuleMeta {
    id: "EW-PLC-003",
    code: "110003",
    pack: "EW-PLC",
    title: "Bit index out of range",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Module,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-PLC-003: a stored manual address whose bit index is not below the profile's unit width (`I4.9` in an 8-bit
/// unit), which the address parser refuses but a file or an import can hold: a channel's manual address (target the
/// channel, discriminator `address`) or a module's manual start (target the module, discriminator `start.input` or
/// `start.output`). The width is the configuration project's profile's, 8 bits without one.
#[derive(Clone, Copy, Debug)]
pub struct BitOutOfRange;

impl Rule for BitOutOfRange {
    fn meta(&self) -> &'static RuleMeta {
        &BIT_OUT_OF_RANGE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Module(module) = *scope else {
            return Vec::new();
        };
        let Some(aspect) = q.devices().get(module).and_then(|d| d.aspects.plc_module.clone()) else {
            return Vec::new();
        };
        let unit_bits = module_cfg(q, module)
            .and_then(|c| profile_of(q, c))
            .map_or(8, |p| p.unit_bits);
        let bad = |spec: &AddrSpec| -> Option<u8> {
            match spec {
                AddrSpec::Manual(a) => a.bit.filter(|b| u32::from(*b) >= unit_bits),
                _ => None,
            }
        };
        let tag = full_tag(q, module, "module");
        let mut out = Vec::new();
        for (which, spec) in [("input", &aspect.start.input), ("output", &aspect.start.output)] {
            if let Some(bit) = bad(spec) {
                out.push(
                    Finding::new(
                        &BIT_OUT_OF_RANGE,
                        [Target::new(Kind::Device, module)],
                        format!("start.{which}"),
                        tag.clone(),
                        format!(
                            "{tag}: the {which} start names bit {bit}, outside the {unit_bits} bits of a unit"
                        ),
                    )
                    .arg("subject", tag.clone())
                    .arg("what", which)
                    .arg("bit", bit.to_string())
                    .arg("width", unit_bits.to_string()),
                );
            }
        }
        let channels = q.table::<Channel>();
        for (number, id) in q.get::<ModuleChannelIds>(module).iter() {
            let Some(bit) = channels.get(*id).and_then(|c| bad(&c.address)) else {
                continue;
            };
            let channel = channel_name(q, module, *id, *number);
            out.push(
                Finding::new(
                    &BIT_OUT_OF_RANGE,
                    [Target::new(Kind::Channel, *id)],
                    "address",
                    channel.clone(),
                    format!(
                        "{channel}: the manual address names bit {bit}, outside the {unit_bits} bits of a unit"
                    ),
                )
                .arg("subject", channel)
                .arg("what", "address")
                .arg("bit", bit.to_string())
                .arg("width", unit_bits.to_string()),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-007

/// EW-PLC-007 "Duplicate symbol" (doc 15 §4.5.1): conflict, o p r, error, cfg (FR-PLC-016).
pub const DUPLICATE_SYMBOL: RuleMeta = RuleMeta {
    id: "EW-PLC-007",
    code: "110007",
    pack: "EW-PLC",
    title: "Duplicate symbol",
    kind: RuleKind::Conflict,
    modes: Modes::letters("opr"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Cfg,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-PLC-007: a determined symbol that two or more channels of one configuration project carry (symbols compare
/// exactly; the determined symbol is the manual one until the symbol templates of 1.0). One finding per symbol, every
/// channel that carries it as a target, discriminator `symbol`; the channels listed in natural order.
#[derive(Clone, Copy, Debug)]
pub struct DuplicateSymbol;

impl Rule for DuplicateSymbol {
    fn meta(&self) -> &'static RuleMeta {
        &DUPLICATE_SYMBOL
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Cfg(cfg) = *scope else {
            return Vec::new();
        };
        let mut by_symbol: BTreeMap<String, Vec<(String, Id<Channel>)>> = BTreeMap::new();
        for module in cfg_modules(q, cfg) {
            let channels = q.table::<Channel>();
            for (id, text) in q.get::<ChannelTexts>(module).iter() {
                let Some(symbol) = &text.symbol else {
                    continue;
                };
                let number = channels.get(*id).map_or(0, |c| c.number);
                by_symbol
                    .entry(symbol.clone())
                    .or_default()
                    .push((channel_name(q, module, *id, number), *id));
            }
        }
        let name = cfg_name(q, cfg);
        let mut out = Vec::new();
        for (symbol, mut channels) in by_symbol {
            if channels.len() < 2 {
                continue;
            }
            channels.sort_by(|a, b| {
                ew_derive::ew_mltext::natural_cmp(&a.0, &b.0)
                    .then_with(|| a.0.cmp(&b.0))
                    .then_with(|| a.1.cmp(&b.1))
            });
            let list = channels
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            out.push(
                Finding::new(
                    &DUPLICATE_SYMBOL,
                    channels.iter().map(|(_, id)| Target::new(Kind::Channel, *id)),
                    "symbol",
                    symbol.clone(),
                    format!(
                        "the symbol {symbol} is used by {} channels of configuration project {name}: {list}; give \
                         each channel its own symbol",
                        channels.len()
                    ),
                )
                .arg("symbol", symbol)
                .arg("count", channels.len().to_string())
                .arg("cfg", name.clone())
                .arg("channels", list),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-008

/// EW-PLC-008 "Used channel without address" (doc 15 §4.5.1): completeness, o x r, warning, module (FR-PLC-044).
pub const USED_WITHOUT_ADDRESS: RuleMeta = RuleMeta {
    id: "EW-PLC-008",
    code: "110008",
    pack: "EW-PLC",
    title: "Used channel without address",
    kind: RuleKind::Completeness,
    modes: Modes::letters("oxr"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Module,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-PLC-008: a used channel (effective state `used`, A3) that resolves to no address: an excluded address or
/// start, or no derivable address (a module without a station or configuration project, D4: "place a CPU first";
/// an address form of a newer format). A `Tbd` address is EW-PLC-015's. Target the channel, discriminator `address`.
/// The export mode's error severity waits for the exporter's check (the export mode is not modelled yet).
#[derive(Clone, Copy, Debug)]
pub struct UsedWithoutAddress;

impl Rule for UsedWithoutAddress {
    fn meta(&self) -> &'static RuleMeta {
        &USED_WITHOUT_ADDRESS
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Module(module) = *scope else {
            return Vec::new();
        };
        let states = q.get::<ChannelStates>(module);
        let addresses = q.get::<ChannelAddresses>(module);
        let mut out = Vec::new();
        for e in addresses.iter() {
            if !matches!(e.address, ChannelAddr::Excluded | ChannelAddr::Unresolved) {
                continue;
            }
            let used = states
                .iter()
                .any(|(id, s)| *id == e.channel && *s == EffectiveState::Used);
            if !used {
                continue;
            }
            let channel = channel_name(q, module, e.channel, e.number);
            let (reason, why) = if module_cfg(q, module).is_none() {
                (
                    "station",
                    "its module belongs to no PLC station; place a CPU first or attach the module to a station",
                )
            } else if e.address == ChannelAddr::Excluded {
                ("excluded", "it is excluded from addressing; give it an address")
            } else {
                (
                    "other",
                    "no address can be derived for it; check the module's start and its part's PLC section",
                )
            };
            out.push(
                Finding::new(
                    &USED_WITHOUT_ADDRESS,
                    [Target::new(Kind::Channel, e.channel)],
                    "address",
                    channel.clone(),
                    format!("{channel} is used but has no address: {why}"),
                )
                .arg("channel", channel)
                .arg("reason", reason),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-012

/// EW-PLC-012 "Binding differs from the part map" (doc 15 §4.5.1): conflict, o r, error, module (FR-PLC-045);
/// supersedes EW-NUM-002 for addresses.
pub const DIFFERS_FROM_MAP: RuleMeta = RuleMeta {
    id: "EW-PLC-012",
    code: "110012",
    pack: "EW-PLC",
    title: "Binding differs from the part map",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Module,
    supersedes: &["EW-NUM-002"],
    release: Release::Mvp,
};

/// EW-PLC-012 (doc 10 §3.10, D-26): a binding or an address that differs from the part's address map, by
/// discriminator:
///
/// - **`binding`**: a valve child's solenoid whose coil CP a channel binds, where the module part's solenoid-position
///   map assigns another channel of the module (`solenoid_positions(module)`): one finding per binding, the bound
///   channel and the coil's function as targets. It names the bound channel, the channel the map expects, both
///   addresses, and offers the quick fix "Rebind to the map's channel" (`plc.bind_channel` of the map's channel to the
///   coil CP, which unbinds the other, previewed). On Golden A, map H1 gives 28 findings and H2 none (AC-29); the second
///   fix of the catalogue, "open the part's PLC section", is a window action, not a command (a finding carries one
///   command fix).
/// - **`manual`**: a channel whose manual address differs from the address A2 derives for it (`auto`); target the
///   channel.
///
/// Both read the module's part, so a part whose `plc` section is not of confidence `verified` marks the findings as
/// resting on unverified data (FT-323's PLC half, `part_confidence(copy, "plc")`).
#[derive(Clone, Copy, Debug)]
pub struct DiffersFromMap;

impl Rule for DiffersFromMap {
    fn meta(&self) -> &'static RuleMeta {
        &DIFFERS_FROM_MAP
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Module(module) = *scope else {
            return Vec::new();
        };
        let Some((copy, _)) = module_part(q, module) else {
            return Vec::new();
        };
        let confidence = part_confidence(&copy, "plc");
        let part = copy.item.part_number.as_str().to_owned();
        let addresses = q.get::<ChannelAddresses>(module);
        let by_number: BTreeMap<u16, Id<Channel>> =
            q.get::<ModuleChannelIds>(module).iter().copied().collect();
        let channels = q.table::<Channel>();
        let address_of = |id: Id<Channel>| -> String {
            addresses
                .iter()
                .find(|e| e.channel == id)
                .map_or_else(|| "no address".to_owned(), |e| address_text(&e.address))
        };
        let mut out = Vec::new();
        let slots = q.get::<SolenoidPositions>(module);
        if slots.iter().any(|s| s.coil.is_some()) {
            let bound = q.get::<ChannelsByCp>(());
            for s in slots.iter() {
                let (Some(coil), Some(expected)) = (s.coil, s.channel) else {
                    continue;
                };
                let expected_id = by_number.get(&expected).copied();
                for at in bound.at_cp(coil.function, coil.idx) {
                    if Some(at) == expected_id {
                        continue;
                    }
                    let Some(c) = channels.get(at) else {
                        continue;
                    };
                    let channel = channel_name(q, c.module, at, c.number);
                    let address = q
                        .get::<ChannelAddresses>(c.module)
                        .iter()
                        .find(|e| e.channel == at)
                        .map_or_else(|| "no address".to_owned(), |e| address_text(&e.address));
                    let valve = s
                        .valve
                        .map_or_else(|| "the valve".to_owned(), |v| full_tag(q, v, "valve"));
                    let expected_name = match expected_id {
                        Some(id) => channel_name(q, module, id, expected),
                        None => format!("{}:{expected}", full_tag(q, module, "module")),
                    };
                    let expected_address = expected_id.map_or_else(|| "no address".to_owned(), address_of);
                    let mut f = Finding::new(
                        &DIFFERS_FROM_MAP,
                        [
                            Target::new(Kind::Channel, at),
                            Target::new(Kind::Function, coil.function),
                        ],
                        "binding",
                        channel.clone(),
                        format!(
                            "{channel} ({address}) drives solenoid {} of {valve}, which the position map of part \
                             {part} assigns to {expected_name} ({expected_address}); rebind it to the map's channel, \
                             or correct the part's PLC section",
                            s.solenoid
                        ),
                    )
                    .arg("form", "binding")
                    .arg("channel", channel)
                    .arg("address", address)
                    .arg("solenoid", s.solenoid.as_str())
                    .arg("valve", valve)
                    .arg("part", part.clone())
                    .arg("expected", expected_name)
                    .arg("expected_address", expected_address)
                    .reading(&confidence);
                    if let Some(id) = expected_id {
                        f = f.with_fix(QuickFix {
                            label: "Rebind to the map's channel".to_owned(),
                            command: "plc.bind_channel".to_owned(),
                            params: serde_json::json!({
                                "channel": id.to_string(),
                                "endpoint": {"cp": {"function": coil.function.to_string(), "idx": coil.idx}},
                            }),
                        });
                    }
                    out.push(f);
                }
            }
        }
        for e in addresses.iter() {
            let manual = channels
                .get(e.channel)
                .is_some_and(|c| matches!(c.address, AddrSpec::Manual(_)));
            let (true, ChannelAddr::At { address, text }, Some(auto)) = (manual, &e.address, e.auto) else {
                continue;
            };
            if *address == auto {
                continue;
            }
            let derived = module_cfg(q, module)
                .and_then(|c| profile_of(q, c))
                .map_or_else(String::new, |p| {
                    p.render(&auto, ew_derive::ew_plcaddr::Purpose::Drawing)
                });
            let channel = channel_name(q, module, e.channel, e.number);
            out.push(
                Finding::new(
                    &DIFFERS_FROM_MAP,
                    [Target::new(Kind::Channel, e.channel)],
                    "manual",
                    channel.clone(),
                    format!(
                        "{channel}: the manual address {text} differs from the derived address {derived}; set the \
                         address to auto, or keep the deviation deliberately"
                    ),
                )
                .arg("form", "manual")
                .arg("channel", channel)
                .arg("address", text.clone())
                .arg("derived", derived)
                .reading(&confidence),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-015

/// EW-PLC-015 "`Tbd` address" (doc 15 §4.5.1): completeness, o r, info/error, cfg (FR-PLC-013); supersedes
/// EW-CORE-044 for module starts and channel addresses.
pub const TBD_ADDRESS: RuleMeta = RuleMeta {
    id: "EW-PLC-015",
    code: "110015",
    pack: "EW-PLC",
    title: "Tbd address",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Info,
    release_severity: Some(Severity::Error),
    scope: ScopeKind::Cfg,
    supersedes: &["EW-CORE-044"],
    release: Release::Mvp,
};

/// EW-PLC-015: a `Tbd` module start or channel address of a module of the configuration project (D-15), one finding
/// per open value: a start with the module as target and discriminator `start.input` or `start.output`; a channel
/// address with the module and the channel as targets and discriminator `address`. The targets are EW-CORE-044's for
/// the same values, so its findings are superseded (doc 15 §3.6: one defect, one finding); a module outside every
/// configuration project stays EW-CORE-044's. A bus address is EW-BUS-001's, never this rule's. An info online, an
/// error in release mode (an open item blocks release).
#[derive(Clone, Copy, Debug)]
pub struct TbdAddress;

impl Rule for TbdAddress {
    fn meta(&self) -> &'static RuleMeta {
        &TBD_ADDRESS
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Cfg(cfg) = *scope else {
            return Vec::new();
        };
        let channels = q.table::<Channel>();
        let mut out = Vec::new();
        for module in cfg_modules(q, cfg) {
            let Some(aspect) = q.devices().get(module).and_then(|d| d.aspects.plc_module.clone()) else {
                continue;
            };
            let tag = full_tag(q, module, "module");
            for (which, spec) in [("input", &aspect.start.input), ("output", &aspect.start.output)] {
                if *spec == AddrSpec::Tbd {
                    out.push(
                        Finding::new(
                            &TBD_ADDRESS,
                            [Target::new(Kind::Device, module)],
                            format!("start.{which}"),
                            tag.clone(),
                            format!("{tag}: the {which} start address is TBD; enter it before release"),
                        )
                        .arg("subject", tag.clone())
                        .arg("what", which),
                    );
                }
            }
            for (number, id) in q.get::<ModuleChannelIds>(module).iter() {
                if !channels.get(*id).is_some_and(|c| c.address == AddrSpec::Tbd) {
                    continue;
                }
                let channel = channel_name(q, module, *id, *number);
                out.push(
                    Finding::new(
                        &TBD_ADDRESS,
                        [Target::new(Kind::Device, module), Target::new(Kind::Channel, *id)],
                        "address",
                        channel.clone(),
                        format!("{channel}: the channel address is TBD; enter it before release"),
                    )
                    .arg("subject", channel)
                    .arg("what", "address"),
                );
            }
        }
        out
    }
}
