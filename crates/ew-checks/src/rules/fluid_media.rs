// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The EW-FL pack, its 1.0 rules (doc 11 §6, doc 15 §4.5.3): EW-FL-005 … -010, -012 and -014. They read the
//! media propagation, the effective tube properties and the physical tubes of `ew-derive` (doc 11 §5).
//!
//! Texts name a tube by its line tag, else by its two ends (`-201U3-Q1:2 → -203Q1:A`), a port by `DT:port` and a
//! medium by its name; lists are in natural order. IDs enter only targets and fingerprints.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::{
    CpRef, FluidNets, FunctionCps, Galleries, MediumKey, MediumOrigin, MediumZones, NetId, PotentialAttrsOf,
    PotentialPartition, Q, TubeRow, TubeRows, cp_label, is_fluid, is_fluid_terminal, node_net, parent_chain,
    solenoid_coil, valve_functions, valve_solenoids,
};
use ew_ids::Id;
use ew_mltext::natural_key;
use ew_model::EndpointKey;
use ew_schema::{
    AcDc, ContactKind, CpOverride, Device, DeviceState, FluidPortRole, Function, Kind, Medium, SupplyKind,
};
use ew_units::Dec3;

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::term::full_tag;
use crate::rules::{and_listed, device_tag};
use ew_l10n::TextArg;

/// A tube as the messages name it: its line tag, else its two ends ("tee" at a junction), as the subject prints it and
/// in the reader's language.
fn tube_text(r: &TubeRow) -> (String, TextArg) {
    let name = TextArg::Ml(r.name());
    (name.to_string(), name)
}

/// Whether a CP of a function carries a closure.
fn closed(q: &Q, f: Id<Function>, idx: u16) -> bool {
    q.functions().get(f).is_some_and(|x| {
        x.cp_overrides
            .iter()
            .any(|o: &CpOverride| o.idx == idx && o.closure.is_some())
    })
}

/// The members of a net, or the node alone for a net of one node.
fn members(q: &Q, net: NetId, alone: EndpointKey) -> Vec<EndpointKey> {
    let nets = q.get::<FluidNets>(());
    nets.net_of(alone).map_or_else(
        || vec![alone],
        |n| {
            debug_assert_eq!(n.id, net);
            n.members.clone()
        },
    )
}

macro_rules! meta {
    ($name:ident, $id:literal, $message:literal, $code:literal, $title:literal, $kind:ident, $modes:literal, $sev:ident, $scope:ident) => {
        #[doc = concat!($id, " \"", $title, "\" (doc 11 §6): 1.0.")]
        pub const $name: RuleMeta = RuleMeta {
            id: $id,
            message: key!($message),
            code: $code,
            title: $title,
            kind: RuleKind::$kind,
            modes: Modes::letters($modes),
            severity: Severity::$sev,
            release_severity: None,
            scope: ScopeKind::$scope,
            supersedes: &[],
            release: Release::V1_0,
        };
    };
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-005

meta!(
    TWO_MEDIA,
    "EW-FL-005",
    "rule-ew-fl-005",
    "120005",
    "Two media in one net",
    Conflict,
    "or",
    Error,
    Project
);

/// EW-FL-005 (FR-FLU-019): a fluid net that two different media reach directly (two definition points, or a
/// definition and a regulator's outlet); target the net's lines, else its definition points.
#[derive(Clone, Copy, Debug)]
pub struct TwoMedia;

impl Rule for TwoMedia {
    fn meta(&self) -> &'static RuleMeta {
        &TWO_MEDIA
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let zones = q.get::<MediumZones>(());
        let nets = q.get::<FluidNets>(());
        let mut out = Vec::new();
        for (net, keys) in &zones.media {
            let direct: BTreeSet<&MediumKey> = keys
                .iter()
                .filter(|(_, o)| *o == MediumOrigin::Direct)
                .map(|(k, _)| k)
                .collect();
            if direct.len() < 2 {
                continue;
            }
            let found = nets.nets.iter().find(|n| n.id == *net);
            let mut targets: Vec<Target> = found
                .map(|n| {
                    n.connections
                        .iter()
                        .map(|c| Target::new(Kind::Connection, *c))
                        .collect()
                })
                .unwrap_or_default();
            let first = found.and_then(|n| n.members.iter().find_map(|m| CpRef::of(*m)));
            if targets.is_empty() {
                targets = q
                    .table::<ew_schema::MediumDef>()
                    .iter()
                    .filter(|(_, d)| EndpointKey::of(&d.at).and_then(|a| node_net(q, a)) == Some(*net))
                    .map(|(id, _)| Target::new(Kind::MediumDef, id))
                    .collect();
            }
            let at = first
                .map(|cp| cp_label(q, cp))
                .unwrap_or_else(|| "a fluid net".to_owned());
            let mut media: Vec<TextArg> = direct.iter().map(|k| TextArg::Ml(k.text(q))).collect();
            media.sort_by_cached_key(|m| natural_key(&m.to_string()));
            out.push(
                Finding::new(&TWO_MEDIA, targets, "media", at.clone())
                    .arg("at", at)
                    .arg("media", and_listed(media)),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-006

meta!(
    TUBE_MISFIT,
    "EW-FL-006",
    "rule-ew-fl-006",
    "120006",
    "Tube does not fit",
    Conflict,
    "or",
    Warning,
    Project
);

/// The highest nominal pressure a net can carry: the media's `p_nom`, a regulated key's set pressure (exhaust and
/// vacuum aside).
fn highest_pressure(q: &Q, net: NetId) -> Option<Dec3> {
    let media = q.table::<Medium>();
    q.get::<MediumZones>(())
        .keys(net)
        .filter_map(|(k, _)| match k {
            MediumKey::Defined(m) => media.get(*m).map(|m| m.p_nom),
            MediumKey::Regulated(_, p) => Some(*p),
            MediumKey::Vacuum(_) | MediumKey::Exhaust(_) => None,
        })
        .max()
}

/// The push-in outer diameter of a port: the valve module's entry for the valve's position, else the device's own
/// part entry (doc 11 §2.6).
pub fn push_in_od(q: &Q, f: Id<Function>, port: &str) -> Option<ew_units::Nm> {
    let device = q.functions().get(f)?.device?;
    let devices = q.devices();
    let parts = q.parts();
    let dev = devices.get(device)?;
    let entry = |d: &Device, position: Option<u16>| {
        d.parts.iter().find_map(|u| {
            parts
                .get(u.part.copy)?
                .item
                .sections
                .fluid
                .as_ref()?
                .ports
                .iter()
                .find(|p| p.designation.as_str() == port && p.position == position)?
                .push_in_od
        })
    };
    let position = dev.position.and_then(|p| u16::try_from(p).ok());
    dev.parent
        .and_then(|m| devices.get(m))
        .and_then(|m| position.and_then(|p| entry(m, Some(p))))
        .or_else(|| entry(dev, None))
}

/// EW-FL-006 (FR-FLU-020): per tube, a pressure rating below the highest nominal pressure its net carries, and an
/// outer diameter other than the push-in size of the port at either end; one finding per misfit, target the tube.
#[derive(Clone, Copy, Debug)]
pub struct TubeMisfit;

impl Rule for TubeMisfit {
    fn meta(&self) -> &'static RuleMeta {
        &TUBE_MISFIT
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let mut out = Vec::new();
        for r in q.get::<TubeRows>(()).iter() {
            let (text, text_nl) = tube_text(r);
            if let (Some(rating), Some(p)) = (&r.props.p_rating, r.net.and_then(|n| highest_pressure(q, n)))
                && rating.value < p
            {
                out.push(
                    Finding::new(
                        &TUBE_MISFIT,
                        [Target::new(Kind::Connection, r.id)],
                        "rating",
                        text.clone(),
                    )
                    .arg("form", "rating")
                    .arg("tube", text_nl.clone())
                    .arg("rating", rating.value.to_string())
                    .arg("pressure", p.to_string()),
                );
            }
            let Some(od) = &r.props.od else { continue };
            for end in [&r.from, &r.to] {
                let EndpointKey::Cp(f, _) = end.at else { continue };
                let Some(port_od) = push_in_od(q, f, &end.port) else {
                    continue;
                };
                if port_od != od.value {
                    out.push(
                        Finding::new(
                            &TUBE_MISFIT,
                            [Target::new(Kind::Connection, r.id)],
                            format!("od {}", end.text),
                            text.clone(),
                        )
                        .arg("form", "od")
                        .arg("tube", text_nl.clone())
                        .arg("od", ew_derive::mm_micro(od.value))
                        .arg("port", end.text.clone())
                        .arg("port_od", ew_derive::mm_micro(port_od)),
                    );
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-007

meta!(
    NO_TUBE_SPEC,
    "EW-FL-007",
    "rule-ew-fl-007",
    "120007",
    "Tube without specification",
    Completeness,
    "or",
    Warning,
    Project
);

/// EW-FL-007 (FR-FLU-020): a tube without an effective outer and inner diameter and material (doc 11 §5.4); target the
/// tube.
#[derive(Clone, Copy, Debug)]
pub struct NoTubeSpec;

impl Rule for NoTubeSpec {
    fn meta(&self) -> &'static RuleMeta {
        &NO_TUBE_SPEC
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        q.get::<TubeRows>(())
            .iter()
            .filter(|r| !r.props.is_specified())
            .map(|r| {
                let (text, text_nl) = tube_text(r);
                // A tube the tube table showed as 6×4 had "no tube specification"; it lacked a
                // material, and the finding says so.
                let size = r.props.size_text();
                let missing = if size.is_empty() { "spec" } else { "material" };
                Finding::new(
                    &NO_TUBE_SPEC,
                    [Target::new(Kind::Connection, r.id)],
                    "spec",
                    text.clone(),
                )
                .arg("tube", text_nl)
                .arg("missing", missing)
                .arg("size", size)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-008

meta!(
    EXHAUST_OPEN,
    "EW-FL-008",
    "rule-ew-fl-008",
    "120008",
    "Exhaust open",
    Completeness,
    "o",
    Info,
    Project
);

/// Whether a role is an exhaust role (3, 5, 82, 84).
fn exhaust_role(role: &Option<FluidPortRole>) -> bool {
    matches!(
        role,
        Some(
            FluidPortRole::Exhaust3
                | FluidPortRole::Exhaust5
                | FluidPortRole::PilotExhaust82
                | FluidPortRole::PilotExhaust84
        )
    )
}

/// EW-FL-008 (FR-FLU-021): a fluid net whose ports all have exhaust roles, or that carries exhaust, with no line (a
/// collector), no closure (a silencer, plug or blanking) and no port of another role; one finding per net, target
/// the functions of its ports. A gallery of a valve terminal is named as the terminal's gallery. Off by default
/// (`pre-delivery` and `migration` switch it off).
#[derive(Clone, Copy, Debug)]
pub struct ExhaustOpen;

impl Rule for ExhaustOpen {
    fn meta(&self) -> &'static RuleMeta {
        &EXHAUST_OPEN
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        // Every exhaust port, by its net.
        let mut by_net: BTreeMap<NetId, EndpointKey> = BTreeMap::new();
        let mut functions: Vec<Id<Function>> = q.functions().ids().collect();
        functions.sort();
        for f in functions {
            for cp in q.get::<FunctionCps>(f).iter() {
                if exhaust_role(&cp.port_role) {
                    let at = EndpointKey::Cp(f, cp.idx);
                    if let Some(net) = node_net(q, at) {
                        by_net.entry(net).or_insert(at);
                    }
                }
            }
        }
        // The gallery segments by the net of their first member: (terminal, gallery).
        let mut galleries: BTreeMap<NetId, (Id<Device>, FluidPortRole)> = BTreeMap::new();
        for vt in q.devices().ids().filter(|d| is_fluid_terminal(q, *d)) {
            for s in q.get::<Galleries>(vt).iter() {
                if let Some(net) = s.members.first().and_then(|m| node_net(q, *m)) {
                    galleries.insert(net, (vt, s.gallery.clone()));
                }
            }
        }
        let nets = q.get::<FluidNets>(());
        let zones = q.get::<MediumZones>(());
        let mut out = Vec::new();
        for (net, alone) in by_net {
            if nets.net_of(alone).is_some_and(|n| !n.connections.is_empty()) {
                continue; // a collector line
            }
            let ports = members(q, net, alone);
            let mut all_exhaust = true;
            let mut texts = Vec::new();
            let mut targets = Vec::new();
            let mut any_closed = false;
            for m in &ports {
                let EndpointKey::Cp(f, idx) = *m else { continue };
                let role = q
                    .get::<FunctionCps>(f)
                    .iter()
                    .find(|c| c.idx == idx)
                    .and_then(|c| c.port_role.clone());
                all_exhaust &= exhaust_role(&role);
                any_closed |= closed(q, f, idx);
                texts.push(cp_label(q, CpRef::new(f, idx)));
                targets.push(Target::new(Kind::Function, f));
            }
            let carries = zones.keys(net).any(|(k, _)| k.is_exhaust());
            if any_closed || !(all_exhaust || carries) {
                continue;
            }
            texts.sort_by_key(|s| natural_key(s));
            let finding = match galleries.get(&net) {
                Some((vt, g)) => {
                    let (vt_text, g) = (device_tag(q, *vt), gallery_number(g));
                    Finding::new(
                        &EXHAUST_OPEN,
                        targets,
                        format!("gallery {g}"),
                        full_tag(q, *vt, text!("word-device")),
                    )
                    .arg("form", "gallery")
                    .arg("terminal", vt_text)
                    .arg("gallery", g)
                    .arg("count", texts.len().to_string())
                }
                None => Finding::new(&EXHAUST_OPEN, targets, "ports", texts[0].clone())
                    .arg("form", "ports")
                    .arg("ports", and_listed(texts.iter().cloned())),
            };
            out.push(finding);
        }
        out
    }
}

/// The port number of a gallery ("1", "3", "5", "82", "84").
fn gallery_number(g: &FluidPortRole) -> &'static str {
    match g {
        FluidPortRole::Supply1 => "1",
        FluidPortRole::Exhaust3 => "3",
        FluidPortRole::Exhaust5 => "5",
        FluidPortRole::PilotExhaust82 => "82",
        FluidPortRole::PilotExhaust84 => "84",
        _ => "?",
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-009

meta!(
    COIL_VOLTAGE,
    "EW-FL-009",
    "rule-ew-fl-009",
    "120009",
    "Coil voltage",
    Conflict,
    "or",
    Error,
    Device
);

/// A voltage as the messages print it: "24 V", "24 V DC".
fn volts(v: Dec3, env: Option<&AcDc>) -> String {
    match env {
        Some(AcDc::Ac) => format!("{v} V AC"),
        Some(AcDc::Dc) => format!("{v} V DC"),
        _ => format!("{v} V"),
    }
}

/// The voltages of the potentials at the CPs of a function, with their AC or DC where the potential names it and their
/// CP texts.
fn voltages_at(q: &Q, cps: impl IntoIterator<Item = CpRef>) -> Vec<(String, Dec3, Option<AcDc>)> {
    let partition = q.get::<PotentialPartition>(());
    let mut out = Vec::new();
    for cp in cps {
        let Some(p) = partition.potential_at(EndpointKey::Cp(cp.function, cp.idx)) else {
            continue;
        };
        let attrs = q.get::<PotentialAttrsOf>(p.id);
        if let Some(v) = attrs.voltage.as_ref() {
            let env = attrs.env.as_ref().and_then(|e| e.value.ac_dc.clone());
            out.push((cp_label(q, cp), v.value, env));
        }
    }
    out
}

/// The load supply pins (`L+` of a supply group of kind `load`) of the first device of the module's parent chain whose
/// part has one.
fn load_supply(q: &Q, module: Id<Device>) -> Vec<CpRef> {
    let parts = q.parts();
    for d in parent_chain(q, module) {
        let Some(dev) = q.devices().get(d) else { continue };
        let pins: Vec<String> = dev
            .parts
            .iter()
            .filter_map(|u| parts.get(u.part.copy)?.item.sections.plc.clone())
            .flat_map(|plc| plc.supply_groups)
            .filter(|g| g.kind == SupplyKind::Load)
            .map(|g| g.cps.l_plus.as_str().to_owned())
            .collect();
        if pins.is_empty() {
            continue;
        }
        let mut functions: Vec<Id<Function>> = q.idx().functions_of_device(d).collect();
        functions.sort();
        return functions
            .into_iter()
            .flat_map(|f| {
                q.get::<FunctionCps>(f)
                    .iter()
                    .filter(|c| pins.contains(&c.designation))
                    .map(|c| CpRef::new(f, c.idx))
                    .collect::<Vec<_>>()
            })
            .collect();
    }
    Vec::new()
}

/// EW-FL-009 (FR-FLU-019): a valve whose part's coil voltage differs from the voltage of a potential that feeds a
/// coil, or whose coil's AC or DC differs from the potential's where both name one: at a physical coil, the potentials
/// at its CPs; at an internal coil, the potential at the load supply pin of the module of the channel bound to it (or
/// the first device above that module with one, the valve terminal's valve supply; doc 11 §6). One
/// finding per valve listing the feeding points; target the valve.
#[derive(Clone, Copy, Debug)]
pub struct CoilVoltage;

impl Rule for CoilVoltage {
    fn meta(&self) -> &'static RuleMeta {
        &COIL_VOLTAGE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(d) = *scope else {
            return Vec::new();
        };
        let Some(dev) = q.devices().get(d) else {
            return Vec::new();
        };
        let parts = q.parts();
        let Some((voltage, env, solenoids)) = dev.parts.iter().find_map(|u| {
            let item = &parts.get(u.part.copy)?.item;
            let coil = item.sections.fluid.as_ref()?.coil.as_ref()?;
            Some((
                coil.voltage?,
                coil.env.clone(),
                valve_solenoids(item).map(<[_]>::to_vec).unwrap_or_default(),
            ))
        }) else {
            return Vec::new();
        };
        let mut feeds: Vec<(String, Dec3, Option<AcDc>)> = Vec::new();
        let functions = q.functions();
        let fndefs = q.fndefs();
        let mut own: Vec<Id<Function>> = q.idx().functions_of_device(d).collect();
        own.sort();
        for f in own {
            let coil = functions
                .get(f)
                .and_then(|x| fndefs.get(x.def))
                .is_some_and(|x| x.item.contact_kind == ContactKind::Coil);
            if !coil {
                continue;
            }
            let physical: Vec<CpRef> = q
                .get::<FunctionCps>(f)
                .iter()
                .filter(|c| c.physical && !is_fluid(&c.discipline))
                .map(|c| CpRef::new(f, c.idx))
                .collect();
            feeds.extend(voltages_at(q, physical));
        }
        let bound = q.get::<ew_derive::ChannelsByCp>(());
        for s in &solenoids {
            let Some(cp) = solenoid_coil(q, d, s) else {
                continue;
            };
            for ch in bound.at_cp(cp.function, cp.idx) {
                if let Some(module) = q.table::<ew_schema::Channel>().get(ch).map(|c| c.module) {
                    feeds.extend(voltages_at(q, load_supply(q, module)));
                }
            }
        }
        // AC against DC is a mismatch only where the coil and the potential both name one.
        let known = |e: &Option<AcDc>| matches!(e, Some(AcDc::Ac | AcDc::Dc));
        let mut wrong: Vec<String> = feeds
            .into_iter()
            .filter(|(_, v, e)| *v != voltage || (known(e) && known(&env) && *e != env))
            .map(|(at, v, e)| format!("{at} ({})", volts(v, e.as_ref())))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if wrong.is_empty() {
            return Vec::new();
        }
        wrong.sort_by_key(|s| natural_key(s));
        let valve = device_tag(q, d);
        let takes = volts(voltage, env.as_ref());
        vec![
            Finding::new(
                &COIL_VOLTAGE,
                [Target::new(Kind::Device, d)],
                "voltage",
                full_tag(q, d, text!("word-device")),
            )
            .arg("valve", valve)
            .arg("voltage", takes)
            .arg("feeds", and_listed(wrong)),
        ]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-010

meta!(
    NO_SUPPLY,
    "EW-FL-010",
    "rule-ew-fl-010",
    "120010",
    "No supply",
    Completeness,
    "or",
    Warning,
    Device
);

/// Whether a net carries a medium (not exhaust) or reaches a node outside the devices of `inside`: a junction, or a
/// CP of another device.
fn supplied(q: &Q, net: NetId, alone: EndpointKey, inside: &dyn Fn(Id<Device>) -> bool) -> bool {
    if q.get::<MediumZones>(()).keys(net).any(|(k, _)| !k.is_exhaust()) {
        return true;
    }
    members(q, net, alone).iter().any(|m| match m {
        EndpointKey::Junction(_) => true,
        EndpointKey::Cp(f, _) => q
            .functions()
            .get(*f)
            .and_then(|x| x.device)
            .is_none_or(|d| !inside(d)),
    })
}

/// EW-FL-010 (FR-FLU-020): a valve terminal whose gallery-1 segment, or a valve outside a terminal whose port 1,
/// carries no medium and reaches no member outside the terminal (outside the valve); target the terminal or the
/// valve. A valve that is not fitted is skipped.
#[derive(Clone, Copy, Debug)]
pub struct NoSupply;

impl Rule for NoSupply {
    fn meta(&self) -> &'static RuleMeta {
        &NO_SUPPLY
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        let Scope::Device(d) = *scope else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if is_fluid_terminal(q, d) {
            let inside = |x: Id<Device>| parent_chain(q, x).contains(&d);
            for s in q
                .get::<Galleries>(d)
                .iter()
                .filter(|s| s.gallery == FluidPortRole::Supply1)
            {
                let Some(first) = s.members.first() else { continue };
                let Some(net) = node_net(q, *first) else { continue };
                if supplied(q, net, *first, &inside) {
                    continue;
                }
                let vt = device_tag(q, d);
                out.push(
                    Finding::new(
                        &NO_SUPPLY,
                        [Target::new(Kind::Device, d)],
                        format!("gallery {}", cp_text(q, *first)),
                        full_tag(q, d, text!("word-device")),
                    )
                    .arg("form", "gallery")
                    .arg("device", vt),
                );
            }
            return out;
        }
        if q.devices()
            .get(d)
            .is_none_or(|x| x.state == DeviceState::NotFitted)
            || parent_chain(q, d).iter().any(|a| is_fluid_terminal(q, *a))
        {
            return out;
        }
        let inside = |x: Id<Device>| x == d;
        for f in valve_functions(q, d) {
            for cp in q.get::<FunctionCps>(f).iter() {
                if cp.port_role != Some(FluidPortRole::Supply1) {
                    continue;
                }
                let at = EndpointKey::Cp(f, cp.idx);
                let Some(net) = node_net(q, at) else { continue };
                if supplied(q, net, at, &inside) {
                    continue;
                }
                let valve = device_tag(q, d);
                out.push(
                    Finding::new(
                        &NO_SUPPLY,
                        [Target::new(Kind::Function, f)],
                        format!("port {}", cp.designation),
                        full_tag(q, d, text!("word-device")),
                    )
                    .arg("form", "valve")
                    .arg("device", valve)
                    .arg("port", cp.designation.clone()),
                );
            }
        }
        out
    }
}

/// A node as a fingerprint discriminator names it.
fn cp_text(q: &Q, at: EndpointKey) -> String {
    CpRef::of(at).map(|cp| cp_label(q, cp)).unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-012

meta!(
    PORT_ROLE_CONFLICT,
    "EW-FL-012",
    "rule-ew-fl-012",
    "120012",
    "Port-role conflict",
    Conflict,
    "or",
    Warning,
    Project
);

/// EW-FL-012 (FR-FLU-032): in one fluid net, two ports of one valve function that are not all exhausts (two exhausts
/// of one valve may share a silencer); or a working port of a valve joined to an exhaust port of another device's
/// valve, or to a supply plate's exhaust tap (it is the valves' exhaust gallery), with no valve between them. A working
/// port into an inlet (port 1 of another valve, of a manifold, of a quick-exhaust, shuttle or service unit) is how a
/// circuit is built, and the ports of another component that is no valve (a silencer, a regulator's exhaust) never
/// conflict. Target the net's lines.
#[derive(Clone, Copy, Debug)]
pub struct PortRoleConflict;

impl Rule for PortRoleConflict {
    fn meta(&self) -> &'static RuleMeta {
        &PORT_ROLE_CONFLICT
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let functions = q.functions();
        let fndefs = q.fndefs();
        let mut out = Vec::new();
        for net in q.get::<FluidNets>(()).nets.iter() {
            if net.connections.is_empty() {
                continue; // a gallery alone joins ports by design
            }
            let targets: Vec<Target> = net
                .connections
                .iter()
                .map(|c| Target::new(Kind::Connection, *c))
                .collect();
            // Per valve function: its ports in the net, and whether every one is an exhaust.
            let mut by_valve: BTreeMap<Id<Function>, (Vec<String>, bool)> = BTreeMap::new();
            let mut working: Vec<(Option<Id<Device>>, String)> = Vec::new();
            let mut exhausts: Vec<(Option<Id<Device>>, String)> = Vec::new();
            for m in &net.members {
                let EndpointKey::Cp(f, idx) = *m else { continue };
                let Some(def) = functions.get(f).and_then(|x| fndefs.get(x.def)) else {
                    continue;
                };
                let Some(cp) = q.get::<FunctionCps>(f).iter().find(|c| c.idx == idx).cloned() else {
                    continue;
                };
                // A valve's ports, and a supply plate's taps, which are on the valves' sides.
                let side = ew_derive::ValvePort::of(&def.item, cp.port_role.as_ref());
                let valve = def.item.contact_kind == ContactKind::Valve;
                if !valve && side.is_none() {
                    continue;
                }
                let text = cp_label(q, CpRef::new(f, idx));
                let device = functions.get(f).and_then(|x| x.device);
                if valve {
                    let exhaust = exhaust_role(&cp.port_role);
                    let entry = by_valve.entry(f).or_insert((Vec::new(), true));
                    entry.0.push(text.clone());
                    entry.1 &= exhaust;
                }
                match side {
                    Some(ew_derive::ValvePort::Working) => working.push((device, text)),
                    Some(ew_derive::ValvePort::Exhaust) => exhausts.push((device, text)),
                    None => {}
                }
            }
            for (ports, _) in by_valve
                .values()
                .filter(|(p, all_exhaust)| p.len() > 1 && !all_exhaust)
            {
                let mut ports = ports.clone();
                ports.sort_by_key(|s| natural_key(s));
                out.push(
                    Finding::new(
                        &PORT_ROLE_CONFLICT,
                        targets.clone(),
                        format!("valve {}", ports[0]),
                        ports[0].clone(),
                    )
                    .arg("form", "valve")
                    .arg("ports", and_listed(ports.iter().cloned())),
                );
            }
            for (wd, w) in &working {
                for (fd, f) in &exhausts {
                    if wd == fd {
                        continue;
                    }
                    out.push(
                        Finding::new(
                            &PORT_ROLE_CONFLICT,
                            targets.clone(),
                            format!("role {w} {f}"),
                            w.clone(),
                        )
                        .arg("form", "role")
                        .arg("working", w.clone())
                        .arg("other", f.clone()),
                    );
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-014

meta!(
    LOCATION_CROSSING,
    "EW-FL-014",
    "rule-ew-fl-014",
    "120014",
    "Tube crosses a location",
    Completeness,
    "o",
    Info,
    Project
);

/// EW-FL-014 (FR-FLU-032): a tube whose two ends lie in different locations, with no transition device between them
/// (a bulkhead device ends the tube); target the tube. Off by default (`pre-delivery` and `migration` switch it off).
#[derive(Clone, Copy, Debug)]
pub struct LocationCrossing;

impl Rule for LocationCrossing {
    fn meta(&self) -> &'static RuleMeta {
        &LOCATION_CROSSING
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        q.get::<TubeRows>(())
            .iter()
            .filter(|r| r.location_change())
            .map(|r| {
                let (text, text_nl) = tube_text(r);
                let (from, to) = (r.from.location.clone(), r.to.location.clone());
                Finding::new(
                    &LOCATION_CROSSING,
                    [Target::new(Kind::Connection, r.id)],
                    "location",
                    text.clone(),
                )
                .arg("tube", text_nl)
                .arg("from", from)
                .arg("to", to)
            })
            .collect()
    }
}
