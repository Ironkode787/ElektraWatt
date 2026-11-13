// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Media propagation and effective tube properties (doc 11 §5.3, §5.4): `medium_zones()` and `tube_props_of(line)`.
//!
//! **Propagation** seeds every medium definition (`medium_def`) at its CP's fluid net and follows, to a fixpoint,
//! every path of every switching position of every section of a function's valve model (a `flow` path from its
//! `from` CP to its `to` CP only, an `open` path both ways). A key reaching a port with an exhaust role (3, 5, 82,
//! 84) becomes `Exhaust(root)`; a key reaching the outlet of a regulator part becomes `Regulated(root, p_set)`, the
//! vacuum port of an ejector part `Vacuum(root)`, both direct. The union over all positions describes what a port
//! *can* carry, which is what tube sizing and the pressure checks need; it never depends on a simulated state. A
//! net that no line or gallery joins is the CP alone. A definition whose CP or medium is gone seeds nothing.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::MlText;
use ew_model::EndpointKey;
use ew_schema::{
    Connection, ConnectionKind, Endpoint, FluidPortRole, FluidTransform, Function, LibCopy, LineKind, Medium,
    MediumDef, Part, TempRange, TubeMaterial, TubeSpec, ValvePath,
};
use ew_units::{Dec3, Nm};

use crate::nets::NetId;
use crate::query::{Policy, Query};
use crate::{FluidNets, FunctionCps, Q};

/// A medium as propagation carries it (doc 11 §2.5).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum MediumKey {
    /// A project medium, seeded by a definition point.
    Defined(Id<Medium>),
    /// The root medium at a regulator's set pressure.
    Regulated(Id<Medium>, Dec3),
    /// Vacuum from the root medium (an ejector's vacuum port).
    Vacuum(Id<Medium>),
    /// Exhaust of the root medium.
    Exhaust(Id<Medium>),
}

impl MediumKey {
    /// The project medium it comes from.
    pub fn root(&self) -> Id<Medium> {
        match self {
            MediumKey::Defined(m)
            | MediumKey::Regulated(m, _)
            | MediumKey::Vacuum(m)
            | MediumKey::Exhaust(m) => *m,
        }
    }

    /// Whether it is an exhaust key.
    pub fn is_exhaust(&self) -> bool {
        matches!(self, MediumKey::Exhaust(_))
    }

    /// The text it prints as: the medium's name; "compressed air 6 bar → 4 bar"; "vacuum (from compressed air 6
    /// bar)"; "exhaust", in en-GB and nl-NL where the derivation adds words.
    pub fn text(&self, q: &Q) -> MlText {
        let name = q
            .table::<Medium>()
            .get(self.root())
            .map(|m| m.name.clone())
            .unwrap_or_default();
        match self {
            MediumKey::Defined(_) => name,
            MediumKey::Regulated(_, p) => crate::map_ml(&name, |s| format!("{s} → {p} bar")),
            MediumKey::Vacuum(_) => two(
                &name,
                |s| format!("vacuum (from {s})"),
                |s| format!("vacuüm (uit {s})"),
            ),
            MediumKey::Exhaust(_) => MlText::from_langs([
                ("en-GB".parse().expect("tag"), "exhaust"),
                ("nl-NL".parse().expect("tag"), "afvoer"),
            ]),
        }
    }
}

/// A text in en-GB and nl-NL around a name, each from the name in that language (or its neutral text).
fn two(name: &MlText, en: impl Fn(&str) -> String, nl: impl Fn(&str) -> String) -> MlText {
    let in_lang = |tag: &str| {
        let tag = tag.parse().expect("a language tag");
        name.text_for(&tag)
            .map(str::to_owned)
            .unwrap_or_else(|| crate::neutral(name))
    };
    MlText::from_langs([
        ("en-GB".parse().expect("tag"), en(&in_lang("en-GB"))),
        ("nl-NL".parse().expect("tag"), nl(&in_lang("nl-NL"))),
    ])
}

/// How a key reached a net.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum MediumOrigin {
    /// Set here: a definition point or a transform's outlet.
    Direct,
    /// Carried here along a path.
    Reached,
}

/// The effective medium of a net (doc 11 §5.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Effective {
    /// No medium reaches it.
    None,
    /// The single non-exhaust key, else the exhaust key.
    One(MediumKey),
    /// Several keys reach it (behind a shuttle valve): allowed, shown as "mixed".
    Mixed,
}

/// The value of [`MediumZones`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct MediumZonesV {
    /// The keys of each net that carries one, by net ID.
    pub media: BTreeMap<NetId, BTreeSet<(MediumKey, MediumOrigin)>>,
    /// A node of each net of `media`: the first through which a key reached it.
    pub at: BTreeMap<NetId, EndpointKey>,
}

impl MediumZonesV {
    /// The keys of a net.
    pub fn keys(&self, net: NetId) -> impl Iterator<Item = &(MediumKey, MediumOrigin)> {
        self.media.get(&net).into_iter().flatten()
    }

    /// The effective medium of a net.
    pub fn effective(&self, net: NetId) -> Effective {
        let keys: BTreeSet<&MediumKey> = self.keys(net).map(|(k, _)| k).collect();
        let live: Vec<&MediumKey> = keys.iter().copied().filter(|k| !k.is_exhaust()).collect();
        match (live.as_slice(), keys.len()) {
            ([one], _) => Effective::One((*one).clone()),
            ([], 0) => Effective::None,
            ([], 1) => Effective::One((*keys.iter().next().expect("one key")).clone()),
            _ => Effective::Mixed,
        }
    }
}

/// The net of a CP: its fluid net's ID, else the CP alone.
pub fn node_net(q: &Q, at: EndpointKey) -> Option<NetId> {
    let nets = q.get::<FluidNets>(());
    nets.net_of(at).map(|n| n.id).or_else(|| NetId::of([at]))
}

/// The members of a net: the fluid net's members, else the node alone.
fn members_of(q: &Q, net: NetId, alone: &BTreeMap<NetId, EndpointKey>) -> Vec<EndpointKey> {
    let nets = q.get::<FluidNets>(());
    match nets.nets.binary_search_by(|n| n.id.cmp(&net)) {
        Ok(i) => nets.nets[i].members.clone(),
        Err(_) => alone.get(&net).copied().into_iter().collect(),
    }
}

/// The transform of the part of a function's device (the first part use whose copy has one).
fn transform_of(q: &Q, f: Id<Function>) -> Option<FluidTransform> {
    let device = q.functions().get(f)?.device?;
    let dev = q.devices().get(device)?;
    let parts = q.parts();
    dev.parts.iter().find_map(|u| {
        parts
            .get(u.part.copy)?
            .item
            .sections
            .fluid
            .as_ref()?
            .transform
            .clone()
    })
}

/// The CPs a key at CP `idx` of function `f` moves to along the paths of its FnDef's valve model, in CP order.
fn paths_from(q: &Q, f: Id<Function>, idx: u16) -> Vec<u16> {
    let Some(def) = q.functions().get(f).and_then(|x| q.fndefs().get(x.def)) else {
        return Vec::new();
    };
    let Some(model) = def.item.valve_model.as_ref() else {
        return Vec::new();
    };
    let mut out = BTreeSet::new();
    for section in &model.sections {
        for pos in &section.positions {
            for p in &pos.paths {
                match p {
                    ValvePath::Flow { from, to } if *from == idx => {
                        out.insert(*to);
                    }
                    ValvePath::Open { a, b } if *a == idx => {
                        out.insert(*b);
                    }
                    ValvePath::Open { a, b } if *b == idx => {
                        out.insert(*a);
                    }
                    _ => {}
                }
            }
        }
    }
    out.into_iter().collect()
}

/// `medium_zones()` (doc 11 §5.3, FR-FLU-009): the media every fluid net can carry. Deterministic: definitions in ID
/// order, a queue in net-ID order. Policy Memo.
#[derive(Debug)]
pub struct MediumZones;

impl Query for MediumZones {
    type Key = ();
    type Value = MediumZonesV;
    const FAMILY: &'static str = "medium_zones";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _: &()) -> Arc<MediumZonesV> {
        let mut media: BTreeMap<NetId, BTreeSet<(MediumKey, MediumOrigin)>> = BTreeMap::new();
        let mut alone: BTreeMap<NetId, EndpointKey> = BTreeMap::new();
        let mediums = q.table::<Medium>();
        let mut defs: Vec<(Id<MediumDef>, Endpoint, Id<Medium>)> = q
            .table::<MediumDef>()
            .iter()
            .map(|(id, d)| (id, d.at.clone(), d.medium))
            .collect();
        defs.sort_by_key(|d| d.0);
        let mut queue: BTreeSet<NetId> = BTreeSet::new();
        for (_, at, m) in defs {
            let Some(at) = EndpointKey::of(&at) else { continue };
            if !mediums.contains(m) || !exists(q, at) {
                continue;
            }
            let Some(net) = node_net(q, at) else { continue };
            alone.entry(net).or_insert(at);
            if media
                .entry(net)
                .or_default()
                .insert((MediumKey::Defined(m), MediumOrigin::Direct))
            {
                queue.insert(net);
            }
        }
        let mut queue: VecDeque<NetId> = queue.into_iter().collect();
        while let Some(n) = queue.pop_front() {
            let keys: Vec<MediumKey> = media
                .get(&n)
                .map(|s| {
                    s.iter()
                        .map(|(k, _)| k.clone())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect()
                })
                .unwrap_or_default();
            for member in members_of(q, n, &alone) {
                let EndpointKey::Cp(f, idx) = member else { continue };
                let targets = paths_from(q, f, idx);
                if targets.is_empty() {
                    continue;
                }
                let cps = q.get::<FunctionCps>(f);
                let transform = transform_of(q, f);
                for to in targets {
                    let Some(cp) = cps.iter().find(|c| c.idx == to) else {
                        continue;
                    };
                    let at = EndpointKey::Cp(f, to);
                    let Some(net) = node_net(q, at) else { continue };
                    alone.entry(net).or_insert(at);
                    for k in &keys {
                        let root = k.root();
                        let (key, origin) = match &transform {
                            Some(FluidTransform::Regulator { outlet, p_set })
                                if outlet.as_str() == cp.designation =>
                            {
                                (MediumKey::Regulated(root, *p_set), MediumOrigin::Direct)
                            }
                            Some(FluidTransform::Ejector { vacuum }) if vacuum.as_str() == cp.designation => {
                                (MediumKey::Vacuum(root), MediumOrigin::Direct)
                            }
                            _ if matches!(
                                cp.port_role,
                                Some(
                                    FluidPortRole::Exhaust3
                                        | FluidPortRole::Exhaust5
                                        | FluidPortRole::PilotExhaust82
                                        | FluidPortRole::PilotExhaust84
                                )
                            ) =>
                            {
                                (MediumKey::Exhaust(root), MediumOrigin::Reached)
                            }
                            _ => (k.clone(), MediumOrigin::Reached),
                        };
                        if media.entry(net).or_default().insert((key, origin)) && !queue.contains(&net) {
                            queue.push_back(net);
                        }
                    }
                }
            }
        }
        alone.retain(|n, _| media.contains_key(n));
        Arc::new(MediumZonesV { media, at: alone })
    }
}

/// Whether an endpoint names a live CP or junction.
fn exists(q: &Q, at: EndpointKey) -> bool {
    match at {
        EndpointKey::Cp(f, idx) => q.get::<FunctionCps>(f).iter().any(|c| c.idx == idx),
        EndpointKey::Junction(j) => q.junctions().contains(j),
    }
}

/// Where one value of an effective tube specification comes from (doc 11 §5.4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TubeSource {
    /// Level 1: the project's default tube.
    Project,
    /// Level 2: the medium of the line's net.
    Medium(Id<Medium>),
    /// Level 3: the effective tube part's data.
    Part(Id<Part>),
    /// Level 4: the line's own override.
    Override,
}

/// One value of an effective tube specification with its source.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Sourced<T> {
    /// The value.
    pub value: T,
    /// Where it comes from.
    pub source: TubeSource,
}

/// The effective tube specification of a line (doc 11 §5.4, FR-FLU-010): each value from the highest level that
/// sets it, with its source; `None` where no level sets it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TubeProps {
    /// The outer diameter.
    pub od: Option<Sourced<Nm>>,
    /// The inner diameter.
    pub id: Option<Sourced<Nm>>,
    /// The material.
    pub material: Option<Sourced<TubeMaterial>>,
    /// The colour.
    pub colour: Option<Sourced<ew_schema::ColourSpec>>,
    /// The tube part.
    pub part: Option<Sourced<Id<Part>>>,
    /// The pressure rating (bar).
    pub p_rating: Option<Sourced<Dec3>>,
    /// The temperature range.
    pub t_range: Option<Sourced<TempRange>>,
    /// The effective medium of the line's net.
    pub medium: Option<MediumKey>,
    /// Whether several media reach the line's net.
    pub mixed: bool,
}

impl TubeProps {
    /// Whether the specification is complete enough to cut a tube: outer and inner diameter and material (EW-FL-007).
    pub fn is_specified(&self) -> bool {
        self.od.is_some() && self.id.is_some() && self.material.is_some()
    }

    /// "6×4" in millimetres; empty without both diameters.
    pub fn size_text(&self) -> String {
        match (&self.od, &self.id) {
            (Some(od), Some(id)) => format!("{}×{}", mm(od.value), mm(id.value)),
            _ => String::new(),
        }
    }
}

/// A length in millimetres as text: whole millimetres plain, else three decimals at most.
pub fn mm(nm: Nm) -> String {
    Dec3(nm.0 / 1_000).to_string()
}

/// The default tube of a medium for a line kind: its `line_kind_tubes` entry, else its default tube.
fn medium_tube(m: &Medium, kind: &LineKind) -> TubeSpec {
    m.line_kind_tubes
        .iter()
        .find(|t| &t.line_kind == kind)
        .map(|t| t.tube.clone())
        .unwrap_or_else(|| m.default_tube.clone())
}

/// `tube_props_of(line)` (doc 11 §5.4): the effective tube specification of a fluid line; the default for a line that
/// is no fluid line. Policy Memo.
#[derive(Debug)]
pub struct TubePropsOf;

impl Query for TubePropsOf {
    type Key = Id<Connection>;
    type Value = TubeProps;
    const FAMILY: &'static str = "tube_props_of";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Id<Connection>) -> Arc<TubeProps> {
        let Some(c) = q.connections().get(*key) else {
            return Arc::new(TubeProps::default());
        };
        let ConnectionKind::Fluid(line) = &c.kind else {
            return Arc::new(TubeProps::default());
        };
        let mut out = TubeProps::default();
        let net = EndpointKey::of(&c.a).and_then(|a| node_net(q, a));
        let medium = match net.map(|n| q.get::<MediumZones>(()).effective(n)) {
            Some(Effective::One(k)) => Some(k),
            Some(Effective::Mixed) => {
                out.mixed = true;
                None
            }
            _ => None,
        };
        out.medium = medium.clone();
        let project = q.project().settings.fluid.default_tube.clone();
        let level2 = medium
            .as_ref()
            .and_then(|k| q.table::<Medium>().get(k.root()).map(|m| (k.root(), m.clone())));
        // Level by level, lowest first; each later level replaces what it sets.
        let set = |spec: &TubeSpec, source: TubeSource, out: &mut TubeProps| {
            if let Some(v) = spec.od {
                out.od = Some(Sourced {
                    value: v,
                    source: source.clone(),
                });
            }
            if let Some(v) = spec.id {
                out.id = Some(Sourced {
                    value: v,
                    source: source.clone(),
                });
            }
            if let Some(v) = &spec.material {
                out.material = Some(Sourced {
                    value: v.clone(),
                    source: source.clone(),
                });
            }
            if let Some(v) = &spec.colour {
                out.colour = Some(Sourced {
                    value: v.clone(),
                    source: source.clone(),
                });
            }
            if let Some(v) = &spec.part {
                out.part = Some(Sourced {
                    value: v.copy,
                    source: source.clone(),
                });
            }
            if let Some(v) = spec.p_rating {
                out.p_rating = Some(Sourced {
                    value: v,
                    source: source.clone(),
                });
            }
            if let Some(v) = &spec.t_range {
                out.t_range = Some(Sourced {
                    value: v.clone(),
                    source,
                });
            }
        };
        set(&project, TubeSource::Project, &mut out);
        if let Some((id, m)) = &level2 {
            if let Some(colour) = &m.colour {
                out.colour = Some(Sourced {
                    value: colour.clone(),
                    source: TubeSource::Medium(*id),
                });
            }
            set(
                &medium_tube(m, &line.line_kind),
                TubeSource::Medium(*id),
                &mut out,
            );
        }
        // Level 3: the override's part, else the part the lower levels give.
        let part = line
            .tube
            .part
            .as_ref()
            .map(|p| p.copy)
            .or(out.part.as_ref().map(|p| p.value));
        if let Some(data) = part.and_then(|p| {
            q.table::<LibCopy<Part>>()
                .get(p)
                .and_then(|c| c.item.sections.fluid.as_ref()?.tube.clone())
                .map(|t| (p, t))
        }) {
            let (p, t) = data;
            let spec = TubeSpec {
                od: t.od,
                id: t.id,
                material: t.material,
                colour: t.colour,
                part: None,
                p_rating: t.p_rating,
                t_range: t.t_range,
                extra: Default::default(),
            };
            set(&spec, TubeSource::Part(p), &mut out);
        }
        set(&line.tube, TubeSource::Override, &mut out);
        Arc::new(out)
    }
}
