// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The ways of a cabinet (doc 12 §8, §9) on a small cabinet of its own parts: where a placed device's pins stand,
//! the ducts as a graph, the entries into a duct, the direct way, the pass with its room, the status of a stored way,
//! a wire's length and a duct's fill.
//!
//! The cabinet: a plate with rail 1 at v = 580 mm and rail 2 at v = 270 mm, both from u = 0 to 500 mm, and duct 1
//! (40 × 60) between them at v = 440 mm from u = 0 to 500 mm. Its devices are of two parts made here: K (40 × 80 × 60
//! mm, its pin 1 wired at the top by the part) and PLAIN (40 × 80, no depth, no edges). The symbol draws K's pins
//! downwards and PLAIN's pin 1 upwards, pin 2 downwards.

use crate::common;

use common::{Fx, LIBRARY, id, sha, uid};
use ew_derive::layout::{
    DuctFill, DuctGraph, InnerFrom, NoWay, PinSpots, Planned, RoutePlan, RouteScope, RouteStatus, StaleWhy,
    entries, plan_routes, route_status,
};
use ew_derive::{Q, WireLength, WireRecordSet};
use ew_ids::{Id, Uuid, v5};
use ew_schema::{FnDef, PinFace, Record, WayStep};
use ew_units::{Nm, Pt2};
use serde_json::{Value, json};

const MM: i64 = 1_000_000;

fn mm(v: i64) -> i64 {
    v * MM
}

const LOC: u128 = 0x10;
const SPACE: u128 = 0x20;
const ENCL: u128 = 0x21;
const PLATE: u128 = 0x22;
const R1: u128 = 0x23;
const R2: u128 = 0x24;
const D1: u128 = 0x25;
const D_TOP: u128 = 0x26;
const D_RISE: u128 = 0x27;
const D_LOW: u128 = 0x28;
const PAGE: u128 = 0x30;
const F2: u128 = 0x500;
const SYM_DOWN: u128 = 0x510;
const SYM_UP: u128 = 0x511;
const PART_K: u128 = 0x600;
const PART_PLAIN: u128 = 0x601;
const PART_DUCT: u128 = 0x602;
const PART_WIRE: u128 = 0x603;
const U: u128 = 0xe0;
const K1: u128 = 0xd1; // rail 1, s = 100
const K2: u128 = 0xd2; // rail 1, s = 160, beside K1
const K3: u128 = 0xd3; // rail 2, s = 103, PLAIN
const K4: u128 = 0xd4; // rail 1, s = 400, PLAIN, both pins on its bottom edge
const BLOCK: u128 = 0xd5; // on the plate at (395, 490), an unknown footprint under K4's pin 1
const W1: u128 = 0x4001; // K1:1 → K3:1, through duct 1
const W2: u128 = 0x4002; // K1:2 → K2:2, direct
const W3: u128 = 0x4003; // K4:1 → K3:2, no way: K4:1 is behind BLOCK
const W4: u128 = 0x4004; // K1:2 → K4:2, not neighbours: through duct 1

fn f(device: u128) -> u128 {
    device + 0x1000
}

fn pl(device: u128) -> u128 {
    device + 0x2000
}

fn cp(device: u128, idx: u16) -> Value {
    json!({"cp": [uid(f(device)), idx]})
}

fn fndef_copy() -> String {
    let copy: Id<FnDef> = v5::lib_copy_id(Uuid::from_u128(LIBRARY), Uuid::from_u128(F2));
    copy.as_uuid().hyphenated().to_string()
}

fn pose(s: i64, half: bool) -> Value {
    json!({"t": [s, 0, 0], "q": ew_derive::layout::Quarter::new(if half { 2 } else { 0 }).quat()})
}

fn carrier(n: u128, kind: &str, path: &[(i64, i64)], params: Value) -> Value {
    let path: Vec<Value> = path.iter().map(|(u, v)| json!([mm(*u), mm(*v)])).collect();
    json!({"$k": "carrier", "id": uid(n), "surface": uid(PLATE), "kind": kind, "path": path, "params": params})
}

fn duct(n: u128, path: &[(i64, i64)], w: i64, h: i64) -> Value {
    carrier(
        n,
        "duct",
        path,
        json!({"duct": {"width": mm(w), "height": mm(h)}}),
    )
}

fn symbol(fx: &mut Fx, n: u128, dirs: [&str; 2]) {
    fx.lib_item(
        "symbol",
        n,
        json!({
            "meta": {}, "name": "symbol",
            "fndef": {"library": uid(LIBRARY), "item": uid(F2), "version_req": "^1"},
            "grid": {"num": 4_000_000, "den": 1},
            "cps": [
                {"idx": 0, "fn_cp": 0, "pos": [0, 0], "dir": dirs[0]},
                {"idx": 1, "fn_cp": 1, "pos": [0, 8_000_000], "dir": dirs[1]},
            ],
            "anchor": "origin",
        }),
    );
}

fn part(fx: &mut Fx, n: u128, mech: Value, pattern: Value) {
    fx.lib_item(
        "part",
        n,
        json!({
            "meta": {"key": format!("P{n}")}, "part_number": format!("P{n}"),
            "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9000), "version_req": "^1"},
            "mech": mech, "cp_pattern": pattern,
            "function_template": [{"fndef": {"library": uid(LIBRARY), "item": uid(F2), "version_req": "^1"}, "main": true}],
        }),
    );
}

fn device(fx: &mut Fx, n: u128, counter: u32, part: Option<u128>, symbol: u128) {
    fx.device(n, "K", Some(counter))
        .with("structure", json!({"location": uid(LOC)}));
    if let Some(p) = part {
        fx.with("parts", json!([{"part": {"copy": uid(p)}, "qty": "1"}]));
        fx.push(json!({"$k": "function", "id": uid(f(n)), "device": uid(n), "def": fndef_copy()}));
        fx.rep(n + 0x3000, PAGE, symbol, &[f(n)], mm(i64::from(counter) * 20), 0);
    }
}

fn wire(fx: &mut Fx, n: u128, a: Value, b: Value) {
    fx.push(json!({
        "$k": "connection", "id": uid(n), "page": uid(PAGE), "a": a, "b": b, "overrides": {"csa": "1.5"},
    }));
}

/// The cabinet; `loop_` adds a second way round: a duct above rail 1, one below rail 2 and one rising between them
/// beside duct 1, which then stops at u = 400 mm.
fn cabinet(loop_: bool) -> Fx {
    let mut fx = Fx::new();
    fx.node(LOC, "location", "CAB", None);
    fx.lib(json!({
        "$k": "fndef", "id": fndef_copy(), "library_id": uid(LIBRARY), "item_id": uid(F2), "version": "1.0.0",
        "sha256": sha(0x42), "meta": {}, "category": ["Switching", "Contactor", "Coil"], "contact_kind": "pole",
        "cps": [{"idx": 0, "designation": "1"}, {"idx": 1, "designation": "2"}],
    }));
    symbol(&mut fx, SYM_DOWN, ["down", "down"]);
    symbol(&mut fx, SYM_UP, ["up", "down"]);
    let k = json!({"width": mm(40), "height": mm(80), "depth": mm(60), "mount_kind": "ts35x7_5"});
    let plain = json!({"width": mm(40), "height": mm(80), "mount_kind": "ts35x7_5"});
    part(
        &mut fx,
        PART_K,
        k,
        json!([{"idx": 0, "designation": "1", "face": "top"}, {"idx": 1, "designation": "2"}]),
    );
    part(
        &mut fx,
        PART_PLAIN,
        plain,
        json!([{"idx": 0, "designation": "1"}, {"idx": 1, "designation": "2"}]),
    );
    fx.page(PAGE, 1, None, "multi_line", &[], "one");
    fx.device(U, "U", Some(1))
        .with("structure", json!({"location": uid(LOC)}));
    device(&mut fx, K1, 1, Some(PART_K), SYM_DOWN);
    device(&mut fx, K2, 2, Some(PART_K), SYM_DOWN);
    device(&mut fx, K3, 3, Some(PART_PLAIN), SYM_UP);
    device(&mut fx, K4, 4, Some(PART_PLAIN), SYM_DOWN);
    device(&mut fx, BLOCK, 5, None, SYM_UP);
    let identity = json!({"t": [0, 0, 0], "q": [1_000_000_000, 0, 0, 0]});
    fx.push(json!({"$k": "layout_space", "id": uid(SPACE), "location": uid(LOC)}))
        .push(json!({"$k": "enclosure", "id": uid(ENCL), "space": uid(SPACE), "device": uid(U), "pose": identity}))
        .push(json!({"$k": "mounting_surface", "id": uid(PLATE), "enclosure": uid(ENCL), "kind": "plate",
                     "frame": identity}))
        .push(carrier(R1, "rail", &[(0, 580), (500, 580)], json!({"rail": {"profile": "ts35x7_5"}})))
        .push(carrier(R2, "rail", &[(0, 270), (500, 270)], json!({"rail": {"profile": "ts35x7_5"}})));
    if loop_ {
        fx.push(duct(D1, &[(0, 440), (400, 440)], 10, 10))
            .push(duct(D_TOP, &[(0, 700), (450, 700)], 40, 60))
            .push(duct(D_RISE, &[(450, 150), (450, 700)], 40, 60))
            .push(duct(D_LOW, &[(0, 150), (450, 150)], 40, 60));
        fx.project_set("settings", json!({"workshop": {"fill_limit": 15}}));
    } else {
        fx.push(duct(D1, &[(0, 440), (500, 440)], 40, 60));
    }
    let on = |rail: u128, s: i64| (json!({"carrier": uid(rail)}), pose(mm(s), false));
    for (d, (carrier, pose)) in [
        (K1, on(R1, 100)),
        (K2, on(R1, 160)),
        (K3, on(R2, 103)),
        (K4, on(R1, 400)),
    ] {
        fx.push(
            json!({"$k": "placement", "id": uid(pl(d)), "device": uid(d), "carrier": carrier, "pose": pose}),
        );
    }
    fx.push(json!({"$k": "placement", "id": uid(pl(BLOCK)), "device": uid(BLOCK),
                   "carrier": {"surface": uid(PLATE)}, "pose": {"t": [mm(395), mm(490), 0], "q": [1_000_000_000, 0, 0, 0]}}));
    wire(&mut fx, W1, cp(K1, 0), cp(K3, 0));
    wire(&mut fx, W2, cp(K1, 1), cp(K2, 1));
    wire(&mut fx, W3, cp(K4, 0), cp(K3, 1));
    wire(&mut fx, W4, cp(K1, 1), cp(K4, 1));
    fx
}

fn at(u: i64, v: i64) -> Pt2 {
    Pt2::new(Nm(mm(u)), Nm(mm(v)))
}

fn plan(q: &Q, scope: &RouteScope) -> RoutePlan {
    plan_routes(q, id(SPACE), scope)
}

fn planned(p: &RoutePlan, wire: u128) -> Planned {
    p.wires
        .iter()
        .find(|w| w.wire.wire.id == id(wire))
        .map(|w| w.planned.clone())
        .unwrap_or_else(|| panic!("no wire {wire:#x}"))
}

/// The fixture with the ways of `plan` stored, as `layout.route` stores them.
fn routed(mut fx: Fx) -> Fx {
    let m = fx.build();
    let q = Q::new(&m);
    for w in plan(&q, &RouteScope::All).wires {
        if let Planned::Way(way) = w.planned {
            let r = Record::CommittedRoute(way.record(id(SPACE), w.wire.wire.id));
            fx.push(serde_json::to_value(r).unwrap());
        }
    }
    fx
}

fn status(q: &Q, wire: u128) -> RouteStatus {
    let set = q.get::<WireRecordSet>(());
    let r = set.iter().find(|r| r.wire.id == id(wire)).expect("the wire");
    route_status(q, &r.wire)
}

fn length(q: &Q, wire: u128) -> WireLength {
    q.get::<WireRecordSet>(())
        .iter()
        .find(|r| r.wire.id == id(wire))
        .map(|r| r.length)
        .expect("the wire")
}

/// A pin stands where its part says, else where its drawing shows it: K's pin 1 at the top though its symbol draws it
/// downwards, its pin 2 at the bottom; PLAIN's pin 1, drawn upwards, at the top. Two pins of one edge share it; a pin's
/// height is the rail's plus half the depth, the rail's alone without a depth; a half turn takes the edges along.
#[test]
fn pins_stand_on_the_edge_their_part_or_their_drawing_gives_them() {
    let m = cabinet(false).build();
    let q = Q::new(&m);
    let spots = |d: u128| q.get::<PinSpots>(id(pl(d))).as_ref().clone().expect("pins");
    let k1 = spots(K1);
    let (one, two) = (k1.spot((id(f(K1)), 0)).unwrap(), k1.spot((id(f(K1)), 1)).unwrap());
    assert_eq!(
        (one.at, one.out, one.face.clone(), one.drawn),
        (at(100, 620), (0, 1), PinFace::Top, false)
    );
    assert_eq!(
        (two.at, two.out, two.face.clone(), two.drawn),
        (at(100, 540), (0, -1), PinFace::Bottom, true)
    );
    assert_eq!(one.h, Nm(37_500_000), "7.5 mm of rail and half of 60 mm");
    let k3 = spots(K3);
    let up = k3.spot((id(f(K3)), 0)).unwrap();
    assert_eq!(
        (up.at, up.face.clone(), up.drawn, up.h),
        (at(103, 310), PinFace::Top, true, Nm(7_500_000))
    );
    let k4 = spots(K4);
    let bottom: Vec<Pt2> = k4.spots.iter().map(|s| s.at).collect();
    assert_eq!(
        bottom,
        [at(390, 540), at(410, 540)],
        "a quarter and three quarters of the edge"
    );

    let mut fx = cabinet(false);
    fx.record(pl(K1))["pose"] = pose(mm(100), true);
    let m = fx.build();
    let q = Q::new(&m);
    let turned = q.get::<PinSpots>(id(pl(K1))).as_ref().clone().unwrap();
    let one = turned.spot((id(f(K1)), 0)).unwrap();
    assert_eq!((one.at, one.out), (at(100, 540), (0, -1)));
}

/// A duct joint by the carriers' IDs: `(a, position along a, b, position along b, length)`.
type Joined = (Id<ew_schema::Carrier>, i128, Id<ew_schema::Carrier>, i128, i128);

/// The joints of the plate's ducts by their carriers.
fn joined(q: &Q) -> Vec<Joined> {
    let g = q.get::<DuctGraph>(id(PLATE));
    g.joints
        .iter()
        .map(|j| {
            (
                g.ducts[j.a.0].carrier,
                j.a.1,
                g.ducts[j.b.0].carrier,
                j.b.1,
                j.len,
            )
        })
        .collect()
}

/// Two ducts are joined where their outlines touch or overlap: a T at the rising duct's foot, the corners of the
/// loop at their shared points; two ducts apart are not joined.
#[test]
fn ducts_are_joined_where_their_outlines_touch() {
    let m = cabinet(true).build();
    let q = Q::new(&m);
    let named = joined(&q);
    let mm = |v: i64| i128::from(mm(v));
    assert_eq!(
        named,
        [
            (id(D_TOP), mm(450), id(D_RISE), mm(550), 0),
            (id(D_RISE), 0, id(D_LOW), mm(450), 0),
        ],
        "duct 1 (u to 400) stands apart from the rising duct (u 430 to 470)"
    );

    // A T: a duct ending at another's side, and a cross.
    let mut fx = Fx::new();
    fx.node(LOC, "location", "CAB", None)
        .device(U, "U", Some(1))
        .push(json!({"$k": "layout_space", "id": uid(SPACE), "location": uid(LOC)}))
        .push(
            json!({"$k": "enclosure", "id": uid(ENCL), "space": uid(SPACE), "device": uid(U),
                     "pose": {"t": [0, 0, 0], "q": [1_000_000_000, 0, 0, 0]}}),
        )
        .push(
            json!({"$k": "mounting_surface", "id": uid(PLATE), "enclosure": uid(ENCL), "kind": "plate",
                     "frame": {"t": [0, 0, 0], "q": [1_000_000_000, 0, 0, 0]}}),
        )
        .push(duct(D1, &[(0, 0), (400, 0)], 40, 60))
        .push(duct(D_TOP, &[(200, 20), (200, 300)], 40, 60))
        .push(duct(D_RISE, &[(0, 1000), (400, 1000)], 40, 60))
        .push(duct(D_LOW, &[(200, 900), (200, 1100)], 40, 60));
    let m = fx.build();
    let q = Q::new(&m);
    let named = joined(&q);
    assert_eq!(
        named,
        [
            (id(D1), mm(200), id(D_TOP), 0, mm(20)),
            (id(D_RISE), mm(200), id(D_LOW), mm(100), 0),
        ]
    );
}

/// A pin enters a duct straight out of its edge, or back across its own device's front; a line that meets another
/// footprint first is not taken.
#[test]
fn a_pin_enters_the_first_duct_its_line_meets_unless_a_footprint_stands_between() {
    let m = cabinet(false).build();
    let q = Q::new(&m);
    let g = q.get::<DuctGraph>(id(PLATE));
    let others: Vec<ew_units::Rect> = [K1, K2, K3, K4, BLOCK]
        .iter()
        .filter_map(|d| {
            q.get::<PinSpots>(id(pl(*d)))
                .as_ref()
                .as_ref()
                .map(|s| s.footprint)
        })
        .collect();
    let of = |d: u128, idx: u16| {
        let s = q.get::<PinSpots>(id(pl(d))).as_ref().clone().unwrap();
        let spot = s.spot((id(f(d)), idx)).unwrap().clone();
        entries(&g, &spot, s.footprint, &others)
            .iter()
            .map(|e| (e.pos, e.stub, e.climb))
            .collect::<Vec<_>>()
    };
    let mm = |v: i64| i128::from(mm(v));
    assert_eq!(
        of(K3, 0),
        [(mm(103), mm(130), 22_500_000)],
        "forward, up from rail 2"
    );
    assert_eq!(
        of(K1, 0),
        [(mm(100), mm(180), 7_500_000)],
        "back across K1's front"
    );
    assert_eq!(of(K4, 0), [], "BLOCK stands between K4:1 and duct 1");
    assert_eq!(of(K4, 1).len(), 1, "K4:2 passes beside BLOCK");
}

/// The plan: a way through the duct with its run, a direct way between neighbours, none behind a footprint (in words
/// of the pin and its rail), and a duct way between devices that are not neighbours; the same plan from the records
/// read in another order.
#[test]
fn route_lays_each_wire_its_shortest_way() {
    let fx = cabinet(false);
    let m = fx.build();
    let q = Q::new(&m);
    let p = plan(&q, &RouteScope::All);
    let Planned::Way(w1) = planned(&p, W1) else {
        panic!("W1 has a way")
    };
    assert!(
        matches!(&w1.steps[..], [WayStep::Duct(d)] if d.carrier == id(D1) && d.from == Nm(mm(100)) && d.to == Nm(mm(103)))
    );
    assert_eq!(w1.run, Nm(mm(343)), "180 + 7.5 + 3 + 130 + 22.5");
    let Planned::Way(w2) = planned(&p, W2) else {
        panic!("W2 has a way")
    };
    assert!(w2.steps.is_empty(), "direct");
    assert_eq!(w2.run, Nm(mm(60)));
    assert_eq!(
        planned(&p, W3),
        Planned::NoWay(NoWay::NoDuct {
            cp: (id(f(K4)), 0),
            rail: Some(id(R1))
        })
    );
    let Planned::Way(w4) = planned(&p, W4) else {
        panic!("W4 has a way")
    };
    assert_eq!(w4.steps.len(), 1, "K2 stands between K1 and K4: through the duct");

    let mut reversed = fx.clone();
    reversed.records.reverse();
    let m2 = reversed.build();
    assert_eq!(plan(&Q::new(&m2), &RouteScope::All), p);
}

/// The pass: each wire takes the shortest way that has room; when the first filled duct 1 to the limit, the second
/// goes round; with no way of room the wire takes the shortest all the same.
#[test]
fn a_wire_goes_round_a_full_duct_and_takes_the_shortest_where_none_has_room() {
    let mut fx = cabinet(true);
    // Two more wires through duct 1 between u = 100 and 103; one fills its 10 × 10 (85 mm²) to 10 %, the limit is 15 %.
    wire(&mut fx, 0x4005, cp(K1, 1), cp(K3, 0));
    wire(&mut fx, 0x4006, cp(K1, 0), cp(K3, 1));
    let m = fx.build();
    let q = Q::new(&m);
    let p = plan(&q, &RouteScope::All);
    let carriers = |p: &RoutePlan, w: u128| match planned(p, w) {
        Planned::Way(way) => {
            let mut c: Vec<_> = way
                .steps
                .iter()
                .filter_map(|s| match s {
                    WayStep::Duct(d) => Some(d.carrier),
                    _ => None,
                })
                .collect();
            c.sort();
            c
        }
        other => panic!("{other:?}"),
    };
    // W1 and the two wires all want duct 1 between u = 100 and 103: the first in the list's order takes it, the
    // others go round.
    let ways: Vec<Vec<_>> = [W1, 0x4005, 0x4006].iter().map(|w| carriers(&p, *w)).collect();
    assert_eq!(ways.iter().filter(|c| **c == [id(D1)]).count(), 1, "{ways:?}");
    let round: Vec<Id<ew_schema::Carrier>> = vec![id(D_TOP), id(D_RISE), id(D_LOW)];
    assert_eq!(ways.iter().filter(|c| **c == round).count(), 2, "{ways:?}");

    // With duct 1 alone the second wire takes it all the same, over the limit.
    let mut fx = cabinet(true);
    fx.records.retain(|r| {
        ![uid(D_TOP), uid(D_RISE), uid(D_LOW)].contains(&r["id"].as_str().unwrap_or("").to_owned())
    });
    wire(&mut fx, 0x4005, cp(K1, 1), cp(K3, 0));
    wire(&mut fx, 0x4006, cp(K1, 0), cp(K3, 1));
    let m = fx.build();
    let q = Q::new(&m);
    let p = plan(&q, &RouteScope::All);
    for w in [W1, 0x4005, 0x4006] {
        assert_eq!(carriers(&p, w), [id(D1)]);
    }
}

/// A stored way holds while its ends are the pins as they are and its run measured again is the run it stored; it is
/// stale when a device moved, an end lands on another pin, a device is no longer placed, a duct moved across or is
/// gone; a duct moved along itself leaves the run as it was, and the way holds.
#[test]
fn a_stored_way_holds_until_what_it_was_laid_from_changes() {
    let base = routed(cabinet(false));
    let m = base.build();
    let q = Q::new(&m);
    assert_eq!(
        status(&q, W1),
        RouteStatus::Holds {
            run: Nm(mm(343)),
            door: false
        }
    );
    let stale = |fx: Fx, wire: u128| match status(&Q::new(&fx.build()), wire) {
        RouteStatus::Stale { why, .. } => Some(why),
        _ => None,
    };
    let mut moved = base.clone();
    moved.record(pl(K1))["pose"] = pose(mm(110), false);
    assert_eq!(stale(moved, W1), Some(StaleWhy::Moved(id(K1))));
    let mut end = base.clone();
    end.record(W1)["a"] = cp(K1, 1);
    assert_eq!(
        stale(end, W1),
        Some(StaleWhy::EndChanged {
            from: (id(f(K1)), 0),
            to: (id(f(K1)), 1)
        })
    );
    let mut unplaced = base.clone();
    unplaced.records.retain(|r| r["id"] != json!(uid(pl(K3))));
    assert_eq!(stale(unplaced, W1), Some(StaleWhy::Unplaced(id(K3))));
    let mut across = base.clone();
    across.record(D1)["path"] = json!([[0, mm(450)], [mm(500), mm(450)]]);
    // W4's ends both stand above duct 1: moved 10 mm up, its run is 20 mm shorter. (W1's ends stand on either side, and
    // its run stays.)
    assert_eq!(stale(across, W4), Some(StaleWhy::DuctChanged(id(D1))));
    let mut along = base.clone();
    along.record(D1)["path"] = json!([[mm(-50), mm(440)], [mm(450), mm(440)]]);
    assert!(matches!(
        status(&Q::new(&along.build()), W1),
        RouteStatus::Holds { .. }
    ));
    let mut gone = base.clone();
    gone.records.retain(|r| r["id"] != json!(uid(D1)));
    assert_eq!(stale(gone, W1), Some(StaleWhy::DuctGone(id(D1))));
    // A direct way holds whatever the ducts do.
    assert!(matches!(status(&q, W2), RouteStatus::Holds { .. }));
}

/// A typed length wins and is not rounded; a routed one is the run with the reserve at both ends rounded up once to
/// the cut increment; a stale way keeps its length; a change of the reserve changes it at once.
#[test]
fn a_routed_length_is_made_from_the_stored_run() {
    let base = routed(cabinet(false));
    let m = base.build();
    let q = Q::new(&m);
    assert_eq!(
        length(&q, W1),
        WireLength::Routed {
            length: Nm(mm(445)),
            run: Nm(mm(343)),
            door: false
        },
        "343 + 2 × 50 = 443, rounded up to 445"
    );
    let mut typed = base.clone();
    typed.record(W1)["overrides"]["length"] = json!(401_000_000);
    assert_eq!(
        length(&Q::new(&typed.build()), W1),
        WireLength::Typed(Nm(401_000_000))
    );
    let mut moved = base.clone();
    moved.record(pl(K1))["pose"] = pose(mm(110), false);
    assert!(matches!(
        length(&Q::new(&moved.build()), W1),
        WireLength::Stale {
            length: Nm(445_000_000),
            why: StaleWhy::Moved(_),
            ..
        }
    ));
    let mut reserve = base.clone();
    reserve.project_set("settings", json!({"workshop": {"reserve": mm(60)}}));
    assert_eq!(length(&Q::new(&reserve.build()), W1).value(), Some(Nm(mm(465))));
    // A project without stored ways has no length for the wire.
    assert_eq!(length(&Q::new(&cabinet(false).build()), W1), WireLength::Missing);
}

/// A duct's fill: per piece between the points where ways enter or leave it, the fullest; the inner area its part
/// gives, else 85 % of its nominal size; a default diameter flagged; a wire without a diameter not counted.
#[test]
fn a_duct_fill_counts_the_stored_ways_along_its_pieces() {
    let mut fx = cabinet(false);
    // W5 is a wire of the project's own wire part, 4 mm over its insulation, along duct 1 from K1:1 to K3:2.
    fx.lib_item(
        "part",
        PART_WIRE,
        json!({"meta": {"key": "WIRE"}, "part_number": "WIRE", "kind": "wire",
               "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9000), "version_req": "^1"},
               "sections": {"wire": {"csa": "1.5", "outer_diameter": mm(4)}}}),
    );
    wire(&mut fx, 0x4005, cp(K1, 0), cp(K3, 1));
    fx.record(0x4005)["overrides"] = json!({"wire_part": {"copy": uid(PART_WIRE)}});
    // W6, from K3:2 back across its front into duct 1, has a size the table of diameters does not hold.
    wire(&mut fx, 0x4006, cp(K3, 1), cp(K1, 1));
    fx.record(0x4006)["overrides"] = json!({"csa": {"awg": 18}});
    let base = routed(fx);
    let m = base.build();
    let q = Q::new(&m);
    let fill = q.get::<DuctFill>(id(D1)).as_ref().clone().expect("a fill");
    assert_eq!(
        fill.inner,
        (
            i128::from(mm(40)) * i128::from(mm(60)) * 85 / 100,
            InnerFrom::Share
        )
    );
    assert!(fill.default_diameter && fill.without_diameter == 1);
    let full = fill.pieces[fill.fullest.unwrap()];
    // Between u = 100 and 103 run W1 (1.5 mm², 3.3 mm by the table), W4 (the same) and W5 (4 mm of its part).
    assert_eq!(
        (full.from, full.to, full.wires),
        (i128::from(mm(100)), i128::from(mm(103)), 3)
    );
    let area = |d: i128| d * d * 785_398_163 / 1_000_000_000;
    assert_eq!(full.area, 2 * area(3_300_000) + area(4_000_000));
    assert!(!fill.over());

    let mut own = base.clone();
    own.lib_item(
        "part",
        PART_DUCT,
        json!({"meta": {"key": "DUCT"}, "part_number": "DUCT", "kind": "duct",
               "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9000), "version_req": "^1"},
               "sections": {"duct": {"inner_width": mm(36), "inner_height": mm(55)}}}),
    );
    own.record(D1)["part"] = json!({"copy": uid(PART_DUCT)});
    let m = own.build();
    let fill = Q::new(&m).get::<DuctFill>(id(D1)).as_ref().clone().unwrap();
    assert_eq!(
        fill.inner,
        (i128::from(mm(36)) * i128::from(mm(55)), InnerFrom::Part)
    );
}

/// A stored way of the space whose wire is no longer a wire of the cabinet is the plan's to remove.
#[test]
fn a_way_whose_wire_is_gone_is_removed_by_the_next_route() {
    let mut fx = routed(cabinet(false));
    let mut stray = fx
        .record(
            v5::route_id::<_, ew_schema::CommittedRoute>(id::<ew_schema::Connection>(W1))
                .as_uuid()
                .as_u128(),
        )
        .clone();
    stray["id"] = json!(uid(0x4999));
    fx.push(stray);
    let m = fx.build();
    let p = plan(&Q::new(&m), &RouteScope::Due);
    assert_eq!(p.gone, [id(0x4999)]);
}

const DOOR: u128 = 0x29;
const PART_LAMP: u128 = 0x604;
const L1: u128 = 0xd6; // in the door at (300, 560)
const L2: u128 = 0xd7; // in the door at (360, 560)
const K6: u128 = 0xd8; // rail 2, s = 300
const W7: u128 = 0x4007; // K2:1 → L1:1, through duct 1 and across the hinge
const W8: u128 = 0x4008; // L1:2 → L2:1, on the door
const W9: u128 = 0x4009; // L2:2 → K6:1, the door's end first

/// The cabinet with a door hinged on `hinge` in front of the plate (both in one front view at the origin) and two lamps
/// of a part made here in it (30 × 30, 40 mm behind the door).
fn door_cabinet(hinge: &str) -> Fx {
    let mut fx = cabinet(false);
    let sheet = |w: i64, h: i64| json!([[0, 0], [mm(w), 0], [mm(w), mm(h)], [0, mm(h)]]);
    fx.record(PLATE)["usable"] = sheet(500, 800);
    fx.push(
        json!({"$k": "mounting_surface", "id": uid(DOOR), "enclosure": uid(ENCL), "kind": "door",
                   "frame": {"t": [0, 0, mm(250)], "q": [1_000_000_000, 0, 0, 0]}, "hinge": hinge,
                   "usable": sheet(600, 800)}),
    );
    let lamp = json!({"width": mm(30), "height": mm(30), "depth": mm(40), "mount_kind": "door_cutout"});
    part(
        &mut fx,
        PART_LAMP,
        lamp,
        json!([{"idx": 0, "designation": "1"}, {"idx": 1, "designation": "2"}]),
    );
    device(&mut fx, L1, 6, Some(PART_LAMP), SYM_DOWN);
    device(&mut fx, L2, 7, Some(PART_LAMP), SYM_DOWN);
    device(&mut fx, K6, 8, Some(PART_K), SYM_DOWN);
    for (d, u) in [(L1, 300), (L2, 360)] {
        fx.push(
            json!({"$k": "placement", "id": uid(pl(d)), "device": uid(d), "carrier": {"surface": uid(DOOR)},
                       "pose": {"t": [mm(u), mm(560), 0], "q": [1_000_000_000, 0, 0, 0]}}),
        );
    }
    fx.push(
        json!({"$k": "placement", "id": uid(pl(K6)), "device": uid(K6), "carrier": {"carrier": uid(R2)},
                   "pose": pose(mm(300), false)}),
    );
    wire(&mut fx, W7, cp(K2, 0), cp(L1, 0));
    wire(&mut fx, W8, cp(L1, 1), cp(L2, 0));
    wire(&mut fx, W9, cp(L2, 1), cp(K6, 0));
    fx
}

/// A lamp's pin in the door: at its centre, half its 40 mm behind the door away from it.
fn door_pin(q: &Q, lamp: u128) -> (i64, i64) {
    let spots = q.get::<PinSpots>(id(pl(lamp)));
    let spots = spots.as_ref().as_ref().expect("the lamp's pins");
    let spot = &spots.spots[0];
    assert!(spots.spots.iter().all(|s| s.at == spot.at && s.out == (0, 0)));
    assert_eq!(spot.h, Nm(mm(20)));
    (spot.at.x.0, spot.at.y.0)
}

/// Into a door (doc 12 §8.1, §9): a wire runs through the ducts to the duct's end on the hinge side, in free air to the
/// plate's edge, along the door to the lamp and up half what the lamp reaches behind the door; its length adds the door
/// loop. Two lamps of one door are joined directly; a wire whose door end is its first stores the door step first.
/// Hinged on the right, the way leaves the duct at its right end.
#[test]
fn a_wire_into_the_door_crosses_the_hinge_and_runs_along_the_door() {
    let base = routed(door_cabinet("left"));
    let m = base.build();
    let q = Q::new(&m);
    let ((x1, y1), (x2, y2)) = (door_pin(&q, L1), door_pin(&q, L2));
    let p = plan(&q, &RouteScope::All);
    let Planned::Way(w7) = planned(&p, W7) else {
        panic!("W7 has a way")
    };
    assert!(
        matches!(&w7.steps[..], [WayStep::Duct(d), WayStep::Door(s)]
            if d.carrier == id(D1) && d.from == Nm(mm(160)) && d.to == Nm(0) && *s == id(DOOR)),
        "{:?}",
        w7.steps
    );
    // K2's pin 1 back across its front 180, climb 7.5, along duct 1 to its end 160, free air 0, along the door, up 20.
    let along = x1 + (y1 - mm(440)).abs();
    assert_eq!(w7.run, Nm(mm(180) + 7_500_000 + mm(160) + along + mm(20)));
    assert!(w7.door());
    let Planned::Way(w8) = planned(&p, W8) else {
        panic!("W8 has a way")
    };
    assert!(w8.steps.is_empty(), "two lamps of one door: direct");
    assert_eq!(w8.run, Nm((x2 - x1).abs() + (y2 - y1).abs()));
    let Planned::Way(w9) = planned(&p, W9) else {
        panic!("W9 has a way")
    };
    assert!(
        matches!(&w9.steps[..], [WayStep::Door(s), WayStep::Duct(d)]
            if *s == id(DOOR) && d.from == Nm(0) && d.to == Nm(mm(300))),
        "{:?}",
        w9.steps
    );
    assert_eq!(
        w9.run,
        Nm(x2 + (y2 - mm(440)).abs() + mm(20) + mm(300) + mm(130) + 7_500_000)
    );
    assert_eq!(
        status(&q, W7),
        RouteStatus::Holds {
            run: w7.run,
            door: true
        }
    );
    assert!(matches!(status(&q, W9), RouteStatus::Holds { door: true, .. }));
    assert_eq!(
        length(&q, W7).value(),
        Some(ew_derive::layout::routed_length(&q, w7.run, true))
    );
    let loop_ = ew_derive::layout::routed_length(&q, w7.run, true).0
        - ew_derive::layout::routed_length(&q, w7.run, false).0;
    assert!((mm(295)..=mm(305)).contains(&loop_), "the 300 mm door loop");

    let m = door_cabinet("right").build();
    let q = Q::new(&m);
    let Planned::Way(right) = planned(&plan(&q, &RouteScope::All), W7) else {
        panic!("W7 has a way")
    };
    assert!(matches!(&right.steps[..], [WayStep::Duct(d), WayStep::Door(_)] if d.to == Nm(mm(500))));
    let along = (x1 - mm(600)).abs() + (y1 - mm(440)).abs();
    assert_eq!(right.run, Nm(mm(180) + 7_500_000 + mm(340) + along + mm(20)));
}
