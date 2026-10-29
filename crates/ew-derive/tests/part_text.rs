// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The part texts of a device's symbol slots (FR-DEV-009, FR-DEV-016, doc 07 §9.1 step 4, U-32): the main
//! part, the manufacturer name and the technical-data text.

mod common;

use common::{Fx, LIBRARY, id, uid};
use ew_derive::ew_ids::{Id, Uuid, v5};
use ew_derive::ew_schema::Confidence;
use ew_derive::{Q, main_part, manufacturer_name, part_confidence, tech_data_text};
use serde_json::{Value, json};

const P_MCB: u128 = 0x601;
const P_SWITCH: u128 = 0x602;
const P_PLAIN: u128 = 0x603;

fn qty(value: &str, unit: &str) -> Value {
    json!({"value": value, "unit": unit})
}

fn part(fx: &mut Fx, n: u128, number: &str, manufacturer: &str, tech: Value) {
    fx.lib_item(
        "part",
        n,
        json!({
            "meta": {},
            "kind": "device",
            "manufacturer": {
                "library": uid(LIBRARY), "item": uid(0x9000), "version_req": "*", "key": manufacturer,
            },
            "part_number": number,
            "description": "Part",
            "unit": "piece",
            "tech_data": tech,
        }),
    );
}

fn device(fx: &mut Fx, n: u128, parts: &[u128], tech: Value) {
    let uses: Vec<Value> = parts
        .iter()
        .map(|p| json!({"part": {"copy": uid(*p)}, "qty": "1"}))
        .collect();
    fx.push(json!({
        "$k": "device", "id": uid(n), "tag": {"letter": "F", "frozen": {"text": format!("F{n:x}")}},
        "parts": uses, "tech_data": tech,
    }));
}

/// An MCB (characteristic C, 10 A), a switch (40 A) and a part without data; devices with and without
/// their own values.
fn fixture() -> Fx {
    let mut fx = Fx::new();
    part(
        &mut fx,
        P_MCB,
        "A9P52610",
        "Schneider Electric",
        json!([
            {"key": "characteristic", "value": "C"},
            {"key": "rated_current", "value": qty("10", "A")},
        ]),
    );
    part(
        &mut fx,
        P_SWITCH,
        "533356",
        "Eaton",
        json!([{"key": "rated_current", "value": qty("40", "A")}]),
    );
    part(&mut fx, P_PLAIN, "SK 3110.000", "Rittal", json!([]));
    // 0x1000: the MCB; 0x1010: the switch; 0x1020: a thermostat with a setpoint of its own on a plain part;
    // 0x1030: an MCB with a value of its own that overrides the part's rated current; 0x1040: no part, no data.
    device(&mut fx, 0x1000, &[P_MCB], json!([]));
    device(&mut fx, 0x1010, &[P_SWITCH], json!([]));
    device(
        &mut fx,
        0x1020,
        &[P_PLAIN],
        json!([{"key": "setpoint", "value": qty("15", "degC")}]),
    );
    device(
        &mut fx,
        0x1030,
        &[P_MCB],
        json!([{"key": "rated_current", "value": qty("16", "A")}]),
    );
    device(&mut fx, 0x1040, &[], json!([]));
    fx
}

#[test]
fn an_mcb_prints_its_characteristic_and_rated_current_as_c10() {
    let m = fixture().build();
    let q = Q::new(&m);
    assert_eq!(tech_data_text(&q, id(0x1000)), "C10");
}

#[test]
fn quantities_print_with_their_unit_symbol() {
    let m = fixture().build();
    let q = Q::new(&m);
    assert_eq!(tech_data_text(&q, id(0x1010)), "40 A");
    assert_eq!(tech_data_text(&q, id(0x1020)), "15 °C");
}

#[test]
fn the_devices_own_value_wins_over_the_part() {
    let m = fixture().build();
    let q = Q::new(&m);
    // The part says C and 10 A, the device 16 A: B16-style notation with the device's current.
    assert_eq!(tech_data_text(&q, id(0x1030)), "C16");
}

#[test]
fn a_device_without_data_or_part_prints_nothing() {
    let m = fixture().build();
    let q = Q::new(&m);
    assert_eq!(tech_data_text(&q, id(0x1040)), "");
    assert_eq!(tech_data_text(&q, id(0x7777)), "");
    assert!(main_part(&q, id(0x1040)).is_none());
}

#[test]
fn the_main_part_is_the_first_part_use_and_names_its_manufacturer() {
    let m = fixture().build();
    let q = Q::new(&m);
    let p = main_part(&q, id(0x1010)).expect("a main part");
    assert_eq!(p.item.part_number.as_str(), "533356");
    assert_eq!(manufacturer_name(&q, &p.item.manufacturer), "Eaton");
    let p = main_part(&q, id(0x1020)).expect("a main part");
    assert_eq!(p.item.part_number.as_str(), "SK 3110.000");
}

#[test]
fn a_manufacturer_without_a_name_prints_nothing() {
    let mut fx = fixture();
    fx.lib_item(
        "part",
        0x604,
        json!({
            "meta": {}, "kind": "device",
            "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9001), "version_req": "*"},
            "part_number": "X1", "description": "Part", "unit": "piece",
        }),
    );
    device(&mut fx, 0x1050, &[0x604], json!([]));
    let m = fx.build();
    let q = Q::new(&m);
    let p = main_part(&q, id(0x1050)).expect("a main part");
    assert_eq!(manufacturer_name(&q, &p.item.manufacturer), "");
}

/// The copy ID `v5(NS_LIBCOPY, LIBRARY ‖ item)` as text.
fn copy_uid(item: u128) -> String {
    v5::lib_copy_id::<()>(Uuid::from_u128(LIBRARY), Uuid::from_u128(item))
        .as_uuid()
        .to_string()
}

#[test]
fn an_embedded_manufacturer_copy_names_the_manufacturer_before_the_key() {
    // WP-S1.19 D10: the reference resolves to the copy of (library, item); the `key` of an older file is the fallback.
    let mut fx = fixture();
    fx.lib(json!({
        "$k": "manufacturer", "id": copy_uid(0x9000), "library_id": uid(LIBRARY), "item_id": uid(0x9000),
        "version": "1.0.0", "sha256": common::sha(7), "key": "ETN", "name": "Eaton Industries",
    }));
    let m = fx.build();
    let q = Q::new(&m);
    let p = main_part(&q, id(0x1010)).expect("a main part");
    assert_eq!(manufacturer_name(&q, &p.item.manufacturer), "Eaton Industries");
    // Without the copy the key prints, as before.
    let m = fixture().build();
    let q = Q::new(&m);
    let p = main_part(&q, id(0x1010)).expect("a main part");
    assert_eq!(manufacturer_name(&q, &p.item.manufacturer), "Eaton");
}

/// Two parts whose templates provide the main functions of FnDef items 0x50 and 0x51, and a device 0x1070 using both
/// whose main function has the FnDef of item `main_def`, or that has no main function.
fn two_part_device(main_def: Option<u128>) -> ew_model::Model {
    let mut fx = fixture();
    for item in [0x50, 0x51] {
        fx.lib(json!({
            "$k": "fndef", "id": copy_uid(item), "library_id": uid(LIBRARY), "item_id": uid(item),
            "version": "1.0.0", "sha256": common::sha(9), "meta": {}, "category": ["A", "B", "C"],
            "contact_kind": "pole", "cps": [],
        }));
    }
    let template = |item: u128| json!([{"fndef": {"library": uid(LIBRARY), "item": uid(item), "version_req": "*"}, "main": true, "cps": []}]);
    for (n, number, item) in [(0x611, "FIRST", 0x50), (0x612, "SECOND", 0x51)] {
        fx.lib_item(
            "part",
            n,
            json!({
                "meta": {}, "kind": "device",
                "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9000), "version_req": "*", "key": "M"},
                "part_number": number, "description": "Part", "unit": "piece", "function_template": template(item),
            }),
        );
    }
    device(&mut fx, 0x1070, &[0x611, 0x612], json!([]));
    if let Some(item) = main_def {
        let def = Uuid::parse_str(&copy_uid(item)).expect("uuid").as_u128();
        fx.function(0x2070, Some(0x1070), def, true);
    }
    fx.build()
}

fn main_number(main_def: Option<u128>) -> String {
    let m = two_part_device(main_def);
    let q = Q::new(&m);
    let p = main_part(&q, id(0x1070)).expect("a main part");
    p.item.part_number.as_str().to_owned()
}

#[test]
fn the_main_part_provides_the_main_function() {
    // WP-S1.19 D8 (FT-208): the second use's template has a main function with the main function's FnDef.
    assert_eq!(main_number(Some(0x51)), "SECOND");
    // The first use when it provides the main function, or when no use does.
    assert_eq!(main_number(Some(0x50)), "FIRST");
    assert_eq!(main_number(None), "FIRST");
}

#[test]
fn part_confidence_reads_the_section_then_its_head_then_verified() {
    let mut fx = fixture();
    fx.lib_item(
        "part",
        0x620,
        json!({
            "meta": {"confidence": {"sections": "test", "cp_pattern": "unverified"}}, "kind": "device",
            "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9000), "version_req": "*"},
            "part_number": "C1", "description": "Part", "unit": "piece",
        }),
    );
    let m = fx.build();
    let q = Q::new(&m);
    let copy = q
        .parts()
        .get(Id::from_uuid(Uuid::from_u128(0x620)))
        .expect("copy")
        .clone();
    assert_eq!(part_confidence(&copy, "cp_pattern"), Confidence::Unverified);
    assert_eq!(part_confidence(&copy, "sections"), Confidence::Test);
    assert_eq!(part_confidence(&copy, "sections.plc"), Confidence::Test);
    assert_eq!(part_confidence(&copy, "mech"), Confidence::Verified);
}
