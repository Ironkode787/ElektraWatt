// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Another cabinet for an enclosure (`layout.enclosure.set_part`, doc 12 §3.1): what stands on its surfaces keeps its
//! place measured from each surface's datum. The sample's -UC1 carries rails, a duct and devices on its plate, devices
//! on its door, an earth stud on the plate and glands in its bottom; given a larger and then a smaller cabinet of one's
//! own, every rail point, placement and hole is where it was, measured from its surface's datum corner, and what no
//! longer lies inside its surface is named by the layout's own check before Apply.

use std::collections::BTreeMap;

use crate::common;

use ew_api::{PartPick, Session};
use ew_derive::layout::from_datum;
use ew_ids::{Id, Uuid};
use ew_schema::{Carrier, CarrierRef, Enclosure, MountingSurface, Placement, SurfaceFeature};

/// A part of the project's own of kind `kind` with the fields `fields`, and `doors` doors hinged left.
fn own_part(s: &mut Session, kind: &str, fields: &[(&str, String)], doors: usize) -> Uuid {
    let mut d = s.new_part_draft().expect("a new part");
    d.to_library = false;
    s.set_draft_field(&mut d, "kind", kind).expect("kind");
    s.set_draft_field(&mut d, "manufacturer", "Acme")
        .expect("maker");
    for (f, v) in fields {
        s.set_draft_field(&mut d, f, v).unwrap_or_else(|e| panic!("{f}: {e:?}"));
    }
    for i in 0..doors {
        assert!(d.add_table_row("sections.enclosure.doors"));
        s.set_draft_field(&mut d, &format!("sections.enclosure.doors[{i}].hinge"), "left")
            .expect("hinge");
    }
    let number = fields
        .iter()
        .find(|(f, _)| *f == "part_number")
        .map(|(_, v)| v.clone())
        .expect("a number");
    let made = s.new_part_command(&d).expect("the new part's command");
    common::run_cmd(s, &made.command.name, made.command.params.clone());
    s.query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == number)
        .map(|(id, _)| id.as_uuid())
        .expect("held")
}

/// A cabinet of `w` × `h` × `d` mm with one door and its plate of `pw` × `ph` mm, numbered `number`.
fn cabinet(s: &mut Session, number: &str, (w, h, d): (i128, i128, i128), (pw, ph): (i128, i128)) -> Uuid {
    let plate = format!("{number}-MP");
    own_part(
        s,
        "mounting_plate",
        &[
            ("part_number", plate.clone()),
            ("sections.mounting_plate.width", pw.to_string()),
            ("sections.mounting_plate.height", ph.to_string()),
        ],
        0,
    );
    own_part(
        s,
        "enclosure",
        &[
            ("part_number", number.to_owned()),
            ("sections.enclosure.width", w.to_string()),
            ("sections.enclosure.height", h.to_string()),
            ("sections.enclosure.depth", d.to_string()),
            ("sections.enclosure.plate", plate),
        ],
        1,
    )
}

/// Every point that stands on a surface of -UC1, measured from that surface's datum: each rail's and duct's path, each
/// placement on a surface (and on a rail, by its distance along it), each hole and gland.
fn positions(s: &Session) -> BTreeMap<String, (i128, i128)> {
    let q = s.query();
    let surfaces = q.table::<MountingSurface>();
    let measured = |surface: Id<MountingSurface>, (u, v): (i128, i128)| {
        let m = surfaces.get(surface).expect("the surface");
        from_datum(&q, m, (u, v))
    };
    let mut out = BTreeMap::new();
    for (id, c) in q.table::<Carrier>().iter() {
        for (i, p) in c.path.iter().enumerate() {
            out.insert(
                format!("carrier {id} point {i}"),
                measured(c.surface, (i128::from(p.x.0), i128::from(p.y.0))),
            );
        }
    }
    for (id, p) in q.table::<Placement>().iter() {
        let [u, v, _] = p.pose.t;
        let (u, v) = (i128::from(u.0), i128::from(v.0));
        let at = match &p.carrier {
            CarrierRef::Surface(surface) => measured(*surface, (u, v)),
            _ => (u, v),
        };
        out.insert(format!("placement {id}"), at);
    }
    for (id, f) in q.table::<SurfaceFeature>().iter() {
        out.insert(
            format!("feature {id}"),
            measured(f.surface, (i128::from(f.at.x.0), i128::from(f.at.y.0))),
        );
    }
    out
}

/// Gives -UC1 the cabinet `copy` as Change its cabinet… does: the plan's card text, then Apply.
fn give(s: &mut Session, copy: Uuid) -> ew_api::output::EnclosurePartPlan {
    let device = s.resolve_reference("tag:-UC1").expect("-UC1");
    let plan = s
        .enclosure_part_plan(device, &PartPick::Held(copy))
        .expect("a cabinet for an enclosure's device")
        .expect("its plan");
    common::run_cmd(s, &plan.command.name, plan.command.params.clone());
    plan
}

/// The sizes of -UC1's stored surfaces, by kind.
fn sizes(s: &Session) -> Vec<String> {
    let q = s.query();
    let enclosure = q.table::<Enclosure>().iter().next().map(|(id, _)| id).expect("-UC1's enclosure");
    let mut out: Vec<String> = (q.table::<MountingSurface>().iter())
        .filter(|(_, m)| m.enclosure == enclosure)
        .map(|(_, m)| format!("{:?} {:?}", m.kind, ew_derive::layout::sheet_size(m)))
        .collect();
    out.sort();
    out
}

#[test]
fn another_cabinet_keeps_every_place_from_the_datum() {
    let mut s = common::sample();
    let larger = cabinet(&mut s, "ACME-CAB-L", (800, 1000, 300), (760, 960));
    let smaller = cabinet(&mut s, "ACME-CAB-S", (400, 500, 200), (360, 460));
    let before = positions(&s);
    for what in ["carrier", "placement", "feature"] {
        assert!(before.keys().any(|k| k.starts_with(what)), "the sample's {what}s: {before:?}");
    }
    let sizes_before = sizes(&s);
    let plan = give(&mut s, larger);
    assert!(!plan.sizes.is_empty(), "the card names the sizes that change");
    assert!(plan.outside.is_empty(), "nothing lies outside a larger cabinet: {:?}", plan.outside);
    assert_ne!(sizes(&s), sizes_before, "the surfaces took the larger cabinet's size");
    assert_eq!(positions(&s), before, "larger: every place as it was");
    let plan = give(&mut s, smaller);
    assert_eq!(positions(&s), before, "smaller: every place as it was");
    assert!(
        !plan.outside.is_empty(),
        "the card names what no longer lies inside its surface"
    );
}
