// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `plc_address_text(function)` (WP-S1.21 part 2, D7; doc 10 §3.6, FT-227) on one valve-terminal module whose eight
//! output channels derive Q0.0 … Q0.7: a bistable valve V1 (coil CPs 14 and 12 on its fluid function), a valve V2 whose
//! coil is a function of its own beside the fluid function, and a plain device V3 bound to an excluded channel and to
//! an open-item channel. PT-3's fixture is V1.

mod common;

use common::{Fx, LIBRARY, id, uid};
use ew_derive::{PlcAddressText, PlcAddressTexts, Q};
use ew_ids::v5;
use ew_schema::{Channel, Device};
use serde_json::json;

const CFG: u128 = 0x50;
const STATION: u128 = 0x100;
const M1: u128 = 0x101;
const V1: u128 = 0x201;
const V2: u128 = 0x202;
const V3: u128 = 0x203;
/// V1's bistable fluid function.
const F1: u128 = 0x301;
/// V2's fluid function (no coil CP) and its coil function.
const F2: u128 = 0x302;
const F2_COIL: u128 = 0x312;
/// V3's plain function, its CP 0 on an excluded channel, CP 1 on an open item.
const F3: u128 = 0x303;
/// An unbound function of V3.
const F4: u128 = 0x304;
const BISTABLE: u128 = 0x500;
const PORTS: u128 = 0x501;
const COIL: u128 = 0x502;
const PLAIN: u128 = 0x503;
const PART: u128 = 0x600;

fn channel(n: u16) -> u128 {
    v5::plc_channel_id::<Device, Channel>(id(M1), n)
        .as_uuid()
        .as_u128()
}

fn fixture() -> Fx {
    let mut fx = Fx::new();
    let ports = json!([
        {"idx": 0, "designation": "1"}, {"idx": 1, "designation": "2"}, {"idx": 2, "designation": "3"},
        {"idx": 3, "designation": "4"}, {"idx": 4, "designation": "5"}
    ]);
    let mut bistable = ports.clone();
    // 12 before 14 by index: the slot text still prints 14 first
    bistable.as_array_mut().unwrap().extend([
        json!({"idx": 5, "designation": "12", "physical": false}),
        json!({"idx": 6, "designation": "14", "physical": false}),
    ]);
    fx.lib_item(
        "fndef",
        BISTABLE,
        json!({"meta": {}, "category": ["ValveDirectional", "Valve52", "SolenoidSolenoid"], "contact_kind": "valve",
            "cps": bistable}),
    );
    fx.lib_item(
        "fndef",
        PORTS,
        json!({"meta": {}, "category": ["ValveDirectional", "Valve52", "SolenoidSpring"], "contact_kind": "valve",
            "cps": ports}),
    );
    fx.lib_item(
        "fndef",
        COIL,
        json!({"meta": {}, "category": ["ValveDirectional", "Valve52", "SolenoidSpring"], "contact_kind": "coil",
            "cps": [{"idx": 0, "designation": "14", "physical": false}]}),
    );
    fx.fndef(PLAIN, ["Sensor", "Proximity", "Inductive"], &[(0, "1"), (1, "2")]);
    fx.lib_item(
        "part",
        PART,
        json!({
            "meta": {},
            "kind": "device",
            "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9000), "version_req": "^1"},
            "part_number": "533361",
            "sections": {"plc": {
                "role": "module",
                "io": [{"dir": "out", "bits": 8}],
                "channels": {"count": 8, "kind": "do"},
            }},
        }),
    );
    fx.push(json!({"$k": "plc_config_project", "id": uid(CFG), "name": "PLC1", "profile": "siemens-tia-intl@2027.1"}));
    fx.device(STATION, "U", Some(1)).with(
        "plc_station",
        json!({"config_project": uid(CFG), "role": "valve_terminal"}),
    );
    fx.device(M1, "U", Some(3))
        .with("parent", json!(uid(STATION)))
        .with("position", json!(1))
        .with("parts", json!([{"part": {"copy": uid(PART)}, "qty": "1"}]))
        .with("plc_module", json!({}));
    for (v, p) in [(V1, 1), (V2, 2), (V3, 3)] {
        fx.device(v, "Q", Some(p))
            .with("parent", json!(uid(M1)))
            .with("position", json!(p));
    }
    fx.function(F1, Some(V1), BISTABLE, true);
    fx.function(F2, Some(V2), PORTS, true);
    fx.function(F2_COIL, Some(V2), COIL, false);
    fx.function(F3, Some(V3), PLAIN, true);
    fx.function(F4, Some(V3), PLAIN, false);
    // (number, function, CP index, address)
    let bindings: [(u16, u128, u16, Option<&str>); 5] = [
        (4, F1, 5, None),
        (5, F1, 6, None),
        (2, F2_COIL, 0, None),
        (6, F3, 0, Some("excluded")),
        (7, F3, 1, Some("tbd")),
    ];
    for n in 0..8u16 {
        let mut c =
            json!({"$k": "channel", "id": uid(channel(n)), "module": uid(M1), "number": n, "kind": "do"});
        if let Some((_, f, idx, address)) = bindings.iter().find(|b| b.0 == n) {
            c["binding"] = json!({"cp": [uid(*f), idx]});
            if let Some(a) = address {
                c["address"] = json!(a);
            }
        }
        fx.push(c);
    }
    fx
}

fn texts(q: &Q, f: u128) -> PlcAddressTexts {
    (*q.get::<PlcAddressText>(id(f))).clone()
}

fn pins(list: &[(u16, &str)]) -> Vec<(u16, String)> {
    list.iter().map(|(i, t)| (*i, (*t).to_owned())).collect()
}

/// PT-3: a bistable valve prints its coil addresses in solenoid order 14, 12 joined by " / ", whatever the CP or
/// channel order; each bound CP carries its own address.
#[test]
fn a_bistable_valve_prints_its_coils_in_solenoid_order() {
    let m = fixture().build();
    let q = Q::new(&m);
    assert_eq!(
        texts(&q, F1),
        PlcAddressTexts {
            text: Some("Q0.5 / Q0.4".to_owned()),
            pins: pins(&[(5, "Q0.4"), (6, "Q0.5")]),
        }
    );
}

/// A fluid function without a bound CP of its own prints the bound internal coil CPs of its device's other functions;
/// the coil function prints its own.
#[test]
fn a_fluid_function_prints_the_coils_of_its_device() {
    let m = fixture().build();
    let q = Q::new(&m);
    assert_eq!(
        texts(&q, F2),
        PlcAddressTexts {
            text: Some("Q0.2".to_owned()),
            pins: Vec::new(),
        }
    );
    assert_eq!(
        texts(&q, F2_COIL),
        PlcAddressTexts {
            text: Some("Q0.2".to_owned()),
            pins: pins(&[(0, "Q0.2")]),
        }
    );
}

/// An excluded address prints nothing, an open item prints "TBD"; a function without a binding, a function that is
/// not fluid on a device with bound CPs, and an unknown function print nothing.
#[test]
fn excluded_open_and_unbound() {
    let m = fixture().build();
    let q = Q::new(&m);
    assert_eq!(
        texts(&q, F3),
        PlcAddressTexts {
            text: Some("TBD".to_owned()),
            pins: pins(&[(1, "TBD")]),
        }
    );
    assert_eq!(texts(&q, F4), PlcAddressTexts::default());
    assert_eq!(texts(&q, 0xdead), PlcAddressTexts::default());
}
