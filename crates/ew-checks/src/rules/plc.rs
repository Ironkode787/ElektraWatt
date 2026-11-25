// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The MVP rules of the EW-PLC pack (doc 10 §3.10, doc 15 §4.5.1): EW-PLC-001, -002, -003,
//! -007, -008, -012 and -015, in the scopes `module` (a device with the `plc_module` aspect) and `cfg` (a configuration
//! project). The rules of the pack's 1.0 content are in `plc_wiring`, `safety` and `process`.
//!
//! Every rule reads the derived PLC values of `ew-derive` (doc 10 §2.10): the module starts and channel addresses
//! (A1, A2), the address index, the channel states (A3), texts and the solenoid-position slots. None stores or
//! recomputes an address of its own.
//!
//! # Texts
//!
//! A channel is named `<module tag>:<designation>` (its determined designation, else its number), an address by the
//! configuration project's profile (`Q0.1`), a device by its derived tag. IDs enter only targets and fingerprints.
//! Every finding carries its message arguments; the en-GB patterns of `texts/en-GB/rules.ftl` render the messages
//! byte for byte.
//!
//! # Prevent mode
//!
//! EW-PLC-001 and EW-PLC-007 carry the catalogue's prevent letter, so release mode reports them, and their prevent
//! evaluation refuses an edit that creates an overlap or a duplicate symbol: the configuration projects that
//! the touched channels, devices, parts and configuration projects belong to are checked before and after the batch,
//! and the batch is refused when a channel joins a group of channels it was not in ([`new_group_members`]); a repair
//! of a held overlap and an unrelated edit of a project that holds one commit.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::{
    AddressIndex, ChannelAddr, ChannelAddresses, ChannelStates, ChannelTexts, ChannelsByCp, EffectiveState,
    ModuleChannelIds, ModuleOrder, ModuleStartOf, Q, SolenoidPositions, Start, module_part, part_confidence,
    profile_of, station_of,
};
use ew_ids::Id;
use ew_plcaddr::{Area, Size};
use ew_schema::{AddrSpec, Channel, ChannelKind, Device, IoDir, Kind, PlcConfigProject};

use crate::finding::{Finding, QuickFix, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind, Touched};
use crate::rules::Named;
use crate::rules::term::full_tag;

// ---------------------------------------------------------------------------------------------------------
// Shared helpers

/// A channel as printed: `<module tag>:<designation>`, the number where the channel has no designation.
pub(crate) fn channel_name(q: &Q, module: Id<Device>, channel: Id<Channel>, number: u16) -> String {
    let designation = q
        .get::<ChannelTexts>(module)
        .iter()
        .find(|(id, _)| *id == channel)
        .and_then(|(_, t)| t.designation.clone())
        .unwrap_or_else(|| number.to_string());
    format!("{}:{designation}", full_tag(q, module, "module"))
}

/// The configuration project of a module, through its station.
pub(crate) fn module_cfg(q: &Q, module: Id<Device>) -> Option<Id<PlcConfigProject>> {
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
pub(crate) fn cfg_name(q: &Q, cfg: Id<PlcConfigProject>) -> Named {
    q.table::<PlcConfigProject>().get(cfg).map_or_else(
        || text!("word-a-configuration-project").into(),
        |c| c.name.to_string().into(),
    )
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
pub(crate) fn cfg_modules(q: &Q, cfg: Id<PlcConfigProject>) -> Vec<Id<Device>> {
    q.get::<ModuleOrder>(cfg).iter().map(|m| m.module).collect()
}

/// A channel kind as a message prints it (`DO`, `AI`, `multi-function`).
pub(crate) fn kind_word(k: &ChannelKind) -> &'static str {
    ew_derive::channel_kind_word(k)
}

/// The error findings of `rule` in `scopes` after a batch that were not there before it (by discriminator and
/// targets): the violations a prevent evaluation reports (ADR-0009 decision 3).
pub(crate) fn new_findings(rule: &dyn Rule, before: &Q, after: &Q, scopes: &[Scope]) -> Vec<Finding> {
    let key = |f: &Finding| {
        let mut t: Vec<Target> = f.targets.clone();
        t.sort();
        (f.discriminator.clone(), t)
    };
    let mut out = Vec::new();
    for scope in scopes {
        let had: std::collections::BTreeSet<_> = rule.run(before, scope).iter().map(key).collect();
        out.extend(
            rule.run(after, scope)
                .into_iter()
                .filter(|f| f.severity == Severity::Error && !had.contains(&key(f))),
        );
    }
    out
}

/// The devices a batch touches, in the snapshots before and after it (a deleted record is found in the first, a new
/// one in the second): its touched devices, the modules of its touched channels, and every device that uses a touched
/// part (`library.update`).
pub(crate) fn touched_devices(before: &Q, after: &Q, touched: &Touched) -> BTreeSet<Id<Device>> {
    let mut out = BTreeSet::new();
    for q in [before, after] {
        for (kind, id) in touched {
            match kind {
                Kind::Device => {
                    out.insert(id.cast());
                }
                Kind::Channel => out.extend(q.table::<Channel>().get(id.cast()).map(|c| c.module)),
                Kind::Part => out.extend(
                    q.devices()
                        .iter()
                        .filter(|(_, d)| d.parts.iter().any(|u| u.part.copy == id.cast()))
                        .map(|(d, _)| d),
                ),
                _ => {}
            }
        }
    }
    out
}

/// The configuration projects a batch touches: those of the [`touched_devices`] (as stations or modules) in either
/// snapshot, and its touched configuration projects.
fn touched_cfgs(before: &Q, after: &Q, touched: &Touched) -> Vec<Scope> {
    let mut cfgs = BTreeSet::new();
    for d in touched_devices(before, after, touched) {
        cfgs.extend(module_cfg(before, d));
        cfgs.extend(module_cfg(after, d));
    }
    cfgs.extend(
        touched
            .iter()
            .filter(|(k, _)| *k == Kind::PlcConfigProject)
            .map(|(_, id)| id.cast::<PlcConfigProject>()),
    );
    cfgs.into_iter().map(Scope::Cfg).collect()
}

/// The channels a batch touches: its touched channels and the channels of its touched devices, in either snapshot.
fn touched_channels(before: &Q, after: &Q, touched: &Touched) -> BTreeSet<Id<Channel>> {
    let mut out: BTreeSet<Id<Channel>> = touched
        .iter()
        .filter(|(k, _)| *k == Kind::Channel)
        .map(|(_, id)| id.cast())
        .collect();
    for q in [before, after] {
        for (kind, id) in touched {
            if *kind == Kind::Device {
                out.extend(q.get::<ModuleChannelIds>(id.cast()).iter().map(|(_, c)| *c));
            }
        }
    }
    out
}

/// Every grouped channel with the channels of its group (itself included).
type Groups = BTreeMap<Id<Channel>, BTreeSet<Id<Channel>>>;

/// Every channel that an error finding of `rule` in `scope` groups with others, with the channels of its group, and
/// those findings.
fn channel_groups(rule: &dyn Rule, q: &Q, scope: &Scope) -> (Groups, Vec<Finding>) {
    let found: Vec<Finding> = rule
        .run(q, scope)
        .into_iter()
        .filter(|f| f.severity == Severity::Error)
        .collect();
    let mut groups = Groups::new();
    for f in &found {
        let members: BTreeSet<Id<Channel>> = channel_targets(f).collect();
        for m in &members {
            groups.entry(*m).or_default().extend(members.iter().copied());
        }
    }
    (groups, found)
}

fn channel_targets(f: &Finding) -> impl Iterator<Item = Id<Channel>> + '_ {
    f.targets
        .iter()
        .filter(|t| t.kind == Kind::Channel)
        .map(|t| t.id.cast())
}

/// The prevent half of a rule whose findings group channels (EW-PLC-001's overlaps, EW-PLC-007's duplicate symbols),
/// judged per channel: an error finding after the batch is a violation when one of its channels was in no
/// group before, or when a touched channel's group gained a channel it was not grouped with. So a repair of a held
/// group (a channel moved out of it) and an edit that leaves the groups alone or only shrinks them commit, even though
/// the shrunk group is a finding of another key; a new overlap or duplicate is refused.
fn new_group_members(rule: &dyn Rule, before: &Q, after: &Q, touched: &Touched) -> Vec<Finding> {
    let edited = touched_channels(before, after, touched);
    let mut out = Vec::new();
    for scope in touched_cfgs(before, after, touched) {
        let (had, _) = channel_groups(rule, before, &scope);
        let (has, found) = channel_groups(rule, after, &scope);
        let violates = |c: Id<Channel>| match had.get(&c) {
            None => true,
            Some(old) => edited.contains(&c) && has.get(&c).is_some_and(|new| !new.is_subset(old)),
        };
        out.extend(found.into_iter().filter(|f| channel_targets(f).any(violates)));
    }
    out
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-001

/// EW-PLC-001 "Address overlap" (doc 15 §4.5.1): conflict, o p, error, cfg (FR-PLC-044).
pub const ADDRESS_OVERLAP: RuleMeta = RuleMeta {
    id: "EW-PLC-001",
    message: key!("rule-ew-plc-001"),
    code: "110001",
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
/// `I1.3`; `address_index(cfg)`), one finding per overlap group (the connected run of overlapping channels,
/// so N channels on one address are one finding, not N(N−1)/2), every member channel as a target, subject the first
/// in address order, discriminator `overlap`; the text names the first [`SHOWN_OVERLAP_CHANNELS`] channels with
/// their addresses and counts the rest ("and 7 more channels"); and a word, double-word or long-word address at a
/// byte that is not a multiple of the profile's word alignment, a warning (an unverified convention), discriminator
/// `alignment`. Automatic starts never overlap (A1), so a finding involves a manual value or a part map that gives
/// two channels one bit.
///
/// A group is chained, not a clique: with `IW0`, `I0.0` and `I1.7` only `IW0` overlaps the others, so the
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
            out.push(
                Finding::new(
                    &ADDRESS_OVERLAP,
                    members.iter().map(|u| Target::new(Kind::Channel, u.channel)),
                    "overlap",
                    subject,
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

    fn prevent(&self, before: &Q, after: &Q, touched: &Touched) -> Vec<Finding> {
        new_group_members(self, before, after, touched)
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-002

/// EW-PLC-002 "Address outside the module" (doc 15 §4.5.1): conflict, o r, error, module (FR-PLC-044).
pub const OUTSIDE_MODULE: RuleMeta = RuleMeta {
    id: "EW-PLC-002",
    message: key!("rule-ew-plc-002"),
    code: "110002",
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
/// the module's ranges in that direction ([`ModuleRange`]). Only a module whose start in that direction is an address
/// is checked (an excluded, open or unresolved start bounds nothing). Target the channel, discriminator `range`. Such
/// a manual address is not also an EW-PLC-012 `manual` deviation.
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
        let Some(range) = ModuleRange::of(q, module) else {
            return Vec::new();
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
            let Some((area, from, to)) = range.outside(&dir, address) else {
                continue;
            };
            let channel = channel_name(q, module, e.channel, e.number);
            let tag = full_tag(q, module, "module");
            let form = if to > from { "bytes" } else { "none" };
            out.push(
                Finding::new(
                    &OUTSIDE_MODULE,
                    [Target::new(Kind::Channel, e.channel)],
                    "range",
                    channel.clone(),
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

/// The I/O range of a module (EW-PLC-002): its derived starts (A1) and the units of its ranges per direction
/// (`module_io`: an IO-Link master's port windows, else the part's ranges), in the profile's unit width.
struct ModuleRange {
    start: ew_derive::ModuleStart,
    io: Vec<ew_schema::IoRange>,
    unit_bits: u32,
}

impl ModuleRange {
    /// `None` for a module outside every configuration project or without a part's PLC section.
    fn of(q: &Q, module: Id<Device>) -> Option<ModuleRange> {
        let start = (*q.get::<ModuleStartOf>(module))?;
        module_part(q, module)?;
        let unit_bits = module_cfg(q, module)
            .and_then(|c| profile_of(q, c))
            .map_or(8, |p| p.unit_bits.max(1));
        Some(ModuleRange {
            start,
            io: ew_derive::module_io(q, module),
            unit_bits,
        })
    }

    /// The area and the bytes `[from, to)` of the range in direction `dir` when `address` lies outside them: another
    /// area, or a byte outside. `None` when it lies inside, or when the module's start in that direction is no
    /// address (an excluded, open or unresolved start bounds nothing).
    fn outside(&self, dir: &IoDir, address: &ew_plcaddr::PlcAddress) -> Option<(Area, u32, u32)> {
        let (area, s) = match dir {
            IoDir::In => (Area::I, self.start.input),
            _ => (Area::Q, self.start.output),
        };
        let Some(Start::At(from)) = s else {
            return None;
        };
        let units = self
            .io
            .iter()
            .filter(|r| r.dir == *dir)
            .map(|r| r.bits.div_ceil(self.unit_bits))
            .fold(0u32, u32::saturating_add);
        let to = from.saturating_add(units);
        let last = address
            .byte
            .saturating_add(address.size.bits().div_ceil(self.unit_bits).saturating_sub(1));
        let inside = address.area == area && address.byte >= from && last < to;
        (!inside).then_some((area, from, to))
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-003

/// EW-PLC-003 "Bit index out of range" (doc 15 §4.5.1): conflict, o r, error, module (FR-PLC-044).
pub const BIT_OUT_OF_RANGE: RuleMeta = RuleMeta {
    id: "EW-PLC-003",
    message: key!("rule-ew-plc-003"),
    code: "110003",
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
    message: key!("rule-ew-plc-007"),
    code: "110007",
    title: "Duplicate symbol",
    kind: RuleKind::Conflict,
    modes: Modes::letters("opr"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Cfg,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-PLC-007: a determined symbol (manual, else the template's) that two or more channels of one configuration project
/// carry. Under the configuration project's strict IEC policy (the default) symbols compare cleaned into identifiers
/// (doc 10 §3.4: "TA05-V-15" and "TA05_V_15" collide), else exactly. One finding per cleaned symbol, every channel
/// that carries it as a target, discriminator `symbol`, the symbol of the first channel in natural order printed; the
/// channels listed in natural order. Its prevent half refuses the edit that creates a duplicate.
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
        let strict = q
            .table::<PlcConfigProject>()
            .get(cfg)
            .is_none_or(|c| c.symbols.as_ref().is_none_or(|s| s.strict_iec));
        let mut by_symbol: BTreeMap<String, Vec<(String, Id<Channel>, String)>> = BTreeMap::new();
        for module in cfg_modules(q, cfg) {
            let channels = q.table::<Channel>();
            for (id, text) in q.get::<ChannelTexts>(module).iter() {
                let Some(symbol) = &text.symbol else {
                    continue;
                };
                let number = channels.get(*id).map_or(0, |c| c.number);
                let key = if strict {
                    ew_derive::iec_ident(symbol)
                } else {
                    symbol.clone()
                };
                by_symbol.entry(key).or_default().push((
                    channel_name(q, module, *id, number),
                    *id,
                    symbol.clone(),
                ));
            }
        }
        let name = cfg_name(q, cfg);
        let mut out = Vec::new();
        for (_, mut channels) in by_symbol {
            if channels.len() < 2 {
                continue;
            }
            channels.sort_by(|a, b| {
                ew_mltext::natural_cmp(&a.0, &b.0)
                    .then_with(|| a.0.cmp(&b.0))
                    .then_with(|| a.1.cmp(&b.1))
            });
            let symbol = channels[0].2.clone();
            let list = channels
                .iter()
                .map(|(n, _, _)| n.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            out.push(
                Finding::new(
                    &DUPLICATE_SYMBOL,
                    channels.iter().map(|(_, id, _)| Target::new(Kind::Channel, *id)),
                    "symbol",
                    symbol.clone(),
                )
                .arg("symbol", symbol)
                .arg("count", channels.len().to_string())
                .arg("cfg", name.clone())
                .arg("channels", list),
            );
        }
        out
    }

    fn prevent(&self, before: &Q, after: &Q, touched: &Touched) -> Vec<Finding> {
        new_group_members(self, before, after, touched)
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-008

/// EW-PLC-008 "Used channel without address" (doc 15 §4.5.1): completeness, o x r, warning, module (FR-PLC-044).
pub const USED_WITHOUT_ADDRESS: RuleMeta = RuleMeta {
    id: "EW-PLC-008",
    message: key!("rule-ew-plc-008"),
    code: "110008",
    title: "Used channel without address",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
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
            let reason = if module_cfg(q, module).is_none() {
                "station"
            } else if e.address == ChannelAddr::Excluded {
                "excluded"
            } else {
                "other"
            };
            out.push(
                Finding::new(
                    &USED_WITHOUT_ADDRESS,
                    [Target::new(Kind::Channel, e.channel)],
                    "address",
                    channel.clone(),
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
    message: key!("rule-ew-plc-012"),
    code: "110012",
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
///   coil CP, which unbinds the other, previewed). The second
///   fix of the catalogue, "open the part's PLC section", is a window action, not a command (a finding carries one
///   command fix).
/// - **`manual`**: a channel whose manual address differs from the address A2 derives for it (`auto`); target the
///   channel. A manual address outside the module's I/O range (a frozen channel after its module's start moved) is
///   EW-PLC-002's alone: that it lies on no byte of the card is the defect, and the deviation from the map follows
///   from it.
///
/// Both read the module's part, so a part whose `plc` section is not of confidence `verified` marks the findings as
/// resting on unverified data (the PLC half, `part_confidence(copy, "plc")`).
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
        let address_of = |id: Id<Channel>| {
            ew_derive::channel_address_words(addresses.iter().find(|e| e.channel == id).map(|e| &e.address))
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
                    let address = ew_derive::channel_address_words(
                        q.get::<ChannelAddresses>(c.module)
                            .iter()
                            .find(|e| e.channel == at)
                            .map(|e| &e.address),
                    );
                    let valve = s.valve.map_or_else(
                        || text!("word-the-valve").into(),
                        |v| full_tag(q, v, text!("word-valve")),
                    );
                    let expected_name = match expected_id {
                        Some(id) => channel_name(q, module, id, expected),
                        None => format!("{}:{expected}", full_tag(q, module, "module")),
                    };
                    let expected_address =
                        expected_id.map_or_else(|| ew_derive::channel_address_words(None), address_of);
                    let mut f = Finding::new(
                        &DIFFERS_FROM_MAP,
                        [
                            Target::new(Kind::Channel, at),
                            Target::new(Kind::Function, coil.function),
                        ],
                        "binding",
                        channel.clone(),
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
                            label: text!("fix-rebind-map-channel"),
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
        let range = ModuleRange::of(q, module);
        for e in addresses.iter() {
            let manual = channels
                .get(e.channel)
                .filter(|c| matches!(c.address, AddrSpec::Manual(_)));
            let (Some(c), ChannelAddr::At { address, text }, Some(auto)) = (manual, &e.address, e.auto)
            else {
                continue;
            };
            if *address == auto {
                continue;
            }
            // An address outside the module's range is EW-PLC-002's: one defect, one finding.
            let outside = dir_of(&c.kind)
                .zip(range.as_ref())
                .is_some_and(|(dir, r)| r.outside(&dir, address).is_some());
            if outside {
                continue;
            }
            let derived = module_cfg(q, module)
                .and_then(|c| profile_of(q, c))
                .map_or_else(String::new, |p| p.render(&auto, ew_plcaddr::Purpose::Drawing));
            let channel = channel_name(q, module, e.channel, e.number);
            out.push(
                Finding::new(
                    &DIFFERS_FROM_MAP,
                    [Target::new(Kind::Channel, e.channel)],
                    "manual",
                    channel.clone(),
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
    message: key!("rule-ew-plc-015"),
    code: "110015",
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
                    )
                    .arg("subject", channel)
                    .arg("what", "address"),
                );
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-018

/// EW-PLC-018 "Module drawn with its channels unbound": conflict, o r, warning, module.
pub const PINS_UNBOUND: RuleMeta = RuleMeta {
    id: "EW-PLC-018",
    message: key!("rule-ew-plc-018"),
    code: "110018",
    title: "Module drawn with its channels unbound",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Module,
    supersedes: &[],
    release: Release::V1_0,
};

/// EW-PLC-018: a module that is drawn, whose part maps its channels to pins, while none of its channels is bound to a
/// pin: what the rack dialog's Draw on this page left in a project of an earlier build (its addresses stand at no pin,
/// and a device wired to a pin is no channel's target). One channel unbound on purpose is no finding. Target the
/// device, discriminator `module`; the quick fix `plc.bind_pins` binds the channels.
#[derive(Clone, Copy, Debug)]
pub struct PinsUnbound;

impl Rule for PinsUnbound {
    fn meta(&self) -> &'static RuleMeta {
        &PINS_UNBOUND
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Module(device) = *scope else {
            return Vec::new();
        };
        if module_part(q, device).is_none_or(|(_, s)| s.pin_of.is_none())
            || q.get::<ew_derive::MainRep>(device).is_none()
        {
            return Vec::new();
        }
        let channels = q.table::<Channel>();
        let ids = q.get::<ModuleChannelIds>(device);
        if ids.is_empty()
            || ids
                .iter()
                .any(|(_, c)| channels.get(*c).is_none_or(|c| c.binding.is_some()))
        {
            return Vec::new();
        }
        let tag = full_tag(q, device, text!("word-device"));
        vec![
            Finding::new(
                &PINS_UNBOUND,
                [Target::new(Kind::Device, device)],
                "module",
                tag.clone(),
            )
            .arg("device", tag)
            .with_fix(QuickFix {
                label: text!("fix-plc-bind-pins"),
                command: "plc.bind_pins".to_owned(),
                params: serde_json::json!({ "module": device.to_string() }),
            }),
        ]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-PLC-017

/// EW-PLC-017 "Not a PLC module": conflict, o r, warning, module.
pub const NOT_A_MODULE: RuleMeta = RuleMeta {
    id: "EW-PLC-017",
    message: key!("rule-ew-plc-017"),
    code: "110017",
    title: "Not a PLC module",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Module,
    supersedes: &[],
    release: Release::V1_0,
};

/// EW-PLC-017: a device that carries `plc_module` while nothing of it describes a module
/// ([`ew_derive::not_a_module`]: a bus node whose part has no PLC data, attached to a CPU, or a copy of a station that
/// kept the station's own module). Its channels are no part's, and an export would list a module that does not exist.
/// Target the device, discriminator `module`; the quick fix `plc.remove_from_rack` takes it out of the rack and removes
/// its channels.
#[derive(Clone, Copy, Debug)]
pub struct NotAModule;

impl Rule for NotAModule {
    fn meta(&self) -> &'static RuleMeta {
        &NOT_A_MODULE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Module(device) = *scope else {
            return Vec::new();
        };
        if !ew_derive::not_a_module(q, device) {
            return Vec::new();
        }
        let tag = full_tag(q, device, text!("word-device"));
        let part = q
            .devices()
            .get(device)
            .and_then(|d| d.parts.first().and_then(|u| q.parts().get(u.part.copy)))
            .map(|c| c.item.part_number.to_string())
            .unwrap_or_default();
        vec![
            Finding::new(
                &NOT_A_MODULE,
                [Target::new(Kind::Device, device)],
                "module",
                tag.clone(),
            )
            .arg("device", tag)
            .arg("part", part)
            .with_fix(QuickFix {
                label: text!("fix-plc-take-out"),
                command: "plc.remove_from_rack".to_owned(),
                params: serde_json::json!({ "device": device.to_string() }),
            }),
        ]
    }
}
