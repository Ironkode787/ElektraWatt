// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arms for the panel layout.

use super::*;

/// 1 mm in nanometres.
pub(super) const LAY_MM: i64 = 1_000_000;

/// The quarter turns about `w` as sign-normalised quaternions: 0°, 90°, 180°, 270°.
pub(super) const QUARTERS: [[i64; 4]; 4] = [
    [1_000_000_000, 0, 0, 0],
    [707_106_781, 0, 0, 707_106_781],
    [0, 0, 0, 1_000_000_000],
    [707_106_781, 0, 0, -707_106_781],
];

pub(super) fn lay_pose(t: [i64; 3], q: [i64; 4]) -> Value {
    json!({"t": t, "q": q})
}

/// EW-LAY-002, the fuzzer's layout invariant (LM-6): every placement's device exists; and (AS-7)
/// every placement's carrier or surface exists.
pub fn placements_have_devices(q: &ew_derive::Q) -> Result<(), String> {
    let placements = q.table::<ew_schema::Placement>();
    if let Some((id, _)) = placements.iter().find(|(_, p)| !q.devices().contains(p.device)) {
        return Err(format!("placement {} has no device", id_text(&id_json(id))));
    }
    let carriers = q.table::<ew_schema::Carrier>();
    let surfaces = q.table::<ew_schema::MountingSurface>();
    let held = |p: &ew_schema::Placement| match &p.carrier {
        ew_schema::CarrierRef::Carrier(c) => carriers.get(*c).is_some(),
        ew_schema::CarrierRef::Surface(s) => surfaces.get(*s).is_some(),
        ew_schema::CarrierRef::Unknown(_) => true,
    };
    match placements.iter().find(|(_, p)| !held(p)) {
        Some((id, _)) => Err(format!(
            "placement {} has no carrier or surface",
            id_text(&id_json(id))
        )),
        None => Ok(()),
    }
}

/// `part.assign` of a new part copy with an enclosure section (a golden template with a new lineage and a 750 × 1000
/// × 420 mm enclosure, no plate) to a plain device, appended to its part uses: what Golden A lacks for
/// `layout.enclosure.create`.
pub(super) fn assign_enclosure_part(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let device = rng.pick(&live.plain_devices)?.clone();
    let (_, template) = rng.pick(&live.fixture.part_json)?;
    let mut t = template.clone();
    if !t["sections"].is_object() {
        t["sections"] = json!({});
    }
    t["sections"]["enclosure"] =
        json!({"width": 750 * LAY_MM, "height": 1000 * LAY_MM, "depth": 420 * LAY_MM});
    let c = relineaged(&t, &rng.uuid(), &rng.uuid())?;
    let id = c["id"].clone();
    Some(
        Step::cmd(
            "part.assign",
            json!({"devices": [device], "part": id, "copies": [c]}),
        )
        .counts(&[(Table::Parts, 1)]),
    )
}

pub(super) fn setup_space(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let location = rng.pick(&live.layout.free_locations)?;
    Some(Step::cmd("layout.space.create", json!({"location": location})))
}

pub(super) fn setup_enclosure_part(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    if !live.layout.enclosure_devices.is_empty() {
        return None;
    }
    assign_enclosure_part(live, rng)
}

pub(super) fn setup_enclosure(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let space = rng.pick(&live.layout.empty_spaces)?;
    let device = rng.pick(&live.layout.enclosure_devices)?;
    Some(Step::cmd(
        "layout.enclosure.create",
        json!({"space": space, "device": device}),
    ))
}

pub(super) fn setup_rail(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let surface = rng.pick(&live.layout.surfaces)?;
    let path = [[10 * LAY_MM, 500 * LAY_MM], [610 * LAY_MM, 500 * LAY_MM]];
    let p = json!({"surface": surface, "kind": "rail", "path": path, "params": {"rail": {"profile": "ts35x7_5"}}});
    Some(Step::cmd("layout.carrier.add", p))
}

/// A plain device without a placement put on the plate itself (a surface checks no profile).
pub(super) fn setup_place(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let surface = rng.pick(&live.layout.surfaces)?;
    let placed: std::collections::BTreeSet<String> = live
        .layout
        .placements
        .iter()
        .map(|p| id_text(&p.device))
        .collect();
    let free: Vec<&Value> = live
        .plain_devices
        .iter()
        .filter(|d| !placed.contains(&id_text(d)))
        .collect();
    let device = *rng.pick(&free)?;
    let pose = lay_pose([100 * LAY_MM, 100 * LAY_MM, 0], QUARTERS[0]);
    Some(Step::cmd(
        "layout.place",
        json!({"device": device, "carrier": {"surface": surface}, "pose": pose}),
    ))
}

/// `layout.space.create`: a location without a space, with or without a name (accepted); refused: a node that does not
/// exist, a location that has a space, a node of another aspect.
pub(super) fn layout_space_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let l = &live.layout;
    let cmd = |p: Value| Step::cmd("layout.space.create", p);
    match rng.below(6) {
        0 => Some(cmd(json!({"location": rng.uuid()})).refuse()),
        1 => {
            let taken: Vec<&Value> = live
                .locations
                .iter()
                .filter(|n| !l.free_locations.contains(n))
                .collect();
            Some(cmd(json!({"location": *rng.pick(&taken)?})))
        }
        2 => {
            let other: Vec<&super::super::live::NodeInfo> =
                live.nodes.iter().filter(|n| n.aspect != "location").collect();
            Some(cmd(json!({"location": rng.pick(&other)?.id})))
        }
        _ => {
            let mut p = json!({"location": rng.pick(&l.free_locations)?});
            if rng.chance(2) {
                p["name"] = json!(rng.word(8));
            }
            Some(cmd(p).counts(&[(Table::LayoutSpaces, 1)]))
        }
    }
}

/// `layout.space.delete`: an empty space goes with its enclosure, surfaces and carriers (accepted); refused: one with a
/// placement below it (the forbid policy of `placement.carrier`), one that does not exist.
pub(super) fn layout_space_delete(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let l = &live.layout;
    let cmd = |p: Value| Step::cmd("layout.space.delete", p);
    if rng.chance(6) {
        return Some(cmd(json!({"space": rng.uuid()})).refuse());
    }
    let space = rng.pick(&l.spaces)?;
    Some(if l.occupied.contains(&id_text(space)) {
        cmd(json!({"space": space}))
    } else {
        cmd(json!({"space": space})).counts(&[(Table::LayoutSpaces, -1), (Table::Placements, 0)])
    })
}

/// `layout.enclosure.create`: an enclosure device in a space without an enclosure (accepted, with its plate surface);
/// refused: a space that does not exist, a device without an enclosure part, a space that has an enclosure. While no
/// device has an enclosure part (Golden A), the arm first gives one to a plain device (`part.assign` with a copy that
/// carries an enclosure section), so that the layout arms find something to build on.
pub(super) fn layout_enclosure_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let l = &live.layout;
    let Some(device) = rng.pick(&l.enclosure_devices).cloned() else {
        return assign_enclosure_part(live, rng);
    };
    let cmd = |p: Value| Step::cmd("layout.enclosure.create", p);
    match rng.below(6) {
        0 => Some(cmd(json!({"space": rng.uuid(), "device": device})).refuse()),
        1 => {
            let plain = rng.pick(&live.plain_devices)?;
            if l.enclosure_devices.contains(plain) {
                return None;
            }
            let space = rng.pick(&l.empty_spaces)?;
            Some(cmd(json!({"space": space, "device": plain})))
        }
        2 => {
            let full: Vec<&Value> = l.spaces.iter().filter(|s| !l.empty_spaces.contains(s)).collect();
            Some(cmd(json!({"space": *rng.pick(&full)?, "device": device})))
        }
        _ => {
            let space = rng.pick(&l.empty_spaces)?;
            Some(
                cmd(json!({"space": space, "device": device, "pe_bond": rng.chance(2)}))
                    .counts(&[(Table::Enclosures, 1), (Table::Surfaces, 1)]),
            )
        }
    }
}

/// `layout.enclosure.delete`: an empty enclosure goes with its surface and carriers (accepted); refused: an occupied
/// one, one that does not exist.
pub(super) fn layout_enclosure_delete(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let l = &live.layout;
    let cmd = |p: Value| Step::cmd("layout.enclosure.delete", p);
    if rng.chance(6) {
        return Some(cmd(json!({"enclosure": rng.uuid()})).refuse());
    }
    let enclosure = rng.pick(&l.enclosures)?;
    Some(if l.occupied.contains(&id_text(enclosure)) {
        cmd(json!({"enclosure": enclosure}))
    } else {
        cmd(json!({"enclosure": enclosure})).counts(&[(Table::Enclosures, -1), (Table::Surfaces, -1)])
    })
}

/// A random axis-parallel segment on a plate in 10 mm steps: start, end, and whether it is horizontal.
pub(super) fn lay_segment(rng: &mut SplitMix64) -> ([i64; 2], [i64; 2], bool) {
    let start = [10 * LAY_MM * rng.range(0, 60), 10 * LAY_MM * rng.range(0, 90)];
    let len = 10 * LAY_MM * rng.range(1, 60);
    let horizontal = rng.chance(2);
    let end = if horizontal {
        [start[0] + len, start[1]]
    } else {
        [start[0], start[1] + len]
    };
    (start, end, horizontal)
}

/// `layout.carrier.add`: an axis-parallel rail of any profile, or an orthogonal duct with a turn (accepted); refused: a
/// surface that does not exist, a diagonal rail, a duct with a rail's parameters, a segment without length.
pub(super) fn layout_carrier_add(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let surface = rng.pick(&live.layout.surfaces)?.clone();
    let cmd = |p: Value| Step::cmd("layout.carrier.add", p);
    let (start, end, horizontal) = lay_segment(rng);
    let rail = json!({"rail": {"profile": *rng.pick(&["ts35x7_5", "ts35x15", "ts32", "ts15"])?}});
    let duct = json!({"duct": {"width": 40 * LAY_MM, "height": 60 * LAY_MM}});
    match rng.below(8) {
        0 => {
            let p = json!({"surface": rng.uuid(), "kind": "rail", "path": [start, end], "params": rail});
            Some(cmd(p).refuse())
        }
        1 => {
            let diagonal = [end[0] + 10 * LAY_MM, end[1] + 10 * LAY_MM];
            let p = json!({"surface": surface, "kind": "rail", "path": [start, diagonal], "params": rail});
            Some(cmd(p))
        }
        2 => {
            let p = json!({"surface": surface, "kind": "duct", "path": [start, end], "params": rail});
            Some(cmd(p))
        }
        3 => {
            let p = json!({"surface": surface, "kind": "duct", "path": [start, start, end], "params": duct});
            Some(cmd(p))
        }
        4 | 5 => {
            let turn = 10 * LAY_MM * rng.range(1, 30);
            let corner = if horizontal {
                [end[0], end[1] + turn]
            } else {
                [end[0] + turn, end[1]]
            };
            let p = json!({"surface": surface, "kind": "duct", "path": [start, end, corner], "params": duct});
            Some(cmd(p).counts(&[(Table::Carriers, 1)]))
        }
        _ => {
            let p = json!({"surface": surface, "kind": "rail", "path": [start, end], "params": rail,
                "pe_bond": rng.chance(2)});
            Some(cmd(p).counts(&[(Table::Carriers, 1)]))
        }
    }
}

/// `layout.carrier.edit`: a rail on another surface or along another axis-parallel path, a carrier's PE bond
/// (accepted); refused: a carrier that does not exist, no member, a diagonal path.
pub(super) fn layout_carrier_edit(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let l = &live.layout;
    let cmd = |p: Value| Step::cmd("layout.carrier.edit", p);
    match rng.below(7) {
        0 => Some(cmd(json!({"carrier": rng.uuid(), "pe_bond": true})).refuse()),
        1 => Some(cmd(json!({"carrier": rng.pick(&l.carriers)?.0}))),
        2 => {
            let r = rng.pick(&l.rails)?;
            Some(cmd(
                json!({"carrier": r.id, "path": [[0, 0], [10 * LAY_MM, 10 * LAY_MM]]}),
            ))
        }
        3 => {
            let r = rng.pick(&l.rails)?;
            let s = rng.pick(&l.surfaces)?;
            Some(cmd(json!({"carrier": r.id, "surface": s})).counts(&[(Table::Carriers, 0)]))
        }
        4 | 5 => {
            let r = rng.pick(&l.rails)?;
            let (start, end, _) = lay_segment(rng);
            Some(cmd(json!({"carrier": r.id, "path": [start, end]})))
        }
        _ => {
            let (c, _) = rng.pick(&l.carriers)?;
            Some(cmd(json!({"carrier": c, "pe_bond": rng.chance(2)})))
        }
    }
}

/// `layout.carrier.delete`: an empty carrier goes without `then` (accepted); an occupied one is refused without
/// `then`, goes with its placements with `remove_placements` (accepted), and with `rehome_to_surface` moves them to its
/// surface unless it is not a rail or a pose is off it (either); one that does not exist is refused.
pub(super) fn layout_carrier_delete(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let l = &live.layout;
    let cmd = |p: Value| Step::cmd("layout.carrier.delete", p);
    if rng.chance(6) {
        return Some(cmd(json!({"carrier": rng.uuid()})).refuse());
    }
    let (carrier, n) = rng.pick(&l.carriers)?;
    let n = isize::try_from(*n).expect("a count fits isize");
    if n == 0 {
        return Some(cmd(json!({"carrier": carrier})).counts(&[(Table::Carriers, -1)]));
    }
    Some(match rng.below(3) {
        0 => cmd(json!({"carrier": carrier})),
        1 => cmd(json!({"carrier": carrier, "then": "remove_placements"}))
            .counts(&[(Table::Carriers, -1), (Table::Placements, -n)]),
        _ => cmd(json!({"carrier": carrier, "then": "rehome_to_surface"}))
            .counts(&[(Table::Carriers, -1), (Table::Placements, 0)]),
    })
}

/// `layout.place`: a plain device on a rail at `s ∈ [0, L]` turned 0° or 180°, or on a plate turned `k · 90°` (either:
/// the device may be placed already with its part and function, its part may not fit the profile); refused: a device
/// or carrier that does not exist, `s` beyond the rail's end, a pose off the rail's axis, a duct. After a commit every
/// placement has its device. WP-S4.02 part 1 (appended): with `mode` free or push on a rail (either: no gap may fit, a
/// push may pass the rail end or move a locked placement); refused: swap (a device on no rail swaps with nothing), push
/// or swap on a surface.
pub(super) fn layout_place(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let l = &live.layout;
    let device = rng.pick(&live.plain_devices)?.clone();
    let cmd = |p: Value| Step::cmd("layout.place", p);
    let at = |s: i64| lay_pose([s, 0, 0], QUARTERS[0]);
    let rails: Vec<&super::super::live::RailInfo> = l.rails.iter().filter(|r| r.length > 0).collect();
    match rng.below(11) {
        8 => {
            let r = *rng.pick(&rails)?;
            let q = if rng.chance(2) { QUARTERS[0] } else { QUARTERS[2] };
            let pose = lay_pose([rng.range(0, r.length), 0, 0], q);
            let mode = *rng.pick(&["free", "push"])?;
            Some(
                cmd(json!({"device": device, "carrier": {"carrier": r.id}, "pose": pose, "mode": mode}))
                    .check(|_, after| placements_have_devices(after)),
            )
        }
        9 => {
            let surface = rng.pick(&l.surfaces)?;
            let pose = lay_pose(
                [LAY_MM * rng.range(0, 650), LAY_MM * rng.range(0, 900), 0],
                QUARTERS[0],
            );
            let mode = *rng.pick(&["push", "swap"])?;
            Some(cmd(
                json!({"device": device, "carrier": {"surface": surface}, "pose": pose, "mode": mode}),
            ))
        }
        10 => {
            let r = *rng.pick(&rails)?;
            let pose = at(rng.range(0, r.length));
            Some(cmd(
                json!({"device": device, "carrier": {"carrier": r.id}, "pose": pose, "mode": "swap"}),
            ))
        }
        0 => {
            let r = *rng.pick(&rails)?;
            Some(cmd(json!({"device": rng.uuid(), "carrier": {"carrier": r.id}, "pose": at(0)})).refuse())
        }
        1 => Some(cmd(json!({"device": device, "carrier": {"carrier": rng.uuid()}, "pose": at(0)})).refuse()),
        2 => {
            let r = *rng.pick(&rails)?;
            let s = r.length + 1 + rng.range(0, 1000);
            Some(cmd(
                json!({"device": device, "carrier": {"carrier": r.id}, "pose": at(s)}),
            ))
        }
        3 => {
            let r = *rng.pick(&rails)?;
            let pose = lay_pose([0, LAY_MM, 0], QUARTERS[0]);
            Some(cmd(
                json!({"device": device, "carrier": {"carrier": r.id}, "pose": pose}),
            ))
        }
        4 => {
            let d = rng.pick(&l.ducts)?;
            Some(cmd(
                json!({"device": device, "carrier": {"carrier": d}, "pose": at(0)}),
            ))
        }
        5 => {
            let surface = rng.pick(&l.surfaces)?;
            let t = [LAY_MM * rng.range(0, 650), LAY_MM * rng.range(0, 900), 0];
            let pose = lay_pose(t, *rng.pick(&QUARTERS)?);
            Some(
                cmd(json!({"device": device, "carrier": {"surface": surface}, "pose": pose}))
                    .counts(&[(Table::Placements, 1)])
                    .check(|_, after| placements_have_devices(after)),
            )
        }
        _ => {
            let r = *rng.pick(&rails)?;
            let q = if rng.chance(2) { QUARTERS[0] } else { QUARTERS[2] };
            let pose = lay_pose([rng.range(0, r.length), 0, 0], q);
            Some(
                cmd(json!({"device": device, "carrier": {"carrier": r.id}, "pose": pose}))
                    .counts(&[(Table::Placements, 1)])
                    .check(|_, after| placements_have_devices(after)),
            )
        }
    }
}

/// `layout.move`: along its rail within `[0, L]`, or on its plate (accepted); onto a plate (either: the device may be
/// on that plate already with its part and function); refused: a placement that does not exist, `s` beyond its rail's
/// end. WP-S4.02 part 1 (appended): along its rail with `mode` free, push or swap (either: a refusal of D5, a locked
/// placement); push or swap on its plate (refused). After a commit every placement has its device.
pub(super) fn layout_move(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let l = &live.layout;
    let cmd = |p: Value| Step::cmd("layout.move", p);
    if rng.chance(6) {
        return Some(
            cmd(json!({"placement": rng.uuid(), "pose": lay_pose([0, 0, 0], QUARTERS[0])})).refuse(),
        );
    }
    let p = rng.pick(&l.placements)?;
    match (&p.rail, &p.surface, rng.below(6)) {
        (Some((_, len)), _, 4 | 5) => {
            let pose = lay_pose([rng.range(0, *len), 0, 0], QUARTERS[0]);
            let mode = *rng.pick(&["free", "push", "swap"])?;
            Some(
                cmd(json!({"placement": p.id, "pose": pose, "mode": mode}))
                    .check(|_, after| placements_have_devices(after)),
            )
        }
        (None, Some(_), 4 | 5) => {
            let pose = lay_pose([10 * LAY_MM, 10 * LAY_MM, 0], QUARTERS[0]);
            let mode = *rng.pick(&["push", "swap"])?;
            Some(cmd(json!({"placement": p.id, "pose": pose, "mode": mode})))
        }
        (Some((_, len)), _, 0) => Some(cmd(
            json!({"placement": p.id, "pose": lay_pose([len + 1, 0, 0], QUARTERS[0])}),
        )),
        (Some((_, len)), _, 1 | 2) => {
            let pose = lay_pose([rng.range(0, *len), 0, 0], QUARTERS[2]);
            Some(cmd(json!({"placement": p.id, "pose": pose})).counts(&[(Table::Placements, 0)]))
        }
        (None, Some(_), 1 | 2) => {
            let t = [LAY_MM * rng.range(0, 650), LAY_MM * rng.range(0, 900), 0];
            let pose = lay_pose(t, *rng.pick(&QUARTERS)?);
            Some(cmd(json!({"placement": p.id, "pose": pose})).counts(&[(Table::Placements, 0)]))
        }
        _ => {
            let surface = rng.pick(&l.surfaces)?;
            let pose = lay_pose([10 * LAY_MM, 10 * LAY_MM, 0], QUARTERS[1]);
            Some(cmd(
                json!({"placement": p.id, "carrier": {"surface": surface}, "pose": pose}),
            ))
        }
    }
}

/// `layout.remove`: a placement goes (accepted); one that does not exist is refused.
pub(super) fn layout_remove(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let cmd = |p: Value| Step::cmd("layout.remove", p);
    if rng.chance(5) {
        return Some(cmd(json!({"placement": rng.uuid()})).refuse());
    }
    let p = rng.pick(&live.layout.placements)?;
    Some(cmd(json!({"placement": p.id})).counts(&[(Table::Placements, -1)]))
}

/// `layout.lock`: a placement locked or unlocked (accepted); one that does not exist is refused.
pub(super) fn layout_lock(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let cmd = |p: Value| Step::cmd("layout.lock", p);
    if rng.chance(5) {
        return Some(cmd(json!({"placement": rng.uuid(), "locked": true})).refuse());
    }
    let p = rng.pick(&live.layout.placements)?;
    Some(cmd(json!({"placement": p.id, "locked": rng.chance(2)})))
}

/// `page.surface_view.set`: a panel-layout page shows a surface at a datum, in either label mode
/// (accepted: the page's view is replaced in place, or a first one is added), some of the time with a scale the page
/// takes; refused: a page that does not exist, a page of another kind, a surface that is not in the
/// project, a scale of 1:0 (the schema's minimum).
pub(super) fn page_surface_view_set(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let cmd = |p: Value| Step::cmd("page.surface_view.set", p);
    let surface = rng.pick(&live.layout.surfaces)?.clone();
    let datum = [(rng.below(250) as i64) * LAY_MM, (rng.below(180) as i64) * LAY_MM];
    let mode = *rng.pick(&["abbreviated", "full"])?;
    let panels: Vec<&PageInfo> = live
        .page_info
        .iter()
        .filter(|p| p.kind == "panel_layout")
        .collect();
    let others: Vec<&PageInfo> = live
        .page_info
        .iter()
        .filter(|p| p.kind != "panel_layout")
        .collect();
    match rng.below(9) {
        0 => Some(cmd(json!({"page": rng.uuid(), "surface": surface, "datum_at": datum})).refuse()),
        // Refused by the registry's schema check: a scale of 1:0.
        3 => {
            let page = rng.pick(&panels)?;
            Some(
                cmd(json!({"page": page.id, "surface": surface, "datum_at": datum, "scale": {"one_to": 0}}))
                    .refuse(),
            )
        }
        1 => {
            let page = rng.pick(&others)?;
            Some(cmd(
                json!({"page": page.id, "surface": surface, "datum_at": datum}),
            ))
        }
        2 => {
            let page = rng.pick(&panels)?;
            Some(cmd(json!({"page": page.id, "surface": rng.uuid(), "datum_at": datum})).refuse())
        }
        _ => {
            let page = rng.pick(&panels)?;
            let mut p = json!({"page": page.id, "surface": surface, "datum_at": datum, "label_mode": mode});
            if rng.chance(3) {
                p["scale"] = if rng.chance(4) {
                    json!("not_to_scale")
                } else {
                    json!({"one_to": rng.range(1, 50)})
                };
            }
            Some(cmd(p))
        }
    }
}

/// The assembly root of a device, in the text form of its ID: the device itself when it has children,
/// else its parent; `None` for a device outside any assembly (the rule of `layout.place_assembly`).
pub(super) fn assembly_root_text(live: &Live, device: &str) -> Option<String> {
    if live.tree.iter().any(|t| t.parent.as_deref() == Some(device)) {
        return Some(device.to_owned());
    }
    live.tree
        .iter()
        .find(|t| id_text(&t.id) == device)?
        .parent
        .clone()
}

/// The rail (with its length) of a placed member of the assembly `root`, if one is on a rail.
pub(super) fn assembly_rail(live: &Live, root: &str) -> Option<(Value, i64)> {
    live.layout
        .placements
        .iter()
        .filter(|p| p.rail.is_some())
        .find(|p| assembly_root_text(live, &id_text(&p.device)).as_deref() == Some(root))
        .and_then(|p| p.rail.clone())
}

/// An assembly member for the WP-S4.02 part-2 arms, with the rail a member of its assembly sits on: mostly an unplaced
/// member whose assembly has a member on a rail (what `setup_unplace_member` leaves), else any member.
pub(super) fn assembly_member(live: &Live, rng: &mut SplitMix64) -> Option<(Value, Option<(Value, i64)>)> {
    let placed: std::collections::BTreeSet<String> = live
        .layout
        .placements
        .iter()
        .map(|p| id_text(&p.device))
        .collect();
    let members: Vec<(Value, String)> = live
        .tree
        .iter()
        .filter_map(|t| assembly_root_text(live, &id_text(&t.id)).map(|r| (t.id.clone(), r)))
        .collect();
    let ready: Vec<&(Value, String)> = members
        .iter()
        .filter(|(d, root)| !placed.contains(&id_text(d)) && assembly_rail(live, root).is_some())
        .collect();
    let (device, root) = if !ready.is_empty() && !rng.chance(4) {
        *rng.pick(&ready)?
    } else {
        rng.pick(&members)?
    };
    Some((device.clone(), assembly_rail(live, root)))
}

/// A placement on a rail of an assembly member goes (`layout.remove`), so that the member is unplaced and its
/// assembly has a member on a rail: what `layout.place_assembly` and `layout.auto_fill` act on in Golden A.
pub(super) fn setup_unplace_member(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let on_rails: Vec<&super::super::live::PlacementInfo> = live
        .layout
        .placements
        .iter()
        .filter(|p| p.rail.is_some() && assembly_root_text(live, &id_text(&p.device)).is_some())
        .collect();
    let p = *rng.pick(&on_rails)?;
    Some(Step::cmd("layout.remove", json!({"placement": p.id})))
}

/// `layout.place_assembly`: the unplaced members of an assembly go onto the rail one of its
/// members sits on, else onto a rail at `s` in free or push mode (either: a member may not mount or fit, the rail may
/// lack room, every member may be placed); refused: a device that does not exist, a surface, swap, a device outside
/// any assembly. After a commit every placement has its device and its carrier or surface.
pub(super) fn layout_place_assembly(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let l = &live.layout;
    let cmd = |p: Value| Step::cmd("layout.place_assembly", p);
    let rails: Vec<&super::super::live::RailInfo> = l.rails.iter().filter(|r| r.length > 0).collect();
    match rng.below(9) {
        0 => {
            let r = *rng.pick(&rails)?;
            Some(cmd(json!({"device": rng.uuid(), "carrier": {"carrier": r.id}, "s": 0})).refuse())
        }
        1 => {
            let (device, _) = assembly_member(live, rng)?;
            let surface = rng.pick(&l.surfaces)?;
            Some(cmd(
                json!({"device": device, "carrier": {"surface": surface}, "s": 0}),
            ))
        }
        2 => {
            let (device, _) = assembly_member(live, rng)?;
            let r = *rng.pick(&rails)?;
            Some(cmd(
                json!({"device": device, "carrier": {"carrier": r.id}, "s": 0, "mode": "swap"}),
            ))
        }
        3 => {
            let alone: Vec<&Value> = live
                .plain_devices
                .iter()
                .filter(|d| assembly_root_text(live, &id_text(d)).is_none())
                .collect();
            let device = *rng.pick(&alone)?;
            let r = *rng.pick(&rails)?;
            Some(cmd(
                json!({"device": device, "carrier": {"carrier": r.id}, "s": 0}),
            ))
        }
        _ => {
            let (device, rail) = assembly_member(live, rng)?;
            let (rail, length) = match rail {
                Some(r) if !rng.chance(4) => r,
                _ => {
                    let r = *rng.pick(&rails)?;
                    (r.id.clone(), r.length)
                }
            };
            let mode = *rng.pick(&["free", "push"])?;
            let p = json!({"device": device, "carrier": {"carrier": rail}, "s": rng.range(0, length), "mode": mode});
            Some(cmd(p).check(|_, after| placements_have_devices(after)))
        }
    }
}

/// `layout.auto_fill`: the unplaced devices of a rail's space that fit it, or a listed device,
/// next-fit in `dt` or `schematic` order (either: nothing may fit or have room); refused: a carrier that does not
/// exist, a duct, a device listed twice, a device that does not exist. After a commit every placement has its device
/// and its carrier or surface.
pub(super) fn layout_auto_fill(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let l = &live.layout;
    let cmd = |p: Value| Step::cmd("layout.auto_fill", p);
    let order = *rng.pick(&["dt", "schematic"])?;
    let rails: Vec<&super::super::live::RailInfo> = l.rails.iter().filter(|r| r.length > 0).collect();
    match rng.below(9) {
        0 => Some(cmd(json!({"carrier": rng.uuid(), "order": order})).refuse()),
        1 => Some(cmd(json!({"carrier": rng.pick(&l.ducts)?, "order": order}))),
        2 => {
            let r = *rng.pick(&rails)?;
            let d = rng.pick(&live.devices)?;
            Some(cmd(json!({"carrier": r.id, "devices": [d, d], "order": order})))
        }
        3 => {
            let r = *rng.pick(&rails)?;
            Some(cmd(json!({"carrier": r.id, "devices": [rng.uuid()], "order": order})).refuse())
        }
        _ => {
            let (device, rail) = assembly_member(live, rng)?;
            let rail = match rail {
                Some((r, _)) if !rng.chance(4) => r,
                _ => rng.pick(&rails)?.id.clone(),
            };
            let mut p = json!({"carrier": rail, "order": order});
            if rng.chance(3) {
                p["devices"] = json!([device]);
            }
            Some(cmd(p).check(|_, after| placements_have_devices(after)))
        }
    }
}

/// `layout.enclosure.create_from_part`: a new part copy with an enclosure section (no plate,
/// embedded through `copies`) on a location without a layout space gives a space, an enclosure device, an enclosure and
/// its plate surface (accepted); refused: a location that does not exist, a node of another aspect, a part without an
/// enclosure section.
pub(super) fn layout_enclosure_create_from_part(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let cmd = |p: Value| Step::cmd("layout.enclosure.create_from_part", p);
    let (_, template) = rng.pick(&live.fixture.part_json)?;
    let mut t = template.clone();
    if !t["sections"].is_object() {
        t["sections"] = json!({});
    }
    let branch = rng.below(6);
    if branch == 2 {
        if let Some(s) = t["sections"].as_object_mut() {
            s.remove("enclosure");
        }
    } else {
        t["sections"]["enclosure"] =
            json!({"width": 600 * LAY_MM, "height": 800 * LAY_MM, "depth": 300 * LAY_MM});
    }
    let c = relineaged(&t, &rng.uuid(), &rng.uuid())?;
    let part = c["id"].clone();
    match branch {
        0 => Some(cmd(json!({"location": rng.uuid(), "part": part, "copies": [c]})).refuse()),
        1 => {
            let other: Vec<&super::super::live::NodeInfo> =
                live.nodes.iter().filter(|n| n.aspect != "location").collect();
            Some(cmd(
                json!({"location": rng.pick(&other)?.id, "part": part, "copies": [c]}),
            ))
        }
        2 => {
            let location = rng.pick(&live.layout.free_locations)?;
            Some(cmd(json!({"location": location, "part": part, "copies": [c]})))
        }
        _ => {
            let location = rng.pick(&live.layout.free_locations)?;
            Some(
                cmd(json!({"location": location, "part": part, "copies": [c], "pe_bond": rng.chance(2)}))
                    .counts(&[
                        (Table::Parts, 1),
                        (Table::Devices, 1),
                        (Table::LayoutSpaces, 1),
                        (Table::Enclosures, 1),
                        (Table::Surfaces, 1),
                    ])
                    .check(|_, after| placements_have_devices(after)),
            )
        }
    }
}

/// `layout.space.edit` (WP-S4.02 part 2, FT-840): a space renamed or its name cleared (accepted); refused: a space that
/// does not exist, no name member.
pub(super) fn layout_space_edit(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let cmd = |p: Value| Step::cmd("layout.space.edit", p);
    match rng.below(6) {
        0 => Some(cmd(json!({"space": rng.uuid(), "name": rng.word(8)})).refuse()),
        1 => Some(cmd(json!({"space": rng.pick(&live.layout.spaces)?}))),
        2 => Some(
            cmd(json!({"space": rng.pick(&live.layout.spaces)?, "name": null}))
                .counts(&[(Table::LayoutSpaces, 0)]),
        ),
        _ => Some(
            cmd(json!({"space": rng.pick(&live.layout.spaces)?, "name": rng.word(8)}))
                .counts(&[(Table::LayoutSpaces, 0)]),
        ),
    }
}

/// `layout.enclosure.edit` (WP-S4.02 part 2, FT-840): an enclosure moved in its space (accepted); refused: an enclosure
/// that does not exist, a turned pose.
pub(super) fn layout_enclosure_edit(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let cmd = |p: Value| Step::cmd("layout.enclosure.edit", p);
    let t = [LAY_MM * rng.range(0, 2000), LAY_MM * rng.range(0, 500), 0];
    match rng.below(6) {
        0 => Some(cmd(json!({"enclosure": rng.uuid(), "pose": lay_pose(t, QUARTERS[0])})).refuse()),
        1 => {
            let e = rng.pick(&live.layout.enclosures)?;
            Some(cmd(json!({"enclosure": e, "pose": lay_pose(t, QUARTERS[1])})))
        }
        _ => {
            let e = rng.pick(&live.layout.enclosures)?;
            Some(
                cmd(json!({"enclosure": e, "pose": lay_pose(t, QUARTERS[0])}))
                    .counts(&[(Table::Enclosures, 0)])
                    .check(|_, after| placements_have_devices(after)),
            )
        }
    }
}

/// `layout.surface.edit` (WP-S4.02 part 2, FT-840): a surface's PE bond set (accepted); refused: a surface that does
/// not exist.
pub(super) fn layout_surface_edit(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let cmd = |p: Value| Step::cmd("layout.surface.edit", p);
    if rng.chance(5) {
        return Some(cmd(json!({"surface": rng.uuid(), "pe_bond": true})).refuse());
    }
    let s = rng.pick(&live.layout.surfaces)?;
    Some(cmd(json!({"surface": s, "pe_bond": rng.chance(2)})).counts(&[(Table::Surfaces, 0)]))
}

/// `edit.delete` of one layout record (WP-S4.02 part 2, FT-840): a placement goes; an empty carrier, enclosure or space
/// goes with what it contains; refused: an occupied carrier, enclosure or space, a plate surface without its
/// enclosure. After a commit every placement has its device and its carrier or surface.
pub(super) fn edit_delete_layout(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let l = &live.layout;
    let del =
        |kind: &str, id: &Value| Step::cmd("edit.delete", json!({"targets": [{"kind": kind, "id": id}]}));
    let step = match rng.below(5) {
        0 => {
            let p = rng.pick(&l.placements)?;
            del("placement", &p.id).counts(&[(Table::Placements, -1)])
        }
        1 => {
            let (c, n) = rng.pick(&l.carriers)?;
            if *n == 0 {
                del("carrier", c).counts(&[(Table::Carriers, -1)])
            } else {
                del("carrier", c)
            }
        }
        2 => del("mounting_surface", rng.pick(&l.surfaces)?),
        3 => {
            let e = rng.pick(&l.enclosures)?;
            if l.occupied.contains(&id_text(e)) {
                del("enclosure", e)
            } else {
                del("enclosure", e).counts(&[(Table::Enclosures, -1), (Table::Surfaces, -1)])
            }
        }
        _ => {
            let s = rng.pick(&l.spaces)?;
            if l.occupied.contains(&id_text(s)) {
                del("layout_space", s)
            } else {
                del("layout_space", s).counts(&[(Table::LayoutSpaces, -1)])
            }
        }
    };
    Some(step.check(|_, after| placements_have_devices(after)))
}
