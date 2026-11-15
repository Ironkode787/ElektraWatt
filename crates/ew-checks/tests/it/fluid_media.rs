// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The EW-FL 1.0 rules that compare what a circuit joins (doc 11 §6): EW-FL-005, -006, -009 and -012, each made to
//! report on a small circuit and kept silent on the correct one beside it.

use crate::common;

use common::{Ep, Fx, P1};
use common::{dutch, findings_of, us};
use serde_json::{Value, json};

const VALVE_DEF: u128 = 0x540;
const VALVE_SYM: u128 = 0x541;
const QE_DEF: u128 = 0x542;
const QE_SYM: u128 = 0x543;
const ONE_DEF: u128 = 0x544;
const ONE_SYM: u128 = 0x545;
const CYL_DEF: u128 = 0x546;
const CYL_SYM: u128 = 0x547;
const REG_DEF: u128 = 0x548;
const REG_SYM: u128 = 0x549;
const COIL_DEF: u128 = 0x54a;
const COIL_SYM: u128 = 0x54b;
const PLATE_DEF: u128 = 0x54c;
const PLATE_SYM: u128 = 0x54d;
const REG_PART: u128 = 0x650;
const TUBE_PART: u128 = 0x651;
const VALVE_PART: u128 = 0x652;
const PLATE_PART: u128 = 0x653;
const AIR: u128 = 0x660;
const AIR_4: u128 = 0x661;

fn id<T>(n: u128) -> ew_derive::ew_ids::Id<T> {
    ew_derive::ew_ids::Id::from_uuid(ew_derive::ew_ids::Uuid::from_u128(n))
}

fn port(role: &str) -> Value {
    json!({"discipline": "pneumatic", "port_role": role})
}

/// The definitions: a 5/2 valve (ports 1 to 5, a model 1→4, 2→3 / 1→2, 4→5), a quick-exhaust valve (1, 2, 3), a
/// one-port end (a silencer, an air supply), a cylinder (A, B), a regulator (1→2, 2→3), a physical coil (A1, A2)
/// and a supply plate (1, 3, 5); the parts: a regulator setting 4 bar, a tube rated 4 bar of 6 mm, a valve with a
/// 24 V DC coil and a supply plate tapping gallery 1; the media air at 6 bar and air at 4 bar.
fn base() -> Fx {
    let mut fx = Fx::base();
    let flow = |from: u16, to: u16| json!({"flow": {"from": from, "to": to}});
    fx.fndef_with(
        VALVE_DEF,
        VALVE_SYM,
        &[
            (0, "1", port("supply1")),
            (1, "2", port("working2")),
            (2, "3", port("exhaust3")),
            (3, "4", port("working4")),
            (4, "5", port("exhaust5")),
        ],
        &json!([]),
        &json!({"contact_kind": "valve", "valve_model": {"sections": [{"positions": [
            {"label": "a", "paths": [flow(0, 3), flow(1, 2)]},
            {"label": "b", "paths": [flow(0, 1), flow(3, 4)]}], "rest": 1}]}}),
    );
    fx.fndef_with(
        QE_DEF,
        QE_SYM,
        &[(0, "1", port("supply1")), (1, "2", port("working2")), (2, "3", port("exhaust3"))],
        &json!([]),
        &json!({"valve_model": {"sections": [{"positions": [{"label": "0", "paths": [flow(0, 1), flow(1, 2)]}]}]}}),
    );
    fx.fndef_with(
        ONE_DEF,
        ONE_SYM,
        &[(0, "1", port("generic"))],
        &json!([]),
        &json!({}),
    );
    fx.fndef_with(
        CYL_DEF,
        CYL_SYM,
        &[(0, "A", port("consumer")), (1, "B", port("consumer"))],
        &json!([]),
        &json!({}),
    );
    fx.fndef_with(
        REG_DEF,
        REG_SYM,
        &[(0, "1", port("supply1")), (1, "2", port("working2")), (2, "3", port("exhaust3"))],
        &json!([]),
        &json!({"valve_model": {"sections": [{"positions": [{"label": "0", "paths": [flow(0, 1), flow(1, 2)]}]}]}}),
    );
    fx.fndef_with(
        COIL_DEF,
        COIL_SYM,
        &[(0, "A1", Value::Null), (1, "A2", Value::Null)],
        &json!([]),
        &json!({"contact_kind": "coil"}),
    );
    fx.fndef_with(
        PLATE_DEF,
        PLATE_SYM,
        &[
            (0, "1", port("supply1")),
            (1, "3", port("exhaust3")),
            (2, "5", port("exhaust5")),
        ],
        &json!([]),
        &json!({"category": ["ValveTerminal", "SupplyPlate", "Generic"]}),
    );
    fx.part(
        REG_PART,
        "REG-4",
        &json!({"sections": {"fluid": {"transform": {"regulator": {"outlet": "2", "p_set": "4"}}}}}),
    );
    fx.part(
        TUBE_PART,
        "PUN-6",
        &json!({"sections": {"fluid": {"tube": {"od": 6_000_000, "id": 4_000_000, "material": "pu", "p_rating": "4"}}}}),
    );
    fx.part(
        VALVE_PART,
        "VLV-24",
        &json!({"sections": {"fluid": {"solenoids": ["14"], "coil": {"voltage": "24", "env": "dc"}}}}),
    );
    fx.part(
        PLATE_PART,
        "PLATE-1",
        &json!({"sections": {"fluid": {"galleries": [{"gallery": "supply1", "tap": "1"}]}}}),
    );
    for (n, name, p) in [(AIR, "air 6 bar", "6"), (AIR_4, "air 4 bar", "4")] {
        fx.push(json!({"$k": "medium", "id": us(n), "name": {"en": name, "nl": name}, "p_nom": p}));
    }
    fx
}

/// A device `n` (tag `-<letter><counter>`) with one function `n + 0x100` of `def`, drawn on P1.
fn part_of(fx: &mut Fx, n: u128, letter: &str, counter: u32, (def, sym): (u128, u128)) {
    fx.device(n, letter, Some(counter));
    fx.push(json!({"$k": "function", "id": us(n + 0x100), "device": us(n), "def": us(def)}));
    fx.push(json!({
        "$k": "representation", "id": us(n + 0x200), "functions": [us(n + 0x100)], "page": us(P1),
        "symbol": us(sym), "pos": [counter as i64 * 40_000_000, 0],
    }));
}

/// Port `idx` of the function of device `n`.
fn at(n: u128, idx: u16) -> Ep {
    Ep::Cp(n + 0x100, idx)
}

fn line(fx: &mut Fx, n: u128, a: Ep, b: Ep) {
    fx.conn(n, P1, a, b, &json!({"kind": {"fluid": {}}}));
}

fn medium_def(fx: &mut Fx, n: u128, a: Ep, medium: u128) {
    let Ep::Cp(f, idx) = a else { unreachable!() };
    fx.push(json!({"$k": "medium_def", "id": us(n), "at": {"cp": [us(f), idx]}, "medium": us(medium)}));
}

fn messages(fx: &Fx, rule: &str) -> Vec<String> {
    let mut out: Vec<String> = findings_of(&fx.build(), rule)
        .into_iter()
        .map(|f| f.message)
        .collect();
    out.sort();
    out
}

const V1: u128 = 0x7101;
const V2: u128 = 0x7102;
const QE: u128 = 0x7103;
const SIL: u128 = 0x7104;
const CYL: u128 = 0x7105;
const SRC: u128 = 0x7106;
const REG: u128 = 0x7107;
const VT: u128 = 0x7108;
const PLATE: u128 = 0x7109;
const TEE: u128 = 0x710a;
const CFG: u128 = 0x710b;

/// EW-FL-012 (FR-FLU-032): a valve's working port into another valve's exhaust and two ports of one valve joined
/// report; a working port into a quick-exhaust valve's inlet, an on/off valve into a manifold's port 1 and the two
/// exhausts of one valve teed into one silencer are correct circuits and report nothing.
#[test]
fn ew_fl_012_reports_a_real_port_conflict_only() {
    let valves = |fx: &mut Fx| {
        part_of(fx, V1, "Q", 1, (VALVE_DEF, VALVE_SYM));
        part_of(fx, V2, "Q", 2, (VALVE_DEF, VALVE_SYM));
    };
    // V1:2 into V2's exhaust 3; V1:4 joined to its own port 1.
    let mut fx = base();
    valves(&mut fx);
    line(&mut fx, 0x4001, at(V1, 1), at(V2, 2));
    line(&mut fx, 0x4002, at(V1, 3), at(V1, 0));
    assert_eq!(
        messages(&fx, "EW-FL-012"),
        [
            "-1Q1:1 and -1Q1:4 are ports of one valve and are joined",
            "working port -1Q1:2 is joined to -1Q2:3 with no valve between them",
        ]
    );
    assert_eq!(
        dutch(&fx.build(), "EW-FL-012"),
        [
            "-1Q1:1 en -1Q1:4 zijn poorten van één ventiel en zijn verbonden",
            "werkpoort -1Q1:2 is verbonden met -1Q2:3 zonder ventiel ertussen",
        ]
    );

    let mut fx = base();
    valves(&mut fx);
    // V1:2 into the quick-exhaust valve's inlet, its outlet to the cylinder.
    part_of(&mut fx, QE, "V", 1, (QE_DEF, QE_SYM));
    part_of(&mut fx, CYL, "M", 1, (CYL_DEF, CYL_SYM));
    line(&mut fx, 0x4001, at(V1, 1), at(QE, 0));
    line(&mut fx, 0x4002, at(QE, 1), at(CYL, 0));
    // V1's exhausts 3 and 5 teed into one silencer.
    part_of(&mut fx, SIL, "R", 1, (ONE_DEF, ONE_SYM));
    fx.junction(TEE, P1, &[0x4003, 0x4004, 0x4005]);
    line(&mut fx, 0x4003, at(V1, 2), Ep::J(TEE));
    line(&mut fx, 0x4004, at(V1, 4), Ep::J(TEE));
    line(&mut fx, 0x4005, Ep::J(TEE), at(SIL, 0));
    // V2 as an on/off valve feeding port 1 of a manifold: its supply plate taps gallery 1 of a terminal whose valve
    // V3 joins it with its own port 1.
    fx.push(json!({"$k": "plc_config_project", "id": us(CFG), "name": "PLC1", "profile": "siemens-tia-intl@2027.1"}));
    fx.device(VT, "U", Some(1)).with(
        "plc_station",
        json!({"config_project": us(CFG), "role": "valve_terminal"}),
    );
    part_of(&mut fx, PLATE, "U", 2, (PLATE_DEF, PLATE_SYM));
    fx.with_parts(PLATE, &[PLATE_PART]);
    fx.rec_mut(&us(PLATE))["parent"] = json!(us(VT));
    fx.rec_mut(&us(PLATE))["position"] = json!(1);
    part_of(&mut fx, 0x710c, "Q", 3, (VALVE_DEF, VALVE_SYM));
    fx.rec_mut(&us(0x710c))["parent"] = json!(us(VT));
    fx.rec_mut(&us(0x710c))["position"] = json!(2);
    line(&mut fx, 0x4006, at(V2, 1), at(PLATE, 0));
    assert_eq!(messages(&fx, "EW-FL-012"), Vec::<String>::new());
    // Persona round 3: a working port joined to the plate's exhaust tap 5, the valves' exhaust gallery, conflicts.
    line(&mut fx, 0x4007, at(V2, 3), at(PLATE, 2));
    assert_eq!(
        messages(&fx, "EW-FL-012"),
        ["working port -1Q2:4 is joined to -1U2:5 with no valve between them"]
    );
}

/// EW-FL-005 (FR-FLU-019): two media defined in one net report; a medium defined at a regulator's outlet replaces the
/// regulated air there (doc 11 §2.5), so the outlet net carries one medium and nothing reports.
#[test]
fn ew_fl_005_reports_two_media_and_a_defined_outlet_replaces_the_regulated_one() {
    let circuit = || {
        let mut fx = base();
        part_of(&mut fx, SRC, "AS", 1, (ONE_DEF, ONE_SYM));
        part_of(&mut fx, REG, "V", 1, (REG_DEF, REG_SYM));
        fx.with_parts(REG, &[REG_PART]);
        part_of(&mut fx, CYL, "M", 1, (CYL_DEF, CYL_SYM));
        line(&mut fx, 0x4001, at(SRC, 0), at(REG, 0));
        line(&mut fx, 0x4002, at(REG, 1), at(CYL, 0));
        medium_def(&mut fx, 0x5001, at(SRC, 0), AIR);
        fx
    };
    let mut fx = circuit();
    medium_def(&mut fx, 0x5002, at(REG, 0), AIR_4);
    assert_eq!(
        messages(&fx, "EW-FL-005"),
        [
            "the fluid net at -1AS1:1 carries air 4 bar and air 6 bar directly; keep one medium per net",
            "the fluid net at -1M1:A carries air 4 bar → 4 bar and air 6 bar → 4 bar directly; keep one medium per net",
        ]
    );
    let mut fx = circuit();
    medium_def(&mut fx, 0x5002, at(REG, 1), AIR_4);
    assert_eq!(messages(&fx, "EW-FL-005"), Vec::<String>::new());
    let m = fx.build();
    let q = ew_derive::Q::new(&m);
    let net = ew_derive::node_net(&q, ew_derive::EndpointKey::Cp(id(CYL + 0x100), 0)).expect("a net");
    assert_eq!(
        q.get::<ew_derive::MediumZones>(()).effective(net),
        ew_derive::Effective::One(ew_derive::MediumKey::Defined(id(AIR_4)))
    );
}

/// EW-FL-006 (FR-FLU-020): a tube rated below the pressure its net carries, and a tube whose outer diameter is not the
/// port's push-in size, report; the same tube on a port of its size in a 4 bar net reports nothing.
#[test]
fn ew_fl_006_reports_a_tube_rated_too_low_or_of_another_size() {
    let circuit = |medium: u128, push_in: i64| {
        let mut fx = base();
        part_of(&mut fx, SRC, "AS", 1, (ONE_DEF, ONE_SYM));
        part_of(&mut fx, CYL, "M", 1, (CYL_DEF, CYL_SYM));
        fx.part(
            0x654,
            "CYL-1",
            &json!({"sections": {"fluid": {"ports": [{"designation": "A", "push_in_od": push_in}]}}}),
        );
        fx.with_parts(CYL, &[0x654]);
        fx.conn(
            0x4001,
            P1,
            at(SRC, 0),
            at(CYL, 0),
            &json!({"kind": {"fluid": {"tube": {"part": {"copy": us(TUBE_PART)}}}}}),
        );
        medium_def(&mut fx, 0x5001, at(SRC, 0), medium);
        fx
    };
    assert_eq!(
        messages(&circuit(AIR, 8_000_000), "EW-FL-006"),
        [
            "tube -1AS1:1 → -1M1:A is 6 mm outside, but port -1M1:A takes 8 mm",
            "tube -1AS1:1 → -1M1:A is rated 4 bar, below the 6 bar its net carries",
        ]
    );
    assert_eq!(
        messages(&circuit(AIR_4, 6_000_000), "EW-FL-006"),
        Vec::<String>::new()
    );
}

/// EW-FL-009 (FR-FLU-019): a 24 V DC coil fed at 230 V, or at 24 V AC, reports; fed at 24 V DC it does not, nor where
/// the potential names no AC or DC.
#[test]
fn ew_fl_009_reports_a_coil_fed_at_another_voltage_or_current() {
    let circuit = |feed: Value| {
        let mut fx = base();
        fx.device(V1, "Q", Some(1));
        fx.with_parts(V1, &[VALVE_PART]);
        fx.push(json!({"$k": "function", "id": us(V1 + 0x100), "device": us(V1), "def": us(COIL_DEF)}));
        fx.push(json!({
            "$k": "representation", "id": us(V1 + 0x200), "functions": [us(V1 + 0x100)], "page": us(P1),
            "symbol": us(COIL_SYM), "pos": [0, 0],
        }));
        fx.func(0x7401, P1);
        fx.conn(0x4001, P1, Ep::Cp(V1 + 0x100, 0), Ep::Cp(0x7401, 0), &Value::Null);
        fx.def(0x5001, Ep::Cp(0x7401, 0), &feed);
        fx
    };
    assert_eq!(
        messages(&circuit(json!({"voltage": "230"})), "EW-FL-009"),
        ["-1Q1: the coil takes 24 V DC, but -1Q1:A1 (230 V) feed it"]
    );
    let ac = circuit(json!({"voltage": "24", "env": {"ac_dc": "ac"}}));
    assert_eq!(
        messages(&ac, "EW-FL-009"),
        ["-1Q1: the coil takes 24 V DC, but -1Q1:A1 (24 V AC) feed it"]
    );
    assert_eq!(
        dutch(&ac.build(), "EW-FL-009"),
        ["-1Q1: de spoel neemt 24 V DC, maar -1Q1:A1 (24 V AC) voeden haar"]
    );
    for silent in [
        json!({"voltage": "24", "env": {"ac_dc": "dc"}}),
        json!({"voltage": "24"}),
    ] {
        assert_eq!(messages(&circuit(silent), "EW-FL-009"), Vec::<String>::new());
    }
}
