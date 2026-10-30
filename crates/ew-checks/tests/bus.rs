// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The MVP rules of the EW-BUS pack (WP-S1.21 part 4, D12): EW-BUS-001 (a TBD or duplicate station address in a
//! logical net, superseding EW-CORE-044) and EW-BUS-004 (the termination at the segment ends), on a PROFIBUS line of
//! three ports `-1K8193` — `-1K8194` — `-1K8195` (bus connections, no cable) on one physical and one logical net: the
//! master at address 1 and the two slaves at 2 and 3, the end ports terminated On, the middle one Off.

mod common;

use common::*;
use ew_checks::{Finding, Severity};
use ew_derive::ew_schema::Kind;
use serde_json::{Value, json};

const PNET: u128 = 0x40;
const LNET: u128 = 0x41;
const A: u128 = 0x2001;
const B: u128 = 0x2002;
const C: u128 = 0x2003;

/// The port names as the rules print them.
const NAME_A: &str = "-1K8193:DP";
const NAME_B: &str = "-1K8194:DP";
const NAME_C: &str = "-1K8195:DP";

fn port(role: &str, address: Value, termination: Option<&str>) -> Value {
    let mut p = json!({
        "interface": "DP", "bus": "profibus_dp", "role": role, "address": address,
        "logical_net": us(LNET), "physical_net": us(PNET),
    });
    if let Some(t) = termination {
        p["termination"] = json!(t);
    }
    p
}

/// The line with the ports `(role, address, termination)` of A, B and C.
fn line(ports: [(&str, Value, Option<&str>); 3]) -> Fx {
    let mut fx = Fx::base();
    fx.push(json!({"$k": "physical_net", "id": us(PNET), "name": "DP", "medium": "copper"}));
    fx.push(
        json!({"$k": "logical_net", "id": us(LNET), "bus": "profibus_dp", "name": "DP",
        "master": {"cp": [us(A), 0]}}),
    );
    for (n, (role, address, termination)) in [A, B, C].into_iter().zip(ports) {
        fx.func(n, P1);
        let f = fx
            .records
            .iter_mut()
            .find(|r| r["$k"] == "function" && r["id"] == json!(us(n)))
            .expect("the function");
        f["bus_port"] = port(role, address, termination);
    }
    fx.conn(
        0x3001,
        P1,
        Ep::Cp(A, 1),
        Ep::Cp(B, 0),
        &json!({"kind": {"bus": {}}}),
    );
    fx.conn(
        0x3002,
        P1,
        Ep::Cp(B, 1),
        Ep::Cp(C, 0),
        &json!({"kind": {"bus": {}}}),
    );
    fx
}

fn consistent() -> [(&'static str, Value, Option<&'static str>); 3] {
    [
        ("master", json!({"value": {"profibus": 1}}), Some("on")),
        ("slave", json!({"value": {"profibus": 2}}), Some("off")),
        ("slave", json!({"value": {"profibus": 3}}), Some("on")),
    ]
}

/// (severity, discriminator, subject) of each finding.
fn rows(f: &[Finding]) -> Vec<(Severity, String, String)> {
    let mut out: Vec<(Severity, String, String)> = f
        .iter()
        .map(|f| (f.severity, f.discriminator.clone(), f.subject.clone()))
        .collect();
    out.sort();
    out
}

fn row(severity: Severity, discriminator: &str, subject: &str) -> (Severity, String, String) {
    (severity, discriminator.to_owned(), subject.to_owned())
}

#[test]
fn a_consistent_line_has_no_bus_finding() {
    let m = line(consistent()).build();
    for rule in ["EW-BUS-001", "EW-BUS-004", "EW-CORE-044"] {
        assert!(
            findings_of(&m, rule).is_empty(),
            "{rule}: {:#?}",
            findings_of(&m, rule)
        );
    }
}

/// EW-BUS-004 (BU-3): an end element Off is an error at that end (`end`); an element inside the segment On is an
/// error (`middle`); each with the port as target.
#[test]
fn ew_bus_004_reports_an_end_off_and_a_middle_on() {
    let mut ports = consistent();
    ports[2].2 = Some("off");
    let f = findings_of(&line(ports).build(), "EW-BUS-004");
    assert_eq!(rows(&f), [row(Severity::Error, "end", NAME_C)], "{f:#?}");
    assert_eq!(
        f[0].message,
        format!("{NAME_C}: the termination is off at the segment end; switch it on")
    );
    assert_eq!(f[0].targets.len(), 1);
    assert_eq!(f[0].targets[0].kind, Kind::Function);

    let mut ports = consistent();
    ports[1].2 = Some("on");
    let f = findings_of(&line(ports).build(), "EW-BUS-004");
    assert_eq!(rows(&f), [row(Severity::Error, "middle", NAME_B)], "{f:#?}");
}

/// EW-BUS-004: an end walks inward to the nearest unclaimed terminating element; with one switch in the segment the
/// second end has none left (`missing`, an error at that end). A switch state of a newer format is an info
/// (`unknown`).
#[test]
fn ew_bus_004_reports_a_missing_termination_and_an_unknown_state() {
    let ports = [
        ("master", json!({"value": {"profibus": 1}}), None),
        ("slave", json!({"value": {"profibus": 2}}), None),
        ("slave", json!({"value": {"profibus": 3}}), Some("on")),
    ];
    let f = findings_of(&line(ports).build(), "EW-BUS-004");
    assert_eq!(rows(&f), [row(Severity::Error, "missing", NAME_C)], "{f:#?}");

    let mut ports = consistent();
    ports[2].2 = Some("auto");
    let f = findings_of(&line(ports).build(), "EW-BUS-004");
    assert_eq!(rows(&f), [row(Severity::Info, "unknown", NAME_C)], "{f:#?}");
}

/// EW-BUS-001 (BU-6, EF-03): a TBD station address is one error on its port (`address`) and supersedes
/// EW-CORE-044's finding for the same port; a port without an address is not checked.
#[test]
fn ew_bus_001_reports_a_tbd_address_and_supersedes_ew_core_044() {
    let mut ports = consistent();
    ports[1].1 = json!("tbd");
    ports[2].1 = json!("not_applicable");
    let m = line(ports).build();
    let f = findings_of(&m, "EW-BUS-001");
    assert_eq!(rows(&f), [row(Severity::Error, "address", NAME_B)], "{f:#?}");
    assert_eq!(
        f[0].message,
        format!("{NAME_B}: the station address is TBD; enter it before release")
    );
    assert!(findings_of(&m, "EW-CORE-044").is_empty(), "superseded");
}

/// EW-BUS-001: two ports on one PROFIBUS address are one `duplicate` finding with both ports as targets.
#[test]
fn ew_bus_001_reports_a_duplicate_address_once() {
    let mut ports = consistent();
    ports[2].1 = json!({"value": {"profibus": 1}});
    let f = findings_of(&line(ports).build(), "EW-BUS-001");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].discriminator, "duplicate");
    assert_eq!(f[0].targets.len(), 2);
    assert_eq!(
        f[0].message,
        format!(
            "{NAME_A}, {NAME_C}: station address 1 is used more than once in logical net DP; give each port its own \
             address"
        )
    );
}
