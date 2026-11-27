// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rules of the EW-PLC and EW-BUS packs' 1.0 content: each with a failing and a passing fixture, run
//! in the `full` scheme with the 1.0 content, every message rendered byte for byte from its en-GB pattern and worded
//! by nl-NL; the prevent halves of EW-PLC-007 and EW-PLC-011.

use crate::common;

use common::*;
use ew_checks::{CheckMode, FULL};
use ew_ids::v5;
use ew_schema::{Device, Kind};
use serde_json::{Value, json};

const CFG: u128 = 0x50;
const CARD: u128 = 0x1100;
const CARD_FN: u128 = 0x1101;
const CARD_DEF: u128 = 0x530;
const CARD_SYM: u128 = 0x531;
const CARD_PART: u128 = 0x600;
const FIELD: u128 = 0x1200;
const FIELD_FN: u128 = 0x1201;
const FIELD_DEF: u128 = 0x540;
const FIELD_SYM: u128 = 0x541;
const FIELD_PART: u128 = 0x601;
const OTHER: u128 = 0x1300;
const OTHER_FN: u128 = 0x1301;

/// The findings of `rule` in a run of the `full` scheme with the 1.0 content, their messages checked (P5-2).
fn found_in(m: &ew_model::Model, rule: &str, mode: CheckMode) -> Vec<ew_checks::Finding> {
    let q = ew_derive::Q::new(m);
    let found: Vec<ew_checks::Finding> = ew_checks::run(&q, &FULL, mode)
        .findings
        .into_iter()
        .filter(|f| f.rule == rule)
        .collect();
    assert_worded(&found);
    found
}

fn found(m: &ew_model::Model, rule: &str) -> Vec<ew_checks::Finding> {
    found_in(m, rule, CheckMode::Release)
}

fn forms(f: &[ew_checks::Finding]) -> Vec<String> {
    let mut out: Vec<String> = f.iter().map(|f| f.discriminator.clone()).collect();
    out.sort();
    out
}

/// The record ID text of channel `n` of `module`.
fn ch(module: u128, n: u16) -> String {
    v5::plc_channel_id::<Device, ()>(ew_ids::Id::from_uuid(ew_ids::Uuid::from_u128(module)), n)
        .as_uuid()
        .to_string()
}

/// A card `-A1` of channel kind `kind` (its own station) with channels 0 and 1; channel 0 bound to its pin 0, which a
/// wire joins to the field device `-B1`'s function of contact kind `field`. `plc` members are merged into the card
/// part's `plc` section; `channel` members into channel 0's record, `spare` into channel 1's.
fn card(kind: &str, field: &str, plc: &Value, channel: &Value, spare: &Value) -> Fx {
    let mut fx = Fx::base();
    fx.fndef_with(
        CARD_DEF,
        CARD_SYM,
        &[
            (0, "0", Value::Null),
            (1, "1", Value::Null),
            (2, "TP", Value::Null),
        ],
        &json!([]),
        &json!({"contact_kind": "plc_io"}),
    );
    fx.fndef_with(
        FIELD_DEF,
        FIELD_SYM,
        &[(0, "1", Value::Null), (1, "2", Value::Null)],
        &json!([]),
        &json!({"contact_kind": field}),
    );
    let mut section = json!({
        "role": "module",
        "io": [{"dir": "in", "bits": 8}, {"dir": "out", "bits": 8}],
        "channels": {"count": 2, "kind": kind, "data_type": "Bool"},
    });
    for (k, v) in plc.as_object().expect("plc") {
        section[k] = v.clone();
    }
    fx.part(
        CARD_PART,
        "IO-1",
        &json!({"kind": "device", "sections": {"plc": section}}),
    );
    fx.part(
        FIELD_PART,
        "COIL-1",
        &json!({"kind": "device", "tech_data": [{"key": "i_load", "value": {"value": "1", "unit": "A"}}]}),
    );
    fx.push(json!({"$k": "plc_config_project", "id": us(CFG), "name": "PLC1", "profile": "siemens-tia-intl@2027.1"}));
    fx.device(CARD, "A", Some(1));
    fx.rec_mut(&us(CARD))["plc_station"] = json!({"config_project": us(CFG), "role": "io_device"});
    fx.rec_mut(&us(CARD))["plc_module"] = json!({});
    fx.with_parts(CARD, &[CARD_PART]);
    fx.push(json!({"$k": "function", "id": us(CARD_FN), "device": us(CARD), "def": us(CARD_DEF)}));
    fx.push(json!({
        "$k": "representation", "id": us(CARD_FN + 0x10_000), "functions": [us(CARD_FN)], "page": us(P1),
        "symbol": us(CARD_SYM), "pos": [0, 0],
    }));
    for (dev, f, counter) in [(FIELD, FIELD_FN, 1), (OTHER, OTHER_FN, 2)] {
        fx.device(dev, "B", Some(counter));
        fx.with_parts(dev, &[FIELD_PART]);
        fx.push(json!({"$k": "function", "id": us(f), "device": us(dev), "def": us(FIELD_DEF)}));
        fx.push(json!({
            "$k": "representation", "id": us(f + 0x10_000), "functions": [us(f)], "page": us(P1),
            "symbol": us(FIELD_SYM), "pos": [40_000_000, i64::from(counter) * 20_000_000],
        }));
    }
    fx.conn(0x1400, P1, Ep::Cp(CARD_FN, 0), Ep::Cp(FIELD_FN, 0), &json!({}));
    for (n, extra) in [(0u16, channel), (1, spare)] {
        let mut c =
            json!({"$k": "channel", "id": ch(CARD, n), "module": us(CARD), "number": n, "kind": kind});
        if n == 0 {
            c["binding"] = json!({"cp": [us(CARD_FN), 0]});
        }
        for (k, v) in extra.as_object().expect("channel members") {
            c[k] = v.clone();
        }
        fx.push(c);
    }
    fx
}

fn plain(kind: &str, field: &str) -> ew_model::Model {
    card(kind, field, &json!({}), &json!({}), &json!({})).build()
}

/// EW-PLC-004 (FR-PLC-046): a DO driving a contact and a DI reading a coil; a DI reading a contact is right.
#[test]
fn ew_plc_004_channel_kind_against_the_connected_function() {
    assert_eq!(forms(&found(&plain("do", "no"), "EW-PLC-004")), ["output"]);
    assert_eq!(forms(&found(&plain("di", "coil"), "EW-PLC-004")), ["input"]);
    assert!(found(&plain("di", "no"), "EW-PLC-004").is_empty());
    assert!(
        found_in(&plain("do", "no"), "EW-PLC-004", CheckMode::Online).is_empty(),
        "an offline rule"
    );
}

/// EW-PLC-005 and EW-PLC-007 (FR-PLC-047, FR-PLC-016): "TA05-V-15" is no identifier, and collides with "TA05_V_15"
/// once cleaned; a lenient policy checks neither identifier nor cleaning.
#[test]
fn ew_plc_005_and_007_judge_the_cleaned_symbol() {
    let m = card(
        "di",
        "no",
        &json!({}),
        &json!({"symbol": "TA05-V-15"}),
        &json!({"symbol": "TA05_V_15"}),
    )
    .build();
    let invalid = found(&m, "EW-PLC-005");
    assert_eq!(invalid.len(), 1);
    assert!(
        invalid[0].message().contains("TA05-V-15"),
        "{}",
        invalid[0].message()
    );
    assert_eq!(found(&m, "EW-PLC-007").len(), 1, "the cleaned symbols collide");
    let mut fx = card(
        "di",
        "no",
        &json!({}),
        &json!({"symbol": "TA05-V-15"}),
        &json!({"symbol": "TA05_V_15"}),
    );
    fx.rec_mut(&us(CFG))["symbols"] = json!({"strict_iec": false});
    let m = fx.build();
    assert!(found(&m, "EW-PLC-005").is_empty());
    assert!(found(&m, "EW-PLC-007").is_empty());
}

/// The prevent half of EW-PLC-007 refuses the edit that gives a second channel the same symbol, and not an
/// edit that leaves an existing duplicate alone.
#[test]
fn ew_plc_007_prevents_a_new_duplicate() {
    let before = card("di", "no", &json!({}), &json!({"symbol": "Pump_On"}), &json!({})).build();
    let after = card(
        "di",
        "no",
        &json!({}),
        &json!({"symbol": "Pump_On"}),
        &json!({"symbol": "Pump_On"}),
    )
    .build();
    let keys = vec![(
        Kind::Channel,
        ch(CARD, 1).parse::<ew_schema::AnyId>().expect("an ID"),
    )];
    let prevented = ew_checks::prevent(&ew_derive::Q::new(&before), &ew_derive::Q::new(&after), &keys);
    assert_eq!(
        prevented.iter().map(|f| f.rule).collect::<Vec<_>>(),
        ["EW-PLC-007"]
    );
    let again = ew_checks::prevent(&ew_derive::Q::new(&after), &ew_derive::Q::new(&after), &keys);
    assert!(
        again.is_empty(),
        "a held duplicate is reported, not refused: {again:?}"
    );
}

/// EW-PLC-009 (FR-PLC-046): a symbol on an unconnected channel nobody marked; marked spare it is fine.
#[test]
fn ew_plc_009_channel_without_connection() {
    let m = card("di", "no", &json!({}), &json!({}), &json!({"symbol": "Spare_1"})).build();
    assert_eq!(forms(&found(&m, "EW-PLC-009")), ["symbol"]);
    let m = card(
        "di",
        "no",
        &json!({}),
        &json!({}),
        &json!({"symbol": "Spare_1", "state": "spare"}),
    )
    .build();
    assert!(found(&m, "EW-PLC-009").is_empty());
}

/// EW-PLC-010 (FR-PLC-046): an Int on a DI; a Bool on an AI; the part's Bool on a DI is right.
#[test]
fn ew_plc_010_data_type_width() {
    let m = card("di", "no", &json!({}), &json!({"data_type": "Int"}), &json!({})).build();
    assert_eq!(found(&m, "EW-PLC-010").len(), 1);
    let ai = found(&plain("ai", "no"), "EW-PLC-010");
    assert_eq!(ai.len(), 2, "the part's Bool on both AI channels");
    assert!(!ai[0].unverified_data);
    assert!(found(&plain("di", "no"), "EW-PLC-010").is_empty());
    // On an analogue channel (a word) every other width of IEC 61131-3 is wrong, in any case.
    let word = json!({"data_type": "Int"});
    for (data_type, wrong) in [("Byte", 1), ("dint", 1), ("LReal", 1), ("UINT", 0)] {
        let m = card("ai", "no", &json!({}), &json!({"data_type": data_type}), &word).build();
        assert_eq!(found(&m, "EW-PLC-010").len(), wrong, "{data_type}");
    }
}

/// EW-PLC-011 (FR-PLC-028): a module in a slot its part does not allow, and a station beyond its module count; its
/// prevent half refuses the edit that adds the module.
#[test]
fn ew_plc_011_slot_rules() {
    const M2: u128 = 0x1500;
    let station_limits = json!({"limits": {"max_modules": 1}, "allowed_slots": [1]});
    let with_second = |second: bool| {
        let mut fx = card("di", "no", &station_limits, &json!({}), &json!({}));
        if second {
            fx.device(M2, "A", Some(2));
            let r = fx.rec_mut(&us(M2));
            r["parent"] = json!(us(CARD));
            r["position"] = json!(1);
            r["plc_module"] = json!({});
            fx.with_parts(M2, &[CARD_PART]);
        }
        fx.build()
    };
    assert!(found(&with_second(false), "EW-PLC-011").is_empty());
    let after = with_second(true);
    assert_eq!(forms(&found(&after, "EW-PLC-011")), ["max_modules", "slot"]);
    let keys = vec![(Kind::Device, uid(M2))];
    let prevented = ew_checks::prevent(
        &ew_derive::Q::new(&with_second(false)),
        &ew_derive::Q::new(&after),
        &keys,
    );
    assert_eq!(prevented.len(), 2, "{prevented:#?}");
}

/// EW-PLC-013 (FR-PLC-048): the L+ pin of a supply group that no wire reaches.
#[test]
fn ew_plc_013_unconnected_supply_pin() {
    let groups = json!({"supply_groups": [{"kind": "electronics", "cps": {"l_plus": "TP", "m": "1"}}]});
    let m = card("di", "no", &groups, &json!({}), &json!({})).build();
    let f = found(&m, "EW-PLC-013");
    assert_eq!(f.len(), 2, "TP and 1 are not connected: {f:#?}");
    assert!(f.iter().all(|f| f.discriminator.starts_with("unconnected")));
    assert!(found(&plain("di", "no"), "EW-PLC-013").is_empty());
}

/// EW-PLC-013: a bus port's pin "A" is no supply pin though a supply group names "A"; an M pin fed from L− is right,
/// one fed from L+ is wrong.
#[test]
fn ew_plc_013_skips_bus_port_pins_and_accepts_l_minus_for_m() {
    const SRC_DEF: u128 = 0x560;
    const SRC_SYM: u128 = 0x561;
    const SRC: u128 = 0x1900;
    let groups = json!({"supply_groups": [{"kind": "load", "cps": {"l_plus": "TP", "m": "A"}}]});
    let mut fx = card("di", "no", &groups, &json!({}), &json!({}));
    fx.fndef_with(
        PORT_DEF,
        PORT_SYM,
        &[(0, "A", Value::Null), (1, "B", Value::Null)],
        &json!([]),
        &json!({"contact_kind": "bus_port"}),
    );
    fx.push(json!({
        "$k": "function", "id": us(CARD_FN + 2), "device": us(CARD), "def": us(PORT_DEF), "role": "aux",
        "bus_port": {"interface": "X1", "bus": "profibus_dp", "role": "slave", "address": "tbd"},
    }));
    let f = found(&fx.build(), "EW-PLC-013");
    assert_eq!(f.len(), 1, "TP only: {f:#?}");
    assert_eq!(f[0].targets[0].id, uid(CARD_FN));

    let fed = |role: &str| {
        let groups = json!({"supply_groups": [{"kind": "load", "cps": {"l_plus": "TP", "m": "1"}}]});
        let mut fx = card("di", "no", &groups, &json!({}), &json!({}));
        fx.fndef_with(
            SRC_DEF,
            SRC_SYM,
            &[(0, "1", json!({"potential_role": role}))],
            &json!([]),
            &json!({}),
        );
        fx.device(SRC, "G", Some(1));
        fx.push(json!({"$k": "function", "id": us(SRC + 1), "device": us(SRC), "def": us(SRC_DEF)}));
        fx.push(json!({
            "$k": "representation", "id": us(SRC + 0x10_001), "functions": [us(SRC + 1)], "page": us(P1),
            "symbol": us(SRC_SYM), "pos": [80_000_000, 0],
        }));
        fx.conn(0x1403, P1, Ep::Cp(CARD_FN, 1), Ep::Cp(SRC + 1, 0), &json!({}));
        forms(&found(&fx.build(), "EW-PLC-013"))
    };
    assert!(
        !fed("l_minus").iter().any(|f| f.starts_with("wrong")),
        "{:?}",
        fed("l_minus")
    );
    assert!(
        fed("l_plus").iter().any(|f| f.starts_with("wrong")),
        "{:?}",
        fed("l_plus")
    );
}

/// EW-PLC-013's FE form (D-16, EF-22): the card's FE pin (a function of the card whose CP has the potential role
/// `fe`) wired to a point whose potential is defined as PE gives the finding with form `fe` at that function; an FE-PE
/// bond at the pin clears it.
#[test]
fn ew_plc_013_reports_an_fe_pin_fed_from_pe_without_a_bond() {
    const FE_DEF: u128 = 0x570;
    const FE_SYM: u128 = 0x571;
    const FE_FN: u128 = CARD_FN + 3;
    const PE_FN: u128 = 0x1950;
    let fe_pin = |bonded: bool| {
        let mut fx = card("di", "no", &json!({}), &json!({}), &json!({}));
        fx.fndef_with(
            FE_DEF,
            FE_SYM,
            &[(0, "FE", json!({"potential_role": "fe"}))],
            &json!([]),
            &json!({}),
        );
        fx.push(
            json!({"$k": "function", "id": us(FE_FN), "device": us(CARD), "def": us(FE_DEF), "role": "aux"}),
        );
        fx.push(json!({
            "$k": "representation", "id": us(FE_FN + 0x10_000), "functions": [us(FE_FN)], "page": us(P1),
            "symbol": us(FE_SYM), "pos": [0, 60_000_000],
        }));
        fx.func(PE_FN, P1);
        fx.conn(0x1405, P1, Ep::Cp(FE_FN, 0), cp(PE_FN), &json!({}));
        fx.def(0x5001, cp(PE_FN), &json!({"ptype": "pe"}));
        if bonded {
            fx.func(PE_FN + 1, P1);
            fx.push(json!({
                "$k": "bond", "id": us(0x4001), "a": {"cp": [us(FE_FN), 0]}, "b": {"cp": [us(PE_FN + 1), 0]},
                "kind": "fe_pe",
            }));
        }
        found(&fx.build(), "EW-PLC-013")
    };
    let open = fe_pin(false);
    assert_eq!(forms(&open), ["fe:0"], "{open:#?}");
    assert_eq!(open[0].targets[0].id, uid(FE_FN));
    assert!(
        open[0]
            .message()
            .contains("is the FE pin of a PLC card and is fed from the PE potential"),
        "{}",
        open[0].message()
    );
    assert!(fe_pin(true).is_empty(), "{:#?}", fe_pin(true));
}

/// EW-PLC-014 (FR-PLC-048): a 1 A coil on an output rated 500 mA, and the module rated 800 mA.
#[test]
fn ew_plc_014_output_load() {
    let ratings = json!({"ratings": {"channel_ma": 500, "module_ma": 800}});
    let m = card("do", "coil", &ratings, &json!({}), &json!({})).build();
    assert_eq!(forms(&found(&m, "EW-PLC-014")), ["channel", "module"]);
    let ratings = json!({"ratings": {"channel_ma": 2000}});
    assert!(
        found(
            &card("do", "coil", &ratings, &json!({}), &json!({})).build(),
            "EW-PLC-014"
        )
        .is_empty()
    );
}

/// EW-SAF-001 (FR-PLC-042): two F-modules with F-destination 100, one outside its range; a missing address.
#[test]
fn ew_saf_001_f_destination() {
    const M2: u128 = 0x1500;
    let build = |first: Value, second: Value| {
        let mut fx = card("di", "no", &json!({}), &json!({}), &json!({}));
        fx.rec_mut(&us(CARD))["plc_module"] = json!({"safety": first});
        fx.device(M2, "A", Some(2));
        let r = fx.rec_mut(&us(M2));
        r["parent"] = json!(us(CARD));
        r["position"] = json!(1);
        r["plc_module"] = json!({"safety": second});
        fx.build()
    };
    let m = build(
        json!({"f_dest": 100}),
        json!({"f_dest": 100, "range": {"min": 1, "max": 99}}),
    );
    assert_eq!(forms(&found(&m, "EW-SAF-001")), ["duplicate:100", "range"]);
    let m = build(json!({}), json!({"f_dest": 7}));
    assert_eq!(forms(&found(&m, "EW-SAF-001")), ["missing"]);
    let m = build(json!({"f_dest": 6}), json!({"f_dest": 7}));
    assert!(found(&m, "EW-SAF-001").is_empty());
}

/// EW-SAF-002: a 1oo2 channel named as its own pair has no partner.
#[test]
fn ew_saf_002_a_channel_paired_with_itself_is_incomplete() {
    let m = card(
        "di",
        "no",
        &json!({}),
        &json!({"safety": {"evaluation": "1oo2", "pair": ch(CARD, 0)}}),
        &json!({}),
    )
    .build();
    assert_eq!(forms(&found(&m, "EW-SAF-002")), ["incomplete"]);
    // A partner that does not name it back (it names itself), or that is no 1oo2 channel, is none either.
    for spare in [
        json!({"safety": {"evaluation": "1oo2", "pair": ch(CARD, 1)}}),
        json!({"safety": {"evaluation": "1oo1", "pair": ch(CARD, 0)}}),
    ] {
        let m = card(
            "di",
            "no",
            &json!({}),
            &json!({"safety": {"evaluation": "1oo2", "pair": ch(CARD, 1)}}),
            &spare,
        )
        .build();
        assert!(
            forms(&found(&m, "EW-SAF-002")).contains(&"incomplete".to_owned()),
            "{spare}"
        );
    }
}

/// EW-SAF-002 and EW-SAF-003 (FR-PLC-043): a 1oo2 channel without its partner; a pair reaching two sensors; a sensor
/// not fed from the test-pulse pin.
#[test]
fn ew_saf_002_and_003_pair_and_test_pulse() {
    let lone = card(
        "di",
        "no",
        &json!({}),
        &json!({"safety": {"evaluation": "1oo2"}}),
        &json!({}),
    )
    .build();
    assert_eq!(forms(&found(&lone, "EW-SAF-002")), ["incomplete"]);
    let pair = |second_wired_to: u128| {
        let mut fx = card(
            "di",
            "no",
            &json!({}),
            &json!({"safety": {"evaluation": "1oo2", "pair": ch(CARD, 1)}}),
            &json!({"binding": {"cp": [us(CARD_FN), 1]}, "safety": {"evaluation": "1oo2", "pair": ch(CARD, 0)}}),
        );
        fx.conn(
            0x1401,
            P1,
            Ep::Cp(CARD_FN, 1),
            Ep::Cp(second_wired_to, 1),
            &json!({}),
        );
        fx.build()
    };
    assert_eq!(forms(&found(&pair(OTHER_FN), "EW-SAF-002")), ["devices"]);
    assert!(found(&pair(FIELD_FN), "EW-SAF-002").is_empty());

    let pulse = json!({"safety": {"evaluation": "1oo1", "test_pulse": {"cp": [us(CARD_FN), 2]}}});
    let m = card("di", "no", &json!({}), &pulse, &json!({})).build();
    assert_eq!(found(&m, "EW-SAF-003").len(), 1);
    let mut fx = card("di", "no", &json!({}), &pulse, &json!({}));
    fx.conn(0x1402, P1, Ep::Cp(CARD_FN, 2), Ep::Cp(FIELD_FN, 1), &json!({}));
    assert!(found(&fx.build(), "EW-SAF-003").is_empty());
}

/// EW-PRC-001 (FR-PLC-049): one process tag on two devices, and a tag outside the project's scheme.
#[test]
fn ew_prc_001_process_tags() {
    let mut fx = card("di", "no", &json!({}), &json!({}), &json!({}));
    fx.rec_mut(&us(FIELD))["process_tag"] = json!("TA05-V-15");
    fx.rec_mut(&us(OTHER))["process_tag"] = json!("TA05-V-15");
    assert_eq!(forms(&found(&fx.build(), "EW-PRC-001")), ["duplicate:TA05-V-15"]);
    fx.rec_mut(&us(OTHER))["process_tag"] = json!("pump 7");
    fx.settings = json!({"process_tag_scheme": r"(?P<area>TA\d\d)-(?P<kind>[A-Z]+)-(?P<number>\d+)"});
    assert_eq!(forms(&found(&fx.build(), "EW-PRC-001")), ["scheme"]);
    fx.rec_mut(&us(OTHER))["process_tag"] = json!("TA05-V-16");
    assert!(found(&fx.build(), "EW-PRC-001").is_empty());
}

const LNET: u128 = 0x60;
const LNET2: u128 = 0x61;
const PNET: u128 = 0x62;
const PORT_DEF: u128 = 0x550;
const PORT_SYM: u128 = 0x551;
const P_A: u128 = 0x1600;
const P_B: u128 = 0x1610;
const BUS_PART: u128 = 0x602;
const CABLE: u128 = 0x1700;
const CABLE_PART: u128 = 0x603;

/// Two bus ports `-A1:X1` and `-A2:X1` of logical net `LNET` and physical net `PNET`, joined by a `Bus` connection
/// with the cable `-W1` (300 m); `a` and `b` are merged into the ports, `net` into the logical net, `part` into the
/// devices' part port, `cable` into the cable part's bus section.
fn bus(net: &Value, a: &Value, b: &Value, part: &Value, cable: Option<&Value>) -> ew_model::Model {
    bus_with_meta(net, a, b, part, cable, &json!({}))
}

/// [`bus`] with the ports' part's `meta` (its confidence marks).
fn bus_with_meta(
    net: &Value,
    a: &Value,
    b: &Value,
    part: &Value,
    cable: Option<&Value>,
    meta: &Value,
) -> ew_model::Model {
    bus_fx(net, a, b, part, cable, meta).build()
}

/// [`bus_with_meta`]'s fixture.
fn bus_fx(net: &Value, a: &Value, b: &Value, part: &Value, cable: Option<&Value>, meta: &Value) -> Fx {
    let mut fx = Fx::base();
    fx.fndef_with(
        PORT_DEF,
        PORT_SYM,
        &[(0, "A", Value::Null), (1, "B", Value::Null)],
        &json!([]),
        &json!({"contact_kind": "bus_port"}),
    );
    let mut port = json!({"interface": "X1", "bus": "profibus_dp", "role": "slave"});
    for (k, v) in part.as_object().expect("part") {
        port[k] = v.clone();
    }
    fx.part(
        BUS_PART,
        "NODE-1",
        &json!({"kind": "device", "meta": meta, "sections": {"bus": {"ports": [port]}}}),
    );
    let mut lnet = json!({"$k": "logical_net", "id": us(LNET), "bus": "profibus_dp", "name": "DP"});
    for (k, v) in net.as_object().expect("net") {
        lnet[k] = v.clone();
    }
    fx.push(lnet);
    fx.push(json!({"$k": "logical_net", "id": us(LNET2), "bus": "profibus_dp", "name": "DP2"}));
    fx.push(json!({"$k": "physical_net", "id": us(PNET), "name": "DP", "medium": "copper"}));
    for (n, counter, extra) in [(P_A, 1, a), (P_B, 2, b)] {
        fx.device(n, "A", Some(counter));
        fx.with_parts(n, &[BUS_PART]);
        let mut p = json!({
            "interface": "X1", "bus": "profibus_dp", "role": "slave", "address": "tbd",
            "logical_net": us(LNET), "physical_net": us(PNET),
        });
        for (k, v) in extra.as_object().expect("port") {
            p[k] = v.clone();
        }
        fx.push(
            json!({"$k": "function", "id": us(n + 1), "device": us(n), "def": us(PORT_DEF), "bus_port": p}),
        );
        fx.push(json!({
            "$k": "representation", "id": us(n + 0x10_001), "functions": [us(n + 1)], "page": us(P1),
            "symbol": us(PORT_SYM), "pos": [i64::from(counter) * 40_000_000, 0],
        }));
    }
    let mut kind = json!({"bus": {}});
    if let Some(section) = cable {
        fx.part(
            CABLE_PART,
            "CABLE-1",
            &json!({"kind": "device", "sections": {"bus": section}}),
        );
        fx.device(CABLE, "W", Some(1));
        fx.rec_mut(&us(CABLE))["cable"] = json!({"length": {"manual": 300_000_000_000_i64}});
        fx.with_parts(CABLE, &[CABLE_PART]);
        kind = json!({"bus": {"cable": us(CABLE)}});
    }
    fx.conn(
        0x1800,
        P1,
        Ep::Cp(P_A + 1, 0),
        Ep::Cp(P_B + 1, 0),
        &json!({"kind": kind}),
    );
    fx
}

/// EW-BUS-006: a segment compares its length to the nanometre (100.4 m is longer than the 100 m of 12 Mbit/s), and a
/// segment of PROFINET ports that name no logical net is not held to PROFIBUS's 32 stations.
#[test]
fn ew_bus_006_judges_the_exact_length_and_profibus_segments_only() {
    let mut fx = bus_fx(
        &json!({"rate": 12_000_000}),
        &json!({}),
        &json!({}),
        &json!({}),
        Some(&json!({"cable_for": ["profibus_dp"]})),
        &json!({}),
    );
    fx.rec_mut(&us(CABLE))["cable"] = json!({"length": {"manual": 100_400_000_000_i64}});
    assert_eq!(forms(&found(&fx.build(), "EW-BUS-006")), ["length"]);

    let mut fx = Fx::base();
    fx.fndef_with(
        PORT_DEF,
        PORT_SYM,
        &[(0, "A", Value::Null), (1, "B", Value::Null)],
        &json!([]),
        &json!({"contact_kind": "bus_port"}),
    );
    fx.push(json!({"$k": "physical_net", "id": us(PNET), "name": "PN", "medium": "copper"}));
    for i in 0..33u32 {
        let n = 0x3000 + u128::from(i) * 0x10;
        fx.device(n, "A", Some(i + 1));
        fx.push(json!({
            "$k": "function", "id": us(n + 1), "device": us(n), "def": us(PORT_DEF),
            "bus_port": {"interface": "X1", "bus": "profinet", "role": "device", "address": "tbd",
                         "physical_net": us(PNET)},
        }));
        fx.push(json!({
            "$k": "representation", "id": us(n + 0x10_001), "functions": [us(n + 1)], "page": us(P1),
            "symbol": us(PORT_SYM), "pos": [i64::from(i) * 8_000_000, 0],
        }));
        if i > 0 {
            fx.conn(
                n + 2,
                P1,
                Ep::Cp(n - 0x10 + 1, 1),
                Ep::Cp(n + 1, 0),
                &json!({"kind": {"bus": {}}}),
            );
        }
    }
    assert!(found(&fx.build(), "EW-BUS-006").is_empty());
}

/// EW-BUS-002, -005, -007 (FR-BUS-018): an address beyond the part's range, a rate the part does not support, a net
/// without a master; each silent once corrected.
#[test]
fn ew_bus_002_005_007_range_rate_master() {
    let addr = json!({"address": {"value": {"profibus": 126}}});
    let m = bus(
        &json!({"rate": 12_000_000}),
        &addr,
        &json!({}),
        &json!({"rates": [1_500_000]}),
        None,
    );
    assert_eq!(found(&m, "EW-BUS-002").len(), 1, "126 is beyond 0..125");
    assert_eq!(found(&m, "EW-BUS-005").len(), 2, "both slaves lack 12 Mbit/s");
    let masterless = |m: &ew_model::Model| -> Vec<String> {
        found(m, "EW-BUS-007")
            .into_iter()
            .map(|f| f.subject_english())
            .collect()
    };
    assert_eq!(masterless(&m), ["logical net DP", "logical net DP2"]);
    let m = bus(
        &json!({"rate": 1_500_000}),
        &json!({"address": {"value": {"profibus": 3}}, "role": "master"}),
        &json!({}),
        &json!({"rates": [1_500_000], "address_range": {"min": 1, "max": 125}}),
        None,
    );
    for rule in ["EW-BUS-002", "EW-BUS-005"] {
        assert!(found(&m, rule).is_empty(), "{rule}");
    }
    assert_eq!(
        masterless(&m),
        ["logical net DP2"],
        "a port with role master counts"
    );
}

/// EW-BUS-001's next free address on a part whose address range holds no PROFIBUS station address (its minimum above
/// its maximum, or above 255): each open port keeps its finding, without the quick fix, and the rule does not fail.
#[test]
fn ew_bus_001_offers_no_address_from_a_range_that_holds_none() {
    for range in [json!({"min": 10, "max": 5}), json!({"min": 300, "max": 400})] {
        let m = bus(
            &json!({}),
            &json!({}),
            &json!({}),
            &json!({"address_range": range}),
            None,
        );
        let f = found(&m, "EW-BUS-001");
        assert_eq!(f.len(), 2, "{range}: both open ports, {f:#?}");
        assert!(f.iter().all(|f| f.fix.is_none()), "{range}: no address to give");
        assert!(found(&m, "EW-CORE-083").is_empty(), "{range}: the rule ran");
    }
}

/// Next free address of an Address row, by the rule of EW-BUS-001's quick fix, on a bus device made here whose part
/// gives the range 20–30: the open port takes the address after the highest its net holds, a port that holds an
/// address one free from the others (its own does not count), and a port of a bus without station numbers none.
#[test]
fn the_next_free_address_of_a_port_follows_its_parts_range() {
    use ew_ids::Id;
    let next = |m: &ew_model::Model, port: u128| {
        ew_checks::rules::bus::next_free_address(
            &ew_derive::Q::new(m),
            Id::from_uuid(ew_ids::Uuid::from_u128(port + 1)),
        )
    };
    let m = bus(
        &json!({}),
        &json!({"address": {"value": {"profibus": 25}}}),
        &json!({}),
        &json!({"address_range": {"min": 20, "max": 30}}),
        None,
    );
    assert_eq!(next(&m, P_B), Some(26), "the open port");
    assert_eq!(
        next(&m, P_A),
        Some(20),
        "the port that holds 25: none other is taken"
    );
    let m = bus(
        &json!({"bus": "profinet"}),
        &json!({"bus": "profinet"}),
        &json!({"bus": "profinet"}),
        &json!({}),
        None,
    );
    assert_eq!(
        next(&m, P_A),
        None,
        "a PROFINET port has a name, no station number"
    );
}

/// EW-BUS-003 (FR-BUS-017): two stations at 192.168.0.10, a device name with an underscore, an address outside the
/// master's subnet.
#[test]
fn ew_bus_003_profinet_names_and_addresses() {
    let pn = |name: &str, ip: &str| json!({"bus": "profinet", "address": {"value": {"profinet": {"name": name, "ip": ip, "mask": "255.255.255.0"}}}});
    let m = bus(
        &json!({"bus": "profinet"}),
        &pn("cpx_1", "192.168.0.10"),
        &pn("cpx-2", "192.168.0.10"),
        &json!({}),
        None,
    );
    assert_eq!(
        forms(&found(&m, "EW-BUS-003")),
        ["duplicate_ip:192.168.0.10", "name"]
    );
    let mut master = pn("plc-1", "192.168.0.1");
    master["role"] = json!("controller");
    let m = bus(
        &json!({"bus": "profinet", "master": {"cp": [us(P_A + 1), 0]}}),
        &master,
        &pn("cpx-2", "192.168.1.10"),
        &json!({}),
        None,
    );
    let f = found(&m, "EW-BUS-003");
    assert_eq!(forms(&f), ["subnet"]);
    assert_eq!(
        f[0].subject_english(),
        "-1A2:X1",
        "the port outside, not the master"
    );
    // In the master's subnet it is silent.
    let m = bus(
        &json!({"bus": "profinet", "master": {"cp": [us(P_A + 1), 0]}}),
        &master,
        &pn("cpx-2", "192.168.0.10"),
        &json!({}),
        None,
    );
    assert!(found(&m, "EW-BUS-003").is_empty());
}

/// EW-BUS-006: the longest PROFIBUS copper segment at each rate (1200 m up to 93.75 kbit/s, 1000 m at 187.5 kbit/s,
/// 400 m at 500 kbit/s, 200 m at 1.5 Mbit/s, 100 m above) is allowed; a millimetre more is not.
#[test]
fn ew_bus_006_allows_each_rate_s_longest_segment_and_no_more() {
    for (rate, metres) in [
        (93_750, 1200),
        (187_500, 1000),
        (500_000, 400),
        (1_500_000, 200),
        (12_000_000, 100),
    ] {
        for (extra_mm, want) in [(0_i64, &[][..]), (1, &["length"][..])] {
            let mut fx = bus_fx(
                &json!({"rate": rate}),
                &json!({}),
                &json!({}),
                &json!({}),
                Some(&json!({"cable_for": ["profibus_dp"]})),
                &json!({}),
            );
            let nm = metres * 1_000_000_000_i64 + extra_mm * 1_000_000;
            fx.rec_mut(&us(CABLE))["cable"] = json!({"length": {"manual": nm}});
            assert_eq!(
                forms(&found(&fx.build(), "EW-BUS-006")),
                want,
                "{rate} bit/s, {metres} m and {extra_mm} mm"
            );
        }
    }
}

/// A bus port `-A<counter>:X1` (device `n`, function `n + 1`) on [`PNET`] and `lnet`, drawn on page 1.
fn port_device(fx: &mut Fx, n: u128, counter: u32, role: &str, lnet: Option<u128>) {
    fx.device(n, "A", Some(counter));
    let mut p = json!({
        "interface": "X1", "bus": "profibus_dp", "role": role, "address": "tbd", "physical_net": us(PNET),
    });
    if let Some(l) = lnet {
        p["logical_net"] = json!(us(l));
    }
    fx.push(json!({"$k": "function", "id": us(n + 1), "device": us(n), "def": us(PORT_DEF), "bus_port": p}));
    fx.push(json!({
        "$k": "representation", "id": us(n + 0x10_001), "functions": [us(n + 1)], "page": us(P1),
        "symbol": us(PORT_SYM), "pos": [i64::from(counter) * 8_000_000, 0],
    }));
}

/// EW-BUS-006: a PROFIBUS segment takes 32 stations; a connector on the line is none.
#[test]
fn ew_bus_006_counts_the_stations_of_a_profibus_segment() {
    for (stations, want) in [(32_u32, &[][..]), (33, &["stations"][..])] {
        let mut fx = Fx::base();
        fx.fndef_with(
            PORT_DEF,
            PORT_SYM,
            &[(0, "A", Value::Null), (1, "B", Value::Null)],
            &json!([]),
            &json!({"contact_kind": "bus_port"}),
        );
        fx.push(json!({"$k": "physical_net", "id": us(PNET), "name": "DP", "medium": "copper"}));
        // The stations, and a connector at the end of the line.
        for i in 0..=stations {
            let n = 0x3000 + u128::from(i) * 0x10;
            let role = if i == stations { "passive" } else { "slave" };
            port_device(&mut fx, n, i + 1, role, None);
            if i > 0 {
                fx.conn(
                    n + 2,
                    P1,
                    Ep::Cp(n - 0x10 + 1, 1),
                    Ep::Cp(n + 1, 0),
                    &json!({"kind": {"bus": {}}}),
                );
            }
        }
        assert_eq!(forms(&found(&fx.build(), "EW-BUS-006")), want, "{stations}");
    }
}

/// EW-BUS-006: a PROFIBUS segment that branches is a fault above 1.5 Mbit/s only.
#[test]
fn ew_bus_006_reports_a_branch_above_1_5_mbit_s() {
    for (rate, want) in [(12_000_000, &["stub"][..]), (1_500_000, &[][..])] {
        let mut fx = bus_fx(
            &json!({"rate": rate}),
            &json!({}),
            &json!({}),
            &json!({}),
            None,
            &json!({}),
        );
        // Two more ports on the first: it has three neighbours.
        for (n, counter, cp) in [(0x1620, 3, 1), (0x1630, 4, 0)] {
            port_device(&mut fx, n, counter, "slave", Some(LNET));
            fx.conn(
                n + 2,
                P1,
                Ep::Cp(P_A + 1, cp),
                Ep::Cp(n + 1, 0),
                &json!({"kind": {"bus": {}}}),
            );
        }
        assert_eq!(forms(&found(&fx.build(), "EW-BUS-006")), want, "{rate}");
    }
}

/// EW-BUS-009: two ports mated without a cable have one connector type.
#[test]
fn ew_bus_009_reports_ports_mated_with_different_connectors() {
    let part = json!({"connector": "sub-d-9"});
    let same = bus(&json!({}), &json!({}), &json!({}), &part, None);
    assert!(found(&same, "EW-BUS-009").is_empty());
    let mut fx = bus_fx(&json!({}), &json!({}), &json!({}), &part, None, &json!({}));
    fx.part(
        0x606,
        "NODE-2",
        &json!({"kind": "device", "sections": {"bus": {"ports": [
            {"interface": "X1", "bus": "profibus_dp", "role": "slave", "connector": "m12-b"},
        ]}}}),
    );
    fx.with_parts(P_B, &[0x606]);
    let f = found(&fx.build(), "EW-BUS-009");
    assert_eq!(forms(&f), ["connector"]);
    assert_eq!(f[0].subject_english(), "-1A1:X1");
}

/// EW-BUS-006, -009, -010 (FR-BUS-018, FR-BUS-024): a 300 m segment at 1.5 Mbit/s, a cable made for PROFINET, a port
/// of another logical net on the same line.
#[test]
fn ew_bus_006_009_010_segment_cable_nets() {
    let m = bus(
        &json!({"rate": 1_500_000}),
        &json!({}),
        &json!({"logical_net": us(LNET2)}),
        &json!({}),
        Some(&json!({"cable_for": ["profinet"]})),
    );
    assert_eq!(forms(&found(&m, "EW-BUS-006")), ["length"]);
    assert_eq!(forms(&found(&m, "EW-BUS-009")), ["cable", "cable"]);
    assert_eq!(found(&m, "EW-BUS-010").len(), 1);
    let m = bus(
        &json!({"rate": 93_750}),
        &json!({}),
        &json!({}),
        &json!({}),
        Some(&json!({"cable_for": ["profibus_dp"]})),
    );
    for rule in ["EW-BUS-006", "EW-BUS-009", "EW-BUS-010"] {
        assert!(found(&m, rule).is_empty(), "{rule}");
    }
}

const MASTER_PART: u128 = 0x604;
const DEV_PART: u128 = 0x605;

/// An IO-Link master `-A1` (8-byte class-A ports) whose port 0 is wired to the device `-B1` (88 input bits, class
/// `class`) in mode `mode`.
fn iolink(class: &str, mode: &str) -> ew_model::Model {
    iolink_fx(class, mode, &json!({})).build()
}

/// [`iolink`]'s fixture; `plc` members are merged into the master part's `plc` section.
fn iolink_fx(class: &str, mode: &str, plc: &Value) -> Fx {
    let mut fx = card("multi_function", "black_box", &json!({}), &json!({}), &json!({}));
    let mut section = json!({
        "role": "module",
        "channels": {"count": 2, "kind": "multi_function"},
        "iolink": {"input_bytes": 8, "output_bytes": 8, "class": "a", "max_cable_m": 20},
    });
    for (k, v) in plc.as_object().expect("plc") {
        section[k] = v.clone();
    }
    fx.part(
        MASTER_PART,
        "IOL-8",
        &json!({"kind": "device", "sections": {"plc": section}}),
    );
    fx.with_parts(CARD, &[MASTER_PART]);
    fx.part(
        DEV_PART,
        "BNI-88",
        &json!({"kind": "device", "sections": {"bus": {"ports": [{
            "interface": "IOL", "bus": "io_link", "role": "device",
            "iolink": {"class": class, "input_bits": 88, "data_type": "BNI_IOL_88"},
        }]}}}),
    );
    fx.with_parts(FIELD, &[DEV_PART]);
    fx.rec_mut(&ch(CARD, 0))["mode"] = json!(mode);
    fx
}

/// Doc 10 §3.8: an IO-Link port's window is as wide as its process data. Port 0's 11 input bytes (88 bits) from IB0
/// overlap a manual IB3 on port 1 (EW-PLC-001); the master's 19 input bytes exceed a station limit of 10 (EW-PLC-011);
/// under the tag-based Rockwell profile the used port prints by its channel instead of having no address
/// (EW-PLC-008).
#[test]
fn an_iolink_window_counts_its_whole_width() {
    let mut fx = iolink_fx("a", "io_link", &json!({"limits": {"max_input_bytes": 10}}));
    fx.rec_mut(&ch(CARD, 1))["address"] = json!({"manual": {"area": "i", "size": "byte", "byte": 3}});
    let m = fx.build();
    assert_eq!(forms(&found(&m, "EW-PLC-001")), ["overlap"]);
    assert_eq!(forms(&found(&m, "EW-PLC-011")), ["input"]);
    let mut fx = iolink_fx("a", "io_link", &json!({}));
    fx.rec_mut(&us(CFG))["profile"] = json!("rockwell-logix@2027.1");
    let m = fx.build();
    assert!(found(&m, "EW-PLC-008").is_empty());
    let q = ew_derive::Q::new(&m);
    let port = q
        .get::<ew_derive::ChannelAddresses>(ew_ids::Id::from_uuid(ew_ids::Uuid::from_u128(CARD)))
        .first()
        .map(|e| e.address.clone());
    assert!(
        matches!(&port, Some(ew_derive::ChannelAddr::At { text, .. }) if text.ends_with(":I.Ch0Data")),
        "{port:?}"
    );
}

/// EW-IOL-001 and EW-IOL-002 (FR-BUS-019): a class-B device on a class-A port, an IO-Link device on a DI port, and
/// 88 bits of process data in an 8-byte window.
#[test]
fn ew_iol_001_and_002_class_mode_window() {
    let m = iolink("b", "io_link");
    assert_eq!(forms(&found(&m, "EW-IOL-001")), ["class"]);
    assert_eq!(forms(&found(&m, "EW-IOL-002")), ["window:input"]);
    // 64 bits fill the 8-byte window exactly.
    let mut fx = iolink_fx("a", "io_link", &json!({}));
    fx.part(
        0x606,
        "BNI-64",
        &json!({"kind": "device", "sections": {"bus": {"ports": [{
            "interface": "IOL", "bus": "io_link", "role": "device",
            "iolink": {"class": "a", "input_bits": 64, "data_type": "BNI_IOL_64"},
        }]}}}),
    );
    fx.with_parts(FIELD, &[0x606]);
    assert!(found(&fx.build(), "EW-IOL-002").is_empty());
    let m = iolink("a", "di");
    assert_eq!(forms(&found(&m, "EW-IOL-001")), ["mode"]);
    assert!(
        found(&m, "EW-IOL-002").is_empty(),
        "a DI port has no IO-Link window"
    );
}

/// A bus finding that rests on the port part's `bus` section is marked "based on non-verified part data" when
/// that section's confidence is not `verified`, and not otherwise.
#[test]
fn a_bus_finding_on_unverified_part_data_is_marked() {
    let rate = |meta: Value| {
        let m = bus_with_meta(
            &json!({"rate": 12_000_000}),
            &json!({}),
            &json!({}),
            &json!({"rates": [1_500_000]}),
            None,
            &meta,
        );
        found(&m, "EW-BUS-005")
            .iter()
            .map(|f| f.unverified_data)
            .collect::<Vec<_>>()
    };
    assert_eq!(rate(json!({"confidence": {"bus": "test"}})), [true, true]);
    assert_eq!(rate(json!({})), [false, false]);
}

/// EW-PLC-018: a drawn module whose part maps its channels to pins, with none of its channels bound, is reported with
/// the fix that binds them; one channel bound (the other unbound on purpose) is fine.
#[test]
fn ew_plc_018_reports_a_drawn_module_with_no_channel_bound() {
    let pinned = json!({"pin_of": "{n}"});
    let bound = card("di", "contact", &pinned, &json!({}), &json!({})).build();
    assert!(found_in(&bound, "EW-PLC-018", CheckMode::Online).is_empty());
    let unbound = card("di", "contact", &pinned, &json!({"binding": null}), &json!({})).build();
    let f = found_in(&unbound, "EW-PLC-018", CheckMode::Online);
    assert_eq!(forms(&f), ["module"]);
    assert_eq!(
        f[0].fix.as_ref().map(|x| x.command.as_str()),
        Some("plc.bind_pins")
    );
}

/// EW-PLC-016: a configuration project whose exchanged objects differ from the model since its last exchange is one
/// info; a project never exchanged has none.
#[test]
fn ew_plc_016_reports_drift_since_the_last_exchange() {
    let mut fx = card("di", "contact", &json!({}), &json!({}), &json!({}));
    assert!(found_in(&fx.build(), "EW-PLC-016", CheckMode::Online).is_empty());
    fx.rec_mut(&us(CFG))["exchange"] = json!({
        "format": "aml", "version": "1.4.0", "file_sha256": format!("sha256:{}", "ab".repeat(32)),
        "direction": "import", "date": "2026-01-02",
    });
    fx.push(json!({
        "$k": "exchange_record", "id": us(0x1900), "config_project": us(CFG), "object": "channel",
        "target": ch(CARD, 0), "fields": {"address": "I99.7", "symbol": "NOT_THE_MODEL_S"},
    }));
    let f = found_in(&fx.build(), "EW-PLC-016", CheckMode::Online);
    assert_eq!(forms(&f), ["drift"], "{f:#?}");
}

/// EW-PRC-002: an instrument-list tag whose device is gone, or that has no channel, is an info; one with its device
/// and channel is not.
#[test]
fn ew_prc_002_reports_an_instrument_tag_without_device_or_channel() {
    let mut fx = card("ai", "contact", &json!({}), &json!({}), &json!({}));
    for (n, target, channel) in [
        (0x1910, us(0x1999), None),
        (0x1911, us(FIELD), None),
        (0x1912, us(OTHER), Some(ch(CARD, 1))),
    ] {
        let mut fields = json!({"tag": format!("FT-{n:x}"), "signal": "AI"});
        if let Some(c) = channel {
            fields["channel"] = json!(c);
        }
        fx.push(json!({
            "$k": "exchange_record", "id": us(n), "config_project": us(CFG), "object": "instrument",
            "target": target, "fields": fields,
        }));
    }
    let f = found_in(&fx.build(), "EW-PRC-002", CheckMode::Online);
    assert_eq!(forms(&f), ["channel:FT-1911", "device:FT-1910"], "{f:#?}");
}
