// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The 1.0 rules of the EW-PLC pack (doc 10 §3.10, doc 15 §4.5.1): EW-PLC-004 (channel kind against the connected
//! function), -005 (symbol not an identifier), -009 (channel without connection), -010 (data-type width), -011 (slot
//! rules), -013 (card supply) and -014 (output load). They read the derived PLC values of `ew-derive` (targets,
//! states, texts, port windows) and the part's `plc` section; a part whose `plc` section is not of confidence
//! `verified` marks the findings that rest on it.

use std::collections::BTreeMap;

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{
    AddrSpec, BondKind, ChannelKind, ContactKind, Device, Kind, PlcConfigProject, PlcRole, PlcSection,
    PotentialType, SupplyFeeds, SupplyKind, TechVal, Unit,
};
use ew_derive::{
    ChannelStates, ChannelTargets, ChannelTexts, CpRef, EffectiveState, FunctionCps, ModuleOrder, Q, Slot,
    channel_data_type, cp_label, is_iec_ident, module_part, part_confidence, potential_id_at,
};

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind, Touched};
use crate::rules::plc::{cfg_modules, cfg_name, channel_name, kind_word, new_findings};
use crate::rules::pot::{PinWords, final_type, potential_name};
use crate::rules::term::full_tag;

/// The PLC section a station takes its limits from: the first part use whose part's `plc` section has role
/// `station`, else the station's module section.
fn station_section(
    q: &Q,
    station: Id<Device>,
) -> Option<(
    std::sync::Arc<ew_derive::ew_schema::LibCopy<ew_derive::ew_schema::Part>>,
    PlcSection,
)> {
    let dev = q.devices().get(station)?;
    let parts = q.parts();
    dev.parts
        .iter()
        .find_map(|u| {
            let copy = parts.get(u.part.copy)?;
            let plc = copy
                .item
                .sections
                .plc
                .clone()
                .filter(|s| s.role == PlcRole::Station)?;
            Some((std::sync::Arc::clone(copy), plc))
        })
        .or_else(|| module_part(q, station))
}

/// The modules of a station, in allocation order.
fn station_modules(q: &Q, station: Id<Device>) -> Vec<(Id<Device>, Slot)> {
    let Some(cfg) = q
        .devices()
        .get(station)
        .and_then(|d| d.aspects.plc_station.as_ref().map(|s| s.config_project))
    else {
        return Vec::new();
    };
    q.get::<ModuleOrder>(cfg)
        .iter()
        .filter(|m| m.station == station)
        .map(|m| (m.module, m.slot))
        .collect()
}

/// The devices of a station's tree: the station and its descendants, a nested station ending the walk below it.
fn station_tree(q: &Q, station: Id<Device>) -> Vec<Id<Device>> {
    let devices = q.devices();
    let mut out = Vec::new();
    let mut stack = vec![station];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(d) = stack.pop() {
        if !seen.insert(d) {
            continue;
        }
        out.push(d);
        let mut children: Vec<Id<Device>> = q
            .idx()
            .device_children(d)
            .filter(|c| devices.get(*c).is_some_and(|x| x.aspects.plc_station.is_none()))
            .collect();
        children.sort();
        stack.extend(children.into_iter().rev());
    }
    out
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-004

/// EW-PLC-004 "Channel kind mismatch" (doc 15 §4.5.1): conflict, f r, warning, module (FR-PLC-046).
pub const KIND_MISMATCH: RuleMeta = RuleMeta {
    id: "EW-PLC-004",
    code: "110004",
    pack: "EW-PLC",
    title: "Channel kind mismatch",
    kind: RuleKind::Conflict,
    modes: Modes::letters("fr"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Module,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-plc-004",
};

/// EW-PLC-004: an output channel (DO, AO) whose target is a contact (a sensor, `no`, `nc`, `co`), or an input channel
/// (DI, AI, counter) whose target is a coil or a valve solenoid (an actuator). One finding per channel; targets the
/// channel and the target function. A black box and the other functions are not judged.
#[derive(Clone, Copy, Debug)]
pub struct KindMismatch;

impl Rule for KindMismatch {
    fn meta(&self) -> &'static RuleMeta {
        &KIND_MISMATCH
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Module(module) = *scope else {
            return Vec::new();
        };
        let channels = q.table::<ew_derive::ew_schema::Channel>();
        let mut out = Vec::new();
        for t in q.get::<ChannelTargets>(module).iter() {
            let (Some(c), Some(f)) = (channels.get(t.channel), t.function) else {
                continue;
            };
            let Some(contact) = q
                .functions()
                .get(f)
                .and_then(|x| q.fndefs().get(x.def).map(|d| d.item.contact_kind.clone()))
            else {
                continue;
            };
            let sensor = matches!(contact, ContactKind::No | ContactKind::Nc | ContactKind::Co);
            let actuator = matches!(contact, ContactKind::Coil | ContactKind::Valve);
            let form = match c.kind {
                ChannelKind::Do | ChannelKind::Ao if sensor => "output",
                ChannelKind::Di | ChannelKind::Ai | ChannelKind::Counter if actuator => "input",
                _ => continue,
            };
            let channel = channel_name(q, module, c.id, c.number);
            let target = t
                .device
                .map_or_else(|| "its target".to_owned(), |d| full_tag(q, d, "its target"));
            let kind = kind_word(&c.kind);
            let message = match form {
                "output" => format!(
                    "{channel}: the {kind} channel drives the contact of {target}; an output drives an actuator, check \
                     the wiring or the channel kind"
                ),
                _ => format!(
                    "{channel}: the {kind} channel reads the coil of {target}; an input reads a sensor, check the \
                     wiring or the channel kind"
                ),
            };
            out.push(
                Finding::new(
                    &KIND_MISMATCH,
                    [Target::new(Kind::Channel, c.id), Target::new(Kind::Function, f)],
                    form,
                    channel.clone(),
                    message,
                )
                .arg("form", form)
                .arg("channel", channel)
                .arg("kind", kind)
                .arg("target", target),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-005

/// EW-PLC-005 "Invalid symbol" (doc 15 §4.5.1): conflict, x r, error, cfg (FR-PLC-047).
pub const INVALID_SYMBOL: RuleMeta = RuleMeta {
    id: "EW-PLC-005",
    code: "110005",
    pack: "EW-PLC",
    title: "Invalid symbol",
    kind: RuleKind::Conflict,
    modes: Modes::letters("xr"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Cfg,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-plc-005",
};

/// EW-PLC-005: under the configuration project's strict IEC policy (the default; `SymbolPolicy.strict_iec`) a
/// determined symbol that is not an IEC 61131-3 identifier (letters, digits and single underscores, no leading
/// digit, no trailing underscore): "TA05-V-15" as a manual symbol. One finding per channel, target the channel. A
/// lenient policy (TIA quoting) checks nothing.
#[derive(Clone, Copy, Debug)]
pub struct InvalidSymbol;

impl Rule for InvalidSymbol {
    fn meta(&self) -> &'static RuleMeta {
        &INVALID_SYMBOL
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Cfg(cfg) = *scope else {
            return Vec::new();
        };
        let strict = q
            .table::<PlcConfigProject>()
            .get(cfg)
            .is_none_or(|c| c.symbols.as_ref().is_none_or(|s| s.strict_iec));
        if !strict {
            return Vec::new();
        }
        let channels = q.table::<ew_derive::ew_schema::Channel>();
        let name = cfg_name(q, cfg);
        let mut out = Vec::new();
        for module in cfg_modules(q, cfg) {
            for (id, text) in q.get::<ChannelTexts>(module).iter() {
                let Some(symbol) = text.symbol.as_ref().filter(|s| !is_iec_ident(s)) else {
                    continue;
                };
                let number = channels.get(*id).map_or(0, |c| c.number);
                let channel = channel_name(q, module, *id, number);
                out.push(
                    Finding::new(
                        &INVALID_SYMBOL,
                        [Target::new(Kind::Channel, *id)],
                        "symbol",
                        channel.clone(),
                        format!(
                            "{channel}: the symbol {symbol} is not an IEC 61131-3 identifier, which configuration \
                             project {name} requires; use letters, digits and single underscores"
                        ),
                    )
                    .arg("channel", channel)
                    .arg("symbol", symbol.clone())
                    .arg("cfg", name.clone()),
                );
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-009

/// EW-PLC-009 "Channel without connection" (doc 15 §4.5.1): completeness, o r, warning, module (FR-PLC-046).
pub const WITHOUT_CONNECTION: RuleMeta = RuleMeta {
    id: "EW-PLC-009",
    code: "110009",
    pack: "EW-PLC",
    title: "Channel without connection",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Module,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-plc-009",
};

/// EW-PLC-009: a channel that carries a manual address or a symbol but reaches nothing (no target, and not used
/// through a wired pin of its own module), and that is not marked spare, reserved or deactivated (`Channel.state`
/// unset): the address or symbol was given for a channel that is not connected. One finding per channel, target the
/// channel. A channel whose part's default state makes it spare is judged too, since nobody marked it.
#[derive(Clone, Copy, Debug)]
pub struct WithoutConnection;

impl Rule for WithoutConnection {
    fn meta(&self) -> &'static RuleMeta {
        &WITHOUT_CONNECTION
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Module(module) = *scope else {
            return Vec::new();
        };
        let channels = q.table::<ew_derive::ew_schema::Channel>();
        let targets = q.get::<ChannelTargets>(module);
        let states = q.get::<ChannelStates>(module);
        let texts = q.get::<ChannelTexts>(module);
        let mut out = Vec::new();
        for (id, state) in states.iter() {
            let Some(c) = channels.get(*id) else {
                continue;
            };
            let connected = *state == EffectiveState::Used
                || targets.iter().any(|t| t.channel == *id && t.function.is_some());
            if connected || c.state.is_some() {
                continue;
            }
            let manual = matches!(c.address, AddrSpec::Manual(_));
            let symbol = texts
                .iter()
                .find(|(x, _)| x == id)
                .and_then(|(_, t)| t.symbol.clone());
            let what = match (manual, &symbol) {
                (true, Some(_)) => "both",
                (true, None) => "address",
                (false, Some(_)) => "symbol",
                (false, None) => continue,
            };
            let channel = channel_name(q, module, c.id, c.number);
            let given = match what {
                "both" => "a manual address and a symbol",
                "address" => "a manual address",
                _ => "a symbol",
            };
            out.push(
                Finding::new(
                    &WITHOUT_CONNECTION,
                    [Target::new(Kind::Channel, c.id)],
                    what,
                    channel.clone(),
                    format!(
                        "{channel} has {given} but no connection; connect it, or mark it spare or reserved"
                    ),
                )
                .arg("channel", channel)
                .arg("what", what),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-010

/// EW-PLC-010 "Data-type width mismatch" (doc 15 §4.5.1): conflict, o r, error, module (FR-PLC-046).
pub const WIDTH_MISMATCH: RuleMeta = RuleMeta {
    id: "EW-PLC-010",
    code: "110010",
    pack: "EW-PLC",
    title: "Data-type width mismatch",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Module,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-plc-010",
};

/// The width in bits of an IEC 61131-3 elementary data type (compared without regard to case); `None` for any other
/// name (a struct or a vendor type is not judged).
fn type_bits(name: &str) -> Option<u32> {
    Some(match name.to_ascii_lowercase().as_str() {
        "bool" => 1,
        "byte" | "sint" | "usint" | "char" => 8,
        "word" | "int" | "uint" => 16,
        "dword" | "dint" | "udint" | "real" => 32,
        "lword" | "lint" | "ulint" | "lreal" => 64,
        _ => return None,
    })
}

/// The width in bits of a channel kind: one bit for a digital channel, a word for an analogue one; `None` for the
/// others (counter, IO-Link port, multi-function port, supply).
fn kind_bits(kind: &ChannelKind) -> Option<u32> {
    match kind {
        ChannelKind::Di | ChannelKind::Do => Some(1),
        ChannelKind::Ai | ChannelKind::Ao => Some(16),
        _ => None,
    }
}

/// EW-PLC-010: a channel whose data type (its own, else its part's) is an elementary type of another width than the
/// channel: `Bool` (1 bit) on a 16-bit AI. One finding per channel, target the channel; it rests on the part's
/// `plc` section when the data type is the part's.
#[derive(Clone, Copy, Debug)]
pub struct WidthMismatch;

impl Rule for WidthMismatch {
    fn meta(&self) -> &'static RuleMeta {
        &WIDTH_MISMATCH
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Module(module) = *scope else {
            return Vec::new();
        };
        let confidence = module_part(q, module).map(|(copy, _)| part_confidence(&copy, "plc"));
        let channels = q.table::<ew_derive::ew_schema::Channel>();
        let mut out = Vec::new();
        for (id, _) in q.get::<ChannelStates>(module).iter() {
            let Some(c) = channels.get(*id) else {
                continue;
            };
            let Some(width) = kind_bits(&c.kind) else {
                continue;
            };
            let Some(data_type) = channel_data_type(q, module, c) else {
                continue;
            };
            let Some(bits) = type_bits(&data_type).filter(|b| *b != width) else {
                continue;
            };
            let channel = channel_name(q, module, c.id, c.number);
            let kind = kind_word(&c.kind);
            let mut f = Finding::new(
                &WIDTH_MISMATCH,
                [Target::new(Kind::Channel, c.id)],
                "width",
                channel.clone(),
                format!(
                    "{channel}: the data type {data_type} ({bits} bit(s)) does not fit the {width}-bit {kind} channel; \
                     give the channel a data type of its width"
                ),
            )
            .arg("channel", channel)
            .arg("data_type", data_type)
            .arg("bits", bits.to_string())
            .arg("width", width.to_string())
            .arg("kind", kind);
            if c.data_type.is_none() {
                if let Some(confidence) = &confidence {
                    f = f.reading(confidence);
                }
            }
            out.push(f);
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-011

/// EW-PLC-011 "Slot rules" (doc 15 §4.5.1): conflict, p o r, error, station (FR-PLC-028).
pub const SLOT_RULES: RuleMeta = RuleMeta {
    id: "EW-PLC-011",
    code: "110011",
    pack: "EW-PLC",
    title: "Slot rules",
    kind: RuleKind::Conflict,
    modes: Modes::letters("por"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Station,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-plc-011",
};

/// EW-PLC-011: over the modules of a station, a module in a slot its part does not allow (`PlcSection.allowed_slots`,
/// discriminator `slot`, target the module), more modules than the station's part allows (`limits.max_modules`,
/// `max_modules`), and more input or output bytes than it allows (`limits.max_input_bytes`, `max_output_bytes`;
/// discriminators `input` and `output`; target the station). The station itself counts as a module when it carries
/// the aspect. Its prevent half refuses the edit that creates a violation.
#[derive(Clone, Copy, Debug)]
pub struct SlotRules;

impl Rule for SlotRules {
    fn meta(&self) -> &'static RuleMeta {
        &SLOT_RULES
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Station(station) = *scope else {
            return Vec::new();
        };
        let modules = station_modules(q, station);
        let tag = full_tag(q, station, "the station");
        let mut out = Vec::new();
        let (mut input, mut output) = (0u64, 0u64);
        for (module, slot) in &modules {
            let Some((copy, plc)) = module_part(q, *module) else {
                continue;
            };
            for r in &plc.io {
                let bytes = u64::from(r.bits.div_ceil(8));
                match r.dir {
                    ew_derive::ew_schema::IoDir::In => input += bytes,
                    ew_derive::ew_schema::IoDir::Out => output += bytes,
                    ew_derive::ew_schema::IoDir::Unknown(_) => {}
                }
            }
            let number = match slot {
                Slot::Rack { slot, .. } => u32::from(*slot),
                Slot::Positional(n) => *n,
                Slot::Unresolved => continue,
            };
            if plc.allowed_slots.is_empty() || plc.allowed_slots.iter().any(|s| u32::from(*s) == number) {
                continue;
            }
            let name = full_tag(q, *module, "a module");
            let allowed = plc
                .allowed_slots
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            out.push(
                Finding::new(
                    &SLOT_RULES,
                    [Target::new(Kind::Device, *module)],
                    "slot",
                    name.clone(),
                    format!(
                        "{name} sits in slot {number} of {tag}, but its part {} allows slots {allowed}; move it to an \
                         allowed slot",
                        copy.item.part_number
                    ),
                )
                .arg("form", "slot")
                .arg("module", name)
                .arg("slot", number.to_string())
                .arg("station", tag.clone())
                .arg("part", copy.item.part_number.to_string())
                .arg("allowed", allowed)
                .reading(&part_confidence(&copy, "plc")),
            );
        }
        let Some((copy, plc)) = station_section(q, station) else {
            return out;
        };
        let Some(limits) = plc.limits.as_ref() else {
            return out;
        };
        let confidence = part_confidence(&copy, "plc");
        let count = modules.len() as u64;
        let checks = [
            ("max_modules", count, limits.max_modules.map(u64::from), "modules"),
            (
                "input",
                input,
                limits.max_input_bytes.map(u64::from),
                "input bytes",
            ),
            (
                "output",
                output,
                limits.max_output_bytes.map(u64::from),
                "output bytes",
            ),
        ];
        for (form, have, limit, what) in checks {
            let Some(limit) = limit.filter(|l| have > *l) else {
                continue;
            };
            out.push(
                Finding::new(
                    &SLOT_RULES,
                    [Target::new(Kind::Device, station)],
                    form,
                    tag.clone(),
                    format!("{tag} has {have} {what}, more than the {limit} its part allows; remove modules"),
                )
                .arg("form", form)
                .arg("station", tag.clone())
                .arg("have", have.to_string())
                .arg("limit", limit.to_string())
                .reading(&confidence),
            );
        }
        out
    }

    fn prevent(&self, before: &Q, after: &Q, touched: &Touched) -> Vec<Finding> {
        let mut stations = std::collections::BTreeSet::new();
        for (kind, id) in touched {
            if *kind == Kind::Device {
                let d: Id<Device> = id.cast();
                if let Some(s) = ew_derive::station_of(after, d) {
                    stations.insert(s);
                }
            }
        }
        let scopes: Vec<Scope> = stations.into_iter().map(Scope::Station).collect();
        new_findings(self, before, after, &scopes)
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-013

/// EW-PLC-013 "Card supply wrong" (doc 15 §4.5.1): conflict, o r, warning, station (FR-PLC-048); supersedes EW-POT-003
/// for PLC supply CPs.
pub const CARD_SUPPLY: RuleMeta = RuleMeta {
    id: "EW-PLC-013",
    code: "110013",
    pack: "EW-PLC",
    title: "Card supply wrong",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Station,
    supersedes: &["EW-POT-003"],
    release: Release::V1_0,
    docs: "reference/rules/ew-plc-013",
};

/// EW-PLC-013 over the devices of a station's tree (FR-PLC-048, D-16):
///
/// - **`fe`**: a CP with the potential role FE on a PE or PEN potential without an FE–PE bond, the CPX earth screw of
///   Golden A (EF-22). It targets the CP's function, so it supersedes EW-POT-003's finding for the same pin.
/// - **`unconnected`**: an L+ or M pin of a supply group of the device's part (`PlcSection.supply_groups`) that no
///   connection reaches.
/// - **`wrong`**: such a pin on a potential of another type than its role (an M pin on an L+ potential, an L+ pin on
///   PE).
#[derive(Clone, Copy, Debug)]
pub struct CardSupply;

impl Rule for CardSupply {
    fn meta(&self) -> &'static RuleMeta {
        &CARD_SUPPLY
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Station(station) = *scope else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for device in station_tree(q, station) {
            let mut functions: Vec<_> = q.idx().functions_of_device(device).collect();
            functions.sort();
            let groups = module_part(q, device)
                .map(|(copy, s)| (part_confidence(&copy, "plc"), s.supply_groups))
                .or_else(|| {
                    q.devices().get(device).and_then(|d| {
                        d.parts.iter().find_map(|u| {
                            let copy = q.parts().get(u.part.copy)?.clone();
                            let s = copy.item.sections.plc.clone()?;
                            Some((part_confidence(&copy, "plc"), s.supply_groups))
                        })
                    })
                });
            for f in functions {
                for cp in q.get::<FunctionCps>(f).iter() {
                    let at = CpRef::new(f, cp.idx);
                    let potential = potential_id_at(q, at.key());
                    let ptype = potential.and_then(|p| final_type(q, p));
                    if cp.potential_role == Some(PotentialType::Fe)
                        && matches!(ptype, Some(PotentialType::Pe | PotentialType::Pen))
                    {
                        let bonded = q
                            .idx()
                            .bonds_at(at.key())
                            .filter_map(|b| q.bonds().get(b))
                            .any(|b| b.kind == BondKind::FePe);
                        if !bonded {
                            let label = cp_label(q, at);
                            let pin = PinWords::of(q, at, &label);
                            let name = potential.map(|p| potential_name(q, p)).unwrap_or_default();
                            out.push(
                                pin.args(Finding::new(
                                    &CARD_SUPPLY,
                                    [Target::new(Kind::Function, f)],
                                    format!("fe:{}", cp.idx),
                                    label.clone(),
                                    format!(
                                        "{} is the FE pin of a PLC card and is fed from the PE potential {name}; \
                                         connect it to functional earth, or declare an FE–PE bond",
                                        pin.sentence
                                    ),
                                ))
                                .arg("form", "fe")
                                .arg("cp", label)
                                .arg("potential", name),
                            );
                        }
                    }
                    let Some((confidence, groups)) = &groups else {
                        continue;
                    };
                    for g in groups {
                        let role = if g.cps.l_plus.as_str() == cp.designation {
                            PotentialType::LPlus
                        } else if g.cps.m.as_str() == cp.designation {
                            PotentialType::M
                        } else {
                            continue;
                        };
                        let role_word = if role == PotentialType::LPlus { "L+" } else { "M" };
                        let group_word = match g.kind {
                            SupplyKind::Electronics => "electronics",
                            SupplyKind::Load => "load",
                            SupplyKind::Unknown(_) => "supply",
                        };
                        let label = cp_label(q, at);
                        let (form, message) = if q.idx().connections_at_cp(f, cp.idx).next().is_none() {
                            (
                                "unconnected",
                                format!(
                                    "{label}, the {role_word} pin of the {group_word} supply group, is not connected; \
                                     connect the card's supply"
                                ),
                            )
                        } else if let Some(t) = ptype.as_ref().filter(|t| {
                            !matches!(t, PotentialType::Undefined | PotentialType::Unknown(_)) && **t != role
                        }) {
                            let type_word = potential_type_word(t);
                            (
                                "wrong",
                                format!(
                                    "{label}, the {role_word} pin of the {group_word} supply group, is fed from a \
                                     potential of type {type_word}; feed it from {role_word}"
                                ),
                            )
                        } else {
                            continue;
                        };
                        out.push(
                            Finding::new(
                                &CARD_SUPPLY,
                                [Target::new(Kind::Function, f)],
                                format!("{form}:{}", cp.idx),
                                label.clone(),
                                message,
                            )
                            .arg("form", form)
                            .arg("cp", label)
                            .arg("role", role_word)
                            .arg("group", group_word)
                            .arg(
                                "type",
                                ptype
                                    .as_ref()
                                    .map_or_else(String::new, |t| potential_type_word(t).to_owned()),
                            )
                            .reading(confidence),
                        );
                    }
                }
            }
        }
        out
    }
}

/// A potential type as a message prints it.
fn potential_type_word(t: &PotentialType) -> &'static str {
    match t {
        PotentialType::L1 => "L1",
        PotentialType::L2 => "L2",
        PotentialType::L3 => "L3",
        PotentialType::L => "L",
        PotentialType::N => "N",
        PotentialType::Pe => "PE",
        PotentialType::Pen => "PEN",
        PotentialType::Fe => "FE",
        PotentialType::LPlus => "L+",
        PotentialType::LMinus => "L-",
        PotentialType::M => "M",
        PotentialType::Signal => "signal",
        PotentialType::Undefined | PotentialType::Unknown(_) => "undefined",
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-014

/// EW-PLC-014 "Output load exceeds rating" (doc 15 §4.5.1): safety, f r, warning, station (FR-PLC-048).
pub const OUTPUT_LOAD: RuleMeta = RuleMeta {
    id: "EW-PLC-014",
    code: "110014",
    pack: "EW-PLC",
    title: "Output load exceeds rating",
    kind: RuleKind::Safety,
    modes: Modes::letters("fr"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Station,
    supersedes: &[],
    release: Release::V1_0,
    docs: "reference/rules/ew-plc-014",
};

/// The load current of a device in milliamperes from its parts' technical data: `i_load` in A, else `power` in W at
/// `voltage` in V (24 V when the part states none). `None` without either.
fn load_ma(q: &Q, device: Id<Device>) -> Option<u64> {
    let dev = q.devices().get(device)?;
    let parts = q.parts();
    // a quantity in thousandths of its unit (Dec3)
    let quantity = |key: &str, unit: Unit| -> Option<i64> {
        dev.parts.iter().find_map(|u| {
            let copy = parts.get(u.part.copy)?;
            match &copy.item.tech_data.iter().find(|t| t.key.as_str() == key)?.value {
                TechVal::Quantity(v) if v.unit == unit => Some(v.value.0),
                _ => None,
            }
        })
    };
    if let Some(ma) = quantity("i_load", Unit::A) {
        return u64::try_from(ma).ok();
    }
    let mw = quantity("power", Unit::W)?;
    let mv = quantity("voltage", Unit::V).unwrap_or(24_000);
    (mv > 0)
        .then(|| u64::try_from(mw.saturating_mul(1000) / mv).ok())
        .flatten()
}

/// EW-PLC-014 over the output modules of a station (FR-PLC-048, D-25): the load current of each used output channel's
/// target device (its part's technical data, [`load_ma`]) against the module part's ratings (`PlcSection.ratings`): a
/// channel above `channel_ma` (discriminator `channel`, target the channel), the sum of the outputs a load supply group
/// feeds above `group_ma` (`group`) and the sum of the module above `module_ma` (`module`, target the module). A
/// module without ratings is not judged; the feeding channel's rating is EW-POT-018's.
#[derive(Clone, Copy, Debug)]
pub struct OutputLoad;

impl Rule for OutputLoad {
    fn meta(&self) -> &'static RuleMeta {
        &OUTPUT_LOAD
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Station(station) = *scope else {
            return Vec::new();
        };
        let channels = q.table::<ew_derive::ew_schema::Channel>();
        let mut out = Vec::new();
        for (module, _) in station_modules(q, station) {
            let Some((copy, plc)) = module_part(q, module) else {
                continue;
            };
            let Some(ratings) = plc.ratings.as_ref() else {
                continue;
            };
            let confidence = part_confidence(&copy, "plc");
            let targets = q.get::<ChannelTargets>(module);
            let states = q.get::<ChannelStates>(module);
            let mut total = 0u64;
            for (id, state) in states.iter() {
                let Some(c) = channels.get(*id) else {
                    continue;
                };
                if *state != EffectiveState::Used || !matches!(c.kind, ChannelKind::Do | ChannelKind::Ao) {
                    continue;
                }
                let Some(load) = targets
                    .iter()
                    .find(|t| t.channel == *id)
                    .and_then(|t| t.device)
                    .and_then(|d| load_ma(q, d))
                else {
                    continue;
                };
                total = total.saturating_add(load);
                if let Some(limit) = ratings.channel_ma.map(u64::from).filter(|l| load > *l) {
                    let channel = channel_name(q, module, c.id, c.number);
                    out.push(load_finding(
                        &channel,
                        Target::new(Kind::Channel, c.id),
                        "channel",
                        load,
                        limit,
                        &confidence,
                    ));
                }
            }
            let name = full_tag(q, module, "a module");
            let loads_groups = plc
                .supply_groups
                .iter()
                .filter(|g| {
                    g.kind == SupplyKind::Load && matches!(g.feeds(), SupplyFeeds::All | SupplyFeeds::Outputs)
                })
                .count();
            let mut sums: BTreeMap<&str, Option<u32>> = BTreeMap::new();
            if loads_groups > 0 {
                sums.insert("group", ratings.group_ma);
            }
            sums.insert("module", ratings.module_ma);
            for (form, limit) in sums {
                if let Some(limit) = limit.map(u64::from).filter(|l| total > *l) {
                    out.push(load_finding(
                        &name,
                        Target::new(Kind::Device, module),
                        form,
                        total,
                        limit,
                        &confidence,
                    ));
                }
            }
        }
        out
    }
}

fn load_finding(
    subject: &str,
    target: Target,
    form: &'static str,
    load: u64,
    limit: u64,
    confidence: &ew_derive::ew_schema::Confidence,
) -> Finding {
    let what = match form {
        "channel" => "the output",
        "group" => "the load supply group",
        _ => "the module",
    };
    Finding::new(
        &OUTPUT_LOAD,
        [target],
        form,
        subject.to_owned(),
        format!(
            "{subject}: {what} carries {load} mA, more than its rating of {limit} mA; spread the loads or use a \
             stronger output"
        ),
    )
    .arg("form", form)
    .arg("subject", subject.to_owned())
    .arg("load", load.to_string())
    .arg("limit", limit.to_string())
    .reading(confidence)
}
