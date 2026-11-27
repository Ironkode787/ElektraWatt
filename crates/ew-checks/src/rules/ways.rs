// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rules of a cabinet's ways and room (doc 15 §4.5.4, §4.5.5; doc 12 §7 to §9): EW-LAY-007, a footprint or a duct
//! inside the room a device's part keeps free; EW-LAY-008, a duct over the fill limit where it is fullest; EW-LAY-011,
//! ways that no longer hold, one finding per cause; EW-MFG-001, the wires of a cabinet without a length. They run at
//! the project scope, as the other layout rules (`rules/layout.rs`), and read `ew_derive::layout`.

use std::collections::BTreeMap;

use ew_derive::Q;
use ew_derive::layout::{
    CabinetWire, DuctFill, PlacementGeometry, RouteStatus, StaleWhy, cabinet_name, cabinet_wires,
    carrier_label, route_status,
};
use ew_ids::Id;
use ew_l10n::TextArg;
use ew_schema::{Carrier, CarrierKind, Kind, LayoutSpace, LibCopy, Part, Placement};
use ew_units::Rect;
use serde_json::json;

use crate::finding::{Finding, QuickFix, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::Named;
use crate::rules::term::full_tag;

/// A device as printed: its full designation.
fn device_name(q: &Q, device: Id<ew_schema::Device>) -> Named {
    full_tag(q, device, text!("word-a-device-without-designation"))
}

/// A length in millimetres as printed: exact, without trailing zeros, with its unit.
fn mm(nm: i128) -> String {
    format!("{} mm", ew_derive::mm_exact(nm))
}

/// A duct inside a sentence: its label ("duct 1"); `None` for a carrier that is gone or of a newer kind.
fn duct_name(q: &Q, c: Id<Carrier>) -> Option<TextArg> {
    carrier_label(q, c).map(|l| ew_derive::text::carrier_in_sentence(l).into())
}

/// Route as a finding's quick fix: `layout.route` over the space.
fn route_fix(space: Id<LayoutSpace>) -> QuickFix {
    QuickFix {
        label: text!("fix-route"),
        command: "layout.route".to_owned(),
        params: json!({ "space": space.as_uuid() }),
    }
}

/// A finding whose fingerprint is its rule's, its cause's ID and `discriminator`, with `more` targets added after it is
/// made, so that the fingerprint does not move with them (a new wire at a moved device keeps the finding).
fn with_more_targets(f: Finding, more: impl IntoIterator<Item = Target>) -> Finding {
    let mut f = f;
    f.targets.extend(more);
    f.targets.sort();
    f.targets.dedup();
    f
}

// ---------------------------------------------------------------------------------------------------------
// EW-LAY-007

/// EW-LAY-007 "Clearance violated" (doc 15 §4.5.4): conflict, o r, warning, space (as built: project) (FR-LAY-029).
pub const CLEARANCE: RuleMeta = RuleMeta {
    id: "EW-LAY-007",
    message: key!("rule-ew-lay-007"),
    code: "130007",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::V1_0,
};

/// The four sides of a footprint in the surface's frame, by the word the message prints.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Side {
    Above,
    Below,
    Left,
    Right,
}

impl Side {
    fn word(self) -> &'static str {
        match self {
            Side::Above => "above",
            Side::Below => "below",
            Side::Left => "left",
            Side::Right => "right",
        }
    }
}

/// The clearance a placement's part keeps on each side in the surface's frame: the part's top, bottom, left and right
/// turned with the part (a quarter turn puts its top on the left).
fn clearance_sides(q: &Q, pl: Id<Placement>) -> Option<(Rect, [(Side, i128); 4])> {
    let g = q.get::<PlacementGeometry>(pl);
    let g = g.as_ref().as_ref()?;
    if g.unknown {
        return None;
    }
    let c = g
        .part
        .as_ref()
        .and_then(|p| q.table::<LibCopy<Part>>().get(p.copy))
        .and_then(|c| c.item.mech.as_ref().and_then(|m| m.clearance.clone()))?;
    let mut sides = [
        (Side::Above, 0),
        (Side::Below, 0),
        (Side::Left, 0),
        (Side::Right, 0),
    ];
    for ((dx, dy), n) in [
        ((0, 1), c.top),
        ((0, -1), c.bottom),
        ((-1, 0), c.left),
        ((1, 0), c.right),
    ] {
        let side = match g.turn.rotate((dx, dy)) {
            (0, 1) => Side::Above,
            (0, _) => Side::Below,
            (x, _) if x < 0 => Side::Left,
            _ => Side::Right,
        };
        let n = i128::from(n.0.max(0));
        if let Some(s) = sides.iter_mut().find(|(s, _)| *s == side) {
            s.1 = s.1.max(n);
        }
    }
    sides.iter().any(|(_, n)| *n > 0).then_some((g.footprint, sides))
}

/// How far `other` stands from `fp` on `side`, where it lies within that side's band of width `need` (overlapping the
/// footprint's span across it): `None` where it stands outside the band, or overlaps the footprint itself (EW-LAY-006's).
fn intrusion(fp: Rect, other: Rect, side: Side, need: i128) -> Option<i128> {
    let (x0, x1, y0, y1) = (
        i128::from(fp.min().x.0),
        i128::from(fp.max().x.0),
        i128::from(fp.min().y.0),
        i128::from(fp.max().y.0),
    );
    let (ox0, ox1, oy0, oy1) = (
        i128::from(other.min().x.0),
        i128::from(other.max().x.0),
        i128::from(other.min().y.0),
        i128::from(other.max().y.0),
    );
    let across_x = ox0 < x1 && x0 < ox1;
    let across_y = oy0 < y1 && y0 < oy1;
    let at = match side {
        Side::Above if across_x => oy0 - y1,
        Side::Below if across_x => y0 - oy1,
        Side::Left if across_y => x0 - ox1,
        Side::Right if across_y => ox0 - x1,
        _ => return None,
    };
    (0..need).contains(&at).then_some(at)
}

/// EW-LAY-007: a footprint or a duct of the surface inside the zone a device's part keeps free (its footprint grown by
/// its clearance top, bottom, left and right, turned with the part), one finding per (device, intruder) with the side
/// and the amount of the nearest; not the device's own rail (no rail intrudes), not an end bracket, and no zone of an
/// unknown footprint. Targets: the device, its placement and the intruder; discriminator the intruder.
#[derive(Clone, Copy, Debug)]
pub struct Clearance;

impl Rule for Clearance {
    fn meta(&self) -> &'static RuleMeta {
        &CLEARANCE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let placements = q.table::<Placement>();
        let carriers = q.table::<Carrier>();
        let mut out = Vec::new();
        for (id, pl) in placements.iter() {
            let Some((fp, sides)) = clearance_sides(q, id) else {
                continue;
            };
            let Some(surface) = q
                .get::<PlacementGeometry>(id)
                .as_ref()
                .as_ref()
                .map(|g| g.surface)
            else {
                continue;
            };
            let mut intruders: Vec<(Rect, Target, ew_l10n::TextArg)> = Vec::new();
            for (oid, opl) in placements.iter() {
                if oid == id {
                    continue;
                }
                if let Some(g) = q.get::<PlacementGeometry>(oid).as_ref()
                    && g.surface == surface
                {
                    intruders.push((
                        g.footprint,
                        Target::new(Kind::Placement, oid),
                        device_name(q, opl.device).into(),
                    ));
                }
            }
            for (cid, c) in carriers.iter() {
                if c.kind != CarrierKind::Duct || c.surface != surface {
                    continue;
                }
                if let Some(g) = q.get::<ew_derive::layout::CarrierGeometry>(cid).as_ref() {
                    if let Some(name) = duct_name(q, cid) {
                        for r in &g.outline {
                            intruders.push((*r, Target::new(Kind::Carrier, cid), name.clone()));
                        }
                    }
                }
            }
            let mut found: BTreeMap<Target, (i128, Side, i128, ew_l10n::TextArg)> = BTreeMap::new();
            for (r, target, name) in intruders {
                for (side, need) in sides {
                    let Some(at) = intrusion(fp, r, side, need) else {
                        continue;
                    };
                    let short = need - at;
                    if found.get(&target).is_none_or(|f| short > f.0) {
                        found.insert(target, (short, side, need, name.clone()));
                    }
                }
            }
            let name = device_name(q, pl.device);
            for (target, (short, side, need, other)) in found {
                let f = Finding::new(
                    &CLEARANCE,
                    [
                        Target::new(Kind::Device, pl.device),
                        Target::new(Kind::Placement, id),
                        target,
                    ],
                    target.id.as_uuid().to_string(),
                    name.clone(),
                )
                .arg("device", name.clone())
                .arg("need", mm(need))
                .arg("side", side.word())
                .arg("other", other)
                .arg("at", mm(need - short))
                .arg("short", mm(short));
                out.push(f);
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-LAY-008

/// EW-LAY-008 "Duct fill over limit" (doc 15 §4.5.4): conflict, o r, warning, carrier (as built: project)
/// (FR-LAY-049).
pub const FILL_OVER: RuleMeta = RuleMeta {
    id: "EW-LAY-008",
    message: key!("rule-ew-lay-008"),
    code: "130008",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::V1_0,
};

/// EW-LAY-008: a duct whose fullest piece, on the stored ways, is over the shop's fill limit (`duct_fill`, doc 12
/// §8.6). Target the duct; discriminator `fill`.
#[derive(Clone, Copy, Debug)]
pub struct FillOver;

impl Rule for FillOver {
    fn meta(&self) -> &'static RuleMeta {
        &FILL_OVER
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (id, c) in q.table::<Carrier>().iter() {
            if c.kind != CarrierKind::Duct {
                continue;
            }
            let Some(fill) = q.get::<DuctFill>(id).as_ref().clone() else {
                continue;
            };
            if !fill.over() {
                continue;
            }
            let (area, inner) = fill.fill();
            let percent = ew_units::arith::div_round_ties_away(area * 100, inner.max(1));
            let size = match &c.params {
                ew_schema::CarrierParams::Duct(d) => format!(
                    "{}×{}",
                    ew_derive::mm_exact(i128::from(d.width.0)),
                    ew_derive::mm_exact(i128::from(d.height.0))
                ),
                _ => String::new(),
            };
            let Some(label) = carrier_label(q, id) else {
                continue;
            };
            let label = TextArg::Text(ew_derive::text::carrier_label(label));
            out.push(
                Finding::new(
                    &FILL_OVER,
                    [Target::new(Kind::Carrier, id)],
                    "fill",
                    label.clone(),
                )
                .arg("duct", label)
                .arg("size", size)
                .arg("fill", format!("{percent} %"))
                .arg("limit", format!("{} %", fill.limit)),
            );
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-LAY-011

/// EW-LAY-011 "Stale way" (doc 15 §4.5.4): completeness, o r, warning (release: error), space (as built: project)
/// (FR-LAY-046).
pub const STALE_WAY: RuleMeta = RuleMeta {
    id: "EW-LAY-011",
    message: key!("rule-ew-lay-011"),
    code: "130011",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: Some(Severity::Error),
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::V1_0,
};

/// EW-LAY-011: the stored ways that no longer hold (doc 12 §8.5), one finding per cause and cabinet: a device that moved
/// or is no longer placed, a duct that changed or is gone, a wire whose end moved to another pin. Its wires are its
/// targets beside the cause, and its fingerprint is the cause's alone ([`with_more_targets`]), so a wire more or less
/// at the cause keeps the finding; quick fix Route.
#[derive(Clone, Copy, Debug)]
pub struct StaleWay;

impl Rule for StaleWay {
    fn meta(&self) -> &'static RuleMeta {
        &STALE_WAY
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (space, _) in q.table::<LayoutSpace>().iter() {
            let mut causes: BTreeMap<StaleWhy, Vec<(Id<ew_schema::Connection>, String)>> = BTreeMap::new();
            for (r, kind) in cabinet_wires(q, space) {
                if kind != CabinetWire::Routed {
                    continue;
                }
                if let RouteStatus::Stale { why, .. } = route_status(q, &r.wire) {
                    // A wire whose end moved is its own cause.
                    let key = match why {
                        StaleWhy::EndChanged { .. } => why,
                        other => other,
                    };
                    causes
                        .entry(key)
                        .or_default()
                        .push((r.wire.id, crate::rules::pot::wire_text(q, &r.wire)));
                }
            }
            for (why, wires) in causes {
                let count = i64::try_from(wires.len()).unwrap_or(i64::MAX);
                let wire_targets = wires.iter().map(|(w, _)| Target::new(Kind::Connection, *w));
                let (cause, form, subject): (Target, &str, ew_l10n::TextArg) = match why {
                    StaleWhy::Moved(d) => (Target::new(Kind::Device, d), "moved", device_name(q, d).into()),
                    StaleWhy::Unplaced(d) => {
                        (Target::new(Kind::Device, d), "unplaced", device_name(q, d).into())
                    }
                    StaleWhy::DuctChanged(c) => match duct_name(q, c) {
                        Some(name) => (Target::new(Kind::Carrier, c), "duct", name),
                        None => (
                            Target::new(Kind::Carrier, c),
                            "gone",
                            ew_derive::text::stale_why(q, why).into(),
                        ),
                    },
                    StaleWhy::DuctGone(c) => (
                        Target::new(Kind::Carrier, c),
                        "gone",
                        ew_derive::text::stale_why(q, why).into(),
                    ),
                    StaleWhy::EndChanged { .. } => {
                        let (w, text) = wires[0].clone();
                        (Target::new(Kind::Connection, w), "end", text.into())
                    }
                };
                let mut f = Finding::new(
                    &STALE_WAY,
                    [cause],
                    cause.id.as_uuid().to_string(),
                    subject.clone(),
                )
                .arg("form", form)
                .arg("count", TextArg::Number(count))
                .arg("at", subject)
                .with_fix(route_fix(space));
                if let StaleWhy::EndChanged { from, to } = why {
                    let pin = |(f, i)| ew_derive::cp_label(q, ew_derive::CpRef::new(f, i));
                    f = f.arg("from", pin(from)).arg("to", pin(to));
                }
                out.push(with_more_targets(f, wire_targets));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-MFG-001

/// EW-MFG-001 "Wires without a length" (doc 15 §4.5.5): completeness, o r, info, project (FR-MFG-024).
pub const NO_LENGTH: RuleMeta = RuleMeta {
    id: "EW-MFG-001",
    message: key!("rule-ew-mfg-001"),
    code: "140001",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Info,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::V1_0,
};

/// EW-MFG-001: in a project with a panel layout, one finding per cabinet while wires Route lays (no core, no field wire)
/// have no length (doc 12 §9); none in a project without a layout. Its wires are its targets beside the layout space,
/// its fingerprint the space's; quick fix Route. A stale length is EW-LAY-011's.
#[derive(Clone, Copy, Debug)]
pub struct NoLength;

impl Rule for NoLength {
    fn meta(&self) -> &'static RuleMeta {
        &NO_LENGTH
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (space, _) in q.table::<LayoutSpace>().iter() {
            let wires: Vec<Id<ew_schema::Connection>> = cabinet_wires(q, space)
                .into_iter()
                .filter(|(r, k)| *k == CabinetWire::Routed && r.length == ew_derive::WireLength::Missing)
                .map(|(r, _)| r.wire.id)
                .collect();
            if wires.is_empty() {
                continue;
            }
            let cabinet = cabinet_name(q, space);
            let f = Finding::new(
                &NO_LENGTH,
                [Target::new(Kind::LayoutSpace, space)],
                "length",
                cabinet.clone(),
            )
            .arg(
                "count",
                TextArg::Number(i64::try_from(wires.len()).unwrap_or(i64::MAX)),
            )
            .arg("cabinet", cabinet)
            .with_fix(route_fix(space));
            out.push(with_more_targets(
                f,
                wires.into_iter().map(|w| Target::new(Kind::Connection, w)),
            ));
        }
        out
    }
}
