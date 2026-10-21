// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC queries of doc 10 §2.10 and §3.1–§3.4 (FT-111): module order and starts per configuration
//! project, then addresses, states, targets and texts per module, and the PLC rows of a station.
//!
//! # Address derivation (doc 10 §3.1)
//!
//! [`ModuleOrder`] lists the modules of a configuration project: stations by (`order`, ID); a station's
//! modules are the station device itself and its descendants that carry a `plc_module` aspect, rack slots
//! first by (rack, slot, subslot), then positional modules in manifold order (a depth-first walk with the
//! children in (`Device.position`, ID) order), whose slot is their 1-based rank. Tags, folios and page
//! order are not inputs, so renumbering never re-addresses (FR-PLC-055).
//!
//! [`ModuleStarts`] is algorithm A1: every manual range is occupied first; then one cursor per direction
//! and address class walks the modules in order, an automatic range takes the first free range of its area
//! at or after the cursor, and the cursor advances by the module's own width whatever its start mode. The
//! widths come from the part's `plc` section (the part use named by `PlcModule.part_use`, else the first
//! part use whose part has one). [`ChannelAddresses`] is A2: `start + bit_of(number)`, rendered by the
//! configuration project's profile (`ew-plcaddr`).
//!
//! # States, targets, texts (doc 10 §3.2–§3.5)
//!
//! [`ChannelStates`] applies A3 steps 1, 2, 3 and 5: an explicit state, a `Tbd` address, the state of the
//! bound target device, else the part's `unbound` state (spare by default). Step 4 (a channel claimed by the
//! solenoid-position map gives `Reserved`) needs the position map, which the part data does not carry yet;
//! on the golden project it changes nothing (doc 10 §3.2). [`ChannelTargets`] takes the manual target, else
//! the function of the bound CP (direct targets, MVP; traced targets are 1.0). [`ChannelTexts`] determines
//! the designation (manual, else the part pattern over `{n}`), the symbol (manual only: templates are 1.0),
//! the function text (the target device's function text, else the process tag of the device it drives
//! through `DeviceLink{Drives}`) and the process tag (the target device's own, else the driven device's).
//!
//! # Granularity
//!
//! The finest key is a module, station or configuration project (ARCH §6.3). A module reads its channels by
//! ID (`v5(module, "channel", n)` for `n` below the part's channel count), so a channel edit invalidates
//! its module only. Stations and device links are found by whole-table reads, cut off early by value.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_ids::{Id, v5};
use ew_plcaddr::{AddressProfile, Area, AreaIntervals, MapExpr, MapVar, Purpose, Size};
use ew_schema::{
    AddrSpec, Channel, ChannelKind, ChannelState, Device, DeviceLink, DeviceLinkKind, DeviceState, Endpoint,
    Function, IoClass, IoDir, LibCopy, MapSpec, Part, PlcArea, PlcConfigProject, PlcSection, PlcSize,
    RepType, Representation, SlotPath,
};

use crate::Q;
use crate::query::{Policy, Query};

// ---------------------------------------------------------------------------------------------------------
// Shared helpers

/// The part use of a module device that supplies its PLC section, with the section.
pub fn module_part(q: &Q, module: Id<Device>) -> Option<(Arc<LibCopy<Part>>, PlcSection)> {
    let dev = q.devices().get(module)?;
    let parts = q.parts();
    let named = dev.aspects.plc_module.as_ref().and_then(|m| m.part_use);
    let with_plc = |i: usize| {
        let copy = parts.get(dev.parts.get(i)?.part.copy)?;
        let plc = copy.item.sections.plc.clone()?;
        Some((Arc::clone(copy), plc))
    };
    match named {
        Some(i) => with_plc(usize::from(i)),
        None => (0..dev.parts.len()).find_map(with_plc),
    }
}

/// The station of a device: the device itself when it carries `plc_station`, else its nearest ancestor that
/// does (bounded by the number of devices, so a hostile parent cycle terminates).
pub fn station_of(q: &Q, device: Id<Device>) -> Option<Id<Device>> {
    let devices = q.devices();
    let mut at = device;
    for _ in 0..=devices.len() {
        let dev = devices.get(at)?;
        if dev.aspects.plc_station.is_some() {
            return Some(at);
        }
        at = dev.parent?;
    }
    None
}

/// The address profile of a configuration project, when this build ships it.
pub fn profile_of(q: &Q, cfg: Id<PlcConfigProject>) -> Option<&'static AddressProfile> {
    let c = q.table::<PlcConfigProject>().get(cfg)?;
    ew_plcaddr::builtin(c.profile.as_str())
}

fn area_of(a: &PlcArea) -> Option<Area> {
    Some(match a {
        PlcArea::I => Area::I,
        PlcArea::Q => Area::Q,
        PlcArea::M => Area::M,
        PlcArea::Pi => Area::Pi,
        PlcArea::Pq => Area::Pq,
        PlcArea::Unknown(_) => return None,
    })
}

fn size_of(s: &PlcSize) -> Option<Size> {
    Some(match s {
        PlcSize::Bit => Size::Bit,
        PlcSize::Byte => Size::Byte,
        PlcSize::Word => Size::Word,
        PlcSize::DWord => Size::DWord,
        PlcSize::LWord => Size::LWord,
        PlcSize::Unknown(_) => return None,
    })
}

/// The canonical form of a stored address.
fn canonical(a: &ew_schema::PlcAddress) -> Option<ew_plcaddr::PlcAddress> {
    Some(ew_plcaddr::PlcAddress {
        area: area_of(&a.area)?,
        size: size_of(&a.size)?,
        byte: a.byte,
        bit: a.bit,
    })
}

/// The direction of a channel kind: outputs for `do` and `ao`, inputs for `di`, `ai` and counters; none for
/// the others (IO-Link ports, supply pins).
fn dir_of(kind: &ChannelKind) -> Option<IoDir> {
    match kind {
        ChannelKind::Do | ChannelKind::Ao => Some(IoDir::Out),
        ChannelKind::Di | ChannelKind::Ai | ChannelKind::Counter => Some(IoDir::In),
        _ => None,
    }
}

fn is_analog(kind: &ChannelKind) -> bool {
    matches!(kind, ChannelKind::Ai | ChannelKind::Ao)
}

/// The area of a direction.
fn dir_area(dir: &IoDir) -> Option<Area> {
    match dir {
        IoDir::In => Some(Area::I),
        IoDir::Out => Some(Area::Q),
        IoDir::Unknown(_) => None,
    }
}

/// The value of a part-data map at `x`; `None` for an unparsable expression, a `p` map or an index outside
/// a list.
fn map_at(map: Option<&MapSpec>, x: u32) -> Option<u32> {
    match map {
        None => Some(x),
        Some(MapSpec::Expr(s)) => {
            let e = MapExpr::parse(s)?;
            if e.var != MapVar::N {
                return None;
            }
            e.eval(x)
        }
        Some(MapSpec::List(l)) => l.get(usize::try_from(x).ok()?).copied(),
        Some(MapSpec::Unknown(_)) => None,
    }
}

// ---------------------------------------------------------------------------------------------------------
// module_order(cfg)

/// The slot of a module (doc 10 §2.3).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Slot {
    /// An authored rack position.
    Rack {
        /// The rack.
        rack: u16,
        /// The slot.
        slot: u16,
        /// The subslot.
        subslot: Option<u16>,
    },
    /// The 1-based rank among the station's positional modules (manifold order).
    Positional(u32),
    /// A slot form of a newer format: listed after the others, not ranked, never addressed.
    Unresolved,
}

/// One module of a configuration project in allocation order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OrderedModule {
    /// The station.
    pub station: Id<Device>,
    /// The module device.
    pub module: Id<Device>,
    /// Its slot.
    pub slot: Slot,
}

/// `module_order(cfg)`: the modules of a configuration project in allocation order (see the module
/// documentation).
#[derive(Debug)]
pub struct ModuleOrder;

impl Query for ModuleOrder {
    type Key = Id<PlcConfigProject>;
    type Value = Vec<OrderedModule>;
    const FAMILY: &'static str = "module_order";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<OrderedModule>> {
        let devices = q.devices();
        let mut stations: Vec<(bool, u32, Id<Device>)> = devices
            .iter()
            .filter_map(|(id, d)| {
                let s = d.aspects.plc_station.as_ref()?;
                (s.config_project == *key).then(|| (s.order.is_none(), s.order.unwrap_or(0), id))
            })
            .collect();
        stations.sort();
        let mut out = Vec::new();
        for (_, _, station) in stations {
            out.extend(station_modules(q, station));
        }
        Arc::new(out)
    }
}

/// The modules of one station: rack slots first, then positional modules in manifold order.
fn station_modules(q: &Q, station: Id<Device>) -> Vec<OrderedModule> {
    let devices = q.devices();
    let mut walk: Vec<Id<Device>> = Vec::new();
    // Depth-first, children in (position, ID) order; a device that is a station of its own ends the walk
    // below it (a nested station owns its modules). `seen` bounds a hostile parent cycle.
    let mut stack = vec![station];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(d) = stack.pop() {
        if !seen.insert(d) {
            continue;
        }
        walk.push(d);
        let mut children: Vec<(Option<u32>, Id<Device>)> = q
            .idx()
            .device_children(d)
            .filter(|c| {
                devices
                    .get(*c)
                    .is_some_and(|dev| dev.aspects.plc_station.is_none())
            })
            .map(|c| (devices.get(c).and_then(|dev| dev.position), c))
            .collect();
        children.sort();
        stack.extend(children.into_iter().rev().map(|(_, c)| c));
    }
    let mut racks = Vec::new();
    let mut positional = Vec::new();
    let mut unresolved = Vec::new();
    for d in walk {
        let Some(m) = devices.get(d).and_then(|dev| dev.aspects.plc_module.as_ref()) else {
            continue;
        };
        match &m.slot {
            SlotPath::Rack(r) => racks.push(OrderedModule {
                station,
                module: d,
                slot: Slot::Rack {
                    rack: r.rack,
                    slot: r.slot,
                    subslot: r.subslot,
                },
            }),
            SlotPath::Positional => positional.push(d),
            // A newer slot form does not take a positional rank, so its siblings keep theirs.
            SlotPath::Unknown(_) => unresolved.push(d),
        }
    }
    racks.sort_by_key(|m| (m.slot, m.module));
    racks.extend(
        positional
            .into_iter()
            .enumerate()
            .map(|(i, module)| OrderedModule {
                station,
                module,
                slot: Slot::Positional(u32::try_from(i + 1).unwrap_or(u32::MAX)),
            }),
    );
    racks.extend(unresolved.into_iter().map(|module| OrderedModule {
        station,
        module,
        slot: Slot::Unresolved,
    }));
    racks
}

// ---------------------------------------------------------------------------------------------------------
// module_starts(cfg)

/// The start of one direction of a module.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Start {
    /// The start byte in the direction's area.
    At(u32),
    /// Not addressed.
    Excluded,
    /// An open item ("TBD").
    Tbd,
    /// Not derivable: an address form of a newer format, or no address profile.
    Unresolved,
}

/// The starts of one module, by direction (`None`: the part has no range in that direction).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ModuleStart {
    /// The input start.
    pub input: Option<Start>,
    /// The output start.
    pub output: Option<Start>,
}

/// `module_starts(cfg)`: algorithm A1 of doc 10 §3.1, the start of every module and direction.
#[derive(Debug)]
pub struct ModuleStarts;

impl Query for ModuleStarts {
    type Key = Id<PlcConfigProject>;
    type Value = BTreeMap<Id<Device>, ModuleStart>;
    const FAMILY: &'static str = "module_starts";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<BTreeMap<Id<Device>, ModuleStart>> {
        let order = q.get::<ModuleOrder>(*key);
        let profile = profile_of(q, *key);
        let devices = q.devices();
        let modules: Vec<(Id<Device>, ew_schema::IoStarts, Vec<ew_schema::IoRange>)> = order
            .iter()
            .filter(|m| m.slot != Slot::Unresolved)
            .map(|m| {
                let starts = devices
                    .get(m.module)
                    .and_then(|d| d.aspects.plc_module.as_ref())
                    .map(|p| p.start.clone())
                    .unwrap_or_default();
                let io = module_part(q, m.module).map(|(_, s)| s.io).unwrap_or_default();
                (m.module, starts, io)
            })
            .collect();
        let mut out = BTreeMap::new();
        // A module whose slot is of a newer format is not allocated: its ranges are unresolved.
        for m in order.iter().filter(|m| m.slot == Slot::Unresolved) {
            let mut s = ModuleStart::default();
            for r in module_part(q, m.module).map(|(_, s)| s.io).unwrap_or_default() {
                set(&mut s, &r.dir, Start::Unresolved);
            }
            out.insert(m.module, s);
        }
        let Some(prof) = profile else {
            for (module, _, io) in &modules {
                let mut s = ModuleStart::default();
                for r in io {
                    set(&mut s, &r.dir, Start::Unresolved);
                }
                out.insert(*module, s);
            }
            return Arc::new(out);
        };
        let spec = |starts: &ew_schema::IoStarts, dir: &IoDir| match dir {
            IoDir::In => starts.input.clone(),
            _ => starts.output.clone(),
        };
        // Pass 1: every manual range is occupied before any automatic range is placed.
        let mut used = AreaIntervals::new();
        for (_, starts, io) in &modules {
            for r in io {
                if let AddrSpec::Manual(a) = spec(starts, &r.dir) {
                    // A manual start in another area than the range's direction is not placed (unresolved).
                    if let (Some(area), Some(dir)) = (area_of(&a.area), dir_area(&r.dir)) {
                        if area == dir {
                            used.insert(area, a.byte, prof.units(r.bits));
                        }
                    }
                }
            }
        }
        // Pass 2: one cursor per direction and class; it advances by each module's own width.
        let mut cursor: BTreeMap<(u8, u8), u32> = BTreeMap::new();
        for (module, starts, io) in &modules {
            let mut s = ModuleStart::default();
            for r in io {
                let Some(area) = dir_area(&r.dir) else {
                    continue;
                };
                let class = match r.class {
                    IoClass::Analog => 1,
                    _ => 0,
                };
                let ckey = (u8::from(matches!(r.dir, IoDir::Out)), class);
                let default = if class == 1 {
                    prof.default_start_analog
                } else {
                    prof.default_start_digital
                };
                let units = prof.units(r.bits);
                let mut at = *cursor.get(&ckey).unwrap_or(&default);
                if r.bits >= 16 && prof.word_align > 1 {
                    at = at.div_ceil(prof.word_align).saturating_mul(prof.word_align);
                }
                let (start, next) = match spec(starts, &r.dir) {
                    AddrSpec::Manual(a) if area_of(&a.area) == Some(area) => {
                        (Start::At(a.byte), at.saturating_add(units))
                    }
                    AddrSpec::Manual(_) => (Start::Unresolved, at.saturating_add(units)),
                    AddrSpec::Excluded => (Start::Excluded, at.saturating_add(units)),
                    AddrSpec::Tbd => (Start::Tbd, at.saturating_add(units)),
                    AddrSpec::Auto => {
                        let free = used.first_free(area, at, units);
                        used.insert(area, free, units);
                        (Start::At(free), free.saturating_add(units))
                    }
                    AddrSpec::Unknown(_) => (Start::Unresolved, at.saturating_add(units)),
                };
                set(&mut s, &r.dir, start);
                cursor.insert(ckey, next);
            }
            out.insert(*module, s);
        }
        Arc::new(out)
    }
}

fn set(s: &mut ModuleStart, dir: &IoDir, start: Start) {
    match dir {
        IoDir::In => s.input = Some(start),
        IoDir::Out => s.output = Some(start),
        IoDir::Unknown(_) => {}
    }
}

/// The configuration project of a module (through its station).
fn module_cfg(q: &Q, module: Id<Device>) -> Option<(Id<Device>, Id<PlcConfigProject>)> {
    let station = station_of(q, module)?;
    let cfg = q
        .devices()
        .get(station)?
        .aspects
        .plc_station
        .as_ref()?
        .config_project;
    Some((station, cfg))
}

/// The channel records of a module, read by ID for the numbers of the part's channel spec.
fn module_channels(q: &Q, module: Id<Device>) -> Vec<Arc<Channel>> {
    let Some((_, plc)) = module_part(q, module) else {
        return Vec::new();
    };
    let count = plc.channels.as_ref().map_or(0, |c| c.count);
    let table = q.table::<Channel>();
    (0..count)
        .filter_map(|n| table.get(v5::plc_channel_id(module, n)).cloned())
        .filter(|c| c.module == module)
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// channel_addresses(module)

/// The derived address of a channel.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ChannelAddr {
    /// An address with its rendering for drawings and reports ("Q0.1").
    At {
        /// The canonical address.
        address: ew_plcaddr::PlcAddress,
        /// The profile's drawing form.
        text: String,
    },
    /// Not addressed (`Excluded`, or a channel kind without a direction).
    Excluded,
    /// An open item ("TBD").
    Tbd,
    /// Not derivable (no profile, no start, a map that does not parse, an address of a newer format).
    Unresolved,
}

/// One channel's address.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ChannelAddrEntry {
    /// The channel.
    pub channel: Id<Channel>,
    /// Its number.
    pub number: u16,
    /// Its address.
    pub address: ChannelAddr,
}

/// `channel_addresses(module)`: algorithm A2 of doc 10 §3.1, in channel-number order.
#[derive(Debug)]
pub struct ChannelAddresses;

impl Query for ChannelAddresses {
    type Key = Id<Device>;
    type Value = Vec<ChannelAddrEntry>;
    const FAMILY: &'static str = "channel_addresses";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<ChannelAddrEntry>> {
        let channels = module_channels(q, *key);
        let cfg = module_cfg(q, *key).map(|(_, c)| c);
        let profile = cfg.and_then(|c| profile_of(q, c));
        let start = cfg.and_then(|c| q.get::<ModuleStarts>(c).get(key).copied());
        let bit_of = module_part(q, *key).and_then(|(_, s)| s.bit_of);
        let out = channels
            .iter()
            .map(|c| ChannelAddrEntry {
                channel: c.id,
                number: c.number,
                address: address_of(c, profile, start, bit_of.as_ref()),
            })
            .collect();
        Arc::new(out)
    }
}

fn address_of(
    c: &Channel,
    profile: Option<&'static AddressProfile>,
    start: Option<ModuleStart>,
    bit_of: Option<&MapSpec>,
) -> ChannelAddr {
    let render = |a: ew_plcaddr::PlcAddress| match profile {
        Some(p) => ChannelAddr::At {
            address: a,
            text: p.render(&a, Purpose::Drawing),
        },
        None => ChannelAddr::Unresolved,
    };
    match &c.address {
        AddrSpec::Excluded => ChannelAddr::Excluded,
        AddrSpec::Tbd => ChannelAddr::Tbd,
        AddrSpec::Unknown(_) => ChannelAddr::Unresolved,
        AddrSpec::Manual(a) => canonical(a).map_or(ChannelAddr::Unresolved, render),
        AddrSpec::Auto => {
            let Some(dir) = dir_of(&c.kind) else {
                return ChannelAddr::Excluded;
            };
            let (Some(prof), Some(area)) = (profile, dir_area(&dir)) else {
                return ChannelAddr::Unresolved;
            };
            let s = start.and_then(|s| match dir {
                IoDir::In => s.input,
                _ => s.output,
            });
            match s {
                Some(Start::At(byte)) => {
                    let Some(offset) = map_at(bit_of, u32::from(c.number)) else {
                        return ChannelAddr::Unresolved;
                    };
                    let a = if is_analog(&c.kind) {
                        byte.checked_add(offset / prof.unit_bits)
                            .map(|b| ew_plcaddr::PlcAddress::unit(area, Size::Word, b))
                    } else {
                        prof.bit_at(area, byte, offset)
                    };
                    // An address beyond the u32 byte range (a hostile start or map) is not derivable.
                    a.map_or(ChannelAddr::Unresolved, render)
                }
                Some(Start::Excluded) => ChannelAddr::Excluded,
                Some(Start::Tbd) => ChannelAddr::Tbd,
                Some(Start::Unresolved) | None => ChannelAddr::Unresolved,
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// channel_targets(module)

/// One channel's target (doc 10 §3.5, direct targets).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ChannelTarget {
    /// The channel.
    pub channel: Id<Channel>,
    /// The target function: the manual target, else the function of the bound CP.
    pub function: Option<Id<Function>>,
    /// The target function's device.
    pub device: Option<Id<Device>>,
    /// The representation that draws the bound CP's function (the coil on 203–206), else the target's.
    pub rep: Option<Id<Representation>>,
    /// The bound CP (the coil's internal CP), the link destination of the channel's row (doc 10 §5).
    pub cp: Option<crate::CpRef>,
}

/// `channel_targets(module)`: the direct targets, in channel-number order.
#[derive(Debug)]
pub struct ChannelTargets;

impl Query for ChannelTargets {
    type Key = Id<Device>;
    type Value = Vec<ChannelTarget>;
    const FAMILY: &'static str = "channel_targets";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<ChannelTarget>> {
        let functions = q.functions();
        let reps = q.reps();
        let drawn = |f: Id<Function>| {
            q.idx()
                .reps_of_function(f)
                .find(|r| reps.get(*r).is_some_and(|r| r.rep_type != RepType::ContactImage))
        };
        let out = module_channels(q, *key)
            .iter()
            .map(|c| {
                let bound = match &c.binding {
                    Some(Endpoint::Cp(f, _)) => Some(*f),
                    _ => None,
                };
                let function = c.target.or(bound).filter(|f| functions.contains(*f));
                ChannelTarget {
                    channel: c.id,
                    function,
                    device: function.and_then(|f| functions.get(f).and_then(|f| f.device)),
                    rep: bound.or(function).and_then(drawn),
                    cp: match &c.binding {
                        Some(Endpoint::Cp(f, idx)) if functions.contains(*f) => {
                            Some(crate::CpRef::new(*f, *idx))
                        }
                        _ => None,
                    },
                }
            })
            .collect();
        Arc::new(out)
    }
}

// ---------------------------------------------------------------------------------------------------------
// channel_states(module)

/// The effective state of a channel (doc 10 §3.2).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum EffectiveState {
    /// Bound to a used device.
    Used,
    /// Spare.
    Spare,
    /// Reserved.
    Reserved,
    /// Deactivated.
    Deactivated,
    /// The address is an open item.
    Tbd,
}

impl EffectiveState {
    /// The state as reports print it.
    pub fn name(self) -> &'static str {
        match self {
            EffectiveState::Used => "used",
            EffectiveState::Spare => "spare",
            EffectiveState::Reserved => "reserved",
            EffectiveState::Deactivated => "deactivated",
            EffectiveState::Tbd => "TBD",
        }
    }
}

fn explicit(s: &ChannelState) -> Option<EffectiveState> {
    match s {
        ChannelState::Spare => Some(EffectiveState::Spare),
        ChannelState::Reserved => Some(EffectiveState::Reserved),
        ChannelState::Deactivated => Some(EffectiveState::Deactivated),
        ChannelState::Unknown(_) => None,
    }
}

/// `channel_states(module)`: A3 of doc 10 §3.2 (steps 1, 2, 3 and 5), in channel-number order.
#[derive(Debug)]
pub struct ChannelStates;

impl Query for ChannelStates {
    type Key = Id<Device>;
    type Value = Vec<(Id<Channel>, EffectiveState)>;
    const FAMILY: &'static str = "channel_states";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<(Id<Channel>, EffectiveState)>> {
        let unbound = module_part(q, *key)
            .and_then(|(_, s)| s.unbound.as_ref().and_then(explicit))
            .unwrap_or(EffectiveState::Spare);
        let targets = q.get::<ChannelTargets>(*key);
        let devices = q.devices();
        let out = module_channels(q, *key)
            .iter()
            .map(|c| {
                let target = targets.iter().find(|t| t.channel == c.id);
                let state = if let Some(s) = c.state.as_ref().and_then(explicit) {
                    s
                } else if c.address == AddrSpec::Tbd {
                    EffectiveState::Tbd
                } else if let Some(dev) = target.and_then(|t| t.device).and_then(|d| devices.get(d)) {
                    match dev.state {
                        DeviceState::Spare => EffectiveState::Spare,
                        DeviceState::Reserved => EffectiveState::Reserved,
                        DeviceState::NotFitted => EffectiveState::Deactivated,
                        _ => EffectiveState::Used,
                    }
                } else {
                    unbound
                };
                (c.id, state)
            })
            .collect();
        Arc::new(out)
    }
}

// ---------------------------------------------------------------------------------------------------------
// channel_texts(module)

/// The determined texts of a channel (doc 10 §2.4, §3.4).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ChannelText {
    /// The designation: manual, else the part's pattern over `{n}`.
    pub designation: Option<String>,
    /// The symbol: manual only (templates are 1.0).
    pub symbol: Option<String>,
    /// The function text: the target device's, else the process tag of the device it drives.
    pub function_text: Option<ew_mltext::MlText>,
    /// The process tag: the target device's own, else that of the device it drives.
    pub process_tag: Option<String>,
}

/// The device a device drives (`DeviceLink{Drives}` from it; the smallest link ID when there are several).
pub fn driven_device(q: &Q, device: Id<Device>) -> Option<Id<Device>> {
    q.table::<DeviceLink>()
        .iter()
        .find(|(_, l)| l.from == device && l.kind == DeviceLinkKind::Drives)
        .map(|(_, l)| l.to)
}

/// `channel_texts(module)`: the determined texts, in channel-number order.
#[derive(Debug)]
pub struct ChannelTexts;

impl Query for ChannelTexts {
    type Key = Id<Device>;
    type Value = Vec<(Id<Channel>, ChannelText)>;
    const FAMILY: &'static str = "channel_texts";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<(Id<Channel>, ChannelText)>> {
        let pattern = module_part(q, *key).and_then(|(_, s)| s.channels.and_then(|c| c.designation));
        let targets = q.get::<ChannelTargets>(*key);
        let devices = q.devices();
        let out = module_channels(q, *key)
            .iter()
            .map(|c| {
                let designation = c
                    .designation
                    .as_ref()
                    .map(|d| d.as_str().to_owned())
                    .or_else(|| pattern.as_ref().map(|p| p.replace("{n}", &c.number.to_string())));
                let device = targets.iter().find(|t| t.channel == c.id).and_then(|t| t.device);
                let own = device.and_then(|d| devices.get(d));
                let driven = device
                    .and_then(|d| driven_device(q, d))
                    .and_then(|d| devices.get(d))
                    .and_then(|d| d.process_tag.as_ref().map(|t| t.as_str().to_owned()));
                let own_tag = own.and_then(|d| d.process_tag.as_ref().map(|t| t.as_str().to_owned()));
                let process_tag = own_tag.or_else(|| driven.clone());
                let function_text = own
                    .map(|d| d.function_text.clone())
                    .filter(|t| !t.is_empty())
                    .or_else(|| driven.map(|t| ew_mltext::MlText::neutral(&t)));
                (
                    c.id,
                    ChannelText {
                        designation,
                        symbol: c.symbol.as_ref().map(|s| s.as_str().to_owned()),
                        function_text,
                        process_tag,
                    },
                )
            })
            .collect();
        Arc::new(out)
    }
}

// ---------------------------------------------------------------------------------------------------------
// plc_rows(station)

/// One row of the PLC overview and the I/O list (doc 10 §5): one channel with everything derived.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlcRow {
    /// The station.
    pub station: Id<Device>,
    /// The module device.
    pub module: Id<Device>,
    /// The module's slot.
    pub slot: Slot,
    /// The module's part copy (the one that supplies the PLC section).
    pub part: Option<Id<Part>>,
    /// The channel.
    pub channel: Id<Channel>,
    /// Its number.
    pub number: u16,
    /// Its kind.
    pub kind: ChannelKind,
    /// Its address.
    pub address: ChannelAddr,
    /// Its texts.
    pub text: ChannelText,
    /// Its target.
    pub target: Option<Id<Function>>,
    /// The target's device.
    pub target_device: Option<Id<Device>>,
    /// The representation of the bound coil (or of the target).
    pub rep: Option<Id<Representation>>,
    /// The bound CP, which the row links to (doc 10 §5).
    pub cp: Option<crate::CpRef>,
    /// The effective state.
    pub state: EffectiveState,
}

/// `plc_rows(station)`: the channels of a station's modules, by slot then channel number (doc 10 §5).
#[derive(Debug)]
pub struct PlcRows;

impl Query for PlcRows {
    type Key = Id<Device>;
    type Value = Vec<PlcRow>;
    const FAMILY: &'static str = "plc_rows";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<PlcRow>> {
        let Some(cfg) = q
            .devices()
            .get(*key)
            .and_then(|d| d.aspects.plc_station.as_ref())
            .map(|s| s.config_project)
        else {
            return Arc::new(Vec::new());
        };
        let order = q.get::<ModuleOrder>(cfg);
        let mut out = Vec::new();
        for m in order.iter().filter(|m| m.station == *key) {
            let part = module_part(q, m.module).map(|(c, _)| c.id);
            let channels = module_channels(q, m.module);
            let addresses = q.get::<ChannelAddresses>(m.module);
            let targets = q.get::<ChannelTargets>(m.module);
            let states = q.get::<ChannelStates>(m.module);
            let texts = q.get::<ChannelTexts>(m.module);
            for c in &channels {
                let target = targets.iter().find(|t| t.channel == c.id);
                out.push(PlcRow {
                    station: *key,
                    module: m.module,
                    slot: m.slot,
                    part,
                    channel: c.id,
                    number: c.number,
                    kind: c.kind.clone(),
                    address: addresses
                        .iter()
                        .find(|a| a.channel == c.id)
                        .map_or(ChannelAddr::Unresolved, |a| a.address.clone()),
                    text: texts
                        .iter()
                        .find(|(id, _)| *id == c.id)
                        .map(|(_, t)| t.clone())
                        .unwrap_or_default(),
                    target: target.and_then(|t| t.function),
                    target_device: target.and_then(|t| t.device),
                    rep: target.and_then(|t| t.rep),
                    cp: target.and_then(|t| t.cp),
                    state: states
                        .iter()
                        .find(|(id, _)| *id == c.id)
                        .map_or(EffectiveState::Spare, |(_, s)| *s),
                });
            }
        }
        Arc::new(out)
    }
}

/// The stations of the project, in (configuration project, order, ID) order.
pub fn stations(q: &Q) -> Vec<Id<Device>> {
    let mut out: Vec<(Id<PlcConfigProject>, bool, u32, Id<Device>)> = q
        .devices()
        .iter()
        .filter_map(|(id, d)| {
            let s = d.aspects.plc_station.as_ref()?;
            Some((s.config_project, s.order.is_none(), s.order.unwrap_or(0), id))
        })
        .collect();
    out.sort();
    out.into_iter().map(|(_, _, _, id)| id).collect()
}
