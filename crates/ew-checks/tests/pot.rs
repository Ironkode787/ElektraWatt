// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-POT rules over small drawings: potential attribute and flow conflicts, junction feeds, rings,
//! physical-wire conflicts and rail-bonded terminals without a location.

mod common;

use common::*;
use ew_checks::{Severity, Target};
use ew_derive::ew_schema::Kind;
use ew_model::Model;
use serde_json::{Value, json};

const STRIP: u128 = 0x400;
const LOC: u128 = 0x900;

fn none() -> Value {
    json!({})
}

fn target(kind: Kind, n: u128) -> Target {
    Target::new(kind, uid(n))
}

/// Two plain functions on one wire; the potential definitions `first` (lower ID) and `second` sit on them.
fn two_defs(first: &Value, second: &Value) -> Model {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.func(0x1002, P1);
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &none());
    // The higher ID goes first: the ID order decides, not the record order.
    fx.def(0x5002, cp(0x1002), second);
    fx.def(0x5001, cp(0x1001), first);
    fx.build()
}

#[test]
fn pot001_reports_conflicting_definition_attributes_once_per_potential() {
    let m = two_defs(&json!({"voltage": "24"}), &json!({"voltage": "230"}));
    let f = findings_of(&m, "EW-POT-001");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "005001");
    assert_eq!(f[0].severity, Severity::Error);
    assert_eq!(f[0].discriminator, "attrs");
    assert_eq!(f[0].targets, vec![target(Kind::PotentialDef, 0x5001)]);
    assert_eq!(f[0].subject, "P");
    assert!(f[0].message.contains("voltage"), "{}", f[0].message);
}

#[test]
fn pot001_joins_several_conflicts_into_one_finding() {
    let m = two_defs(
        &json!({"voltage": "24", "circuit_class": "control"}),
        &json!({"voltage": "230", "circuit_class": "mains"}),
    );
    let f = findings_of(&m, "EW-POT-001");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert!(f[0].message.contains("; "), "{}", f[0].message);
    assert!(f[0].message.contains("voltage") && f[0].message.contains("circuit class"));
}

#[test]
fn pot001_agreeing_definitions_have_no_finding() {
    let both =
        json!({"ptype": "l_plus", "voltage": "24", "env": {"ac_dc": "dc"}, "circuit_class": "control"});
    let m = two_defs(&both, &both);
    assert!(findings_of(&m, "EW-POT-001").is_empty());
}

#[test]
fn pot001_flow_reports_a_pass_through_between_different_types() {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.func(0x1002, P1);
    fx.pass(0x1003, P1);
    fx.conn(0x2001, P1, cp(0x1001), Ep::Cp(0x1003, 0), &none());
    fx.conn(0x2002, P1, Ep::Cp(0x1003, 1), cp(0x1002), &none());
    fx.def(0x5001, cp(0x1001), &json!({"ptype": "l_plus"}));
    fx.def(0x5002, cp(0x1002), &json!({"ptype": "n"}));
    let m = fx.build();
    let f = findings_of(&m, "EW-POT-001");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].discriminator, "flow");
    assert_eq!(f[0].targets.len(), 2);
    assert!(
        f[0].targets.contains(&target(Kind::Function, 0x1003)),
        "{:?}",
        f[0].targets
    );
    assert!(f[0].message.contains("pass-through"), "{}", f[0].message);
    // The same type on both sides is no conflict.
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.func(0x1002, P1);
    fx.pass(0x1003, P1);
    fx.conn(0x2001, P1, cp(0x1001), Ep::Cp(0x1003, 0), &none());
    fx.conn(0x2002, P1, Ep::Cp(0x1003, 1), cp(0x1002), &none());
    fx.def(0x5001, cp(0x1001), &json!({"ptype": "l_plus"}));
    fx.def(0x5002, cp(0x1002), &json!({"ptype": "l_plus"}));
    assert!(findings_of(&fx.build(), "EW-POT-001").is_empty());
}

#[test]
fn pot001_fingerprint_ignores_the_potential_name() {
    let fingerprint = |name: &str| {
        let m = two_defs(
            &json!({"voltage": "24", "name": name}),
            &json!({"voltage": "230"}),
        );
        let f = findings_of(&m, "EW-POT-001");
        assert_eq!(f.len(), 1, "{f:#?}");
        assert_eq!(f[0].subject, name);
        f[0].fingerprint
    };
    assert_eq!(fingerprint("L1"), fingerprint("L1-renamed"));
}

/// Two junctions J1 (0x3001) and J2 (0x3002), the functions 0x1001 and 0x1002; connection 0x2001 joins
/// the junctions, 0x2002 is J1 -- A and 0x2003 is J2 -- B. `seq1` and `seq2` are the junction sequences.
fn two_junctions(seq1: &[u128], seq2: &[u128]) -> Model {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.func(0x1002, P1);
    fx.conn(0x2001, P1, Ep::J(0x3001), Ep::J(0x3002), &none());
    fx.conn(0x2002, P1, Ep::J(0x3001), cp(0x1001), &none());
    fx.conn(0x2003, P1, Ep::J(0x3002), cp(0x1002), &none());
    fx.junction(0x3001, P1, seq1);
    fx.junction(0x3002, P1, seq2);
    fx.build()
}

#[test]
fn pot011_reports_an_ambiguous_junction_feed() {
    let m = two_junctions(&[0x2001, 0x2002], &[0x2001, 0x2003]);
    let f = findings_of(&m, "EW-POT-011");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "005011");
    assert_eq!(f[0].severity, Severity::Warning);
    // A potential without a definition is identified by one of its functions.
    assert_eq!(f[0].targets.len(), 1);
    assert_eq!(f[0].targets[0].kind, Kind::Function);
    assert!([uid(0x1001), uid(0x1002)].contains(&f[0].targets[0].id));
    assert!(!f[0].discriminator.is_empty());
    assert!(!f[0].message.is_empty());
}

#[test]
fn pot011_a_consistent_junction_tree_is_silent() {
    let m = two_junctions(&[0x2002, 0x2001], &[0x2001, 0x2003]);
    assert!(findings_of(&m, "EW-POT-011").is_empty());
}

#[test]
fn pot012_reports_a_ring_by_its_broken_connection() {
    let mut fx = Fx::base();
    for f in [0x1001, 0x1002, 0x1003] {
        fx.func(f, P1);
    }
    fx.conn(0x2002, P1, cp(0x1001), cp(0x1002), &none());
    fx.conn(0x2003, P1, cp(0x1002), cp(0x1003), &none());
    fx.conn(0x2001, P1, cp(0x1003), cp(0x1001), &none());
    let f = findings_of(&fx.build(), "EW-POT-012");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "005012");
    assert_eq!(f[0].severity, Severity::Warning);
    // The highest connection of the ring forms no wire; its ID is the discriminator.
    assert_eq!(f[0].discriminator, us(0x2003));
}

#[test]
fn pot012_a_chain_has_no_ring() {
    let mut fx = Fx::base();
    for f in [0x1001, 0x1002, 0x1003] {
        fx.func(f, P1);
    }
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &none());
    fx.conn(0x2002, P1, cp(0x1002), cp(0x1003), &none());
    assert!(findings_of(&fx.build(), "EW-POT-012").is_empty());
}

/// A cable with the conductors 0x6101 and 0x6102 and an interruption-point pair joining the functions
/// 0x1001 and 0x1002 through the connections 0x2001 and 0x2002, which carry `core1` and `core2`.
fn conflict_fixture(core1: u128, core2: u128) -> Model {
    let mut fx = Fx::base();
    fx.push(json!({"$k": "device", "id": us(0x6000), "tag": {"letter": "W", "counter": 1}, "cable": {}}));
    for (n, core, csa) in [(0x6101, "1", "1"), (0x6102, "2", "4")] {
        fx.push(json!({"$k": "conductor", "id": us(n), "cable": us(0x6000), "core": core, "csa": csa}));
    }
    fx.func(0x1001, P1);
    fx.func(0x1002, P2);
    fx.ip(0x1101, P1, "X", "source");
    fx.ip(0x1102, P2, "X", "target");
    fx.conn(
        0x2001,
        P1,
        cp(0x1001),
        cp(0x1101),
        &json!({"kind": {"core": us(core1)}}),
    );
    fx.conn(
        0x2002,
        P2,
        cp(0x1102),
        cp(0x1002),
        &json!({"kind": {"core": us(core2)}}),
    );
    fx.build()
}

#[test]
fn pot014_reports_constituents_that_name_different_conductors() {
    let f = findings_of(&conflict_fixture(0x6101, 0x6102), "EW-POT-014");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "005014");
    assert_eq!(f[0].severity, Severity::Error);
    // The wire is carried by the connection at the target point; its ID is the discriminator.
    assert_eq!(f[0].discriminator, us(0x2002));
    assert_eq!(f[0].targets.len(), 1);
}

#[test]
fn pot014_agreeing_constituents_are_silent() {
    assert!(findings_of(&conflict_fixture(0x6102, 0x6102), "EW-POT-014").is_empty());
}

/// A strip with the rail-bonded terminal 0x4001 in `location`, with a PE function `pe` when given.
fn rail_model(location: Option<u128>, pe: Option<u128>) -> Model {
    let mut fx = Fx::base();
    fx.push(json!({"$k": "structure_node", "id": us(LOC), "aspect": "location", "code": "A"}));
    fx.push(json!({"$k": "device", "id": us(STRIP), "tag": {"letter": "X", "counter": 1}, "strip": {}}));
    fx.rail_terminal(0x4001, STRIP, location, pe);
    fx.build()
}

#[test]
fn pot008_reports_a_rail_bonded_terminal_without_a_location() {
    let m = rail_model(None, Some(0x1001));
    let f = findings_of(&m, "EW-POT-008");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "005008");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].targets, vec![target(Kind::Device, 0x4001)]);
    assert!(f[0].message.contains("no location"), "{}", f[0].message);
}

#[test]
fn pot008_a_located_terminal_has_no_finding() {
    let m = rail_model(Some(LOC), Some(0x1001));
    assert!(findings_of(&m, "EW-POT-008").is_empty());
}
