// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rails of the sample's plate (doc 12 §3): the derived end brackets (SN-1), the free lengths, the carrier labels
//! (SN-7), and `layout.place` and `layout.move` with `mode` (SN-2 to SN-4, SN-6), every refusal text whole. Positions
//! are clip points in tenths of a millimetre.
//!
//! The set-up is `common::strips_on_a_test_rail` (the three strips with the TBD end bracket in one run on a third rail
//! at 30, 66 and 117.6 mm), which leaves gaps on rail 1 where the strips stood, with -102F1's placement removed (the
//! device to place) and the fourth strip of `common::add_a_fourth_strip` added unplaced. Rail 1 then holds -101Q1
//! [26, 80], -101F1 [80, 116], -101G1 [116, 176], a gap [176, 235.6], -102S2 [235.6, 255.6], -102S1, -102Q1, -102E1,
//! -103F1 [340.6, 394.6], a gap [394.6, 420.6], -104E1, -104E2 [440.6, 460.6] and a gap to the rail end at 470 mm.

use crate::common::{self, placement_of, rail_at};

use ew_api::{AnyCommand, CmdError, Origin, Session};
use ew_derive::Q;
use ew_ids::{Id, Uuid};
use ew_derive::layout::{
    CarrierLabel, CarrierLabelKind, GroupAccessories, RailAxis, brackets, carrier_labels, free_length,
    rail_items,
};
use ew_schema::{Carrier, Device, LibCopy, LibDocument, Part, Placement, Record, Sha256};
use ew_units::Nm;
use serde_json::{Value, json};

const MM: i128 = 1_000_000;

/// The set-up of the module documentation: the session, rail 1, rail 2 and the test rail.
fn session() -> (Session, Id<Carrier>, Id<Carrier>, Id<Carrier>) {
    let f1 = common::sample().resolve_reference("tag:-102F1").expect("-102F1");
    let (s, rail3) = common::strips_on_a_test_rail_over(
        |records| {
            records.retain(|r| !matches!(r, Record::Placement(p) if p.device.as_uuid() == f1));
            common::add_a_fourth_strip(records);
        },
        ew_api::MemoMode::On,
    );
    let tables = s.panel_tables("902").expect("902");
    let (r1, r2) = (tables.carriers[0].carrier, tables.carriers[1].carrier);
    (s, r1, r2, rail3)
}

fn dev(s: &Session, tag: &str) -> Id<Device> {
    Id::from_uuid(s.resolve_reference(&format!("tag:{tag}")).expect("the device"))
}

fn x33() -> Id<Device> {
    Id::from_uuid(Uuid::from_u128(common::FOURTH_STRIP))
}

fn id_json<T>(id: Id<T>) -> Value {
    serde_json::to_value(id).expect("an ID serialises")
}

/// Runs a command; the receipt's primary record, or the refusal.
fn run(s: &mut Session, name: &str, params: Value) -> Result<Option<Value>, CmdError> {
    let cmd = AnyCommand {
        name: name.into(),
        params,
    };
    s.execute(&cmd).map(|r| {
        r.outcome
            .primary
            .map(|k| serde_json::to_value(k.1).expect("a key serialises"))
    })
}

fn refusal(r: Result<Option<Value>, CmdError>) -> String {
    match r {
        Err(CmdError::Rejected { reason, .. }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn place(
    s: &mut Session,
    device: Id<Device>,
    rail: Id<Carrier>,
    s10: i64,
    mode: &str,
) -> Result<Option<Value>, CmdError> {
    run(
        s,
        "layout.place",
        json!({"device": id_json(device), "carrier": {"carrier": id_json(rail)}, "pose": rail_at(s10), "mode": mode}),
    )
}

fn mv(s: &mut Session, p: Id<Placement>, s10: i64, mode: &str) -> Result<Option<Value>, CmdError> {
    run(
        s,
        "layout.move",
        json!({"placement": id_json(p), "pose": rail_at(s10), "mode": mode}),
    )
}

/// A device's short tag as the session prints it.
fn tag(q: &Q, device: Id<Device>) -> String {
    q.get::<ew_derive::DeviceTag>(device)
        .as_ref()
        .as_ref()
        .map(ew_derive::Tag::short)
        .unwrap_or_default()
}

/// Every placement of a rail with its clip point in tenths of a millimetre, by tag.
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

fn clip_of(s: &Session, rail: Id<Carrier>, tag: &str) -> i128 {
    clips(s, rail)
        .into_iter()
        .find(|(t, _)| t == tag)
        .unwrap_or_else(|| panic!("{tag}: {:?}", clips(s, rail)))
        .1
}

/// A rail's brackets as (s0, s1) in tenths of a millimetre with the strip's tag.
fn rail_brackets(s: &Session, rail: Id<Carrier>) -> Vec<(i128, i128, String)> {
    let q = s.query();
    q.get::<GroupAccessories>(rail)
        .iter()
        .map(|b| (b.s0 * 10 / MM, b.s1 * 10 / MM, tag(&q, b.strip)))
        .collect()
}

fn owned(v: &[(i128, i128, &str)]) -> Vec<(i128, i128, String)> {
    v.iter().map(|(a, b, t)| (*a, *b, (*t).to_owned())).collect()
}

/// SN-1 (FR-LAY-060): the run's brackets are exactly [20, 30], [56, 66], [107.6, 117.6], [143.6, 153.6] mm attributed
/// -X1011, -X1011, -X1021, -X1031, each the strip's TBD end bracket part use (10 mm); -X1031 moved to s = 167.6 mm opens
/// a run of its own, with a bracket on each side: five.
#[test]
fn the_test_rail_has_four_brackets() {
    let (mut s, r1, r2, rail) = session();
    let four = owned(&[
        (200, 300, "-X1011"),
        (560, 660, "-X1011"),
        (1076, 1176, "-X1021"),
        (1436, 1536, "-X1031"),
    ]);
    assert_eq!(rail_brackets(&s, rail), four);
    let q = s.query();
    let parts = q.table::<LibCopy<Part>>();
    for b in q.get::<GroupAccessories>(rail).iter() {
        let copy = parts.get(b.part.copy).expect("the bracket part");
        assert_eq!(copy.item.kind, ew_schema::PartKind::EndBracket);
        assert_eq!(
            copy.item.mech.as_ref().and_then(|m| m.width),
            Some(Nm(10_000_000))
        );
    }
    // Rails 1 and 2 hold no strips now.
    assert!(q.get::<GroupAccessories>(r1).is_empty());
    assert!(q.get::<GroupAccessories>(r2).is_empty());
    drop(q);
    let x1031 = placement_of(&s, "-X1031");
    run(
        &mut s,
        "layout.move",
        json!({"placement": id_json(x1031), "pose": rail_at(1676)}),
    )
    .unwrap();
    assert_eq!(
        rail_brackets(&s, rail),
        owned(&[
            (200, 300, "-X1011"),
            (560, 660, "-X1011"),
            (1076, 1176, "-X1021"),
            (1576, 1676, "-X1031"),
            (1936, 2036, "-X1031"),
        ])
    );
}

/// D12's free lengths (`L − Σ items − Σ brackets`): rail 1 121 mm (470 mm less its 10 devices, 349 mm, -102F1 being
/// unplaced), rail 2 140 mm, the test rail 336.4 mm (470 mm less the three strips, 93.6 mm, and four brackets).
#[test]
fn free_lengths_of_the_rails() {
    let (s, r1, r2, rail) = session();
    let q = s.query();
    let free = |rail: Id<Carrier>| {
        let c = q.table::<Carrier>().get(rail).unwrap().clone();
        let axis = RailAxis::of(&c.path).unwrap();
        let items = rail_items(&q, rail);
        free_length(axis.length, &items, &brackets(&items)) * 10 / MM
    };
    assert_eq!((free(r1), free(r2), free(rail)), (1210, 1400, 3364));
}

/// D8, SN-7: the sample's carriers are Rail 1, Rail 2 and Duct 1.
#[test]
fn carrier_labels_are_numbered_by_kind() {
    let s = common::sample();
    let tables = s.panel_tables("902").expect("902");
    let q: Q = s.query();
    let labels: Vec<(Id<Carrier>, String)> = carrier_labels(&q, tables.surface_id)
        .into_iter()
        .map(|(c, l)| (c, l.label()))
        .collect();
    let expected: Vec<(Id<Carrier>, String)> = tables
        .carriers
        .iter()
        .zip(["Rail 1", "Rail 2", "Duct 1"])
        .map(|(c, l)| (c.carrier, l.to_owned()))
        .collect();
    assert_eq!(labels, expected);
    let one = CarrierLabel {
        kind: CarrierLabelKind::Rail,
        n: 1,
    };
    assert_eq!(
        (one.label(), one.in_sentence()),
        ("Rail 1".into(), "rail 1".into())
    );
}

/// A copy of -102F1's part 300 mm wide (centre clip), embedded and assigned to it (the spec's 300 mm test part).
fn widen_f1(s: &mut Session) {
    let f1 = dev(s, "-102F1");
    let q = s.query();
    let old = q.devices().get(f1).unwrap().parts[0].part.copy;
    let mut copy: LibCopy<Part> = LibCopy::clone(q.table::<LibCopy<Part>>().get(old).unwrap());
    drop(q);
    let item = Uuid::from_u128(0x5e42_0000_0000_4000_8000_0000_0000_0300);
    copy.item_id = Id::from_uuid(item);
    copy.library_id = Id::from_uuid(Uuid::from_u128(0x5e42_0000_0000_4000_8000_0000_0000_0001));
    copy.id = ew_ids::v5::lib_copy_id(copy.library_id.as_uuid(), item);
    copy.item.meta.key = Some("SN-300".parse().unwrap());
    copy.item.part_number = "SN-300".parse().unwrap();
    let mech = copy.item.mech.get_or_insert_with(Default::default);
    mech.width = Some(Nm(300_000_000));
    mech.rail_offset = None;
    copy.sha256 = Sha256(ew_format::library_copy_hash(&LibDocument::Part(copy.clone())).unwrap());
    run(
        s,
        "part.assign",
        json!({"devices": [id_json(f1)], "part": id_json(copy.id),
               "copies": [serde_json::to_value(LibDocument::Part(copy.clone())).unwrap()],
               "mode": "replace", "from": id_json(old)}),
    )
    .unwrap();
}

/// SN-2 (free): -102F1 (18 mm) dropped at clip 300 mm (on -102Q1) lands in the nearer gap, at its end; at clip 200 mm
/// (in the gap) it stays; the 300 mm test part fits no gap.
#[test]
fn sn2_free_on_rail_1() {
    let (mut s, r1, _, _) = session();
    let f1 = dev(&s, "-102F1");
    place(&mut s, f1, r1, 3000, "free").unwrap();
    // The gap [176, 235.6] ends 64.4 mm before the drop, the gap [394.6, 420.6] starts 94.6 mm after it.
    assert_eq!(clip_of(&s, r1, "-102F1"), 2356 - 90);
    let (mut s, r1, _, _) = session();
    place(&mut s, f1, r1, 2000, "free").unwrap();
    assert_eq!(clip_of(&s, r1, "-102F1"), 2000);
    let (mut s, r1, _, _) = session();
    widen_f1(&mut s);
    let refused = refusal(place(&mut s, f1, r1, 3000, "free"));
    assert_eq!(refused, "No gap on rail 1 is wide enough for -102F1 (300 mm).");
}

/// SN-3 (push): -102F1 dropped with its left edge at -102S2's (clip 244.6 mm) stays and pushes -102S2 … -103F1 18 mm
/// to the right, into the gap [394.6, 420.6], in one step; one undo restores them; a locked one refuses; the 300 mm
/// test part pushes past the rail end.
#[test]
fn sn3_push_on_rail_1() {
    let (mut s, r1, _, _) = session();
    let f1 = dev(&s, "-102F1");
    let before = clips(&s, r1);
    place(&mut s, f1, r1, 2446, "push").unwrap();
    let after = clips(&s, r1);
    assert_eq!(clip_of(&s, r1, "-102F1"), 2446, "the dropped device stays");
    // -102F1 [235.6, 253.6] takes -102S2's place: the devices from -102S2 on move right by 18 mm until the gap after
    // -103F1 takes it; the rest stays.
    let moved: Vec<(String, i128)> = after
        .iter()
        .filter(|c| !before.contains(c) && c.0 != "-102F1")
        .cloned()
        .collect();
    let shift = |t: &str, by: i128| (t.to_owned(), before.iter().find(|c| c.0 == t).unwrap().1 + by);
    assert_eq!(
        moved,
        [
            shift("-102S2", 180),
            shift("-102S1", 180),
            shift("-102Q1", 180),
            shift("-102E1", 180),
            shift("-103F1", 180),
        ]
    );
    s.undo(&Origin::Gui).unwrap();
    assert_eq!(clips(&s, r1), before, "one undo restores them all");

    let q1 = placement_of(&s, "-102Q1");
    run(
        &mut s,
        "layout.lock",
        json!({"placement": id_json(q1), "locked": true}),
    )
    .unwrap();
    // Locked placements other than the named one are never moved.
    let refused = refusal(place(&mut s, f1, r1, 2446, "push"));
    assert_eq!(refused, "Pushing would move -102Q1, which is locked.");

    let (mut s, r1, _, _) = session();
    widen_f1(&mut s);
    let refused = refusal(place(&mut s, f1, r1, 2446, "push"));
    assert!(
        refused.starts_with("Pushing would move -104E2 past the rail end by "),
        "{refused}"
    );
}

/// SN-4 (swap): -101F1 [80, 116] dropped onto -102S2 [235.6, 255.6] trades places with it within the span they bound:
/// -102S2 at 80 mm (clip 90), -101G1 after it at 100 mm (clip 130), -101F1 ending at 255.6 mm (clip 237.6), and nothing
/// else moves; two valves on rail 2 swap; a swap onto another rail or onto an empty spot is refused.
#[test]
fn swap_on_rail_1_and_rail_2() {
    let (mut s, r1, r2, _) = session();
    let before = clips(&s, r1);
    let (f1, q1) = (placement_of(&s, "-101F1"), placement_of(&s, "-201K1-Q1"));
    mv(&mut s, f1, 2456, "swap").unwrap();
    let after = clips(&s, r1);
    let changed: Vec<(String, i128)> = after.iter().filter(|c| !before.contains(c)).cloned().collect();
    let want: Vec<(String, i128)> = [("-102S2", 900), ("-101G1", 1300), ("-101F1", 2376)]
        .iter()
        .map(|(t, c)| ((*t).to_owned(), *c))
        .collect();
    assert_eq!(changed, want, "nothing else moved");
    let r2_before = clips(&s, r2);
    mv(&mut s, q1, 2200, "swap").unwrap();
    assert_eq!(clip_of(&s, r2, "-201K1-Q1"), 2200);
    assert_eq!(clip_of(&s, r2, "-201K1-Q2"), 2000);
    let changed: Vec<_> = clips(&s, r2)
        .into_iter()
        .filter(|c| !r2_before.contains(c))
        .collect();
    assert_eq!(changed.len(), 2, "nothing else moved: {changed:?}");
    let across = run(
        &mut s,
        "layout.move",
        json!({"placement": id_json(f1), "carrier": {"carrier": id_json(r2)},
               "pose": rail_at(2000), "mode": "swap"}),
    );
    assert_eq!(
        refusal(across),
        "Swap works within one rail; use push or free to place it there."
    );
    assert_eq!(
        refusal(mv(&mut s, f1, 4650, "swap")),
        "Drop onto a placement to swap with it."
    );
}

/// SN-6 (strips): on the test rail an 18 mm non-strip (-102F1) dropped in free mode at left edge 150 mm lands at
/// 153.6 mm, after -X1031's run-end bracket; the 26 mm fourth strip with the 10 mm bracket dropped at left edge 150 mm
/// (on that bracket) lands at 153.6 mm, joins the run, and the rail then has five brackets.
#[test]
fn strips_on_the_test_rail() {
    let (mut s, _, _, rail) = session();
    // -102F1's clip point is its centre: left edge 150 mm is clip 159 mm; 153.6 mm is clip 162.6 mm.
    let f1 = dev(&s, "-102F1");
    place(&mut s, f1, rail, 1590, "free").unwrap();
    assert_eq!(clip_of(&s, rail, "-102F1"), 1626);
    let (mut s, _, _, rail) = session();
    let bracket = common::part_copy(&s, "TBD end bracket");
    common::add_part_use_to(&mut s, x33().as_uuid(), bracket, "1");
    // A strip's clip point is its left end.
    place(&mut s, x33(), rail, 1500, "free").unwrap();
    // The copied strip's tag is whatever numbering prints for it.
    let x33_tag = tag(&s.query(), x33());
    assert_eq!(clip_of(&s, rail, &x33_tag), 1536);
    assert_eq!(
        rail_brackets(&s, rail),
        owned(&[
            (200, 300, "-X1011"),
            (560, 660, "-X1011"),
            (1076, 1176, "-X1021"),
            (1436, 1536, "-X1031"),
            (1796, 1896, x33_tag.as_str()),
        ])
    );
}
