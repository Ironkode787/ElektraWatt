// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The layout queries of WP-S4.01 part 2 on a small fixture: the mountable predicate case by case (D8), `unplaced`,
//! `placement_geometry` on a horizontal and a vertical rail, flipped, on the surface turned 270°, unknown and on a
//! duct (D5), `rail_occupancy` and `carrier_geometry`. Golden A's values are `ew-api`'s `tests/golden_layout.rs`.

use crate::common;

use common::{Fx, id, uid};
use ew_derive::Q;
use ew_derive::layout::{
    CarrierGeometry, Exempt, MountAs, Mountability, Mountable, PlacementGeometry, RailOccupancy, Unplaced,
};
use ew_ids::Id;
use ew_schema::{Device, PartRef};
use ew_units::{Nm, Pt2, Rect};
use serde_json::{Value, json};

const MM: i64 = 1_000_000;

const LOC_A: u128 = 0x10;
const LOC_B: u128 = 0x11;
const FN_MCB: u128 = 0x500;
const FN_PE: u128 = 0x501;
const P_MCB: u128 = 0x600; // TS 35, 18 × 90 mm, centre clip
const P_BARE: u128 = 0x601; // no mech
const P_PARENT: u128 = 0x602; // on_parent
const P_SCREW: u128 = 0x603; // screw, 50 × 120 mm
const SPACE: u128 = 0x20;
const ENCLOSURE: u128 = 0x21;
const SURFACE: u128 = 0x22;
const RAIL_H: u128 = 0x23; // (0, 0) → (500, 0) mm
const RAIL_V: u128 = 0x24; // (0, 0) → (0, 500) mm
const DUCT: u128 = 0x25; // (0, 600) → (400, 600) → (400, 900) mm, 40 × 60

const D_MCB: u128 = 0xd1; // case 5, on RAIL_H at s = 100
const D_BARE: u128 = 0xd2; // case 7, unplaced
const D_PARENT: u128 = 0xd3; // case 6
const D_PE: u128 = 0xd4; // case 8
const D_NOPART: u128 = 0xd5; // case 9, unplaced
const D_ELSEWHERE: u128 = 0xd6; // case 1 (location B has no space)
const D_MCB_V: u128 = 0xd7; // case 5, on RAIL_V at s = 100
const D_NOFN: u128 = 0xd8; // case 4, no functions
const D_SCREW: u128 = 0xd9; // case 5, on the surface at (200, 300) turned 270°
const D_UNKNOWN: u128 = 0xda; // case 9, on RAIL_H at s = 300
const D_ON_DUCT: u128 = 0xdb; // case 5, a placement on the duct
const D_ENCLOSURE: u128 = 0xe0; // case 2

const PL_MCB: u128 = 0xf1;
const PL_MCB_V: u128 = 0xf7;
const PL_SCREW: u128 = 0xf9;
const PL_UNKNOWN: u128 = 0xfa;
const PL_ON_DUCT: u128 = 0xfb;

fn mm(v: i64) -> i64 {
    v * MM
}

fn pose(t: [i64; 2], k: u8) -> Value {
    json!({"t": [t[0], t[1], 0], "q": ew_derive::layout::Quarter::new(k).quat()})
}

fn part(fx: &mut Fx, n: u128, number: &str, mech: Option<Value>) {
    let mut item = json!({
        "meta": {"key": number}, "part_number": number,
        "manufacturer": {"library": uid(common::LIBRARY), "item": uid(0x9000), "version_req": "^1"},
    });
    if let Some(m) = mech {
        item["mech"] = m;
    }
    fx.lib_item("part", n, item);
}

fn device(
    fx: &mut Fx,
    n: u128,
    letter: &str,
    counter: u32,
    loc: u128,
    parts: &[u128],
    function: Option<u128>,
) {
    fx.device(n, letter, Some(counter))
        .with("structure", json!({"location": uid(loc)}));
    if !parts.is_empty() {
        let uses: Vec<Value> = parts
            .iter()
            .map(|p| json!({"part": {"copy": uid(*p)}, "qty": "1"}))
            .collect();
        fx.with("parts", json!(uses));
    }
    if let Some(def) = function {
        fx.function(n + 0x1000, Some(n), def, true);
    }
}

fn place(fx: &mut Fx, n: u128, device: u128, on: Value, pose: Value) {
    fx.push(json!({"$k": "placement", "id": uid(n), "device": uid(device), "carrier": on, "pose": pose}));
}

/// The fixture; `flip` turns D_MCB's placement by 180°.
fn fixture(flip: bool) -> Fx {
    let mut fx = Fx::new();
    fx.node(LOC_A, "location", "A", None)
        .node(LOC_B, "location", "B", None)
        .fndef(
            FN_MCB,
            ["Protection", "Overcurrent", "CircuitBreaker"],
            &[(1, "1"), (2, "2")],
        )
        .fndef(FN_PE, ["EarthingBar", "ProtectiveEarth", "Point"], &[(1, "PE")]);
    part(
        &mut fx,
        P_MCB,
        "MCB",
        Some(json!({"width": mm(18), "height": mm(90), "mount_kind": "ts35x7_5"})),
    );
    part(&mut fx, P_BARE, "BARE", None);
    part(
        &mut fx,
        P_PARENT,
        "PLUG",
        Some(json!({"mount_kind": "on_parent"})),
    );
    part(
        &mut fx,
        P_SCREW,
        "HEATER",
        Some(json!({"width": mm(50), "height": mm(120), "mount_kind": "screw"})),
    );
    device(&mut fx, D_ENCLOSURE, "U", 1, LOC_A, &[], None);
    device(&mut fx, D_MCB, "F", 1, LOC_A, &[P_MCB], Some(FN_MCB));
    device(&mut fx, D_BARE, "K", 2, LOC_A, &[P_BARE], Some(FN_MCB));
    device(&mut fx, D_PARENT, "X", 3, LOC_A, &[P_PARENT], Some(FN_MCB));
    device(&mut fx, D_PE, "X", 4, LOC_A, &[], Some(FN_PE));
    device(&mut fx, D_NOPART, "K", 5, LOC_A, &[], Some(FN_MCB));
    device(&mut fx, D_ELSEWHERE, "F", 6, LOC_B, &[P_MCB], Some(FN_MCB));
    device(&mut fx, D_MCB_V, "F", 7, LOC_A, &[P_MCB], Some(FN_MCB));
    device(&mut fx, D_NOFN, "F", 8, LOC_A, &[P_MCB], None);
    device(&mut fx, D_SCREW, "E", 9, LOC_A, &[P_SCREW], Some(FN_MCB));
    device(&mut fx, D_UNKNOWN, "S", 10, LOC_A, &[], Some(FN_MCB));
    device(&mut fx, D_ON_DUCT, "F", 11, LOC_A, &[P_MCB], Some(FN_MCB));
    let identity = pose([0, 0], 0);
    fx.push(json!({"$k": "layout_space", "id": uid(SPACE), "location": uid(LOC_A)}))
        .push(
            json!({"$k": "enclosure", "id": uid(ENCLOSURE), "space": uid(SPACE), "device": uid(D_ENCLOSURE),
                     "pose": identity}),
        )
        .push(
            json!({"$k": "mounting_surface", "id": uid(SURFACE), "enclosure": uid(ENCLOSURE), "kind": "plate",
                     "frame": identity}),
        )
        .push(
            json!({"$k": "carrier", "id": uid(RAIL_H), "surface": uid(SURFACE), "kind": "rail",
                     "path": [[0, 0], [mm(500), 0]], "params": {"rail": {"profile": "ts35x7_5"}}}),
        )
        .push(
            json!({"$k": "carrier", "id": uid(RAIL_V), "surface": uid(SURFACE), "kind": "rail",
                     "path": [[0, 0], [0, mm(500)]], "params": {"rail": {"profile": "ts35x7_5"}}}),
        )
        .push(
            json!({"$k": "carrier", "id": uid(DUCT), "surface": uid(SURFACE), "kind": "duct",
                     "path": [[0, mm(600)], [mm(400), mm(600)], [mm(400), mm(900)]],
                     "params": {"duct": {"width": mm(40), "height": mm(60)}}}),
        );
    let rail = |c: u128| json!({"carrier": uid(c)});
    place(
        &mut fx,
        PL_MCB,
        D_MCB,
        rail(RAIL_H),
        pose([mm(100), 0], if flip { 2 } else { 0 }),
    );
    place(&mut fx, PL_MCB_V, D_MCB_V, rail(RAIL_V), pose([mm(100), 0], 0));
    place(
        &mut fx,
        PL_SCREW,
        D_SCREW,
        json!({"surface": uid(SURFACE)}),
        pose([mm(200), mm(300)], 3),
    );
    place(
        &mut fx,
        PL_UNKNOWN,
        D_UNKNOWN,
        rail(RAIL_H),
        pose([mm(300), 0], 0),
    );
    place(&mut fx, PL_ON_DUCT, D_ON_DUCT, rail(DUCT), pose([mm(10), 0], 0));
    fx
}

fn rect(a: (i64, i64), b: (i64, i64)) -> Rect {
    Rect::from_points(
        Pt2::new(Nm(mm(a.0)), Nm(mm(a.1))),
        Pt2::new(Nm(mm(b.0)), Nm(mm(b.1))),
    )
}

fn footprint(q: &Q, p: u128) -> (Rect, bool) {
    let g = q.get::<PlacementGeometry>(id(p));
    let g = g.as_ref().as_ref().expect("a geometry");
    (g.footprint, g.unknown)
}

#[test]
fn the_mountable_predicate_case_by_case() {
    let m = fixture(false).build();
    let q = Q::new(&m);
    let of = |d: u128| q.get::<Mountable>(id(d)).as_ref().clone();
    let mountable = |how, primary: Option<u128>| Mountability::Mountable {
        how,
        primary: primary.map(|p| PartRef::new(id(p))),
    };
    assert_eq!(of(D_ELSEWHERE), Mountability::NotConsidered, "case 1");
    assert_eq!(of(D_ENCLOSURE), Mountability::Exempt(Exempt::Layout), "case 2");
    assert_eq!(of(D_NOFN), Mountability::Exempt(Exempt::NoFunctions), "case 4");
    assert_eq!(of(D_MCB), mountable(MountAs::Part, Some(P_MCB)), "case 5");
    assert_eq!(
        of(D_SCREW),
        mountable(MountAs::Part, Some(P_SCREW)),
        "case 5, screw"
    );
    assert_eq!(of(D_PARENT), Mountability::Exempt(Exempt::WithParent), "case 6");
    assert_eq!(
        of(D_BARE),
        mountable(MountAs::PartWithoutMountKind, Some(P_BARE)),
        "case 7"
    );
    assert_eq!(of(D_PE), Mountability::Exempt(Exempt::PePoint), "case 8");
    assert_eq!(of(D_NOPART), mountable(MountAs::NoPart, None), "case 9");
    assert_eq!(of(0x9999), Mountability::NotConsidered, "a missing device");
}

#[test]
fn unplaced_lists_the_mountable_devices_without_a_placement_in_tag_order() {
    let m = fixture(false).build();
    let q = Q::new(&m);
    let list: Vec<Id<Device>> = q.get::<Unplaced>(id(SPACE)).as_ref().clone();
    // D_ELSEWHERE is of location B; the placed devices, the exempt ones and the enclosure are out.
    assert_eq!(list, vec![id(D_BARE), id(D_NOPART)]);
    assert!(
        q.get::<Unplaced>(id(0x9999)).is_empty(),
        "a missing space lists nothing"
    );
}

#[test]
fn footprints_on_rails_on_the_surface_unknown_and_on_a_duct() {
    let m = fixture(false).build();
    let q = Q::new(&m);
    // Horizontal rail: the centre clip (9, 45) at (100, 0).
    assert_eq!(footprint(&q, PL_MCB), (rect((91, -45), (109, 45)), false));
    // Vertical rail: the part turns 90°, the clip point at (0, 100).
    assert_eq!(footprint(&q, PL_MCB_V), (rect((-45, 91), (45, 109)), false));
    // The surface, origin (200, 300) turned 270°: W × H becomes H × W below the origin.
    assert_eq!(footprint(&q, PL_SCREW), (rect((200, 250), (320, 300)), false));
    // No part: the unknown 20 × 20 mm square about the clip point (300, 0).
    assert_eq!(footprint(&q, PL_UNKNOWN), (rect((290, -10), (310, 10)), true));
    // A duct carries no placements: no geometry.
    assert!(q.get::<PlacementGeometry>(id(PL_ON_DUCT)).is_none());
    // Flipped by 180° the centre-clipped part covers the same rectangle.
    let flipped = fixture(true).build();
    let fq = Q::new(&flipped);
    assert_eq!(footprint(&fq, PL_MCB), footprint(&q, PL_MCB));
    let g = fq.get::<PlacementGeometry>(id(PL_MCB));
    assert_eq!(g.as_ref().as_ref().map(|g| g.turn.k()), Some(2));
}

#[test]
fn rail_occupancy_and_carrier_geometry() {
    let m = fixture(false).build();
    let q = Q::new(&m);
    let occ = q.get::<RailOccupancy>(id(RAIL_H));
    let intervals: Vec<_> = occ.iter().map(|o| (o.s0, o.s1, o.placement, o.unknown)).collect();
    let i = |a: i64, b: i64| (i128::from(mm(a)), i128::from(mm(b)));
    assert_eq!(
        intervals,
        vec![
            (i(91, 109).0, i(91, 109).1, id(PL_MCB), false),
            (i(290, 310).0, i(290, 310).1, id(PL_UNKNOWN), true),
        ]
    );
    let v = q.get::<RailOccupancy>(id(RAIL_V));
    assert_eq!(
        v.iter().map(|o| (o.s0, o.s1)).collect::<Vec<_>>(),
        vec![i(91, 109)]
    );
    assert!(
        q.get::<RailOccupancy>(id(DUCT)).is_empty(),
        "a duct has no occupancy"
    );

    let rail = q.get::<CarrierGeometry>(id(RAIL_H));
    let rail = rail.as_ref().as_ref().expect("a rail geometry");
    assert_eq!(rail.length, Some(i128::from(mm(500))));
    assert_eq!(rail.outline.len(), 1);
    assert_eq!(rail.outline[0].min(), Pt2::new(Nm::ZERO, Nm(-17_500_000)));
    let duct = q.get::<CarrierGeometry>(id(DUCT));
    let duct = duct.as_ref().as_ref().expect("a duct geometry");
    assert_eq!(duct.length, Some(i128::from(mm(700))));
    assert_eq!(
        duct.outline,
        vec![rect((0, 580), (400, 620)), rect((380, 600), (420, 900))]
    );
    assert!(q.get::<CarrierGeometry>(id(0x9999)).is_none());
}

/// The footprint of D_MCB placed on RAIL_H at `s_um` µm, flipped by 180° when `half`.
fn mcb_at(s_um: i64, half: bool) -> Rect {
    let mut fx = fixture(false);
    fx.record(PL_MCB)["pose"] = pose([s_um * 1000, 0], if half { 2 } else { 0 });
    let m = fx.build();
    footprint(&Q::new(&m), PL_MCB).0
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(32))]

    /// D5 on a rail: a centre-clipped part covers `[s − W/2, s + W/2]` across the axis' `±H/2` whatever `s`, and the
    /// 180° flip covers the same rectangle.
    #[test]
    fn a_centre_clipped_part_spans_its_width_about_s_flipped_or_not(s_um in 0i64..500_000) {
        let r = mcb_at(s_um, false);
        let s = Nm(s_um * 1000);
        proptest::prop_assert_eq!(r.min(), Pt2::new(Nm(s.0 - mm(9)), Nm(-mm(45))));
        proptest::prop_assert_eq!(r.max(), Pt2::new(Nm(s.0 + mm(9)), Nm(mm(45))));
        proptest::prop_assert_eq!(mcb_at(s_um, true), r);
    }
}
