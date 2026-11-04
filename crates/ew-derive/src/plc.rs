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
//! [`ChannelStates`] applies A3: an explicit state, a `Tbd` address, the state of the bound target device, a
//! channel that the solenoid-position map claims gives `Reserved` (step 4, FT-228), else the part's
//! `unbound` state (spare by default). [`SolenoidPositions`] applies the part's position map
//! (`PlcSection.positions`) to the module's valve children; on the golden project (test input H1) it claims
//! no channel, because every coil is bound to channel 2p + 1 (doc 10 §3.2). [`ChannelTargets`] takes the manual target, else
//! the function of the bound CP (direct targets, MVP; traced targets are 1.0). [`ChannelTexts`] determines
//! the designation (manual, else the part pattern over `{n}`), the symbol (manual only: templates are 1.0),
//! the function text (the target device's function text, else the process tag of the device it drives
//! through `DeviceLink{Drives}`) and the process tag (the target device's own, else the driven device's).
//!
//! # Granularity
//!
//! The finest key is a module, station or configuration project (ARCH §6.3). A module reads its channels
//! through the channels-by-module index (`module_channel_ids(module)`, keyed on (module, number) as N1 keys
//! them, FT-319) and then each channel row by ID, so a channel edit invalidates its module only; step 4 reads
//! the CP → channel index. Stations and device links are found by whole-table reads, cut off early by value.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::Ident;
use ew_plcaddr::{
    AddressProfile, Area, AreaIntervals, MapExpr, MapVar, ProfileError, ProfileFields, Purpose, Size,
};
use ew_schema::{
    AddrSpec, Channel, ChannelKind, ChannelState, Device, DeviceLink, DeviceLinkKind, DeviceState, Endpoint,
    FnDef, Function, IoClass, IoDir, LibCopy, LibDocument, MapSpec, Part, PinMap, PlcArea, PlcConfigProject,
    PlcProfileDef, PlcSection, PlcSize, RepType, Representation, SlotPath, SolenoidChannel,
};

use crate::Q;
use crate::channel_index::{ChannelsByCp, ModuleChannelIds};
use crate::note::DeriveNote;
use crate::pattern::{PatternError, expand_pattern};
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
/// does. A hostile parent cycle terminates: the walk stops at a device it has visited. It reads the rows of the
/// device and its ancestors only, never the whole device table, so an edit of another device does not invalidate
/// its readers (WP-S1.21 D9).
pub fn station_of(q: &Q, device: Id<Device>) -> Option<Id<Device>> {
    let devices = q.devices();
    let mut seen = std::collections::BTreeSet::new();
    let mut at = device;
    while seen.insert(at) {
        let dev = devices.get(at)?;
        if dev.aspects.plc_station.is_some() {
            return Some(at);
        }
        at = dev.parent?;
    }
    None
}

/// The built-in address profiles' files (WP-S7.03 D7): `content/plc-profiles`, library documents of kind
/// `plc_profile`, compiled in and parsed once ([`builtin_profiles`]).
const BUILTIN_PROFILE_FILES: [&str; 2] = [
    include_str!("../../../content/plc-profiles/siemens-tia-intl.json"),
    include_str!("../../../content/plc-profiles/iec-61131-3.json"),
];

/// The address profile of a profile document (WP-S7.03 D7): the one parse path of the built-ins and of a user's file.
///
/// # Errors
///
/// The [`ProfileError`] of a profile the engine does not render.
pub fn address_profile(def: &PlcProfileDef) -> Result<AddressProfile, ProfileError> {
    let (a, s) = (&def.mnemonics.area, &def.mnemonics.size);
    AddressProfile::new(ProfileFields {
        id: def.profile.id.clone(),
        version: def.profile.version.clone(),
        label: def.label.clone(),
        absolute: def.absolute,
        base: def.base,
        unit_bits: def.unit_bits,
        word_align: def.word_align,
        area_mnemonics: [&a.i, &a.q, &a.m, &a.pi, &a.pq].map(String::clone),
        size_mnemonics: [&s.bit, &s.byte, &s.word, &s.dword, &s.lword].map(String::clone),
        render_bit: def.render.bit.clone(),
        render_unit: def.render.unit.clone(),
        drawing_prefix: def.render.drawing.clone(),
        exchange_prefix: def.render.exchange.clone(),
        default_start_digital: def.allocation.start.digital,
        default_start_analog: def.allocation.start.analog,
        allocation_verified: def.allocation.verified,
    })
}

/// Why a profile document is no address profile at hand (WP-S7.03 T2), as a library's listing and `library.embed`
/// say it: `the address profile <id@version> cannot be read: <reason>`; `None` for a profile that renders.
pub fn unreadable_profile(def: &PlcProfileDef) -> Option<String> {
    address_profile(def).err().map(|reason| {
        format!(
            "the address profile {}@{} cannot be read: {reason}",
            def.profile.id, def.profile.version
        )
    })
}

/// The built-in address profiles in shipping order (`siemens-tia-intl@2027.1`, `iec-61131-3@2027.1`), parsed once
/// from their compiled-in files. A built-in file that does not read is a build defect, which a test of this crate
/// rules out.
pub fn builtin_profiles() -> &'static [Arc<AddressProfile>] {
    static ALL: std::sync::OnceLock<Vec<Arc<AddressProfile>>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        BUILTIN_PROFILE_FILES
            .iter()
            .map(|text| match LibDocument::from_json(text) {
                Ok(LibDocument::PlcProfile(c)) => Arc::new(
                    address_profile(&c.item).unwrap_or_else(|e| panic!("a built-in address profile: {e}")),
                ),
                other => panic!("a built-in address profile file does not read: {other:?}"),
            })
            .collect()
    })
}

/// The address profiles at hand (WP-S7.03 D8): the built-ins, then the profiles of the project's `plc_profile`
/// `copies` that render and whose pin no built-in has, in copy order. A copy that does not render is left out. A
/// copy that has a built-in's pin takes the built-in's place: the project's own copy comes first, as in
/// [`profile_of`] (ID-1375), so a project prints and parses in one notation.
pub fn profiles_at_hand<'a>(
    copies: impl IntoIterator<Item = &'a LibCopy<PlcProfileDef>>,
) -> Vec<Arc<AddressProfile>> {
    let mut held: Vec<Arc<AddressProfile>> = Vec::new();
    for c in copies {
        if let Ok(p) = address_profile(&c.item)
            && held.iter().all(|o| o.reference() != p.reference())
        {
            held.push(Arc::new(p));
        }
    }
    let mut out: Vec<Arc<AddressProfile>> = builtin_profiles()
        .iter()
        .map(|b| pinned_profile(&held, &b.reference()).unwrap_or_else(|| b.clone()))
        .collect();
    for p in held {
        if out.iter().all(|o| o.reference() != p.reference()) {
            out.push(p);
        }
    }
    out
}

/// The profile of the pin `reference` (`id@version`) among `at_hand`.
pub fn pinned_profile(at_hand: &[Arc<AddressProfile>], reference: &str) -> Option<Arc<AddressProfile>> {
    at_hand.iter().find(|p| p.reference() == reference).cloned()
}

/// The address profile of a configuration project: its pin among the project's `plc_profile` copies, else among
/// the built-ins (WP-S7.03 D8).
pub fn profile_of(q: &Q, cfg: Id<PlcConfigProject>) -> Option<Arc<AddressProfile>> {
    let c = q.table::<PlcConfigProject>().get(cfg)?;
    let reference = c.profile.as_str();
    let copies = q.plc_profiles();
    if let Some(p) = copies
        .iter()
        .filter_map(|(_, c)| address_profile(&c.item).ok())
        .find(|p| p.reference() == reference)
    {
        return Some(Arc::new(p));
    }
    pinned_profile(builtin_profiles(), reference)
}

/// The reference (`id@version`) of the address profile that a configuration project created on demand pins
/// (FR-PLC-008's default, WP-S1.21 D3): the built-in TIA international profile.
pub fn default_profile() -> String {
    builtin_profiles()[0].reference()
}

/// An address text in the notation of the profile `profile` (`id@version`) at hand (`at_hand`), in the stored form
/// of a channel's `Manual` address (WP-S1.21 D6): `"I4.0"` under the TIA profile is input byte 4, bit 0. The error
/// is the message a refusal shows: the profile's example (`expected an address like Q0.1`) for a text that does not
/// parse, or that this build does not ship the profile.
pub fn parse_plc_address(
    at_hand: &[Arc<AddressProfile>],
    profile: &str,
    text: &str,
) -> Result<ew_schema::PlcAddress, String> {
    let Some(p) = pinned_profile(at_hand, profile) else {
        return Err(format!(
            "the address profile {profile} is not shipped by this build, so an address text cannot be read; give \
             auto, excluded or tbd"
        ));
    };
    let Some(a) = p.parse(text) else {
        return Err(format!(
            "`{text}` is not an address of profile {profile}: expected an address like {}",
            p.example()
        ));
    };
    let area = match a.area {
        Area::I => PlcArea::I,
        Area::Q => PlcArea::Q,
        Area::M => PlcArea::M,
        Area::Pi => PlcArea::Pi,
        Area::Pq => PlcArea::Pq,
    };
    let size = match a.size {
        Size::Bit => PlcSize::Bit,
        Size::Byte => PlcSize::Byte,
        Size::Word => PlcSize::Word,
        Size::DWord => PlcSize::DWord,
        Size::LWord => PlcSize::LWord,
    };
    Ok(ew_schema::PlcAddress {
        area,
        size,
        byte: a.byte,
        bit: a.bit,
        extra: ew_schema::Extra::new(),
    })
}

/// A module-start text in the notation of the profile `profile` (`id@version`) at hand (`at_hand`), in the stored
/// form of a `Manual` start (WP-S1.21 D8, `plc.set_start`): the area and the byte, no size and no bit (`"Q10"`,
/// `"Q10.0"` and `"QB10"` under the TIA profile are output byte 10; see [`AddressProfile::parse_start`]). The error is
/// the message a refusal shows, naming the profile's example start (`expected a start like Q0`).
pub fn parse_plc_start(
    at_hand: &[Arc<AddressProfile>],
    profile: &str,
    text: &str,
) -> Result<ew_schema::PlcAddress, String> {
    let Some(p) = pinned_profile(at_hand, profile) else {
        return Err(format!(
            "the address profile {profile} is not shipped by this build, so a start text cannot be read; give auto, \
             excluded or tbd"
        ));
    };
    let Some((area, byte)) = p.parse_start(text) else {
        return Err(format!(
            "`{text}` is not a start of profile {profile}: expected a start like {}",
            start_example(&p, Area::Q)
        ));
    };
    Ok(ew_schema::PlcAddress {
        area: plc_area(area),
        size: PlcSize::Bit,
        byte,
        bit: None,
        extra: ew_schema::Extra::new(),
    })
}

/// The example start of `area` in profile `p` (`Q0` under TIA, `%QX0` under IEC): the unit template with the bit-size
/// mnemonic and byte 0.
pub fn start_example(p: &AddressProfile, area: Area) -> String {
    p.render(
        &ew_plcaddr::PlcAddress::unit(area, Size::Bit, 0),
        Purpose::Drawing,
    )
}

/// The reference (`id@version`) of the profile among `at_hand` ([`profiles_at_hand`]) that `text` names: its exact
/// reference, or its ID alone (`iec-61131-3`), compared without regard to ASCII case; the first in `at_hand` order.
/// `None` when no profile at hand has that name (`plc.set_profile`, WP-S1.21 D8, WP-S7.03 D8).
pub fn address_profile_named(at_hand: &[Arc<AddressProfile>], text: &str) -> Option<String> {
    let text = text.trim();
    at_hand
        .iter()
        .find(|p| p.reference().eq_ignore_ascii_case(text) || p.id.eq_ignore_ascii_case(text))
        .map(|p| p.reference())
}

/// The stored area of a canonical area.
fn plc_area(a: Area) -> PlcArea {
    match a {
        Area::I => PlcArea::I,
        Area::Q => PlcArea::Q,
        Area::M => PlcArea::M,
        Area::Pi => PlcArea::Pi,
        Area::Pq => PlcArea::Pq,
    }
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

/// The canonical form of a stored address; `None` for an area or a size of a newer format.
pub fn canonical(a: &ew_schema::PlcAddress) -> Option<ew_plcaddr::PlcAddress> {
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

/// The value of a part-data map over the variable `var` at `x` (doc 10 §2.6): an expression in `var`, or the
/// list entry at index `x`. `None` for an unparsable expression, an expression in the other variable, an
/// index outside a list, an overflow or a form of a newer format.
pub fn eval_map(map: &MapSpec, var: MapVar, x: u32) -> Option<u32> {
    match map {
        MapSpec::Expr(s) => {
            let e = MapExpr::parse(s)?;
            if e.var != var {
                return None;
            }
            e.eval(x)
        }
        MapSpec::List(l) => l.get(usize::try_from(x).ok()?).copied(),
        MapSpec::Unknown(_) => None,
    }
}

/// The value of a part-data channel map at `x`; absent: linear (`x`). See [`eval_map`].
fn map_at(map: Option<&MapSpec>, x: u32) -> Option<u32> {
    map.map_or(Some(x), |m| eval_map(m, MapVar::N, x))
}

/// The channel number a solenoid-position map entry assigns at position index `p` (0-based; doc 10 §2.6):
/// a `p` expression or a list indexed by `p`; `None` when it does not resolve or exceeds `u16`. The single
/// evaluation of the map, shared by `solenoid_positions` and normaliser N2 (`ew-commands`).
pub fn solenoid_channel(entry: &SolenoidChannel, p: u32) -> Option<u16> {
    eval_map(&entry.channel, MapVar::P, p).and_then(|c| u16::try_from(c).ok())
}

/// The pin designation a channel → pin map assigns to channel `number` (doc 10 §3.3, normaliser N3): the
/// pattern expanded by [`expand_pattern`] (`"X2:{n+1}"` → `X2:1` for channel 0), or the list entry at
/// `number`. `None` for a pattern that does not expand, an index outside the list, an empty result or a form
/// of a newer format; [`pin_pattern_error`] gives the reason of the first.
pub fn channel_pin(pin_of: &PinMap, number: u16) -> Option<String> {
    let out = match pin_of {
        PinMap::Pattern(pattern) => expand_pattern(pattern, u32::from(number)).ok()?,
        PinMap::List(l) => l.get(usize::from(number))?.as_str().to_owned(),
        PinMap::Unknown(_) => return None,
    };
    (!out.is_empty()).then_some(out)
}

/// Why a channel → pin map gives no pin for channel `number`, when its pattern does not expand (for the notice
/// of normaliser N3); `None` for the other reasons ([`channel_pin`]).
pub fn pin_pattern_error(pin_of: &PinMap, number: u16) -> Option<PatternError> {
    match pin_of {
        PinMap::Pattern(pattern) => expand_pattern(pattern, u32::from(number)).err(),
        _ => None,
    }
}

/// The solenoids a valve part declares (WP-S1.20 D4, D5): its `fluid.solenoids`, `None` for a part without a fluid
/// section with solenoids. A device's **valve part** is its first part use whose copy gives `Some`.
pub fn valve_solenoids(part: &Part) -> Option<&[Ident]> {
    part.sections
        .fluid
        .as_ref()
        .map(|f| f.solenoids.as_slice())
        .filter(|s| !s.is_empty())
}

/// The coil CP of solenoid `solenoid` in one function (WP-S1.20 D5): on a FnDef of contact kind `coil`, its first
/// non-physical CP (by index) whose effective designation (the function's override, else the FnDef CP's) is the
/// solenoid's name; `None` on any other FnDef. A physical coil gives none. Shared by [`solenoid_coil`] and
/// normaliser N2, so the derivation and the binding agree.
pub fn coil_cp(
    def: &FnDef,
    overrides: &ew_schema::KeyedList<ew_schema::CpOverride>,
    solenoid: &Ident,
) -> Option<u16> {
    if def.contact_kind != ew_schema::ContactKind::Coil {
        return None;
    }
    def.cps
        .iter()
        .filter(|cp| !cp.physical)
        .find(|cp| {
            let designation = overrides
                .iter()
                .find(|o| o.idx == cp.idx)
                .and_then(|o| o.designation.as_ref())
                .unwrap_or(&cp.designation);
            designation == solenoid
        })
        .map(|cp| cp.idx)
}

/// `solenoid_coil(device, s)` (WP-S1.20 D5, replacing the interim rule of decision log ID-469): a valve **declares**
/// solenoid s exactly when s is in `fluid.solenoids` of its valve part ([`valve_solenoids`]); its coil CP is the
/// first [`coil_cp`] among the device's functions in function-ID order. `None` when the device does not declare s
/// or has no internal coil for it. Read by [`SolenoidPositions`] and, through the same rule, normaliser N2.
pub fn solenoid_coil(q: &Q, device: Id<Device>, solenoid: &Ident) -> Option<crate::CpRef> {
    let dev = q.devices().get(device)?;
    let parts = q.parts();
    let declared = dev.parts.iter().find_map(|u| {
        parts
            .get(u.part.copy)
            .and_then(|c| valve_solenoids(&c.item).map(<[Ident]>::to_vec))
    })?;
    if !declared.contains(solenoid) {
        return None;
    }
    let functions = q.functions();
    let fndefs = q.fndefs();
    let mut fns: Vec<Id<Function>> = q.idx().functions_of_device(device).collect();
    fns.sort();
    fns.into_iter().find_map(|f| {
        let func = functions.get(f)?;
        let def = fndefs.get(func.def)?;
        coil_cp(&def.item, &func.cp_overrides, solenoid).map(|idx| crate::CpRef::new(f, idx))
    })
}

/// The function whose representation draws an internal coil (doc 11 §3.1: the coil is drawn in its valve's
/// symbol, WP-S1.20 D5): for a function on a FnDef of contact kind `coil` without a representation of its own, the
/// lowest-ID function of its device on a FnDef of contact kind `valve`; `None` otherwise. The PLC rows and the CP
/// link destinations read it, so a channel bound to an internal coil points at its valve.
pub fn coil_host(q: &Q, function: Id<Function>) -> Option<Id<Function>> {
    let functions = q.functions();
    let fndefs = q.fndefs();
    let kind = |f: &Function| fndefs.get(f.def).map(|d| d.item.contact_kind.clone());
    let func = functions.get(function)?;
    if kind(func) != Some(ew_schema::ContactKind::Coil) || q.idx().reps_of_function(function).next().is_some()
    {
        return None;
    }
    let mut fns: Vec<Id<Function>> = q.idx().functions_of_device(func.device?).collect();
    fns.sort();
    fns.into_iter().find(|f| {
        functions
            .get(*f)
            .is_some_and(|x| kind(x) == Some(ew_schema::ContactKind::Valve))
    })
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

/// `module_start_of(module)`: one module's entry of [`ModuleStarts`] of its configuration project, `None` for a module
/// outside every configuration project (WP-S1.21 D9, FT-224). A projection read as a cut-off point: a start edit of one
/// module re-runs [`ModuleStarts`] and every module's projection, but only the modules whose own start value changed
/// pass the change on, so [`ChannelAddresses`] re-runs for them alone (AC-29: "only its 8 channels").
#[derive(Debug)]
pub struct ModuleStartOf;

impl Query for ModuleStartOf {
    type Key = Id<Device>;
    type Value = Option<ModuleStart>;
    const FAMILY: &'static str = "module_start_of";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Option<ModuleStart>> {
        let start = module_cfg(q, *key).and_then(|(_, cfg)| q.get::<ModuleStarts>(cfg).get(key).copied());
        Arc::new(start)
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

/// The channel records of a module for the numbers of the part's channel spec, in number order: the module's
/// slice of the channels-by-module index ([`ModuleChannelIds`], keyed on (module, number) as N1 keys the
/// channels), then each row by ID. An edit of one channel invalidates its own module's readers only.
fn module_channels(q: &Q, module: Id<Device>) -> Vec<Arc<Channel>> {
    let Some((_, plc)) = module_part(q, module) else {
        return Vec::new();
    };
    let count = plc.channels.as_ref().map_or(0, |c| c.count);
    let table = q.table::<Channel>();
    q.get::<ModuleChannelIds>(module)
        .iter()
        .filter(|(number, _)| *number < count)
        .filter_map(|(_, id)| table.get(*id).cloned())
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
    /// The address A2 derives for it with the specification `auto` (the module's start and the part's map),
    /// whatever its own specification: the value a `Manual` address deviates from (EW-PLC-012 `manual`). `None`
    /// when that does not resolve to an address.
    pub auto: Option<ew_plcaddr::PlcAddress>,
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
        // D9: this module's start only, so another module's start edit cuts off before this family
        let start = *q.get::<ModuleStartOf>(*key);
        let bit_of = module_part(q, *key).and_then(|(_, s)| s.bit_of);
        let out = channels
            .iter()
            .map(|c| {
                let address = address_of(c, profile.as_deref(), start, bit_of.as_ref());
                let auto = match (&c.address, &address) {
                    (AddrSpec::Auto, ChannelAddr::At { address, .. }) => Some(*address),
                    (AddrSpec::Auto, _) => None,
                    _ => {
                        let derived = Channel {
                            address: AddrSpec::Auto,
                            ..c.as_ref().clone()
                        };
                        match address_of(&derived, profile.as_deref(), start, bit_of.as_ref()) {
                            ChannelAddr::At { address, .. } => Some(address),
                            _ => None,
                        }
                    }
                };
                ChannelAddrEntry {
                    channel: c.id,
                    number: c.number,
                    address,
                    auto,
                }
            })
            .collect();
        Arc::new(out)
    }
}

// ---------------------------------------------------------------------------------------------------------
// address_index(cfg)

/// One channel address in use in a configuration project ([`AddressIndex`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AddressUse {
    /// The module device.
    pub module: Id<Device>,
    /// The channel.
    pub channel: Id<Channel>,
    /// Its number.
    pub number: u16,
    /// Its derived address.
    pub address: ew_plcaddr::PlcAddress,
    /// The address as the drawing prints it ("Q0.1").
    pub text: String,
    /// Whether the channel's own specification is `Manual`.
    pub manual: bool,
    /// The first bit of the address in its area (the byte times the profile's unit width, plus the bit).
    pub from: u64,
    /// One past the last bit: `from` plus the access size's width.
    pub to: u64,
}

/// The value of [`AddressIndex`]: the addresses in use and the groups among them that overlap.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct AddressIndexV {
    /// Every channel of the configuration project that resolves to an address, in (area, first bit, module, number,
    /// channel) order.
    pub uses: Vec<AddressUse>,
    /// The overlap groups (FX38a): each connected run of [`uses`](Self::uses) in one area whose bit ranges overlap one
    /// another in a chain (`IW0` against `I1.3`), once, as ascending indices into `uses` (so in address order), two
    /// members or more; the groups in the order of their first member. Their total size is at most `uses.len()`, so
    /// N channels on one address make one group of N, never N(N−1)/2 pairs.
    pub groups: Vec<Vec<usize>>,
}

/// `address_index(cfg)` (doc 10 §2.10): the channel addresses of a configuration project as bit intervals per area,
/// sorted, with the overlap groups (EW-PLC-001). The unit width is the configuration project's profile's; without a
/// shipped profile no address resolves and the value is empty.
#[derive(Debug)]
pub struct AddressIndex;

impl Query for AddressIndex {
    type Key = Id<PlcConfigProject>;
    type Value = AddressIndexV;
    const FAMILY: &'static str = "address_index";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<AddressIndexV> {
        let Some(prof) = profile_of(q, *key) else {
            return Arc::new(AddressIndexV::default());
        };
        let unit = u64::from(prof.unit_bits);
        let channels = q.table::<Channel>();
        let mut uses: Vec<AddressUse> = Vec::new();
        for m in q.get::<ModuleOrder>(*key).iter() {
            for e in q.get::<ChannelAddresses>(m.module).iter() {
                let ChannelAddr::At { address, text } = &e.address else {
                    continue;
                };
                let from = u64::from(address.byte) * unit + address.bit.map_or(0, u64::from);
                uses.push(AddressUse {
                    module: m.module,
                    channel: e.channel,
                    number: e.number,
                    address: *address,
                    text: text.clone(),
                    manual: channels
                        .get(e.channel)
                        .is_some_and(|c| matches!(c.address, AddrSpec::Manual(_))),
                    from,
                    to: from + u64::from(address.size.bits()),
                });
            }
        }
        uses.sort_by(|a, b| {
            (a.address.area, a.from, a.module, a.number, a.channel).cmp(&(
                b.address.area,
                b.from,
                b.module,
                b.number,
                b.channel,
            ))
        });
        // One sweep in (area, first bit) order: a use joins the current run when it starts below the run's end in
        // the same area; the runs of two or more are the groups (the components of the interval-overlap graph).
        let mut groups = Vec::new();
        let mut run: Vec<usize> = Vec::new();
        let mut end = 0u64;
        for (i, u) in uses.iter().enumerate() {
            let joins = run
                .last()
                .is_some_and(|&l| uses[l].address.area == u.address.area && u.from < end);
            if joins {
                run.push(i);
                end = end.max(u.to);
            } else {
                if run.len() > 1 {
                    groups.push(std::mem::take(&mut run));
                }
                run.clear();
                run.push(i);
                end = u.to;
            }
        }
        if run.len() > 1 {
            groups.push(run);
        }
        Arc::new(AddressIndexV { uses, groups })
    }
}

/// The derived (`auto`) address of channel `number` of `module` with the module's current start, whatever the
/// channel's own address specification: the value a `Manual` address is compared against (EW-PLC-012 `manual`,
/// `plc.convert_manual_to_auto`). `None` when the channel or the address does not resolve.
pub fn auto_address(q: &Q, module: Id<Device>, channel: Id<Channel>) -> Option<ew_plcaddr::PlcAddress> {
    q.get::<ChannelAddresses>(module)
        .iter()
        .find(|e| e.channel == channel)
        .and_then(|e| e.auto)
}

fn address_of(
    c: &Channel,
    profile: Option<&AddressProfile>,
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
                    // WP-S1.20 D5: an internal coil is drawn in its valve's symbol
                    rep: bound
                        .or(function)
                        .and_then(|f| drawn(f).or_else(|| coil_host(q, f).and_then(drawn))),
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
// solenoid_positions(module)

/// One solenoid of one valve position of a module (doc 10 §2.6, §3.2 step 4): what the module part's
/// solenoid-position map assigns, and what the valve child at that position declares.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SolenoidSlot {
    /// The 1-based position (`Device.position`; the map's index is `position − 1`).
    pub position: u32,
    /// The solenoid name of the map entry ("14").
    pub solenoid: Ident,
    /// The channel number the map assigns; `None` when the entry does not resolve at this position or the
    /// number is not below the part's channel count.
    pub channel: Option<u16>,
    /// The device at that position among the module's children (the lowest ID when several hold it).
    pub valve: Option<Id<Device>>,
    /// The valve's coil CP for the solenoid ([`solenoid_coil`]); `None` when the valve does not declare the
    /// solenoid or no valve sits at the position.
    pub coil: Option<crate::CpRef>,
}

/// `solenoid_positions(module)`: the solenoid-position map of the module's part (doc 10 §2.6
/// `[plc.positions]`) applied to the module's valve children, one [`SolenoidSlot`] per position (1 …
/// `count`) and map entry, in (position, map order). Empty for a module whose part has no position map.
/// Derivation step 4 of doc 10 §3.2 ([`ChannelStates`]) and, later, EW-PLC-012 (FT-192) read it; normaliser
/// N2 (`ew-commands`) applies the same map through [`solenoid_channel`] and the rule of [`solenoid_coil`].
#[derive(Debug)]
pub struct SolenoidPositions;

impl Query for SolenoidPositions {
    type Key = Id<Device>;
    type Value = Vec<SolenoidSlot>;
    const FAMILY: &'static str = "solenoid_positions";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<SolenoidSlot>> {
        let Some((_, plc)) = module_part(q, *key) else {
            return Arc::new(Vec::new());
        };
        let Some(map) = plc.positions.as_ref() else {
            return Arc::new(Vec::new());
        };
        let channel_count = plc.channels.as_ref().map_or(0, |c| c.count);
        let devices = q.devices();
        // the child at each position, the lowest ID first
        let mut at: BTreeMap<u32, Id<Device>> = BTreeMap::new();
        let mut children: Vec<Id<Device>> = q.idx().device_children(*key).collect();
        children.sort();
        for c in children {
            if let Some(p) = devices.get(c).and_then(|d| d.position) {
                at.entry(p).or_insert(c);
            }
        }
        let mut out = Vec::new();
        for position in 1..=u32::from(map.count) {
            let valve = at.get(&position).copied();
            for entry in &map.map {
                let channel = solenoid_channel(entry, position - 1).filter(|c| *c < channel_count);
                let coil = valve.and_then(|v| solenoid_coil(q, v, &entry.solenoid));
                out.push(SolenoidSlot {
                    position,
                    solenoid: entry.solenoid.clone(),
                    channel,
                    valve,
                    coil,
                });
            }
        }
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

/// `channel_states(module)`: A3 of doc 10 §3.2 (steps 1 to 5), in channel-number order. Step 4 reads
/// [`SolenoidPositions`].
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
        let slots = q.get::<SolenoidPositions>(*key);
        let devices = q.devices();
        let channels = module_channels(q, *key);
        // Step 4: the map claims channel c when it assigns c to solenoid s of position p, the valve at p
        // declares s, and s is unbound or bound to c. Only an unbound channel reaches step 4, so "bound to c"
        // cannot hold there; a coil counts as bound when any channel binds it, of this module or another
        // (a valve moved by `assembly.attach` keeps its binding) and whatever its ID (review ruling, cycle
        // 18): the CP → channel index, as normaliser N2 and `plc.bind_channel` read it (FT-319). A module
        // whose map claims nothing does not read it.
        let claimed: std::collections::BTreeSet<u16> =
            if slots.iter().any(|s| s.coil.is_some() && s.channel.is_some()) {
                let bound = q.get::<ChannelsByCp>(());
                slots
                    .iter()
                    .filter_map(|s| {
                        let coil = s.coil?;
                        let channel = s.channel?;
                        (!bound.is_bound(coil.function, coil.idx)).then_some(channel)
                    })
                    .collect()
            } else {
                std::collections::BTreeSet::new()
            };
        let out = channels
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
                } else if c.binding.is_none() && claimed.contains(&c.number) {
                    EffectiveState::Reserved
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
        let part = module_part(q, *key);
        let pattern = part
            .as_ref()
            .and_then(|(_, s)| s.channels.as_ref().and_then(|c| c.designation.clone()));
        let targets = q.get::<ChannelTargets>(*key);
        let devices = q.devices();
        // a pattern that does not expand gives no automatic designation, with one note per module (FT-318)
        let mut noted = false;
        let out = module_channels(q, *key)
            .iter()
            .map(|c| {
                let designation = c.designation.as_ref().map(|d| d.as_str().to_owned()).or_else(|| {
                    let pattern = pattern.as_ref()?;
                    match expand_pattern(pattern, u32::from(c.number)) {
                        Ok(text) => (!text.is_empty()).then_some(text),
                        Err(e) => {
                            if !std::mem::replace(&mut noted, true) {
                                q.note(DeriveNote::DesignationPattern {
                                    module: key.to_string(),
                                    pattern: pattern.clone(),
                                    detail: e.to_string(),
                                });
                            }
                            None
                        }
                    }
                });
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
// plc_address_text(function)

/// The printed PLC addresses of a function (doc 10 §3.6; WP-S1.21 D7, FT-227): what a `PlcAddress` text slot
/// of a representation drawing it prints, and what each bound pin of a black-box module prints beside it. Never
/// stored (FR-PLC-007): every text is the derived [`ChannelAddr`] of the bound channel rendered by the
/// configuration project's profile.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PlcAddressTexts {
    /// The slot text: the addresses of the function's own bound CPs, else (a fluid function, contact kind
    /// `valve`) of the bound internal coil CPs of its device's other functions; in solenoid order (a CP designated
    /// `14`, then `12`, then the others by CP index), joined by `" / "`. `None` when no address resolves.
    pub text: Option<String>,
    /// The function's own bound CPs that resolve, by CP index: `(idx, address text)`.
    pub pins: Vec<(u16, String)>,
}

/// `plc_address_text(function)`: the [`PlcAddressTexts`] of a function. A CP bound to several channels prints the
/// channel with the lowest ID. An address resolves to the profile's drawing form (`Q0.1`), an open item to
/// `TBD`; an excluded or unresolved address prints nothing, and so does a function without a binding.
#[derive(Debug)]
pub struct PlcAddressText;

impl Query for PlcAddressText {
    type Key = Id<Function>;
    type Value = PlcAddressTexts;
    const FAMILY: &'static str = "plc_address_text";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<PlcAddressTexts> {
        let Some(function) = q.functions().get(*key).cloned() else {
            return Arc::new(PlcAddressTexts::default());
        };
        let bound = q.get::<ChannelsByCp>(());
        let channels = q.table::<Channel>();
        let address = |id: Id<Channel>| -> Option<String> {
            let module = channels.get(id)?.module;
            let entry = q.get::<ChannelAddresses>(module);
            match &entry.iter().find(|e| e.channel == id)?.address {
                ChannelAddr::At { text, .. } => Some(text.clone()),
                ChannelAddr::Tbd => Some("TBD".to_owned()),
                ChannelAddr::Excluded | ChannelAddr::Unresolved => None,
            }
        };
        // (solenoid rank, function, CP index, text) of the bound CPs of `f` that `keep` admits
        let collect =
            |f: Id<Function>, keep: &dyn Fn(&crate::Cp) -> bool| -> Vec<(u8, Id<Function>, u16, String)> {
                q.get::<crate::FunctionCps>(f)
                    .iter()
                    .filter(|cp| keep(cp))
                    .filter_map(|cp| {
                        let channel = bound.at_cp(f, cp.idx).next()?;
                        let rank = match cp.designation.as_str() {
                            "14" => 0,
                            "12" => 1,
                            _ => 2,
                        };
                        Some((rank, f, cp.idx, address(channel)?))
                    })
                    .collect()
            };
        let mut own = collect(*key, &|_| true);
        let pins = {
            let mut p: Vec<(u16, String)> = own.iter().map(|(_, _, idx, t)| (*idx, t.clone())).collect();
            p.sort();
            p
        };
        if own.is_empty() {
            let fluid = q
                .fndefs()
                .get(function.def)
                .is_some_and(|d| d.item.contact_kind == ew_schema::ContactKind::Valve);
            if let (true, Some(device)) = (fluid, function.device) {
                let mut others: Vec<Id<Function>> =
                    q.idx().functions_of_device(device).filter(|f| f != key).collect();
                others.sort();
                for f in others {
                    own.extend(collect(f, &|cp| !cp.physical));
                }
            }
        }
        own.sort();
        let text = (!own.is_empty()).then(|| {
            own.iter()
                .map(|(_, _, _, t)| t.as_str())
                .collect::<Vec<_>>()
                .join(" / ")
        });
        Arc::new(PlcAddressTexts { text, pins })
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
