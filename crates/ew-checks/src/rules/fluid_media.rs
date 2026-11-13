// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The EW-FL pack, its 1.0 rules (doc 11 §6, doc 15 §4.5.3): EW-FL-005 … -010, -012 and -014. They read the
//! media propagation, the effective tube properties and the physical tubes of `ew-derive` (doc 11 §5), and run only
//! in a scheme with the 1.0 content (`Scheme::with_content`).
//!
//! Texts name a tube by its line tag, else by its two ends (`-201U3-Q1:2 → -203Q1:A`), a port by `DT:port` and a
//! medium by its name; lists are in natural order. IDs enter only targets and fingerprints.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{MlText, natural_key};
use ew_derive::ew_schema::{
    ContactKind, CpOverride, Device, DeviceState, FluidPortRole, Function, Kind, Medium, SupplyKind,
};
use ew_derive::ew_units::Dec3;
use ew_derive::{
    CpRef, EndpointKey, FluidNets, FunctionCps, Galleries, MediumKey, MediumOrigin, MediumZones, NetId,
    PotentialAttrsOf, PotentialPartition, Q, TubeRow, TubeRows, cp_label, is_fluid, is_valve_terminal,
    node_net, parent_chain, solenoid_coil, valve_functions, valve_solenoids,
};

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::term::full_tag;

/// A device's displayed tag (`-201U3-Q1`), else `device`.
fn tag(q: &Q, d: Id<Device>) -> String {
    let t = ew_derive::short_tag(q, d);
    if t.is_empty() { "device".to_owned() } else { t }
}

/// A tube as the messages name it: its line tag, else its two ends.
fn tube_text(r: &TubeRow) -> String {
    if r.tag.is_empty() {
        format!("{} → {}", r.from.text, r.to.text)
    } else {
        r.tag.clone()
    }
}

/// The en-GB and nl-NL text of a multi-language text.
fn en_nl(t: &MlText) -> (String, String) {
    let get = |tag: &str| {
        let tag = tag.parse().expect("a language tag");
        t.text_for(&tag)
            .map(str::to_owned)
            .unwrap_or_else(|| ew_derive::neutral(t))
    };
    (get("en-GB"), get("nl-NL"))
}

/// "A, B and C".
fn and_list(items: &[String], and: &str) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} {and} {last}", init.join(", ")),
    }
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
    ($name:ident, $id:literal, $page:literal, $code:literal, $title:literal, $kind:ident, $modes:literal, $sev:ident, $scope:ident) => {
        #[doc = concat!($id, " \"", $title, "\" (doc 11 §6): 1.0.")]
        pub const $name: RuleMeta = RuleMeta {
            id: $id,
            code: $code,
            pack: "EW-FL",
            title: $title,
            kind: RuleKind::$kind,
            modes: Modes::letters($modes),
            severity: Severity::$sev,
            release_severity: None,
            scope: ScopeKind::$scope,
            supersedes: &[],
            release: Release::V1_0,
            docs: concat!("reference/rules/", $page),
        };
    };
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-005

meta!(
    TWO_MEDIA,
    "EW-FL-005",
    "ew-fl-005",
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
                    .table::<ew_derive::ew_schema::MediumDef>()
                    .iter()
                    .filter(|(_, d)| EndpointKey::of(&d.at).and_then(|a| node_net(q, a)) == Some(*net))
                    .map(|(id, _)| Target::new(Kind::MediumDef, id))
                    .collect();
            }
            let at = first
                .map(|cp| cp_label(q, cp))
                .unwrap_or_else(|| "a fluid net".to_owned());
            let (mut en, mut nl): (Vec<String>, Vec<String>) =
                direct.iter().map(|k| en_nl(&k.text(q))).unzip();
            en.sort_by_key(|s| natural_key(s));
            nl.sort_by_key(|s| natural_key(s));
            out.push(
                Finding::new(
                    &TWO_MEDIA,
                    targets,
                    "media",
                    at.clone(),
                    format!(
                        "the fluid net at {at} carries {} directly; keep one medium per net",
                        and_list(&en, "and")
                    ),
                )
                .arg("at", at)
                .arg("media", and_list(&en, "and"))
                .arg("media_nl", and_list(&nl, "en")),
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
    "ew-fl-006",
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
pub fn push_in_od(q: &Q, f: Id<Function>, port: &str) -> Option<ew_derive::ew_units::Nm> {
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
            let text = tube_text(r);
            if let (Some(rating), Some(p)) = (&r.props.p_rating, r.net.and_then(|n| highest_pressure(q, n)))
                && rating.value < p
            {
                out.push(
                    Finding::new(
                        &TUBE_MISFIT,
                        [Target::new(Kind::Connection, r.id)],
                        "rating",
                        text.clone(),
                        format!(
                            "tube {text} is rated {} bar, below the {p} bar its net carries",
                            rating.value
                        ),
                    )
                    .arg("form", "rating")
                    .arg("tube", text.clone())
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
                            format!(
                                "tube {text} is {} mm outside, but port {} takes {} mm",
                                ew_derive::mm(od.value),
                                end.text,
                                ew_derive::mm(port_od)
                            ),
                        )
                        .arg("form", "od")
                        .arg("tube", text.clone())
                        .arg("od", ew_derive::mm(od.value))
                        .arg("port", end.text.clone())
                        .arg("port_od", ew_derive::mm(port_od)),
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
    "ew-fl-007",
    "120007",
    "Line without tube specification",
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
                let text = tube_text(r);
                Finding::new(
                    &NO_TUBE_SPEC,
                    [Target::new(Kind::Connection, r.id)],
                    "spec",
                    text.clone(),
                    format!("tube {text} has no tube specification; give the medium or the project a default tube, or set it on the line"),
                )
                .arg("tube", text)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-008

meta!(
    EXHAUST_OPEN,
    "EW-FL-008",
    "ew-fl-008",
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
        for vt in q.devices().ids().filter(|d| is_valve_terminal(q, *d)) {
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
                    let (vt_text, g) = (tag(q, *vt), gallery_number(g));
                    Finding::new(
                        &EXHAUST_OPEN,
                        targets,
                        format!("gallery {g}"),
                        full_tag(q, *vt, "device"),
                        format!(
                            "{vt_text}: gallery {g} ({} exhaust ports) has no silencer, collector line or closure",
                            texts.len()
                        ),
                    )
                    .arg("form", "gallery")
                    .arg("terminal", vt_text)
                    .arg("gallery", g)
                    .arg("count", texts.len().to_string())
                }
                None => {
                    let ports = and_list(&texts, "and");
                    Finding::new(
                        &EXHAUST_OPEN,
                        targets,
                        "ports",
                        texts[0].clone(),
                        format!("exhaust {ports} has no silencer, collector line or closure"),
                    )
                    .arg("form", "ports")
                    .arg("ports", ports)
                    .arg("ports_nl", and_list(&texts, "en"))
                }
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
    "ew-fl-009",
    "120009",
    "Coil voltage",
    Conflict,
    "or",
    Error,
    Device
);

/// The voltages of the potentials at the CPs of a function, with their CP texts.
fn voltages_at(q: &Q, cps: impl IntoIterator<Item = CpRef>) -> Vec<(String, Dec3)> {
    let partition = q.get::<PotentialPartition>(());
    let mut out = Vec::new();
    for cp in cps {
        let Some(p) = partition.potential_at(EndpointKey::Cp(cp.function, cp.idx)) else {
            continue;
        };
        if let Some(v) = q.get::<PotentialAttrsOf>(p.id).voltage.as_ref() {
            out.push((cp_label(q, cp), v.value));
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
/// coil: at a physical coil, the potentials at its CPs; at an internal coil, the potential at the load supply pin of
/// the module of the channel bound to it (or the first device above that module with one, the valve terminal's
/// valve supply; doc 11 Open question 11). One finding per valve listing the feeding points; target the valve.
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
        let Some((voltage, solenoids)) = dev.parts.iter().find_map(|u| {
            let item = &parts.get(u.part.copy)?.item;
            let v = item.sections.fluid.as_ref()?.coil.as_ref()?.voltage?;
            Some((v, valve_solenoids(item).map(<[_]>::to_vec).unwrap_or_default()))
        }) else {
            return Vec::new();
        };
        let mut feeds: Vec<(String, Dec3)> = Vec::new();
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
                if let Some(module) = q
                    .table::<ew_derive::ew_schema::Channel>()
                    .get(ch)
                    .map(|c| c.module)
                {
                    feeds.extend(voltages_at(q, load_supply(q, module)));
                }
            }
        }
        let mut wrong: Vec<String> = feeds
            .into_iter()
            .filter(|(_, v)| *v != voltage)
            .map(|(at, v)| format!("{at} ({v} V)"))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if wrong.is_empty() {
            return Vec::new();
        }
        wrong.sort_by_key(|s| natural_key(s));
        let valve = tag(q, d);
        vec![
            Finding::new(
                &COIL_VOLTAGE,
                [Target::new(Kind::Device, d)],
                "voltage",
                full_tag(q, d, "device"),
                format!(
                    "{valve}: the coil takes {voltage} V, but {} feed it",
                    and_list(&wrong, "and")
                ),
            )
            .arg("valve", valve)
            .arg("voltage", voltage.to_string())
            .arg("feeds", and_list(&wrong, "and"))
            .arg("feeds_nl", and_list(&wrong, "en")),
        ]
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-FL-010

meta!(
    NO_SUPPLY,
    "EW-FL-010",
    "ew-fl-010",
    "120010",
    "No supply on port 1",
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
        if is_valve_terminal(q, d) {
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
                let vt = tag(q, d);
                out.push(
                    Finding::new(
                        &NO_SUPPLY,
                        [Target::new(Kind::Device, d)],
                        format!("gallery {}", cp_text(q, *first)),
                        full_tag(q, d, "device"),
                        format!("{vt}: gallery 1 has no supply; define a medium at a supply port or draw the supply line"),
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
            || parent_chain(q, d).iter().any(|a| is_valve_terminal(q, *a))
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
                let valve = tag(q, d);
                out.push(
                    Finding::new(
                        &NO_SUPPLY,
                        [Target::new(Kind::Function, f)],
                        format!("port {}", cp.designation),
                        full_tag(q, d, "device"),
                        format!(
                            "{valve}: port {} has no supply; define a medium at it or draw the supply line",
                            cp.designation
                        ),
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
    "ew-fl-012",
    "120012",
    "Port-role conflict",
    Conflict,
    "or",
    Warning,
    Project
);

/// EW-FL-012 (FR-FLU-032): in one fluid net, two ports of one valve function; or a working port of a valve joined to
/// an exhaust or supply port of another device with no valve between them. Target the net's lines.
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
        let is_valve = |f: Id<Function>| {
            functions
                .get(f)
                .and_then(|x| fndefs.get(x.def))
                .is_some_and(|x| x.item.contact_kind == ContactKind::Valve)
        };
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
            let mut by_valve: BTreeMap<Id<Function>, Vec<String>> = BTreeMap::new();
            let mut working: Vec<(Option<Id<Device>>, String)> = Vec::new();
            let mut feeds: Vec<(Option<Id<Device>>, String)> = Vec::new();
            for m in &net.members {
                let EndpointKey::Cp(f, idx) = *m else { continue };
                let Some(cp) = q.get::<FunctionCps>(f).iter().find(|c| c.idx == idx).cloned() else {
                    continue;
                };
                let text = cp_label(q, CpRef::new(f, idx));
                let device = functions.get(f).and_then(|x| x.device);
                if is_valve(f) {
                    by_valve.entry(f).or_default().push(text.clone());
                    if matches!(
                        cp.port_role,
                        Some(FluidPortRole::Working2 | FluidPortRole::Working4)
                    ) {
                        working.push((device, text.clone()));
                    }
                }
                if matches!(
                    cp.port_role,
                    Some(FluidPortRole::Supply1 | FluidPortRole::Exhaust3 | FluidPortRole::Exhaust5)
                ) {
                    feeds.push((device, text));
                }
            }
            for ports in by_valve.values().filter(|p| p.len() > 1) {
                let mut ports = ports.clone();
                ports.sort_by_key(|s| natural_key(s));
                out.push(
                    Finding::new(
                        &PORT_ROLE_CONFLICT,
                        targets.clone(),
                        format!("valve {}", ports[0]),
                        ports[0].clone(),
                        format!(
                            "{} are ports of one valve and are joined",
                            and_list(&ports, "and")
                        ),
                    )
                    .arg("form", "valve")
                    .arg("ports", and_list(&ports, "and"))
                    .arg("ports_nl", and_list(&ports, "en")),
                );
            }
            for (wd, w) in &working {
                for (fd, f) in &feeds {
                    if wd == fd {
                        continue;
                    }
                    out.push(
                        Finding::new(
                            &PORT_ROLE_CONFLICT,
                            targets.clone(),
                            format!("role {w} {f}"),
                            w.clone(),
                            format!("working port {w} is joined to {f} with no valve between them"),
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
    "ew-fl-014",
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
                let text = tube_text(r);
                let (from, to) = (r.from.location.clone(), r.to.location.clone());
                Finding::new(
                    &LOCATION_CROSSING,
                    [Target::new(Kind::Connection, r.id)],
                    "location",
                    text.clone(),
                    format!("tube {text} crosses from {from} to {to} without a bulkhead"),
                )
                .arg("tube", text)
                .arg("from", from)
                .arg("to", to)
            })
            .collect()
    }
}
