// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The panel layout records (doc 20 §13's test geometry, WP-S4.01 part 2): `layout_spaces.csv`, `enclosures.csv`
//! (an enclosure and its plate surface), `carriers.csv` and `placements.csv`. Lengths are millimetres in the plate's
//! frame (`u` right, `v` up); the records hold nanometres.
//!
//! The layout kinds carry no `source_ref` member (WP-S4.01 D3 types none), so their records have only the ID of the
//! identity rule: `v5(ns, "{table}|{key}")`, and the plate surface of an enclosure row `v5(ns,
//! "mounting_surfaces|{key}")`.

use std::collections::BTreeMap;

use ew_ids::Id;
use ew_schema::{
    Carrier, CarrierKind, CarrierParams, CarrierRef, DuctParams, Enclosure, LayoutSpace, MountKind,
    MountingSurface, PartRef, Placement, RailParams, Record, SurfaceKind,
};
use ew_units::{Nm, Pose3, Pt2, Quat9};

use super::parts::MOUNT_KINDS;
use super::{Cx, fresh};
use crate::ids::record_id;
use crate::table::Table;

/// The kinds of `carriers.csv`.
const CARRIER_KINDS: &[(&str, CarrierKind)] = &[("rail", CarrierKind::Rail), ("duct", CarrierKind::Duct)];

/// The emitted layout keys the later tables resolve.
#[derive(Default)]
pub(crate) struct LayoutKeys {
    spaces: BTreeMap<String, Id<LayoutSpace>>,
    /// Enclosure key → its plate surface.
    surfaces: BTreeMap<String, Id<MountingSurface>>,
    /// Carrier key → its ID and whether it is a rail.
    carriers: BTreeMap<String, (Id<Carrier>, bool)>,
}

/// Why a carrier path breaks WP-S4.01 D5 (a rail is two points on one horizontal or vertical line, a duct an
/// orthogonal polyline), or `None`.
pub fn path_problem(kind: &CarrierKind, path: &[Pt2]) -> Option<String> {
    match kind {
        CarrierKind::Rail if path.len() != 2 => {
            return Some(format!("a rail is two points, not {}", path.len()));
        }
        CarrierKind::Duct if path.len() < 2 => {
            return Some(format!("a duct is at least two points, not {}", path.len()));
        }
        _ => {}
    }
    path.windows(2).enumerate().find_map(|(i, w)| {
        let (du, dv) = (w[1].x.0 != w[0].x.0, w[1].y.0 != w[0].y.0);
        match (du, dv) {
            (false, false) => Some(format!("segment {i} has no length")),
            (true, true) => Some(format!("segment {i} is neither horizontal nor vertical")),
            _ => None,
        }
    })
}

/// Emits every layout table in dependency order: spaces, enclosures with their plates, carriers, placements.
pub(crate) fn emit(
    cx: &mut Cx<'_>,
    spaces: Option<&Table>,
    enclosures: Option<&Table>,
    carriers: Option<&Table>,
    placements: Option<&Table>,
) {
    let mut keys = LayoutKeys::default();
    emit_spaces(cx, spaces, &mut keys);
    emit_enclosures(cx, enclosures, &mut keys);
    emit_carriers(cx, carriers, &mut keys);
    emit_placements(cx, placements, &keys);
}

fn emit_spaces(cx: &mut Cx<'_>, table: Option<&Table>, keys: &mut LayoutKeys) {
    let Some(table) = table else { return };
    let mut by_location = BTreeMap::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &keys.spaces) {
            continue;
        }
        let mut r = row.reader();
        let location = r.req("location");
        let name = r.ident("name");
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let Some(location) = location else { continue };
        let Some(node) = cx.structure.get(location).copied() else {
            cx.unresolved(row, &format!("unknown structure node `{location}`"));
            continue;
        };
        if let Some(first) = by_location.get(&node) {
            cx.malformed(
                row,
                &format!("the location already has layout space `{first}`; a location has one layout space"),
            );
            continue;
        }
        by_location.insert(node, row.key().to_owned());
        let id = record_id::<LayoutSpace>("layout_spaces", row.key());
        keys.spaces.insert(row.key().to_owned(), id);
        cx.records.push(Record::LayoutSpace(LayoutSpace {
            id,
            v: 1,
            location: node,
            name,
            extra: Default::default(),
        }));
    }
}

fn emit_enclosures(cx: &mut Cx<'_>, table: Option<&Table>, keys: &mut LayoutKeys) {
    let Some(table) = table else { return };
    let mut seen = BTreeMap::<String, ()>::new();
    let mut spaces_used = BTreeMap::<Id<LayoutSpace>, String>::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let space = r.req("space");
        let device = r.req("device");
        let plate_device = r.text("plate_device");
        let frame = r.points("plate_frame");
        let usable = r.points("plate_usable");
        let pe_bond = r.flag("plate_pe_bond");
        if frame.len() > 1 {
            r.fail("column `plate_frame`: one point `u v`");
        }
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(space), Some(device)) = (space, device) else {
            continue;
        };
        let Some(space_id) = keys.spaces.get(space).copied() else {
            cx.unresolved(row, &format!("unknown layout space `{space}`"));
            continue;
        };
        let Some(device_id) = cx.devices.get(device).map(|d| d.id) else {
            cx.unresolved(row, &format!("unknown device `{device}`"));
            continue;
        };
        let plate_id = match plate_device {
            None => None,
            Some(p) => match cx.devices.get(p) {
                Some(d) => Some(d.id),
                None => {
                    cx.unresolved(row, &format!("unknown device `{p}` in column `plate_device`"));
                    continue;
                }
            },
        };
        if let Some(first) = spaces_used.get(&space_id) {
            cx.malformed(
                row,
                &format!(
                    "the layout space already has enclosure `{first}`; a layout space has one enclosure"
                ),
            );
            continue;
        }
        spaces_used.insert(space_id, row.key().to_owned());
        seen.insert(row.key().to_owned(), ());
        let id = record_id::<Enclosure>("enclosures", row.key());
        let surface = record_id::<MountingSurface>("mounting_surfaces", row.key());
        keys.surfaces.insert(row.key().to_owned(), surface);
        cx.records.push(Record::Enclosure(Enclosure {
            id,
            v: 1,
            space: space_id,
            device: device_id,
            pose: Pose3::IDENTITY,
            extra: Default::default(),
        }));
        let t = frame.first().copied().unwrap_or(Pt2::ORIGIN);
        cx.records.push(Record::MountingSurface(MountingSurface {
            id: surface,
            v: 1,
            enclosure: id,
            kind: SurfaceKind::Plate,
            device: plate_id,
            frame: Pose3 {
                t: [t.x, t.y, Nm::ZERO],
                q: Quat9::IDENTITY,
            },
            usable,
            pe_bond,
            extra: Default::default(),
        }));
    }
}

fn emit_carriers(cx: &mut Cx<'_>, table: Option<&Table>, keys: &mut LayoutKeys) {
    let Some(table) = table else { return };
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &keys.carriers) {
            continue;
        }
        let mut r = row.reader();
        let surface = r.req("surface");
        let kind = r.req_choice("kind", CARRIER_KINDS);
        let part = r.text("part");
        let path = r.points("path");
        let profile = r.choice("profile", MOUNT_KINDS);
        let width = r.mm("width");
        let height = r.mm("height");
        let pe_bond = r.flag("pe_bond");
        let params = match &kind {
            Some(CarrierKind::Rail) => match profile {
                Some(p @ (MountKind::Ts35x7_5 | MountKind::Ts35x15 | MountKind::Ts32 | MountKind::Ts15)) => {
                    Some(CarrierParams::Rail(RailParams {
                        profile: p,
                        extra: Default::default(),
                    }))
                }
                _ => {
                    r.fail("column `profile`: a rail's profile is ts35x7_5, ts35x15, ts32 or ts15");
                    None
                }
            },
            Some(CarrierKind::Duct) => match (width, height) {
                (Some(w), Some(h)) if w > Nm::ZERO && h > Nm::ZERO => Some(CarrierParams::Duct(DuctParams {
                    width: w,
                    height: h,
                    slot_pitch: None,
                    cover: None,
                    extra: Default::default(),
                })),
                _ => {
                    r.fail("columns `width` and `height`: a duct's size is two lengths greater than 0");
                    None
                }
            },
            _ => None,
        };
        if let Some(problem) = kind.as_ref().and_then(|k| path_problem(k, &path)) {
            r.fail(format!("column `path`: {problem}"));
        }
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(surface), Some(kind), Some(params)) = (surface, kind, params) else {
            continue;
        };
        let Some(surface_id) = keys.surfaces.get(surface).copied() else {
            cx.unresolved(row, &format!("unknown enclosure `{surface}` in column `surface`"));
            continue;
        };
        let part = match part {
            None => None,
            Some(p) => match cx.parts.get(p) {
                Some(info) => Some(PartRef::new(info.id)),
                None => {
                    cx.unresolved(row, &format!("unknown part `{p}`"));
                    continue;
                }
            },
        };
        let id = record_id::<Carrier>("carriers", row.key());
        keys.carriers
            .insert(row.key().to_owned(), (id, kind == CarrierKind::Rail));
        cx.records.push(Record::Carrier(Carrier {
            id,
            v: 1,
            surface: surface_id,
            kind,
            part,
            path,
            params,
            pe_bond,
            extra: Default::default(),
        }));
    }
}

fn emit_placements(cx: &mut Cx<'_>, table: Option<&Table>, keys: &LayoutKeys) {
    let Some(table) = table else { return };
    let mut seen = BTreeMap::<String, ()>::new();
    let mut placed = BTreeMap::<(String, Option<String>), String>::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let device = r.req("device");
        let carrier = r.req("carrier");
        let s = r.req("s").and(r.mm("s"));
        let part = r.text("part");
        if s.is_some_and(|s| s < Nm::ZERO) {
            r.fail("column `s`: a position on a rail is not negative");
        }
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(device), Some(carrier), Some(s)) = (device, carrier, s) else {
            continue;
        };
        let Some(info) = cx.devices.get(device) else {
            cx.unresolved(row, &format!("unknown device `{device}`"));
            continue;
        };
        let device_id = info.id;
        let part = match part {
            None => None,
            Some(p) if !info.parts.iter().any(|k| k == p) => {
                cx.malformed(row, &format!("device `{device}` does not use part `{p}`"));
                continue;
            }
            Some(p) => match cx.parts.get(p) {
                Some(part) => Some(PartRef::new(part.id)),
                None => {
                    cx.unresolved(row, &format!("unknown part `{p}`"));
                    continue;
                }
            },
        };
        let Some((carrier_id, rail)) = keys.carriers.get(carrier).copied() else {
            cx.unresolved(row, &format!("unknown carrier `{carrier}`"));
            continue;
        };
        if !rail {
            cx.malformed(
                row,
                &format!("carrier `{carrier}` is a duct; a duct carries no placements"),
            );
            continue;
        }
        let identity = (device.to_owned(), row.get("part").map(str::to_owned));
        if let Some(first) = placed.get(&identity) {
            cx.malformed(
                row,
                &format!("device `{device}` is already placed with this part by `{first}`"),
            );
            continue;
        }
        placed.insert(identity, row.key().to_owned());
        seen.insert(row.key().to_owned(), ());
        cx.records.push(Record::Placement(Placement {
            id: record_id::<Placement>("placements", row.key()),
            v: 1,
            device: device_id,
            function: None,
            part,
            carrier: CarrierRef::Carrier(carrier_id),
            pose: Pose3 {
                t: [s, Nm::ZERO, Nm::ZERO],
                q: Quat9::IDENTITY,
            },
            locked: false,
            extra: Default::default(),
        }));
    }
}
