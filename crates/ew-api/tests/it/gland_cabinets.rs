// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A cable between two cabinets has a gland in each (doc 12 §3.9). The sample's -103W1 runs from +CAB to its motor in
//! +FLD; with a cabinet in +FLD too, its gland placed in +CAB says nothing of +FLD: there it waits, EW-LAY-018 says so
//! under a fingerprint of its own, Place cable glands places it, and each bottom draws its own hole. A cable named twice
//! is refused. When the motor moves into +CAB the cable leaves no cabinet, and each of its holes is EW-LAY-018's, with
//! Remove the hole; Keep it as a hole quiets it.

use crate::common;

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::{AnyCommand, Session};
use ew_derive::layout::{CableGlands, Owner, SurfaceFeatures};
use ew_ids::{Id, Uuid};
use ew_schema::{Enclosure, LayoutSpace, MountingSurface, SurfaceFeature, SurfaceKind};
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, params: Value) -> Result<(), ew_api::CmdError> {
    let cmd = AnyCommand {
        name: name.into(),
        params,
    };
    match s.execute(&cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(&cmd).and_then(|p| s.commit(p)),
        other => other,
    }
    .map(drop)
}

fn node(s: &Session, code: &str) -> Uuid {
    s.query()
        .structure_nodes()
        .iter()
        .find(|(_, n)| n.code.as_str() == code)
        .map(|(id, _)| id.as_uuid())
        .expect("the location")
}

/// The bottom of the cabinet of location `code`.
fn bottom(s: &Session, code: &str) -> (Id<LayoutSpace>, Id<MountingSurface>) {
    let q = s.query();
    let location = node(s, code);
    let space = q
        .table::<LayoutSpace>()
        .iter()
        .find(|(_, l)| l.location.as_uuid() == location)
        .map(|(id, _)| id)
        .expect("its space");
    let bottom = q
        .table::<MountingSurface>()
        .iter()
        .find(|(_, m)| {
            m.kind == SurfaceKind::Bottom
                && q.table::<Enclosure>()
                    .get(m.enclosure)
                    .is_some_and(|e| e.space == space)
        })
        .map(|(id, _)| id)
        .expect("its bottom");
    (space, bottom)
}

/// Whether the bottom `surface` draws a gland hole for `cable`.
fn drawn_for(s: &Session, surface: Id<MountingSurface>, cable: Uuid) -> bool {
    let q = s.query();
    let features = q.table::<SurfaceFeature>();
    q.get::<SurfaceFeatures>(surface).iter().any(|f| {
        f.owners.iter().any(|o| match o {
            Owner::ByHand { feature } => features
                .get(*feature)
                .is_some_and(|x| x.cable.map(|c| c.as_uuid()) == Some(cable)),
            _ => false,
        })
    })
}

fn gland_findings(s: &Session, form: &str) -> Vec<ew_api::checks::Finding> {
    s.check(&PRE_DELIVERY, CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-LAY-018" && f.discriminator.starts_with(form))
        .collect()
}

#[test]
fn a_cable_between_two_cabinets_has_a_gland_in_each() {
    let mut s = common::sample();
    let cable = s.resolve_reference("tag:-103W1").expect("-103W1");
    let part = s
        .layout_part_offers("enclosure")
        .into_iter()
        .find(|o| o.number == "EW-GEN-ENC-1" && o.copies.is_empty())
        .expect("the project's enclosure part")
        .part;
    let field = node(&s, "FLD");
    run(
        &mut s,
        "layout.enclosure.create_from_part",
        json!({"location": field.to_string(), "part": part.to_string()}),
    )
    .expect("a cabinet in +FLD");
    let enclosure = {
        let q = s.query();
        q.table::<Enclosure>()
            .iter()
            .find(|(_, e)| {
                q.table::<LayoutSpace>()
                    .get(e.space)
                    .is_some_and(|l| l.location.as_uuid() == field)
            })
            .map(|(id, _)| id.as_uuid())
            .expect("its enclosure")
    };
    run(
        &mut s,
        "layout.surface.add",
        json!({"enclosure": enclosure.to_string(), "kind": "bottom"}),
    )
    .expect("its bottom");
    let (cab, cab_bottom) = bottom(&s, "CAB");
    let (fld, fld_bottom) = bottom(&s, "FLD");
    let placed = |s: &Session, space| {
        s.query()
            .get::<CableGlands>(space)
            .iter()
            .find(|g| g.cable.as_uuid() == cable)
            .map(|g| g.placed.is_some())
    };
    assert_eq!(placed(&s, cab), Some(true), "placed in +CAB");
    assert_eq!(placed(&s, fld), Some(false), "waiting in +FLD");
    let unplaced = gland_findings(&s, "unplaced");
    assert_eq!(unplaced.len(), 1, "{unplaced:#?}");
    assert!(unplaced[0].discriminator.ends_with(&fld.as_uuid().to_string()));

    // Named twice: refused in words.
    let twice = run(
        &mut s,
        "layout.glands_place",
        json!({"surface": fld_bottom.as_uuid().to_string(), "at": [80_000_000, 125_000_000],
            "cables": [cable.to_string(), cable.to_string()]}),
    );
    assert!(twice.is_err(), "a cable named twice");
    run(
        &mut s,
        "layout.glands_place",
        json!({"surface": fld_bottom.as_uuid().to_string(), "at": [80_000_000, 125_000_000],
            "cables": [cable.to_string()]}),
    )
    .expect("its gland in +FLD");
    assert_eq!(placed(&s, fld), Some(true));
    assert!(drawn_for(&s, cab_bottom, cable), "+CAB's bottom draws its hole");
    assert!(drawn_for(&s, fld_bottom, cable), "+FLD's bottom draws its own");
    assert!(gland_findings(&s, "unplaced").is_empty());

    // The motor moves into +CAB: the cable runs inside +CAB only, and its hole there serves none.
    let motor = s.resolve_reference("tag:-103M1").expect("the motor");
    let cab = node(&s, "CAB");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [motor.to_string()], "path": "structure.location",
            "value": cab.to_string()}),
    )
    .expect("moved");
    // Both its holes now serve no cable that leaves their cabinet: +CAB's and +FLD's.
    let inside = gland_findings(&s, "inside");
    assert_eq!(inside.len(), 2, "{inside:#?}");
    for f in &inside {
        let fix = f.fix.clone().expect("Remove the hole");
        assert_eq!(fix.command, "edit.delete");
        // Keep it as a hole: the hole stays, made by hand, and nothing is said.
        let feature = f.targets[0].id.as_uuid();
        run(
            &mut s,
            "layout.feature.edit",
            json!({"feature": feature.to_string(), "for_cable": false}),
        )
        .expect("kept as a hole");
    }
    assert!(gland_findings(&s, "inside").is_empty());
    assert!(gland_findings(&s, "no_cable").is_empty());
}
