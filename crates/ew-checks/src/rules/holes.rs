// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rules of a surface's holes (doc 12-3d §10.5, doc 15): EW-LAY-012 Holes too close and EW-LAY-017 Holes not
//! known, both over `ew_derive::layout::SurfaceFeatures`.

use std::collections::BTreeMap;

use ew_derive::Q;
use ew_derive::layout::{Feature, Owner, SurfaceFeatures, carrier_labels, sheet_size};
use ew_ids::Id;
use ew_l10n::TextArg;
use ew_schema::{CarrierRef, Kind, MountKind, MountingSurface, Placement, SurfaceFeature, SurfaceKind};
use ew_units::Nm;

use super::layout::{device_name, mount_of, part_name, realised_part, shown, surface_words};
use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};

/// The web between two holes when the workshop sets none (doc 12-3d §10.5; the program's assumption): 5 mm.
pub const DEFAULT_WEB: Nm = Nm(5_000_000);

/// The parts without holes EW-LAY-017 reports: each with its part use, its devices and the first surface it is on.
type PartsWithoutHoles = BTreeMap<
    Id<ew_schema::Part>,
    (
        ew_schema::PartRef,
        Vec<Id<ew_schema::Device>>,
        Id<MountingSurface>,
    ),
>;

/// A hole as a finding names it: its number and its first owner ("H5 (-104E2)").
fn hole_words(
    q: &Q,
    f: &Feature,
    labels: &BTreeMap<Id<ew_schema::Carrier>, ew_derive::layout::CarrierLabel>,
) -> TextArg {
    let owner: Option<TextArg> = f.owners.first().and_then(|o| match o {
        Owner::Placement { p, .. } => {
            let device = q.table::<Placement>().get(*p)?.device;
            Some(TextArg::Plain(ew_derive::short_tag(q, device)))
        }
        Owner::Carrier { c, .. } => Some(TextArg::Text(ew_derive::text::carrier_label(*labels.get(c)?))),
        Owner::ByHand { feature } => {
            let x = q.table::<SurfaceFeature>().get(*feature)?;
            match x.cable {
                Some(cable) => Some(TextArg::Plain(ew_derive::short_tag(q, cable))),
                None if !x.purpose.is_empty() => Some(TextArg::Ml(x.purpose.clone())),
                None => None,
            }
        }
    });
    let no = format!("H{}", f.no);
    match owner {
        Some(o) => TextArg::Text(text!("word-hole-of", no, o)),
        None => TextArg::Plain(no),
    }
}

/// The targets of a hole: its owners' records.
fn hole_targets(q: &Q, f: &Feature) -> Vec<Target> {
    f.owners
        .iter()
        .filter_map(|o| match o {
            Owner::Placement { p, .. } => {
                let pl = q.table::<Placement>().get(*p)?;
                Some(Target::new(Kind::Device, pl.device))
            }
            Owner::Carrier { c, .. } => Some(Target::new(Kind::Carrier, *c)),
            Owner::ByHand { feature } => Some(Target::new(Kind::SurfaceFeature, *feature)),
        })
        .collect()
}

/// A stable name of a hole for a finding's discriminator: its first owner and index.
fn hole_key(f: &Feature) -> String {
    match f.owners.first() {
        Some(Owner::Placement { p, index }) => format!("p{}.{index}", p.as_uuid()),
        Some(Owner::Carrier { c, index }) => format!("c{}.{index}", c.as_uuid()),
        Some(Owner::ByHand { feature }) => format!("f{}", feature.as_uuid()),
        None => format!("H{}", f.no),
    }
}

/// Half a hole's extent across, `(x, y)`.
fn half(f: &Feature) -> (i128, i128) {
    let (w, h) = f.made.extent();
    (i128::from(w.0) / 2, i128::from(h.0) / 2)
}

/// The gap between the edges of two holes: exact for two round holes, by their boxes otherwise.
fn gap(a: &Feature, b: &Feature) -> i128 {
    let (dx, dy) = (
        i128::from(a.at.x.0) - i128::from(b.at.x.0),
        i128::from(a.at.y.0) - i128::from(b.at.y.0),
    );
    let (ha, hb) = (half(a), half(b));
    let round = |f: &Feature| !matches!(f.made, ew_derive::layout::Made::Cut { .. });
    if round(a) && round(b) {
        let centres = (dx * dx + dy * dy).isqrt();
        return centres - ha.0 - hb.0;
    }
    (dx.abs() - ha.0 - hb.0).max(dy.abs() - ha.1 - hb.1)
}

// ---------------------------------------------------------------------------------------------------------
// EW-LAY-012

/// EW-LAY-012 "Holes too close" (doc 15): conflict, o r, warning, surface (as built: project) (FR-LAY-057).
pub const HOLES_TOO_CLOSE: RuleMeta = RuleMeta {
    id: "EW-LAY-012",
    message: key!("rule-ew-lay-012"),
    code: "130012",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-LAY-012: on every surface, two holes whose edges are closer than the workshop's web between holes (form
/// `close`), a hole whose edge is closer to the sheet's edge than its edge margin and that web (form `edge`; the
/// plate, which has no margin, by the web alone), and a hole whose centre lies off the sheet (form `off`). The
/// targets are the holes' owners; shown on the surface's page.
#[derive(Clone, Copy, Debug)]
pub struct HolesTooClose;

impl Rule for HolesTooClose {
    fn meta(&self) -> &'static RuleMeta {
        &HOLES_TOO_CLOSE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let web = q.project().settings.workshop.min_web.unwrap_or(DEFAULT_WEB);
        let mm = |n: i128| ew_derive::mm_tenth_short(n);
        let mut out = Vec::new();
        for (id, s) in q.table::<MountingSurface>().iter() {
            let features = q.get::<SurfaceFeatures>(id);
            if features.is_empty() {
                continue;
            }
            let labels: BTreeMap<_, _> = carrier_labels(q, id).into_iter().collect();
            let surface = surface_words(q, Some(id));
            let finding = |form: &str, key: String, holes: &[&Feature]| {
                let targets: Vec<Target> = holes.iter().flat_map(|f| hole_targets(q, f)).collect();
                let f = Finding::new(&HOLES_TOO_CLOSE, targets, &key, hole_words(q, holes[0], &labels))
                    .arg("form", form)
                    .arg("surface", surface.clone())
                    .arg("hole", hole_words(q, holes[0], &labels));
                shown(f, q, Some(id))
            };
            for (i, a) in features.iter().enumerate() {
                for b in features.iter().skip(i + 1) {
                    let g = gap(a, b);
                    if g < i128::from(web.0) {
                        let key = format!("close:{}:{}", hole_key(a), hole_key(b));
                        out.push(
                            finding("close", key, &[a, b])
                                .arg("other", hole_words(q, b, &labels))
                                .arg("gap", mm(g.max(0)))
                                .arg("web", mm(i128::from(web.0))),
                        );
                    }
                }
            }
            let Some((w, h)) = sheet_size(s) else { continue };
            let margin = match s.kind {
                SurfaceKind::Plate => Nm::ZERO,
                _ => s.edge_margin.unwrap_or(Nm::ZERO),
            };
            for f in features.iter() {
                let (x, y) = (i128::from(f.at.x.0), i128::from(f.at.y.0));
                let (w, h) = (i128::from(w.0), i128::from(h.0));
                if x < 0 || y < 0 || x > w || y > h {
                    out.push(finding("off", format!("off:{}", hole_key(f)), &[f]));
                    continue;
                }
                let (hx, hy) = half(f);
                let edge = (x - hx).min(w - x - hx).min(y - hy).min(h - y - hy);
                let keep = i128::from(margin.0) + i128::from(web.0);
                if edge < keep {
                    out.push(
                        finding("edge", format!("edge:{}", hole_key(f)), &[f])
                            .arg("distance", mm(edge.max(0)))
                            .arg("margin", mm(i128::from(margin.0)))
                            .arg("web", mm(i128::from(web.0))),
                    );
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-LAY-017

/// EW-LAY-017 "Holes not known" (doc 15): completeness, o r, warning, surface (as built: project) (FR-LAY-057).
pub const HOLES_NOT_KNOWN: RuleMeta = RuleMeta {
    id: "EW-LAY-017",
    message: key!("rule-ew-lay-017"),
    code: "130017",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-LAY-017: a part placed in a cut-out or screwed to a surface whose `mech.holes` is empty, once per part with its
/// devices (form `part`, quick fix Edit part…), and a device placed on a surface itself without a part (form
/// `device`, quick fix Choose part…). A part on a rail needs no holes and is never reported.
#[derive(Clone, Copy, Debug)]
pub struct HolesNotKnown;

impl Rule for HolesNotKnown {
    fn meta(&self) -> &'static RuleMeta {
        &HOLES_NOT_KNOWN
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let mut by_part: PartsWithoutHoles = BTreeMap::new();
        let mut out = Vec::new();
        let mut placements: Vec<(Id<Placement>, &Placement)> = Vec::new();
        let table = q.table::<Placement>();
        for (id, pl) in table.iter() {
            placements.push((id, pl));
        }
        for (_, pl) in placements {
            let CarrierRef::Surface(surface) = pl.carrier else {
                continue;
            };
            let part = realised_part(q, pl);
            let Some(part) = part else {
                let device = device_name(q, pl.device);
                let f = Finding::new(
                    &HOLES_NOT_KNOWN,
                    [Target::new(Kind::Device, pl.device)],
                    "no_part",
                    device.clone(),
                )
                .arg("form", "device")
                .arg("device", device)
                .arg("surface", surface_words(q, Some(surface)));
                out.push(shown(f, q, Some(surface)));
                continue;
            };
            let (mount, copy) = mount_of(q, Some(&part));
            let needs = matches!(mount, Some(MountKind::DoorCutout | MountKind::Screw));
            let empty = copy
                .as_ref()
                .and_then(|c| c.item.mech.as_ref())
                .is_none_or(|m| m.holes.is_empty());
            if needs && empty {
                let entry = by_part
                    .entry(part.copy)
                    .or_insert_with(|| (part.clone(), Vec::new(), surface));
                entry.1.push(pl.device);
            }
        }
        for (copy, (part, mut devices, surface)) in by_part {
            devices.sort_by_key(|d| ew_derive::short_tag(q, *d));
            devices.dedup();
            let names: Vec<TextArg> = devices.iter().map(|d| device_name(q, *d).into()).collect();
            let between = || ew_l10n::Text::bare(TextArg::Plain(", ".to_owned()));
            let f = Finding::new(
                &HOLES_NOT_KNOWN,
                devices.iter().map(|d| Target::new(Kind::Device, *d)),
                format!("no_holes:{}", copy.as_uuid()),
                part_name(q, &part),
            )
            .arg("form", "part")
            .arg("part", part_name(q, &part))
            .arg(
                "devices",
                ew_l10n::Text::bare(ew_l10n::List::new(names, between(), between())),
            )
            .arg("surface", surface_words(q, Some(surface)));
            out.push(shown(f, q, Some(surface)));
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-LAY-018

/// EW-LAY-018 "Cable gland" (doc 15): completeness, o r, warning, cabinet (as built: project) (doc 12 §3.9).
pub const CABLE_GLAND: RuleMeta = RuleMeta {
    id: "EW-LAY-018",
    message: key!("rule-ew-lay-018"),
    code: "130018",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-LAY-018: a cable that leaves a cabinet whose gland is not placed (form `unplaced`), whose gland is not known
/// (`no_diameter`: its part gives no outer diameter; `none_seals`: no stock gland seals it), whose placed hole is not
/// its gland's thread (`differs`), and a hole made for a cable that serves none any more (`no_cable`).
#[derive(Clone, Copy, Debug)]
pub struct CableGland;

impl Rule for CableGland {
    fn meta(&self) -> &'static RuleMeta {
        &CABLE_GLAND
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let mut out = Vec::new();
        let features = q.table::<SurfaceFeature>();
        let hole_text = |h: &ew_schema::Hole| TextArg::Text(ew_derive::text::hole_what(h));
        for (space, _) in q.table::<ew_schema::LayoutSpace>().iter() {
            for g in q.get::<ew_derive::layout::CableGlands>(space).iter() {
                let cable = device_name(q, g.cable);
                let base = |form: &str, key: String| {
                    Finding::new(
                        &CABLE_GLAND,
                        [Target::new(Kind::Device, g.cable)],
                        key,
                        cable.clone(),
                    )
                    .arg("form", form)
                    .arg("cable", cable.clone())
                };
                let section = match &g.part {
                    Err(ew_derive::layout::NoGland::NoDiameter) => {
                        out.push(base("no_diameter", "no_diameter".to_owned()));
                        continue;
                    }
                    Err(ew_derive::layout::NoGland::NoneSeals { od }) => {
                        let od = ew_derive::mm_tenth_short(i128::from(od.0));
                        out.push(base("none_seals", "none_seals".to_owned()).arg("od", od));
                        continue;
                    }
                    Ok(p) => ew_derive::layout::gland_section(q, p),
                };
                let Some(f) = g.placed.and_then(|id| features.get(id)) else {
                    out.push(base("unplaced", "unplaced".to_owned()));
                    continue;
                };
                let wanted = section.map(|s| ew_schema::Hole::Gland(s.thread));
                if wanted.as_ref().is_some_and(|w| *w != f.hole) {
                    let w = wanted.expect("a gland");
                    let finding = base("differs", "differs".to_owned())
                        .arg("surface", surface_words(q, Some(f.surface)))
                        .arg("hole", hole_text(&f.hole))
                        .arg("gland", hole_text(&w));
                    out.push(shown(finding, q, Some(f.surface)));
                }
            }
        }
        for (id, f) in features.iter().filter(|(_, f)| f.for_cable && f.cable.is_none()) {
            let finding = Finding::new(
                &CABLE_GLAND,
                [Target::new(Kind::SurfaceFeature, id)],
                "no_cable",
                hole_text(&f.hole),
            )
            .arg("form", "no_cable")
            .arg("hole", hole_text(&f.hole))
            .arg("surface", surface_words(q, Some(f.surface)));
            out.push(shown(finding, q, Some(f.surface)));
        }
        out
    }
}
