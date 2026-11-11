// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! On Golden A (doc 20 §13): `layout.place_assembly` beside `layout.place` with push on R3 (AS-1),
//! the assembly -201U1 placed as one block on an emptied R3 in its member order (AS-2), `layout.auto_fill` on an
//! emptied R1 with and without clearance pads (AS-3), and EW-LAY-003's quick fix run through the session (AS-8).
//! Positions are clip points in tenths of a millimetre.

use crate::golden_common;

use ew_api::{AnyCommand, CmdError, Origin, Session};
use ew_derive::Q;
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::layout::member_order;
use ew_import_tables::ids::record_id;
use ew_schema::{Carrier, Clearance, Device, LibCopy, LibDocument, Part, PartRef, Placement, Sha256};
use ew_units::Nm;
use serde_json::{Value, json};

const MM: i128 = 1_000_000;

fn dev(key: &str) -> Id<Device> {
    record_id("devices", key)
}

fn placement(key: &str) -> Id<Placement> {
    record_id("placements", key)
}

fn carrier(key: &str) -> Id<Carrier> {
    record_id("carriers", key)
}

fn id_json<T>(id: Id<T>) -> Value {
    serde_json::to_value(id).expect("an ID serialises")
}

fn session() -> Session {
    golden_common::session(&golden_common::batch())
}

/// Runs a command; its receipt's notices, or the refusal.
fn run(s: &mut Session, name: &str, params: Value) -> Result<Vec<String>, CmdError> {
    let cmd = AnyCommand {
        name: name.into(),
        params,
    };
    s.execute(&cmd).map(|r| r.outcome.notices)
}

/// A rail pose at clip point `s10` tenths of a millimetre, not turned.
fn at(s10: i128) -> Value {
    json!({"t": [i64::try_from(s10 * MM / 10).unwrap(), 0, 0], "q": [1_000_000_000, 0, 0, 0]})
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
fn clips(s: &Session, rail: &str) -> Vec<(String, i128)> {
    let q = s.query();
    let mut out: Vec<(String, i128)> = q
        .table::<Placement>()
        .iter()
        .filter(|(_, p)| p.carrier == ew_schema::CarrierRef::Carrier(carrier(rail)))
        .map(|(_, p)| (tag(&q, p.device), i128::from(p.pose.t[0].0) * 10 / MM))
        .collect();
    out.sort_by_key(|(_, s)| *s);
    out
}

fn owned(v: &[(&str, i128)]) -> Vec<(String, i128)> {
    v.iter().map(|(t, c)| ((*t).to_owned(), *c)).collect()
}

/// Removes every placement of a rail (`layout.remove`, one step each).
fn empty(s: &mut Session, rail: &str) -> usize {
    let on: Vec<Id<Placement>> = s
        .query()
        .table::<Placement>()
        .iter()
        .filter(|(_, p)| p.carrier == ew_schema::CarrierRef::Carrier(carrier(rail)))
        .map(|(id, _)| id)
        .collect();
    for p in &on {
        run(s, "layout.remove", json!({"placement": id_json(*p)})).unwrap();
    }
    on.len()
}

/// The assembly -201U1 in its member order: the station, then its modules by position.
const MEMBERS: [&str; 12] = [
    "-201U1", "-201U2", "-201U3", "-201U4", "-201U5", "-201U6", "-201U7", "-201U8", "-201U9", "-202U1",
    "-202U2", "-202U3",
];

/// AS-1 (spec values): -201U9 pushed in at clip 338 mm by `layout.place`, and `layout.place_assembly` of -201U1 (its
/// one unplaced member goes after its predecessor -201U8), each give -201U9 at 338 mm and -202U1, -202U2, -202U3 at
/// 373, 418 and 448 mm, every other placement unchanged; one undo restores all.
#[test]
fn as1_the_unplaced_member_goes_after_its_predecessor() {
    let before = clips(&session(), "R3");
    let mut want: Vec<(String, i128)> = before
        .iter()
        .map(|(t, c)| {
            let moved = match t.as_str() {
                "-202U1" => 3730,
                "-202U2" => 4180,
                "-202U3" => 4480,
                _ => *c,
            };
            (t.clone(), moved)
        })
        .collect();
    want.push(("-201U9".to_owned(), 3380));
    want.sort_by_key(|(_, c)| *c);
    let routes = [
        (
            "layout.place",
            json!({"device": id_json(dev("-201U9")), "carrier": {"carrier": id_json(carrier("R3"))},
                   "pose": at(3380), "mode": "push"}),
        ),
        (
            "layout.place_assembly",
            json!({"device": id_json(dev("-201U1")), "carrier": {"carrier": id_json(carrier("R3"))}}),
        ),
    ];
    for (name, params) in routes {
        let mut s = session();
        run(&mut s, name, params).unwrap();
        assert_eq!(clips(&s, "R3"), want, "{name}");
        s.undo(&Origin::Gui).unwrap();
        assert_eq!(clips(&s, "R3"), before, "{name}: one undo restores all");
    }
}

/// AS-2 (the brief's author's arithmetic): R3 emptied of its 11 placements, `layout.place_assembly` of -201U1 at
/// s = 20 mm lays the twelve members abutting in member order: -201U1 45, -201U2 79, -201U3 … -201U8 108 … 308,
/// -201U9 338, -202U1 373, -202U2 418, -202U3 448 mm; EW-LAY-016 reports nothing on the result.
#[test]
fn as2_the_assembly_as_one_block_on_an_empty_rail() {
    let mut s = session();
    assert_eq!(empty(&mut s, "R3"), 11);
    let q = s.query();
    let order: Vec<String> = member_order(&q, dev("-201U1"))
        .into_iter()
        .map(|d| tag(&q, d))
        .collect();
    assert_eq!(order, MEMBERS.map(str::to_owned).to_vec());
    run(
        &mut s,
        "layout.place_assembly",
        json!({"device": id_json(dev("-201U1")), "carrier": {"carrier": id_json(carrier("R3"))},
               "s": 20_000_000}),
    )
    .unwrap();
    let want: Vec<(&str, i128)> = MEMBERS
        .iter()
        .copied()
        .zip([
            450, 790, 1080, 1480, 1880, 2280, 2680, 3080, 3380, 3730, 4180, 4480,
        ])
        .collect();
    assert_eq!(clips(&s, "R3"), owned(&want));
    let q = s.query();
    let report = ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Online);
    let order_findings: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule == "EW-LAY-016")
        .collect();
    assert!(order_findings.is_empty(), "{order_findings:#?}");
}

/// A copy of -102F1's part (A9F78116) with clearance pads of 5 mm left and right (SN-5's fixture), given to -102F1
/// only (-101F1's part is another copy and stays unpadded).
fn pad_102f1(s: &mut Session) {
    let q = s.query();
    let f1 = q.devices().get(dev("-102F1")).unwrap().clone();
    let old = f1.parts[0].part.copy;
    let mut copy: LibCopy<Part> = LibCopy::clone(q.table::<LibCopy<Part>>().get(old).unwrap());
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
        json!({"devices": [id_json(dev("-102F1"))], "part": id_json(copy.id),
               "copies": [serde_json::to_value(LibDocument::Part(copy.clone())).unwrap()],
               "mode": "replace", "from": id_json(old)}),
    )
    .unwrap();
}

fn fill_r1(s: &mut Session) -> Vec<String> {
    let devices: Vec<Value> = ["-102G1", "-102F2", "-101F1", "-102F1", "E55"]
        .iter()
        .map(|d| id_json(dev(d)))
        .collect();
    run(
        s,
        "layout.auto_fill",
        json!({"carrier": id_json(carrier("R1")), "devices": devices, "order": "dt"}),
    )
    .unwrap()
}

/// AS-3: R1 emptied, `layout.auto_fill` of -102G1, -102F2, -101F1, -102F1 and E55 in DT order gives -101F1 9,
/// -102F1 27, -102F2 58.5, -102G1 116 mm (spec values); with -102F1's pads of 5 mm 9, 32, 68.5, 126 mm (the brief's
/// author's arithmetic from D3); E55 is not placed, with its notice.
#[test]
fn as3_auto_fill_on_an_empty_r1() {
    let mut s = session();
    assert_eq!(empty(&mut s, "R1"), 6);
    let notices = fill_r1(&mut s);
    assert_eq!(
        clips(&s, "R1"),
        owned(&[("-101F1", 90), ("-102F1", 270), ("-102F2", 585), ("-102G1", 1160)])
    );
    assert_eq!(notices, vec!["E55: screw-mounted, not on a rail".to_owned()]);

    let mut s = session();
    empty(&mut s, "R1");
    pad_102f1(&mut s);
    let notices = fill_r1(&mut s);
    assert_eq!(
        clips(&s, "R1"),
        owned(&[("-101F1", 90), ("-102F1", 320), ("-102F2", 685), ("-102G1", 1260)])
    );
    assert_eq!(notices, vec!["E55: screw-mounted, not on a rail".to_owned()]);
}

fn lay003(s: &Session) -> Vec<ew_checks::Finding> {
    let q = s.query();
    ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-LAY-003")
        .collect()
}

/// AS-8: -102F1's placement made to realise a part the device then loses (append 533354, move onto it, replace every
/// part use by A9F78116 again, which the placement does not follow: the part is not new to the device) gives
/// EW-LAY-003 with "Realise the primary part"; running its command through the session clears the finding.
#[test]
fn as8_the_quick_fix_of_ew_lay_003_clears_it() {
    let mut s = session();
    assert!(lay003(&s).is_empty());
    let q = s.query();
    let primary = q.devices().get(dev("-102F1")).unwrap().parts[0].part.copy;
    let other = q.devices().get(dev("-201U9")).unwrap().parts[0].part.copy;
    let pose = serde_json::to_value(q.table::<Placement>().get(placement("-102F1")).unwrap().pose).unwrap();
    run(
        &mut s,
        "part.assign",
        json!({"devices": [id_json(dev("-102F1"))], "part": id_json(other), "mode": "append"}),
    )
    .unwrap();
    run(
        &mut s,
        "layout.move",
        json!({"placement": id_json(placement("-102F1")), "pose": pose, "part": id_json(other)}),
    )
    .unwrap();
    run(
        &mut s,
        "part.assign",
        json!({"devices": [id_json(dev("-102F1"))], "part": id_json(primary), "mode": "replace_all"}),
    )
    .unwrap();
    let found = lay003(&s);
    assert_eq!(found.len(), 1, "{found:#?}");
    let fix = found[0].fix.clone().expect("the quick fix");
    assert_eq!(fix.label, "Realise the primary part");
    assert_eq!(
        fix.params,
        json!({"placement": id_json(placement("-102F1")), "pose": pose, "part": id_json(primary)})
    );
    run(&mut s, &fix.command, fix.params).unwrap();
    assert!(lay003(&s).is_empty());
    let q = s.query();
    assert_eq!(
        q.table::<Placement>().get(placement("-102F1")).unwrap().part,
        Some(PartRef::new(primary))
    );
}
