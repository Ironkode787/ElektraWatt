// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cable glands of a cabinet (doc 12 §3.9): every cable that leaves the cabinet's location gets one gland, the part
//! its Properties chose or the first gland of the workshop stock whose clamping range holds the cable's outer
//! diameter; placed, it is a `SurfaceFeature` of the cable. [`cable_outer_diameter`] is the one reader of a cable's
//! outer diameter.

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{
    Device, GlandSection, LayoutSpace, LibCopy, Part, PartKind, PartRef, StructureNode, SurfaceFeature,
    TechVal, Unit,
};
use ew_units::Nm;

use crate::Q;
use crate::query::{Policy, Query};

/// A cable's outer diameter, the one reader of it (doc 12 §3.9): its part's `sections.cable.outer_diameter`, else
/// the technical value `outer_diameter` in millimetres a part of an older build holds.
pub fn cable_outer_diameter(part: &Part) -> Option<Nm> {
    if let Some(d) = part.sections.cable.as_ref().and_then(|c| c.outer_diameter) {
        return Some(d);
    }
    part.tech_data
        .iter()
        .find(|t| t.key.as_str() == "outer_diameter")
        .and_then(|t| match &t.value {
            TechVal::Quantity(q) if q.unit == Unit::Mm => q.value.0.checked_mul(1_000).map(Nm),
            _ => None,
        })
}

/// A gland's nut across its flats: its own, else 1.4 × its thread (doc 12 §3.9).
pub fn gland_nut(g: &GlandSection) -> Nm {
    g.nut
        .unwrap_or_else(|| Nm(i64::from(g.thread.mm().unwrap_or(0)) * 1_400_000))
}

/// The gap between two glands in a row as the workshop keeps it: `nut_a / 2 + nut_b / 2 + web` ([`gland_nut`]).
pub fn gland_pitch(a: &GlandSection, b: &GlandSection, web: Nm) -> Nm {
    Nm(gland_nut(a).0 / 2 + gland_nut(b).0 / 2 + web.0)
}

/// Why a cable has no gland the program can propose.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum NoGland {
    /// Its part gives no outer diameter (or it has no cable part).
    NoDiameter,
    /// No gland of the workshop stock seals it.
    NoneSeals {
        /// Its outer diameter.
        od: Nm,
    },
}

/// The gland of one cable that leaves a cabinet.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CableGland {
    /// The cable.
    pub cable: Id<Device>,
    /// Its gland part: the chosen one, else the proposal; why there is none.
    pub part: Result<PartRef, NoGland>,
    /// The part is the cable's own choice (`Device.gland`).
    pub chosen: bool,
    /// Its outer diameter, where its part gives one.
    pub od: Option<Nm>,
    /// The feature that is its hole, once placed.
    pub placed: Option<Id<SurfaceFeature>>,
}

/// The gland section of a part copy.
pub fn gland_section(q: &Q, part: &PartRef) -> Option<GlandSection> {
    q.table::<LibCopy<Part>>()
        .get(part.copy)
        .and_then(|c| c.item.sections.gland.clone())
}

/// `cable_glands(space)` (memo): every cable with an end in the space's location and an end elsewhere (another
/// location, or a device without one), in natural order of its tag, with its gland in this cabinet (doc 12 §3.9): a
/// cable between two cabinets has one in each, and `placed` is the hole made for it on a surface of this space. A cable
/// that runs inside the cabinet only, and a field wire in no cable, get none.
#[derive(Debug)]
pub struct CableGlands;

impl Query for CableGlands {
    type Key = Id<LayoutSpace>;
    type Value = Vec<CableGland>;
    const FAMILY: &'static str = "cable_glands";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<CableGland>> {
        Arc::new(cable_glands(q, *key))
    }
}

fn cable_glands(q: &Q, space: Id<LayoutSpace>) -> Vec<CableGland> {
    let Some(location) = q.table::<LayoutSpace>().get(space).map(|s| s.location) else {
        return Vec::new();
    };
    let records = q.get::<crate::WireRecordSet>(());
    let mut out = Vec::new();
    let mut cables: Vec<(ew_mltext::NaturalKey, Id<Device>)> = q
        .devices()
        .iter()
        .filter(|(_, d)| d.aspects.cable.is_some())
        .map(|(id, _)| (ew_mltext::natural_key(&crate::short_tag(q, id)), id))
        .collect();
    cables.sort();
    let stock = &q.project().settings.workshop.stock;
    let features = q.table::<SurfaceFeature>();
    let surfaces = q.table::<ew_schema::MountingSurface>();
    let enclosures = q.table::<ew_schema::Enclosure>();
    let in_space = |s: Id<ew_schema::MountingSurface>| {
        surfaces
            .get(s)
            .and_then(|s| enclosures.get(s.enclosure))
            .is_some_and(|e| e.space == space)
    };
    for (_, cable) in cables {
        let cores: BTreeSet<_> = q.idx().conductors_of_cable(cable).collect();
        let ends: BTreeSet<Option<Id<StructureNode>>> = records
            .iter()
            .filter(|r| r.wire.core.is_some_and(|c| cores.contains(&c)))
            .flat_map(|r| r.wire.ends.iter().map(|e| e.cp.function))
            // An end at an external point (a function of no device) lies outside every cabinet.
            .map(|f| q.functions().get(f).and_then(|f| f.device))
            // A terminal's end is its strip's, as the cable overview names it.
            .map(|d| {
                d.map(|d| {
                    q.devices()
                        .get(d)
                        .and_then(|x| x.aspects.terminal.as_ref().map(|t| t.strip))
                        .unwrap_or(d)
                })
            })
            .map(|d| d.and_then(|d| super::queries::location_of(q, d)))
            .collect();
        if !ends.contains(&Some(location)) || ends.iter().all(|e| *e == Some(location)) {
            continue;
        }
        let Some(device) = q.devices().get(cable) else {
            continue;
        };
        let od = cable_part(q, device).and_then(|p| cable_outer_diameter(&p.item));
        let (part, chosen) = match &device.gland {
            Some(g) => (Ok(g.clone()), true),
            None => (propose(q, stock, od), false),
        };
        // Its gland in this cabinet: a hole made for it on a surface of this space. A cable that leaves two cabinets
        // has a gland in each, and placing one says nothing about the other.
        let placed = features
            .iter()
            .filter(|(_, f)| f.cable == Some(cable) && in_space(f.surface))
            .map(|(id, _)| id)
            .min();
        out.push(CableGland {
            cable,
            part,
            chosen,
            od,
            placed,
        });
    }
    out
}

/// The cable's part copy: its first part use of kind cable.
fn cable_part<'q>(q: &'q Q, device: &Device) -> Option<&'q LibCopy<Part>> {
    let copies = q.table::<LibCopy<Part>>();
    cable_part_use(q, device)
        .and_then(|p| copies.get(p.copy))
        .map(|c| &**c)
}

/// A cable's part use of kind cable: the part whose outer diameter its gland is proposed from, and where a person
/// enters that diameter (the part editor's Cable section); `None` without one.
pub fn cable_part_use(q: &Q, device: &Device) -> Option<PartRef> {
    let copies = q.table::<LibCopy<Part>>();
    device
        .parts
        .iter()
        .find(|u| copies.get(u.part.copy).is_some_and(|c| c.item.kind == PartKind::Cable))
        .map(|u| u.part.clone())
}

/// The first gland of the stock whose clamping range holds `od`.
fn propose(q: &Q, stock: &[PartRef], od: Option<Nm>) -> Result<PartRef, NoGland> {
    let od = od.ok_or(NoGland::NoDiameter)?;
    stock
        .iter()
        .find(|p| gland_section(q, p).is_some_and(|g| g.clamp_min <= od && od <= g.clamp_max))
        .cloned()
        .ok_or(NoGland::NoneSeals { od })
}
