// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A hole moved or left out names its part's hole (doc 12 §2.2): when the part's holes change, by an edit in Front
//! view and holes or by a library update that replaces the copy, the choice stays on that hole; when the part no longer
//! has it, the choice applies to no hole and EW-LAY-017 says so, its fix dropping it.

use crate::common;

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::{AnyCommand, Session};
use ew_ids::{Id, Uuid};
use ew_schema::{LibDocument, MountingSurface, Placement, SurfaceKind};
use ew_units::Nm;
use serde_json::{Value, json};

fn run(s: &mut Session, cmd: &AnyCommand) {
    match s.execute(cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(cmd).and_then(|p| s.commit(p)),
        other => other,
    }
    .expect(&cmd.name);
}

fn command(name: &str, params: Value) -> AnyCommand {
    AnyCommand {
        name: name.into(),
        params,
    }
}

/// The door's placement whose part is the copy numbered `number`.
fn placement_of(s: &Session, number: &str) -> Id<Placement> {
    let q = s.query();
    let door = q
        .table::<MountingSurface>()
        .iter()
        .find(|(_, f)| f.kind == SurfaceKind::Door)
        .map(|(id, _)| id)
        .expect("the door");
    q.table::<Placement>()
        .iter()
        .find(|(id, _)| {
            q.get::<ew_derive::layout::PlacementGeometry>(*id)
                .as_ref()
                .as_ref()
                .is_some_and(|g| {
                    g.surface == door
                        && g.part.as_ref().is_some_and(|p| {
                            q.parts()
                                .get(p.copy)
                                .is_some_and(|c| c.item.part_number.as_str() == number)
                        })
                })
        })
        .map(|(id, _)| id)
        .expect("placed on the door")
}

/// The pattern index of the placement's part hole whose centre is `x` mm across.
fn index_at(s: &Session, p: Id<Placement>, x: f64) -> usize {
    let x = Nm((x * 1e6).round() as i64);
    ew_derive::layout::part_holes(&s.query(), p)
        .iter()
        .position(|h| h.x == x)
        .expect("a hole there")
}

/// The placement's holes on the door's drawings, by their part holes' `x` in tenths of a millimetre.
fn drawn(s: &Session, p: Id<Placement>) -> Vec<i64> {
    let q = s.query();
    let holes = ew_derive::layout::part_holes(&q, p);
    let placement = q.table::<Placement>().get(p).cloned().expect("the placement");
    (0..holes.len())
        .filter(|i| {
            ew_derive::layout::choice_for(&placement.hole_choices, *i, holes.get(*i)).is_none_or(|c| !c.omit)
        })
        .map(|i| holes[i].x.0 / 100_000)
        .collect()
}

fn lost(s: &Session) -> Vec<ew_api::checks::Finding> {
    s.check(&PRE_DELIVERY, CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-LAY-017" && f.discriminator.starts_with("lost:"))
        .collect()
}

#[test]
fn a_library_update_keeps_a_choice_on_its_hole_and_one_whose_hole_is_gone_is_said() {
    let mut s = common::sample();
    let handle = placement_of(&s, "EW-GEN-HDL-1");
    // The handle's right fixing (56.5 mm) is left out.
    let right = index_at(&s, handle, 56.5);
    run(
        &mut s,
        &command(
            "layout.hole_choice",
            json!({"placement": handle.as_uuid().to_string(), "index": right, "omit": true}),
        ),
    );
    assert_eq!(drawn(&s, handle), [325, 85], "the cut-out and the left fixing");
    // An update of the handle's copy drops its cut-out: the holes after it count one less.
    let copy = s
        .query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == "EW-GEN-HDL-1")
        .map(|(id, c)| (id, LibDocument::Part((*c).as_ref().clone())))
        .expect("the handle's copy");
    let mut doc = serde_json::to_value(&copy.1).unwrap();
    let holes = doc["mech"]["holes"].as_array().cloned().unwrap();
    let without = |x: i64| -> Vec<Value> {
        holes
            .iter()
            .filter(|h| h["x"].as_i64() != Some(x))
            .cloned()
            .collect()
    };
    doc["mech"]["holes"] = json!(without(32_500_000));
    run(
        &mut s,
        &command("library.update", json!({"copies": [rehashed(doc.clone())]})),
    );
    assert_eq!(
        drawn(&s, handle),
        [85],
        "still the right fixing left out, not the left one"
    );
    assert!(lost(&s).is_empty());
    // A second update drops the right fixing itself: the choice applies to none, and EW-LAY-017 says so.
    let mut doc2 = doc;
    doc2["mech"]["holes"] = json!(
        without(32_500_000)
            .into_iter()
            .filter(|h| h["x"].as_i64() != Some(56_500_000))
            .collect::<Vec<_>>()
    );
    doc2["version"] = json!("9.1.0");
    run(
        &mut s,
        &command("library.update", json!({"copies": [with_hash(doc2)]})),
    );
    assert_eq!(drawn(&s, handle), [85]);
    let found = lost(&s);
    assert_eq!(found.len(), 1, "{found:#?}");
    let fix = found[0].fix.clone().expect("Drop the choices");
    run(&mut s, &command(&fix.command, fix.params));
    assert!(lost(&s).is_empty());
    assert!(
        s.query()
            .table::<Placement>()
            .get(handle)
            .is_some_and(|p| p.hole_choices.is_empty())
    );
    let _ = copy.0;
}

#[test]
fn a_part_edited_in_front_view_and_holes_keeps_a_choice_on_its_hole() {
    let mut s = common::sample();
    // A lamp of one's own on -102E1's place in the door: a cut-out and two M4 fixings.
    let mut d = s.new_part_draft().expect("a new part");
    d.to_library = false;
    for (f, v) in [
        ("manufacturer", "Acme"),
        ("part_number", "OWN-LAMP-40"),
        ("mech.width", "40"),
        ("mech.height", "40"),
        ("mech.mount_kind", "door_cutout"),
    ] {
        s.set_draft_field(&mut d, f, v).expect(f);
    }
    for (i, (x, kind, size)) in [
        ("20", "cutout", "22 mm"),
        ("5", "fixing", "M4"),
        ("35", "fixing", "M4"),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(d.add_table_row("mech.holes"));
        for (f, v) in [("x", x), ("y", "20"), ("hole.kind", kind), ("hole.size", size)] {
            s.set_draft_field(&mut d, &format!("mech.holes[{i}].{f}"), v)
                .expect(f);
        }
    }
    let made = s.new_part_command(&d).expect("the new part's command");
    run(&mut s, &made.command);
    let copy: Uuid = s
        .query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == "OWN-LAMP-40")
        .map(|(id, _)| id.as_uuid())
        .expect("held");
    let lamp: Uuid = s.resolve_reference("tag:-102E1").expect("-102E1");
    let old = s
        .query()
        .table::<Placement>()
        .iter()
        .find(|(_, p)| p.device.as_uuid() == lamp)
        .map(|(id, _)| id.as_uuid())
        .expect("placed");
    run(
        &mut s,
        &command(
            "fields.set",
            json!({"kind": "device", "targets": [lamp.to_string()], "path": "parts",
                "value": [{"part": {"copy": copy.to_string()}, "qty": "1"}]}),
        ),
    );
    run(
        &mut s,
        &command(
            "edit.delete",
            json!({"targets": [{"kind": "placement", "id": old.to_string()}]}),
        ),
    );
    let door = s
        .query()
        .table::<MountingSurface>()
        .iter()
        .find(|(_, f)| f.kind == SurfaceKind::Door)
        .map(|(id, _)| id.as_uuid())
        .expect("the door");
    run(
        &mut s,
        &command(
            "layout.place",
            json!({"device": lamp.to_string(), "part": copy.to_string(), "carrier": {"surface": door.to_string()},
                "pose": {"t": [240_000_000, 560_000_000, 0], "q": [1_000_000_000, 0, 0, 0]}}),
        ),
    );
    let placement = placement_of(&s, "OWN-LAMP-40");
    let right = index_at(&s, placement, 35.0);
    run(
        &mut s,
        &command(
            "layout.hole_choice",
            json!({"placement": placement.as_uuid().to_string(), "index": right, "omit": true}),
        ),
    );
    // Front view and holes: the cut-out's row removed and saved.
    let mut d = s.part_draft(copy).expect("its draft");
    assert!(d.remove_table_row("mech.holes", 0));
    let edit = s.part_edit_command(&d).expect("the edit");
    run(&mut s, &edit);
    assert_eq!(
        drawn(&s, placement),
        [50],
        "the left fixing; the right one still left out"
    );
    assert!(lost(&s).is_empty());
}

/// A changed library document: version 9.0.0 and the hash of its item.
fn rehashed(mut v: Value) -> Value {
    v["version"] = json!("9.0.0");
    with_hash(v)
}

fn with_hash(v: Value) -> Value {
    let doc = LibDocument::from_value(v).unwrap();
    let hash = ew_format::library_copy_hash(&doc).unwrap();
    let mut v = serde_json::to_value(&doc).unwrap();
    v["sha256"] = json!(ew_schema::Sha256(hash).to_string());
    v
}
