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

/// A PE-role CP and an M-role CP on one wire, without a definition; `pe_first` gives the PE function the
/// lower ID. Returns the model and the IDs of the PE and the M function.
fn pe_meets_m(pe_first: bool) -> (Model, u128, u128) {
    let (pe, m) = if pe_first {
        (0x1001, 0x1002)
    } else {
        (0x1002, 0x1001)
    };
    let mut fx = Fx::base();
    fx.func_pe(pe, P1);
    fx.func_m(m, P1);
    fx.conn(0x2001, P1, Ep::Cp(pe, 1), Ep::Cp(m, 1), &none());
    (fx.build(), pe, m)
}

#[test]
fn pot001_member_role_conflict_does_not_depend_on_the_id_order() {
    // The two ID orders of one drawing (a new project mints random IDs): the applied role, the text and the
    // targets are the same in both. Before the fix the CP (ID) order chose the applied role ("Pe meet M" in
    // one order) and the one target (the function of the smallest CP), so a properties sheet of the PE
    // terminal showed the finding in one copy of a project and not in the other (blind replay gpp-2).
    for pe_first in [true, false] {
        let (m, pe, mf) = pe_meets_m(pe_first);
        let f: Vec<_> = findings_of(&m, "EW-POT-001")
            .into_iter()
            .filter(|f| f.discriminator.starts_with("attrs"))
            .collect();
        assert_eq!(f.len(), 1, "pe_first {pe_first}: {f:#?}");
        let mut want = vec![target(Kind::Function, pe), target(Kind::Function, mf)];
        want.sort();
        assert_eq!(
            f[0].targets, want,
            "pe_first {pe_first}: every member is a target"
        );
        assert!(
            f[0].message.contains("M meet Pe"),
            "pe_first {pe_first}: {}",
            f[0].message
        );
        assert_eq!(
            f[0].message, "member roles M meet Pe; the first in role order applies",
            "pe_first {pe_first}"
        );
    }
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
    // A potential without a definition targets every member function, whatever their ID order.
    assert_eq!(
        f[0].targets,
        vec![target(Kind::Function, 0x1001), target(Kind::Function, 0x1002)]
    );
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
    // Every member function: the two functions and the two interruption points.
    assert_eq!(f[0].targets.len(), 4);
    assert!(f[0].targets.iter().all(|t| t.kind == Kind::Function));
}

#[test]
fn pot014_agreeing_constituents_are_silent() {
    assert!(findings_of(&conflict_fixture(0x6102, 0x6102), "EW-POT-014").is_empty());
}

/// The second own location of [`rail_model`]'s two-location project.
const LOC2: u128 = 0x901;

/// A strip with the rail-bonded terminal 0x4001 in `location`, with a PE function `pe` when given; the project has
/// the location A, and a second own location B when `two_locations`.
fn rail_model(location: Option<u128>, pe: Option<u128>, two_locations: bool) -> Model {
    let mut fx = Fx::base();
    fx.push(json!({"$k": "structure_node", "id": us(LOC), "aspect": "location", "code": "A"}));
    if two_locations {
        fx.push(json!({"$k": "structure_node", "id": us(LOC2), "aspect": "location", "code": "B"}));
    }
    fx.push(json!({"$k": "device", "id": us(STRIP), "tag": {"letter": "X", "counter": 1}, "strip": {}}));
    fx.rail_terminal(0x4001, STRIP, location, pe);
    fx.build()
}

/// Amended by cycle 33 (G-PP ruling #20): the warning stays in a project with two or more own locations.
#[test]
fn pot008_reports_a_rail_bonded_terminal_without_a_location() {
    let m = rail_model(None, Some(0x1001), true);
    let f = findings_of(&m, "EW-POT-008");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "005008");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].targets, vec![target(Kind::Device, 0x4001)]);
    assert!(f[0].message.contains("no location"), "{}", f[0].message);
    assert!(f[0].message.contains("2 own locations"), "{}", f[0].message);
}

#[test]
fn pot008_a_located_terminal_has_no_finding() {
    let m = rail_model(Some(LOC), Some(0x1001), true);
    assert!(findings_of(&m, "EW-POT-008").is_empty());
}

/// Cycle 33 (G-PP ruling #20), the -X1032:1-4 case of the G-PP run: in a project with one own location (an
/// external reference's location does not count), unlocated rail-bonded terminals belong to it: no EW-POT-008, and
/// their PE connection points join one PE potential through the rail.
#[test]
fn pot008_one_location_project_joins_unlocated_rail_terminals_to_pe() {
    let mut fx = Fx::base();
    fx.push(json!({"$k": "structure_node", "id": us(LOC), "aspect": "location", "code": "A"}));
    fx.push(json!({"$k": "structure_node", "id": us(LOC2), "aspect": "location", "code": "J-E-206"}));
    fx.push(
        json!({"$k": "external_ref", "id": us(0x950), "location": us(LOC2), "device": "JE-N1000-03",
        "connection": "PE"}),
    );
    fx.push(json!({"$k": "device", "id": us(STRIP), "tag": {"letter": "X", "counter": 1}, "strip": {}}));
    for (k, n) in [0x4001u128, 0x4002, 0x4003, 0x4004].into_iter().enumerate() {
        let pe = 0x1001 + k as u128;
        fx.rail_terminal(n, STRIP, None, Some(pe));
        // Each PE CP is wired to its own plain function, so it is a node of the stitch graph.
        fx.func(pe + 0x100, P1);
        fx.conn(0x2001 + k as u128, P1, Ep::Cp(pe, 1), cp(pe + 0x100), &none());
    }
    let m = fx.build();
    assert!(findings_of(&m, "EW-POT-008").is_empty());
    let q = ew_derive::Q::new(&m);
    let part = q.get::<ew_derive::PotentialPartition>(());
    let pe = |f: u128| {
        part.potential_at(ew_model::EndpointKey::Cp(uid(f).cast(), 1))
            .expect("the PE CP is in a potential")
            .id
    };
    for f in [0x1002, 0x1003, 0x1004] {
        assert_eq!(pe(f), pe(0x1001), "the rail joins {f:#x}'s PE CP");
    }
}

/// Two potentials of two wires each: one on the wire of functions 0x1001 and 0x1003 (its definition `def`
/// at 0x1001) and a PE potential on 0x1002 and 0x1004, with `bonds` 0 V-PE bonds between them (the first at
/// the two lower functions, the second at the upper ones).
fn bonded(def: &Value, bonds: usize) -> Model {
    let mut fx = Fx::base();
    for n in [0x1001, 0x1002, 0x1003, 0x1004] {
        fx.func(n, P1);
    }
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1003), &none());
    fx.conn(0x2002, P1, cp(0x1002), cp(0x1004), &none());
    fx.def(0x5001, cp(0x1001), def);
    fx.def(0x5002, cp(0x1002), &json!({"ptype": "pe"}));
    for (i, (a, b)) in [(0x1001u128, 0x1002u128), (0x1003, 0x1004)]
        .into_iter()
        .take(bonds)
        .enumerate()
    {
        fx.push(json!({
            "$k": "bond", "id": us(0x4001 + i as u128), "a": {"cp": [us(a), 0]}, "b": {"cp": [us(b), 0]},
            "kind": "pelv_0v_pe",
        }));
    }
    fx.build()
}

fn pelv_m() -> Value {
    json!({"ptype": "m", "env": {"earthing": "pelv"}})
}

#[test]
fn pot002_reports_a_pelv_potential_without_a_bond() {
    let f = findings_of(&bonded(&pelv_m(), 0), "EW-POT-002");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "005002");
    assert_eq!(f[0].severity, Severity::Error);
    assert_eq!(f[0].discriminator, "pelv");
    assert_eq!(f[0].targets, vec![target(Kind::PotentialDef, 0x5001)]);
    assert!(f[0].message.contains("no 0 V–PE bond"), "{}", f[0].message);
}

#[test]
fn pot002_a_pelv_potential_with_one_bond_has_no_finding() {
    assert!(findings_of(&bonded(&pelv_m(), 1), "EW-POT-002").is_empty());
}

#[test]
fn pot002_reports_a_pelv_potential_with_two_bonds() {
    let f = findings_of(&bonded(&pelv_m(), 2), "EW-POT-002");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].discriminator, "pelv");
    assert!(f[0].message.contains("has 2 0 V–PE bonds"), "{}", f[0].message);
    let bonds = f[0].targets.iter().filter(|t| t.kind == Kind::Bond).count();
    assert_eq!(bonds, 2, "{:?}", f[0].targets);
}

#[test]
fn pot002_reports_a_bonded_selv_potential() {
    let m = bonded(&json!({"ptype": "m", "env": {"earthing": "selv"}}), 1);
    let f = findings_of(&m, "EW-POT-002");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].severity, Severity::Error);
    assert_eq!(f[0].discriminator, "selv");
    assert!(
        f[0].targets.contains(&target(Kind::Bond, 0x4001)),
        "{:?}",
        f[0].targets
    );
}

#[test]
fn pot002_a_pelv_potential_that_is_not_m_type_has_no_finding() {
    let m = bonded(&json!({"ptype": "l_plus", "env": {"earthing": "pelv"}}), 0);
    assert!(findings_of(&m, "EW-POT-002").is_empty());
}
