// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The EW-FL MVP rules (WP-S1.20 D16, FL-7, FL-9, FL-12's fixtures): EW-FL-001, -002, -004, -011 and -013, each on a
//! fixture of its defect: a valve module `-1U3` (4 positions, the map 14 on 2p) of the valve terminal `-1U1`, valves
//! with a placed 5/2 body (ports 1 to 5 with their roles) and an internal coil 14, cylinders with consumer ports A and B.

use crate::common;

use common::{Ep, Fx, P1};
use common::{findings_of, us};
use ew_checks::Severity;
use serde_json::{Value, json};

const TERMINAL: u128 = 0x1100;
const MODULE: u128 = 0x1101;
const MODULE_2: u128 = 0x1102;
const VALVE_DEF: u128 = 0x530;
const VALVE_SYM: u128 = 0x531;
const COIL_DEF: u128 = 0x532;
const COIL_SYM: u128 = 0x533;
const CYL_DEF: u128 = 0x534;
const CYL_SYM: u128 = 0x535;
const MODULE_PART: u128 = 0x600;
const VALVE_PART: u128 = 0x601;

/// Valve `n`: the device `n`, its body `n + 0x100` (drawn on P1) and its coil `n + 0x200`.
fn body(n: u128) -> u128 {
    n + 0x100
}

fn coil(n: u128) -> u128 {
    n + 0x200
}

fn channel_id(module: u128, n: u16) -> String {
    ew_derive::ew_ids::v5::plc_channel_id::<ew_derive::ew_schema::Device, ()>(
        ew_derive::ew_ids::Id::from_uuid(ew_derive::ew_ids::Uuid::from_u128(module)),
        n,
    )
    .as_uuid()
    .to_string()
}

fn pneumatic(role: &str) -> Value {
    json!({"discipline": "pneumatic", "port_role": role})
}

/// The definitions, the parts, the valve terminal and the module with its 8 channels (none bound).
fn base() -> Fx {
    let mut fx = Fx::base();
    fx.fndef_with(
        VALVE_DEF,
        VALVE_SYM,
        &[
            (0, "1", pneumatic("supply1")),
            (1, "2", pneumatic("working2")),
            (2, "3", pneumatic("exhaust3")),
            (3, "4", pneumatic("working4")),
            (4, "5", pneumatic("exhaust5")),
        ],
        &json!([]),
        &json!({"contact_kind": "valve"}),
    );
    fx.fndef_with(
        COIL_DEF,
        COIL_SYM,
        &[(0, "14", json!({"physical": false}))],
        &json!([]),
        &json!({"contact_kind": "coil"}),
    );
    fx.fndef_with(
        CYL_DEF,
        CYL_SYM,
        &[(0, "A", pneumatic("consumer")), (1, "B", pneumatic("consumer"))],
        &json!([]),
        &json!({}),
    );
    fx.part(
        MODULE_PART,
        "533361",
        &json!({"kind": "device", "sections": {"plc": {
            "role": "module", "io": [{"dir": "out", "bits": 8}], "channels": {"count": 8, "kind": "do"},
            "positions": {"count": 4, "map": [{"solenoid": "14", "channel": "2p"}]},
        }}}),
    );
    fx.part(
        VALVE_PART,
        "571334",
        &json!({"kind": "device", "sections": {"fluid": {"solenoids": ["14"]}}}),
    );
    fx.drawn_device(TERMINAL, "U", Some(1), P1);
    module(&mut fx, MODULE, 3, 1);
    fx
}

/// A module `n` (tag `-1U<counter>`) at `position` of the valve terminal, with its 8 channels.
fn module(fx: &mut Fx, n: u128, counter: u32, position: u32) {
    fx.drawn_device(n, "U", Some(counter), P1);
    rec(fx, n)["parent"] = json!(us(TERMINAL));
    rec(fx, n)["position"] = json!(position);
    rec(fx, n)["plc_module"] = json!({});
    fx.with_parts(n, &[MODULE_PART]);
    for c in 0..8u16 {
        fx.push(json!({"$k": "channel", "id": channel_id(n, c), "module": us(n), "number": c, "kind": "do"}));
    }
}

/// A valve `n` (tag `-1Q<counter>`) at `position` of `module` in `state`, its body drawn on P1, its coil 14.
fn valve(fx: &mut Fx, n: u128, counter: u32, position: u32, state: Option<&str>) {
    fx.device(n, "Q", Some(counter))
        .with("parent", json!(us(MODULE)))
        .with("position", json!(position));
    if let Some(s) = state {
        fx.with("state", json!(s));
    }
    fx.with_parts(n, &[VALVE_PART]);
    fx.push(json!({"$k": "function", "id": us(body(n)), "device": us(n), "def": us(VALVE_DEF)}));
    fx.push(json!({
        "$k": "representation", "id": us(body(n) + 0x10_000), "functions": [us(body(n))], "page": us(P1),
        "symbol": us(VALVE_SYM), "pos": [0, counter as i64 * 20_000_000],
    }));
    fx.push(
        json!({"$k": "function", "id": us(coil(n)), "device": us(n), "def": us(COIL_DEF), "role": "aux"}),
    );
}

/// A cylinder `n` (tag `-1M<counter>`) drawn on P1.
fn cylinder(fx: &mut Fx, n: u128, counter: u32) {
    fx.device(n, "M", Some(counter));
    fx.push(json!({"$k": "function", "id": us(body(n)), "device": us(n), "def": us(CYL_DEF)}));
    fx.push(json!({
        "$k": "representation", "id": us(body(n) + 0x10_000), "functions": [us(body(n))], "page": us(P1),
        "symbol": us(CYL_SYM), "pos": [100_000_000, counter as i64 * 20_000_000],
    }));
}

fn fluid_line(fx: &mut Fx, n: u128, a: Ep, b: Ep) {
    fx.conn(n, P1, a, b, &json!({"kind": {"fluid": {}}}));
}

fn bind(fx: &mut Fx, module: u128, number: u16, valve: u128) {
    let id = channel_id(module, number);
    let r = fx
        .records
        .iter_mut()
        .find(|r| r["id"] == json!(id))
        .expect("the channel");
    r["binding"] = json!({"cp": [us(coil(valve)), 0]});
}

fn rec(fx: &mut Fx, n: u128) -> &mut Value {
    let id = us(n);
    fx.records
        .iter_mut()
        .find(|r| r["id"] == json!(id))
        .expect("the record")
}

/// The findings of `rule` as (severity, message), by message.
fn messages(fx: &Fx, rule: &str) -> Vec<(Severity, String)> {
    let mut out: Vec<(Severity, String)> = findings_of(&fx.build(), rule)
        .into_iter()
        .map(|f| (f.severity, f.message))
        .collect();
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// The messages of `rule` rendered in nl-NL, sorted.
fn dutch(fx: &Fx, rule: &str) -> Vec<String> {
    let mut out: Vec<String> = findings_of(&fx.build(), rule)
        .iter()
        .map(|f| ew_checks::messages::render(f, "nl-NL"))
        .collect();
    out.sort();
    out
}

const V1: u128 = 0x7001;
const V2: u128 = 0x7002;
const V3: u128 = 0x7003;
const V4: u128 = 0x7004;

/// EW-FL-001 (FL-9's rule): the open working ports of each placed valve, plugged ports and connected ones left out, a
/// not-fitted valve none.
#[test]
fn ew_fl_001_lists_the_open_working_ports() {
    let mut fx = base();
    valve(&mut fx, V1, 1, 1, None);
    valve(&mut fx, V2, 2, 2, Some("spare"));
    valve(&mut fx, V3, 3, 3, Some("not_fitted"));
    valve(&mut fx, V4, 4, 4, None);
    // V2's port 2 is plugged; V4's port 4 has a line to a cylinder
    rec(&mut fx, body(V2))["cp_overrides"] = json!([{"idx": 1, "closure": {"kind": "plug"}}]);
    cylinder(&mut fx, 0x8001, 1);
    fluid_line(&mut fx, 0x4001, Ep::Cp(body(V4), 3), Ep::Cp(body(0x8001), 0));
    assert_eq!(
        messages(&fx, "EW-FL-001"),
        [
            (
                Severity::Warning,
                "-1Q1: working ports 2 and 4 are open; connect, plug or blank them".to_owned()
            ),
            (
                Severity::Warning,
                "-1Q2: working port 4 is open; connect, plug or blank it".to_owned()
            ),
            (
                Severity::Warning,
                "-1Q4: working port 2 is open; connect, plug or blank it".to_owned()
            ),
        ]
    );
    // C50-p6#8: the closure choices' words, "blindstop" and "afgeblind".
    assert_eq!(
        dutch(&fx, "EW-FL-001"),
        [
            "-1Q1: werkpoorten 2 en 4 zijn open; sluit ze aan, zet er een blindstop op of blind ze af",
            "-1Q2: werkpoort 4 is open; sluit hem aan, zet er een blindstop op of blind hem af",
            "-1Q4: werkpoort 2 is open; sluit hem aan, zet er een blindstop op of blind hem af",
        ]
    );
    assert_eq!(
        ew_checks::messages::hint("EW-FL-001", "nl-NL").as_deref(),
        Some("Teken de werkleiding, of sluit de poort af: met een blindstop, met een geluiddemper of afgeblind.")
    );
}

/// EW-FL-002: a solenoid bound to two channels and a channel bound to an undeclared coil are errors; a used valve with
/// an unbound coil a warning; a spare one none.
#[test]
fn ew_fl_002_reports_excess_and_unbound_coils() {
    let mut fx = base();
    valve(&mut fx, V1, 1, 1, None);
    valve(&mut fx, V2, 2, 2, None);
    valve(&mut fx, V3, 3, 3, Some("spare"));
    valve(&mut fx, V4, 4, 4, None);
    bind(&mut fx, MODULE, 0, V1);
    bind(&mut fx, MODULE, 1, V1);
    bind(&mut fx, MODULE, 6, V4);
    // V4's coil is a coil 12, which its part does not declare
    rec(&mut fx, coil(V4))["cp_overrides"] = json!([{"idx": 0, "designation": "12"}]);
    assert_eq!(
        messages(&fx, "EW-FL-002"),
        [
            (
                Severity::Error,
                "-1Q1: solenoid 14 is bound to 2 channels: channel 0 of -1U3, channel 1 of -1U3".to_owned()
            ),
            (Severity::Warning, "-1Q2: solenoid 14 has no channel".to_owned()),
            (
                Severity::Error,
                "-1Q4: channel 6 of -1U3 drives a coil that the part does not declare".to_owned()
            ),
            (Severity::Warning, "-1Q4: solenoid 14 has no channel".to_owned()),
        ]
    );
    assert_eq!(
        ew_checks::messages::title("EW-FL-002", "nl-NL").as_deref(),
        Some("Kanalen en magneetspoelen komen niet overeen")
    );
}

/// EW-FL-004 (EF-14's rule): one info per spare valve and per blanking plate.
#[test]
fn ew_fl_004_lists_spare_valves_and_blanking_plates() {
    let mut fx = base();
    valve(&mut fx, V1, 1, 1, None);
    valve(&mut fx, V2, 2, 2, Some("spare"));
    fx.device(V3, "Q", Some(3))
        .with("parent", json!(us(MODULE)))
        .with("position", json!(3))
        .with("kind", json!("blanking_plate"));
    assert_eq!(
        messages(&fx, "EW-FL-004"),
        [
            (
                Severity::Info,
                "-1U3: position 2 holds a spare valve, -1Q2".to_owned()
            ),
            (
                Severity::Info,
                "-1U3: position 3 holds a blanking plate, -?Q3".to_owned()
            ),
        ]
    );
    // C50-p6#8: "ventielplaats" for a valve slot.
    assert_eq!(
        dutch(&fx, "EW-FL-004"),
        [
            "-1U3: ventielplaats 2 bevat een reserveventiel, -1Q2",
            "-1U3: ventielplaats 3 bevat een blindplaat, -?Q3",
        ]
    );
    assert_eq!(
        (
            ew_checks::messages::title("EW-FL-004", "nl-NL").as_deref(),
            ew_checks::messages::hint("EW-FL-004", "nl-NL").as_deref()
        ),
        (
            Some("Ventielplaatsen in reserve"),
            Some(
                "Houd het reserveventiel of de blindplaat bewust, of plaats een gebruikt ventiel op de ventielplaats."
            )
        )
    );
}

/// EW-FL-011 (FL-10's rule): a `drives` link to another device than the one the working-port lines reach.
#[test]
fn ew_fl_011_reports_a_drives_link_against_the_lines() {
    let mut fx = base();
    valve(&mut fx, V1, 1, 1, None);
    cylinder(&mut fx, 0x8001, 1);
    cylinder(&mut fx, 0x8002, 2);
    fluid_line(&mut fx, 0x4001, Ep::Cp(body(V1), 1), Ep::Cp(body(0x8002), 0));
    fx.push(
        json!({"$k": "device_link", "id": us(0x5001), "from": us(V1), "to": us(0x8001), "kind": "drives"}),
    );
    assert_eq!(
        messages(&fx, "EW-FL-011"),
        [(
            Severity::Warning,
            "-1Q1 drives -1M1, but its working-port lines reach -1M2".to_owned()
        )]
    );
    // the line to the linked cylinder: none
    rec(&mut fx, 0x4001)["b"] = json!({"cp": [us(body(0x8001)), 0]});
    assert!(messages(&fx, "EW-FL-011").is_empty());
}

/// EW-FL-013 (FL-7): a second valve at a position and a valve outside the positions (errors), an empty position (a
/// warning), and two modules at one position of the valve terminal (an error).
#[test]
fn ew_fl_013_reports_the_occupancy_defects() {
    let mut fx = base();
    valve(&mut fx, V1, 1, 1, None);
    valve(&mut fx, V2, 2, 1, None);
    valve(&mut fx, V3, 3, 3, None);
    valve(&mut fx, V4, 4, 5, None);
    module(&mut fx, MODULE_2, 4, 1);
    assert_eq!(
        messages(&fx, "EW-FL-013"),
        [
            (
                Severity::Error,
                "-1U1: position 1 holds both -1U3 and -1U4".to_owned()
            ),
            (
                Severity::Error,
                "-1U3: position 1 holds both -1Q1 and -1Q2".to_owned()
            ),
            (
                Severity::Warning,
                "-1U3: position 2 is empty; fit a valve or a blanking plate".to_owned()
            ),
            (
                Severity::Warning,
                "-1U3: position 4 is empty; fit a valve or a blanking plate".to_owned()
            ),
            (
                Severity::Error,
                "-1U3: position 5 is outside its 4 valve positions".to_owned()
            ),
            (
                Severity::Warning,
                "-1U4: position 1 is empty; fit a valve or a blanking plate".to_owned()
            ),
            (
                Severity::Warning,
                "-1U4: position 2 is empty; fit a valve or a blanking plate".to_owned()
            ),
            (
                Severity::Warning,
                "-1U4: position 3 is empty; fit a valve or a blanking plate".to_owned()
            ),
            (
                Severity::Warning,
                "-1U4: position 4 is empty; fit a valve or a blanking plate".to_owned()
            ),
        ]
    );
    // C50-p6#8: "ventielplaats" for a valve slot; a module's place on the valve terminal stays a "positie".
    assert_eq!(
        dutch(&fx, "EW-FL-013"),
        [
            "-1U1: positie 1 bevat zowel -1U3 als -1U4",
            "-1U3: ventielplaats 1 bevat zowel -1Q1 als -1Q2",
            "-1U3: ventielplaats 2 is leeg; plaats een ventiel of een blindplaat",
            "-1U3: ventielplaats 4 is leeg; plaats een ventiel of een blindplaat",
            "-1U3: ventielplaats 5 ligt buiten de 4 ventielplaatsen",
            "-1U4: ventielplaats 1 is leeg; plaats een ventiel of een blindplaat",
            "-1U4: ventielplaats 2 is leeg; plaats een ventiel of een blindplaat",
            "-1U4: ventielplaats 3 is leeg; plaats een ventiel of een blindplaat",
            "-1U4: ventielplaats 4 is leeg; plaats een ventiel of een blindplaat",
        ]
    );
}

/// EW-FL-013's prevent half (FL-7): a batch that puts a valve outside its module's positions or at a held position is
/// refused with T12 and the free positions, or T13; a valve that already stood there is not refused.
#[test]
fn ew_fl_013_prevents_a_valve_outside_or_at_a_held_position() {
    use ew_derive::Q;
    let mut fx = base();
    valve(&mut fx, V1, 1, 1, None);
    let before = fx.build();
    let mut moved = fx.clone();
    valve(&mut moved, V2, 2, 6, None);
    let touched = [(ew_derive::ew_schema::Kind::Device, common::uid(V2))];
    let messages = |after: &Fx| -> Vec<String> {
        ew_checks::prevent(&Q::new(&before), &Q::new(&after.build()), &touched)
            .into_iter()
            .map(|f| f.message)
            .collect()
    };
    assert_eq!(
        messages(&moved),
        ["-1U3: position 6 is outside its 4 valve positions; free positions: 2, 3, 4"]
    );
    let nl = ew_checks::prevent(&Q::new(&before), &Q::new(&moved.build()), &touched);
    assert_eq!(
        nl.iter()
            .map(|f| ew_checks::messages::render(f, "nl-NL"))
            .collect::<Vec<_>>(),
        ["-1U3: ventielplaats 6 ligt buiten de 4 ventielplaatsen; vrije ventielplaatsen: 2, 3, 4"]
    );
    let mut held = fx.clone();
    valve(&mut held, V2, 2, 1, None);
    assert_eq!(messages(&held), ["-1U3: position 1 holds both -1Q1 and -1Q2"]);
    // a valve at a free position: no violation
    let mut free = fx.clone();
    valve(&mut free, V2, 2, 2, None);
    assert!(messages(&free).is_empty());
}

/// EW-FL-013's prevent half (FL-7, C50-p6#1): a valve added at a position another valve holds is refused whatever the
/// IDs' order, here the new valve's ID being the lower.
#[test]
fn ew_fl_013_prevents_a_lower_id_valve_at_a_held_position() {
    use ew_derive::Q;
    let mut fx = base();
    valve(&mut fx, V2, 2, 1, None);
    let before = fx.build();
    let mut held = fx.clone();
    valve(&mut held, V1, 1, 1, None);
    let touched = [(ew_derive::ew_schema::Kind::Device, common::uid(V1))];
    let messages: Vec<String> = ew_checks::prevent(&Q::new(&before), &Q::new(&held.build()), &touched)
        .into_iter()
        .map(|f| f.message)
        .collect();
    assert_eq!(messages, ["-1U3: position 1 holds both -1Q2 and -1Q1"]);
}
