// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! On the sample (doc 12 §3): `layout.place_assembly` beside `layout.place` with push on rail 2 (AS-1), the CPU
//! station -104A1 with its two cards placed as one block on an emptied rail 2 in its member order (AS-2),
//! `layout.auto_fill` on an emptied rail 1 with and without clearance pads (AS-3), and EW-LAY-003's quick fix run
//! through the session (AS-8). Positions are clip points in tenths of a millimetre; on rail 2 the CPU (100 mm) and its
//! cards (25 mm each) stand from its start, then the valve terminal and its valves.

use crate::common;

use ew_api::{AnyCommand, CmdError, Origin, Session};
use ew_derive::Q;
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::layout::member_order;
use ew_schema::{Carrier, Clearance, Device, LibCopy, LibDocument, Part, PartRef, Placement, Record, Sha256};
use ew_units::Nm;
use serde_json::{Value, json};

const MM: i128 = 1_000_000;

fn dev(s: &Session, tag: &str) -> Id<Device> {
    Id::from_uuid(s.resolve_reference(&format!("tag:{tag}")).expect("the device"))
}

fn id_json<T>(id: Id<T>) -> Value {
    serde_json::to_value(id).expect("an ID serialises")
}

/// The sample over its records with `unplace`'s placement removed; rail 1 and rail 2.
fn session_without(unplace: Option<&str>) -> (Session, Id<Carrier>, Id<Carrier>) {
    let gone = unplace.map(|t| {
        common::sample()
            .resolve_reference(&format!("tag:{t}"))
            .expect("the device")
    });
    let (project, mut records, lib) = common::sample_records();
    records.retain(|r| !matches!(r, Record::Placement(p) if Some(p.device.as_uuid()) == gone));
    let s = common::session_of(project, records, lib);
    let tables = s.panel_tables("902").expect("902");
    let (r1, r2) = (tables.carriers[0].carrier, tables.carriers[1].carrier);
    (s, r1, r2)
}

fn session() -> (Session, Id<Carrier>, Id<Carrier>) {
    session_without(None)
}

/// Runs a command; its receipt's notices, or the refusal.
fn run(s: &mut Session, name: &str, params: Value) -> Result<Vec<String>, CmdError> {
    let cmd = AnyCommand {
        name: name.into(),
        params,
    };
    s.execute(&cmd).map(|r| r.outcome.notices)
}

/// A device's short tag as the session prints it.
fn tag(q: &Q, device: Id<Device>) -> String {
    q.get::<ew_derive::DeviceTag>(device)
        .as_ref()
        .as_ref()
        .map(ew_derive::Tag::short)
        .unwrap_or_default()
}

/// Every placement of a rail with its clip point in tenths of a millimetre, by tag, along the rail.
fn clips(s: &Session, rail: Id<Carrier>) -> Vec<(String, i128)> {
    let q = s.query();
    let mut out: Vec<(String, i128)> = q
        .table::<Placement>()
        .iter()
        .filter(|(_, p)| p.carrier == ew_schema::CarrierRef::Carrier(rail))
        .map(|(_, p)| (tag(&q, p.device), i128::from(p.pose.t[0].0) * 10 / MM))
        .collect();
    out.sort_by_key(|(_, s)| *s);
    out
}

fn owned(v: &[(&str, i128)]) -> Vec<(String, i128)> {
    v.iter().map(|(t, c)| ((*t).to_owned(), *c)).collect()
}

/// Removes every placement of a rail (`layout.remove`, one step each).
fn empty(s: &mut Session, rail: Id<Carrier>) -> usize {
    let on: Vec<Id<Placement>> = s
        .query()
        .table::<Placement>()
        .iter()
        .filter(|(_, p)| p.carrier == ew_schema::CarrierRef::Carrier(rail))
        .map(|(id, _)| id)
        .collect();
    for p in &on {
        run(s, "layout.remove", json!({"placement": id_json(*p)})).unwrap();
    }
    on.len()
}

/// The CPU station -104A1 in its member order: the station, then its cards by slot.
const MEMBERS: [&str; 3] = ["-104A1", "-104A1-A1", "-104A1-A2"];

/// AS-1: with the output card -104A1-A2 unplaced, `layout.place` of it with push at its old clip 137.5 mm, and
/// `layout.place_assembly` of -104A1 (its one unplaced member goes after its predecessor -104A1-A1), each give the
/// card at 137.5 mm, every other placement unchanged (the gap it left takes it); one undo restores all.
#[test]
fn as1_the_unplaced_member_goes_after_its_predecessor() {
    let (s, _, r2) = session_without(Some("-104A1-A2"));
    let before = clips(&s, r2);
    let mut want = before.clone();
    want.push(("-104A1-A2".to_owned(), 1375));
    want.sort_by_key(|(_, c)| *c);
    let (station, card) = (dev(&s, "-104A1"), dev(&s, "-104A1-A2"));
    let routes = [
        (
            "layout.place",
            json!({"device": id_json(card), "carrier": {"carrier": id_json(r2)},
                   "pose": common::rail_at(1375), "mode": "push"}),
        ),
        (
            "layout.place_assembly",
            json!({"device": id_json(station), "carrier": {"carrier": id_json(r2)}}),
        ),
    ];
    for (name, params) in routes {
        let (mut s, _, r2) = session_without(Some("-104A1-A2"));
        run(&mut s, name, params).unwrap();
        assert_eq!(clips(&s, r2), want, "{name}");
        s.undo(&Origin::Gui).unwrap();
        assert_eq!(clips(&s, r2), before, "{name}: one undo restores all");
    }
}

/// AS-2: rail 2 emptied of its 11 placements, `layout.place_assembly` of -104A1 at s = 20 mm lays the three members
/// abutting in member order: -104A1 70 (100 mm wide), -104A1-A1 132.5, -104A1-A2 157.5 mm (25 mm each); EW-LAY-016
/// reports nothing on the result.
#[test]
fn as2_the_assembly_as_one_block_on_an_empty_rail() {
    let (mut s, _, r2) = session();
    assert_eq!(empty(&mut s, r2), 11);
    let station = dev(&s, "-104A1");
    let q = s.query();
    let order: Vec<String> = member_order(&q, station)
        .into_iter()
        .map(|d| tag(&q, d))
        .collect();
    assert_eq!(order, MEMBERS.map(str::to_owned).to_vec());
    drop(q);
    run(
        &mut s,
        "layout.place_assembly",
        json!({"device": id_json(station), "carrier": {"carrier": id_json(r2)}, "s": 20_000_000}),
    )
    .unwrap();
    let want: Vec<(&str, i128)> = MEMBERS.iter().copied().zip([700, 1325, 1575]).collect();
    assert_eq!(clips(&s, r2), owned(&want));
    let q = s.query();
    let report = ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Online);
    let order_findings: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule == "EW-LAY-016")
        .collect();
    assert!(order_findings.is_empty(), "{order_findings:#?}");
}

/// A copy of -102F1's part with clearance pads of 5 mm left and right (SN-5's fixture), given to -102F1 only.
fn pad_102f1(s: &mut Session) {
    let f1 = dev(s, "-102F1");
    let q = s.query();
    let old = q.devices().get(f1).unwrap().parts[0].part.copy;
    let mut copy: LibCopy<Part> = LibCopy::clone(q.table::<LibCopy<Part>>().get(old).unwrap());
    drop(q);
    let item = Uuid::from_u128(0x5e42_0000_0000_4000_8000_0000_0000_0400);
    copy.item_id = Id::from_uuid(item);
    copy.library_id = Id::from_uuid(Uuid::from_u128(0x5e42_0000_0000_4000_8000_0000_0000_0001));
    copy.id = ew_derive::ew_ids::v5::lib_copy_id(copy.library_id.as_uuid(), item);
    copy.item.meta.key = Some("AS-3-PAD".parse().unwrap());
    copy.item.part_number = "AS-3-PAD".parse().unwrap();
    let mech = copy.item.mech.get_or_insert_with(Default::default);
    mech.clearance = Some(Clearance {
        left: Nm(5_000_000),
        right: Nm(5_000_000),
        ..Clearance::default()
    });
    copy.sha256 = Sha256(ew_commands::commands::library_copy_hash(&LibDocument::Part(copy.clone())).unwrap());
    run(
        s,
        "part.assign",
        json!({"devices": [id_json(f1)], "part": id_json(copy.id),
               "copies": [serde_json::to_value(LibDocument::Part(copy.clone())).unwrap()],
               "mode": "replace", "from": id_json(old)}),
    )
    .unwrap();
}

/// Auto-fill of rail 1 with four breakers and switches and the field cylinder -201M1, in DT order.
fn fill_r1(s: &mut Session, r1: Id<Carrier>) -> Vec<String> {
    let devices: Vec<Value> = ["-103F1", "-102F1", "-101Q1", "-101F1", "-201M1"]
        .iter()
        .map(|d| id_json(dev(s, d)))
        .collect();
    run(
        s,
        "layout.auto_fill",
        json!({"carrier": id_json(r1), "devices": devices, "order": "dt"}),
    )
    .unwrap()
}

/// AS-3: rail 1 emptied, `layout.auto_fill` of -101F1 (36 mm), -101Q1 (54 mm), -102F1 (18 mm) and -103F1 (54 mm) in DT
/// order lays them abutting from the rail's start: clips 18, 63, 99 and 135 mm; with -102F1's pads of 5 mm, 18, 63,
/// 104 and 145 mm; the cylinder -201M1, which mounts on no rail, is not placed, with its notice.
#[test]
fn as3_auto_fill_on_an_empty_rail_1() {
    let (mut s, r1, _) = session();
    assert_eq!(empty(&mut s, r1), 14);
    let notices = fill_r1(&mut s, r1);
    assert_eq!(
        clips(&s, r1),
        owned(&[
            ("-101F1", 180),
            ("-101Q1", 630),
            ("-102F1", 990),
            ("-103F1", 1350)
        ])
    );
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(notices[0].starts_with("-201M1: "), "{notices:?}");

    let (mut s, r1, _) = session();
    empty(&mut s, r1);
    pad_102f1(&mut s);
    let padded = fill_r1(&mut s, r1);
    assert_eq!(
        clips(&s, r1),
        owned(&[
            ("-101F1", 180),
            ("-101Q1", 630),
            ("-102F1", 1040),
            ("-103F1", 1450)
        ])
    );
    assert_eq!(padded, notices);
}

fn lay003(s: &Session) -> Vec<ew_checks::Finding> {
    let q = s.query();
    ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-LAY-003")
        .collect()
}

/// AS-8: -102F1's placement made to realise a part the device then loses (append -101F1's part, move onto it, replace
/// every part use by -102F1's own again, which the placement does not follow: the part is not new to the device) gives
/// EW-LAY-003 with "Realise the primary part"; running its command through the session clears the finding.
#[test]
fn as8_the_quick_fix_of_ew_lay_003_clears_it() {
    let (mut s, _, _) = session();
    assert!(lay003(&s).is_empty());
    let (f1, other_device) = (dev(&s, "-102F1"), dev(&s, "-101F1"));
    let placement = common::placement_of(&s, "-102F1");
    let q = s.query();
    let primary = q.devices().get(f1).unwrap().parts[0].part.copy;
    let other = q.devices().get(other_device).unwrap().parts[0].part.copy;
    let pose = serde_json::to_value(q.table::<Placement>().get(placement).unwrap().pose).unwrap();
    drop(q);
    run(
        &mut s,
        "part.assign",
        json!({"devices": [id_json(f1)], "part": id_json(other), "mode": "append"}),
    )
    .unwrap();
    run(
        &mut s,
        "layout.move",
        json!({"placement": id_json(placement), "pose": pose, "part": id_json(other)}),
    )
    .unwrap();
    run(
        &mut s,
        "part.assign",
        json!({"devices": [id_json(f1)], "part": id_json(primary), "mode": "replace_all"}),
    )
    .unwrap();
    let found = lay003(&s);
    assert_eq!(found.len(), 1, "{found:#?}");
    let fix = found[0].fix.clone().expect("the quick fix");
    assert_eq!(fix.label, "Realise the primary part");
    assert_eq!(
        fix.params,
        json!({"placement": id_json(placement), "pose": pose, "part": id_json(primary)})
    );
    run(&mut s, &fix.command, fix.params).unwrap();
    assert!(lay003(&s).is_empty());
    let q = s.query();
    assert_eq!(
        q.table::<Placement>().get(placement).unwrap().part,
        Some(PartRef::new(primary))
    );
}
