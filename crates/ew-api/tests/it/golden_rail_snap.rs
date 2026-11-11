// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S4.02 part 1 on Golden A (doc 20 §13): the derived end brackets of R2 (SN-1), the free lengths of R1 and R2
//! (D12), the carrier labels (D8, SN-7), and `layout.place` and `layout.move` with `mode` on R1, R2 and R3 (SN-2 to
//! SN-4, SN-6), every refusal text whole. Positions are clip points in tenths of a millimetre.

use crate::golden_common;

use ew_api::{AnyCommand, CmdError, Origin, Session};
use ew_derive::Q;
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::layout::{
    CarrierLabel, CarrierLabelKind, GroupAccessories, RailAxis, brackets, carrier_labels, free_length,
    rail_items,
};
use ew_import_tables::ids::record_id;
use ew_schema::{Carrier, Device, LibCopy, LibDocument, MountingSurface, Part, Placement, Record, Sha256};
use ew_units::{Dec3, Nm};
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

/// The test strip: a copy of -X1032 (five PT 2.5-TWIN-PE, 26 mm, with its "TBD end bracket" part use) that is not
/// placed, for SN-6's test strip.
const X33: u128 = 0x5e42_0000_0000_4000_8000_0000_0000_0033;

fn session() -> Session {
    golden_common::session_with(&golden_common::batch(), |_, records| {
        let x1032 = dev("-X1032");
        let strip_id: Id<Device> = Id::from_uuid(Uuid::from_u128(X33));
        let mut extra = Vec::new();
        for r in records.iter() {
            let Record::Device(d) = r else { continue };
            if d.id == x1032 {
                let mut s = d.clone();
                s.id = strip_id;
                s.tag.counter = Some(33);
                extra.push(Record::Device(s));
            } else if let Some(t) = d.aspects.terminal.as_ref().filter(|t| t.strip == x1032) {
                let mut c = d.clone();
                c.id = Id::from_uuid(Uuid::from_u128(X33 + 0x100 + u128::from(t.seq)));
                if let Some(t) = c.aspects.terminal.as_mut() {
                    t.strip = strip_id;
                }
                extra.push(Record::Device(c));
            }
        }
        assert_eq!(extra.len(), 6, "-X1032 and its five terminals");
        records.extend(extra);
    })
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

/// A rail pose at clip point `s10` tenths of a millimetre, not turned.
fn at(s10: i128) -> Value {
    json!({"t": [i64::try_from(s10 * MM / 10).unwrap(), 0, 0], "q": [1_000_000_000, 0, 0, 0]})
}

fn place(
    s: &mut Session,
    device: Id<Device>,
    rail: &str,
    s10: i128,
    mode: &str,
) -> Result<Option<Value>, CmdError> {
    run(
        s,
        "layout.place",
        json!({"device": id_json(device), "carrier": {"carrier": id_json(carrier(rail))}, "pose": at(s10), "mode": mode}),
    )
}

fn mv(s: &mut Session, p: Id<Placement>, s10: i128, mode: &str) -> Result<Option<Value>, CmdError> {
    run(
        s,
        "layout.move",
        json!({"placement": id_json(p), "pose": at(s10), "mode": mode}),
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

fn clip_of(s: &Session, rail: &str, tag: &str) -> i128 {
    clips(s, rail)
        .into_iter()
        .find(|(t, _)| t == tag)
        .unwrap_or_else(|| panic!("{tag} on {rail}: {:?}", clips(s, rail)))
        .1
}

/// R2's brackets as (s0, s1) in tenths of a millimetre with the strip's tag.
fn r2_brackets(s: &Session) -> Vec<(i128, i128, String)> {
    let q = s.query();
    q.get::<GroupAccessories>(carrier("R2"))
        .iter()
        .map(|b| (b.s0 * 10 / MM, b.s1 * 10 / MM, tag(&q, b.strip)))
        .collect()
}

fn owned(v: &[(i128, i128, &str)]) -> Vec<(i128, i128, String)> {
    v.iter().map(|(a, b, t)| (*a, *b, (*t).to_owned())).collect()
}

/// SN-1 (FR-LAY-060): R2's brackets are exactly [20, 30], [48.6, 58.6], [162.6, 172.6], [198.6, 208.6] mm attributed
/// 1X1, 1X1, -X1031, -X1032, each the strip's "TBD end bracket" part use, and per strip as many as its authored
/// quantity (1X1 × 2, -X1031 × 1, -X1032 × 1: row 26's 4); -X1032 moved to s = 222.6 mm gives five.
#[test]
fn golden_a_r2_has_four_brackets() {
    let mut s = session();
    let four = owned(&[
        (200, 300, "1X1"),
        (486, 586, "1X1"),
        (1626, 1726, "-X1031"),
        (1986, 2086, "-X1032"),
    ]);
    assert_eq!(r2_brackets(&s), four);
    let q = s.query();
    let parts = q.table::<LibCopy<Part>>();
    for b in q.get::<GroupAccessories>(carrier("R2")).iter() {
        let copy = parts.get(b.part.copy).expect("the bracket part");
        assert_eq!(copy.item.kind, ew_schema::PartKind::EndBracket);
        assert_eq!(
            copy.item.mech.as_ref().and_then(|m| m.width),
            Some(Nm(10_000_000))
        );
        let strip = q.devices().get(b.strip).unwrap().clone();
        let authored = strip
            .parts
            .iter()
            .find(|u| u.part.copy == b.part.copy)
            .map(|u| u.qty)
            .unwrap();
        let count = q
            .get::<GroupAccessories>(carrier("R2"))
            .iter()
            .filter(|x| x.strip == b.strip)
            .count();
        assert_eq!(Dec3(i64::try_from(count).unwrap() * 1000), authored);
    }
    // R1 and R3 hold no strips; a duct none.
    assert!(q.get::<GroupAccessories>(carrier("R1")).is_empty());
    assert!(q.get::<GroupAccessories>(carrier("D1")).is_empty());
    run(
        &mut s,
        "layout.move",
        json!({"placement": id_json(placement("-X1032")), "pose": at(2226)}),
    )
    .unwrap();
    assert_eq!(
        r2_brackets(&s),
        owned(&[
            (200, 300, "1X1"),
            (486, 586, "1X1"),
            (1626, 1726, "-X1031"),
            (2126, 2226, "-X1032"),
            (2486, 2586, "-X1032"),
        ])
    );
}

/// D12's free lengths, for part 4: R1 341 mm, R2 381.4 mm (`L − Σ items − Σ brackets`).
#[test]
fn free_lengths_of_r1_and_r2() {
    let s = session();
    let q = s.query();
    let free = |rail: &str| {
        let c = q.table::<Carrier>().get(carrier(rail)).unwrap().clone();
        let axis = RailAxis::of(&c.path).unwrap();
        let items = rail_items(&q, carrier(rail));
        free_length(axis.length, &items, &brackets(&items)) * 10 / MM
    };
    assert_eq!((free("R1"), free("R2")), (3410, 3814));
}

/// D8, SN-7: Golden A's carriers are Rail 1 … 3 and Duct 1 … 5, as doc 20 §13's R1 … R3 and D1 … D5.
#[test]
fn carrier_labels_match_doc_20() {
    let s = session();
    let q: Q = s.query();
    let surface: Id<MountingSurface> = record_id("mounting_surfaces", "-UC1");
    let labels: Vec<(Id<Carrier>, String)> = carrier_labels(&q, surface)
        .into_iter()
        .map(|(c, l)| (c, l.label()))
        .collect();
    let expected: Vec<(Id<Carrier>, String)> = [
        ("R1", "Rail 1"),
        ("R2", "Rail 2"),
        ("R3", "Rail 3"),
        ("D1", "Duct 1"),
        ("D2", "Duct 2"),
        ("D3", "Duct 3"),
        ("D4", "Duct 4"),
        ("D5", "Duct 5"),
    ]
    .iter()
    .map(|(k, l)| (carrier(k), (*l).to_owned()))
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

/// A copy of F55's part 300 mm wide (centre clip), embedded and assigned to F55 (the spec's 300 mm test part).
fn widen_f55(s: &mut Session) {
    let q = s.query();
    let f55 = q.devices().get(dev("F55")).unwrap().clone();
    let old = f55.parts[0].part.copy;
    let mut copy: LibCopy<Part> = LibCopy::clone(q.table::<LibCopy<Part>>().get(old).unwrap());
    let item = Uuid::from_u128(0x5e42_0000_0000_4000_8000_0000_0000_0300);
    copy.item_id = Id::from_uuid(item);
    copy.library_id = Id::from_uuid(Uuid::from_u128(0x5e42_0000_0000_4000_8000_0000_0000_0001));
    copy.id = ew_derive::ew_ids::v5::lib_copy_id(copy.library_id.as_uuid(), item);
    copy.item.meta.key = Some("SN-300".parse().unwrap());
    copy.item.part_number = "SN-300".parse().unwrap();
    let mech = copy.item.mech.get_or_insert_with(Default::default);
    mech.width = Some(Nm(300_000_000));
    mech.rail_offset = None;
    copy.sha256 = Sha256(ew_commands::commands::library_copy_hash(&LibDocument::Part(copy.clone())).unwrap());
    run(
        s,
        "part.assign",
        json!({"devices": [id_json(dev("F55"))], "part": id_json(copy.id),
               "copies": [serde_json::to_value(LibDocument::Part(copy.clone())).unwrap()],
               "mode": "replace", "from": id_json(old)}),
    )
    .unwrap();
}

/// SN-2 (free): F55 (18 mm) at clip 158 mm lands at clip 52 mm (the left gap's end is nearer than the right gap's
/// start); at clip 320 mm it stays; the 300 mm test part fits neither gap.
#[test]
fn sn2_free_on_r1() {
    let mut s = session();
    place(&mut s, dev("F55"), "R1", 1580, "free").unwrap();
    assert_eq!(clip_of(&s, "R1", "F55"), 520);
    let mut s = session();
    place(&mut s, dev("F55"), "R1", 3200, "free").unwrap();
    assert_eq!(clip_of(&s, "R1", "F55"), 3200);
    let mut s = session();
    widen_f55(&mut s);
    let refused = refusal(place(&mut s, dev("F55"), "R1", 1580, "free"));
    assert_eq!(refused, "No gap on rail 1 is wide enough for F55 (300 mm).");
}

/// SN-3 (push): F55 at clip 158 mm stays and moves -102F1, -102F2, T55 and S51 to 176, 207.5, 246.5 and 285.5 mm in
/// one step; one undo restores all five; a locked -102F2 refuses; the 300 mm test part pushes S51 20 mm past the end.
#[test]
fn sn3_push_on_r1() {
    let mut s = session();
    let before = clips(&s, "R1");
    place(&mut s, dev("F55"), "R1", 1580, "push").unwrap();
    let want: Vec<(String, i128)> = [
        ("-102G1", 960),
        ("-101F1", 1400),
        ("F55", 1580),
        ("-102F1", 1760),
        ("-102F2", 2075),
        ("T55", 2465),
        ("S51", 2855),
    ]
    .iter()
    .map(|(t, c)| ((*t).to_owned(), *c))
    .collect();
    assert_eq!(clips(&s, "R1"), want);
    s.undo(&Origin::Gui).unwrap();
    assert_eq!(clips(&s, "R1"), before, "one undo restores all five");

    run(
        &mut s,
        "layout.lock",
        json!({"placement": id_json(placement("-102F2")), "locked": true}),
    )
    .unwrap();
    // Orchestrator ruling 6 (replaced): "locked placements other than the named one are never moved, as D5 says".
    let refused = refusal(place(&mut s, dev("F55"), "R1", 1580, "push"));
    assert_eq!(refused, "Pushing would move -102F2, which is locked.");

    let mut s = session();
    widen_f55(&mut s);
    let refused = refusal(place(&mut s, dev("F55"), "R1", 1580, "push"));
    assert_eq!(refused, "Pushing would move S51 past the rail end by 20 mm.");
}

/// SN-4 (swap): -101F1 dropped onto -102F2 gives -102F2 153.5, -102F1 185, -101F1 203 mm and moves nothing else;
/// -201U3 and -201U4 on R3 swap to 148 and 108 mm; a swap onto another rail or onto an empty spot is refused.
#[test]
fn swap_on_r1_and_r3() {
    let mut s = session();
    mv(&mut s, placement("-101F1"), 1895, "swap").unwrap();
    let want: Vec<(String, i128)> = [
        ("-102G1", 960),
        ("-102F2", 1535),
        ("-102F1", 1850),
        ("-101F1", 2030),
        ("T55", 2285),
        ("S51", 2675),
    ]
    .iter()
    .map(|(t, c)| ((*t).to_owned(), *c))
    .collect();
    assert_eq!(clips(&s, "R1"), want);
    let r3_before = clips(&s, "R3");
    mv(&mut s, placement("-201U3"), 1480, "swap").unwrap();
    assert_eq!(clip_of(&s, "R3", "-201U3"), 1480);
    assert_eq!(clip_of(&s, "R3", "-201U4"), 1080);
    let changed: Vec<_> = clips(&s, "R3")
        .into_iter()
        .filter(|c| !r3_before.contains(c))
        .collect();
    assert_eq!(changed.len(), 2, "nothing else moved: {changed:?}");
    let across = run(
        &mut s,
        "layout.move",
        json!({"placement": id_json(placement("-101F1")), "carrier": {"carrier": id_json(carrier("R3"))},
               "pose": at(1080), "mode": "swap"}),
    );
    assert_eq!(
        refusal(across),
        "Swap works within one rail; use push or free to place it there."
    );
    assert_eq!(
        refusal(mv(&mut s, placement("-101F1"), 4000, "swap")),
        "Drop onto a placement to swap with it."
    );
}

/// SN-6 (strips): on R2 an 18 mm non-strip (F55) dropped in free mode at left edge 200 mm lands at 208.6 mm, after
/// -X1032's run-end bracket; a 26 mm strip with a 10 mm bracket part (a copy of -X1032) dropped at left edge 205 mm lands at
/// 208.6 mm, joins the run and R2 then has five brackets.
#[test]
fn strips_on_r2() {
    let mut s = session();
    // F55's clip point is its centre: left edge 200 mm is clip 209 mm; 208.6 mm is clip 217.6 mm.
    place(&mut s, dev("F55"), "R2", 2090, "free").unwrap();
    assert_eq!(clip_of(&s, "R2", "F55"), 2176);
    let mut s = session();
    let x33: Id<Device> = Id::from_uuid(Uuid::from_u128(X33));
    // A strip's clip point is its left end.
    place(&mut s, x33, "R2", 2050, "free").unwrap();
    // The copied strip's tag is whatever numbering prints for it (it is not a strip of Golden A).
    let x33_tag = tag(&s.query(), x33);
    assert_eq!(clip_of(&s, "R2", &x33_tag), 2086);
    assert_eq!(
        r2_brackets(&s),
        owned(&[
            (200, 300, "1X1"),
            (486, 586, "1X1"),
            (1626, 1726, "-X1031"),
            (1986, 2086, "-X1032"),
            (2346, 2446, x33_tag.as_str()),
        ])
    );
}
