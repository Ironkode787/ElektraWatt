// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The part texts of a device's symbol slots (FR-DEV-009, FR-DEV-016, doc 07 §9.1 step 4, U-32): the main
//! part, the manufacturer name and the technical-data text.

mod common;

use common::{Fx, LIBRARY, id, uid};
use ew_derive::{Q, main_part, manufacturer_name, tech_data_text};
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
    assert_eq!(manufacturer_name(&p.item.manufacturer), "Eaton");
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
    assert_eq!(manufacturer_name(&p.item.manufacturer), "");
}
